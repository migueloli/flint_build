# 0006 — Constructor-aware emission: build `fromJson` from the real constructor

| | |
| --- | --- |
| **Status** | In progress (accepted with the proposed answers to the open questions) |
| **Resolves** | R8 (and a related bug found while writing this spec: `final int a, b;` loses `b`) |
| **Touches** | `parser/` (constructors, getters, field modifiers), `generators/flint_json/` (emitter, template), `index.rs` (superclass members, step 3), docs |

## Problem

`flint_json` always generates `ClassName(field: …)` with **every** parsed field as a named argument. It
ignores the constructor. With this class:

```dart
@JsonSerializable()
class Point {
  static const origin = 0;
  final int x;
  final int y;
  final int z;
  final List<String> tags = const [];
  late String label;
  final int _secret;
  final int a, b;
  int get sum => x + y;

  Point(this.x, this.y, [this.z = 0, int secret = 0, this.a = 1, this.b = 2]) : _secret = secret;
  …
}
```

the current engine generates (none of it compiles):

```dart
Point _$PointFromJson(Map<String, dynamic> json) => Point(
      x: (json['x'] as num).toInt(),           // positional parameter passed by name
      y: (json['y'] as num).toInt(),
      z: (json['z'] as num).toInt(),           // throws when the key is missing, despite `= 0`
      tags: (json['tags'] as List<dynamic>)…,  // not a parameter: the field has an initialiser
      label: json['label'] as String,          // not a parameter: a `late` field
      _secret: (json['_secret'] as num).toInt(), // named arguments can't be private
      a: (json['a'] as num).toInt(),           // …and `b` is missing: only the first variable is parsed
    );
```

`toJson` also writes `'_secret'` and `'tags'`, and never writes `b`.

### What json_serializable does

These are the outputs of json_serializable (resolved from pub on 2026-09-27, `dart run build_runner build`) for
the same shapes, written as separate classes. They are the reference for this spec; the Dart golden fixtures
will check the same JSON.

| Class shape | json_serializable output |
| ----------- | ------------------------ |
| `Point(this.x, this.y, [this.z = 0]) : doubled = x * 2;` with `final tags = const []`, `late String label`, `String? note`, `int count = 0`, `int get sum`, `static const origin` | `Point((json['x'] as num).toInt(), (json['y'] as num).toInt(), (json['z'] as num?)?.toInt() ?? 0)..label = …..note = …..count = …`. `toJson` writes `x, y, z, label, note, count`: **not** `tags`, `doubled` or `sum` |
| `Opts(this.a, {required this.b, this.c, this.d = 7})` | `Opts(a, b: …, c: json['c'] as String?, d: (json['d'] as num?)?.toInt() ?? 7)` |
| `Secret({required this.visible, int secret = 0}) : _secret = secret;` plus `int get secret => _secret;` | `Secret(visible: …, secret: … ?? 0)`; `toJson` writes `visible` and `secret` (the getter), never `_secret` |
| `final int a, b; Multi(this.a, this.b);` | both `a` and `b` |
| `@JsonSerializable(constructor: 'create')` with `factory Made.create({required int x})` | `Made.create(x: …)` |
| `Child(super.id, this.name)` where `id` is declared in `class Base` | `Child(id, name)`, and `toJson` writes `id` first |
| `Kid(int id, this.name) : super(id)`, `Base` also has `String? tag` | `Kid(id, name)..tag = …`; `toJson` writes `id, tag, name` (superclass first) |
| `Plain(int x, {int y = 3}) : x = x, y = y;` | `Plain(x, y: … ?? 3)` (plain parameters match fields by name) |
| `@JsonKey(includeFromJson: true, includeToJson: true) final int _hidden;` with `PrivKey(this._hidden)` | `PrivKey((json['_hidden'] as num).toInt())`, key `'_hidden'` |
| `late final String y;` not in the constructor | set with a cascade, `..y = …` |
| `@JsonKey(includeToJson: true) final int derived;` set in the initialiser list | not in `fromJson`; written by `toJson` |
| `@JsonKey(includeToJson: true) int get twice` | written by `toJson` |
| `@JsonKey(name: 'the_x', defaultValue: 5)` on a positional `this.x` | `(json['the_x'] as num?)?.toInt() ?? 5`: the annotation's default wins over the constructor's |
| `@JsonSerializable(createFactory: false)` with `final tags = const []`, `int get sum`, `final int _hidden = 1` | `toJson` writes `x, tags, sum`: without `fromJson`, unsettable fields and getters **are** written; private ones aren't |
| `Bad({required this.x, required int extra})` | **error:** `Cannot populate the required constructor argument: extra.` |
| `@JsonKey(includeFromJson: false)` on a field set by a required parameter | **error:** `Cannot populate the required constructor argument: x. It is assigned to a field not meant to be used in fromJson.` |
| Only `OnlyNamed.make(this.x)`, no unnamed constructor | **error:** `The class 'OnlyNamed' has no default constructor.` |

