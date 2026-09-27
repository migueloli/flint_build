# AGENTS.md

Instructions for AI coding agents (Claude Code, Codex, Cursor, Copilot, …) working in this repository.
People are welcome to read it too. It's the short version of [docs/SDD.md](docs/SDD.md).

## What this project is

Flint (`flint_build`) is a fast replacement for Dart's `build_runner`: a code-generation platform. A Rust
engine parses Dart with tree-sitter (syntax only, no analyzer), builds a project index, and runs generators
that write owned output files. `flint_json` (json_serializable) is the first built-in generator; custom
generators are Tera templates today, and Dart or YAML generators are planned
([spec 0007](docs/specs/0007-generator-platform.md)). The target generators are listed in
[docs/ROADMAP.md](docs/ROADMAP.md#generators). A thin Dart CLI finds the engine binary and runs it.

```text
engine/   Rust crate "flint_build" (lib + bin). All the logic lives here.
  src/main.rs            clap commands: build | watch | clean
  src/builder.rs         orchestration: discover → parse once → run plugins → write/delete owned outputs
  src/output.rs          .g.dart header, ownership marker, section assembly
  src/index.rs           project symbol index: resolve type names across files (spec 0005)
  src/model/             generator model v1 + JSON Schema, dump-model (spec 0007)
  src/config/            pubspec.yaml + flint.yaml + build.yaml loading; resolve() merges them
  src/discovery/         walk lib/, split sources vs *.g.dart
  src/parser/            tree-sitter → ParsedFile (dart_file.rs, dart_types.rs)
  src/generators/        Generator trait, flint_json (members.rs plan + emitter.rs), generic Tera generator
  src/templates/         built-in flint_json.tera (embedded with include_str!)
  src/watcher/           notify + debounce; ignores access events and .g.dart paths
  tests/                 integration + insta snapshots (tests/snapshots/*.snap)
cli/      Dart package "flint_build": bin/flint_build.dart only (launcher)
  example/               sample app + benchmark (tool/benchmark.dart)
docs/     HANDOFF.md (start here), SDD.md, ROADMAP.md, REVIEW.md, configuration.md, specs/
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

Dart golden check (needs a Dart SDK): `engine/tests/dart_golden/check.sh`. It builds the engine, regenerates
the fixtures, then runs `dart analyze --fatal-infos` and `dart test`. CI (`.github/workflows/ci.yml`) runs it
together with the Rust checks.

Benchmark: `engine/bench/run.sh [files] [runs]` times the engine alone on a synthetic project (rule 9).

End-to-end check without the Dart SDK: `cd cli/example && ../../engine/target/release/flint_build build --force`,
then `git diff lib/user_model.g.dart`. Any output change must be intentional.

With the Dart SDK: `cd cli/example && dart pub get && dart run flint_build build` (the repo uses `fvm dart …`).

## Rules

1. **Never write or delete a file Flint doesn't own.** Every change that touches `clean`, discovery, or
   output writing must keep the rules in [spec 0001](docs/specs/0001-generated-output-ownership.md).
2. **Output is deterministic.** No `HashMap` iteration order in anything that reaches generated code, and no
   timestamps or absolute paths in output.
3. **Generated Dart must compile.** When you change the emitter or a template, add or extend a fixture in
   `engine/tests/fixtures/gold/` and review the snapshot diff line by line. Never run `cargo insta accept`
   without reading the diff. When a feature starts working, add it to the Dart golden package
   (`engine/tests/dart_golden/`, see its README) and run `check.sh`; CI fails if generated code doesn't
   analyze cleanly or round-trip.
4. **No panics in library code.** Return `Result`. `unwrap`/`expect` are fine in tests and in provably
   infallible spots (add a comment explaining why). Use `thiserror` for typed errors and `anyhow` at the
   binary edge.
5. **The parser stays syntax-only.** The engine never depends on the Dart analyzer. Cross-file knowledge
   comes from the project symbol index ([SDD §4](docs/SDD.md#4-the-key-constraint-parsing-syntax-only)). The
   Dart SDK is only needed to *run* generators that users write in Dart (planned, spec 0007), never to parse.
6. **Built-in generators use the public generator API.** Anything `flint_json` or a future built-in needs from
   the model must be available to custom generators too (spec 0007). Don't give built-ins private shortcuts.
7. **Specs come before larger changes.** Anything that changes generated output, `flint.yaml`, CLI flags, the
   template context, or the `Generator` trait needs a spec in `docs/specs/` first
   ([workflow](docs/specs/README.md)). If there is none, write a draft and stop for review.
8. **Docs change with the code.** When behaviour changes, update `docs/configuration.md` (support matrix),
   `docs/SDD.md` (Current vs Target), `docs/ROADMAP.md` status, and the READMEs **in the same change**.
9. **No unmeasured performance claims.** Numbers in the docs must come from a benchmark in the repo, with the
   method stated (engine-only vs `dart run` end to end).

## Conventions

- Rust edition 2024, MSRV 1.88 (`rust-version` in `Cargo.toml`, checked by CI; let-chains need it). Format
  with `rustfmt` defaults. Clippy runs with `-D warnings` in CI.
- Tests go next to the code (`#[cfg(test)] mod tests`) for units and in `engine/tests/` for pipeline and
  snapshot tests. Use temporary directories, never fixed paths in `/tmp`.
- Commits use Conventional Commits with a scope, matching the history:
  `feat(engine): …`, `fix(cli): …`, `test(engine): …`, `docs: …`, `chore: …`.
- Refer to review findings by ID (`R3`, `D1`, …) in commits and PRs when you fix one, and mark it in
  `docs/ROADMAP.md`.

## Known sharp edges

Read [docs/REVIEW.md](docs/REVIEW.md) before changing the parser, builder, or emitter. The ones most likely to
catch you out:

- The parser returns **every** class in a file; filtering by annotation happens later, via
  `generators::matches_plugin` / `retain_annotated` (A3). Annotations are read with
  `parser::dart_file::read_annotations`; use tree-sitter field names (`name:`, `body:`) in queries, because
  positional `(_)` captures can match an annotation instead of the name (that was R3).
- Every build parses **every** file to build the symbol index, and resolves field types *before* the
  up-to-date check. Keep that pass cheap (compile queries once; see `engine/bench/run.sh`).
- Generators return a *section*, not a file. `output::assemble` adds the header, ownership marker and
  `part of`; never emit them from a template.
- On Linux, notify reports Flint's own file *reads* as events. Anything added to the watcher must keep
  ignoring access events, or watch mode loops.
- `DartField.from_json_expr` / `to_json_expr` / `converter` are emitter scratch state stored on the parsed
  model (A4).
- The CLI finds the engine at `cli/../engine/target/…`, so it only works in this monorepo (D1).
- tree-sitter-dart quirks (spec 0006): a parameter's `= default` is a *sibling* of the parameter inside
  `[…]`/`{…}`; `required` is a sibling token for `required int x` but a `type_identifier` inside
  `required this.x`; `static set x(…)` puts `static` inside the setter's signature. Check the tree before
  writing a query (the recipe is in [docs/HANDOFF.md](docs/HANDOFF.md#6-how-to-work-on-this-repo)).
- Which members `flint_json` serializes is decided in one place, `generators::flint_json::members::plan`,
  following json_serializable's rules. Before changing a rule, check json_serializable's real output (recipe
  in HANDOFF §6); deliberate deviations are listed in the specs and in HANDOFF §4.
