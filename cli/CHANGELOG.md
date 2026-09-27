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
- **Types across files** ([spec 0005](../docs/specs/0005-project-symbol-index.md)): enums and
  classes from any file in the package (also through import prefixes and exports), `num`, `dynamic`,
  `Object`, `Uri`, `BigInt`, `Duration`, `Set` and `Iterable`. Types Flint can't convert (a class without
  `fromJson`, records, mixins, typedefs, extension types, unknown names) are errors with a fix, instead of
  code that doesn't compile. Classes from other packages warn once unless listed in `external_types`.
- **Constructors** ([spec 0006](../docs/specs/0006-constructor-aware-emission.md), in progress): `fromJson`
  calls the class's real constructor (positional and named parameters, defaults for missing keys,
  `@JsonSerializable(constructor:)`) and sets other writable fields with cascades. Every variable of
  `final int a, b;` is serialized (only `a` was), and static fields are no longer treated as instance fields.
  Custom templates get `class.constructors`, `class.getters` and new field flags.
- **Changed output, matching json_serializable:** private fields are skipped unless `@JsonKey` includes
  them; when `fromJson` is generated, fields it can't set (initialised `final`s) are left out of `toJson`;
  with `createFactory: false`, public getters are written. A `flint_json` `template_path` should use the new
  `class.from_json` and `class.json_members`.
- **`dump-model`** ([spec 0007](../docs/specs/0007-generator-platform.md), in progress): prints the generator
  model of Dart files as JSON (`--schema` for its JSON Schema), the input future Dart and YAML generators get.
- **Fix:** a constructor default like `Mood.calm` or `const Duration(seconds: 1).inSeconds` was cut to its
  first part, generating code that didn't compile.
- **`--force`** replaces `--delete-conflicting-outputs` (kept as an alias, like `-d`).
- **Watch mode** rebuilds once per change instead of looping on its own writes.
- **Fixes:** classes and enums with several annotations (R3); typed `@JsonValue` values and quotes inside
  them (R6); template errors are reported instead of crashing (R12,
  [spec 0004](../docs/specs/0004-template-errors.md)).
- **Breaking:** `field_rename: camel` now means lowerCamelCase
  ([spec 0003](../docs/specs/0003-field-rename-camel.md)); unknown `field_rename` values are errors.