## Goals / non-goals

- **Goal:** `fromJson` calls the class's real constructor, with positional and named arguments in the right
  places, and sets the remaining writable fields with cascades.
- **Goal:** the set of JSON keys matches json_serializable (table above): private members, unsettable finals,
  getters and statics are handled the same way.
- **Goal:** a class Flint can't build gets an error that says which parameter or constructor is the problem,
  never Dart that doesn't compile.
- **Goal:** fields declared in a superclass in the same package (step 3).
- **Non-goal:** superclasses from other packages, generic superclasses (`extends Base<T>`) and fields from
  mixins. They get an error naming the superclass or mixin.
- **Non-goal:** `@JsonKey(readValue:)`, `required`, `disallowNullValue` and `$checkedCreate` (`checked: true`).
- **Non-goal:** matching json_serializable's text exactly. The defaulting expression stays in Flint's existing
  form, `json['k'] == null ? <default> : <conversion>`, which behaves the same as `(… as T?) ?? <default>`.

## Behaviour

### Which constructor

1. `@JsonSerializable(constructor: 'name')` picks `ClassName.name`; otherwise the unnamed constructor.
   Generative, `const`, factory and redirecting factory (`factory X(int a) = _X;`) constructors all count.
2. None found: **error**, “class `OnlyNamed` has no unnamed constructor; add one, or pick one with
   `@JsonSerializable(constructor: 'make')`” (listing the named constructors it does have).
3. Only needed when `fromJson` is generated (`createFactory` isn't false).

### Members (JSON properties)

A class's **members** are, in this order: the superclass's members (step 3), then the class's own instance
fields (every variable of `final int a, b;`) and getters, in declaration order.

- `static` fields and getters are never members.
- **Private** members (`_x`) are skipped unless their `@JsonKey` sets `includeFromJson: true` or
  `includeToJson: true`; their key is then the name as written (`'_hidden'`), subject to `field_rename`.
- A getter whose name is also a field is ignored (the field wins).

### `fromJson`

For each parameter of the chosen constructor, in order:

| Parameter | Matched member | Argument |
| --------- | -------------- | -------- |
| `this.x`, `super.x`, or plain `T x` | the member named `x` (for `super.x` and plain parameters, a field or getter; see step 3 for inherited ones) | the member's conversion |
| … with a default (`[this.z = 0]`, `{this.d = 7}`) | same | `json['z'] == null ? 0 : <conversion>`; a `@JsonKey(defaultValue:)` on the member wins |
| required, no member, or the member has `ignore`/`includeFromJson: false` | — | **error** (below) |
| optional, no member, or the member is excluded | — | omitted |

Positional parameters are passed positionally, named ones by name. A converter, `@JsonKey(fromJson:)` hooks and
`defaultValue` apply to the argument as they do today.

After the constructor call, every member that **no parameter set** and that is **writable** (a non-`final`
field, or a `late final` field without an initialiser) is set with a cascade, `..label = <conversion>`, unless
excluded from `fromJson`.

Members that no parameter sets and that aren't writable (`final` fields with an initialiser or set in the
initialiser list, getters) are **not members for this class at all** when `fromJson` is generated, so `toJson`
skips them too, unless their `@JsonKey` sets `includeToJson: true`. This matches json_serializable; see open
question 2.

### `toJson`

Writes every member left after the rules above, in member order, except those with `ignore` or
`includeToJson: false`. With `createFactory: false` nothing is dropped for being unsettable, so final fields
with initialisers and public getters are written (json_serializable does the same).

### Diagnostics

Errors are per source file (spec 0001): the output is left unchanged and the build exits non-zero.

```text
❌ lib/bad.dart: line 4: constructor 'Bad' has a required parameter 'extra' that doesn't match a field or getter, so fromJson can't fill it. Give it a default, make it optional, or add a field named 'extra'.
❌ lib/ign.dart: line 5: constructor 'Ign' has a required parameter 'x', but field 'x' is excluded from fromJson (includeFromJson: false). Make the parameter optional, or include the field.
❌ lib/only.dart: line 2: class 'OnlyNamed' has no unnamed constructor. Add one, or pick one with @JsonSerializable(constructor: 'make').
❌ lib/kid.dart: line 9: constructor 'Kid' sets 'id' through its superclass 'Base', which Flint can't read (declared in another package). Use a @JsonKey(fromJson:, toJson:) on a field of Kid, or a custom fromJson.
```

