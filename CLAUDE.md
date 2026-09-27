@AGENTS.md

## Claude Code notes

- Start with `docs/HANDOFF.md`: the current state, the next task and open decisions. Then check
  `docs/ROADMAP.md` and `docs/REVIEW.md` for the matching finding ID, and `docs/specs/` for an existing spec.
- When you finish a spec step or leave work half done, update `docs/HANDOFF.md` in the same commit.
- Cloud sessions may not have the Dart SDK. It can be installed into the scratchpad from
  `https://storage.googleapis.com/dart-archive/channels/stable/release/<version>/sdk/dartsdk-linux-x64-release.zip`
  (the latest version is in `.../channels/stable/release/latest/VERSION`), then put on `PATH` for
  `engine/tests/dart_golden/check.sh`. Without it, the engine can still be checked end to end by running
  `engine/target/release/flint_build build --force` inside `cli/example` and diffing the output.
- Before finishing a change to `engine/`, run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  and `cargo llvm-cov --summary-only --fail-under-lines 90` (AGENTS.md rule 10: new functionality comes with its
  tests). `cargo-llvm-cov` isn't preinstalled in cloud sessions: `rustup component add llvm-tools-preview &&
  cargo install cargo-llvm-cov --locked`. Use `/code-review` on the diff for anything that changes generated output.
- The docs are the project's memory: there is no other persistent memory between sessions. Decisions go into
  the spec's *Decisions* section and `docs/HANDOFF.md` §5, not only into the conversation.
