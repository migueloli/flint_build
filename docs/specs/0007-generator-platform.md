# 0007 — Generator platform: one versioned API for built-in and custom generators

| | |
| --- | --- |
| **Status** | In progress (accepted with the proposed answers to the open questions) |
| **Resolves** | SDD goals 1, 3 and 4 (replace `build_runner`, custom generators, coexistence); SDD open questions 3 and 4; A4, A5 and R9 as side effects of the new model |
| **Touches** | new `model/` module, `parser/`, `index.rs`, `builder.rs`, `output.rs`, `config/`, `generators/` (`Generator` trait, `flint_json`, `generic`), `main.rs` (new command), a new Dart package `flint_generator`, docs |

## Problem

Flint's goal is to replace `build_runner` (SDD §1), with built-in generators for the packages Flutter apps
use (json_serializable, riverpod_generator, freezed, drift, flutter_gen, mockito, go_router_builder, envied;
later auto_route, retrofit, injectable, slang) and custom generators written by users. Today it can't:

1. **The model is json_serializable-shaped.** Templates and the Rust `Generator` trait see classes and enums
   only: no top-level functions (riverpod), no redirecting factories or supertypes (freezed), no method
   signatures (retrofit, mockito), no mixins or extensions. Annotations are flattened into one
   `metadata: {String: String}` map of raw source text, merging the arguments of every annotation on a node
   (R9, A5). Parser output and emitter scratch state share `DartField` (A4).
2. **Custom generators can only be Tera templates.** There's no way to write one in Dart, and Tera can't
   express the logic freezed or riverpod need.
3. **Built-in and custom generators get different inputs.** `flint_json` receives Rust structs plus
   `ResolvedTypes`; templates get a subset. A custom generator can't do what a built-in does.
4. **One output shape.** Every generator writes a section of `<file>.g.dart`. freezed writes its own part
   (`.freezed.dart`); mockito and injectable write libraries that are imported (`.mocks.dart`,
   `.config.dart`); flutter_gen and slang write project-level libraries (`lib/gen/assets.gen.dart`).
5. **Only `.dart` files under `lib/` are inputs.** flutter_gen reads assets and pubspec, envied reads `.env`,
   slang reads translation files, mockito lives in `test/`.
6. **Coexistence breaks on `.g.dart`.** build_runner's source_gen merges every “shared part” generator
   (json_serializable, riverpod_generator, go_router_builder, envied, retrofit, drift) into one `x.g.dart`.
   If Flint takes over json_serializable while build_runner still runs riverpod_generator for the same file,
   both want to own `x.g.dart`.

## Goals / non-goals

- **Goal:** a **generator model v1**: a documented, versioned description of each library that every
  generator receives, whatever language it's written in.
- **Goal:** one **generator contract**: model in; outputs (with kinds) and diagnostics out. Built-in generators
  in Rust use exactly this contract (AGENTS.md rule 6, SDD DD9).
