# 0005 — Project symbol index: resolve field types across files

| | |
| --- | --- |
| **Status** | Done (accepted with the proposed answers to the open questions; steps 1–7 implemented) |
| **Resolves** | R7 (and two related bugs found while writing this spec), A1 as a prerequisite |
| **Touches** | `parser/` (declarations, imports, type names), new `index` module, `builder.rs`, `generators/` (`Generator` trait, `flint_json` emitter and template), `config/flint.rs`, docs |

## Problem

The `flint_json` emitter only recognises field types that are built in (`String`, `int`, `double`, `bool`,
`DateTime`, `List`, `Map`) or that are an `@JsonEnum` enum **in the same file**. Everything else is assumed to
be a class with `fromJson`. Output of the current engine for one model (all of it fails to compile except
`price`, which only works by accident):

| Field type | Generated `fromJson` | Generated `toJson` |
| ---------- | -------------------- | ------------------ |
| `Color` (enum in another file) | `Color.fromJson(json['color'] as Map<String, dynamic>)` | `instance.color` (not encodable) |
| `Local` (enum in the same file, no `@JsonEnum`) | `Local.fromJson(…)` | `instance.local` |
| `m.Money` (class via `import 'src/money.dart' as m`) | `mMoney.fromJson(…)`: **the dot is dropped** | `instance.price` |
| `num`, `dynamic`, `Object` | `num.fromJson(…)`, `dynamic.fromJson(…)`, `Object.fromJson(…)` | passthrough |
| `Uri`, `BigInt`, `Duration` | `Uri.fromJson(…)`, … | passthrough (not encodable) |
| `Set<int>`, `Iterable<String>` | `Set<int>.fromJson(…)`, … | passthrough (`Set` isn't encodable) |
| `(int, String)` (record) | `.fromJson(…)`: **empty type name** | passthrough |

The two bold rows are bugs outside R7's original description: prefixed type names lose their `.`, and types
that aren't plain names (records, function types) produce an empty name.

The root cause is that Flint parses one file at a time, so it can't tell what an imported name is.

### Measured cost of parsing every file

An index means parsing **every** file on every build, where today up-to-date files are skipped. Measured with
the engine's own timer, release build, 4 cores, on a synthetic project of 1,000 files (one model and one enum
each):

| Run | Time |
| --- | ---: |
| Full build (`--force`) | 1.9 s |
| Parse only (same files without annotations, `--force`) | 1.67 s |
| No-op build (everything up to date, no parsing) | ~10 ms |
| `tree_sitter::Query::new` for one query (microbenchmark) | 3.1 ms |
| Parsing one of those files (microbenchmark) | 86 µs |

Two queries are compiled **per file**, so about 97% of parse time is query compilation (A1). With A1 fixed,
parsing should approach the tree-sitter cost. Without it, an index would add ~1.7 s to every build of a
project this size, including builds where nothing changed. **A1 is therefore step 1 of this spec.**

## Goals / non-goals

- **Goal:** enums and classes declared anywhere in the project resolve correctly, including through import
  prefixes and re-exports.
- **Goal:** the common `dart:core` types json_serializable supports generate the same conversions.
- **Goal:** a type Flint can't handle produces a clear diagnostic, never Dart that doesn't compile.
- **Goal:** the parser stays syntax-only (AGENTS.md rule 5): no Dart SDK, no analyzer.
- **Goal:** a no-op build of the 1,000-file benchmark stays fast (see acceptance criteria).
- **Non-goal:** resolving declarations inside *other packages* (see “External types”).
- **Non-goal:** records, function types, typedef expansion, extension types. They get a diagnostic.
- **Non-goal:** constructor-aware emission (R8), `$enumDecode` and `unknownEnumValue` (R10), non-`String` map
  keys other than enums (R14), a persistent index cache (Phase 3).

## Behaviour

### How a field type is resolved

For a type name `T` (optionally prefixed, `p.T`) in a field of a class in file `F`, the first match wins:

1. **Type parameter** of the class: generic factories, as today.
2. **`dart:core` table** below (only when unprefixed).
3. **`F`'s library**: declarations in `F` and in its `part` files.
4. **Project imports of `F`**: files `F` imports by relative path or as `package:<this package>/…`,
   including what those files `export`, transitively. `show`/`hide` combinators are honoured. A prefixed name
   `p.T` is looked up only in the imports declared `as p`.
5. **`external_types`** configured for the plugin (see below).
6. Otherwise **unresolved**.

If two different declarations named `T` are visible at step 3 or 4, the field gets an **ambiguity error**
naming both files.

