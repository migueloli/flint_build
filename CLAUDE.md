@AGENTS.md

## Claude Code notes

- Start any non-trivial task by checking `docs/ROADMAP.md` and `docs/REVIEW.md` for the matching finding ID,
  and `docs/specs/` for an existing spec.
- Cloud sessions may not have the Dart SDK. It can be installed into the scratchpad from
  `https://storage.googleapis.com/dart-archive/channels/stable/release/<version>/sdk/dartsdk-linux-x64-release.zip`
  (the latest version is in `.../channels/stable/release/latest/VERSION`), then put on `PATH` for
  `engine/tests/dart_golden/check.sh`. Without it, the engine can still be checked end to end by running
  `engine/target/release/flint_build build --force` inside `cli/example` and diffing the output.
- Before finishing a change to `engine/`, run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.
  Use `/code-review` on the diff for anything that changes generated output.
