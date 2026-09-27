# Configuration and Template Reference

This page describes **current behaviour** (engine `0.1.0`). Known gaps are marked ⚠️ and link to
[REVIEW.md](REVIEW.md).

- [Commands](#commands)
- [`flint.yaml`](#flintyaml)
- [Using an existing `build.yaml`](#using-an-existing-buildyaml)
- [Generated files](#generated-files)
- [`flint_json` support matrix](#flint_json-support-matrix)
- [Custom templates](#custom-templates)
- [Template context](#template-context)

---

## Commands

Run these from the package root, where `pubspec.yaml` (and optionally `flint.yaml` / `build.yaml`) live.

| Command | What it does |
| ------- | ------------ |
| `dart run flint_build build` | Generates `<file>.g.dart` for every changed `lib/**.dart` file (see [Generated files](#generated-files)). A file is skipped if its output is newer than the source. |
| `dart run flint_build build --force` | Regenerates everything, and overwrites `.g.dart` files Flint didn't write. Aliases: `-f`, `-d`, `--delete-conflicting-outputs`. |
| `dart run flint_build watch [--force]` | Builds, then rebuilds once per change under `lib/` (500 ms debounce). Flint's own writes don't trigger rebuilds. |
| `dart run flint_build clean` | Deletes the `.g.dart` files Flint generated. Files from other generators are left alone. |

A build reports errors per file and keeps going; if any file failed, it exits with a non-zero code.

Set `RUST_LOG=debug` (or `trace`) for detailed logs from the engine.

A file counts as up to date when its output is newer than the source, than every project file its field
types are declared in, **and** than `flint.yaml`, `build.yaml`, `pubspec.yaml`, every template, and the engine
binary. So editing the config, a template, or
upgrading Flint regenerates everything on the next build. ⚠️ This is based on modification times, not
content: a checkout that restores an older mtime can still leave stale output. Use `build --force` if in
doubt ([R11](REVIEW.md)).

## `flint.yaml`

```yaml
plugins:
  <plugin_name>:
    class_annotations:   [String]   # e.g. ["@JsonSerializable"]
    enum_annotations:    [String]   # e.g. ["@JsonEnum"]
    field_annotations:   [String]   # accepted, currently unused ⚠️ R9
    variant_annotations: [String]   # enum-constant annotations that set the JSON value, e.g. ["@JsonValue"]
    converters:          [String]   # e.g. ["@EpochDateTimeConverter"]
    field_rename:        String     # see below
    template_path:       String     # relative to the package root
    # flint_json only: package-wide defaults for the matching annotation arguments
    explicit_to_json:    bool       # @JsonSerializable(explicitToJson:)   default false
    create_factory:      bool       # @JsonSerializable(createFactory:)    default true
    create_to_json:      bool       # @JsonSerializable(createToJson:)     default true
    include_if_null:     bool       # @JsonKey(includeIfNull:), nullable fields only   default true
    external_types:      [String]   # classes from other packages with fromJson/toJson, e.g. [Money]
```

`flint.yaml` is **optional** for json_serializable projects. See
[Using an existing `build.yaml`](#using-an-existing-buildyaml).

- **Plugin names:** `flint_json` is built in. Any other name is a *custom plugin* and needs `template_path`.
  An unknown plugin without a template prints a warning and is skipped.
- **Annotation matching:** entries include the `@` and are matched against the annotation's name, ignoring its
  arguments. So `"@JsonSerializable"` matches `@JsonSerializable()` and `@JsonSerializable(explicitToJson: true)`.
  Prefixed annotations (`@json.JsonSerializable()`) aren't matched.
- **`flint_json` defaults:** any list you leave empty or leave out is filled in: `@JsonSerializable`,
  `@JsonKey`, `@JsonEnum`, `@JsonValue`. The smallest valid config is:

  ```yaml
  plugins:
    flint_json:
  ```

- **`template_path` on `flint_json`** replaces the built-in template entirely.
- **Plugin order:** plugins run in the order they appear in `flint.yaml`. When several match the same file,
  their sections appear in that order in its `.g.dart`.
- ⚠️ Unknown keys are ignored silently, so check your spelling. (Unknown `field_rename` *values* are errors.)

### Precedence

For `flint_json`, each setting is taken from the first place that sets it:

1. the annotation (`@JsonSerializable(explicitToJson: …)`, `@JsonKey(includeIfNull: …)`);
2. `flint.yaml`;
3. `build.yaml` (json_serializable options);
4. json_serializable's defaults.

### `field_rename`

This applies to fields without an explicit `@JsonKey(name: …)`. An unknown value is an error that lists the
valid ones ([spec 0003](specs/0003-field-rename-camel.md)).

| Value (`x` or `x_case`) | `myFieldName` becomes | `user_id` becomes |
| ----------------------- | --------------------- | ----------------- |
| `none` | `myFieldName` | `user_id` |
| `snake` | `my_field_name` | `user_id` |
| `screaming_snake` | `MY_FIELD_NAME` | `USER_ID` |
| `kebab` | `my-field-name` | `user-id` |
| `screaming_kebab` | `MY-FIELD-NAME` | `USER-ID` |
| `pascal` | `MyFieldName` | `UserId` |
| `camel` (alias `lower_camel`) | `myFieldName` | `userId` |

`camel` means lowerCamelCase, as in serde's `camelCase`. Since Dart fields are already lowerCamelCase, it only
changes names that aren't.

## Using an existing `build.yaml`

Projects migrating from build_runner can keep their json_serializable settings in `build.yaml`
([spec 0002](specs/0002-read-build-yaml.md)):

```yaml
# build.yaml
targets:
  $default:
    builders:
      json_serializable:          # or json_serializable:json_serializable
        options:
          field_rename: snake
          explicit_to_json: true
```

- **No `flint.yaml`?** `flint_json` is enabled automatically when `build.yaml` configures the json_serializable
  builder (unless it has `enabled: false`), or when `pubspec.yaml` depends on `json_serializable`.
- **Both files?** `build.yaml` only fills in settings the `flint_json` entry in `flint.yaml` leaves unset.
  Custom plugins ignore `build.yaml`.
- **Supported options:** `field_rename` (`none`, `kebab`, `snake`, `pascal`, `screamingSnake`),
  `explicit_to_json`, `create_factory`, `create_to_json`, `include_if_null`. `generic_argument_factories` is
  accepted; Flint always generates factory parameters for generic classes.
- **Anything else** (`any_map`, `checked`, `disallow_unrecognized_keys`, …) prints a warning if it's set to a
  non-default value. A supported option with an invalid value is an error.
- Only the `$default` target is read, and `generate_for` is ignored. Flint always processes `lib/`.

Flint prints where its configuration came from:

```text
ℹ️ No flint.yaml found; enabling flint_json because json_serializable is configured in build.yaml.
ℹ️ Using json_serializable options from build.yaml: field_rename, explicit_to_json.
⚠️ build.yaml: json_serializable option 'any_map' is not supported by Flint and was ignored.
```

## Generated files

Rules for writing, keeping and deleting `.g.dart` files ([spec 0001](specs/0001-generated-output-ownership.md)):

- **When a file is written:** `lib/a/b.g.dart` is written only when `b.dart` has at least one declaration
  matching a plugin **and** contains `part 'b.g.dart';`. If the directive is missing, Flint prints a warning
  saying which line to add.
- **Ownership:** every file Flint writes starts like this:

  ```dart
  // GENERATED CODE - DO NOT MODIFY BY HAND
  // flint_build 0.1.0

  part of 'b.dart';
  ```

  The `// flint_build` line marks the file as Flint's. Files from older Flint versions are recognised by
  their `(Powered by Flint)` banner.
- **Other generators' files are safe:** if `b.g.dart` exists without the marker (for example, build_runner
  wrote it), Flint reports an error for that file and leaves it untouched. Use `--force` to replace it.
  `clean` never deletes such files.
- **Several plugins:** each matching plugin adds a section, under a banner with the plugin's name, in
  `flint.yaml` order. There is one header and one `part of` per file.
- **Stale outputs:** an owned `b.g.dart` is deleted when `b.dart` no longer has matching declarations, no
  longer has the `part` directive, or no longer exists.
- **Unchanged output isn't rewritten,** so file timestamps (and watchers) are left alone.

### Type names that could mean two things

Flint looks up each field's type the way Dart does: declarations in the same library first, then the files it
imports (following `export`s, `as` prefixes and `show`/`hide`). If two different declarations with the same
name are visible, the file gets an error naming both files; pick one with an import prefix or `show`/`hide`
(spec 0005).

### Types Flint can't convert

`flint_json` checks every type a generated conversion uses (fields with a converter, `@JsonKey(fromJson:,
toJson:)` hooks, or `ignore` are skipped). These are errors for that file, with the line and a fix, and its
`.g.dart` is left as it was:

- a class in this package with no `fromJson` constructor, factory or static method (a class only written to
  JSON, with `createFactory: false`, doesn't need one). `@JsonSerializable` alone isn't enough: the generated
  code calls `Type.fromJson(...)`;
- a mixin, typedef or extension type;
- a name that isn't declared in this package or anything the file imports, when the file imports no other
  package.

### Classes from other packages

Flint doesn't read other packages, so a type from one (say `Money` from `package:money`) is a name it can't
find. If the file imports another package, Flint assumes the name is a class with `fromJson`/`toJson`, generates
`Money.fromJson(...)` as json_serializable would, and prints one warning per name when it generates the
file. List the name to confirm it and silence the warning:

```yaml
plugins:
  flint_json:
    external_types: [Money]   # also covers prefixed uses like m.Money
```

A listed name is used as a class even in a file that imports no other package. Names that *are* declared in
this package ignore the list.

## `flint_json` support matrix

| Feature | Status | Notes |
| ------- | :----: | ----- |
| `String`, `int`, `double`, `bool`, `DateTime` (and nullable) | ✅ | |
| `List<T>`, `Map<String, V>` (nested, nullable) | ✅ | Non-`String` map keys aren't converted ⚠️ R14 |
| Nested classes, from any file in the package | ✅ | Called as `Type.fromJson(json as Map<String, dynamic>)`; the class needs a `fromJson` constructor, factory or static method, or the file gets an error |
| Classes from other packages | ✅ | As nested classes. Warns once per name unless listed in `external_types`; see [Classes from other packages](#classes-from-other-packages) |
| Classes through an import prefix (`m.Money`) | ✅ | Called as `m.Money.fromJson(...)` |
| Generic classes `Foo<T>` | ✅ | Always generates `fromJsonT` / `toJsonT` parameters, as with `genericArgumentFactories: true` |
| Enums, in the same file or another one, with or without `@JsonEnum` | ✅ | Each generated file gets its own copy of the value map (`_$StatusEnumMap`, or `_$m_MoodEnumMap` for `m.Mood`), because a private map only works inside its own library |
| `@JsonValue('x')`, `@JsonValue(1)`, `@JsonValue(true)` | ✅ | The value keeps its type and is emitted as written (plain `"x"` becomes `'x'`). As map keys, enum values are converted to strings, since JSON keys always are |
| `num`, `dynamic`, `Object` (and nullable) | ✅ | Taken as they are (`as num`, `as Object`, no cast for `dynamic`/`Object?`) |
| `Uri`, `BigInt` | ✅ | Written as strings: `Uri.parse(...)` / `BigInt.parse(...)` and `toString()` |
| `Duration` | ✅ | Written as microseconds: `Duration(microseconds: …)` and `inMicroseconds` |
| `Set<E>`, `Iterable<E>` | ✅ | Read from and written as JSON lists (`toSet()`, `toList()`) |
| Records, function types, fields without a declared type | ❌ | Reported as an error for that file (with the line), unless the field has `@JsonKey(fromJson:, toJson:)`, a converter, or is ignored |
| Mixins, typedefs, extension types | ❌ | Reported as an error for that file, with the same exceptions |
| Positional constructors, fields not set by the constructor | ❌ | Always generates named arguments for every field (R8) |
| Classes and enums with several annotations (`@immutable @JsonSerializable()`) | ✅ | In any order |
| `@JsonSerializable(explicitToJson: true)` | ✅ | Package-wide default: `explicit_to_json` |
| `@JsonSerializable(createFactory: false / createToJson: false)` | ✅ | Package-wide defaults: `create_factory` / `create_to_json` |
| `@JsonSerializable(includeIfNull: false)` | ✅ | Applies to nullable fields. Package-wide default: `include_if_null` |
| `@JsonSerializable(fieldRename: …)` | ❌ | Use `field_rename` in `flint.yaml` or `build.yaml` instead |
| `@JsonKey(name: …)` | ✅ | |
| `@JsonKey(defaultValue: …)` | ✅ | |
| `@JsonKey(ignore: true)` / `includeFromJson` / `includeToJson` | ✅ | |
| `@JsonKey(includeIfNull: false)` | ✅ | |
| `@JsonKey(fromJson: fn, toJson: fn)` | ✅ | |
| `@JsonKey(unknownEnumValue: …)`, `required`, `disallowNullValue`, `readValue` | ❌ | |
| Converter classes (`converters:` in `flint.yaml`) | ✅ | Field-level only; emits `const Converter().fromJson(...)` |

## Custom templates

A custom plugin renders a [Tera](https://keats.github.io/tera/docs/) template once for every source file that
contains a matching class or enum. The result becomes that plugin's section of `<file>.g.dart`.

The engine writes the file header and the `part of` line, so a template only renders declarations. Older
templates that still start with `// GENERATED CODE…` or `part of …` keep working: those lines are removed from
the start of the section.

```yaml
# flint.yaml
plugins:
  describe:
    class_annotations: ["@Describe"]
    template_path: "tool/templates/describe.tera"
```

```jinja
{# tool/templates/describe.tera #}
{% for class in classes %}
extension {{ class.name }}Describe on {{ class.name }} {
  List<String> get fieldNames => const [
{%- for field in class.fields %}
    '{{ field.name }}',
{%- endfor %}
  ];
}
{% endfor %}
```

Tip: while writing a template, dump the whole context with `{{ classes | json_encode(pretty=true) }}`.

**When a template has a problem**, the build reports it and exits non-zero instead of crashing:

- A missing file or a syntax error is reported once, with Tera's explanation (line, column and what it
  expected). The files that plugin matches are **left untouched**, so their existing output survives a typo.
  Other files build normally.
- A render error (for example an unknown variable) is reported for each source file it happens in, and that
  file's output isn't changed.

## Template context

These variables are available in every template, for both built-in and custom plugins:

| Variable | Type | Description |
| -------- | ---- | ----------- |
| `filename` | string | The source file's name, e.g. `user_model.dart` (for `part of`) |
| `classes` | array | Classes carrying one of the plugin's `class_annotations` |
| `resolved_types` | map | For each type name used in those classes' fields (as written, e.g. `Money` or `m.Money`): `{ kind, file, has_from_json, has_to_json }`, where `kind` is `class`, `enum`, `mixin`, `type_alias`, `extension_type` or `unresolved`, and `file` is the declaring file (`lib/src/money.dart`) or null |
| `enums` | array | Enums carrying one of the plugin's `enum_annotations` |
| `enum_maps` | array | `flint_json` only: the value maps this file needs, `{ map_name, type_name, values }`, including enums from other files |

**Class**

```jsonc
{
  "name": "M",
  "type_parameters": ["T"],
  "metadata": { "Model": "", "tag": "'x'" },   // annotation names → "", named args → raw source text
  "fields": [ /* Field */ ]
}
```

**Field**

```jsonc
{
  "name": "id",
  "is_final": true,
  "line": 12,               // 1-based line of the declaration
  "dart_type": { "kind": "Int", "is_nullable": true },
  "metadata": { "JsonKey": "", "name": "'id_'" },
  "converter": null,        // set by flint_json only
  "from_json_expr": null,   // set by flint_json only
  "to_json_expr": null      // set by flint_json only
}
```

`dart_type.kind` is one of:

- `"String"`, `"Int"`, `"Double"`, `"Bool"`, `"DateTime"`, `"Num"`, `"Dynamic"`, `"Object"`, `"Uri"`,
  `"BigInt"`, `"Duration"`
- `{ "Set": <DartType> }`, `{ "Iterable": <DartType> }`
- `{ "List": <DartType> }`
- `{ "Map": [<DartType key>, <DartType value>] }`
- `{ "Custom": "TypeName" }`, including a prefix if the source has one (`"m.Money"`)
- `{ "Unsupported": "(int, String)" }` for records and function types (empty text: no declared type)

**Enum**

```jsonc
{
  "name": "E",
  "annotations": ["Model"],
  "values": [
    {
      "name": "a",
      "value": "1",     // from the first of the plugin's variant_annotations, else null
      "literal": "1",   // the same argument as Dart source: 1, true, 'x', "it's"
      "annotations": [
        { "name": "JsonValue", "value": "1", "literal": "1" },
        { "name": "Note", "value": "legacy", "literal": "'legacy'" }
      ]
    },
    { "name": "b", "value": null, "literal": null, "annotations": [] }
  ]
}
```

A custom plugin only gets `value` and `literal` if it sets `variant_annotations`. Every annotation on a
constant is also listed in `annotations`, with its first argument as `literal` (exact source) and `value` (one
pair of string quotes removed). Emit `literal` when generating Dart, so numbers stay numbers and quotes inside
strings stay valid.

> **Stability:** this context isn't versioned yet, and field names may change (see
> [SDD §6.2](SDD.md#62-target-parsed-model)). Metadata values are raw Dart source, so string arguments keep
> their quotes.