### What each kind generates

| Resolved kind | `fromJson` | `toJson` |
| ------------- | ---------- | -------- |
| Enum (any enum, with or without `@JsonEnum`) | `_$TEnumMap` lookup (as today), and the map is emitted in **this** file's section | `_$TEnumMap[…]` |
| Class that has a `fromJson` constructor/factory (or static method) | `T.fromJson(… as Map<String, dynamic>)` (as today) | as today (`explicitToJson` rules) |
| Class without either | **error**: “`T` has no `fromJson` constructor; add one, or use `@JsonKey(fromJson:, toJson:)` or a converter” | |
| `num` | `json['x'] as num` | passthrough |
| `dynamic`, `Object?` | `json['x']` | passthrough |
| `Object` | `json['x'] as Object` | passthrough |
| `Uri` | `Uri.parse(json['x'] as String)` | `instance.x.toString()` |
| `BigInt` | `BigInt.parse(json['x'] as String)` | `instance.x.toString()` |
| `Duration` | `Duration(microseconds: (json['x'] as num).toInt())` | `instance.x.inMicroseconds` |
| `Set<E>` | `(json['x'] as List<dynamic>).map((e) => …).toSet()` | `instance.x.map(…).toList()` |
| `Iterable<E>` | `(json['x'] as List<dynamic>).map((e) => …)` | `instance.x.map(…).toList()` |
| External type | `T.fromJson(… as Map<String, dynamic>)` | as a class |
| Mixin, typedef, extension type, record, function type | **error**: “Flint can't serialize `…` yet; use `@JsonKey(fromJson:, toJson:)` or a converter” | |
| Unresolved | see “External types” | |

Nullable variants and nesting (`List<Uri>?`, `Map<String, Set<Color>>`) compose as they already do for `List`
and `Map`. The `dart:core` conversions are meant to match json_serializable; the golden fixtures will check
the JSON they produce, and differential tests against json_serializable (Phase 2) will confirm it.

### Enum maps

A private `const _$TEnumMap` can only be used inside its own library. So, like json_serializable, the map for
enum `T` is emitted in **every** library whose generated code uses `T`, once per library, in declaration
order of first use. An enum from another file therefore no longer needs `@JsonEnum` or a `.g.dart` of its
own. `@JsonValue` values are read from the enum's declaration, with the R3 and R6 fixes (`variant_annotations`, typed literals).

### External types

Types from other packages can't be indexed. Configuration lists them explicitly:

```yaml
plugins:
  flint_json:
    external_types: [Money, Currency]   # classes from other packages with fromJson/toJson
```

A listed type generates like a class. For an **unresolved** name that isn't listed, the proposed default
(open question 1) is:

- if `F` imports any package other than its own and `dart:`: assume a class with `fromJson`/`toJson` (today's
  behaviour, so no working project breaks), and print **one warning per type name** suggesting
  `external_types`;
