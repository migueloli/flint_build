# Roadmap

This is the plan for taking Flint from a working prototype to a tool you can safely run on real Flutter apps.
Finding IDs (R1, D3, …) refer to [REVIEW.md](REVIEW.md). Larger items get a spec in [specs/](specs/) before any
code is written (see the [spec workflow](specs/README.md)).

Status: ⬜ not started · 🟨 in progress · ✅ done

---

## Phase 0: Hygiene (small, unblocks everything)

| | Item | Refs |
| --- | ---- | ---- |
| 🟨 | Add a `LICENSE` file (MIT, matching `Cargo.toml`). Added with placeholders: fill in `[YEAR]` and `[COPYRIGHT HOLDER]` | H1 |
| ✅ | `cargo fmt --check` passes | H3 |
| ⬜ | `cargo clippy -- -D warnings` passes. Rename `from_str` to a `FromStr` impl or `parse`; collapse the two `if`s in the parser | H3 |
| ⬜ | Add `rust-version = "1.88"` to `engine/Cargo.toml` | H4 |
| ⬜ | CI workflow: `cargo fmt --check`, `clippy`, `cargo test`, `cargo insta test --check`, `dart analyze cli` | H2 |
| ⬜ | Move `engine/flint.yaml` to `engine/tests/fixtures/`. Snapshot tests should use the built-in template | H5 |
| ⬜ | Use `tempfile` in tests instead of fixed temp paths | H6 |
| ⬜ | Fix `cli/pubspec.yaml` metadata (description, version, repository) and `CHANGELOG.md`; move the example to `dev_dependencies` | D5 |
| ✅ | Accurate READMEs, SDD, configuration reference, agent instructions | H4 |

## Phase 1: Safe to run (P0)

Goal: Flint never damages a project, and generated code compiles for everything it claims to support.

| | Item | Refs |
| --- | ---- | ---- |
| ⬜ | **Dart golden harness:** generate fixtures, then `dart analyze` them in CI (needs the Dart SDK in CI) | H7 |
| ⬜ | **Output ownership:** a header marker; only write when a `part` directive exists; only delete owned files | R1, R2 · [Spec 0001](specs/0001-generated-output-ownership.md) |
| ⬜ | **Multiple plugins per file:** parse once, concatenate sections in config order (`IndexMap`) | R4, A1 · Spec 0001 |
| ⬜ | Watch mode ignores Flint-owned paths | R5 |
| ⬜ | Read *all* annotations on classes, fields, enums and enum constants (walk the nodes instead of one optional query capture) | R3 |
| ⬜ | Keep the literal's kind in annotation arguments; emit typed `@JsonValue` maps | R6 |
| ⬜ | `Result`-returning generators and collected diagnostics; no panics on bad templates | R12 |

## Phase 2: json_serializable parity

Goal: the example app, and a realistic mid-size app, build with Flint and pass the same round-trip tests as
with json_serializable.

| | Item | Refs |
| --- | ---- | ---- |
| ⬜ | **Project symbol index:** resolve enums and classes across files; clear “unresolved type” diagnostics | R7 · SDD §4 |
| ⬜ | **Constructor-aware emission:** positional/named/`this.` params; skip static, late and initialised fields; private-field rules | R8 |
| ⬜ | Scope metadata to configured `field_annotations` / `variant_annotations` | R9 |
| ⬜ | Emit `$enumDecode` / `$enumDecodeNullable`; support `unknownEnumValue` | R10 |
| ⬜ | More types: `num`, `dynamic`, `Object`, `Uri`, `BigInt`, `Duration`, `Set`, `Iterable`, non-String map keys, nested generics | R7, R14 |
| ⬜ | Escape JSON keys; drop identity conversions and `ignore_for_file: unnecessary_cast` | R15, A6 |
| ⬜ | **Differential test suite** against json_serializable, and a **parity matrix** in `configuration.md` | H7 |
| ⬜ | Rename `--delete-conflicting-outputs` to `--force` (keep the old name as a hidden alias) | R13 |
| ✅ | Read json_serializable options from `build.yaml`; `flint.yaml` optional for json_serializable projects | [Spec 0002](specs/0002-read-build-yaml.md) |
| ✅ | `field_rename: camel` means lowerCamelCase; unknown `field_rename` values are errors | [Spec 0003](specs/0003-field-rename-camel.md) |
| ⬜ | `@JsonSerializable(fieldRename: …)` per class | — |

## Phase 3: Incremental and fast at scale

| | Item | Refs |
| --- | ---- | ---- |
| ⬜ | Compile queries and templates once; discover once; parse once for all plugins | A1, A3 |
| ⬜ | Content-hash cache in `.dart_tool/flint/` with an engine/config/template fingerprint; delete outputs that are no longer produced | R11 · SDD §12 |
| ⬜ | Watch mode rebuilds only the dirty set, including files that depend on changed symbols | R5 · SDD §12 |
| ⬜ | `build --check` for CI (non-zero exit if outputs are stale) | — |
| ⬜ | **Benchmark rewrite:** synthetic 10/100/1000-model projects; `hyperfine`; report engine-only *and* end-to-end; define cold/warm | D3 |

## Phase 4: Installable by anyone

| | Item | Refs |
| --- | ---- | ---- |
| ⬜ | Release pipeline: prebuilt binaries per target, SHA-256 checksums, GitHub Releases | D1 · SDD §13 |
| ⬜ | CLI: find the binary via override → cache → download → cargo (dev); version check | D1, D2, D4 |
| ⬜ | `--root`, include/exclude globs, `test/` and `bin/` roots, pub workspaces / melos | A2 |
| ⬜ | Publish `flint_build` to pub.dev and crates.io. pub.dev needs a `LICENSE` inside `cli/` too | D1 |

## Phase 5: Platform and ecosystem

| | Item |
| --- | ---- |
| ⬜ | Stable, versioned **template context** (`context_version`), plus Tera filters for casing and type helpers |
| ⬜ | Per-plugin `output_extension` (e.g. `.flint.dart`) for generators that need their own file |
| ⬜ | More built-in generators: `copyWith`, `==`/`hashCode`, `toString` (the most-used parts of freezed without unions) |
| ⬜ | `flint_build migrate`: check a build_runner project and list what Flint can't generate yet (options are already read from `build.yaml`, spec 0002) |
| ⬜ | `flint_build doctor`: check `part` directives, unresolved types, stale outputs, version mismatch |

---

## Ideas and suggestions

These are worth considering but not scheduled. Promote one to a phase by writing a spec.

- **Dump the parsed model as JSON** (`flint_build dump-ir lib/foo.dart`) so generators can be written in any
  language, including Dart, reading the model from stdin. It also makes a good debugging tool for template
  authors.
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
