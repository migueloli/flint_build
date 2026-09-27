# 0002 — Read json_serializable options from `build.yaml`

| | |
| --- | --- |
| **Status** | Done |
| **Resolves** | SDD open question 1 (“migrating needs no `flint.yaml`”) |
| **Touches** | `config/` (new `build_yaml.rs`, `flint.rs`, `pubspec.rs`, `mod.rs`), `builder.rs`, `generators/flint_json/emitter.rs` |

## Problem

Projects that move from build_runner already have their json_serializable settings in `build.yaml`. Flint
ignores that file and refuses to run without a `flint.yaml`. People have to copy settings by hand, and a
forgotten `field_rename: snake` silently changes the JSON wire format.

## Goals / non-goals

- **Goal:** a project using json_serializable can run `flint_build build` with **no `flint.yaml`**, and gets
  the same options it had with build_runner.
- **Goal:** when both files exist, `build.yaml` fills in what `flint.yaml` leaves unset.
- **Goal:** options Flint doesn't implement are reported, never silently ignored.
- **Non-goal:** `generate_for` globs, targets other than `$default`, or other builders' options.

## Behaviour

### Where the options are read from

```yaml
# build.yaml
targets:
  $default:
    builders:
      json_serializable:            # also: json_serializable:json_serializable, json_serializable|json_serializable
        enabled: true               # optional
        options:
          field_rename: snake
          explicit_to_json: true
          include_if_null: false
```

### Supported options

| `build.yaml` option | Flint setting | Values |
| ------------------- | ------------- | ------ |
| `field_rename` | `field_rename` | `none`, `kebab`, `snake`, `pascal`, `screamingSnake` |
| `explicit_to_json` | `explicit_to_json` | bool |
| `create_factory` | `create_factory` | bool |
| `create_to_json` | `create_to_json` | bool |
| `include_if_null` | `include_if_null` | bool |
| `generic_argument_factories` | *(accepted)* | Flint always generates factory parameters for generic classes |

Any other option set to something other than `false`, `null` or `""` produces the warning
`build.yaml: json_serializable option '<name>' is not supported by Flint and was ignored`.
A supported option with the wrong type, or an unknown `field_rename`, is an error.

The same four settings (`explicit_to_json`, `create_factory`, `create_to_json`, `include_if_null`) can now
also be set per plugin in `flint.yaml`.

### Precedence (highest first)

1. The annotation itself: `@JsonSerializable(explicitToJson: …)`, `@JsonKey(includeIfNull: …)`.
2. `flint.yaml` → `plugins.flint_json.<setting>`.
3. `build.yaml` → json_serializable `options`.
4. json_serializable's defaults.

The class-level `@JsonSerializable(includeIfNull: …)` is now also honoured. Plugin- and class-level
`includeIfNull` apply only to **nullable** fields, as in json_serializable.

### When there is no `flint.yaml`

The `flint_json` plugin is enabled implicitly when:

- `build.yaml` configures the json_serializable builder: its `enabled` flag decides (default `true`); or
- `build.yaml` doesn't mention it, but `pubspec.yaml` lists `json_serializable` in `dependencies` or
  `dev_dependencies`.

Otherwise the build fails with an error explaining both options. Flint prints a note saying where the
configuration came from.

## Design

- `config::build_yaml::parse(&str) -> Result<Option<JsonSerializableOptions>>`, a pure function.
- `config::resolve(flint_yaml: Option<&str>, build_yaml: Option<&str>, &Pubspec) -> Result<ProjectConfig>`
  is pure and unit-tested. `config::load_project_config(root, &Pubspec)` only reads the files.
- `ProjectConfig { flint: FlintConfig, notes, warnings }`. The builder prints the notes and warnings.
- The emitter fills in missing class metadata keys from the plugin defaults before rendering. The template
  is unchanged.

## Acceptance criteria

- [x] With no `flint.yaml` and `json_serializable` in `dev_dependencies`, `build` generates JSON code.
- [x] With no `flint.yaml`, no json_serializable in `pubspec.yaml`, and no `build.yaml`, `build` fails with an explanatory error.
- [x] With `enabled: false` in `build.yaml` and no `flint.yaml`, `flint_json` isn't enabled.
- [x] `field_rename: screamingSnake` in `build.yaml` produces `MY_FIELD` keys.
- [x] `flint.yaml` `field_rename: kebab` overrides `build.yaml` `field_rename: snake`.
- [x] `explicit_to_json: true` in `build.yaml` and `@JsonSerializable(explicitToJson: false)` on a class → that class doesn't call `toJson()`.
- [x] `include_if_null: false` adds `if (x != null)` only for nullable fields.
- [x] `any_map: true` produces a warning. `any_map: false` doesn't.
- [x] The existing snapshots don't change.
