---
doc: adr-0009
type: adr
status: accepted
elements: [blastradius.core.exporter]
---

# ADR-0009: Share = self-contained HTML export (+ PNG/SVG); hosted links are v2

## Status
Accepted — 2026-08-22

## Context
"Share" is the primary button of the entire UI and must mean something in a
local-first app with no server. A hosted link service is the obvious v2
revenue surface, but it drags auth, storage, and a privacy story into v1.

## Decision
v1 Share produces:

1. **A single self-contained interactive HTML file** — the full model,
   zoomable L1→L3, both themes, vendored fonts inlined, zero network
   requests. Openable from disk, attachable to a PR, publishable on any
   static host or CI artifact store.
2. **PNG / SVG of the current view** for slides and documents.

The HTML export is deliberately designed as a **sealed model snapshot**
(embedded model JSON + renderer): the identical artifact becomes the upload
payload when the hosted share service ships in v2. Free-tier exports carry a
"made with Blastradius" footer (PRD pricing hypothesis).

## Consequences
- Share works offline, on day one, with no account — consistent with
  local-first principle 1.
- The renderer must be buildable as a standalone bundle independent of Tauri —
  a real architectural constraint on the UI container (spec/export.md).
- v2's service becomes storage + access control around an artifact that
  already exists, not a new rendering stack.
