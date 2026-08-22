---
doc: adr-0007
type: adr
status: accepted
elements: [blastradius.core.git-service]
---

# ADR-0007: Embedded libgit2; git optional; semantic diff over text diff

## Status
Accepted — 2026-08-22

## Context
Git awareness is a core differentiator (PRD): branch state in the chrome,
per-element diff on the canvas, conflict flagging. Options were shelling out
to the user's `git` (fidelity, but a runtime dependency and process overhead
per query) or embedding libgit2 via the git2 Rust crate (no dependency, fast
in-process reads, bounded write surface).

## Decision
Embed **git2** in the Rust core. v1 uses it read-only: status, branch,
ahead/behind, reading blobs at arbitrary revisions for diffing, conflict
detection. The app performs no commits, merges, or pushes in v1 — the user's
own git tooling owns writes.

**Git is optional.** A workspace outside any repository works fully; git
surfaces (status chip, diff mode, conflict states) are simply absent, not
degraded into errors.

**Diff is semantic, not textual.** The git service loads the model at the base
revision and at working tree, and the model service diffs the two *element
graphs*: added / removed / changed elements and relations, exposed to the
canvas as the `is-added / is-removed / is-changed` states the design system
already defines. Layout-only changes (views files) are excluded from the
default diff and available behind a toggle — a moved box is not an
architecture change.

## Consequences
- No dependency on a system git binary; works on a clean machine.
- Diffing two revisions means parsing the model twice; model loading must be
  fast and side-effect-free (constrains model-service design).
- Conflict handling in v1 is detect-and-display: conflicted files render the
  design system's `is-conflict` state on affected elements, with both
  versions viewable read-only. Resolution happens in the user's merge tool —
  in-app resolution is a named v2 candidate, not a silent gap.
