# Project Review — September 2026

A full review of `flint_build` at commit `ae492d2`. It covers the Rust engine, the Dart CLI wrapper, the example
project, tests, and docs.

**How findings were verified:** each finding marked **Reproduced** was run against a release build of the
engine (`cargo build --release`) in a scratch Dart project. The others come from reading the code. Every finding
has an ID that [`ROADMAP.md`](ROADMAP.md) and the specs refer to.

Severity:

- **Critical**: loses user data.
- **High**: generates wrong or non-compiling code for common inputs.
- **Medium**: wrong in less common cases, or makes the next steps harder.
- **Low**: polish.

---

## What is already good

- **Clear pipeline:** `discovery → parser → generators`, with a `Generator` trait and a `PluginRegistry`. Adding
  a native generator doesn't touch the parser.
- **The engine is fast:** about **13 ms** end to end for the example project (release build, measured with
  the engine's own timer).
- **Output is deterministic:** regenerating the example produced a byte-identical `user_model.g.dart`.
- **Good syntax errors:** errors point at the exact file, line and column, with a caret under the problem
  (`FlintError::Syntax`).
- **Useful test base:** there are insta snapshot tests for the JSON emitter, unit tests per module, and all
  18 tests pass.
- **Custom templates work:** a Tera template lets users write a generator without touching Rust.

---

## Correctness and data safety

| ID  | Severity | Finding | Evidence |
| --- | -------- | ------- | -------- |
| R1  | Critical | `flint_build clean` deletes **every** `*.g.dart` under `lib/`, including files written by other generators (riverpod_generator, retrofit, and others). *(Fixed by spec 0001: `clean` only deletes files carrying the Flint marker.)* | Reproduced: a hand-written `lib/other.g.dart` was deleted. `discovery::find_generated_files` matches on suffix only. |
| R2  | High | Any file with **any** class or enum gets a `.g.dart`, even when nothing in it is annotated. The new file contains `part of 'x.dart'` for a library that never declared `part 'x.g.dart'`, so the analyzer reports errors. *(Fixed by spec 0001: output needs a matching annotation **and** a `part` directive.)* | Reproduced: a plain `class Plain {}` produced `plain.g.dart`. `builder.rs` checks `classes.is_empty()` *before* the generator filters by annotation. |
| R3  | High | A class with **more than one annotation** is silently dropped. For example, `@immutable @JsonSerializable() class X` generates an empty file. *(Fixed: annotations are read by walking the declaration's children. It was worse than described: with the matching annotation first, e.g. `@JsonSerializable() @Tag() class X`, the query took `@Tag()` as the class name and generated broken code.)* | Reproduced. The tree-sitter query captures at most one `(annotation)?` per match, and the de-dup by node id keeps only the first match. |
| R4  | High | Several plugins write to the **same** `<file>.g.dart`. The last writer wins, and the mtime check then makes the next plugin skip every file. Plugin order follows `HashMap` iteration, so it isn't deterministic. *(Fixed by spec 0001: plugins share one file, as sections in `flint.yaml` order.)* | Reproduced with `flint_json` plus a custom template plugin: the template output replaced the JSON code and `flint_json` printed “Skipping” for every file. |
| R5  | High | `watch --delete-conflicting-outputs` loops forever: the watcher sees its own `.g.dart` writes and rebuilds again. Without `-d` it still does one extra rebuild for every change. *(Fixed by spec 0001. A second cause turned up while fixing it: inotify reports Flint's own *reads* of sources as events. The watcher now ignores access events and `.g.dart` paths.)* | Reproduced: **17 rebuilds in 4 s** after a single `touch`. |
| R6  | High | A numeric or boolean `@JsonValue` becomes a string. `@JsonValue(1)` generates `Level.low: '1'`, so the JSON wire format is wrong. *(Fixed: the argument's Dart source is kept and emitted as written, e.g. `Level.low: 1`. The fix also covers two related bugs: quotes inside values broke the output (`@JsonValue('say "hi"')` became `'say "hi'`), and enum map keys are now converted to strings in both directions.)* | Reproduced. `process_json_value_node` strips quotes, so the kind of literal is lost. |
| R7  | High | Only types declared **in the same file** are recognised. An enum from another file is treated as a class (`Color.fromJson(...)`, which doesn't compile). `num`, `dynamic`, `Object`, `Uri`, `BigInt`, `Duration`, `Set`, `Iterable` and records all fall through to `X.fromJson(...)` too. *(Spec 0005 found two more cases: prefixed names lose their dot (`m.Money` → `mMoney`), and records produce an empty name (`.fromJson(…)`).)* *(Partly fixed by spec 0005 step 2: prefixed names keep their dot, and records and function types give a clear error instead of `.fromJson(…)`.)* *(Spec 0005 step 3: `num`, `dynamic`, `Object`, `Uri`, `BigInt`, `Duration`, `Set` and `Iterable` now work. Types from other files are still open.)* *(Spec 0005 step 4: a project symbol index now resolves field types across files, reporting ambiguous names; the emitter starts using it in steps 5 and 6.)* *(Spec 0005 step 5: enums from other files, and enums without `@JsonEnum`, now work.)* *(Spec 0005 step 6: a class without `fromJson`, a mixin, typedef or extension type, and a name nothing declares are now errors; names from other packages warn once unless listed in `external_types`. Fixed by spec 0005.)* | Reproduced with an enum defined in `color.dart` and used in `stat.dart`. |
| R8  | Medium | The emitter assumes the constructor shape. It always generates `ClassName(field: …)` with **every** field as a named argument. Positional constructors, fields with initialisers, `late` fields and private fields produce code that doesn't compile. (json_serializable reads the real constructor.) *(Spec 0006 found one more case: in `final int a, b;` only `a` is parsed, so `b` is never read or written. It also records json_serializable's actual output for each shape, which differs from this description in places: final fields the constructor doesn't set are left out of `toJson` too.)* *(Spec 0006 step 1: the parser now reads constructors, getters and every variable of a declaration, and drops static fields, which were also leaking in; `flint_json` starts using constructors in step 2.)* *(Spec 0006 step 2: `fromJson` now calls the real constructor, with json_serializable's member rules; only superclass members remain, in step 3.)* | From the code: `flint_json.tera` and `parse_field`. |
| R9  | Medium | `field_annotations` and `variant_annotations` are parsed, defaulted and documented, but **never read**. Field metadata also merges the named arguments of *every* annotation on a field, so `@Other(name: 'x')` would rename the JSON key. *(Half fixed with R3: `variant_annotations` now decides which annotation sets an enum constant's value; before, the last annotation with arguments won, e.g. `@JsonValue('lo') @Deprecated('x')` gave `'x'`. `field_annotations` is still unused.)* | `grep` shows no reads outside `config/`. See `parse_field → check_metadata`. |
| R10 | Medium | Enum decoding uses `.entries.firstWhere(...)`, which throws a `StateError` with no useful message on an unknown value. There's no `unknownEnumValue`, and `toJson` produces a nullable lookup. | The generated code in `benchmark_outputs/user_model.flint.dart`. json_serializable uses `$enumDecode`. |
| R11 | Medium | Staleness is judged only by comparing the source's mtime with the output's. Editing `flint.yaml` or a custom template, or upgrading the engine, doesn't trigger regeneration. Removing all annotations from a file leaves its old output behind. *(Partly fixed by spec 0001: config files, templates and the engine binary now count as inputs. Content hashes are still Phase 3.)* *(Outputs now also depend on the files their types are declared in, spec 0005 step 4.)* | `builder.rs` lines 145–157. |
| R12 | Medium | Some failures panic instead of returning an error. A missing or invalid template hits `unwrap`/`expect` inside rayon and prints a full backtrace. `Generator::generate` returns `String`, not `Result`, and a non-UTF-8 file name hits `unwrap()`. *(Fixed by spec 0004: template problems are reported with Tera's full explanation and the build exits non-zero. Files a broken plugin matches are left untouched, and the other plugins still run.)* | Reproduced with `template_path: nope.tera`. |
| R13 | Low | `--delete-conflicting-outputs` here means “ignore mtimes and force a rebuild”. That isn't what the build_runner flag of the same name does, which confuses people migrating. *(Fixed by spec 0001: the flag is now `--force`, with the old name kept as an alias.)* | `main.rs`, `builder.rs`. |
| R14 | Low | `Map<K, V>` is parsed by splitting on the first comma, so `Map<Map<String, int>, int>` is parsed wrongly. Non-`String` keys (int, enum) aren't converted. `k as String` is always emitted and hidden with `ignore_for_file: unnecessary_cast`. *(Nested map arguments are now split at the top-level comma, spec 0005 step 2.)* | `parse_dart_type`, emitter. |
| R15 | Low | JSON keys are placed inside `'…'` without escaping, so a key containing `'` or `$` generates broken Dart. | `flint_json.tera`. |

## Architecture and performance

| ID | Severity | Finding |
| -- | -------- | ------- |
| A1 | Medium | Work is repeated for every file or plugin. The tree-sitter `Query` is compiled for **every file**, the Tera template is compiled for **every file**, discovery walks `lib/` once **per plugin**, and every file is parsed once **per plugin**. Parsing once, compiling once, and running all plugins from the same parse would scale much better. *(Queries are now compiled once, as step 1 of spec 0005: on the 1,000-file benchmark a `--force` build went from ~1.95 s to ~0.28 s, and parsing alone from 1.67 s to ~64 ms. Templates are still compiled per file; that is most of the remaining `--force` time.)* | *(Measured for spec 0005: compiling a query takes 3.1 ms, twice per file, against 86 µs to parse the file, so a 1,000-file build spends ~1.67 s of its 1.9 s compiling queries.)*
| A2 | Medium | Paths are hard-coded and relative to the working directory: `lib/`, `pubspec.yaml`, `flint.yaml`, and the template path. There's no `--root`, no include/exclude globs, and no support for `test/` or `bin/`. Monorepos (melos, pub workspaces) aren't supported. |
| A3 | Medium | The parser doesn't filter by annotation, so the parsed model contains every class and field in every file. That is wasted work, and it is the root cause of R2. |
| A4 | Low | The parsed model (`DartField`) mixes parser output with emitter scratch state (`from_json_expr`, `to_json_expr`, `converter`, `#[allow(dead_code)]`). The emitter also mutates `metadata["name"]`. That leaks into the custom-template context as `null` fields. A separate *parsed model* and *render model* would keep each side clean. |
| A5 | Low | The two sides aren't consistent. Class metadata strips the `@`, while enum annotations keep raw text. The annotation-filter code is duplicated in `generic.rs` and `emitter.rs`. Metadata values are raw source text, quotes included (`"'id_'"`). |
| A6 | Low | The generated code isn't formatted like `dart format` output. It contains identity conversions (`tags.map((elem) => elem).toList()`) and long lines. |

## Distribution and developer experience

| ID | Severity | Finding |
| -- | -------- | ------- |
| D1 | High | The CLI only works **inside this monorepo**. It looks for the binary at `<cli>/../engine/target/{release,debug}`. Installed from pub.dev or as a git dependency, there is no `../engine`, so the package can't be distributed as it stands. |
| D2 | Medium | The CLI can run a stale engine. If `target/release/flint_build` exists, it is used even when the engine sources have changed. There's no version check between the CLI and the engine. |
| D3 | Medium | The benchmark mostly measures the Dart wrapper. The engine takes ~13 ms of the reported 330 ms; the rest is `fvm dart run` startup and package resolution. It also uses one sample and one file, and doesn't control build_runner's `.dart_tool/build` cache. The “10x–50x on enterprise codebases” claim has never been measured. |
| D4 | Low | The CLI's `catch (e)` reports **any** failure as “Cargo is not available”. Its `main` is `void main() async` rather than `Future<void>`. |
| D5 | Low | The package metadata is still the template: `description: A starting point for Dart libraries or applications.`, `version: 1.0.0` (the engine is `0.1.0`), a CHANGELOG saying “Initial version”, no `repository`/`homepage`, and an unused `test` dev dependency. The example puts `flint_build` under `dependencies` instead of `dev_dependencies`, and `path` is unused. | *(Fixed: real description, version `0.1.0` matching the engine, repository links, `publish_to: none` until D1, a real CHANGELOG. The example uses `dev_dependencies` and has no unused packages.)*

## Repository hygiene

| ID | Severity | Finding |
| -- | -------- | ------- |
| H1 | High | There's **no `LICENSE` file**, although the READMEs and `Cargo.toml` declare MIT and link to it. *(MIT `LICENSE` added with a placeholder copyright holder, to be filled in.)* |
| H2 | Medium | There's no CI: nothing runs tests, clippy, rustfmt, `dart analyze`, or the snapshot check on push. | *(Fixed: `.github/workflows/ci.yml`.)*
| H3 | Low | `cargo fmt --check` fails (8 hunks), and `cargo clippy` reports 4 warnings: two `should_implement_trait` for `from_str` and two `collapsible_if`. | *(Fixed: clippy passes with `-D warnings`, and CI enforces it.)*
| H4 | Low | The docs were inaccurate. The root README had a duplicated, broken “Elite Test Coverage” section, a hard-coded coverage badge that couldn't be checked, and a Rust prerequisite of 1.75+ (the crate needs ≥ 1.88 for edition 2024 let-chains). The engine README's library example didn't compile. *(Fixed in this change.)* | *(The MSRV is now declared in `Cargo.toml` and checked by a CI job.)*
| H5 | Low | `engine/flint.yaml` is a test fixture sitting at the crate root. It points `template_path` at `src/templates/flint_json.tera`, so the snapshot tests load the template from disk instead of the `include_str!` copy that users get. | *(Fixed: moved to `engine/tests/fixtures/flint.yaml`, with no `template_path`, so snapshots cover the built-in template.)*
| H6 | Low | Tests use fixed temp paths (`flint_discovery_test`, `invalid.dart`, `mock_template.tera`), so they can collide when run in parallel or by two users. `tempfile` fixes this. | *(Fixed: every test uses `tempfile`.)*
| H7 | Medium | Nothing checks the generated Dart. Snapshots prove the text is stable, not that it compiles or produces the same JSON as json_serializable. The next correctness fixes need a Dart-side check (`dart analyze`, plus round-trip tests against json_serializable). | *(Fixed: `engine/tests/dart_golden/` analyzes the generated fixtures and round-trips them in `dart test`, in CI. Comparing against json_serializable's own output is still Phase 2.)*

---

## The five things that matter most

1. ✅ **Output ownership (R1, R2, R4):** Flint must only write and delete files it owns. This is the only way it
   can destroy user data. → [Spec 0001](specs/0001-generated-output-ownership.md)
2. ✅ **Dart-side golden tests (H7):** everything else is a fix to generated Dart. Without a check that the output
   compiles, each fix is a guess.
3. 🟨 **Project-wide symbol index (R7, R8):** cross-file enums and real constructors are what stop Flint handling
   real apps. This is the biggest design change on the list. *(R7 done: [spec 0005](specs/0005-project-symbol-index.md).
   R8 mostly done: [spec 0006](specs/0006-constructor-aware-emission.md) steps 1–2; superclass members are
   step 3.)*
4. **Distribution (D1, D2):** until the CLI can find a prebuilt, version-matched engine, only people who clone
   this repo can use Flint.
5. **Honest benchmarks (D3):** the engine is genuinely fast, but the published numbers mostly measure Dart VM
   startup. Measuring the engine by itself, on large synthetic projects, will show a much better (and true)
   result.
