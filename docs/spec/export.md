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

- **Content**: the full model — every altitude the app has, including code
  level (L4, from committed facts) and deployment (LD), each segment live only
  when the exported snapshot carries the facts for it — both themes
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
- **Tested as the artifact it is** (0.7.0): `ui/tests/export/` opens the built
  `architecture.html` from `file://` in WebKit and walks it. Until then nothing
  did — the rest of the suite runs the *app* against the mock bridge — and the
  viewer had silently lacked L4 since introspection shipped.

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

**Headless SVG/PNG** (shipped 2026-08-23, 0.2.0 theme 2 — this lifted the
v1 boundary): `node tools/render-views.mjs <snapshot.json> -o <dir>
[--theme dark] [--png] [--scale N]` renders the L1 context plus every
defined view. It reuses the app's exact pipeline — ui/js/layout.js (same
elkjs, same determinism) and the SVG assembly extracted into ui/js/svg.js,
which the in-app Share menu now also consumes — so headless output is
pixel-identical in structure to what the canvas shows. Design tokens are
resolved from ui/ds/tokens/colors.css by a small evaluator (var() chains
plus the one color-mix() pattern the palette uses); fonts embed as data
URIs. PNG rasterizes the SVG via Playwright WebKit (dev dependency; the
SVG path needs no browser). Output is byte-identical across runs
(ui/tests/render.test.mjs) — the PR diff bot depends on that. CI's
frontend job publishes both themes as the `architecture-renders` artifact
on every push, and the `model-diff` workflow comments rendered
before/after views + the semantic diff on any PR touching the model.
