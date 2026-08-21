# Blastradius design system

The brand and component system for **Blastradius** — a local-first desktop app for beautiful, interactive C4 architecture models (YAML-driven, git-versioned, with a zooming canvas from L1 system context down to L4 code).

Derived from the Industry blueprint direction (steel-blue wireframe on a light technical ground) and extended with the app's own diagram grammar.

## Content fundamentals

- Tone: modern dev-tool — matter-of-fact, confident, no exclamation marks, no emoji.
- Copy speaks to "you"; the product speaks in the first person plural sparingly ("We're onboarding from the waitlist weekly").
- Headings are short and declarative, set in condensed uppercase: "Your architecture, at every altitude."
- UI microcopy is lowercase-terse where technical ("synced", "2 ahead", "offline · all local") and sentence case elsewhere.
- Technical nouns stay verbatim: YAML, git, L1–L4, container, mainframe. C4 vocabulary is used correctly (person / software system / container / component).

## Visual foundations

- Ground: light #f2f2f3, ink #1d1f20; one steel accent **#496b8d** with a 100–900 ramp.
- **The ramp is semantic, not absolute.** 100 is the faintest tint, 900 the strongest. Dark theme mirrors it, so `-600` is the brand fill in both themes and one interaction rule works in both: hover moves one step (`--color-accent-hover`), press moves two (`--color-accent-press`), always *toward* more contrast.
- Accent has three roles, split so light and dark can differ: `--color-accent` (fills, focus, selection), `--color-accent-ink` (accent as text), `--color-accent-line` (hairlines, node borders).
- **Semantic colour** exists — `--color-danger / -warning / -success` plus `--diff-added / -removed / -changed / -conflict`. One steel accent cannot say "added", "changed" and "invalid" at once. The three hues are desaturated to sit inside the blueprint world.
- **Colour never carries a state alone.** Every semantic tag, field error, and node badge also renders a glyph (`+ − ~ !`) with an sr-only label, so the model survives greyscale, print, and colour-blindness.
- Theme: follows the OS by default. `[data-theme="light"]` and `[data-theme="dark"]` pin a subtree in either direction.
- Type: Barlow Condensed 600 headings (uppercase for titles), Barlow 400/500 body, `--font-mono` for the model/YAML. Ten size tokens (`--text-2xs` … `--text-3xl`); nothing sets a raw px size. Floor is 10px.
- Corners: square. One token, `--radius: 0`.
- Spacing: 0.85× density on whole pixels — 3 / 7 / 10 / 14 / 20 / 27.
- Surfaces are **recessed, not raised**. Everything sits ON the ground; `--color-surface` is for the things cut slightly INTO it — input wells, panel headers and footers, button-group troughs. Cards and panels stay transparent: the blueprint is a line drawing, so surfaces are cut, never stacked.
- Blueprint frame: cards, figures, dialogs wear `.blueprint` + four `+` registration marks. Transparent line drawings — no surface fill (the solid accent primary button is the one exception). Never combine `.blueprint` and `.duotone` on one element; wrap instead.
- Elevation: `--shadow-sm/md/lg` for chrome; node shadows are their own tokens. Stacking is a `--z-*` scale, not DOM order.
- Imagery: rare; anything photographic goes through `.duotone`. Diagrams ARE the imagery.
- Hover/press as above; focus is the 2px accent `:focus-visible` ring. Disabled drops to 45% opacity.

## Accessibility

The system targets **WCAG AA** and the reference screen is audited against it in both themes.

- Every text/background pair meets 4.5:1; every border, edge and graphic boundary meets 3:1.
- `--color-text-muted` (65% ink) is the muted-text floor and is AA-safe. `--color-text-faint` exists for decorative marks only — never for text a user must read.
- The accent was darkened from #5980a6 to #496b8d for this: white-on-#5980a6 was 3.6:1, i.e. the primary button failed. The hue is unchanged.
- Nodes are real buttons: focusable, Enter/Space activated, with `aria-pressed` for selection.
- `Segmented` renders real radios, so the L1–L4 level switcher works by keyboard.
- Every duration collapses to 0 under `prefers-reduced-motion` — a camera zoom that cannot be opted out of is a vestibular hazard.

## The canvas

Nodes are DOM, edges are SVG, and both ride one transformed camera layer:

```
.canvas                     viewport — clips, paints the ground
  .canvas-camera            transform + --camera-scale; carries the dot grid
    svg.edge-layer          relations (one per canvas, holds the arrow marker)
    .node                   elements
  .canvas-overlay           chrome — zoom control, hints; never scales
```