### Template context (additive)

Custom templates keep `class.fields`, which now lists every variable of a multi-variable declaration, and gets:

- `field.is_late`, `field.has_initializer`, `field.is_private`;
- `class.getters`: `[{ name, dart_type, metadata, line }]` (instance getters only);
- `class.constructors`: `[{ name (null for the unnamed one), is_factory, is_const, line, params: [{ name,
  kind: "positional" | "optional_positional" | "named", required, default (Dart source or null),
  initializes: "this" | "super" | "plain", dart_type (plain parameters only) }] }]`.

Static fields and getters are left out.

The `flint_json` template gets, per class, `class.from_json` (the constructor to call and its arguments in
order, each with the member it came from) and `class.json_members` (the members `toJson` writes). A custom
`template_path` for `flint_json` has to use these instead of iterating `class.fields`; this is called out in
`configuration.md` and the changelog.

## Design

1. **Parser.** `extract_fields_from_tree` returns one `DartField` per variable, with `is_late`,
   `has_initializer` and `is_static` read from the declaration's modifiers (static members are dropped, as
   today). New: `DartClass.getters` and `DartClass.constructors`, read from `method_signature` /
   `getter_signature` and `constructor_signature` / `constant_constructor_signature` /
   `factory_constructor_signature` / `redirecting_factory_constructor_signature` nodes, with tree-sitter field names rather than positional
   captures (AGENTS.md sharp edge, R3). Parameters keep their default value as Dart source text, like `@JsonValue`
   literals (R6).
2. **Emitter.** A new `flint_json::members` module turns a `DartClass` (and, in step 3, its resolved
   superclass chain) into a `JsonPlan { constructor, args: [Arg { positional | named(name), member }],
   cascades: [member], to_json: [member] }` or a `FlintError::Constructor { line, message }`. Expressions are
   still built per member by the existing `generate_from_json_expression` / `generate_to_json_expression`; the
   default wrapper is shared with `@JsonKey(defaultValue:)`. The template renders the plan.
3. **Superclasses (step 3).** The index keeps each class's members, constructor-free (`DartClass` fields and
   getters), and the class's `extends` clause. The builder resolves the superclass chain through
   `SymbolIndex::resolve` (spec 0005), from the file that declares each class, and passes it in
   `ResolvedTypes` so the output depends on the superclass's file (spec 0005's up-to-date rule). Cycles and
   depth are bounded like `export` chains.
4. **SDD:** §4 (constructors in the index), §6.1 (parsed model), §6.2's `Constructor` target becomes Current.

## Acceptance criteria

- [ ] Every row of the “What json_serializable does” table has a Dart golden fixture (a new
      `constructors_model.dart`, plus `inheritance_model.dart` in step 3) whose round-trip test expects the
      JSON json_serializable produces; the three error rows are build tests with the messages above.
- [ ] `final int a, b;` produces two fields, in the parser test and in a snapshot.
- [ ] The `Point` model from the Problem section generates code that analyzes cleanly.
- [ ] Existing snapshots and the example's output don't change (they only use named `this.x` parameters
      without defaults).
- [ ] A custom template sees `class.constructors`, `class.getters` and the new field flags (template test).
- [ ] The 1,000-file no-op benchmark stays under 100 ms (`engine/bench/run.sh 1000 5`).

## Plan

Each step is mergeable on its own and keeps CI green.

1. ✅ **Parser:** multi-variable fields (bug fix; a snapshot only changes for classes that use them), field
   modifiers, getters, constructors and parameters, and the template context additions. No `flint_json`
   output changes except the `a, b` fix. Notes:
   - **Second bug fixed:** static fields with a type (`static int created = 0;`) were parsed as instance
     fields (only `static const x = …` was skipped, by accident of the grammar). Statics are now dropped
     explicitly, fields and getters alike, so getters have no `is_static` flag.
   - Grammar quirks, covered by the parser test: a `= default` is a *sibling* of its parameter inside
     `[…]`/`{…}`, and `required` is a sibling token for plain parameters but a `type_identifier` inside
     `required this.x`.
   - Golden and snapshot fixture `constructors_model.dart` (`Pair` with `final int a, b;`, a static and a
     getter); steps 2 and 3 extend it.
   - Benchmark (`engine/bench/run.sh 1000 5`): no-op 85–97 ms typical, the same as before this step measured
     in the same session (82–110 ms). The machine's run-to-run spread already reaches the 100 ms budget.
