#!/usr/bin/env bash
# Engine-only benchmark on a synthetic project (spec 0005). Times come from the engine's own "Done in"
# timer, so Dart VM startup isn't included. Record the machine (cores) with any numbers you publish.
#
#   engine/bench/run.sh [files] [runs]      defaults: 1000 files, 3 runs of each kind
#
# Each file has one @JsonSerializable model (7 fields) and one @JsonEnum enum. Reported runs:
#   force   `build --force`: parse and generate every file
#   noop    `build` with everything up to date
#   parse   `build --force` on the same files without annotations: parsing only, which is roughly what a
#           project-wide index pass costs on every build (spec 0005)
set -euo pipefail

files="${1:-1000}"
runs="${2:-3}"
engine_dir="$(cd "$(dirname "$0")/.." && pwd)"
(cd "$engine_dir" && cargo build --release --quiet)
engine="$engine_dir/target/release/flint_build"

project="$(mktemp -d)"
trap 'rm -rf "$project"' EXIT
mkdir "$project/lib"
printf 'name: bench\n' > "$project/pubspec.yaml"
printf 'plugins:\n  flint_json:\n' > "$project/flint.yaml"
for ((i = 0; i < files; i++)); do
  cat > "$project/lib/model_$i.dart" <<DART
import 'package:json_annotation/json_annotation.dart';

part 'model_$i.g.dart';

@JsonEnum()
enum Kind$i { a, b, c }

@JsonSerializable()
class Model$i {
  final int id;
  final String name;
  final double? score;
  final List<String> tags;
  final Map<String, int> stats;
  final DateTime createdAt;
  final Kind$i kind;

  Model$i({required this.id, required this.name, this.score, required this.tags, required this.stats, required this.createdAt, required this.kind});
}
DART
done

echo "flint_build benchmark: $files files, $(nproc 2>/dev/null || sysctl -n hw.ncpu) cores"
cd "$project"
time_of() { "$engine" "$@" 2>&1 | sed -n 's/.*Done in //p'; }
for ((r = 1; r <= runs; r++)); do echo "force  run $r: $(time_of build --force)"; done
for ((r = 1; r <= runs; r++)); do echo "noop   run $r: $(time_of build)"; done
"$engine" clean > /dev/null
sed -i.bak -e '/^@JsonSerializable()$/d' -e '/^@JsonEnum()$/d' lib/*.dart && rm -f lib/*.bak
for ((r = 1; r <= runs; r++)); do echo "parse  run $r: $(time_of build --force)"; done
