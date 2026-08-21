# Handoff: Blastradius — canvas editor

## Overview
Blastradius is a local-first desktop app for interactive C4 architecture models: YAML-driven, git-versioned, with a zooming canvas from L1 (system context) down to L4 (code). This package contains the full design system plus a high-fidelity reference of the core screen (the canvas editor) for implementation.

## About the design files
Everything here is a **design reference built in HTML/CSS/JSX** — it shows intended look and behavior, not production code to ship. Recreate these designs in the target environment (Electron/Tauri + React is the assumed stack; adapt to whatever is chosen) using its own patterns. The CSS token files, however, ARE production-ready: import `styles.css` (or port the custom properties) verbatim.

## Fidelity
**High-fidelity.** Colors, type, spacing, radii (none — corners are square), motion and states are final. Copy exact values; do not snap to a grid or framework defaults.

## Screens / Views

### Canvas editor (`ui_kits/app/index.html`, also `templates/canvas-editor/CanvasEditor.dc.html`)
The single main view, built from the shell classes:

```
.app[data-density]          column; the window
  .app-bar                  chrome
  .app-body                 query container
    .panel.panel-nav        model tree
    .canvas                 the drawing
    .panel.panel-side       model source
```

Panels clamp between a min and a max; `--panel-*-w` is runtime state written by a `.panel-grip` drag. The canvas never yields below `--canvas-min` (360px) — below 800px the source panel floats over it, below 560px the model tree does too. Bar items carry `.bar-drop-1/-2/-3` so the bar sheds status chips and the History button as the window narrows; the breadcrumb truncates.

Density is `data-density="compact | comfortable | spacious"` on `.app`: control height 30 / 36 / 42px, rhythm only, type size unchanged.

**Top bar** — `.app-bar` (gap `--space-4`, padding `--bar-py --space-4`, 1px bottom divider):
- Brand mark 14×14 (`assets/mark.svg`)
- Breadcrumb, `--text-sm`: muted "Acme Bank / " + current model bold 500 in `--color-text` + " / Containers"
- Level switcher: Segmented control L1–L4, real radios (see `components/core/Segmented.jsx`)
- Git status: neutral Tag `⎇ main` in `--font-mono` (`.bar-drop-3`), plus semantic Tags for the working-tree diff (`.bar-drop-1`)
- After `.app-bar-spacer`: secondary Button "History" (`.bar-drop-2`), primary Button "Share" (never dropped)

**Left sidebar** — `.panel.panel-nav` (clamped 168–320px, default 200px, 1px right divider):
- Section label: `.tree-label` — `--text-2xs`, uppercase, `--tracking-label`, weight 600, muted
- Rows: `.tree-row` — real `<button>`s, padding `var(--row-py) var(--space-4)`; `.is-child` indents to 28px
- Selected: `.is-active` — `--color-accent-100` ground, `--color-accent-800` text, 2px inset right rule
- Diff: `.is-added` / `.is-removed`, prefixed `+` / `−` so the state is not colour alone
- Unicode micro-icons: ◦ (top-level), ▸ (expanded)
- `.panel-grip` on the trailing edge is the drag-to-resize target; it writes `--panel-nav-w`

**Canvas** — see "The canvas" below.

**Right panel** — `.panel.panel-side` (clamped 260–480px, default 300px, 1px left divider): YAML editor.
- `.panel-head` (on `--color-surface`): `.panel-title` "model.yaml" + "synced" accent Tag right-aligned
- `.code` block: `--code-bg`, keys `--code-key`, comments `--code-comment`, changed lines `.hl`, added lines `.add` (tinted + 2px inset rule), parse errors `.err` (wavy danger underline)
- `.panel-foot` (on `--color-surface`): a danger Tag naming the failing line

## The canvas
Nodes are DOM, edges are SVG, both on one transformed camera layer:

```
.canvas                     viewport — clips, paints the ground
  .canvas-camera            transform + --camera-scale; carries the 26px dot grid
    svg.edge-layer          relations; holds the #br-arrow marker in its own <defs>
    .node                   elements
  .canvas-overlay           chrome — zoom control, hints; never scales
```

The implementation owns exactly two things: the `transform` on `.canvas-camera`, and `--camera-scale` set to the same scale factor. Everything else reads from those.

