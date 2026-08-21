---
name: blastradius-design
description: Use this skill to generate well-branded interfaces and assets for Blastradius, either for production or throwaway prototypes/mocks/etc. Contains essential design guidelines, colors, type, fonts, assets, and UI kit components for prototyping.
user-invocable: true
---

Read the readme.md file within this skill, and explore the other available files.
If creating visual artifacts (slides, mocks, throwaway prototypes, etc), copy assets out and create static HTML files for the user to view. If working on production code, you can copy assets and read the rules here to become an expert in designing with this brand.
If the user invokes this skill without any other guidance, ask them what they want to build or design, ask some questions, and act as an expert designer who outputs HTML artifacts _or_ production code, depending on the need.

Four rules are load-bearing — breaking them breaks the system, not just the look:

1. **Never write a raw px size, hex, or radius.** Every value has a token. Sizes are `--text-2xs`…`--text-3xl`, space is `--space-1`…`--space-8`, radius is `--radius` (0). `_adherence.oxlintrc.json` enforces this.
2. **The accent ramp is semantic, not absolute** — 100 faintest, 900 strongest, mirrored in dark. Use `--color-accent` / `-hover` / `-press` / `-ink` / `-line` rather than picking a step by eye.
3. **Colour never carries a state alone.** Diff and validation states always pair a semantic colour with a glyph (`+ − ~ !`) and an sr-only label.
4. **The system targets WCAG AA in both themes** — 4.5:1 text, 3:1 graphics. `--color-text-muted` is the muted floor; `--color-text-faint` is decorative only.

If you copy assets out for a standalone artifact, take `assets/fonts/` with them — the fonts are vendored, not loaded from a CDN.
