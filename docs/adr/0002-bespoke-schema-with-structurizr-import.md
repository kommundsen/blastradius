---
doc: adr-0002
type: adr
status: accepted
elements: [blastradius.core.model-service, blastradius.core.importer]
---

# ADR-0002: Bespoke YAML schema, with a one-way Structurizr importer

## Status
Accepted — 2026-08-22

## Context
Structurizr DSL is the incumbent text format for C4 models. Adopting it
wholesale would give instant compatibility — and would also inherit its
single-workspace-file assumption, its identity scheme, and view semantics
designed for server-side rendering. Our schema is judged first by what a PR
diff looks like (PRD principle 2) and must carry first-class documents
(ADR-0010); neither survives contact with DSL compatibility.

## Decision
Design a bespoke YAML schema (normative spec: `spec/model-format.md`)
optimised for hand-editing, git diffs, and stable identity. Ship a **one-way
importer** from Structurizr DSL that produces a Blastradius workspace plus a
**fidelity report**: every construct that did not map is listed in the output,
never silently dropped.

## Consequences
- New users who have Structurizr workspaces get a migration path; we get
  schema freedom.
- No export back to Structurizr; the importer is an on-ramp, not a bridge.
  Round-tripping is explicitly a non-goal.
- The importer is a real v1 deliverable with its own test corpus of public
  Structurizr workspaces (PRD metric: ≥ 80% import cleanly).
