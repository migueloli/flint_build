# 0001 — Generated output ownership

| | |
| --- | --- |
| **Status** | Draft |
| **Resolves** | R1, R2, R4, R5 (and part of A1, A3) |
| **Touches** | `builder.rs`, `discovery/`, `watcher/`, `generators/mod.rs`, `templates/flint_json.tera`, `config/flint.rs` |

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
   // flint_build 0.2.0
   ```

   The second line is the **ownership marker**. Built-in and custom templates don't write it; the engine
   adds it. A custom template that also emits the first line is fine, because the engine removes a duplicate.

2. **Write rule.** For source `lib/a/b.dart`, Flint writes `lib/a/b.g.dart` only if:
   - `b.dart` contains `part 'b.g.dart';` (single or double quotes), **and**
   - at least one plugin matched at least one declaration in it.

   If a plugin matches declarations but the `part` directive is missing, Flint reports a warning with the
   exact line to add, and doesn't write the file.

3. **Refuse to overwrite.** If `b.g.dart` exists **without** the ownership marker, Flint reports an error for
   that file and leaves it alone. This protects files produced by build_runner. `--force` overrides it.

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
   longer has the `part` directive) is deleted.

7. **Watch.** Events for files that carry the ownership marker (in practice, any `*.g.dart` path Flint just
   wrote) are ignored.

## Design

- Use `IndexMap` for `plugins` so config order is kept (update SDD §8).
- Restructure `run_build`:
  1. Discover once.
  2. For each file in parallel, parse once, and **filter by annotation in the parser step**, using the union of
     all plugins' annotations (A3).
  3. Run each matching plugin, which returns a section.
  4. Assemble the sections and apply the write rule.
- Move the templates' shared header and `part of` into the engine. Templates render only their section body.
  Existing custom templates that still emit a header or `part of` are detected by prefix and de-duplicated
  (one release of backwards compatibility, then a warning).
- Ownership check: read the first 256 bytes and look for the line `// flint_build `.
- The watcher keeps a `HashSet<PathBuf>` of paths written in the current build and drops matching events. As a
  simpler first step, drop any event whose path ends in `.g.dart`.

## Acceptance criteria

- [ ] Given `lib/other.g.dart` without the marker, `clean` leaves it in place.
- [ ] Given a file with only un-annotated classes, `build` writes nothing for it.
- [ ] Given an annotated class and no `part` directive, `build` writes nothing and prints a warning that names
      the missing directive.
- [ ] Given `x.g.dart` without the marker and an annotated `x.dart`, `build` reports an error and leaves
      `x.g.dart` byte-identical. `build --force` overwrites it.
- [ ] Given `flint_json` and a custom plugin both matching `m.dart`, `m.g.dart` contains both sections in
      config order, and running the build 10 times produces byte-identical output.
- [ ] Given an owned `y.g.dart` whose source had its annotation removed, `build` deletes `y.g.dart`.
- [ ] Given `watch --force`, one `touch` of a source triggers exactly one rebuild.
- [ ] The existing snapshot tests pass after updating for the header change.

## Plan

1. Add `tempfile`-based end-to-end test helpers that run `run_build` in a temporary package (also fixes H6).
2. Ownership marker + clean rule (fixes R1, the most urgent).
3. Watcher ignores generated paths (fixes R5).
4. Parse once, filter by annotation, write rule with the `part` directive check (fixes R2 and A3).
5. `IndexMap` + section assembly (fixes R4).
6. Stale-output deletion, docs update, move spec to `Done`.

## Open questions

- Should a missing `part` directive be a warning (proposed) or an error? An error is stricter, but it breaks
  `build` for files that are in the middle of an edit while in watch mode.
- Should `--delete-conflicting-outputs` keep meaning “force” as an alias, or match build_runner's meaning
  (overwrite unowned `.g.dart` files)? This spec proposes the latter, together with R13.
