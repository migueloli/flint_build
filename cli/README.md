# flint_build (Dart CLI)

The Dart entry point for [Flint](../README.md), a native, parallel replacement for `build_runner`. Its first
built-in generator covers `json_serializable`; more are planned. This package contains only a launcher: `dart run flint_build` finds the Rust engine
binary and runs it with your arguments.

> **Experimental.** Read the [known limitations](../README.md#known-limitations) first.

## Install

Flint isn't published yet. Use a path (or git) dependency that points at a checkout of this repository. The
launcher expects the engine to be at `../engine` relative to this package.

```yaml
# your_app/pubspec.yaml
dependencies:
  json_annotation: ^4.9.0          # still provides the annotations

dev_dependencies:
  flint_build:
    path: ../flint_build/cli
```

On the first run, if `engine/target/release/flint_build` is missing, the launcher runs
`cargo build --release` for you. That needs [Rust 1.88+](https://rustup.rs). If only a debug build exists,
the launcher uses it and prints a warning.

> The launcher doesn't rebuild an engine that already exists. After pulling engine changes, run
> `cargo build --release` in `engine/` yourself.

## Configure

**Already using json_serializable?** You don't need any new config. With no `flint.yaml`, Flint enables its
JSON generator when `pubspec.yaml` depends on `json_serializable`, and reads your `build.yaml` options
(`field_rename`, `explicit_to_json`, `include_if_null`, …). See
[Using an existing `build.yaml`](../docs/configuration.md#using-an-existing-buildyaml).

Otherwise, create `flint.yaml` next to `pubspec.yaml`. The smallest config enables the built-in JSON
generator with json_serializable's annotation names:

```yaml
plugins:
  flint_json:
```

With options:

```yaml
plugins:
  flint_json:
    field_rename: snake_case             # none | snake | kebab | pascal | camel | screaming_snake | …
    explicit_to_json: true               # package-wide default, like build.yaml's option
    converters: ["@EpochDateTimeConverter"]
    external_types: [Money]              # classes from other packages that have fromJson/toJson
```

Settings in `flint.yaml` take priority over `build.yaml`, and annotation arguments take priority over both.

Flint reads your package, not its dependencies. A field whose type comes from another package works as
long as that class has `fromJson`/`toJson`; Flint warns once about it until you list it in `external_types`.
A type it can't convert (a class without `fromJson`, a mixin, a typedef, an unknown name) is an error that
names the line and a fix.

Your models look the same as with json_serializable:

```dart
import 'package:json_annotation/json_annotation.dart';

part 'user.g.dart';

@JsonSerializable()
class User {
  final int id;
  @JsonKey(name: 'display_name')
  final String name;
  final Status status;

  User({required this.id, required this.name, required this.status});

  factory User.fromJson(Map<String, dynamic> json) => _$UserFromJson(json);
  Map<String, dynamic> toJson() => _$UserToJson(this);
}

@JsonEnum()
enum Status { active, inactive }   // could also be imported from another file
```

The full `flint.yaml` reference and the feature support matrix are in
[docs/configuration.md](../docs/configuration.md).

## Run

```bash
dart run flint_build build           # generate changed files
dart run flint_build build --force   # regenerate everything (alias: -d)
dart run flint_build watch           # rebuild on changes under lib/
dart run flint_build clean           # delete the .g.dart files Flint generated
dart run flint_build dump-model lib/user.dart   # print the generator model as JSON (--schema: its JSON Schema)
```

## Custom generators

Any plugin name other than `flint_json` is a custom generator driven by a
[Tera](https://keats.github.io/tera/docs/) template:

```yaml
plugins:
  describe:
    class_annotations: ["@Describe"]
    template_path: tool/templates/describe.tera
```

```jinja
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

The template receives `filename`, `classes` and `enums`. See the
[template context reference](../docs/configuration.md#template-context). Flint adds the file header and the
`part of` line itself. When several plugins match the same file, each one gets its own section of the same
`<file>.g.dart`, in `flint.yaml` order.

## Which files Flint touches

Flint writes `<file>.g.dart` only when `<file>.dart` has an annotated declaration **and** a
`part '<file>.g.dart';` directive. It marks every file it writes with a `// flint_build` line, and never
overwrites or deletes a `.g.dart` without that marker (build_runner's output, for example) unless you pass
`--force`. See [Generated files](../docs/configuration.md#generated-files).

## License

[MIT](../LICENSE)
