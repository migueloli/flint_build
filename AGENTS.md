# AGENTS.md

Instructions for AI coding agents (Claude Code, Codex, Cursor, Copilot, …) working in this repository.
People are welcome to read it too. It's the short version of [docs/SDD.md](docs/SDD.md).

## What this project is

Flint (`flint_build`) is a fast replacement for Dart's `build_runner` + `json_serializable`. A Rust engine
parses Dart with tree-sitter (syntax only, no type resolution) and writes `<file>.g.dart` part files. A thin
Dart CLI finds the engine binary and runs it.

```text
engine/   Rust crate "flint_build" (lib + bin). All the logic lives here.
  src/main.rs            clap commands: build | watch | clean
  src/builder.rs         orchestration: discover → parse → generate → write
  src/config/            pubspec.yaml + flint.yaml loading
  src/discovery/         walk lib/, split sources vs *.g.dart
  src/parser/            tree-sitter → ParsedFile (dart_file.rs, dart_types.rs)
  src/generators/        Generator trait, flint_json native emitter, generic Tera generator
  src/templates/         built-in flint_json.tera (embedded with include_str!)
  src/watcher/           notify + debounce
  tests/                 integration + insta snapshots (tests/snapshots/*.snap)
cli/      Dart package "flint_build": bin/flint_build.dart only (launcher)
  example/               sample app + benchmark (tool/benchmark.dart)
docs/     SDD.md, ROADMAP.md, REVIEW.md, configuration.md, specs/
```

## Commands

Run these from `engine/`:

```bash
cargo build                      # debug build
cargo build --release            # binary used by the CLI: target/release/flint_build
cargo test                       # unit + integration + snapshot tests
cargo insta review               # inspect snapshot changes (cargo install cargo-insta)
cargo clippy --all-targets -- -D warnings
cargo fmt
```

End-to-end check without the Dart SDK: `cd cli/example && ../../engine/target/release/flint_build build -d`,
then `git diff lib/user_model.g.dart`. Any output change must be intentional.

With the Dart SDK: `cd cli/example && dart pub get && dart run flint_build build` (the repo uses `fvm dart …`).

## Rules

1. **Never write or delete a file Flint doesn't own.** Every change that touches `clean`, discovery, or
   output writing must keep the rules in [spec 0001](docs/specs/0001-generated-output-ownership.md).
2. **Output is deterministic.** No `HashMap` iteration order in anything that reaches generated code, and no
   timestamps or absolute paths in output.
3. **Generated Dart must compile.** When you change the emitter or a template, add or extend a fixture in
   `engine/tests/fixtures/gold/` and review the snapshot diff line by line. Never run `cargo insta accept`
   without reading the diff.
4. **No panics in library code.** Return `Result`. `unwrap`/`expect` are fine in tests and in provably
   infallible spots (add a comment explaining why). Use `thiserror` for typed errors and `anyhow` at the
   binary edge.
5. **The parser stays syntax-only.** Don't add a dependency on the Dart SDK or analyzer. Cross-file knowledge
   comes from the project symbol index ([SDD §4](docs/SDD.md#4-the-key-constraint-parsing-syntax-only)).
6. **Specs come before larger changes.** Anything that changes generated output, `flint.yaml`, CLI flags, the
   template context, or the `Generator` trait needs a spec in `docs/specs/` first
   ([workflow](docs/specs/README.md)). If there is none, write a draft and stop for review.
7. **Docs change with the code.** When behaviour changes, update `docs/configuration.md` (support matrix),
   `docs/SDD.md` (Current vs Target), `docs/ROADMAP.md` status, and the READMEs **in the same change**.
8. **No unmeasured performance claims.** Numbers in the docs must come from a benchmark in the repo, with the
   method stated (engine-only vs `dart run` end to end).

## Conventions

- Rust edition 2024, MSRV 1.88 (let-chains are used). Format with `rustfmt` defaults.
- Tests go next to the code (`#[cfg(test)] mod tests`) for units and in `engine/tests/` for pipeline and
  snapshot tests. Use temporary directories, never fixed paths in `/tmp`.
- Commits use Conventional Commits with a scope, matching the history:
  `feat(engine): …`, `fix(cli): …`, `test(engine): …`, `docs: …`, `chore: …`.
- Refer to review findings by ID (`R3`, `D1`, …) in commits and PRs when you fix one, and mark it in
  `docs/ROADMAP.md`.

## Known sharp edges

Read [docs/REVIEW.md](docs/REVIEW.md) before changing the parser, builder, or emitter. The ones most likely to
catch you out:

- The parser returns **every** class in a file; filtering by annotation happens later, in each generator (A3).
  A class with several annotations only keeps one of them (R3).
- `DartField.from_json_expr` / `to_json_expr` / `converter` are emitter scratch state stored on the parsed
  model (A4).
- `engine/flint.yaml` is a test fixture used by `tests/flint_json_test.rs`, not a user config (H5).
- The CLI finds the engine at `cli/../engine/target/…`, so it only works in this monorepo (D1).
