# Flint Build ⚡

A native, parallel code generator for Dart and Flutter that works like `build_runner` + `json_serializable`.
The engine is written in Rust and parses Dart with [tree-sitter](https://tree-sitter.github.io/).

> **Status: experimental (engine 0.1.0).** Flint generates correct `json_serializable`-style code for the
> common cases below, but it has known gaps that can produce code that doesn't compile. Read
> [Known limitations](#known-limitations) before trying it on a real project. Flint only writes, overwrites
> or deletes `.g.dart` files it generated itself, so other generators' files are safe.

---

## Why

`build_runner` starts the Dart VM, resolves the whole program with the analyzer, and then runs generators.
On large apps that takes seconds to minutes. Flint does only what code generation needs: it parses each file's
**syntax** in parallel and renders templates. On the example project the engine finishes in about **13 ms**.

## Repository layout

```mermaid
flowchart LR
    A["Dart / Flutter project"] -->|dart run flint_build| B["cli/ — Dart launcher"]
    B -->|finds and executes| C["engine/ — Rust binary"]
    C --> D["tree-sitter parse<br/>(parallel, rayon)"]
    D --> E["generators<br/>flint_json · custom Tera templates"]
    E --> F["*.g.dart part files"]
```

| Path | What it is |
| ---- | ---------- |
| [`engine/`](engine) | Rust crate `flint_build`: discovery, parsing, generators, watcher. [README](engine/README.md) |
| [`cli/`](cli) | Dart package `flint_build`: the `dart run flint_build` entry point that finds and runs the engine. [README](cli/README.md) |
| [`cli/example/`](cli/example) | Sample app and benchmark comparing Flint with build_runner |
| [`docs/`](docs) | Design, roadmap, review and reference docs (below) |

## Quick start (from this repository)

You need Rust **1.88+** (`rustup`) and a Dart or Flutter SDK. The repo uses [FVM](https://fvm.app/); drop
the `fvm` prefix if you don't.

```bash
git clone https://github.com/migueloli/flint_build.git
cd flint_build/engine && cargo build --release   # the CLI would also build this on first run

cd ../cli/example
fvm dart pub get
fvm dart run flint_build build     # generate lib/**.g.dart
fvm dart run flint_build watch     # rebuild on change
```

In your own project, add Flint as a path dev-dependency. If you already use json_serializable, that's all:
Flint reads your existing `build.yaml` options and needs no `flint.yaml`. See the [CLI README](cli/README.md). Flint isn't on pub.dev yet, because the CLI can only find the engine inside
this repository (see the [roadmap](docs/ROADMAP.md#phase-4-installable-by-anyone)).

## What it supports

`@JsonSerializable` classes with `String`/`int`/`double`/`bool`/`DateTime`, `num`/`dynamic`/`Object`,
`Uri`/`BigInt`/`Duration`, `List`/`Set`/`Iterable`, `Map<String, V>`, nested models (also through import prefixes), generic classes, same-file `@JsonEnum` enums, custom converters, and the common `@JsonKey`
options (`name`, `defaultValue`, `ignore`, `includeIfNull`, `fromJson`/`toJson`, …). You can also write your
own generator as a [Tera](https://keats.github.io/tera/) template, with no Rust required.

The full support matrix and the `flint.yaml` reference are in [docs/configuration.md](docs/configuration.md).

## Known limitations

These are the most important ones. All of them are tracked in [docs/REVIEW.md](docs/REVIEW.md) and scheduled
in the [roadmap](docs/ROADMAP.md).

- Enums declared in another file are treated as classes and generate code that doesn't compile (R7, in progress
  in [spec 0005](docs/specs/0005-project-symbol-index.md)).
- Constructors are assumed to take every field as a named parameter (R8).
- Up-to-date checks use modification times, not content hashes, so unusual mtimes (some checkouts or
  caches) can leave stale output; `build --force` fixes it (R11).

## Performance

| Measurement (example project, 1 model file) | Time |
| ------------------------------------------- | ---: |
| `build_runner build` via `fvm dart run` | 680 ms |
| `flint_build build` via `fvm dart run` (Dart launcher + engine) | 330 ms |
| Flint engine binary alone | ~13 ms |

Most of the 330 ms is Dart VM startup for the launcher, not code generation. The first two rows come from
[`cli/example/benchmark_results.txt`](cli/example/benchmark_results.txt), a single run. The last row is the
engine's own timer on a release build.

**Larger projects:** [`engine/bench/run.sh`](engine/bench/run.sh) generates a synthetic project (default 1,000
files, each with one model and one enum) and times the engine alone. On a 4-core machine:

| Engine-only, 1,000 files | Time |
| ------------------------ | ---: |
| `build --force` (parse and generate everything) | ~0.28 s |
| `build` with everything up to date | ~10 ms |
| Parsing only | ~64 ms |

These are typical of three runs after a warm-up. A comparison with build_runner at this size isn't measured
yet; it's on the [roadmap](docs/ROADMAP.md#phase-3-incremental-and-fast-at-scale).

## Documentation

| Doc | For |
| --- | --- |
| [docs/configuration.md](docs/configuration.md) | Users: commands, `flint.yaml`, support matrix, template context |
| [docs/SDD.md](docs/SDD.md) | Contributors: architecture, data model, design decisions, target design |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Everyone: phased plan and ideas |
| [docs/REVIEW.md](docs/REVIEW.md) | Contributors: findings from the latest project review, with IDs |
| [docs/specs/](docs/specs/README.md) | Contributors: spec-driven workflow for larger changes |
| [AGENTS.md](AGENTS.md) / [CLAUDE.md](CLAUDE.md) | AI coding agents: commands, rules, sharp edges |

## Contributing

```bash
cd engine
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
tests/dart_golden/check.sh     # generated Dart compiles and round-trips (needs a Dart SDK)
```

CI runs all of these on every push ([`.github/workflows/ci.yml`](.github/workflows/ci.yml)).

Changes to generated output, `flint.yaml`, CLI flags or the template context start with a spec. See the
[spec workflow](docs/specs/README.md). Snapshot changes are reviewed with `cargo insta review`.

## License

[MIT](LICENSE). The copyright line in `LICENSE` is still a placeholder.
