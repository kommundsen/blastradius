---
doc: roadmap
type: roadmap
status: draft
elements: [blastradius]
---

# Roadmap

Phases are gates, not sprints: each has an exit criterion that is demonstrable
on this repository. The dogfood gate (PRD) runs through all of them —
**`docs/` is the acceptance workspace for every phase.**

## Phase 0 — Model core (headless)

Rust library, no UI: workspace loading, schema parse + validation
(spec/model-format.md), doc frontmatter, semantic model diff between two
parsed models, `blastradius validate` CLI.

**Exit:** `blastradius validate docs/` passes in CI on this repo, and a
seeded-fault suite (dangling id, bad frontmatter, duplicate id) fails with
correct file+line errors.

## Phase 1 — Read-only canvas

Tauri shell + WebView: open workspace, ELK layout with determinism tests,
zoom L1→L3 with camera motion, selection/keyboard nav, themes, sidebar tree,
docs panel with element↔doc navigation. File watcher: external edits
re-render live. No editing of any kind.

**Exit:** open `docs/`, fly the model of Blastradius, click ADR-0007 from the
git-service node. Screenshot suite green on all three OS WebViews.

## Phase 2 — Git awareness

git2 integration: status chrome, semantic diff vs selectable base with canvas
diff states, layout-diff toggle, conflict flagging with ours/theirs
inspector, History time-travel (spec/git-and-diff.md).

**Exit:** on this repo, diff any two commits of `docs/` and the canvas states
match `git log` ground truth; a manufactured merge conflict renders
`is-conflict` and resolves round-trip through an external editor.

## Phase 3 — Editing & sync

The hard one (ADR-0008, spec/sync-engine.md): CST-preserving writes, canvas
operations (pin, rename, create, delete, relations), in-app YAML panel,
stale-state handling, shared undo, atomic writes, race abort.

**Exit:** a scripted torture session — simultaneous external edits + canvas
operations + malformed intermediate states — ends byte-identical to the
expected files, comments and formatting intact. All edits to `docs/` for one
real week made through the app.

## Phase 4 — Share & import

Self-contained HTML export, PNG/SVG, headless `blastradius export` for CI
(spec/export.md). Structurizr DSL importer with fidelity report (ADR-0002).

**Exit:** CI on this repo publishes `architecture.html` as a build artifact on
every merge; import corpus hits the PRD's 80% clean-import bar.

## Phase 5 — Polish & release

Onboarding (`init` scaffold + in-app template), packaging/signing/updates for
three platforms, performance budgets enforced (spec/sync-engine.md), a11y
audit against the design system's AA contract, docs site.

**Exit:** a platform engineer who has never seen the product reaches a
rendered model of their own repo in under 5 minutes (PRD metric), unassisted.

## v2 themes (not scheduled)

Hosted share links (ADR-0009's payload), in-app conflict resolution, PR-bot
diff rendering in CI, L4 source-derived elements, deployment views.