- **Goal:** custom generators in **Dart** (programmatic), **YAML** (declarative selection plus a template), and
  **Tera** (today's `template_path`, unchanged).
- **Goal:** output kinds for the target generators: shared part section, own part file, library next to the
  source, project-level library.
- **Goal:** project-level generators (one output for many inputs) and non-Dart inputs.
- **Goal:** coexistence with `build_runner` during migration, one generator at a time.
- **Non-goal:** running `build_runner` builders or source_gen generators unchanged (SDD DD10).
- **Non-goal:** indexing dependency packages. mockito and drift need it; it gets its own spec (0008) because
  it changes the index and its cost.
- **Non-goal:** implementing the target generators. Each gets its own spec on top of this one.
- **Non-goal:** WASM generators. Revisit if Dart generators' startup cost is a problem.
- **Non-goal:** `flint_build migrate` automation. This spec defines the coexistence rules it would use.

## Behaviour

### Configuration

`generators:` becomes the name of the top-level key; `plugins:` keeps working as an alias, so every existing
`flint.yaml` is still valid. Each generator has exactly one source:

```yaml
generators:
  flint_json:                         # built-in (Rust); options as today
    field_rename: snake

  describe:                           # Tera template, as today
    class_annotations: ["@Describe"]
    template_path: tool/describe.tera

  to_string:                          # YAML generator: declarative selection + template (new)
    select:
      classes: { annotated: ["@ToString"] }
    output: { kind: shared_part }
    template: |
      {% for class in library.classes %}
      String _${{ class.name }}ToString({{ class.name }} v) => '{{ class.name }}(…)';
      {% endfor %}

  copy_with:                          # Dart generator (new)
    dart: package:my_generators/copy_with.dart   # or a path: tool/generators/copy_with.dart
    options: { deep: true }           # passed to the generator as-is
```

### Generator model v1

The model is defined in Rust (`engine/src/model/`), serialized with serde, and published as a JSON Schema
(`docs/model/v1.schema.json`). Tera templates and Dart generators get the same data; Rust built-ins get the
same structs. Its shape, abbreviated:

```text
Library { uri: "package:app/models/user.dart", path: "lib/models/user.dart",
          imports, exports, parts, part_of?,
          classes, enums, mixins, extensions, extension_types, typedefs, functions, variables }

Class   { name, line, doc?, annotations: [Annotation], modifiers: [abstract|sealed|final|base|interface|mixin],
          type_parameters: [{ name, bound?: Type }], superclass?: Type, mixins: [Type], interfaces: [Type],
          fields, getters, setters, methods, constructors, static_members }
Constructor { name?, kind: generative|factory|redirecting_factory|const, redirects_to?: Type,
          params: [Parameter], line, doc?, annotations }
Parameter { name, kind: positional|optional_positional|named, required, default?: Expr,
          initializes: this|super|plain, type?: Type, annotations }
Field   { name, type: Type, is_final, is_late, is_const, has_initializer, is_private, doc?, annotations, line }
Method  { name, return_type?: Type, params, type_parameters, is_async, is_static, is_abstract, annotations, doc? }
Function { same as Method, top level }        Variable { name, type?, is_final, is_const, initializer?: Expr }
Enum    { name, values: [{ name, annotations, arguments? }], fields, constructors, annotations, doc? }

Annotation { name, prefix?, arguments: { positional: [Expr], named: { name: Expr } }, line }
Expr    { source: "'id_'", literal?: { string | int | double | bool | null | list | map } }
Type    { source: "List<m.Money>?", name: "List", prefix?, arguments: [Type], nullable,
          function?: { return_type, params }, record?: { positional, named },
          resolved?: { kind: class|enum|mixin|typedef|extension_type|type_parameter|dart_core|unresolved,
                       library?: "package:app/money.dart", declaration_line? } }
```

Rules:

- **Everything is included**, annotated or not; generators select what they need. (The parser already keeps
  every class, A3.) Doc comments and declaration line numbers are included; method bodies are not.
- **Annotations keep their structure** (fixes R9 and A5): each annotation is separate, with positional and
  named arguments, each as source text plus a literal value when it is one.
- **Types carry what the index resolved** (spec 0005), so a generator can tell an enum from a class.
- **Versioning:** the model has a `model_version`. Additive changes (new optional fields) keep the major
  version; anything else bumps it. A generator declares the major version it supports, and Flint refuses to
  run it against another one with a clear message.
- **Source text on request:** a generator can ask for each declaration's source text (`needs_source: true`)
  for generators that hash or copy it. It is off by default to keep the model small.

`flint_build dump-model lib/models/user.dart` prints a file's model as JSON. It's the debugging tool for
template and generator authors, and the way to write a generator in any other language.

### Generator contract

Every generator, whatever its language, is called with:

```text
Request  { model_version, generator: name, options: {...}, scope: library | project,
           libraries: [Library], inputs: [InputFile], package: { name, root, pubspec: {...} } }
Response { outputs: [Output], diagnostics: [{ severity: error|warning|info, message, path?, line? }] }
Output   { kind: shared_part | part | library, path?, extension?, for_library?, content }
```

- **Selection.** A generator says what it wants: annotations on classes, enums, functions, fields, variables,
  mixins or extensions; for project-level generators, also input globs. Built-in and Dart generators declare
  defaults (in code, reported to Flint when they start); `select:` in `flint.yaml` overrides them. A library
  with nothing selected isn't sent.
- **Scope.** `library` generators get one library at a time, may run in parallel, and produce outputs for
  that library. `project` generators (injectable, auto_route's router, flutter_gen, slang) get every selected
  library and input in one call and produce project-level outputs.
- **Purity.** The same request must give the same response (AGENTS.md rule 2). Flint may skip a call whose
  inputs didn't change, and may call generators in any order.
- **Errors** in diagnostics are per file, as today (spec 0001): the affected outputs are left unchanged and
  the build exits non-zero.

### Output kinds

| Kind | File | Used by | Header |
| ---- | ---- | ------- | ------ |
| `shared_part` | a section of `<file>.g.dart` (or the coexistence extension, below), with sections in config order | json_serializable, riverpod_generator, go_router_builder, envied, retrofit, drift | one header, ownership marker, `part of` (today) |
| `part` | `<file><extension>`, e.g. `.freezed.dart`, `.gr.dart`; one generator per file | freezed, auto_route | marker, `part of` |
| `library` next to a source | `<file><extension>`, e.g. `.mocks.dart`, `.config.dart` | mockito, injectable | marker only, no `part of` |
| `library` at a path | a fixed path, e.g. `lib/gen/assets.gen.dart` (project scope only) | flutter_gen, slang | marker only |

The ownership rules of spec 0001 apply to every kind: Flint writes a file only if it's absent or carries the
marker, and deletes only marked files it no longer produces. A `part`/`shared_part` output still needs the
matching `part '…';` directive in the source; without it, Flint warns and writes nothing, as today.

### Non-Dart inputs

A project-scope generator can declare input globs, relative to the package root (`assets/**`, `.env*`,
`i18n/*.json`). It receives them as `InputFile { path, text? , size }` (text for UTF-8 files up to a size
limit; binary files by path and size only), plus the parsed `pubspec.yaml`. Changing a matching file makes
that generator's outputs stale.

### Roots

Sources come from `lib/` as today, plus `test/` for generators that select test files (mockito). Wider root
configuration (A2: `--root`, globs, `bin/`, workspaces) stays in Phase 6.

### Dart generators

A Dart generator is a Dart library with a `main` built on the new package `flint_generator`:

```dart
import 'package:flint_generator/flint_generator.dart';

void main(List<String> args) => runGenerator(CopyWithGenerator());

class CopyWithGenerator extends FlintGenerator {
  @override
  Selection get selection => Selection.classes(annotated: ['CopyWith']);

  @override
  GeneratorResult generate(GeneratorRequest request) {
    final out = StringBuffer();
    for (final library in request.libraries) {
      for (final c in library.classes.where((c) => c.hasAnnotation('CopyWith'))) {
        out.writeln('extension \$${c.name}CopyWith on ${c.name} { … }');
      }
    }
    return GeneratorResult.sharedPart(out.toString());
  }
}
```

- **One process per build, not per file.** Flint starts the generator once, sends requests over stdin and
  reads responses from stdout (newline-delimited JSON), and closes it at the end of the build. Watch mode keeps
  it running between builds.
- **Compiled once.** Flint compiles the generator with `dart compile exe` into `.dart_tool/flint/generators/`,
  keyed by a hash of the generator's sources, the project's `pubspec.lock`, and the Dart SDK version, and
  reuses the binary until one of them changes. Compiling needs the Dart SDK, which a Flutter project has.
- **Failures are contained.** A crash, a malformed response or a timeout is an error for that generator's
  outputs only; other generators still run (like broken templates, spec 0004).
- `flint_generator` provides the typed model classes (generated from the JSON Schema, so they can't drift from
  the Rust side), `Selection`, `GeneratorResult`, and helpers for common output code (type names, imports,
  string escaping).

### YAML generators

`select:` plus `template:` (inline) or `template_path:`. The template context is the request: `library`
(library scope) or `libraries` and `inputs` (project scope), `options`, `package`. Tera gets helper filters
for casing (`snake_case`, `camel_case`, `pascal_case`) and types (`type.source`, `nullable`, `non_null`), and
the context carries `context_version`. Existing template variables (`classes`, `enums`, `resolved_types`,
`filename`) stay for templates without `select:`.

### Coexistence with `build_runner`

- **Ownership already protects both sides:** Flint never touches a file without its marker, and build_runner's
  files don't carry it. Files Flint writes are left alone by build_runner, *provided the matching builder is
  disabled there* (otherwise build_runner reports them as conflicting outputs).
- **Shared parts:** when a project still runs any shared-part generator under build_runner, Flint writes its
  shared part as `<file>.flint.dart` instead of `<file>.g.dart`. Set with
  `shared_part_extension: .flint.dart` in `flint.yaml`; the source then needs `part '<file>.flint.dart';`
  (Flint's usual “missing part directive” warning says so). Default stays `.g.dart`.
- **Own-part and library outputs** (`.freezed.dart`, `.mocks.dart`) don't collide, once the builder is
  disabled in `build.yaml`. Flint's warning for an unmarked existing output names the builder to disable.
- **Detection:** when `pubspec.yaml` has `build_runner` and a builder that Flint is configured to replace, and
  `build.yaml` doesn't disable it, Flint prints one warning with the `build.yaml` lines to add.

## Design

1. **`model/`** holds the v1 structs (serde), built from the parser output and the index, plus the schema
   export. `ParsedFile` becomes internal to the parser; the model is what leaves it. A4 (scratch state on
   `DartField`) goes away because generators build their own render data (as `flint_json` already does with
   `JsonClass`).
2. **`Generator` trait v1:** `fn selection(&self, options) -> Selection` and
   `fn generate(&self, request: &Request) -> Result<Response, FlintError>`. Built-ins (`flint_json`), Tera,
   YAML and the Dart host all implement it. `flint_json` moves onto it with byte-identical output; its
   `ResolvedTypes` input becomes the `resolved` data on each `Type`.
3. **Builder:** per library, build the model once; run library-scope generators in parallel (rayon); run
   project-scope generators after, with every selected library; assemble outputs by kind in `output.rs`;
   apply ownership per file.
4. **Dart host** (`generators/dart.rs`): compile cache, process lifecycle, framing, timeouts, crash handling.
   Requests are batched: all selected libraries of a build go to the process in one stream, and it answers
   per library, so one process serves the whole build.
5. **`flint_generator`** lives in this repo (`packages/flint_generator/`), versioned with the model; its model
   classes are generated from `docs/model/v1.schema.json` in CI, and CI fails if they're stale.
6. **SDD:** §1 and §2 already describe the platform; this spec updates §5 (architecture), §5.2 (the trait),
   §6 (the model), §7 (pipeline with project-scope generators), §9 (output kinds), §12 (non-Dart inputs as
   dependencies) and DD5.

## Acceptance criteria

- [ ] `flint_build dump-model` prints the model for every golden fixture; snapshots are reviewed; the output
      validates against `docs/model/v1.schema.json`.
- [ ] `flint_json` runs on the v1 trait with byte-identical output for every existing snapshot, golden
      fixture and the example.
- [ ] Everything `flint_json` reads is in the model (no private shortcuts, AGENTS.md rule 6), checked by
      review.
- [ ] A Dart generator in this repo (`packages/flint_generator/example/`) is configured in a golden package,
      its output analyzes cleanly and its round-trip tests pass, and it is started **once** per build
      (counted in a test).
- [ ] A YAML generator with an inline template and `select:` works, including on top-level functions.
- [ ] Output kinds: a golden fixture per kind (shared part, own part, library next to a source, project-level
      library) with the ownership rules tested (`clean`, stale deletion, unmarked files left alone).
- [ ] A project-scope generator reading non-Dart inputs (a flutter_gen-like asset list) regenerates when an
      asset is added.
- [ ] Coexistence: in a test package with an unmarked `x.g.dart` (build_runner's) and
      `shared_part_extension: .flint.dart`, Flint writes `x.flint.dart` and never touches `x.g.dart`.
- [ ] Expressiveness check: small demo generators (in the repo's tests, not products) show that the model
      covers a riverpod-style function provider (return type, parameters), a freezed-style redirecting
      factory and union, and a retrofit-style abstract method with annotated parameters.
- [ ] With no Dart generators configured, the 1,000-file no-op benchmark is unchanged; with one, the added
      time per build is measured and recorded (rule 9), and a warm build doesn't recompile it.

## Plan

Each step is mergeable on its own and keeps CI green.

1. ✅ **Model v1 and `dump-model`:** structs, conversion from the parser and index, JSON Schema, snapshots.
   Structured annotations are added alongside the old `metadata` map. No output changes. Notes:
   - `engine/src/model/`: the v1 structs (`mod.rs`), a type-text parser (`types.rs`: prefixes, generics,
     `?`, function types, records), and a builder (`build.rs`) that walks the syntax tree directly. The
     top level is read as a sequence, because the grammar puts doc comments, annotations and a
     declaration's tokens side by side (a variable is loose tokens ending in `;`). The build pipeline doesn't
     use the model yet (step 2), so builds are unaffected; the benchmark is unchanged.
   - `flint_build dump-model [files…]` prints the model as JSON; `--schema` prints the JSON Schema, which is
     committed as `docs/model/v1.schema.json`. A test fails when the committed schema is stale.
   - **Tests:** a snapshot of a fixture with every kind of declaration
     (`engine/tests/fixtures/model/`), reviewed; every golden fixture's model validated against the schema
     and round-tripped through JSON. Per-fixture snapshots of the golden package would be thousands of lines
     nobody reviews, so validation stands in for them.
   - **Refinements to the shape above:** top-level getters are in `Library.getters`; static members are
     included with `is_static`; setters are in `setters`; operators are methods with `is_operator`;
     `Resolved` has no declaration line yet; an ambiguous name resolves to `unresolved` (the build reports
     the ambiguity).
   - **Bug found and fixed (spec 0006 code):** a constructor default made of several syntax nodes
     (`this.mood = Mood.calm`, `const Duration(seconds: 1).inSeconds`) was cut to its first node (`Mood`), so
     `flint_json` generated code that didn't compile. Both parsers now take everything up to the next `,` or
     closing bracket; covered by a parser test and a golden round trip (`Defaults`).
   - **Code review** (`/code-review`) found ten issues, all fixed and covered by the model fixture or
     `tests/model_test.rs`: positional annotation arguments cut to their first node (`@Default(Mood.calm)`);
     enum-constant doc comments leaking onto the first member; `dart:core` names checked before the
     package's own declarations (a local `Error` was reported as core; the index is now asked first, and the
     core list is longer); a top-level setter read as a function; typedef annotations dropped; `@Foo.named()`
     read as prefix `Foo` (annotations gained `constructor`, decided by Dart's naming convention); files
     outside `lib/` and absolute paths not resolved; one syntax error aborting the whole dump (now reported
     per file); the golden schema test using the wrong package name, so cross-file resolution was never
     exercised; and every file parsed twice, with the optional-parameter walk duplicated (now one shared
     `optional_parameter_parts`, used by both parsers).
2. **`Generator` trait v1 and output kinds:** move `flint_json` and the Tera generator onto it (byte-identical);
   `part` and `library` output kinds with ownership; `generators:` key with the `plugins:` alias.
3. **YAML generators and project scope:** `select:`, inline templates, Tera helpers, `context_version`;
   project-scope calls; `test/` as a root for generators that select it.
4. **Non-Dart inputs:** input globs, `InputFile`, parsed pubspec, dependency tracking.
5. **Dart generators:** `packages/flint_generator` (model classes generated from the schema), the Dart host,
   compile cache, example generator and golden package, failure handling.
6. **Coexistence:** `shared_part_extension`, build_runner detection and warnings, docs.
7. **Docs:** configuration reference (generators, output kinds, YAML and Dart generators), a “write a generator”
   guide, SDD sections, roadmap; spec Done.

## Decisions

The open questions were accepted with the proposed answers:

1. **Config key:** `generators:`, with `plugins:` kept as an alias.
2. **Selection** is declared by the generator (defaults reported at start-up) and can be overridden in
   `flint.yaml`.
3. **Dart transport:** one AOT-compiled process per build over stdin/stdout, kept alive in watch mode; a
   daemon only if measurements call for it.
4. **Coexistence:** an explicit `shared_part_extension`; no automatic switching.
5. **Built-in generators** in the priority list are written in Rust, using only the public model.
6. **Model size:** doc comments and every declaration always; source text on request.
7. **Resolution data** is carried on each `Type` (`resolved`).

## Open questions (resolved)

1. **Config key:** rename `plugins:` to `generators:` with `plugins:` as an alias? *Proposed: yes; the docs
   and messages use “generator”.*
2. **Who declares selection:** the generator (defaults, reported at start-up) with `flint.yaml` overrides,
   or `flint.yaml` only? *Proposed: the generator, with overrides, so a Dart generator package works with
   a one-line config.*
3. **Dart transport:** one AOT-compiled process per build over stdin/stdout (proposed), or `dart run` per build
   (simpler, ~300 ms VM startup each), or a persistent daemon across builds? *Proposed: AOT per build, kept
   alive in watch mode; revisit a daemon if measurements call for it.*
4. **Coexistence default:** explicit `shared_part_extension` (proposed), or switch automatically when
   build_runner is detected? Automatic switching would rename outputs of projects that keep build_runner in
   `pubspec.yaml` for other reasons.
5. **Built-in generators' language:** Rust for the priority list (proposed: speed, one binary to ship), with
   the rule that they use only the public model; or Dart, so they double as examples? A mix is possible:
   simple ones (envied) could be Dart examples.
6. **Model size:** include doc comments and all declarations always (proposed), with source text on request?
   Large projects send more JSON to Dart generators; library-scope requests only include selected libraries.
7. **Model location of resolution data:** `resolved` on every `Type` (proposed) or a per-library table like
   today's `resolved_types`? Per type is easier to use; a table is smaller.
