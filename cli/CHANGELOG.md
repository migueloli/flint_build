# Changelog

The CLI and the engine share a version number. Engine changes are listed here too, because this package is
how they reach users.

## 0.1.0 (unreleased)

First version with a stable project layout. Highlights since the prototype:

- **Safe outputs** ([spec 0001](../docs/specs/0001-generated-output-ownership.md)): Flint only writes,
  overwrites or deletes `.g.dart` files it generated (marked with `// flint_build <version>`). A file is
  written only when its source has an annotated declaration and a `part '<file>.g.dart';` directive.
  `clean` leaves other generators' files alone.
- **Several plugins per file**, as sections in `flint.yaml` order.
- **`build.yaml` support** ([spec 0002](../docs/specs/0002-read-build-yaml.md)): json_serializable options
  are read from `build.yaml`, and `flint.yaml` is optional for json_serializable projects.
- **`--force`** replaces `--delete-conflicting-outputs` (kept as an alias, like `-d`).
- **Watch mode** rebuilds once per change instead of looping on its own writes.
- **Fixes:** classes and enums with several annotations (R3); typed `@JsonValue` values and quotes inside
  them (R6); template errors are reported instead of crashing (R12,
  [spec 0004](../docs/specs/0004-template-errors.md)).
- **Breaking:** `field_rename: camel` now means lowerCamelCase
  ([spec 0003](../docs/specs/0003-field-rename-camel.md)); unknown `field_rename` values are errors.
