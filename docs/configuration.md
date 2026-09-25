# Configuration and Template Reference

This page describes **current behaviour** (engine `0.1.0`). Known gaps are marked ⚠️ and link to
[REVIEW.md](REVIEW.md).

- [Commands](#commands)
- [`flint.yaml`](#flintyaml)
- [`flint_json` support matrix](#flint_json-support-matrix)
- [Custom templates](#custom-templates)
- [Template context](#template-context)

---

## Commands

Run these from the package root, where `pubspec.yaml` and `flint.yaml` live.

| Command | What it does |
| ------- | ------------ |
| `dart run flint_build build` | Generates `<file>.g.dart` for every changed `lib/**.dart` file. A file is skipped if its output is newer than the source. |
| `dart run flint_build build -d` | Ignores modification times and regenerates everything. The long form is `--delete-conflicting-outputs`. |
| `dart run flint_build watch [-d]` | Builds, then rebuilds when anything under `lib/` changes (500 ms debounce). ⚠️ Don't use `-d` with `watch` ([R5](REVIEW.md)). |
| `dart run flint_build clean` | Deletes generated files. ⚠️ It currently deletes **every** `*.g.dart` under `lib/`, including files from other generators ([R1](REVIEW.md)). |

Set `RUST_LOG=debug` (or `trace`) for detailed logs from the engine.

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
```

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
| `@JsonSerializable(explicitToJson: true)` | ✅ | |
| `@JsonSerializable(createFactory: false / createToJson: false)` | ✅ | |
| `@JsonSerializable(fieldRename: …)` | ❌ | Use `field_rename` in `flint.yaml` instead |
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