- otherwise (the name can't come from anywhere): **error** for that file.

### Diagnostics

Errors are per source file, as in spec 0001: the file's output is left unchanged and the build exits
non-zero. Each message names the file, the line, the field and a fix, for example:

```text
❌ lib/model.dart:14: field 'pair' has type '(int, String)', which Flint can't serialize yet.
   Use @JsonKey(fromJson: …, toJson: …) or a converter.
```

### Staying up to date

A generated file now depends on other files: an enum map copied from `lib/src/color.dart` into
`model.g.dart` goes stale when `color.dart` changes. An owned output is up to date only if it's newer than
its source, the shared inputs (spec 0001), **and every file its resolved types came from**.

### Template context (additive)

- Templates get `resolved_types`: a map from each type name used by the file's generated classes (as written,
  `Money` or `m.Money`) to `{ "kind": "class" | "enum" | "mixin" | "type_alias" | "extension_type" |
  "unresolved", "file": "lib/src/money.dart" | null, "has_from_json", "has_to_json" }`. *(Changed during
  step 4: the draft put a `resolved` field on every `dart_type`; a single sorted map gives templates the same
  information without changing the parsed model's shape.)*
- `enums` keeps its meaning for custom templates (annotated enums declared in this file). The `flint_json`
  template gets `enum_maps`, the enums whose maps this file needs.

## Design

1. **Queries compiled once (A1):** the class and enum queries become `LazyLock<Query>` statics. A committed
   benchmark script generates the synthetic project and prints the timings above, so this spec's numbers can be
   reproduced.
2. **Parser additions:** for each file, top-level declarations (name, kind, annotations, whether a class has
   a `fromJson` constructor or factory and a `toJson` method, enum values); `import`/`export`/`part`
   directives with prefixes and combinators; type names keep their prefix; non-name types become
   `TypeKind::Unsupported(text)` instead of an empty `Custom`.
3. **`SymbolIndex`:** built in the builder right after discovery by parsing every file in parallel. It maps
   each file to its declarations and directives, and resolves a (file, prefix, name) to a declaration using
   the order above. Paths are sorted so resolution and output stay deterministic.
4. **`Generator` trait:** `generate` receives a `&FileScope` (the index, seen from one file). This changes the
   library API again (pre-1.0).
5. **Builder:** per file, resolve every field type of matching classes first. Resolution errors become that
   file's error. Otherwise compute the dependency files, apply the extended up-to-date check, then generate.
6. **Emitter/template:** expression generation switches on the resolved kind (table above); the template
   emits `enum_maps` instead of the file's `@JsonEnum` enums.

## Acceptance criteria

- [x] Every row of the “What each kind generates” table has a Dart golden fixture that analyzes cleanly and
      round-trips, including enums from another file, through an `export`, and through an import prefix.
      *(Golden files: `cross_file_model` (enums from another file, prefixed, un-annotated),
      `export_model` (an enum and a class through a barrel file's `export`s, imported as
      `package:<self>/…`), `prefixed_model`, `core_types_model`, `external_model`, `user_model`. The error rows
      are errors, so they're covered by build tests instead: `test_class_without_from_json_is_an_error`,
      `test_mixins_typedefs_and_extension_types_are_errors`, `test_unsupported_field_type_is_reported_with_its_line`.)*
- [x] The Problem table's model generates compiling code, except the record field, which gets the error above.
      *(Each row is in one of the golden files above; a record with `@JsonKey` hooks is in `prefixed_model`.)*
- [x] Two visible declarations with the same name produce an ambiguity error naming both files.
      *(`test_ambiguous_type_is_an_error_naming_both_files`.)*
- [x] Editing an enum in `lib/src/color.dart` regenerates `model.g.dart` on the next non-forced build.
      *(`test_editing_an_enum_in_another_file_regenerates_its_map`.)*
- [x] An unresolved name in a file that imports another package generates as today and warns once; listing
      it in `external_types` removes the warning. *(`test_unknown_type_from_another_package_warns_once`.)*
- [x] Existing snapshots and the example's output don't change (they only use same-file types).
      *(One intended exception: step 5 renamed the enum lookup's parameter to `entry`, fixing a pre-existing
      shadowing bug, which changed one token in every enum decode.)*
- [x] On the 1,000-file benchmark, a no-op build stays under 100 ms and a `--force` build is faster than
      today's 1.9 s (engine-only timer, same machine class, method recorded in the benchmark script).
      *(`engine/bench/run.sh 1000 5`, 4 cores, after step 6: no-op 84–93 ms, `--force` 268–287 ms, parse-only
      70–78 ms.)*

## Plan

Each step is mergeable on its own and keeps CI green.

1. ✅ A1: compile queries once, plus the benchmark script (`engine/bench/run.sh`). No output changes.
   Measured on 1,000 files, 4 cores: `--force` 1.95 s → ~0.28 s, parse-only 1.67 s → ~64 ms, no-op
   unchanged at ~10 ms. The index pass costs about as much as parse-only, which is within the 100 ms budget.
2. ✅ Parser: keep type prefixes (fixes `mMoney`), `Unsupported` for records and function types with a
   diagnostic, declarations and directives in `ParsedFile`. The diagnostic only fires when a conversion would
   be generated (not with `@JsonKey` hooks, a converter or `ignore`). Code review found three more cases,
   now covered: generic function types (`Function<T>`), classes with a `static fromJson` method, and
   mixin-application classes (`class M = Object with Mx;`). `Map<K, V>` type arguments are now split at the
   top-level comma, so `Map<Map<String, int>, int>` parses correctly (part of R14).
3. ✅ `dart:core` table (`num`, `dynamic`, `Object`, `Uri`, `BigInt`, `Duration`, `Set`, `Iterable`). This needs
   no index. Golden fixtures (`core_types_model.dart`: every type, nullable variants and nesting, round-tripped)
   and a gold snapshot. `Map<String, dynamic>` fields now pass values through instead of `dynamic.fromJson`.
4. ✅ `SymbolIndex`, resolution order, ambiguity errors, `Generator` trait change (`generate` receives
   `&ResolvedTypes`; templates get `resolved_types`). Output is unchanged; the emitter starts using the
   resolutions in steps 5 and 6.
   - Code review found two gaps, both fixed and tested. Type arguments of generic custom types
     (`Page<User>`) were never resolved. And the up-to-date check ran before resolution, so a change in another
     file went unnoticed. The **dependency-aware up-to-date check was pulled forward from step 6**: types are
     resolved on every build before the check, and an output is also stale when a file one of its types is
     declared in is newer. Limitation: a change that only rewires re-exports (a file in the middle of an
     `export` chain) isn't a dependency; `--force` covers it until the Phase 3 cache.
   - Measured (`engine/bench/run.sh 1000 5`, 4 cores): a no-op build now parses every file, **~83 ms typical,
     97 ms worst of 5** (was ~10 ms), inside the 100 ms budget but with little margin; `--force` unchanged at
     ~0.27 s. The index cache from decision 4 is likely needed for larger projects.
5. ✅ Enum maps per using library; un-annotated and cross-file enums. The index keeps each enum's
   declaration, and the emitter adds a `_$…EnumMap` for every enum its generated conversions use
   (`_$m_MoodEnumMap` for a prefixed `m.Mood`), after the file's own `@JsonEnum` maps and in order of first
   use. Fields with a converter, `@JsonKey` hooks or `ignore` don't pull a map in. The `flint_json` template
   iterates `enum_maps`; `enums` keeps its meaning. Found along the way:
   - **Shadowing bug (pre-existing):** a `List<SomeEnum>` decoded with
     `(e) => …firstWhere((e) => e.value == e)`, comparing each entry with itself, so it never matched. No
     fixture covered it. The lookup's parameter is now `entry`, which changes one token in every enum
     decode (snapshots and the example regenerated).
   - **Code review:** a class type parameter with the same name as an enum must win (`class Box<Kind>` next
     to an imported `enum Kind`); fixed and tested.
6. ✅ Class checks (`fromJson`), `external_types`, unresolved-name rules. (The dependency-aware up-to-date
   check moved to step 4.) The checks run in the `flint_json` emitter, only for types a generated conversion
   uses (no converter, hooks or `ignore`); a class only needs `fromJson` when the `fromJson` side is
   generated. Errors name the line, field, problem and fix. Changes from the draft:
   - **`class_annotations` no longer excuse a missing `fromJson`:** the generated code calls `T.fromJson`, so
     `@JsonSerializable` alone would still not compile (json_serializable rejects it too).
   - **Warnings:** `Generator::generate` returns `Generated { code, assumed_external }`; the builder
     prints one warning per unprefixed name (`Money` and `m.Money` share a fix), listing where it's used.
     Warnings appear when a file is generated, not on up-to-date builds. `external_types` entries match a
     name with or without its prefix, and also apply in files that import no other package.
   - "Imports another package" means an `import 'package:x/…'` in the file's library (parts included) where
     `x` isn't this package; `dart:` libraries don't count.
   - Golden fixture: `external_model.dart` uses `Money` from a local path package
     (`tests/dart_golden/packages/golden_money`), plain and prefixed, listed in `external_types`.
7. ✅ Docs: support matrix, template context, SDD §4/§6/§7, roadmap, review. Closing checks added an
   `export` golden fixture (`export_model.dart`, with its snapshot) and a `flint_json` test that editing an
   enum in another file regenerates the map. The benchmark was re-run (see the acceptance criteria).

## Follow-ups

- **Index cache (decision 4):** the no-op build is inside the 100 ms budget with ~10% margin on 1,000 files.
  A cache in `.dart_tool/flint/` is Phase 3 work, together with content hashes (R11).
- **Re-export changes:** a file in the middle of an `export` chain isn't a dependency of the output; `--force`
  covers it until the cache.
- **Warnings on up-to-date builds:** the `external_types` warning appears when a file is generated. Replaying
  it on no-op builds needs the cache too.
- **Not in scope, still open:** constructors (R8), `$enumDecode` and `unknownEnumValue` (R10), non-`String`
  map keys other than enums (R14), `toJson` checks for `explicitToJson` on classes without `toJson`.

## Decisions

The open questions were accepted with the proposed answers:

1. **Unresolved names in files that import other packages:** warn once per type name and assume a class
   (today's behaviour); `external_types` silences the warning. Names that can't come from anywhere are errors.
2. **`@JsonEnum` enums not used in their own file:** keep emitting their map there, as today. `alwaysCreate`
   can come later.
3. **`external_types`:** per plugin, under `plugins.flint_json`.
4. **Budget:** a no-op build of the 1,000-file benchmark stays under 100 ms (engine-only). If A1 alone doesn't
   get there, add an index cache in `.dart_tool/flint/`.
