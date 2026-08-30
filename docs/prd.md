---
doc: prd
type: prd
status: draft
elements: [blastradius]
---

# Blastradius — Product Requirements

## One-liner

A local-first desktop app for beautiful, interactive C4 architecture models —
YAML-driven, git-versioned, with a continuous zooming canvas from system
context (L1) down to code (L4).

## The problem

Architecture documentation for real teams is stuck between two failure modes:

- **Drawing tools** (Miro, draw.io, Lucidchart, Visio) produce pictures that are
  disconnected from the repo, unreviewable in PRs, unmergeable, and stale within
  a sprint. Nobody trusts them, so nobody maintains them.
- **Docs-as-code tools** (Structurizr, PlantUML/C4, Mermaid) put the model in
  git where it belongs, but the feedback loop is compile-and-squint: edit text,
  regenerate, inspect a static image. Layout is either uncontrollable or fights
  you. There is no way to *explore* a model, and diagrams communicate exactly as
  well as their worst auto-layout.

The gap: **a model that lives in git and reviews like code, with an editing and
reading experience that matches a design tool.** Teams keep architecture docs
current when the docs are in the same repo, the same PRs, and the same review
flow as the code they describe — and when the artifact is genuinely pleasant to
navigate.

## Who it is for

**Primary: platform / infrastructure teams of 3–15** who own the architecture
documentation for one or more product groups. They live in monorepos, review
everything in PRs, and are the people who currently maintain (or guiltily
neglect) the draw.io exports in `docs/architecture/`.

Characteristics that shape the product:

- Git fluency is assumed. Git-versioned YAML is a *feature* to this audience.
- They review each other's changes. **The diff is a first-class surface**, not
  an afterthought — "what changed in the architecture this quarter" is a query
  they actually run.
- They present to non-users: engineers on other teams, managers, auditors.
  Output artifacts (exports) matter as much as the editing experience.

Secondary: solo consultants and OSS maintainers documenting systems they own.
They arrive through the free tier and are the top-of-funnel.

## Product principles

1. **The repo is the database.** No account, no server, no sync service in the
   core product. A workspace is a folder of YAML and markdown in the user's
   repo; deleting the app leaves everything legible.
2. **Diffs must stay human.** Every schema and file-layout decision is judged
   first by what a PR diff looks like. Semantic changes and layout changes never
   mix in one file (ADR-0004).
3. **The camera is the interface.** Level navigation is one continuous
   map-style zoom — no page swaps, no crossfades. The model is a single space
   the user flies through (design system: motion tokens).
4. **Never colour alone, never picture alone.** Diff status and validation are
   encoded redundantly (colour + glyph + text), and every diagram has a textual
   twin (the YAML) — the model is fully usable by screen reader and greppable
   by CI.
5. **Documents are model objects.** ADRs, specs, PRDs attach to elements with
   typed, validated links. Docs rot becomes a model error the app can show,
   not a wiki problem nobody owns (ADR-0010).

## v1 scope

### In

- **Workspace**: open a folder containing `blastradius.yaml` — or a repo
  root, and the workspace inside is discovered; multi-file model
  with include globs; file watcher reloads on external edits
  (spec/model-format.md).
- **Model semantics**: C4 people, software systems, containers, components;
  directed relations with protocol labels; external-system flag; stable ids
  with display names (ADR-0003). L4 exists as a navigation level; code elements
  are out of scope for v1 (ADR-0004).
- **Canvas**: continuous zoom L1→L3, deterministic auto-layout with per-node
  pinning (ADR-0006); selection, keyboard navigation, hover; dark and light
  theme following the OS.
- **Bidirectional editing**: edit on canvas (create/rename/delete elements and
  relations, drag to pin) or in any text editor; both converge on the same
  files under the sync rules in spec/sync-engine.md. The in-app YAML panel is a
  full editor with syntax highlighting and inline validation.
- **Git awareness**: when the workspace is inside a git repo — branch and dirty
  state in the chrome; per-element semantic diff vs a chosen base (added /
  removed / changed) rendered as node badges and canvas states; conflict
  detection with in-canvas flagging (ADR-0007, spec/git-and-diff.md). Git is
  optional: a plain folder works with git features absent.
