#!/usr/bin/env bash
# Proves that Flint's generated Dart compiles and round-trips (REVIEW H7).
#
#   1. builds the engine (release) unless FLINT_ENGINE points at a binary
#   2. regenerates every fixture in lib/ with `flint_build build --force`
#   3. `dart analyze --fatal-infos`: the generated code must have no errors, warnings or hints
#   4. `dart test`: decoding and re-encoding must produce the expected JSON
#
# Needs the Rust toolchain and a Dart SDK (3.5+) on PATH.
set -euo pipefail

cd "$(dirname "$0")"
engine="${FLINT_ENGINE:-}"
if [[ -z "$engine" ]]; then
  (cd ../.. && cargo build --release --quiet)
  engine="../../target/release/flint_build"
fi

dart pub get
"$engine" build --force
dart analyze --fatal-infos
dart test
