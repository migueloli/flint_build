# 0003 — `field_rename: camel` means lowerCamelCase; unknown values are errors

| | |
| --- | --- |
| **Status** | Done |
| **Resolves** | SDD open question 2 |
| **Touches** | `config/flint.rs`, `config/build_yaml.rs`, `generators/flint_json/emitter.rs` |

## Problem

`field_rename: camel` produced **PascalCase** (`MyFieldName`), which made it a second name for `pascal`. That
isn't what “camelCase” means in serde (`rename_all = "camelCase"`), in JSON APIs, or in Dart's style guide
(which says *UpperCamelCase* for the capitalised form). A misspelt value such as `snak` was silently ignored,
so it changed the JSON wire format without any warning.

## Behaviour

| Value | `myFieldName` / `user_id` become |
| ----- | -------------------------------- |
| `none` | unchanged |
| `snake`, `snake_case` | `my_field_name` / `user_id` |
| `screaming_snake`, `screaming_snake_case` | `MY_FIELD_NAME` / `USER_ID` |
| `kebab`, `kebab_case` | `my-field-name` / `user-id` |
| `screaming_kebab`, `screaming_kebab_case` | `MY-FIELD-NAME` / `USER-ID` |
| `pascal`, `pascal_case` | `MyFieldName` / `UserId` |
| `camel`, `camel_case`, `lower_camel`, `lower_camel_case` | `myFieldName` / `userId` **(changed: was PascalCase)** |

Any other value makes `flint.yaml` fail to load, with an error listing the valid values.

## Design

`PluginConfig.field_rename` becomes `Option<FieldRename>`, an enum parsed once (via `FromStr`) when
`flint.yaml` is deserialised. The emitter calls `FieldRename::apply` instead of matching strings, so the list
of valid names lives in one place. `build.yaml` maps json_serializable's names straight to the enum.

## Acceptance criteria

- [x] `camel` and `camel_case` produce `myFieldName`; `pascal` still produces `MyFieldName`.
- [x] `none` leaves names unchanged.
- [x] `field_rename: snak` in `flint.yaml` is an error naming the valid values.
- [x] `build.yaml` `field_rename` values map to the same strategies as before.
- [x] Existing snapshots don't change.
