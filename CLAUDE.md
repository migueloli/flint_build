@AGENTS.md

## Claude Code notes

- Start any non-trivial task by checking `docs/ROADMAP.md` and `docs/REVIEW.md` for the matching finding ID,
  and `docs/specs/` for an existing spec.
- Cloud sessions may not have the Dart SDK. The engine can still be checked end to end by running
  `engine/target/release/flint_build build -d` inside `cli/example` (or a scratch package with a
  `pubspec.yaml` and `flint.yaml`) and diffing the output.
- Before finishing a change to `engine/`, run `cargo fmt`, `cargo clippy --all-targets`, and `cargo test`.
  Use `/code-review` on the diff for anything that changes generated output.
