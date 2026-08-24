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

**Status: shipped except packaging** (2026-08-22). Onboarding, the named
debts, budget enforcement, the a11y audit, and the docs site are done;
packaging/signing/updates were deliberately deferred (below).

Shipped:

- **Onboarding** — `blastradius init` scaffolds a five-file commented starter
  workspace (validates with zero warnings; a test keeps it honest); the app
  gained a welcome screen, File → Open (Ctrl+O), runtime `workspace_open` /
  `workspace_init` / `workspace_demo`, and a native folder picker.
- **Semantic dive choreography** — the camera flies *into* the dived node and
  the deeper scene continues the forward motion (inverse on rise), on the
  motion tokens; prefers-reduced-motion collapses to a cut (e2e-asserted).
- **Source editor upgrade** — vendored CodeMirror 5: YAML highlighting and
  inline error-line marking replace the plain textarea.
- **Journal crash recovery** — write-ahead journal (intent/commit), replayed
  on open: undo survives restarts, torn writes roll forward, external changes
  while closed discard the journal (spec/sync-engine.md).
- **Granular staleness** — a stale views file disables only pinning into that
  view; model semantics keep flowing (spec/sync-engine.md).
- **Performance budgets in CI** — release-gated suite against a generated
  ~510-element workspace: parse < 50ms, drop→write < 30ms, keystroke core
  share < 150ms, render share < 100ms in WebKit (spec/sync-engine.md).
- **A11y audit** — axe-core WCAG A/AA scans of shell, welcome, dialogs, and
  source panel in both themes run in the WebKit gate; all findings fixed.
- **Docs site** — tools/build-site.mjs renders docs/ with the live model
  bundled; CI builds it every push and uploads the artifact. Pages
  *publishing* was dropped 2026-08-23 (owner decision — hosting not
  wanted for now); the build stays as a gate, and a deploy job can be
  re-added if hosting is ever wanted. Consequence: docs/privacy.md has
  no live URL — the Store submissions passed with the in-form privacy
  declaration, so none is currently needed.

Deferred to the packaging release (deliberately, user-decided):

- **Packaging/signing/updates** for the three platforms — `bundle.active` is
  still false. **Windows is shipped** (2026-08-22): Microsoft Store MSIX via
  the `winapp` CLI, Store-signed, Store-updated, zero cost. ADR-0013 has the
  decision, spec/msix-store-packaging.md the step-by-step guide plus the
  local-smoke-test and WACK-certification findings hit along the way. First
  submission passed certification 2026-08-22 and is in the Store's publish
  queue, awaiting public listing — nothing left to do but wait. macOS
  remains the open cost decision (Apple Developer ID, $99/year); Linux
  undecided.
- **Native-window verification on macOS and Linux** (ADR-0011) — needs the
  installers (or hardware in hand); WebKit-in-CI remains the rendering gate.
  Windows' packaged-install leg of this is now covered too, beyond the
  continuous dev-machine testing ADR-0011 originally scoped: the MSIX local
  smoke test (spec/msix-store-packaging.md step 12) exercised a real
  Start-menu launch, install/uninstall/reinstall, and external-edit
  detection against the installed package — and caught a real (if
  ultimately driver-related, not app-related) foreground-freeze issue that
  dev-mode testing never would have surfaced.

Known advisory (recorded 2026-08-22, revisit with the packaging release):
Dependabot flags `glib` 0.18 (RUSTSEC unsoundness in `VariantStrIter`,
medium, Linux-only GTK path pulled in transitively by Tauri 2 / rfd). The
fixed 0.20 line is unreachable until Tauri bumps its gtk-rs dependency —
`cargo update` confirms no compatible newer version. Not exploitable in our
usage; re-check when bumping Tauri for packaging.

**Exit:** a platform engineer who has never seen the product reaches a
rendered model of their own repo in under 5 minutes (PRD metric), unassisted.
Everything that makes that run possible now exists (README cold-clone guide,
init, welcome screen, File → Open); the measured run with a real stranger is
still owed and belongs with the packaged build, where "install" does not mean
`cargo build`.

