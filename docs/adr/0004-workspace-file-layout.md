---
doc: adr-0004
type: adr
status: accepted
elements: [blastradius.core.model-service]
---

# ADR-0004: Multi-file workspace; model, views, and docs never share a file

## Status
Accepted — 2026-08-22

## Context
A single `model.yaml` does not survive a real system: hundreds of elements in
one file guarantee merge conflicts between unrelated teams, and mixing layout
(`api: [5, 2]`) with semantics (`api: { tech: Go }`) means every dragged node
dirties the semantic file — PR reviewers cannot tell "added a database" from
"nudged a box".

## Decision
A workspace is a folder with a `workspace.yaml` manifest declaring include
globs for three separate concerns:

- `model/` — semantic files. One file per software system (people and external
  systems in `context.yaml`). Splitting further (per-container files) is
  allowed by glob but not default.
- `views/` — view definitions and pinned layout. Layout coordinates never
  appear in a model file.
- docs — markdown discovered by glob, registered via frontmatter (ADR-0010).

A semantic edit and a layout edit therefore always land in different files,
and typically different PR hunks.

**L4 in v1:** the level exists in navigation (the zoom has a floor below L3
that renders "not modelled"), but code-level elements are out of scope.
Hand-authored code models rot in days; source-derived ones need per-language
tooling. Deferred, not rejected.

## Consequences
- Reviewers can approve semantic changes and skim layout changes separately.
- Cross-file references (relations to another system's containers) use dotted
  ids; the model service resolves them at load and reports dangling ids as
  validation errors.
- The manifest is the single entry point: "open workspace" means "open the
  folder containing `workspace.yaml`".
