---
doc: adr-0010
type: adr
status: accepted
elements: [blastradius.core.model-service, blastradius.ui.docs-panel]
---

# ADR-0010: Documents are first-class model objects

## Status
Accepted — 2026-08-22

## Context
The dogfood requirement: this `docs/` folder must open in Blastradius as a
working workspace. If the schema cannot hold the PRD, ADRs, and specs, they
live beside the model as unlinked prose — two sources of truth, which is the
exact rot this product exists to kill.

## Decision
The schema includes a document type. Markdown files matched by the manifest's
docs globs and carrying frontmatter (`doc:` id, `type:`, `status:`,
`elements:` links) are loaded into the model as typed document objects.

- Links are validated both ways: a doc naming a missing element id is a model
  error, exactly like a dangling relation.
- The UI surfaces links bidirectionally: an element's inspector lists its
  governing documents; a document view highlights its elements on the canvas.
- Document *bodies* remain markdown files owned by the user's editor. The app
  renders them; v1 does not edit them. Blastradius is not becoming a markdown
  editor.

Doc types in v1: `prd`, `adr`, `spec`, `roadmap`, `note` — with per-type
status vocabularies (spec/model-format.md).

## Consequences
- "Which decisions govern this container?" is a query the app answers — the
  feature that makes architecture docs *navigable* rather than merely present.
- Frontmatter is now schema surface and is versioned with the rest
  (spec/model-format.md §7).
- The dogfood gate (PRD) is enforceable: CI validates this folder with the
  real parser from the first phase of the roadmap.
