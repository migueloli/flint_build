# Flint engine (Rust)

The Rust crate `flint_build`. It provides the binary that the [Dart CLI](../cli/README.md) runs, and a
library you can embed. It parses Dart source with **tree-sitter** (syntax only), runs generators in parallel
with **rayon**, and renders output with **Tera** templates.

For the design as a whole (current vs target), see [docs/SDD.md](../docs/SDD.md).

## Pipeline

```text
main.rs (clap: build | watch | clean)
   │
   ▼
builder::run_build ──► config      pubspec.yaml (name), flint.yaml (plugins)
   │
   ├─► discovery    walk lib/, *.dart sources vs *.g.dart outputs
   ├─► parser       tree-sitter → ParsedFile { classes, enums }        (per file, in parallel)
   ├─► registry     plugin name → Generator  (flint_json built in, else GenericTeraGenerator)
   └─► generators   ParsedFile + PluginConfig → String → <file>.g.dart

watcher::watch      notify + 500 ms debounce on lib/ → run_build
```

| Module | Purpose |
| ------ | ------- |
| [`config/`](src/config) | `Pubspec` (only `name`) and `FlintConfig`/`PluginConfig`, with `flint_json` defaults |
| [`discovery/`](src/discovery) | `find_dart_files` / `find_generated_files` using `walkdir` |
| [`parser/`](src/parser) | tree-sitter queries → `DartClass`, `DartField`, `DartType`, `DartEnum`. Syntax errors with a caret |
| [`generators/`](src/generators) | `Generator` trait, `TemplateEngine` (Tera), `flint_json` emitter, `generic` template generator |
| [`templates/`](src/templates) | Built-in `flint_json.tera`, embedded in the binary with `include_str!` |
| [`registry.rs`](src/registry.rs) | `PluginRegistry`: name → `Box<dyn Generator>` |
| [`builder.rs`](src/builder.rs) | `run_build` / `run_clean` orchestration and mtime-based skip |
| [`watcher/`](src/watcher) | Watch mode |
| [`error.rs`](src/error.rs) | `FlintError` (`thiserror`) |

## Build

Requires Rust **1.88+** (edition 2024 with let-chains).

```bash
cargo build --release     # target/release/flint_build, the binary the CLI looks for
RUST_LOG=debug ./target/release/flint_build build    # run inside a Dart package root
```

## Use as a library

The API isn't stable yet (0.x). It will change as the [roadmap](../docs/ROADMAP.md) lands.

```toml
[dependencies]
flint_build = { path = "../flint_build/engine" }
anyhow = "1"
```

```rust
use std::path::Path;

use flint_build::config::FlintConfig;
use flint_build::generators::Generator;
use flint_build::generators::flint_json::emitter::FlintJsonGenerator;
use flint_build::parser;

fn main() -> anyhow::Result<()> {
    // flint.yaml with just `plugins: { flint_json: }` gets json_serializable's annotation names as defaults.
    let config = FlintConfig::from_str("plugins:\n  flint_json:\n")?;
    let plugin = &config.plugins.as_ref().unwrap()["flint_json"];

    let parsed = parser::parse_file(Path::new("lib/user_model.dart"))?;
    let code = FlintJsonGenerator.generate("user_model.dart", parsed, plugin);

    println!("{code}");
    Ok(())
}
```

## Test

```bash
cargo test                          # unit + integration + snapshot tests
cargo insta review                  # review snapshot changes (cargo install cargo-insta)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

- Unit tests live next to the code in `#[cfg(test)]` modules.
- `tests/flint_json_test.rs` renders `tests/fixtures/gold/*.dart` and compares against
  `tests/snapshots/*.snap`. It reads `engine/flint.yaml` as its config.
- `tests/generic_generator_test.rs` covers the custom template path.

When you change the emitter or a template, add a fixture that exercises the change and read the snapshot diff
before accepting it. Snapshots prove the output is stable, not that it compiles. Dart-side checks are planned
(REVIEW H7).

Coverage, if you have [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov) installed:

```bash
cargo llvm-cov --html    # report in target/llvm-cov/html/index.html
```

## License

MIT