- **Docs integration**: markdown files with frontmatter register as typed
  documents linked to elements; broken links are validation errors; element →
  docs and doc → elements navigation (ADR-0010).
- **Structurizr import**: one-way importer from Structurizr DSL workspaces to a
  Blastradius workspace, with a written fidelity report of what did not map
  (ADR-0002).
- **Share**: export the model as a single self-contained interactive HTML file
  (zoom, both themes, no server needed), and the current view as PNG/SVG
  (ADR-0009, spec/export.md).
- **Platform**: Tauri desktop app for Windows, macOS, Linux (ADR-0005).

### Out (named, not implied)

- Hosted share links, accounts, or any server component — v2 revenue feature;
  the HTML export is designed to double as its upload payload (ADR-0009).
- In-app merge conflict *resolution* — v1 detects and displays; resolution
  happens in the user's editor/merge tool (spec/git-and-diff.md).
- L4 code modelling and source-derived elements.
- Real-time multi-user collaboration. Git is the collaboration protocol.
- Model-wide refactor tooling beyond rename (which is safe by construction —
  ids never change).
- Deployment/dynamic/sequence view types. v1 renders structural views only.

## The dogfood gate

`docs/` in the Blastradius repo is a workspace modelling Blastradius itself.
**v1 does not ship until the app can open `docs/`, render its views, resolve
every doc link, and show a correct semantic diff between any two commits of
this repo.** Every schema feature must be exercised by this workspace before it
is considered done; anything this folder cannot express is a schema bug first.

## Success metrics

> **Measurement status — decided 2026-08-30 (owner).** None of the metrics
> below have been measured, and the activation one will not be. It was carried
> as owed work through six releases (0.4.0 → 0.10.0) while three separate
> proxies for it shipped; the decision is to stop listing it as planned and say
> so here instead.
>
> What *is* verified, on every push since 0.10.0: the packaged app, launched at
> a repository it has never seen, reaches a rendered model of it — scaffold,
> canvas, an edit, and code extraction from an installed layout
> (`tools/smoke-app.ps1`). That proves the path exists and completes. It does
> not measure a person, and it is not a substitute for one: it cannot tell us
> whether someone unfamiliar understands what they are looking at, or how long
> they take, which is what the five-minute figure was about.
>
> The retention and commercial metrics are unmeasured for a simpler reason —
> they are proportions of a user population that does not yet exist. They are
> kept as targets, not claims.

Activation (free tier):

- Time from install → first rendered model **< 5 minutes** (template workspace
  in-app; `blastradius init` scaffold).
- Structurizr import succeeds without manual fixes on ≥ 80% of sampled public
  workspaces.

Retention / habit (the real test — does the doc stay alive):

- ≥ 40% of active workspaces receive a model-file commit in any given month.
- Median PR containing model changes touches ≤ 3 model files (diff hygiene
  holding up in practice).

Commercial (validated in v1, monetised in v2):

- ≥ 25% of weekly-active users export HTML at least once — demand signal for
  the hosted link.
- Waitlist conversion for the team tier ≥ 5% of active installs.

## Pricing hypothesis (not a commitment)

- **Free**: full local product, unlimited workspaces, exports watermarked with
  a "made with Blastradius" footer.
- **Team (paid, v2)**: hosted share links with access control, PR-comment
  diff rendering (CI bot), priority support. Per-editor pricing, order of
  $10–20/editor/month.

The core stays fully functional offline and free — the paid surface is
*distribution* of models, never authoring.

## Risks

| Risk | Mitigation |
| --- | --- |
| Bidirectional sync is the hardest v1 feature and could slip everything | Sync engine specced first (spec/sync-engine.md); read-only rendering path ships internally before editing; roadmap phases it explicitly |
| Deterministic layout that is also *good* is a research-grade problem | Constrain v1 to layered ELK with pinning as the escape hatch; "pin what you care about" is a documented workflow, not a failure |
| Schema churn after workspaces exist in the wild | `version:` field from day one; dogfood workspace forces schema exercise before release; migrations are a named deliverable in the roadmap |
| Tauri WebView divergence (WebKit vs WebView2) breaks the canvas | Design system is plain CSS + SVG (no Chromium-only features); CI screenshot suite across all three platforms |
| Structurizr import over-promises | Import ships with a per-workspace fidelity report; unmappable constructs are listed, never silently dropped |
