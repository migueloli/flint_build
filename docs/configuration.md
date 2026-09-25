# Configuration and Template Reference

This page describes **current behaviour** (engine `0.1.0`). Known gaps are marked ⚠️ and link to
[REVIEW.md](REVIEW.md).

- [Commands](#commands)
- [`flint.yaml`](#flintyaml)
- [Using an existing `build.yaml`](#using-an-existing-buildyaml)
- [`flint_json` support matrix](#flint_json-support-matrix)
- [Custom templates](#custom-templates)
- [Template context](#template-context)

---

## Commands

Run these from the package root, where `pubspec.yaml` (and optionally `flint.yaml` / `build.yaml`) live.

| Command | What it does |
| ------- | ------------ |
| `dart run flint_build build` | Generates `<file>.g.dart` for every changed `lib/**.dart` file. A file is skipped if its output is newer than the source. |
| `dart run flint_build build -d` | Ignores modification times and regenerates everything. The long form is `--delete-conflicting-outputs`. |
| `dart run flint_build watch [-d]` | Builds, then rebuilds when anything under `lib/` changes (500 ms debounce). ⚠️ Don't use `-d` with `watch` ([R5](REVIEW.md)). |
| `dart run flint_build clean` | Deletes generated files. ⚠️ It currently deletes **every** `*.g.dart` under `lib/`, including files from other generators ([R1](REVIEW.md)). |

Set `RUST_LOG=debug` (or `trace`) for detailed logs from the engine.

⚠️ Changing `flint.yaml`, `build.yaml` or a template doesn't trigger regeneration by itself. Run
`build -d` afterwards ([R11](REVIEW.md)).

## `flint.yaml`

```yaml
plugins:
  <plugin_name>:
    class_annotations:   [String]   # e.g. ["@JsonSerializable"]
    enum_annotations:    [String]   # e.g. ["@JsonEnum"]
    field_annotations:   [String]   # accepted, currently unused ⚠️ R9
    variant_annotations: [String]   # accepted, currently unused ⚠️ R9
    converters:          [String]   # e.g. ["@EpochDateTimeConverter"]
    field_rename:        String     # see below
    template_path:       String     # relative to the package root
    # flint_json only: package-wide defaults for the matching annotation arguments
    explicit_to_json:    bool       # @JsonSerializable(explicitToJson:)   default false
    create_factory:      bool       # @JsonSerializable(createFactory:)    default true
    create_to_json:      bool       # @JsonSerializable(createToJson:)     default true
    include_if_null:     bool       # @JsonKey(includeIfNull:), nullable fields only   default true
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
- ⚠️ All plugins write to the same `<file>.g.dart`, so only one plugin can target a given file today
  ([R4](REVIEW.md)).
- ⚠️ Unknown keys are ignored silently, so check your spelling.

### Precedence

For `flint_json`, each setting is taken from the first place that sets it:

1. the annotation (`@JsonSerializable(explicitToJson: …)`, `@JsonKey(includeIfNull: …)`);
2. `flint.yaml`;
3. `build.yaml` (json_serializable options);
4. json_serializable's defaults.

### `field_rename`

This applies to fields without an explicit `@JsonKey(name: …)`. An unknown value leaves names unchanged.

| Value (`x` or `x_case`) | `myFieldName` becomes |
| ----------------------- | --------------------- |
| `snake` | `my_field_name` |
| `screaming_snake` | `MY_FIELD_NAME` |
| `kebab` | `my-field-name` |
| `screaming_kebab` | `MY-FIELD-NAME` |
| `pascal` | `MyFieldName` |
| `camel` | `MyFieldName` ⚠️ (same as `pascal`; see [SDD open question 2](SDD.md#16-open-questions)) |
| `lower_camel` | `myFieldName` |

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

## `flint_json` support matrix

| Feature | Status | Notes |
| ------- | :----: | ----- |
| `String`, `int`, `double`, `bool`, `DateTime` (and nullable) | ✅ | |
| `List<T>`, `Map<String, V>` (nested, nullable) | ✅ | Non-`String` map keys aren't converted ⚠️ R14 |
| Nested `@JsonSerializable` classes | ✅ | Called as `Type.fromJson(json as Map<String, dynamic>)` |
| Generic classes `Foo<T>` | ✅ | Always generates `fromJsonT` / `toJsonT` parameters, as with `genericArgumentFactories: true` |
| `@JsonEnum` enums **in the same file** | ✅ | ⚠️ Enums from other files are treated as classes (R7) |
| `@JsonValue('x')` | ✅ | ⚠️ Numeric/bool values become strings (R6) |
| `num`, `dynamic`, `Object`, `Uri`, `BigInt`, `Duration`, `Set`, records | ❌ | Generated as `Type.fromJson(...)`, which doesn't compile (R7) |
| Positional constructors, fields not set by the constructor | ❌ | Always generates named arguments for every field (R8) |
| Classes with several annotations (`@immutable @JsonSerializable()`) | ❌ | Silently skipped (R3) |
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
contains a matching class or enum. The output is written to `<file>.g.dart`.

```yaml
# flint.yaml
plugins:
  describe:
    class_annotations: ["@Describe"]
    template_path: "tool/templates/describe.tera"
```

```jinja
{# tool/templates/describe.tera #}
// GENERATED CODE - DO NOT MODIFY BY HAND

part of '{{ filename }}';
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

## Template context

These variables are available in every template, for both built-in and custom plugins:

| Variable | Type | Description |
| -------- | ---- | ----------- |
| `filename` | string | The source file's name, e.g. `user_model.dart` (for `part of`) |
| `classes` | array | Classes carrying one of the plugin's `class_annotations` |
| `enums` | array | Enums carrying one of the plugin's `enum_annotations` |

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
  "dart_type": { "kind": "Int", "is_nullable": true },
  "metadata": { "JsonKey": "", "name": "'id_'" },
  "converter": null,        // set by flint_json only
  "from_json_expr": null,   // set by flint_json only
  "to_json_expr": null      // set by flint_json only
}
```

`dart_type.kind` is one of:

- `"String"`, `"Int"`, `"Double"`, `"Bool"`, `"DateTime"`
- `{ "List": <DartType> }`
- `{ "Map": [<DartType key>, <DartType value>] }`
- `{ "Custom": "TypeName" }`

**Enum**

```jsonc
{
  "name": "E",
  "annotations": ["Model"],
  "values": [ { "name": "a", "value": "1" }, { "name": "b", "value": null } ]
}
```

> **Stability:** this context isn't versioned yet, and field names may change (see
> [SDD §6.2](SDD.md#62-target-parsed-model)). Metadata values are raw Dart source, so string arguments keep
> their quotes.
