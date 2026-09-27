# Dart golden check

A small Dart package whose models cover everything the [support matrix](../../../docs/configuration.md#flint_json-support-matrix)
marks as supported. `check.sh` regenerates them with the engine, then proves the output **compiles**
(`dart analyze --fatal-infos`) and **behaves correctly** (`dart test` round-trips JSON through the generated
code). CI runs it on every push.

```bash
engine/tests/dart_golden/check.sh                      # builds the engine first
FLINT_ENGINE=path/to/flint_build engine/tests/dart_golden/check.sh
```

`packages/golden_money/` is a tiny path dependency standing in for a third-party package, for classes from
other packages (`external_types` in `flint.yaml`).

The generated `*.g.dart` files are not committed; the reviewed copies of generated text are the insta
snapshots in `engine/tests/snapshots/`.

## Adding a case

When a feature starts working (for example a fix for a finding in [REVIEW.md](../../../docs/REVIEW.md)):

1. Add the model to a file in `lib/` (with its `part '<file>.g.dart';` directive and the usual
   `fromJson`/`toJson` wiring), or a new file.
2. Add expectations to `test/round_trip_test.dart`, comparing against the JSON json_serializable would
   produce.
3. Run `check.sh`.

Only add models that should pass. Known-broken cases (R3, R6, R7, R8, …) belong in the fix that makes them
pass; adding one earlier breaks CI.