- **Zoom rule:** children of the camera are part of the drawing and scale. `.canvas-overlay` is screen-space. `.screen-space` counter-scales the exceptions (drag handles, resize grips).
- Nodes: `.node` — square box, 1px `--node-border`, fill `--node-fill`, `--shadow-node`; kicker / title / optional meta.
- C4 type by geometry: `.is-person` (head circle), `.is-container` (3px left spine), `.is-component` (neutral-200 fill), `.is-system` (base), `.is-external` (dashed, shadowless).
- Selection: `.is-active` — `--node-fill-active`, 1.5px `--node-border-active`, raised to `--z-node-active`.
- Git/validation: `.is-added / .is-removed / .is-changed / .is-conflict / .is-invalid`, each with a `.node-badge` glyph (`+ − ~ !`) carrying an sr-only label. Never colour alone.
- Edges: `.edge` — SVG `<path>`, `marker-end` `#br-arrow`, `vector-effect: non-scaling-stroke`. `.is-secondary` (dashed), `.is-bidirectional`, `.is-undirected`, `.is-added`, `.is-removed`, `.is-active`. Pair each with a transparent `.edge-hit` path (12px stroke) for pointer targeting.
- Edge labels: SVG `<text class="edge-label">` stroked with `--canvas-bg` under `paint-order: stroke`, which knocks a clean hole in the dot grid.
- Below 0.5× scale, coarsen `--canvas-dot-pitch` to 4× rather than letting dots collapse into a wash.
- Bottom-left overlay: zoom `ButtonGroup` (− / 100% / +) + accent Tag hint.

## Interactions & behavior
- Level navigation (L1↔L4) is a continuous map-style zoom: animate the camera transform with `--transition-camera` (520ms, `--ease-camera`). Nothing crossfades, nothing jumps.
- Double-click a container node dives to L3.
- Canvas and YAML are two views of one model; edits sync live ("synced" chip).
- Hover moves one accent ramp step, press two; focus is a 2px accent `:focus-visible` ring; disabled = 45% opacity.
- Nodes are buttons: `tabIndex=0`, `role="button"`, Enter/Space activate, `aria-pressed` when selected.
- All other UI transitions use `--transition-ui` (90ms). Under `prefers-reduced-motion` every duration is 0.

## State management
- Model tree (systems → containers → components), current level (L1–L4), selection, camera (pan/zoom)
- Git state: branch, ahead/behind, dirty count, and per-element diff status
- Validation state: per-element and per-line model errors
- YAML buffer ↔ model kept in two-way sync; layout positions optional per-node pins (`views.containers.layout`)

## Design tokens
All in `tokens/` as CSS custom properties, imported via `styles.css`:
- Ground #f2f2f3, ink #1d1f20, one steel accent **#496b8d** with 100–900 ramps (`tokens/colors.css`). The ramp is semantic — 100 faintest, 900 strongest — and the dark theme mirrors it, so `-600` is the brand fill in both.
- Semantic + diff colour: `--color-danger/-warning/-success`, `--diff-added/-removed/-changed/-conflict`.
- Theme follows the OS; `data-theme` pins a subtree light **or** dark.
- Type: `--text-2xs` … `--text-3xl` (10–42px), `--leading-*`, `--tracking-*` (`tokens/typography.css`). Fonts vendored as woff2 in `assets/fonts/` (`tokens/fonts.css`) — no CDN, so the app renders offline.
- Spacing: 3 / 7 / 10 / 14 / 20 / 27 (`tokens/spacing.css`), plus `--radius: 0`, shadows, and a `--z-*` stacking scale.
- Shell + density: panel min/max, `--canvas-min`, and the `--control-*` / `--row-py` / `--bar-py` density steps (`tokens/layout.css`).
- `--color-surface` is the recessed role — input wells, panel headers and footers, button-group troughs. Cards and panels stay transparent.
- Motion: `--duration-*`, `--ease-*`, `--transition-camera`, `--transition-ui` (`tokens/motion.css`).

## Accessibility
Target is **WCAG AA**, verified on the reference screen in both themes: no text pair below 4.5:1, no graphic boundary below 3:1. Keep it that way — `--color-text-muted` is the muted-text floor; `--color-text-faint` is decorative only.

## Components
React references in `components/` (JSX + `.d.ts` props + `.prompt.md` usage each):
- core/: Button, ButtonGroup, Tag, Input, Segmented, Card, Dialog
- diagram/: Canvas, EdgeLayer, Edge, DiagramNode

These are thin wrappers over the classes in `components/components.css` — implement against the classes or port the JSX. `_adherence.oxlintrc.json` encodes the prop and token contract for linting consuming code.

## Iconography
Lucide (lucide.dev), stroke-width 1.5, 16px, inline SVG. No icon font, no emoji, no filled icons. Unicode micro-glyphs (⎇ · ◦ ▸) in chrome are intentional — keep them.

## Assets
- `assets/mark.svg` / `assets/mark-dark.svg` — brand mark (concentric squares). Wordmark is live type (Barlow Condensed 600 uppercase, `--tracking-wide`), never an image.
- `assets/fonts/` — Barlow and Barlow Condensed woff2, latin + latin-ext, with `OFL.txt`.

## Files
- `readme.md` — brand guide (tone, visual foundations, accessibility, canvas architecture)
- `SKILL.md` — Claude Code agent-skill entry point (invoke as a skill, or just read)
- `styles.css` + `tokens/` + `foundations/` + `components/components.css` — production-usable CSS
- `components/core/`, `components/diagram/` — component references
- `ui_kits/app/index.html` — the canvas editor reference screen (open in a browser)
- `templates/canvas-editor/` — the same screen as a reusable template
