# 0004 — Template errors are reported, not panics

| | |
| --- | --- |
| **Status** | Done |
| **Resolves** | R12 |
| **Touches** | `generators/` (`Generator` trait, `TemplateEngine`, both generators), `error.rs`, `builder.rs` |

## Problem

`TemplateEngine` calls `unwrap`/`expect` on every Tera operation, and `Generator::generate` returns a plain
`String`, so a template problem can't be reported. Reproduced with a custom plugin next to `flint_json`:

| Template problem | Today |
| ---------------- | ----- |
| `template_path` doesn't exist | panic, exit 101, Rust debug dump of the error |
| Syntax error (`{{ c.name }`) | panic, exit 101 |
| Render error (`{{ nope }}`, an unknown variable) | panic, exit 101 |

In every case the panic aborts the whole build, so healthy plugins (`flint_json` here) write nothing either.
These are the only `unwrap`/`expect` calls left in non-test engine code.

## Behaviour

- **Loading problems** (missing file, syntax error) are found **once, before any file is processed**. They
  produce one error per plugin that names the plugin, the template path and Tera's explanation, including the
  line and column for syntax errors.
- **Files a broken plugin matches are left untouched**: not written, not stripped of that plugin's section,
  not deleted. Treating the plugin as absent instead would delete or strip its outputs as soon as they're
  reprocessed, so a typo in a template during watch mode would break the project (found in code review).
  Files the broken plugin doesn't match are built normally. The summary says how many files were left
  unchanged.
- **Render problems** are reported per source file, like other per-file errors: the file is left unchanged,
  the other files are built.
- The build then exits non-zero, as for any error (spec 0001), with no panic and no backtrace.

Real output with `flint_json` plus a `describe` plugin whose template has a syntax error (`user.dart` is
only matched by `flint_json`, `only.dart` only by `describe`):

```text
  ✅ Generated: ./lib/user.g.dart
  ❌ Plugin 'describe': Failed to parse "./describe.tera": --> 1:33
  |
1 | {% for c in classes %}{{ c.name }
  |                                 ^---
  |
  = expected `or`, `and`, `not`, `<=`, `>=`, `<`, `>`, `==`, `!=`, `+`, `-`, `*`, `/`, `%`, a filter, or a variable end (`}}`)
✅ 1 generated, 0 unchanged, 0 up to date
⚠️ 1 file(s) left unchanged because a plugin's template has errors.
Error: Build finished with 1 error(s)
```

A render error looks like this:

```text
  ❌ ./lib/user.dart: Plugin 'custom': Failed to render 'custom': Variable `nope` not found in context while rendering 'custom'
```

## Design

- `Generator::generate` returns `Result<String, FlintError>`. The generators never panic.
- `TemplateEngine::load_template`, `load_template_file` and `render` return `Result<_, tera::Error>`.
- New `FlintError::Template { plugin, message }`. The message joins Tera's error with its `source()` chain,
  because Tera's own `Display` only shows the outermost line (“Failed to parse …”) and hides the reason.
- `builder::active_plugins` checks each plugin's `template_path` by loading it once (`generators::check_template`).
  A failure goes into `BuildReport.errors` and the plugin is kept but marked `broken`. `process_source`
  returns `Outcome::Blocked` for any file a broken plugin matches, before generating or deleting anything
  (`BuildReport.blocked` counts them).
- Out of scope: compiling each template once and reusing it (A1). Templates are still loaded per file.

## Acceptance criteria

- [x] A missing `template_path` gives one error naming the plugin and path; `flint_json` output is still written
      for files the broken plugin doesn't match.
- [x] With a broken template, existing outputs of files it matches are neither deleted nor stripped, even when
      they're reprocessed.
- [x] A template syntax error gives one error with Tera's line/column explanation.
- [x] A render error is reported for that source file, whose output isn't written.
- [x] The CLI exits non-zero with no panic in all three cases.
- [x] No `unwrap`/`expect` remains in non-test engine code.
