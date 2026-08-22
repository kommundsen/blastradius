---
doc: adr-0001
type: adr
status: accepted
elements: [blastradius]
---

# ADR-0001: Record architecture decisions, and make them model objects

## Status
Accepted — 2026-08-22

## Context
Blastradius exists because architecture knowledge rots when it lives outside
the repo and outside review. Its own decisions must not be exempt.

## Decision
Decisions are recorded as ADRs in `docs/adr/`, one file per decision,
numbered, immutable once accepted (superseded, never edited into a different
decision). Each ADR carries frontmatter registering it in the docs model with
typed links to the elements it governs (see ADR-0010). The format is
lightweight MADR: Status, Context, Decision, Consequences.

## Consequences
- A decision that names no element is a smell — either an element is missing
  from the model or the decision is not architectural.
- The app will render ADR ↔ element links; broken links fail validation, so
  deleting an element without addressing its ADRs is a visible error.