- **The zoom rule:** everything inside `.canvas-camera` is part of the drawing and scales with it. Everything in `.canvas-overlay` is screen-space. `.screen-space` is the escape hatch for drag handles.
- Level navigation (L1↔L4) is a continuous map-style zoom on `--transition-camera` (520ms, `--ease-camera`). Nothing crossfades, nothing jumps.
- Edges use `vector-effect: non-scaling-stroke`, so a 1px hairline stays one device pixel at every zoom.
- Edges are **directed** — a C4 relationship always has a direction. `direction="none"` is a deliberate choice, not a default.
- Below 0.5× the dot grid coarsens to a 4× pitch rather than collapsing into a wash.
- C4 element type is encoded by geometry (`is-person` head, `is-container` spine, `is-component` fill, `is-external` dashed), never by colour — colour is spoken for by diff status, and a node must stay legible at L1.

## The app shell

Blastradius is a resizable desktop window, not a fixed 1200×640 mock, so the shell has a size contract:

```
.app                        column; the window
  .app-bar                  chrome
  .app-body                 query container — panels respond to the WINDOW, not the viewport
    .panel.panel-nav        model tree
    .canvas                 the drawing
    .panel.panel-side       model source
```

- Panels clamp between a min and a max (`--panel-nav-*`, `--panel-side-*`); `--panel-*-w` is runtime state that a `.panel-grip` drag writes.
- **The canvas never yields below `--canvas-min` (360px).** The three minimums add up to 788px, which is where the container queries start floating panels *over* the canvas instead of taking width from it: the source panel at 800px, the model tree at 560px.
- Bar items declare how readily they may be dropped as the window narrows — `.bar-drop-1/-2/-3` at 980 / 820 / 680px. The breadcrumb is the one elastic item and truncates. Without this the bar simply overflows below ~500px.
- Breakpoints are literal px because a `@container` condition may not read a custom property. They are the panel minimums added up, and the arithmetic is written next to them.

## Density

`data-density="compact | comfortable | spacious"` on `.app`. Control height goes 30 / 36 / 42px, with row padding and gutters moving with it.

Density changes **rhythm only**. It deliberately does not change type size: legibility should not depend on how tightly the user packs their panels, and the AA audit assumes a fixed type scale. The steps are discrete values rather than a scalar multiplier, which would reintroduce the fractional pixels the whole-pixel spacing rebase removed.

## Iconography

- Lucide (https://lucide.dev) at stroke-width 1.5, inline SVG, 16px in buttons/chrome. No icon font, no emoji, no filled icons.
- The brand mark (assets/mark.svg) is an original: concentric squares — outer 3px stroke (the radius), solid core (the system). Dark variant: assets/mark-dark.svg. Wordmark is live type (Barlow Condensed 600, uppercase, `--tracking-wide`), not an asset — see guidelines/logo-lockup.card.html.
- Git glyphs (⎇) and unicode separators (·, ◦, ▸) are used as micro-icons in chrome — keep them.

## Files

- styles.css — entry point (@imports only). Link this one file.
- tokens/ — colors.css (ramps, semantic, diff, themes), typography.css (scale + tracking), spacing.css (space, radius, shadow, z-index), layout.css (panel contract + density), motion.css, fonts.css.
- foundations/base.css — resets, headings, links, focus/selection, `.sr-only`.
- components/components.css — all component classes, the app shell (.app, .panel, .tree-row) and the app layer (.canvas, .canvas-camera, .node, .edge, .code).
- components/core/ — Button (+ButtonGroup), Tag, Input, Segmented, Card, Dialog (JSX + d.ts + prompt.md each).
- components/diagram/ — Canvas, EdgeLayer, Edge, DiagramNode.
- guidelines/ — specimen cards (Brand, Colors, Type, Spacing, Layout groups).
- assets/ — mark.svg, mark-dark.svg, fonts/ (vendored woff2 + OFL).
- ui_kits/app/ — the canvas editor screen composed from the system.
- templates/canvas-editor/ — the canvas editor as a template consuming projects can start from.
- SKILL.md — agent skill entry point.

## Intentional additions

- Diagram grammar (Canvas, EdgeLayer, Edge, DiagramNode, .code) — the product's core object, not part of the generic Industry set.
- Dark theme scope — the app ships both themes; Industry is light-only.
- Semantic + diff colour — required by a git-versioned model; Industry is single-hue.
- Motion tokens — the camera is the product's signature interaction and needs a spec.
- App shell + density — a desktop window resizes; a fixed-size mock is not a specification.

## Notes / caveats

- Fonts are **vendored** as woff2 under `assets/fonts/` (Barlow, Barlow Condensed; SIL OFL 1.1, licence included). A local-first app must render its own type offline, so there is no CDN dependency. Subsets are latin + latin-ext; add vietnamese there if the product ships vi.
- Component card previews use the CSS classes directly so they render before the bundle is compiled; the JSX components are thin wrappers over the same classes.
- `_ds_bundle.js` is compiled output, rebuilt for this revision: Babel classic-runtime JSX, `sha256(source)[:12]` hashes, all 11 components verified to execute. Regenerate it with the app's self-check after any further JSX edit.