2. ✅ **Constructor-aware `fromJson`/`toJson`** for a class's own members: constructor choice, positional and
   named arguments, constructor defaults, cascades, member rules (private, unsettable, getters), the three
   errors. Golden `constructors_model.dart`. `super.x` parameters get the “superclass” error until step 3;
   plain parameters that match nothing get the “required parameter” error when required, and are left out
   when optional. Notes:
   - **Implemented as designed:** `flint_json::members::plan` returns the constructor, arguments (with the
     member each one takes), cascades and `toJson` members; the emitter builds each value in Rust and the
     template renders it. Every row of the json_serializable table is a unit test of the plan and, where it
     compiles, a golden round trip; the error rows are build tests. Existing snapshots and the example are
     unchanged.
   - **Deviation found while testing, better than json_serializable:** an optional positional parameter that
     no member fills, before one that is filled (`Gap(this.x, [int s = 0, this.a = 1])`), is passed its
     default (or `null`). json_serializable (checked on 2026-09-27) passes `a`'s value into `s`'s slot.
   - **A class that declares no constructor** uses Dart's implicit unnamed one (every field by cascade).
     A class whose only constructors are named (such as `fromJson`) gets the “no unnamed constructor” error.
   - **Code review** (`/code-review`, steps 1 and 2 together) found, and these are fixed and tested:
     - a constructor default that uses a static member (`this.limit = defaultLimit`) didn't compile in the
       top-level function; static member names are now parsed (`class.static_members`) and qualified
       (`Limits.defaultLimit`), skipping string literals;
     - a getter/setter pair was treated as read-only and silently dropped; setters are parsed
       (`class.setters`) and make the getter writable (a cascade);
     - a `template_path` for `flint_json` written before this spec lost `class.fields[*].from_json_expr`;
       the expressions are copied back to `class.fields`;
     - an untyped getter (`get x => 1`) in a `createFactory: false` class was an error; it's now `dynamic`;
     - an error for a class without constructors reported line 0; classes now record their `line`;
     - a required `this._x` for a private field suggested adding the field that already exists; the
       message now points at `@JsonKey(includeFromJson: true, includeToJson: true)`;
     - `includeIfNull` defaults were applied in two places; `apply_plugin_defaults` now covers getters.
     Not changed: a `@JsonKey(fromJson:)` hook still gets the raw value, null included, rather than the
     constructor's default, the same as with `defaultValue` today.
   - Benchmark (`engine/bench/run.sh 1000 5`): no-op 86–91 ms, `--force` 251–286 ms.
3. **Superclass members** from the same package through the index, with the dependency rule; errors for
   other packages, generic superclasses and mixins with fields. Golden `inheritance_model.dart`.
4. **Docs:** support matrix, template context, SDD §4/§6, roadmap, review; spec Done.

## Decisions

The open questions were accepted with the proposed answers:

1. **Private members** are skipped unless their `@JsonKey` includes them, as json_serializable does.
2. **Unsettable `final` fields and getters** are left out of `toJson` when `fromJson` is generated;
   `@JsonKey(includeToJson: true)` keeps one.
3. **Getters with `createFactory: false`** are written by `toJson`; the changelog calls this out.
4. **Superclasses** are handled in this spec, as step 3.
5. **Default expression:** Flint keeps its `json['k'] == null ? d : conv` form.

## Open questions (resolved)

1. **Private members:** skip them unless `@JsonKey` includes them, as json_serializable does? Today Flint
   writes them to `toJson` (and generates a named argument that can't compile). *Proposed: yes; the only
   compiling output this changes is `toJson` of `createFactory: false` classes with private fields.*
2. **Unsettable `final` fields and getters when `fromJson` is generated:** json_serializable silently leaves them
   out of `toJson` too. Follow it, or keep writing them? *Proposed: follow json_serializable, for drop-in
   parity; `@JsonKey(includeToJson: true)` keeps one. No existing fixture is affected.*
3. **Getters with `createFactory: false`:** json_serializable writes public getters. *Proposed: follow it.
   This adds keys to the `toJson` of such classes, which is a visible change for anyone relying on today's
   output; it's called out in the changelog.*
4. **Superclasses in this spec** (step 3) or a separate one? *Proposed: here, because `super.x` parameters are
   common in real models and without step 3 they only get an error.*
5. **Default expression form:** keep `json['k'] == null ? d : conv` rather than json_serializable's
   `(json['k'] as T?) ?? d`? *Proposed: keep Flint's form; it's already used for `defaultValue`, and the two
   behave the same.*