## 0.2.0 — planned (2026-08-23)

**Store status**: 0.2.0.0 was submitted with stale 0.1.0-era binaries
(spec/msix-store-packaging.md Troubleshooting); the corrective
**0.2.1.0 is certified and public in the Store as of 2026-08-23** —
this release's features are what users actually receive.

**Shipped ahead of the themes** (2026-08-23): repo-root opening + the
manifest rename (ADR-0014). Open a repo root anywhere — app, CLI, MCP —
and the workspace inside is discovered (content-sniffed, never
filename-trusted); several hits raise a picker in the app and an explicit
candidate list in the CLI. The manifest is now `blastradius.yaml`
(self-identifying; legacy `workspace.yaml` loads with a deprecation
warning). The app knowing the repo root is the anchor for future
L4 source-derived elements.

Three themes, user-selected from the deferred pool; macOS packaging was
considered and deliberately not taken (the $99/year Apple Developer ID and
Mac-hardware loop stay an open decision). Store updates are cheap, so 0.2.0
ships when its themes are done, not when it is "big enough". Suggested
sequence: layout first (self-contained), export second (unlocks the PR
bot), conflicts last (hardest).

1. **Canvas & layout quality** — **shipped 2026-08-23**, with one recorded
   deviation: two-pass *interactive ELK* turned out to be the wrong tool
   (its interactive strategies preserve relative order but recompute
   coordinates, which would break the pins-are-exact contract). Shipped
   instead: a deterministic obstacle-avoiding post-pass in ui/js/layout.js —
   any edge whose polyline crosses a foreign node box is rerouted via
   Dijkstra over a visibility graph of inflated node corners (fixed
   per-hop bend penalty, fixed tie-breaks). This covers both offender
   classes at once: pinned-adjacent straight lines *and* ELK-routed edges
   that ignored pinned boxes. Pins never move; only lines do.
   *Exit met:* no edge segment crosses a node box — asserted three ways:
   unit (ui/tests/routing.test.mjs, incl. a synthetic forced detour and
   all three dogfood views), determinism (routes byte-identical across
   runs/instances), and DOM e2e (canvas.spec.mjs, L1 + dive-to-L2).
2. **Export & PR integration** — **shipped 2026-08-23**. Headless SVG/PNG:
   tools/render-views.mjs over the app's own pipeline (layout.js + the SVG
   assembly extracted to ui/js/svg.js, which the in-app Share menu now
   shares), tokens resolved headlessly from the design system, byte-stable
   output (ui/tests/render.test.mjs). CI's frontend job publishes both
   themes as the `architecture-renders` artifact on every push. The
   `model-diff` workflow posts one sticky comment per model-touching PR:
   semantic diff + rendered before/after views on a per-PR assets branch.
   *Exit met:* proven live on PR #1 — correct diff
   (`~ element blastradius.core.exporter`), all three view pairs rendered
   and embedded, comment upserted. spec/export.md §CI documents the tool.
3. **Deeper git workflows** — **shipped 2026-08-23** (ADR-0015). The
   conflict inspector gained keep-ours/keep-theirs per element and one
   apply action (undecided keep ours). The core `resolve` module rebuilds
   each conflicted file from the chosen side's stage text via CST splices,
   validates the whole outcome before writing (invalid resolutions are
   refused, working tree untouched), and stages through the user's own
   `git add` — libgit2 stays read-only per ADR-0007.
   *Exit met:* tests/resolve.rs manufactures real merge conflicts and
   asserts byte-clean outcomes: the resolved file equals the chosen base
   with exactly the decided splices, comments intact, index conflict-free,
   resolution staged, workspace valid. MCP-side resolution is a recorded
   follow-up (ADR-0015).

