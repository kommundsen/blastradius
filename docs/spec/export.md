---
doc: spec-export
type: spec
status: draft
elements: [blastradius.core.exporter, blastradius.ui.canvas]
---

# Spec: Share — exports

Implements ADR-0009. Share is a split-button: primary action = interactive
HTML; menu = PNG, SVG.

## Self-contained HTML

One file, zero network requests, works from `file://`:

- **Content**: the full model (all levels L1→L3, all views), both themes
  (following the viewer's OS, with a manual toggle), zoom/pan navigation with
  the same camera motion tokens, element inspector with descriptions and doc
  *summaries* (title, type, status — bodies are included only when the
  "include document bodies" export option is checked, since docs may be more
  sensitive than structure).
- **Mechanism**: a sealed snapshot — `{ model: <json>, views: <json> }`
  embedded beside a standalone renderer bundle built from the same UI
  components (the design-system classes and the canvas grammar), plus the
  vendored woff2 fonts inlined as data URIs. Read-only: no sync engine, no
  git, no editing affordances.
- **Budget**: ≤ 2.5MB for a 200-element workspace (fonts ≈ 200KB of that).
- The identical snapshot format is the v2 hosted-link upload payload; the
  renderer bundle being Tauri-independent is a build-time constraint on the UI
  container (enforced by building the export bundle in CI).
- Free tier: "made with Blastradius" footer, per PRD pricing hypothesis.

## PNG / SVG

Current view, current theme:

- **SVG**: serialize the live canvas — nodes converted to `<foreignObject>`-free
  pure SVG (text, rects) so the file opens in design tools; fonts embedded.
- **PNG**: rasterized from that SVG at 1× / 2× / 4× selectable scale, on a
  solid ground (no transparency surprises in slides).
- Both honour the diff toggle — exporting a diff view is an explicit, expected
  use ("what changed this quarter" as a slide).

## CI / headless

`blastradius export <dir> -o architecture.html` runs from the Rust core
without a WebView — headless **by construction**: the export embeds elkjs and
lays out at open time, so build-time layout is never needed. CI publishes the
artifact on every merge (the Phase 4 exit criterion).

**v1 boundary**: headless SVG/PNG is *not* shipped — deterministic layout
lives in elkjs (ADR-0006), so a headless raster would need a JS runtime in the
export path. SVG/PNG are in-app exports serialized from the live layout, with
fonts embedded via data URIs. If CI ever needs raster output, the route is a
node script reusing ui/js/layout.js (same engine, same determinism) — recorded
as a v2 theme, not planned for v1.
