# 0001 — Generated output ownership

| | |
| --- | --- |
| **Status** | Done |
| **Resolves** | R1, R2, R4, R5, R13 (and part of A1) |
| **Touches** | `builder.rs`, new `output.rs`, `discovery/`, `watcher/`, `parser/`, `generators/`, `templates/flint_json.tera`, `config/flint.rs`, `main.rs` |

## Problem

Flint can't tell which `.g.dart` files it owns:

- `clean` deletes every `*.g.dart`, including other generators' output (R1).
- Files with no annotated declarations still get a `.g.dart` with a `part of` their library never asked for (R2).
- Two plugins targeting one source overwrite each other, and plugin order isn't deterministic (R4).
- Watch mode reacts to its own writes (R5).

Reproduction: see [REVIEW.md](../REVIEW.md), findings R1, R2, R4 and R5.

## Goals / non-goals

- **Goal:** Flint writes only where the user asked (a `part` directive) and deletes only what it wrote.
- **Goal:** several plugins can share one source file, with deterministic output.
- **Non-goal:** content-hash incremental builds (Phase 3). This spec keeps the mtime check, but it must
  consider *all* plugins together.

## Behaviour

1. **Header.** Every output starts with:

   ```dart
   // GENERATED CODE - DO NOT MODIFY BY HAND
   // flint_build 0.1.0
   ```

   The second line is the **ownership marker**. Built-in and custom templates don't write it; the engine
   adds it, together with the `part of` line. A custom template that still emits the header or `part of`
   is fine: the engine strips them from the start of its section.

   Files written by earlier Flint versions have no marker. They're recognised by the old `flint_json`
   banner, `(Powered by Flint)`, and still count as owned. Old custom-template outputs have neither, so
   they need one `build --force`.

2. **Write rule.** For source `lib/a/b.dart`, Flint writes `lib/a/b.g.dart` only if:
   - `b.dart` contains `part 'b.g.dart';` (single or double quotes), **and**
   - at least one plugin matched at least one declaration in it.

   If a plugin matches declarations but the `part` directive is missing, Flint reports a warning with the
   exact line to add, and doesn't write the file.

3. **Refuse to overwrite.** If `b.g.dart` exists **without** the ownership marker, Flint reports an error for
   that file and leaves it alone. This protects files produced by build_runner. `--force` overrides it.
   Errors are collected per file; the build still processes every other file, then exits non-zero.

   `--force` (`-f`) replaces `--delete-conflicting-outputs`, which stays as an alias (`-d` too). It means
   “regenerate everything, and overwrite unowned `.g.dart` files”, which covers both build_runner's meaning
   of the old flag and Flint's old “ignore mtimes” meaning (R13).

4. **Shared output.** When several plugins match `b.dart`, their outputs are concatenated **in the order they
   appear in `flint.yaml`**, each under a banner:

   ```dart
   // ****************************************************************************
   // flint_json
   // ****************************************************************************
   ```

   There is one `part of` line, at the top.

5. **Clean.** `clean` deletes only `*.g.dart` files that carry the ownership marker, and prints how many
   unowned files it skipped.

6. **Stale outputs.** During `build`, an owned `b.g.dart` whose source no longer matches any plugin (or no
   longer has the `part` directive) is deleted. So is an owned `b.g.dart` whose `b.dart` no longer exists.

7. **Watch.** Events for `*.g.dart` paths are ignored, and so are *access* events (a file being opened or
   read). The inotify backend reports every `open`, including Flint's own reads of source files. That was a
   second cause of the rebuild loop, not listed in R5.

8. **Unchanged bytes aren't rewritten.** If the assembled output equals the existing file, only its mtime is
   refreshed, so the next build can skip it and watchers see no content change.

9. **Shared inputs count for the up-to-date check.** An owned output is skipped only if it's newer than its
   source *and* than `flint.yaml`, `build.yaml`, `pubspec.yaml`, every template and the engine binary.
   Without this, removing a plugin from `flint.yaml` would never remove its sections (found in code review).

## Design

- `plugins` is an `IndexMap`, so config order is kept (SDD §8).
- `builder::build(root, &pubspec, force, &registry) -> BuildReport`:
  1. Discover once and sort the sources, so reports are deterministic.
  2. For each source in parallel: skip it if the output is owned and newer (unless `force`). Otherwise parse
     it once, including its `part` directives, and run every plugin whose annotations match
     (`generators::matches_plugin`). Each plugin gets a clone of the parsed file.
  3. Apply the write rule, the ownership check and stale-output deletion, then `output::assemble`.
  4. Collect per-file outcomes in source order; errors don't stop other files.
  5. Delete owned outputs whose source no longer exists.
- `output.rs` holds the header, the marker, `assemble`, `is_owned` (reads the first 1 KiB) and
  `strip_legacy_preamble`.
- The watcher uses `notify` directly, with its own 500 ms debounce loop, instead of
  `notify-debouncer-mini`, which can't filter events by kind.
- Not done here: the parser still extracts every class, not only annotated ones (A3). Filtering now happens
  once in the builder's `matches_plugin` check, and again in each generator.

## Acceptance criteria

- [x] Given `lib/other.g.dart` without the marker, `clean` leaves it in place.
- [x] Given a file with only un-annotated classes, `build` writes nothing for it.
- [x] Given an annotated class and no `part` directive, `build` writes nothing and prints a warning that names
      the missing directive.
- [x] Given `x.g.dart` without the marker and an annotated `x.dart`, `build` reports an error and leaves
      `x.g.dart` byte-identical. `build --force` overwrites it.
- [x] Given `flint_json` and a custom plugin both matching `m.dart`, `m.g.dart` contains both sections in
      config order, and running the build 10 times produces byte-identical output.
- [x] Given an owned `y.g.dart` whose source had its annotation removed, `build` deletes `y.g.dart`.
- [x] Given `watch --force`, one `touch` of a source triggers exactly one rebuild.
- [x] The existing snapshot tests pass after updating for the header change.
- [x] Removing a plugin from `flint.yaml` removes its section on the next build.
- [x] A template copied from the pre-0001 built-in template produces exactly one `part of`.

## Plan

All steps shipped together; the tests are in `engine/tests/build_test.rs`.

1. Add `tempfile`-based end-to-end test helpers that run `run_build` in a temporary package (also fixes H6).
2. Ownership marker + clean rule (fixes R1, the most urgent).
3. Watcher ignores generated paths (fixes R5).
4. Parse once, filter by annotation, write rule with the `part` directive check (fixes R2 and A3).
5. `IndexMap` + section assembly (fixes R4).
6. Stale-output deletion, docs update, move spec to `Done`.

## Decisions on the open questions

- **A missing `part` directive is a warning**, not an error. An error would break watch mode for files that
  are in the middle of an edit.
- **`--delete-conflicting-outputs` became an alias of `--force`.** See behaviour 3.
