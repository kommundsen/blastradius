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
git-service node — verified on a native window. Rendering verified in WebKit,
the constraining engine, via the Playwright suite in CI (ADR-0011 — a native
three-OS screenshot suite is unachievable: tauri-driver has no macOS support);
the shell compiles on all three OSes in the CI matrix. Native macOS/Linux
window verification is deferred to Phase 5 packaging, when that hardware is in
the loop anyway.

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
audit against the design system's AA contract, docs site. Named debts from
earlier phases land here, not silently:

- **Semantic dive choreography** — the camera glides *into* the dived node and
  *out of* the risen one (the motion spec's continuous-zoom intent), replacing
  Phase 1's fit-to-view transitions.
- **Native-window verification on macOS and Linux** (ADR-0011), alongside
  signing and installers.
- **Source editor upgrade** — the v1 YAML panel is a plain textarea; CodeMirror
  brings syntax highlighting and inline `.err` underlines at the offending line.
- **Performance-budget enforcement in CI** against a generated benchmark
  workspace (spec/sync-engine.md budgets are design targets until then).
- **Journal crash recovery** — transactions are journaled per workspace
  (sync.rs) but nothing replays them yet.
- **Granular staleness** — v1 blocks all editing on any stale file; the spec's
  intent is that a stale views file disables only pinning.
- **`workspace_open` at runtime** — the workspace is fixed at launch; switching
  (and a File → Open dialog) lands with onboarding.

**Exit:** a platform engineer who has never seen the product reaches a
rendered model of their own repo in under 5 minutes (PRD metric), unassisted.

## v2 themes (not scheduled)

Hosted share links (ADR-0009's payload), in-app conflict resolution, PR-bot
diff rendering in CI, L4 source-derived elements, deployment views, headless
SVG/PNG export via a node script over ui/js/layout.js (spec/export.md v1
boundary).
