# Flint Build Engine 🦀

The high-performance, concurrent core compiler powering **Flint**, written entirely in Rust. It utilizes **Tree-sitter** for lightning-fast, high-fidelity Dart AST extraction and **Tera** (a modern Jinja2/Liquid-compatible template engine) for zero-Rust custom generator extension.

---

## 🏗️ Architectural Core

The engine is modularized into five high-performance components:

```text
               ┌────────────────────────┐
               │    Flint Build CLI     │
               └───────────┬────────────┘
                           │ Invokes native binary
                           ▼
               ┌────────────────────────┐
               │    discovery/  (lib)   │ (Finds .dart files concurrently)
               └───────────┬────────────┘
                           │
                           ▼
               ┌────────────────────────┐
               │     parser/    (AST)   │ (Concurrently parses files using Tree-sitter)
               └───────────┬────────────┘
                           │ Returns ParsedFile AST
                           ▼
               ┌────────────────────────┐
               │   generators/ (Tera)   │ (Renders templates or invokes native emitter)
               └────────────────────────┘
```

- **[`discovery/`](src/discovery)**: Concurrently walks target directories using parallel file filters, discovering modified `.dart` source files and matching existing generated structures.
- **[`parser/`](src/parser)**: Harnesses tree-sitter AST queries to build strongly typed syntax tree mappings of Dart classes, enums, annotations, and generic type parameters with complete thread safety.
- **[`generators/`](src/generators)**: Houses the high-fidelity native `flint_json` emitter and the generic template loading wrapper utilizing Tera templates.
- **[`config/`](src/config)**: Safely handles pubspec and flint configuration deserialization with automatic defaulting strategies for zero-boilerplate configurations.
- **[`registry/`](src/registry)**: Manages plugin registrations and coordinates generation boundaries.

---

## 🛠️ Rust Crate Integration

Although primarily invoked via the CLI wrapper, the engine can be used directly as a Rust library for custom compilation tools:

### Add dependency

Add the engine to your custom Rust project:

```toml
[dependencies]
flint_build = { path = "../path/to/engine" }
```

### Direct Crate Usage Example

```rust
use flint_build::parser;
use flint_build::generators::flint_json::emitter::FlintJsonGenerator;
use flint_build::generators::Generator;
use flint_build::config::PluginConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse Dart source code to highly detailed AST structure
    let parsed_file = parser::parse_file("lib/user_model.dart")?;

    // 2. Load standard configuration
    let config = PluginConfig {
        class_annotations: vec!["@JsonSerializable".to_string()],
        field_annotations: vec!["@JsonKey".to_string()],
        enum_annotations: vec!["@JsonEnum".to_string()],
        variant_annotations: vec!["@JsonValue".to_string()],
        template_path: None,
        converters: None,
        field_rename: Some("snake_case".to_string()),
    };

    // 3. Concurrently generate Dart outputs using the emitter
    let generator = FlintJsonGenerator;
    let generated_code = generator.generate("user_model.dart", parsed_file, &config);

    println!("{}", generated_code);
    Ok(())
}
```

---

## 🧪 Testing & Diagnostics

The engine maintaining a strict testing standard with comprehensive unit, integration, and snapshot tests.

### Run Unit and Integration Tests

```bash
cargo test
```

### Snapshot Verification

The generator uses `insta` to test syntax rendering correctness. If you make template modifications, accept new valid outputs using:

```bash
cargo insta accept
```

### Analyze Code Coverage

To run a coverage analysis and generate visual HTML charts locally:

```bash
cargo llvm-cov --html && open target/llvm-cov/html/index.html
```

---

## ⚖️ License

Flint Build Engine is released under the [MIT License](../LICENSE).