Version note: Store packages are `0.2.0.0` (the fourth digit is the
Store's; spec/msix-store-packaging.md).

## 0.3.0 — planned (2026-08-23)

Three user-selected themes; macOS/Linux distribution was considered and
deliberately deferred again (same cost/hardware decision as 0.2.0).

1. **L4 code introspection** — source-derived elements below L3, the
   payoff ADR-0014's repo-root anchor was built for. **Language priority
   is C#/.NET and JavaScript/TypeScript** (the user's stack), plus
   Rust added 2026-08-23 so this repo's own crates dogfood the feature.
   Shape settled 2026-08-23 in ADR-0016 + `spec/l4-introspection.md`:
   per-language extractors on the native compiler APIs (TypeScript
   compiler API; Roslyn syntax-level; `syn` built into core for Rust —
   the engines beneath the language servers, not the LSP protocol and
   not tree-sitter), emitting a common facts JSON committed under
   `docs/model/derived/` with a CI staleness gate; modules + types
   granularity; strictly opt-in per component via a repo-root-relative
   `source:` mapping (hand-modeled L4 stays a first-class peer);
   derived elements read-only, never written into workspace YAML.
   *Exit:* dive below an opted-in component and see the real code
   graph — dogfooded on `ui/js/` (TypeScript) and a Core component's
   Rust modules, with a C# fixture corpus exercising the same path in
   tests.
   **Shipped 2026-08-23, exits met.** All three extractors landed
   (core `introspect` module with `syn`; `extractors/typescript/` on
   the compiler API; `extractors/dotnet/` on Roslyn syntax trees with
   a byte-exact fixture gate); `blastradius introspect [--check]` +
   the same-named MCP tool; derived elements graft under `.src.`,
   answer in find_elements/element/blast_radius (code-level fan-in),
   and refuse writes with a source-pointing error; the canvas dives
   component → modules → types with derived/stale styling and an
   `open_source` inspector link; the cross-language `sourceDigest`
   staleness probe is verified byte-identical between Node and Rust.
   Both dogfood mappings are live (`blastradius.ui.canvas` ← ui/js,
   `blastradius.core.git-service` ← git.rs+resolve.rs, exercising
   include globs) and CI gates them with `introspect --check`. The
   gate proved itself during development: editing `ui/js` for the L4
   canvas work immediately flagged the canvas facts stale. Deferred
   as recorded in the spec: C# `--semantic` (MSBuildWorkspace) mode,
   external-dependency rollup nodes, transitive `pub use` following.
2. **Agent workflow deepening** — MCP conflict resolution (the recorded
   ADR-0015 follow-up), plus richer read tools where task-shaped gaps
   show up (e.g. rendering a view for an agent, richer model queries).
   *Exit:* an MCP client resolves a manufactured merge conflict
   end-to-end through the server (integration-tested in tests/mcp.rs),
   and the skill/onboarding text teaches the new tools.
   **Shipped 2026-08-23, exits met.** `git_status`, `git_conflicts`
   (element-shaped, ours/theirs inline), and `resolve_conflicts` ride
   the ADR-0015 pipeline through the MCP server; the exit-criterion
   test manufactures a real merge conflict with git2 and resolves it
   entirely through the tools (byte-clean, comment preserved, staged,
   model reloaded). The onboarding primer (all four agent formats) and
   the dogfood SKILL.md now teach conflicts + L4 introspection — the
   dogfood copy also caught up with the ADR-0014 manifest rename.
   Deferred, recorded: a render-a-view tool — agents consume structure
   better as JSON today (`element`, `blast_radius`, `model_diff`), the
   render path is node-side, and no task has needed pixels yet;
   revisit when one does.
3. **Release ops automation** — msstore-cli submission from CI (tag push
   → Store submission; needs Partner Center API credentials as GitHub
   secrets, an owner step), the arm64 MSIX in the same submission, and
   the owed 5-minute-stranger exit run against the Store build.
   *Exit:* 0.3.0 itself is submitted to the Store by CI, x64 + arm64,
   with the stranger run's result recorded here.
   **Shipped 2026-08-24, primary exit met**: the `v0.3.0` tag push
   built x64+arm64 (`pack-msix.ps1 -Arch arm64` cross-build), bundled
   one `.msixupload`, and `tools/submit-store.ps1` (submission REST
   API — msstore-cli cannot take prebuilt packages, see the spec's
   Troubleshooting) committed submission 1152921505701723784, accepted
   into PreProcessing. Both architectures in one submission, entirely
   from CI; a workflow_dispatch dry run (draft submission, no commit)
   validated the pipeline first. First-ever arm64 build — untested on
   real arm64 hardware; certification is its first gauntlet, and an
   x64-only resubmission is the fallback if it fails there.
   Still owed: the 5-minute-stranger run against the published Store
   build, result to be recorded here.

## v2 themes (not scheduled)

**Bundled in-app help** (idea 2026-08-23, follows dropping Pages
hosting): ship a curated *user-facing* doc set inside the app — getting
started, model-format reference, keyboard shortcuts, privacy policy —
rendered by the existing docs-panel machinery (offline, versioned with
the binary, no hosting). Deliberately not the dogfood ADR/spec site:
that is contributor material, already readable in-app by opening this
repo, and stays a CI artifact.

Hosted share links (ADR-0009's payload), in-app conflict resolution, PR-bot
diff rendering in CI, L4 source-derived elements, deployment views, headless
SVG/PNG export via a node script over ui/js/layout.js (spec/export.md v1
boundary).

**Layout polish** (diagnosed 2026-08-22, measured in scratch experiment):
auto-only views are near-optimal already — ELK layered produces 2 crossings
total across the three dogfood views, 0 once
`crossingMinimization.forceNodeModelOrder` is relaxed (trade-off: layouts
shift more under model edits; determinism unaffected — the seed is fixed
either way). The visible mess is elsewhere: (1) **edge labels collide** —
midpoint placement with no collision avoidance stacks labels into run-on
text where edges converge (L1) and clips them under nodes (L2); (2) **edges
touching pinned nodes bypass ELK** — drawn as straight center-to-center
lines that ignore obstacles, so they pass beneath nodes and bunch labels.
**Shipped same day**: (1) label de-collision — a deterministic placer scores
candidate positions along the path, beside it, and shifted sideways against
node boxes, other labels, and how much of the edge's own line the knockout
would erase (short edges keep their arrows visible); e2e asserts no label
sits on a node at L1/L2. (2) model-order forcing relaxed to PREFER_NODES
(0 crossings measured). (3) minimum distance on pin drops — a drop that
would land a node against a neighbour nudges to the nearest clear grid cell
(deterministic ring scan, e2e-asserted). The containers-view pins were
respaced for label room. Still open for later: two-pass interactive ELK so
pinned-adjacent edges get real routing; obstacle-avoiding routing is v2.

Follow-up (same day, "islands" report): scoped views now join outside
elements only when a relation touches the scope's strict *interior* — a
relation to the bare scope element has no visible node to attach to, so it
no longer pulls disconnected nodes in (this hides context-level actors like
the reviewer from L2, correctly: their relation is to the system as a
whole). `include-context:` turned out to be parsed but never honored by the
renderer — now implemented. Model gaps found by the island audit and fixed
via MCP: cli → importer/git-service, ipc-bridge → git-service, and the App
Shell gained its L3 (Window & Lifecycle, IPC Bridge, File Watcher — the
watcher was mis-modelled under Core; it lives in the app crate). An
every-view connected-components audit now reports 1 component per view.

**MCP server** — raised as a candidate 2026-08-22 and **shipped the same
day** (ADR-0012, spec/mcp-server.md): `blastradius mcp` serves ten tools
over stdio; reads are task-shaped (blast_radius, element, model_diff, doc),
writes route through the sync engine's CST-preserving splices with shared
undo. Building it surfaced and fixed two latent core bugs (system rename,
silently-dropped context-file relations). `blastradius init` grew the
matching onboarding: it offers `git init`, project-scoped MCP registration,
and skills/instructions for Claude Code, Copilot, Cursor, and Codex
(merge-only writes; `--git/--no-git/--agents/--skills` for scripts).
