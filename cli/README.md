# flint_build (Dart CLI)

The Dart entry point for [Flint](../README.md), a native, parallel replacement for `build_runner` +
`json_serializable`. This package contains only a launcher: `dart run flint_build` finds the Rust engine
binary and runs it with your arguments.

> **Experimental.** Read the [known limitations](../README.md#known-limitations) first. In particular,
> `clean` currently deletes every `*.g.dart` under `lib/`.

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
    field_rename: snake_case             # snake | kebab | pascal | screaming_snake | …
    explicit_to_json: true               # package-wide default, like build.yaml's option
    converters: ["@EpochDateTimeConverter"]
```

Settings in `flint.yaml` take priority over `build.yaml`, and annotation arguments take priority over both.

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
enum Status { active, inactive }   // must be declared in the same file for now
```

The full `flint.yaml` reference and the feature support matrix are in
[docs/configuration.md](../docs/configuration.md).

## Run

```bash
dart run flint_build build       # generate changed files
dart run flint_build build -d    # regenerate everything
dart run flint_build watch       # rebuild on changes under lib/
dart run flint_build clean       # delete generated *.g.dart (see warning above)
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

The template receives `filename`, `classes` and `enums`. See the
[template context reference](../docs/configuration.md#template-context). Today every plugin writes to
`<file>.g.dart`, so don't point two plugins at the same source file.

## License

[MIT](../LICENSE)
