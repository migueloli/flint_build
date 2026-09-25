# Specs

Flint uses **spec-driven development** for anything bigger than a local fix. A spec is a short markdown file
that says *what* will change and *how we'll know it works*. It's written and agreed **before** the code. That
applies whether a person or an AI agent writes the code.

## When a spec is required

Write a spec when a change does any of these:

- changes generated output for existing inputs;
- changes `flint.yaml`, the CLI flags, the template context, or the `Generator` trait;
- touches more than one module (`parser` + `generators`, `cli` + `engine`, …);
- is listed on the [roadmap](../ROADMAP.md) with a spec link.

Bug fixes, refactors inside one module with no behaviour change, docs and tests don't need a spec.

## Workflow

1. **Copy** [`0000-template.md`](0000-template.md) to `NNNN-short-title.md`, using the next free number.
2. **Draft** it. Link the review IDs it resolves. Status: `Draft`.
3. **Agree** on it. Resolve the open questions. Status: `Accepted`.
4. **Implement** it, following the *Plan* section, with one commit (or PR) per step where you can. The
   acceptance criteria become tests. Status: `In progress`.
5. **Close** it. Update [SDD.md](../SDD.md), [configuration.md](../configuration.md), the roadmap and the
   READMEs. Status: `Done`.

Specs aren't edited after `Done`. A later change gets a new spec that supersedes the old one.

## Index

| # | Title | Status | Resolves |
| - | ----- | ------ | -------- |
| [0001](0001-generated-output-ownership.md) | Generated output ownership | Draft | R1, R2, R4, R5 |
| [0002](0002-read-build-yaml.md) | Read json_serializable options from `build.yaml` | Done | SDD open question 1 |
| [0003](0003-field-rename-camel.md) | `field_rename: camel` means lowerCamelCase; unknown values are errors | Done | SDD open question 2 |
