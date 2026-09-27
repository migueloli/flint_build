# Roadmap

This is the plan for taking Flint from a working prototype to a fast replacement for `build_runner` that you
can safely run on real Flutter apps: a generator platform, the built-in generators Flutter apps rely on
(see [Generators](#generators)), custom generators in Dart, YAML or templates, and coexistence with
`build_runner` while a project migrates.
Finding IDs (R1, D3, …) refer to [REVIEW.md](REVIEW.md). Larger items get a spec in [specs/](specs/) before any
code is written (see the [spec workflow](specs/README.md)).

Status: ⬜ not started · 🟨 in progress · ✅ done

**Current state and the next task:** see [HANDOFF.md](HANDOFF.md).

Phases were renumbered on 2026-09-27 when the generator platform (Phase 3) and built-in generators (Phase 4)
were added. Specs 0001–0006 were written before that: their “Phase 3” (cache, incremental builds) is now
Phase 5, and “Phase 4” (distribution) is now Phase 6.

---

## Phase 0: Hygiene (small, unblocks everything)

| | Item | Refs |
| --- | ---- | ---- |
| 🟨 | Add a `LICENSE` file (MIT, matching `Cargo.toml`). Added with placeholders: fill in `[YEAR]` and `[COPYRIGHT HOLDER]` | H1 |
| ✅ | `cargo fmt --check` passes | H3 |
| ✅ | `cargo clippy -- -D warnings` passes (`FromStr` impls for `FlintConfig`/`Pubspec`, collapsed `if`s); CI enforces it | H3 |
| ✅ | Add `rust-version = "1.88"` to `engine/Cargo.toml` (verified: 1.88 builds, 1.87 doesn't; a CI job checks it) | H4 |
| ✅ | CI workflow (`.github/workflows/ci.yml`): `cargo fmt --check`, `clippy -D warnings`, `cargo test --locked` (fails on snapshot changes), a Rust 1.88 build, the Dart golden check, `dart analyze` on `cli/` and the example, and a check that the example's committed output is current | H2 |
| ✅ | Move `engine/flint.yaml` to `engine/tests/fixtures/`. Snapshot tests should use the built-in template | H5 |
| ✅ | Use `tempfile` in tests instead of fixed temp paths | H6 |
| ✅ | Fix `cli/pubspec.yaml` metadata (description, version `0.1.0` like the engine, repository, `publish_to: none` until D1) and `CHANGELOG.md`; move the example to `dev_dependencies`; drop unused dependencies | D5 |
| ✅ | Accurate READMEs, SDD, configuration reference, agent instructions | H4 |

## Phase 1: Safe to run (P0)

Goal: Flint never damages a project, and generated code compiles for everything it claims to support.

| | Item | Refs |
| --- | ---- | ---- |
| ✅ | **Dart golden harness** (`engine/tests/dart_golden/check.sh`): generate fixtures, `dart analyze --fatal-infos`, and `dart test` round-trips, in CI | H7 |
| ✅ | **Output ownership:** a header marker; only write when a `part` directive exists; only delete owned files | R1, R2 · [Spec 0001](specs/0001-generated-output-ownership.md) |
| ✅ | **Multiple plugins per file:** parse once, concatenate sections in config order (`IndexMap`) | R4, A1 · Spec 0001 |
| ✅ | Watch mode ignores `.g.dart` paths and access events (one rebuild per change) | R5 · Spec 0001 |
| ✅ | Read *all* annotations on classes, fields, enums and enum constants (walk the nodes instead of one optional query capture); an enum constant's value comes only from `variant_annotations` | R3 |
| ✅ | Keep the literal's kind in annotation arguments; emit typed `@JsonValue` maps | R6 |
| ✅ | `Result`-returning generators and collected errors; no panics on bad templates | R12 · [Spec 0004](specs/0004-template-errors.md) |

## Phase 2: json_serializable parity (the first built-in generator, `flint_json`)

Goal: the example app, and a realistic mid-size app, build with Flint and pass the same round-trip tests as
with json_serializable. Much of this work (the index, constructors, the golden harness) is shared by every
later generator.

| | Item | Refs |
| --- | ---- | ---- |
| ✅ | **Project symbol index:** resolve enums and classes across files; clear “unresolved type” diagnostics; `external_types` | R7 · SDD §4 · [Spec 0005](specs/0005-project-symbol-index.md) |
| 🟨 | **Constructor-aware emission:** positional/named/`this.` params; skip static, late and initialised fields; private-field rules (done, steps 1–2); superclass fields (step 3) | R8 · [Spec 0006](specs/0006-constructor-aware-emission.md) |
| 🟨 | Scope metadata to configured `field_annotations` / `variant_annotations` (`variant_annotations` done with R3) | R9 |
| ⬜ | Emit `$enumDecode` / `$enumDecodeNullable`; support `unknownEnumValue` | R10 |
| 🟨 | More types: `num`, `dynamic`, `Object`, `Uri`, `BigInt`, `Duration`, `Set`, `Iterable` (done, spec 0005 step 3); non-String map keys still open | R7, R14 |
| ⬜ | Escape JSON keys; drop identity conversions and `ignore_for_file: unnecessary_cast` | R15, A6 |
| ⬜ | **Differential test suite** against json_serializable, and a **parity matrix** in `configuration.md` | H7 |
| ✅ | Rename `--delete-conflicting-outputs` to `--force` (the old name stays as an alias) | R13 · Spec 0001 |
| ✅ | Read json_serializable options from `build.yaml`; `flint.yaml` optional for json_serializable projects | [Spec 0002](specs/0002-read-build-yaml.md) |
| ✅ | `field_rename: camel` means lowerCamelCase; unknown `field_rename` values are errors | [Spec 0003](specs/0003-field-rename-camel.md) |
| ⬜ | `@JsonSerializable(fieldRename: …)` per class | — |
| ⬜ | `@JsonKey(readValue:, required:, disallowNullValue:)`, `@JsonSerializable(checked:)`, `genericArgumentFactories: false` | Spec 0006 non-goals |
| ⬜ | Check `toJson` exists on nested classes when `explicitToJson: true` (the `fromJson` check exists, spec 0005) | Spec 0005 follow-ups |
| ⬜ | Match prefixed annotations (`@json.JsonSerializable()`) | SDD §5.1 |

## Phase 3: Generator platform

Goal: generators, built-in or custom, are written against one versioned API, in Rust, Dart, YAML or Tera, and
Flint can run next to `build_runner` during a migration.
→ [Spec 0007](specs/0007-generator-platform.md) (accepted)

| | Item | Refs |
| --- | ---- | ---- |
| 🟨 | **Generator model v1:** a versioned, documented model of each library (classes, enums, mixins, extensions, typedefs, top-level functions and variables, constructors, supertypes, generics, annotations with structured arguments, doc comments, resolved types). Built (step 1); generators move onto it in step 2 | Spec 0007 · A4, A5, R9 |
| ✅ | `flint_build dump-model <file>`: print the model as JSON (debugging, and a way to write generators in any language); `--schema` for the JSON Schema | Spec 0007 step 1 |
| ⬜ | **Output kinds:** a section of the shared part (today), a generator's own part file (`.freezed.dart`), a standalone library (`lib/gen/assets.gen.dart`), all with the ownership marker | Spec 0007 · DD5 |
| ⬜ | **Dart generators:** a `flint_generator` package with typed model classes; Flint compiles the generator once (AOT, cached) and runs it once per build, not per file | Spec 0007 |
| ⬜ | **YAML generators:** declarative selection (annotations on classes, functions, fields…) plus inline or file templates; Tera helpers for casing and types; `context_version` | Spec 0007 |
| ⬜ | **Non-Dart inputs:** generators declare input globs (assets, `.env`, translation files) | Spec 0007 |
| ⬜ | **Coexistence with `build_runner`:** a separate shared part for Flint during migration; guidance (or automation) to disable migrated builders in `build.yaml` | Spec 0007 · SDD §16 |
| ⬜ | `flint_build migrate`: list a build_runner project's generators, which Flint can take over, and switch them file by file | Spec 0007 follow-up |
| ⬜ | Index declarations from **dependency packages** (read-only, syntax-only, via `.dart_tool/package_config.json`), for mockito, drift and cross-package types | Later spec · SDD §4 |

## Phase 4: Built-in generators

Goal: the generators in [Generators](#generators), each matching the original package's output and behaviour,
proven by golden fixtures and a comparison against the original. Each gets its own spec, written against the
spec 0007 API.

## Phase 5: Incremental and fast at scale

| | Item | Refs |
| --- | ---- | ---- |
| 🟨 | Compile queries and templates once; discover once; parse once for all plugins (queries, discovery and parsing done; templates still per file) | A1, A3 |
| 🟨 | Content-hash cache in `.dart_tool/flint/` with an engine/config/template fingerprint (mtime-based version of the fingerprint and stale-output deletion shipped with spec 0001) | R11 · SDD §12 |
| ⬜ | **Index cache** so a no-op build doesn't parse every file (no-op on 1,000 files is ~90 ms of a 100 ms budget); also tracks re-export chains and replays warnings on up-to-date builds | Spec 0005 follow-ups · [HANDOFF §3](HANDOFF.md#3-after-that-in-recommended-order) |
| ⬜ | Watch mode rebuilds only the dirty set, including files that depend on changed symbols | R5 · SDD §12 |
| ⬜ | `build --check` for CI (non-zero exit if outputs are stale) | — |
| ⬜ | **Benchmark rewrite:** synthetic 10/100/1000-model projects; `hyperfine`; report engine-only *and* end-to-end; define cold/warm | D3 |

## Phase 6: Installable by anyone

| | Item | Refs |
| --- | ---- | ---- |
| ⬜ | Release pipeline: prebuilt binaries per target, SHA-256 checksums, GitHub Releases | D1 · SDD §13 |
| ⬜ | CLI: find the binary via override → cache → download → cargo (dev); version check | D1, D2, D4 |
| ⬜ | `--root`, include/exclude globs, `test/` and `bin/` roots, pub workspaces / melos | A2 |
| ⬜ | Publish `flint_build` to pub.dev and crates.io. pub.dev needs a `LICENSE` inside `cli/` too | D1 |

## Phase 7: Ecosystem

| | Item |
| --- | ---- |
| ⬜ | `flint_build doctor`: check `part` directives, unresolved types, stale outputs, version mismatch |
| ⬜ | Publish a guide and template repo for writing Flint generators in Dart |
| ⬜ | A place to list community generators (a pub topic, or a page in the docs) |

## Generators

The generators Flint aims to replace, in priority order. “Needs” is what each one requires beyond what exists
today; items in bold are platform work shared with other generators.

| Priority | Package | Status | Output | Needs |
| -------- | ------- | ------ | ------ | ----- |
| 1 | json_serializable | 🟨 `flint_json`, spec 0005 done, spec 0006 steps 1–2 done | shared part (`.g.dart`) | superclass members (spec 0006 step 3), R9, R10, R14, R15 |
| 2 | riverpod_generator | ⬜ | shared part | **top-level functions and their return types (`Future`, `Stream`)**, class-based notifiers, family parameters; its provider hash is computed from source text |
| 3 | freezed | ⬜ | own part (`.freezed.dart`) | **redirecting factories**, unions and sealed classes, `copyWith` (deep), `==`/`hashCode`/`toString`, `@Default`, generics, **own-part output**; works with json_serializable |
| 4 | drift | ⬜ | shared part, plus `.drift` files | table classes and their getters, **dependency declarations** (drift's `Table`), a SQL parser for `.drift` files and queries. The largest one |
| 5 | flutter_gen | ⬜ | library (`lib/gen/*.gen.dart`) | **non-Dart inputs** (pubspec `flutter: assets`, fonts, colors), **library output** |
| 6 | mockito | ⬜ | library (`*.mocks.dart`, imported by the test) | `@GenerateMocks`/`@GenerateNiceMocks` in **`test/` files (A2)**, **full class interfaces from dependency packages** (e.g. `http.Client`), generics |
| 7 | go_router_builder | ⬜ | shared part | route classes, `@TypedGoRoute` trees with nested routes, parameters from constructors |
| 8 | envied | ⬜ | shared part | **non-Dart input** (`.env`), obfuscation option |
| later | auto_route | ⬜ | own part (`.gr.dart`) | `@RoutePage` classes across the project, the `@AutoRouterConfig` router |
| later | retrofit | ⬜ | shared part | abstract methods with HTTP annotations, **method signatures**; works with json_serializable |
| later | injectable | ⬜ | library (`*.config.dart`, imported) | **whole-project scan** of annotated classes, constructor dependencies, environments |
| later | slang | ⬜ | library | **non-Dart inputs** (JSON/YAML/ARB translations) |

Before writing a generator's spec, record the original package's real output for the shapes the spec covers
(the recipe is in [HANDOFF.md](HANDOFF.md#6-how-to-work-on-this-repo)), as spec 0006 did.

---

## Ideas and suggestions

These are worth considering but not scheduled. Promote one to a phase by writing a spec. Newer suggestions
from the spec 0005 and 0006 work (`flint_build explain`, notes for silently dropped members, a
json_serializable reference harness, finishing the render model) are in
[HANDOFF.md §7](HANDOFF.md#7-feature-suggestions-and-improvements).

- ~~**Dump the parsed model as JSON**~~: now part of [spec 0007](specs/0007-generator-platform.md)
  (`flint_build dump-model`, and the model Dart generators receive).
- **WASM plugins** for generators that outgrow Tera, run sandboxed with `wasmtime`. Only worth it if real
  template users hit Tera's limits.
- **Recommend running the binary directly.** `dart run` costs ~300 ms of VM startup per invocation, far more
  than the engine's own work. A `dart pub global activate` executable, or putting the binary on `PATH` for
  watch/IDE use, removes most of that.
- **IDE integration:** a VS Code task/extension that runs `watch` and shows diagnostics in the Problems panel,
  using a `--message-format json` flag on the engine.
- **Format the output** by emitting `dart format`-shaped code directly, or with an optional
  `--format` that runs `dart format` on changed outputs only. Users read generated diffs in code review.
- **Stop using the mtime check for correctness.** mtimes break across `git checkout`, CI caches and
  containers; content hashes (Phase 3) don't.
- **Error codes** (`FLINT001`, …) with a docs page per code. They make issues easy to search for and give
  AI agents stable anchors.
- **Rust learning path:** the codebase is a good size for learning Rust. Good first issues are H3 (clippy),
  H6 (tempfile), R15 (escaping) and A5 (de-duplicating the annotation filter), each small and well-tested.
