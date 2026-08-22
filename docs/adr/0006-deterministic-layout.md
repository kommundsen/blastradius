---
doc: adr-0006
type: adr
status: accepted
elements: [blastradius.ui.layout-worker]
---

# ADR-0006: Deterministic ELK layout with per-node pinning

## Status
Accepted — 2026-08-22

## Context
"Auto unless pinned" only works if auto-layout is **deterministic**: the same
model must produce the identical diagram on every machine, every run —
otherwise every checkout renders differently and pinned coordinates are
meaningless. Layout must also be *stable enough* that adding one element does
not reshuffle the whole diagram (which would make visual diffs useless).

## Decision
elkjs (ELK layered algorithm) running in a web worker in the WebView. Inputs
are ordered canonically (model file order, then id) and the algorithm is
seeded with fixed options, giving run-to-run determinism. Pinned nodes come
from the views file as fixed-position constraints; unpinned nodes flow around
them.

Stability strategy: on model change, the previous computed positions are fed
back as soft "interactive" hints, so unpinned nodes prefer staying near where
they were. Pinning remains the guaranteed escape hatch — "pin what you care
about" is a documented workflow, not a workaround.

## Consequences
- Layout runs in the WebView, not the Rust core — elkjs is the only mature ELK
  binding, and layout is a rendering concern. The core stays layout-ignorant;
  exports reuse the same worker.
- Grid units in views files are logical (canvas grid cells, 26px at 1×), not
  raw pixels, so pinned layouts survive density/zoom changes.
- Layered layout fits the hierarchical C4 style; force-directed and manual
  free-form modes are explicitly out of v1.
