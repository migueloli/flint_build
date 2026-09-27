# Handoff — state of the project and what comes next

**Last updated:** 2026-09-27, on branch `claude/nice-turing-eu8mvu`, after spec 0007 step 1. CI green.
Read this first, then [AGENTS.md](../AGENTS.md) for the rules and [ROADMAP.md](ROADMAP.md) for the full plan.
Update this file whenever you finish a step, change priorities, or leave work half done.

---

## 1. Where things stand

Flint (`flint_build`) is a **fast replacement for `build_runner`**: a Rust engine (parsing, project index,
output ownership) plus a Dart launcher, with generators on top. Built-in generators reproduce the packages
Flutter apps use; custom generators are written by users (Tera today; Dart and YAML in spec 0007). Flint must
coexist with `build_runner` while a project migrates. See SDD §1–2 and the
[generator list](ROADMAP.md#generators).

Today only the first built-in, `flint_json` (json_serializable), exists. It is **experimental but usable for
the common cases**. Output is checked by compiling and round-tripping real Dart in CI.

**Target generators, in the owner's priority order:** json_serializable, riverpod_generator, freezed, drift,
flutter_gen, mockito, go_router_builder, envied. **Later:** auto_route, retrofit, injectable, slang.

| Area | State |
| ---- | ----- |
| Branch / PR | All work is on `claude/nice-turing-eu8mvu`; PR [migueloli/flint_build#1](https://github.com/migueloli/flint_build/pull/1) is open, not merged |
| Version | Engine and CLI both `0.1.0` (unreleased). A bump to `0.2.0` has been suggested, not decided (§5) |
| CI | `.github/workflows/ci.yml`: fmt, clippy `-D warnings`, `cargo test --locked`, MSRV 1.88, Dart golden check, `dart analyze` on `cli/` and the example, example output current. Runs on PRs, pushes to `main`, manual dispatch |
| Tests | 104 Rust tests (unit, build pipeline, insta snapshots, model schema), 32 Dart golden round-trip tests. All pass |
| Performance | `engine/bench/run.sh 1000 5`, 4 cores, engine only: `--force` ~0.27 s, no-op ~86–91 ms (budget 100 ms, tight, see §4) |
| Roadmap | Phase 0 done except the LICENSE placeholders · Phase 1 done · Phase 2 (json_serializable) in progress, spec 0006 · Phase 3 (generator platform) drafted as spec 0007 · Phases 4–7 not started, apart from the parts noted in the roadmap |

### Specs

| Spec | Status | What it did |
| ---- | ------ | ----------- |
| [0001](specs/0001-generated-output-ownership.md) | Done | Flint only writes and deletes `.g.dart` files it owns (marker line); plugins share one file as sections; watch mode fixed |
| [0002](specs/0002-read-build-yaml.md) | Done | json_serializable options come from `build.yaml`; `flint.yaml` is optional |
| [0003](specs/0003-field-rename-camel.md) | Done | `field_rename: camel` is lowerCamelCase; unknown values are errors |
| [0004](specs/0004-template-errors.md) | Done | Template problems are errors, not panics |
| [0005](specs/0005-project-symbol-index.md) | Done | Project symbol index: enums and classes from any file, `dart:core` types, type checks with clear errors, `external_types` |
| [0006](specs/0006-constructor-aware-emission.md) | **In progress: steps 1–2 done, 3–4 open** | `fromJson` calls the real constructor; json_serializable's member rules |
| [0007](specs/0007-generator-platform.md) | **In progress: step 1 done (model v1, `dump-model`, schema), steps 2–7 open** | Generator platform: model v1, one contract for built-in and custom generators (Rust, Dart, YAML, Tera), output kinds, non-Dart inputs, coexistence with build_runner |

### Review findings ([REVIEW.md](REVIEW.md))

- **Fixed:** R1, R2, R3, R4, R5, R6, R7, R12, R13, H2, H3, H4, H5, H6, H7 (golden check; differential tests
  still open), D5.
- **Mostly fixed:** R8 (superclass members left, spec 0006 step 3), R11 (config, templates, engine binary and
  the files a type is declared in count as inputs; content hashes still open), A1 (templates still compiled
  per file).
- **Open:** R9 (`field_annotations` unused; every annotation's named arguments merge into field metadata),
  R10 (`$enumDecode`, `unknownEnumValue`), R14 (non-`String` map keys other than enums), R15 (JSON keys not
  escaped), A2 (hard-coded paths, no `--root`), A3 (parser keeps every class), A4 (parsed and render model
  mixed), A5 (annotation text inconsistencies), A6 (output not `dart format`-shaped), D1 and D2
  (distribution), D3 (end-to-end benchmark), D4 (CLI error message), H1 (LICENSE placeholders).

---

## 2. The next tasks: spec 0007 step 2, then spec 0006 step 3

**Recommended order (changed after spec 0007 step 1):** the model builder (`engine/src/model/build.rs`)
already reads `extends`, `with`, `implements`, getters, setters and every constructor, but `flint_json` still
reads the older `ParsedFile`. Doing **spec 0007 step 2 first** (the `Generator` trait v1: `flint_json` and the
Tera generator move onto the model with byte-identical output, plus the `part`/`library` output kinds and the
`generators:` key) means superclass support (spec 0006 step 3) is then written once, on the model, instead of
on `ParsedFile` and again during the move. The spec 0007 Plan lists step 2's scope; its acceptance criteria
say what must hold (existing snapshots, golden fixtures and the example byte-identical).

### Spec 0006 step 3 (superclass members): the working checklist

Everything below is agreed in the spec. If spec 0007 step 2 is done first, read “parser” and “index” below as
“model” (the model already has `Class.superclass`, `mixins`, and each class's members); the resolution,
dependency and member rules are the same.

**Goal:** `class Child extends Base { Child(super.id, this.name); }` and `Kid(int id, this.name) : super(id)`
generate `Child(id, name)` / `Kid(id, name)..tag = …`, with the superclass's members first in `toJson`.
json_serializable's output for both is recorded in the spec's table.

1. **Parser.** Read the `superclass:` field of `class_declaration` (a `superclass` node holding the type,
   optional `type_arguments`, and an optional `mixins` child). Add something like
   `DartClass.superclass: Option<String>` (the name as written, prefix kept) and
   `DartClass.mixins: Vec<String>`. Use the field name, not a positional capture (AGENTS.md sharp edge, R3).
2. **Index.** `FileSymbols` already keeps `declarations` and `enums`; it needs each class's members
   (fields, getters, setters, static members, superclass, mixins), not its constructors. Keep them
   per class name, like `enums`.
3. **Builder.** For each generated class, resolve the superclass chain with `SymbolIndex::resolve`, *from the
   file that declares each class* (a superclass's own superclass is resolved from the superclass's file).
   Bound the depth and detect cycles (like `namespace` does for exports). Add each file on the chain to the
   output's dependencies, so editing `base.dart` regenerates `child.g.dart` (spec 0005's up-to-date rule;
   see `resolve_field_types` and `is_up_to_date` in `builder.rs`).
4. **Members.** `members::candidates` gets the superclass members prepended, in chain order (the root first),
   then the class's own. A subclass field with the same name wins. `super.x` and plain parameters then
   match inherited members the same way as own ones.
5. **Field types of inherited members** must be resolved from the file that declares them. Their type names
   need to go into `ResolvedTypes` too (enum maps, class checks). Watch for a name that means different
   things in the two files; the simplest safe rule is an error when the same name resolves differently.
6. **Errors** (replace the step 2 “superclass” error): superclass not in this package (unresolved or
   external), generic superclass (`extends Base<T>`), mixins that declare fields. Each names the class and
   suggests `@JsonKey(fromJson:, toJson:)` or a hand-written `fromJson`.
7. **Tests:** a unit test per shape in `members.rs`; build tests for the dependency rule and the errors;
   golden `inheritance_model.dart` (base class in another file, a chain of two, a `super.x`, a plain
   parameter passed to `super(...)`, a mutable superclass field set by cascade), with a round trip matching
   json_serializable's JSON.
8. Then **step 4**: docs (support matrix row “Fields and constructor parameters from a superclass”, template
   context, SDD §4/§6, ROADMAP, REVIEW R8, CHANGELOG), tick the remaining acceptance criteria, mark the spec
   Done.

Budget note: step 3 adds work per generated class, not per file. Re-run `engine/bench/run.sh 1000 5` and
compare with a run of the previous commit in the same session; the machine's noise is ±10 ms.

---

## 3. After that, in recommended order

Each item links to where it's tracked.

1. ~~Owner review of spec 0007~~: accepted with the proposed answers (see its Decisions).
2. **Finish spec 0006** (step 3 in §2, then step 4, docs). Superclass members matter to freezed and most
   other generators too, not only json_serializable.
3. **Continue spec 0007**, step by step (its Plan): ~~model v1 and `dump-model`~~ (done) → trait v1 and output
   kinds (§2) → YAML generators and project scope → non-Dart inputs → Dart generators and the
   `flint_generator` package → coexistence. `flint_json` must move onto the public API with byte-identical
   output (AGENTS.md rule 6).
4. **The next built-in generators**, one spec each, written against spec 0007, in the owner's order:
   **riverpod_generator**, then **freezed**, then drift, flutter_gen, mockito, go_router_builder, envied.
   Before each spec, record the original package's real output (§6), as spec 0006 did for json_serializable.
   drift and mockito also need dependency packages in the index (a separate spec, “0008” in spec 0007).
5. **Index cache (Phase 5, R11).** The no-op build of 1,000 files is at ~90 ms against a 100 ms budget,
   because every build parses every file for the index (spec 0005). A cache in `.dart_tool/flint/` keyed by
   content hash would also fix mtime problems (R11), re-export chains not being tracked, and warnings not
   being repeated on up-to-date builds (all listed in spec 0005's follow-ups). More generators make this more
   urgent. Needs a spec.
6. **json_serializable parity gaps**, one small spec or fix each: R10 (`$enumDecode` and `unknownEnumValue`:
   an unknown enum value throws a bare `StateError` today; check json_serializable's exact output first), R9
   (read field metadata only from `field_annotations`; the structured annotations of spec 0007 help), R14
   (non-`String` map keys: `int`, `DateTime`…), R15 (escape `'`, `$` and `\` in JSON keys),
   `@JsonSerializable(fieldRename:)` per class, `@JsonKey(readValue:, required:, disallowNullValue:)`,
   `genericArgumentFactories: false`, prefixed annotations (`@json.JsonSerializable()`).
7. **Differential tests (rest of H7):** automate what was done by hand for spec 0006: build fixtures with the
   original generator too and compare. Every built-in generator will need this, so build it generically.
8. **Distribution (Phase 6, D1, D2):** the CLI only works inside this repo. Prebuilt binaries, a version
   check, then pub.dev and crates.io.

Owner decisions that don't block engineering are in §5.

---

## 4. Known limitations and risks (not bugs, but worth knowing)

- **No-op budget margin.** ~90 ms of 100 ms on 1,000 files; noisy machines already touch 100 ms. The fix is
  the index cache (§3 item 3), not micro-optimisation.
- **Warnings only on regeneration.** The `external_types` warning is printed when a file is generated, not on
  up-to-date builds (spec 0005).
- **Re-export chains** aren't dependencies: changing a file in the middle of an `export` chain doesn't
  regenerate users of it; `--force` does (spec 0005).
- **Constructor defaults** are copied as source text. Static members of the class are qualified
  (`Limits.defaultLimit`); anything else that isn't visible from a top-level function in the part file (for
  example a static of *another* class used unqualified through an extension) would not compile. It hasn't
  been seen in practice.
- **Hooks and defaults.** A `@JsonKey(fromJson:)` hook gets the raw value, null included, even when the
  constructor or `defaultValue` has a default (unchanged behaviour, recorded in spec 0006).
- **Deliberate deviation from json_serializable:** an optional positional parameter that no member fills,
  before one that is filled, is passed its default. json_serializable shifts later values into its slot.
- **Syntax-only parsing.** Types from other packages aren't read: they're assumed to be classes with
  `fromJson`/`toJson` (with a warning) unless listed in `external_types`.
- **The CLI only works in this monorepo** (D1). The launcher also never rebuilds an existing engine binary;
  run `cargo build --release` in `engine/` after pulling.

---

## 5. Open decisions for the owner

| Decision | Options | Notes |
| -------- | ------- | ----- |
| LICENSE holder and year | — | `LICENSE` has `[YEAR]` and `[COPYRIGHT HOLDER]` |
| Version | stay `0.1.0` until release, or `0.2.0` now | Output changed in breaking ways since the first `0.1.0` notes (constructors, private fields, getters, enum lookup parameter) |
| PR #1 | merge now, or after spec 0006 | The branch head is green; each spec step was pushed and checked separately |
| Scope of the R10 spec | R10 alone, or R10 + R14 (enum/int map keys) | Both touch the enum conversion code |
| Order after freezed | drift, flutter_gen, mockito, go_router_builder, envied (as given) | flutter_gen and envied are small and exercise non-Dart inputs early; drift and mockito need the dependency index first. Reordering by effort is an option |

---

## 6. How to work on this repo

**Always:** `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (from `engine/`), then
`engine/tests/dart_golden/check.sh`, then the example check (`cd cli/example &&
../../engine/target/release/flint_build build --force && git status --short .`). Any change to generated
output gets a snapshot diff you read line by line, a golden fixture, and `/code-review`.

**Dart SDK in a cloud session.** Not preinstalled. Read the version from
`https://storage.googleapis.com/dart-archive/channels/stable/release/latest/VERSION`, download
`…/channels/stable/release/<version>/sdk/dartsdk-linux-x64-release.zip`, unzip it into the scratchpad, and put
`dart-sdk/bin` on `PATH` for `check.sh` (the same steps as in CLAUDE.md). `dart pub get` works through the
proxy.

**Checking json_serializable's real output** (how the spec 0006 table was built; do this before designing any
parity change):

```bash
mkdir -p ref/lib && cd ref
cat > pubspec.yaml <<'YAML'
name: ref
environment: { sdk: ^3.8.0 }
dependencies: { json_annotation: any }
dev_dependencies: { build_runner: any, json_serializable: any }
YAML
# one class per file in lib/, each with `part '<file>.g.dart';`
dart pub get && dart run build_runner build --delete-conflicting-outputs
```

Put classes that should fail in separate files: a json_serializable error stops the whole file.

**Looking at tree-sitter's view of a snippet.** Write a throwaway `engine/examples/sexp.rs` that parses a file
with `tree_sitter_dart::LANGUAGE` and prints each node's kind, field name and text, and run it with
`cargo run --example sexp <file>`. Delete it afterwards. Grammar quirks found this way are listed in
AGENTS.md.

**Benchmark.** `engine/bench/run.sh [files] [runs]` (engine only). When judging a change, run the previous
commit in the same session too: absolute numbers drift by ±10 ms between sessions.

**Commits.** Conventional Commits with a scope (`feat(engine): … (spec 0006 step 3)`); mention review IDs;
one commit per spec step; docs in the same commit as the behaviour (AGENTS.md rule 8).

---

## 7. Feature suggestions and improvements

Not scheduled. The roadmap's “Ideas and suggestions” section has the longer-standing ones (WASM plugins, IDE
integration, error codes, formatting the output). These came out of the work so far.

**For the generator platform (spec 0007 and after):**

- **Scaffolding:** `flint_build new generator <name>` creates a Dart generator package (with
  `flint_generator`, an example model, and a test), so writing one takes minutes.
- **A testing kit for generator authors:** feed a Dart snippet, get the model and the generator's output,
  compare with a golden file. Built on `dump-model`; the same kit tests the built-ins.
- **One conformance harness for every built-in:** run the original package under build_runner and the Flint
  generator on the same fixtures, and compare output behaviour (JSON round trips, `copyWith` results,
  provider values). It generalises the manual recipe in §6.
- **Cache generator responses** by a hash of the request, so unchanged libraries skip Dart generators too.
- **Keep Dart generators warm in watch mode**, and recompile one automatically when its source changes.
- **Discovery:** a pub topic (for example `flint-generator`) and a docs page listing community generators.
- **Migration assistant** (`flint_build migrate`, roadmap Phase 3): read `pubspec.yaml` and `build.yaml`, list
  the builders, say which Flint can take over, write the `flint.yaml`, the `build.yaml` lines that disable
  them, and the `part` directives for coexistence.

**For `flint_json` and the engine:**

- **`flint_build explain <file>`**: print the plan for each class (constructor, which parameter takes which
  key, cascades, dropped members and why). The member rules are json_serializable's and are not obvious;
  a “why is `tags` missing from my JSON?” answer would save users time. `members::Plan` already has the data.
- **Warn when a member is silently dropped.** json_serializable drops unsettable finals without a word;
  Flint could print a note (“`tags` isn't serialized: it's final with an initialiser”) behind a
  `--verbose` flag, or once per class.
- **`toJson` checks for `explicitToJson`.** Spec 0005 checks that nested classes have `fromJson`; the matching
  check for `toJson` when `explicitToJson: true` is missing.
- **A reference-output harness.** Automate §6's recipe: a script that builds the golden fixtures with
  json_serializable in a scratch package and diffs the JSON of both. It's the differential test suite H7 asks
  for, and it would have caught the positional-gap bug in json_serializable itself.
- **Separate render model (A4).** Spec 0006 added a render model for `flint_json` (`JsonClass`, `FromJson`,
  `Value`). Finishing A4 would move `from_json_expr` / `to_json_expr` / `converter` off `DartField`, keeping
  the old fields only for template compatibility.
- **`flint_build doctor`** (roadmap Phase 7) could reuse the index to list unresolved types, missing `part`
  directives and stale outputs, without generating anything.
- **Per-class opt-out of the json_serializable deviations**, if users ask for byte-identical behaviour
  during migration.
- **Watch mode on the cache.** Once the index cache exists, watch can rebuild only files whose dependencies
  changed (roadmap Phase 5).

When one of these is picked up, move it into the roadmap with a phase, and write a spec if it changes
output, config, CLI flags, the template context or the `Generator` trait.
