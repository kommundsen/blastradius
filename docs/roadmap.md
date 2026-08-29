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
usage; re-check when bumping Tauri for packaging. Re-checked
2026-08-24: still unreachable — latest Tauri (2.11.5) itself requires
`gtk ^0.18` → `glib ^0.18`; needs a Tauri release on the gtk-rs 0.20
line, none published.

Re-checked **2026-08-25** (the alert resurfaced on the push that closed the
first-user findings): unchanged, and the blast radius is smaller than the
alert suggests. `cargo update -p glib --precise 0.20.0` still fails on
`gtk 0.18.2` requiring `glib ^0.18`, and 2.11.5 is still the newest Tauri
published, so there is nothing to move to. `cargo tree -i glib --target
x86_64-pc-windows-msvc` prints *nothing*: glib is not in the Windows graph
at all, so neither the Store package nor the Windows portable archive
contains the code — it is reachable only in the Linux build, only through
Tauri's GTK backend, and we call glib directly nowhere. The advisory is
unsoundness in `VariantStrIter`'s iterator impls, which needs code that
constructs and iterates one; nothing of ours does. Nothing to do but wait
for Tauri's gtk-rs bump.

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

## 0.4.0 — released (2026-08-24)

**Cut 2026-08-24**: all three themes shipped; version bumped across the
three surfaces (`Cargo.toml`, `tauri.conf.json`,
`packaging/msix/Package.appxmanifest`) and tagged `v0.4.0`, which drives
the CI submission pipeline proven in 0.3.0.

**Submitted from CI 2026-08-25**: submission 1152921505701732949,
x64 + arm64 in one `.msixupload`, accepted into PreProcessing — the
third release to go out entirely through the pipeline.

It took two attempts. The first failed because an in-progress
submission created in the Partner Center *web dashboard* can be neither
committed nor deleted through the submission API, so `-ReplacePending`
could not clear it either; the owner deleted it in the browser and the
re-run went straight through. Recorded in
spec/msix-store-packaging.md Troubleshooting, with the rule it implies:
once CI owns submissions, do not open one in the dashboard.

Three user-selected themes. macOS/Linux distribution was considered and
deliberately deferred a third time (same cost/hardware decision; revisit
at 0.5.0). Going-public execution (the ADR-0017 repo flip: public
README, CONTRIBUTING, sponsor setup) was considered and left
unscheduled. Suggested sequence: introspection first (incremental,
self-contained), deployment views second (heaviest, needs spec work),
help last — so the help content documents the release's final feature
set, deployment views included.

1. **Introspection deepening** — **shipped 2026-08-24, all three exits
   met** (details per sub-item below). The recorded L4 follow-ups from
   ADR-0016/spec/l4-introspection.md: C# `--semantic` mode
   (MSBuildWorkspace-backed, resolving cross-project references the
   syntax-level pass cannot), external-dependency rollup nodes, and
   transitive `pub use` following for Rust.
   *Exit (draft):* the C# fixture corpus gains a multi-project solution
   that syntax mode gets wrong and `--semantic` gets right; external
   deps render as rollup nodes on an opted-in dogfood component;
   transitive `pub use` re-exports resolve to the defining module.
   *Exit amended 2026-08-24* for the `pub use` leg: the original wording
   said "on this repo's own crates", but the audit found the repo's only
   re-exports (`crates/blastradius-core/src/lib.rs`) have **no consumer**
   — every module imports directly by convention — so no dogfood edge
   can move no matter how the include globs are widened. Proven on a
   fixture chain instead (two façade hops plus an `as` rename), which is
   the honest test; widening the globs was considered and rejected as
   buying nothing observable.
   **C# semantic mode shipped 2026-08-24, exit met.** Opt in with
   `mode: semantic` on the source mapping; MSBuildWorkspace loads the
   target's own solution and resolves edges from symbols. The fixture
   is a two-project solution where `Alpha.Widget` and `Beta.Widget`
   share a simple name and the consumer reaches Alpha's through a
   `global using` in another file: syntax mode sees two candidates and
   drops the edge, semantic mode resolves
   `Gamma.Consumer → Alpha.Widget` across the project reference.
   Failure of any kind (no SDK, no solution, unrestored, load error)
   degrades to the syntax pass with a stderr warning — never worse
   than v1 — and the effective mode is recorded in the facts, which is
   what lets `--check` distinguish "this machine can't run semantic"
   from "these facts are stale". The semantic check asserts the
   resolved edge rather than byte-comparing, because semantic output
   depends on the resolving SDK; the syntax and fallback checks stay
   byte-exact. Fixed on the way: extractors were spawned from the
   target repo, so a repo pinning an old SDK in `global.json` would
   fail to build our own net8.0 extractor — a bug present since 0.3.0.
   **External-dependency rollups shipped 2026-08-24**, all three
   languages, one node per package (`dep.<package>`, kind `dependency`,
   parentless and pathless). Rust reads the first segment of an
   unresolved `use` (sysroot excluded); TypeScript uses the resolver's
   `packageId` and falls back to a lexical read so facts don't depend on
   whether `node_modules` is installed (`node:` excluded); C# proxies
   packages by the namespace root of a non-corpus `using` (`System`
   excluded), attributed to the declaring namespace. Dogfood proof:
   git-service now shows `dep.git2` and `dep.serde`. Two gaps closed
   along the way — the **TypeScript extractor had no tests at all**
   (it now has a byte-exact fixture gate like the C# one, both wired
   into CI), and the L4 inspector rendered a dead "open source" button
   for any pathless element (dependency rollups, and C# namespaces,
   which shipped that way in 0.3.0). Also fixed: the inspector
   uppercased code identifiers, so a struct `CommitInfo` read
   `COMMITINFO` — the canvas honored the case-preserving contract, the
   inspector didn't.
   **`pub use` shipped 2026-08-24.** There was no re-export logic at all
   — the spec's "followed one level" was accidental behavior, and a
   consumer importing through a façade got an edge to the façade (or
   nothing). Now a fixpoint-built per-module export table resolves
   re-exports transitively, honors renames, and points edges at the
   defining module; glob/module re-exports and re-export cycles are
   dropped, not guessed. Dogfood facts changed by exactly the extractor
   version line, as predicted.
2. **Deployment views** — **shipped 2026-08-24, exit met** (ADR-0018).
   The design question was how to draw a nested tree: C4 draws nested
   boxes, but every Blastradius view shows one altitude and dives, and
   the layout engine is flat with fixed node sizes. Nesting would have
   meant hierarchy support in ELK plus ancestor-exclusion in obstacle
   routing, label placement against containment boxes, and SVG
   z-ordering — the largest and riskiest piece of the theme, for the
   only view in the product that reads by containment. **Deployment
   dives instead**, which is cheaper *and* more consistent, and because
   deployment elements carry dotted ids the existing depth arithmetic
   computes their views with no new algorithm.
   They are ordinary elements, not a parallel namespace like L4 code, so
   relations, pins, blast radius, diff, MCP, and canvas editing all work
   without special cases; the cost was four exhaustive `ElementKind`
   matches and a variable-depth key chain, since deployment nodes nest
   arbitrarily while containers sit at fixed depths. Instances name the
   container they run, validated, so a deployment cannot drift into
   naming containers that no longer exist — and they borrow that
   container's display name unless given their own.
   *Exit met:* this repo's own delivery is modelled in
   `docs/model/deployment.yaml` — dev machine, CI runners, Store
   distribution, 20 elements across 3 environments — validating,
   diveable to the container instances, rendered headlessly to SVG, and
   navigable in the exported HTML like any other view. Recorded
   follow-ups in ADR-0018: nested-box rendering as an optional mode,
   instance multiplicity, and importing Structurizr's deployment blocks
   (until now parsed and discarded).
   As scoped: the C4 deployment diagram — environments, deployment
   nodes, container instances — mapped onto the existing model format
   and canvas, spec first (`spec/model-format.md` §3b), then rendering.
3. **Bundled in-app help** — **content shipped 2026-08-24; the stranger
   run is still owed.** Eleven feature-usage pages live in `ui/help/` as
   real markdown, reached from a **Help** button, `?`/`F1`, or the
   welcome screen — which is where a first-run user actually is. They
   render in the existing docs panel, and because Tauri compiles the
   whole `ui/` tree into the binary they ship offline and versioned with
   the app; an e2e test asserts no request leaves the machine while
   reading them. Delivered as fetched markdown rather than an IPC
   command on purpose: a new command would need a mock branch in every
   Playwright run (ADR-0011), and markdown files stay reviewable and
   diffable.
   Pages: getting started, canvas navigation, editing and pinning,
   deployment views, code-level detail, git diff/history/conflicts,
   export and sharing, coding agents (MCP), model-format reference,
   keyboard shortcuts, privacy. The privacy page is generated from
   `docs/privacy.md` and a test fails if the two drift — it has no live
   URL since Pages publishing was dropped, so in-app is the only place
   it is readable. The panel had no router, so cross-page links are
   rewritten to navigate in place rather than unloading the app.
   *Exit, first half met:* every shipped feature is reachable from an
   in-app Help entry point with no network, asserted in e2e.
   **Still owed: the PRD 5-minute-stranger run** against the published
   0.4.0 Store build — result to be recorded here.
   Original scope (2026-08-24):
   author the feature-usage doc set — getting started, canvas
   navigation/diving, editing and pinning, git diff and conflict
   resolution, L4 introspection setup, export/share, MCP/agent setup,
   keyboard shortcuts, model-format reference, privacy policy — and
   ship it in-app via the existing docs-panel machinery (offline,
   versioned with the binary). The content is the bulk of the work.
   *Exit:* every shipped feature is reachable from an in-app Help
   entry point with no network; **plus the owed PRD 5-minute-stranger
   run**, folded here by decision 2026-08-24: a platform engineer who
   has never seen the product installs the published 0.4.0 Store build
   and reaches a rendered model of their own repo in under 5 minutes,
   unassisted — result recorded in this file.

## 0.5.0 — released (2026-08-25)

**Released 2026-08-25.** Tagged `v0.5.0`; the pipeline built and
published both channels from the one tag.

- **Portable archives are live** on the GitHub Release — Windows zip and
  Linux tar.gz, install-free. They published while the Store job was
  still running, which is exactly the independence the job split was for.
- **Store**: submission 1152921505701734607, x64 + arm64, accepted into
  PreProcessing. It replaced 0.4.0's still-pending submission (owner
  decision 2026-08-25): 0.5.0 supersedes it, so users move from 0.2.1.0
  straight to 0.5.0 and arm64's first certification restarts on identical
  packaging code. **0.4.0 therefore never publishes.**

**First outside use, 2026-08-25**: someone the owner knows tried the
release and reported a positive experience. Recorded as encouragement,
**not** as the PRD metric — that one is specific (a platform engineer who
has never seen the product, their *own* repo rendered, unassisted, under
five minutes, timed) and none of those conditions were measured here.
Worth chasing the details while they are fresh; if they hold, this
becomes the exit run.

Still owed: the PRD 5-minute-stranger run. The portable archive now gives
it a second, faster path that does not wait on certification.

Three themes, user-selected from a pool of five; search and the remaining
deployment follow-ups were deliberately held for 0.6.0 to keep the Store
cadence quick. macOS distribution was considered and **deferred a fourth
time** — the $99/year Apple Developer ID and the Mac-hardware loop remain
the open cost decision, unchanged by the repo going public.

Sequence matters this time: **containment rendering comes first**, because
theme 1 and a 0.6.0 item both sit on it.

1. **Grouped elements, on a containment renderer** — a `group:` label on
   elements that draws a boundary box around them. Decided 2026-08-25:
   grouping is **presentation, not structure** — ids stay
   `system.container`, no new altitude, no new parent, so ADR-0003
   identity and every existing relation are untouched. This is exactly
   Structurizr's `group` semantics, which matters because the importer
   already detects `group` and *flattens it* with a "groups are not
   modelled" diagnostic (`import.rs`) — real workspaces lose their
   grouping on import today, against the PRD's 80% clean-import bar.
   Rendering is **opt-in per view**, off by default, so no existing
   diagram changes shape.
   The hard part is the renderer, not the schema: `ui/js/layout.js`
   builds a flat ELK graph with fixed node sizes, and ADR-0018 dodged
   containment on purpose. This theme finally pays for it —
   `hierarchyHandling` with real nested children and padding,
   ancestor-exclusion in the obstacle-routing pass (a child inside its
   parent must not read as an edge collision), label placement against
   boundary boxes, and SVG draw order and fills so a box never paints
   over what it contains.
   *Exit (draft):* a dogfood view groups its elements behind a flag,
   renders with boundaries in the app and headlessly, and the Structurizr
   importer stops emitting "groups are not modelled" for a corpus
   workspace that uses them.

2. **Architecture drift detection** — **shipped 2026-08-25** (ADR-0019).
   The premise turned out to be wrong: ADR-0016 assumed cross-component
   code edges were already being recorded, but **there were none, and
   could not be** — each component is extracted against its own corpus,
   so a reference to another component's code fails to resolve and is
   dropped. `git.rs` really does import `crate::model`,
   `crate::diagnostics`, `crate::vfs`, `crate::splice` and `crate::diff`;
   every one was discarded at extraction time.
   So extractors now record what they used to throw away: an `outbound`
   entry naming the repo-relative file a reference points at. The
   workspace resolves that file to whichever component's mapping owns it,
   which turns a raw reference into a code dependency between components,
   and compares it against the declared relations — lifted through the
   hierarchy, so a container-level relation covers its components.
   Reported both ways: **undeclared** (code goes somewhere the model does
   not say) and **unbacked** (a declared relation with no code behind it,
   which usually means it points the wrong way). Unbacked is only claimed
   between components in the same language — a TypeScript canvas calling
   a Rust engine over IPC is a real relation no static import can
   evidence. Warnings by default; `validate --strict-drift` is the CI
   opt-in, and this repo now runs it as a hard gate.
   *Exit met, and it earned its keep on first run*: it found three real
   problems in our own model — two undeclared dependencies from
   `git-service`, and a declared `model-service -> sync-engine` edge with
   no code behind it, because the dependency runs the other way and the
   relation had been written as a data flow. All three are corrected, and
   the dogfood model is now drift-free with the gate enforcing it.
   The original framing: the follow-up ADR-0016 named as
   "the natural one this design enables". L4 facts already record edges
   that cross component boundaries; nothing yet *judges* them. Compare
   them against the declared L3 relations and report the disagreements:
   code that depends on something the model never declared, and declared
   relations with no code behind them. Surfaced on the canvas, in
   `validate`, and to agents over MCP.
   This is the product thesis made enforceable — the PRD's whole claim is
   documentation that cannot quietly rot, and this is the first feature
   where the model is checked against reality rather than against itself.
   *Exit (draft):* a seeded drift in this repo (an undeclared
   cross-component import) fails a CI gate with the offending code edge
   named, and clearing it passes.

3. **Reach: a non-Store install path** — **shipped 2026-08-25**. Every
   tag now builds portable archives for Windows (zip) and Linux
   (tar.gz) and attaches them to a **GitHub Release**, so there is a
   plain download URL rather than an artifact buried in a workflow run.
   Each bundle carries both binaries, the out-of-process extractors, the
   licence, and a README; it needs no installer and no admin rights.
   The portable job is deliberately independent of the Store job — a
   submission problem must not take down the only download that works on
   a machine without the Store — and the pipeline smoke-tests the staged
   bundle by validating this repo's own workspace with it before
   archiving.
   One real bug surfaced while testing it from outside the repo:
   TypeScript introspection resolved the compiler relative to the
   *extractor*, so a portable install — which ships no `node_modules` —
   could never do L4 on a TypeScript project. It now falls back to the
   repository being analysed, which is where a TypeScript codebase keeps
   its compiler anyway.
   *Exit met:* a staged bundle run from a directory outside the checkout
   validates and introspects a workspace, TypeScript included.
   The original scope: publish a **portable zip** of the
   built binaries on every tag, and a **Linux** AppImage/deb from CI. No
   signing fees, no new hardware. Prompted by a real case on 2026-08-24:
   an Intune-enrolled machine with the Store app removed had no install
   path at all — winget cannot help (Blastradius has no winget manifest
   and the `msstore` source needs Store infrastructure), and the CI MSIX
   is deliberately unsigned because the Store signs during ingestion, so
   it cannot be sideloaded either. Windows distribution being Store-only
   is a real gap for locked-down and air-gapped machines.
   *Exit (draft):* a tag publishes a portable Windows zip and a Linux
   package that both run on a clean machine with no installation and no
   admin rights.

**Carried, unchanged:** the PRD 5-minute-stranger run, still owed against
a published Store build; and arm64's first real-hardware exposure, whose
first test remains Store certification.

## 0.6.0 — released (2026-08-25): the first-user findings

**Cut 2026-08-25.** No theme selection: 0.6.0 is what the first outside
user found, fixed. That is exactly what the hold below was waiting for —
"a single real reaction is worth more than re-ranking the list" — and the
reaction arrived before any theme was chosen, so the release is the
reaction rather than the list.

Two of the five were worse than reported. **No Store build has ever been
able to introspect TypeScript or C#**: the package shipped two
executables and no `extractors/`, so the one language that worked was
Rust, being compiled into core. And the SVG export had been **silently
dropping the protocol** of every relation that also carried a label,
since the renderer picked one or the other.

Everything below is covered by tests; the two items that need a real
installed machine are listed at the end of this section and carry into
the next release.

### First-user findings (2026-08-25) — **all five fixed**, 2026-08-25

Five issues from the first person to use Blastradius without the owner
driving. Every one is now fixed, with the root cause and the guard recorded
below; two of them turned out to be worse than reported.

| # | Reported as | Actually | Fixed by |
| --- | --- | --- | --- |
| 1 | "unable to run C# introspection" | no Store build has ever shipped `extractors/`, so C# *and* TypeScript were impossible on every installed copy | `b4753eb` |
| 2 | "the skill guessed the schema" | the schema was unreachable, the write tool under-specified, and no bulk path existed | `e32e07f` |
| 3 | "help stays on help" | `select()` cleared everything but `state.help` | `2ee1909` |
| 4 | "the first dialog should just open a folder" | and a folder with no workspace was an error, not an offer | `f555a6a` |
| 5 | "show protocol as a tag" | C4 brackets — and the SVG export was silently dropping protocols | `bd6720c` |

Two decisions taken along the way. The C# extractor now ships **published**
rather than as a project: an install directory is read-only, so `dotnet run`
could not have worked there even once the source was staged, and publishing
drops the requirement from an SDK plus a first-run NuGet restore to the
runtime alone. And the bracket convention applies to **elements as well as
relations** (`[Container: Rust]`, owner decision) — it restyles every node in
the product, and looks better for it.

What follows is the plan as it was written, kept for the reasoning.

### 1. The Store build cannot introspect TypeScript or C# at all

**Worst of the five, and reported only as "C# introspection didn't work".**
`tools/pack-msix.ps1` stages exactly two files into the package — the app and
the CLI — so there is no `extractors/` directory beside the installed binary.
Core looks for `current_exe()/../extractors`, then `<repo-root>/extractors`;
on an installed machine neither exists, so every C# **and TypeScript**
mapping fails with "no csharp extractor found". Only Rust works, because it
is compiled into core.

The 0.5.0 portable archive already ships the extractors — this is specific
to the MSIX, and it has been true of every Store build ever published.

- Stage `extractors/` into the package in `pack-msix.ps1` (minus
  `node_modules`, `bin`, `obj`, `fixtures` — the same filter
  `tools/stage-portable.mjs` already applies; factor it so the two cannot
  drift). Note `Remove-Item packaging\msix\dist\*` needs `-Recurse` once the
  staged tree contains directories.
- Make the failure legible when it does happen: the error should say the
  language needs an extractor, where it looked, and that Rust needs none.
- Two runtime prerequisites remain, and should be named in the error rather
  than discovered: TypeScript needs Node, C# needs a .NET SDK and a first-run
  NuGet restore of `Microsoft.CodeAnalysis`. On a locked-down machine that
  restore is itself a failure mode.
- Verify by installing the package and running `introspect` on a C# repo —
  the only way to catch this class of bug, since it cannot reproduce in a
  checkout.

### 2. The agent edited files by hand and looped on validation

**Corrected 2026-08-25 after the owner checked**: the MCP server *was*
reachable and did return results. The agent used the read tools, then wrote
YAML directly, and validation failed repeatedly. So this is not a
tools-missing problem — the earlier PATH/approval theory was wrong.

Hand-editing is *allowed* by design: files are the source of truth (ADR-0008)
and external edits are first-class. What is not acceptable is that an agent
doing so has nothing to write *against*. Three gaps, all ours:

- **The schema is unreachable.** `spec/model-format.md` lives in this
  repository, not in the user's. No MCP tool returns the format. The primer
  says "run `blastradius validate` afterwards" but never says what valid
  looks like — so an agent asked for a sample file and inferred the schema
  from it, which is exactly what was reported. → Add a **`model_format`
  tool** returning a compact authoritative reference (element kinds, the
  nesting, relations, docs frontmatter, deployment, groups), and embed the
  same summary in the skill so it is available before the first tool call.
- **`apply_operation` is under-specified for a machine.** Its input schema is
  `{"op": {"type": "object"}}` with every real shape described only in prose,
  so mistakes are easy and come back as serde errors. → Give it a proper
  `oneOf` JSON Schema per operation; the model then cannot misshape a call.
- **There is no bootstrap path.** Modelling a repository from scratch means
  dozens of single `create` and `add-relation` calls, so writing one file is
  the rational choice. → Either accept hand-authoring as the bootstrap route
  and support it properly (schema tool above, plus a `validate` call the skill
  is told to make *before* moving on), or add a bulk apply that takes a list
  of operations in one transaction.
- **"Prefer `apply_operation`" is too soft.** State the rule and the reason:
  it splices in place, validates before writing, and is undoable. If the
  agent does hand-edit, it must validate immediately and never re-serialize
  a file it did not write.

### 2b. The skill should teach C4, not just the tool

Owner request, and the sharper half of this: an agent that knows the file
format still models badly. The skill should carry a short set of dos and
don'ts —

- Stop at components. Below them is *derived* from source (`introspect`);
  hand-modelling classes is the classic way to make a model no one maintains.
- One system per file; ids are immutable and renaming means `name:`.
- **A relation is a dependency, not a data flow.** Our own model got this
  wrong — `model-service -> sync-engine` was written as "parse results" while
  the code dependency ran the other way, and drift detection caught it. That
  is a real, checkable example worth teaching.
- Name containers and components after what they *are*, and use `tech:`
  rather than encoding technology in the name.
- Model what a reader needs to reason about, not everything that exists.
- Attach ADRs to the elements they govern; a doc that names a dead element is
  a model error, not a wiki problem.
- Run `validate` before claiming done, and `blast_radius` before changing or
  deleting anything.

### 3. Help is a dead end

`select()` clears `state.doc` and `state.selectedRel` but not `state.help`,
and `renderSide()` tests help first — so once help is open, clicking an
element changes the selection on the canvas while the panel keeps showing
help. Reported exactly as "no way to switch back".

- Clear `state.help` in `select()` and `selectRelation()`: choosing something
  in the model is an unambiguous request to inspect it.
- Keep the Help button a toggle, so there is also a deliberate way out.
- e2e: click an element while help is open and assert the inspector returns.

### 4. Opening a folder should lead somewhere, not error

Today the welcome screen offers three routes (open / new / demo), and opening
a folder that is not a workspace falls through to an error naming the folder.
For someone pointing the app at their own repo for the first time, that is
the whole experience.

- **One primary action**: open a folder or repository. (Owner asked for this
  explicitly; the demo and "new workspace" routes need a decision — keep them
  as secondary, or drop them. The e2e onboarding spec asserts all three
  today.)
- On open, **detect** rather than fail: a workspace here → open it; one
  below → open that; several → pick; **none → offer to initialise**.
- The offer should be the useful one: scaffold the workspace *and* register
  the agent skills and MCP server, since that is what makes the next step
  work.
- On success, show **a sample prompt to paste into the agent** — the thing
  that turns an empty workspace into a model. Something like: *"Read this
  repository and model its architecture in the Blastradius workspace at
  docs/ — use the blastradius MCP tools, and validate when you're done."*
- This needs a runtime command for "initialise here with agents", which the
  CLI already has (`init --agents …`); the app currently exposes only
  `workspace_init`, which does not do the agent half.

### 5. Show technology the way C4 does: in brackets

Owner decision: follow the C4/Structurizr convention rather than inventing
one — technology renders in **square brackets**, so a relation reads

    calls
    [JSON/HTTPS]

rather than today's `calls · JSON/HTTPS`.

- **A real bug found while checking**: `ui/js/svg.js` renders
  `e.label ?? e.protocol`, so an exported SVG or PNG shows the label *or* the
  protocol, never both. Every relation carrying both loses its protocol in
  every exported diagram, while the in-app canvas shows it. Fix regardless of
  the rendering change.
- Apply the bracket form in all three surfaces that draw edges — the canvas,
  `svg.js` (export and headless render), and `viewer.js` — which currently
  disagree with each other.
- ~~Open question, deliberately not decided: elements.~~ **Decided
  2026-08-25** (owner): apply it there too. Classic C4 renders
  `[Container: Rust]` where ours showed a kicker reading `CONTAINER · RUST`.
  It restyles every node in the product, which is the point.
- No schema change: `protocol` already exists on relations. A free-form
  `tags:` list is a separate, larger question and is not part of this.

### Sequencing

1 and 2 first: they are the difference between a tester succeeding and a
tester giving up, and neither is visible in a checkout. 3 is minutes. 4 is
the largest and the most valuable to a newcomer. 5 after the interpretation
is settled, with the export bug fixed immediately either way.

### What still needs a real machine

Everything above is covered by tests, but two of these bugs existed
*because* a checkout cannot see them, and the same is true of their fixes:

- **Install the packaged build and run `introspect` on a C# repository.**
  The published-extractor path, the read-only install directory, and the
  runtime-not-SDK claim are all only truly exercised there. The release
  smoke test pipes a fixture through the staged extractor, which catches a
  missing payload but not a Windows-install quirk.
- **Point the installed app at a repository with no workspace**, take the
  offer, and follow the prompt through to a model — the 5-minute-stranger
  run, which is now a much shorter path than it was.

## Canvas findings (2026-08-26, second tester round)

Two reports from using 0.6.2 on real models, both reproduced before fixing.

**Dense diagrams put nodes on top of each other.** Not a layout-engine
problem: ELK's own geometry had zero overlaps. `layout.js` reserves a
per-kind size *estimate* while a `.node` is content-sized, so a wrapped name
renders taller than reserved and the overflow ran into the row below —
measured at 79–100px against a 76px reservation on a 16-component view, 15 of
16 nodes over. Small diagrams hid it in the 80px inter-layer gap; full ones
did not. The canvas now measures with the real markup and stylesheet before
laying out, and the gap comes back to a full 80px at every height.

**"Elements land top-left and are hard to move."** The drag handler clamped
pins with `Math.max(0, …)`, so nothing could be dragged above or left of the
origin — the corner was a wall to pile things against. Pins may now be
negative; layout reframes around whatever is drawn and reports the
translation, so what reaches the YAML stays in model coordinates. This is the
"infinite canvas" ask: the canvas already fits and centres content, so the
constraint was the clamp rather than any extent.

**The agent surfaces were split** (owner asked whether to; the answer was yes,
but not into three equal parts). Reference and workflow are different shapes:
a *skill* auto-triggers when architecture comes up, which is exactly why it
must not interview — that is why the primer never asked anything. So the
workflows became **commands**, which are user-initiated and may therefore ask:
`/blastradius:model` (interview, then build), `/blastradius:sync`,
`/blastradius:review`. A read-only **subagent**, `blastradius-surveyor`, takes
the read-the-whole-repository pass, where a separate context window genuinely
pays.

`/blastradius:model` branches on what it finds, per the owner's point that the
two cases differ: with code present it surveys first and brings a *proposal* to
correct, because corrections are cheaper than answers; on an empty repository
there is nothing to read, so it interviews properly. The topics asked are the
ones named — scope, level of detail, documents and ADRs, `source:` mappings for
code-level detail, deployment.

**Corrected within the hour, after the owner pushed back**: the first cut of
this claimed only Claude Code had a command or subagent surface. That was
wrong for all three others, and asserted from memory rather than checked.
Copilot has prompt files (`.github/prompts/*.prompt.md`) and custom agents
(`.github/agents/*.agent.md`); Cursor and Codex both discover
`.agents/skills/*/SKILL.md` and invoke a skill by name. Cursor and Copilot
even read `.claude/` directly. Every agent now gets the workflows in its own
format, with the paths and frontmatter keys taken from each vendor's
documentation — a file at the wrong path with the wrong extension does
nothing and says nothing about it, which is the same silent-failure class as
the packaging bugs above.

What stays one document is the *reference*: it is what an agent reads before
doing anything, and splitting it only makes half of it easy to miss. It is now
also *our* file wherever the agent allows one — the owner's follow-up point:
Copilot takes `.github/instructions/blastradius.instructions.md` (`applyTo:
'**'`, so it stays always-on) instead of an append into
`copilot-instructions.md`, which belongs to the project. Uninstalling is then
deleting our files rather than editing theirs. Codex still appends to
`AGENTS.md`, having no other per-repo instructions file; a repo set up by the
earlier appending version is detected and left alone rather than told the same
thing twice.

## 0.6.2 — released (2026-08-26): L4 works on an installed build

**Cut 2026-08-26.** L4 introspection was unusable on every packaged build
ever published — not degraded, unusable: the extractor could not be loaded
from inside the package at all. Details below.

Three fix releases in three days, all for things a checkout cannot see. That
is the finding, more than any individual bug.

## L4 on a packaged install (2026-08-26) — three bugs, found by installing it

The first run of `introspect` from a real Store install, on a real C# repo.
Nothing about it worked, and none of it was visible from a checkout.

1. **The extractor could not be loaded at all.** `WindowsApps` ACLs let an
   outside process read a file but not load it as an assembly, and the C#
   extractor runs in `dotnet.exe`, which is not part of our package:
   `Could not load file or assembly ... Access is denied.` Shipping it
   *published* (0.6.1) was necessary and not sufficient. Core now keeps a
   private copy under `%LOCALAPPDATA%` and runs that, once per version.
2. **`repoRoot` was sent in verbatim form.** `canonicalize()` yields
   `\\?\C:\...` on Windows, which takes no separator normalization; the
   extractors join it with forward slashes and Windows rejects the result.
   This means **C# introspection had never worked with an absolute root on
   Windows** — hidden because the dogfood corpus has no C# mapping and the
   fixture gate passed a relative root. The gate now runs both.
3. **`source:` on a container vanished silently.** Introspection is
   component-level; the key was simply never read, and YAML ignores what it
   does not know. It is now a warning naming the container.

Reported by the owner running the published build with an agent, which
diagnosed (1) correctly and unaided, and declined to work around it by
copying the DLL out — the right call.

**The pattern is now unmistakable.** Three releases in a row have shipped a
bug that only exists in an installed layout: extractors missing from the
package (0.6.0), the scaffold refusing to touch a repo with a README
(0.6.1), and these. CI builds the package and never installs it. Until
something exercises an installed build against a real repository, this will
keep happening — a candidate for 0.7.0 that outranks anything currently in
the pool.

## 0.6.1 — released (2026-08-26): 0.6.0's onboarding, working

**Cut 2026-08-26**, a fix release. 0.6.0's headline change — pointing the app
at your own repository and being offered a model — failed on the first real
repository it met, and would have failed on nearly all of them. A release that
does not work on a normal repo is not much of a release, so this went out
immediately rather than waiting for a theme.

Details in the section below; the short version is one flaw in two surfaces
(an existing file was fatal, and the starter set contains `README.md`), plus
the two things the same session asked for: choosing which agent pieces to set
up, and being asked where the workspace should go.

## Second-user findings (2026-08-26) — fixed the same day

The owner installed 0.6.0 and pointed it at a real repository. The onboarding
offer — 0.6.0's headline fix — **failed on the first try**, and would have
failed on essentially every repository.

`scaffold::starter_workspace` includes a `README.md`, and both surfaces
treated a pre-existing file as fatal. So: the app's "Start a model here"
returned `README.md: exists — refusing to overwrite`, which left its dialog
open having written nothing and, because the agent setup runs *after*
scaffolding, silently skipped the MCP and skill files as well. `blastradius
init .` had the same flaw and was worse — it wrote four files, printed the
error, exited 2, and skipped the agents, leaving a half-initialised repo.

An existing file is not a conflict; it is the user's file. Both surfaces now
share `scaffold::scaffold_into`, which keeps what is there, creates the rest,
and reports both. A skipped README costs nothing — it is a pointer, not part
of the model, and the workspace validates without it.

**Reproduced before fixing**, since the mock harness cannot see this: the app
was driven over CDP (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=
--remote-debugging-port=9222`) against a throwaway repo, which reproduced the
stuck dialog exactly, and then confirmed the fix on the same repo with the
README byte-identical afterwards. Worth remembering as the technique for any
Tauri-only bug: the e2e suite runs against the mock bridge and is blind to
everything on the IPC boundary.

Also delivered, owner request from the same run: the offer now lets you choose
**which pieces and which agents** — MCP server, skills and instructions, per
agent — instead of one all-or-nothing checkbox, matching what `blastradius
init` has always offered. A drift test asserts the ids the dialog sends are
the ones `core::onboard` knows how to write.

**Decided the same day** (owner): both surfaces now **ask** where the
workspace goes and **recommend `docs/`** — or whichever of `docs/` and `doc/`
the project already keeps documentation in, so we never create a
near-duplicate of a folder that is already there. A repository root is for
source; the model is documentation, and this repository keeps its own in
`docs/`. It stays a recommendation: `.` is always a valid answer, and the
CLI's scripted path (`--into` absent, no TTY) still writes to the folder
given, so nobody's existing automation silently relocates its workspace.
Locations are validated — relative, no `..`, no absolute paths — and the
starter model is named after the project rather than the folder it lands in.

## 0.8.0 — released (2026-08-28)

Owner testing of the 0.7.0 build, plus a question about the onboarding
hand-off that turned out to be a real drift — and, from a request that looked
like a one-liner, descriptions on the box: a new view-file key, a new
operation, and the app's first context menu. A minor bump rather than the
0.7.1 this was planned as, because a release that adds a model-format key is
not a patch.

### Descriptions belong on the box, not only in the inspector

Owner request: *"I would like to have the option to add a description to an
element. This is typically shown at the bottom part of the element box."*

The field already existed and was already parsed, searchable and shown in the
inspector — but read-only there, and no diagram had ever drawn it. So there
were two gaps, not one: no way to write a description from the app, and no way
to see it on the picture. The inspector gained an editable field routed
through the `set-field` operation that already whitelisted `description`.

The drawing half needed a decision about **where the choice lives**. C4 puts
the description in the box by default, but nearly every element in a real
workspace already has one, so drawing them all would have made every existing
diagram taller the moment this shipped — the same argument that made groups
opt-in (spec §3c). Owner's call, from three options: a per-element toggle on
the box, stored per view. So `descriptions: [core]` in the view file, beside
`layout:`, resolved with the same scope-relative keys, written by the same
machinery — including authoring the view file when the level+scope has none,
which is most of them. The same container can be a bare name in the L2
overview and carry its paragraph in the L3 view that is about it.

Right-click is a new surface: the app had no context menu at all. It carries
one item, because there is exactly one thing about a box that belongs to the
*diagram* rather than to the element. A box with no description yet is offered
the inspector field instead — there is nothing to show until there is
something to say.

The cost is that a described box is **taller**, and four surfaces had to agree
about how much taller. The canvas measures the real markup (`measureNodes`);
the SVG export, the exported viewer and the layout estimate share one wrap in
`ui/js/labels.js`. A nested deployment container is the one case that cannot
grow — ELK sizes it from its children — so it asks for the room as bottom
padding instead, wrapped at the leaf width, which over-reserves rather than
letting the text land on what is inside.

### A nested container was sat on by what it held

Noticed while looking at the deployment view during the description work, and
confirmed against a pristine worktree at `6419db5`: it was already broken, and
had been since containment shipped (ADR-0018).

`NEST_PAD.top` was a constant, 52px, sized for a container's kicker and name.
But `[DEPLOYMENT NODE: POWERSHELL]` wraps to two lines in a narrow box, and
`elk.padding` is what stands between a container's own chrome and its first
child — so the Terminal container's name rendered underneath the CLI instance
inside it. The meta line had the matching problem at the other end: it is
pushed to the bottom of a nested box by `margin-top: auto`, and the 16px of
bottom padding did not hold it.

Two causes, both the same shape as the description problem next door — a
constant standing in for something that is content-sized:

- The padding is now the container's **measured** chrome, from the same probe
  that measures leaf heights (`measureNodes` returns the breakdown alongside
  the height); the headless path keeps an estimate, generously rounded.
- ELK sizes a compound from its children alone, so a container holding one
  small box came out narrower than its own title line — which is what made the
  kicker wrap. A compound now carries `elk.nodeSize.minimum` of the box it
  would be on its own.

Pinned by an e2e test that walks every nested container and asserts none of
its own spans intersects any descendant's box. It reports eight overlaps
against the old constant.

### The hand-off had drifted past its own workflows

Asked whether the suggested prompt was still correct. It was not, and nothing
pinned it — the only assertion was that the dialog contained the phrase "model
its architecture". It told the agent to read the repository and model it
through `apply_operations`, which is exactly what `/blastradius:model` exists
to replace: **being asked what to cover first is the point**, and the hand-off
walked straight past it. It also assumed MCP was registered, though MCP and
skills are independent checkboxes, so a skills-only setup was handed a prompt
naming tools that did not exist.

The prompt is now built from what was actually selected, and both surfaces —
the app dialog and `blastradius init` — additionally list all three workflows
and how to start each one in each agent chosen. `sync` and `review` were being
written to disk and never mentioned to the person who had just installed them
(owner's point). `workflows::CATALOGUE` and `workflows::invocation` are the
single source for which workflows exist and how each agent starts them, and a
test derives the file each quoted invocation implies and asserts `files_for`
really writes it: a wrong invocation is worse than none, because it sends
someone to a command that does not exist.

### Dragging a node at L1 could not work

Reported from the released build: moving an L1 element warned "cannot pin at
L1 without a scope element" and left the node detached from its own relations.

Two faults. Pinning writes a view file when none exists, and it derived the
file name and the `scope:` key from a scope element — but **L1 has no scope**;
its subject is the whole model, exactly like the deployment overview. So in
any workspace without an L1 view file, which is most of them including this
one, dragging at L1 failed outright. L1 now writes a scope-less
`views/context.yaml` with absolute pin keys, the way `LD` already did, and
`views.rs` accepts a scope-less L1 view.

The second fault is worse than the first, because it was not specific to L1: a
refused operation left the canvas lying. A drag moves the node's own
`style.left/top` for feedback while its edges stay where layout left them, and
`applyOp` toasted the error and returned without re-rendering — so *any*
rejected edit left a node parked away from its relations until something else
forced a render. It now puts the canvas back.

### Moving one node moved all of them

Owner report: pinning one element made the rest jump. Measured before agreeing,
and it was worse than described — on this repository's own L3 view, dragging
one component moved **all eight** others, by 325-425px each. That is not a
layout settling, it is the diagram rearranging itself under your hands.

The cause is structural: a pinned node leaves the ELK graph, so what remains is
a different graph — different nodes, and the edges touching the pinned one
dropped too — and ELK lays it out afresh. ADR-0006 anticipated needing
stability here and specified soft interactive hints; that half was never built.

The owner's suggestion was to pin everything once you pin one, and it is the
right call, so that is what the first drag in a view now does: the dragged node
where you put it, every other node exactly where it already was. Nothing but
the dragged node appears to move. It goes through `apply_operations` as **one
transaction**, so a single undo returns the whole view to auto-layout — and
because it only pins what is not already pinned, later drags send one
operation. New elements added afterwards are still auto-placed, in clear space
rather than reshuffling what you arranged.

The residue is grid rounding and nothing else: settling rounds each node to the
26px grid, so a node can shift up to 14px (measured worst case). The e2e
asserts the *arrangement* — pairwise distances in model space — rather than
screen positions, because a drop that extends the drawing re-fits the camera
and slides and shrinks everything without rearranging anything. Getting that
measurement right took three attempts, each of which looked like a real
regression and was not.

### The dotted canvas is endless again

Reported alongside it: the model looked like it sat in a corner of the canvas
rather than on an infinite sheet. The grid was painted on `.canvas-camera`,
which is viewport-sized and *translated* — so the dotted rectangle slid away
with the drawing and ran out. It now lives on `.canvas`, which fills the pane
and never moves, with `background-size` and `background-position` driven from
the camera so it still pans and zooms with the model and the dot at model
(0,0) stays under the model's origin. Below half scale the pitch steps up to
four grid units, which the design system had always described and nothing had
ever implemented. The exported viewer does the same, or a shared file would
disagree with the app.

### `tools/sync-ds.py` was a landmine

Found while making that change. `design-system/` is documented as the source
of truth and `ui/ds/` as generated — but `ui/ds/` had been edited directly and
was **ahead**: deployment node styles, group boundaries, `--group-border`,
`--group-fill` and `--z-group` existed only there. The script deletes the
destination before copying, so running it silently removed shipped styles and
broke the headless renderer, which reads tokens out of `ui/ds/`.

Both halves fixed: the drifted rules are reconciled into `design-system/`, so
the two agree again, and the script now **refuses** rather than clobbering —
before overwriting any CSS file it checks that every selector and custom
property already in the destination still exists in the source, names what
would be lost, and writes nothing.

## 0.7.0 — released (2026-08-28)

**Cut 2026-08-28.** Store submission `1152921505701761076` reached
`PreProcessing`; portable archives for Windows and Linux are on the [GitHub
Release](https://github.com/kommundsen/blastradius/releases/tag/v0.7.0).

Picked from the pool below plus the structural finding
above it, in that order: the install-shaped hole in CI first, because three
consecutive releases went out through it, then the two things a user actually
feels, then the recorded debts.

The through-line is that four of these were only findable by running the
thing rather than reading it — the install-only bugs, the exported viewer's
missing altitude, the layout that fought a pin, and a 2400px column that was
technically correct. So the release ships two gates that did not exist:
something that runs an *installed* build (`installed`, every push), and
something that opens the *exported file* (`ui/tests/export/`, in the job that
already built it).

### Something finally exercises an installed build

The structural finding of the week, and it outranked everything in the pool.
Three releases in a row shipped a bug that exists **only** in an installed
layout — extractors missing from the package (0.6.0), the scaffold refusing a
repository that already had a README (0.6.1), the C# extractor unloadable from
WindowsApps (0.6.2). CI built the package every time and never ran anything
out of it, so no checkout could see any of them.

`tools/smoke-install.ps1` takes a finished CLI — a staged bundle, or the
MSIX's execution alias via `-Installed` — and runs the flow a new user takes
on a repository it has never seen: the binary runs, the extractors shipped
beside it, `init` on a repository that already has files keeps them, what it
wrote validates, and both out-of-process extractors run, the C# one with an
absolute repository root. That is one assertion per shipped bug.

`-ReadOnly` denies write on the bundle first, which is what makes core stage
the C# extractor into `%LOCALAPPDATA%` rather than run it in place — the
0.6.2 path, reachable without an MSIX at all, since what WindowsApps actually
does is permit reading the DLL while refusing to load it as an assembly. The
run asserts the staged copy exists afterwards, so the path is proved rather
than merely traversed. Checked negatively too: hiding the extractor from the
bundle fails the gate at step 2 with the 0.6.0 message.

A new `installed` job runs both passes on every push, and the release workflow
runs them on the real staged archive before it is published, replacing an
inline smoke that only checked the C# extractor.

**Its own first CI run caught it out**, which is the most encouraging thing
about it. Read-only was a Deny ACE, and `AddAccessRule` appends rather than
canonicalising — so on an elevated account (a runner) the inherited
Administrators Allow is evaluated first and the Deny never bites. Every step
passed and then the staging assertion failed, correctly, because the bundle
had never been unwritable. It is now a protected DACL granting read and
execute, and the script probes the directory the same way core does before
trusting its own setup. A gate that cannot tell you its precondition failed
is a gate you will eventually believe by mistake.

**Open question, deliberately not closed for the release — and much narrower
than it first looked.** With the DACL fixed, the `installed` job reports the
extractor directory as unwritable, C# introspection *succeeds* out of it, and
core never stages into `%LOCALAPPDATA%`. The first reading was "desktop stages,
runner does not". The release run disproved that: the `portable` job ran the
same script, on the same runner image, against the real staged archive, and
logged `extractor staged out of the read-only bundle`.

So it is not a runner-versus-desktop difference at all. It is a difference
between two jobs on the same image — `installed` stages a bundle by hand into
`bundle/`, `portable` stages one through `tools/stage-portable.mjs` into
`dist/<name>/` — and only one of them makes `writable()` say no. That is a
far more tractable question than the one it started as, and whoever picks it
up has both logs.

The staging check is therefore **reported, not enforced**: what protects a
user is step 6 — C# introspection working from a read-only install — and that
is asserted hard. Whether it got there by staging is this implementation's
answer, not the contract, and failing a build over a mechanism nobody has
explained is asserting a guess. The diagnostics print on every read-only run,
so the next person to look has the evidence rather than three rounds of
theorising. Worth returning to: if `writable()` can be wrong about a
directory, the 0.6.2 fix rests on it being right.

**Shipped anyway, and correctly**: every leg of the release ran the smoke
against the artifact it actually publishes, and the Windows portable archive
staged the extractor exactly as designed.

**Found while writing it**: `blastradius init --help` scaffolded a workspace
into a folder literally called `--help`, and any mistyped flag scaffolded one
named after the typo — the argument loop treated every unrecognised token as a
directory name. For a command that creates files that is the wrong default.
An unknown option is now an error, `--help` prints usage and writes nothing.
Also: `resolve_root` handed back Windows verbatim paths (`\\?\C:\...`), which
leaked into error messages and into everything derived from them. The
extractors learned that in 0.6.2; the rest now strips it at the boundary.

### Layout: pins stop relocating the diagram, and long chains stop towering

Two defects with the same root — layout rules that were fine when diagrams
were small.

**A pin was a divider, not a constraint.** Unpinned nodes were laid out as one
block and offset to start *below* the pinned bounding box, so pinning a single
node near the bottom of a diagram shoved every other node underneath it. It
now takes the least displacement that clears the pinned boxes — leave it where
ELK put it, or push below, right, above or left — with "below" kept as the
guaranteed fallback, which is what the old rule was. Deterministic: fixed
candidate order, tie-break on displacement. On this repository's own L2 view
the diagram is 138px shorter for it, because the auto block no longer has to
start under the pins.

**Sixteen chained components rendered as a 2400px column.** Correct and
unreadable at once. ELK's `wrapping.strategy` snakes a long chain into shorter
columns; it now runs as a second pass, taken only when the first result comes
back more than 2.5× taller than wide with at least eight nodes, and kept only
if it is actually squarer. Small diagrams still read straight down, which is
the C4 convention and worth keeping — none of this repository's own views
changes. The 16-node chain goes from 162×2428 to 735×561.

### Finding things in the model

The pool ranked this first "on a hunch, not on evidence". The evidence turned
up on the way: a 16-component container is a scroll before it is anything
else, and an agent has had `find_elements` since 0.5.0 while a human in the
app had the sidebar tree and nothing else.

`Ctrl`/`Cmd`+`K`, or the **Find** button. It searches elements (name, id,
description), **relations** (label or either end — an edge has no row in the
tree, so this is the only way to look one up), documents, and derived L4 code
elements, ranked exact → name-prefix → id-prefix → substring, with a fixed
tie-break so the same query always lists the same order. Enter flies the
camera to whatever altitude the result lives at. Ranking is a pure module
(`ui/js/search.js`) tested in node; the palette itself is pinned by a WebKit
suite.

### The exported viewer can browse code level, and is finally tested at all

`ui/js/viewer.js` had no derived handling: its node classes and kind labels
knew only authored kinds, so an exported page silently dropped a whole
altitude. Recorded as a debt in spec/l4-introspection.md when introspection
shipped in 0.3.0, and carried for four releases.

It now mirrors the app: same node classes, dive from a component into its
modules and from a module into its types, derived rows in the tree, breadcrumb
trail walked through `parent` rather than by splitting ids (a derived id may
itself contain dots), and an inspector that names the file and line but
offers nothing to click — an export has no machine to open a file on. The L4
segment is live only when the snapshot carries derived facts, the rule the
deployment segment already followed.

**Why it survived four releases is the more useful finding.** Nothing tested
the exported file. The whole e2e suite runs the *app* against the mock bridge;
the export is a different artifact — the same modules concatenated into one
classic script with no imports and no IPC — and CI built it, uploaded it, and
never opened it. `ui/tests/export/` now opens `architecture.html` from
`file://` in WebKit and walks L4 and deployment through it, in the job that
already builds the file. Deployment, checked at the same time, turned out to
have been working all along.

### Codex stops being the exception

The last file we wrote into that was not ours. Every other agent got its own
removable file in 0.6.3; Codex kept an 80-line append into `AGENTS.md`,
because `AGENTS.md` is the only per-repo instructions file it reads and the
reference has to auto-load.

Split the two jobs. The reference lives in `.agents/blastradius.md`, ours to
write and ours to delete, beside the `.agents/skills/` the workflows already
use. `AGENTS.md` gets the part that genuinely must auto-load: eleven lines
between `<!-- blastradius:begin -->` markers, saying what the model is and
which file to read. Delimited so a re-run updates the block rather than
stacking another, and so removing us is deleting between two markers rather
than guessing where our text ended.

A repository set up by 0.6.x has the whole primer pasted in unmarked, and is
**left exactly as it is** — it still says the right things, and rewriting
somebody's `AGENTS.md` to tidy our own history is not our call. Pinned by a
test. This repository's own `AGENTS.md` is converted by hand, since it is the
one repo where the old shape was ours to change.

### C# semantic mode names dependencies by assembly

A recorded follow-up since 0.4.0. Dependency rollups came from the
using-directive scan in both modes, so `using Newtonsoft.Json` produced
`dep.Newtonsoft` — a namespace-root proxy for a package name — and the edge
was owned by the file's namespace rather than the type, because a using is
file-scoped and per-type attribution would have invented precision.

Semantic mode has resolved symbols, so neither compromise is needed: a
dependency is a **cross-assembly** reference to something outside the corpus,
named by the assembly (`dep.Newtonsoft.Json`) and attributed to the type that
actually makes it. A reference into the same assembly is your own code that
the mapping does not cover, and calling that a dependency would be a lie. The
using-directive scan now runs only when semantic mode did not, since both
would report one dependency twice under two ids.

Gated without adding a NuGet package: check 2b maps only `Beta/` of the
existing semantic fixture, which puts `Alpha` outside the corpus while leaving
it a real separate assembly. Syntax mode gets that case wrong twice — the
global using sits in a file declaring no types, so no dependency is recorded
at all, and name matching resolves the reference to the in-corpus
`Beta.Widget`, which is the wrong `Widget`. Extractor bumped to 0.4.0 and the
syntax fixture refrozen; syntax-mode facts are otherwise byte-identical.

### The deployment follow-ups (ADR-0018)

**`replicas`, a field rather than repeated elements.** Three identical app
servers are one box marked ×3. Giving each copy an id would put three of
everything in every relation touching them and tell the reader nothing the
count does not. It reads on nodes and on instances, shows on the node's meta
line and in the inspector, and is drawn by all three renderers from one helper
in `labels.js` so they cannot disagree. `1` is the default and never drawn;
`0` is an error, because an element that runs none of itself is one to delete
and a zero is far likelier to be a mistake than a statement. Deliberately
**not** added to this repository's own deployment model: nothing here is
replicated, and dogfooding a fact that is not true would be worse than not
dogfooding it — it is covered by fixtures instead.

**Nested-box rendering, opt-in per view.** `nested: true` on an `LD` view
draws the scope's whole subtree in one frame instead of one altitude at a
time. Everywhere else the answer to "what is inside this" is to dive, and two
ways of saying it would be two things to learn — deployment is the one place
where C4's convention is containment and a reader may genuinely want the
physical picture at once. On any other level the key is ignored *with a
warning*, not silently.

Substantially cheaper than when it was deferred, exactly as the pool
predicted: 0.5.0's grouped elements had already built compound ELK layout,
the absolutising walk, and the draw order. What was new is that a container
here is a **node**, not a boundary — it keeps its kicker, its name, its
inspector and its dive, and is merely large enough to hold its members. A
`group:` box has no identity; this does. Two consequences fall out and are
handled: a container is a region rather than an obstacle for edge routing (an
edge into it crosses it by definition), and it contributes only its label
strip to label de-collision.

Dogfooded on a new `dev-machine` view, which says the thing the dive-based
overview cannot: all three containers run in **one process on one
workstation**. The overview above it deliberately stays dive-based, so the
repo exercises both.

**Structurizr deployment import.** `deploymentEnvironment` and
`deploymentNode` were tokenised and thrown away through 0.6.x: a DSL that says
where its containers run is telling you something the logical model cannot,
and dropping it silently was the worst of both. Environments, nodes nested to
any depth, `infrastructureNode`, `containerInstance` and the relations between
them now all land in `model/deployment.yaml`, with Structurizr's trailing
instance count becoming `replicas`. An infrastructure node imports as an
ordinary deployment node — it is a thing other things run on, and a fourth
kind for a naming difference would be a schema change with no reader benefit.
`softwareSystemInstance` stays the honest gap and is reported, not folded.

Caught by the corpus on the way: the deployment skip arm was reusing the model
block's "skip an unknown keyword and its bare-word argument" rule, which in a
deployment block swallowed the `deploymentNode` that follows a `tags` line.
`aws-s3-upload.dsl` went red; the corpus is back at 10/10, and a test now
asserts every corpus DSL declaring an environment produces a deployment file.

## 0.9.0 — released (2026-08-29): the modelling experience

**Cut 2026-08-29.** All five picked items shipped — A, B, C, D and F — and the
release is one theme rather than five: the app can now *write* the model it has
always been able to draw. Before it, half of the format this project documents
was reachable only by opening YAML, one drag made a view permanently manual with
no way back, the operations the canvas owned were bound to keystrokes nothing
advertised, and the one feature that checks the model against reality reported
its findings as strings in a chip.

Three things fell out that were older than the work and are worth keeping
together, because they are the same shape: **a documented behaviour nothing
exercised**. `external: true` on a system could never be loaded. A system's
`group:` was read as `None` and drew nothing. And 0.8.0's settle test never
dragged anything, for two independent reasons — so the feature it guarded was
real and its proof was not. Each was found by writing a test for something
else.

Version bumped across the three surfaces (`Cargo.toml`, `tauri.conf.json`,
`packaging/msix/Package.appxmanifest`, which the release workflow cross-checks
against the tag) and tagged `v0.9.0`, which drives the Store submission and the
portable archives.

**Carried, unpicked from this release's own pool**: E (general relation repair —
`direction` as a writable field, re-pointing an endpoint, add-a-relation from
the inspector), G (documents and ADRs from inside the app), and H (moving an
element, which needs ADR-0003 amended rather than ignored). Carried from
further back: the PRD five-minute-stranger run, macOS distribution, and the
`writable()` open question from 0.7.0.

Written after an audit of what the app can actually *write*, rather
than from the wish list: every item below names the code that proves the gap.

The through-line is that Phase 3 shipped editing as a set of operations, and
the surfaces on top of them stopped growing at the point where the operations
did. `sync::Operation` has nine variants; the canvas reaches six of them, the
inspector two fields, and the parts of the model format the product documents
in `spec/model-format.md` §3–§4 are, for the most part, only writable by hand.
A user who follows our own onboarding is told to model in the app, and then
has to open the YAML to do half of it.

**Picked 2026-08-28** (owner): all four of A–D, plus **F** as the theme. Five
items, but four of them are surface over operations that already exist, and
the two smallest are traps this project shipped itself. Sequence: **A → B → C
→ D → F**, which is the dependency order — B needs A's unpin to have something
to offer, D shows toggles for the keys C teaches the app to write, and F's
second remedy leans on the relation work E would generalise. **E was not
picked**, so F gets the narrow version of it: reverse an unbacked relation,
without the general endpoint-repair surface.

Draft exits, one per item:

- **A** — a view pinned by a single drag returns to auto-layout in one action
  and one undo, and the view file is left with no `layout:` key rather than an
  empty one; e2e asserts the arrangement matches a never-pinned view.
- **B** — every operation the canvas can perform is reachable from the box
  without knowing a keystroke; the `R` binding stays, and a test derives the
  menu's items from `sync::Operation` so a new op cannot be added without
  deciding whether the box offers it.
- **C** — a workspace gains a group, a `replicas` count, a `tech` and a
  `source:` mapping without the YAML being opened, and the mapping's save runs
  `introspect` and lands a real derived graph.
- **D** — the groups written in C become visible from the view panel, on a view
  that had no file until the toggle was flipped.
- **F** — a seeded undeclared dependency in this repository draws a ghost edge
  on the canvas and is declared with one click, leaving the model drift-free
  under `validate --strict-drift`; a seeded backwards relation is reversed the
  same way. The dogfood model itself must end the release drift-free, as
  `conformance.rs` already asserts.

### A — shipped 2026-08-29: unpin, and a view back to auto-layout

`Unpin` is a new `sync::Operation`: one element, or — with `id` omitted —
every pin in the view, which is the way back to auto-layout. It is deliberately
not pin's mirror image in two ways. It **never authors a view file**: nothing is
pinned in a view that has no file, so the answer is no changes rather than a
new file saying nothing, and `apply` already treats an empty change set as a
non-edit that leaves no undo entry behind. And the **last pin takes `layout:`
with it** rather than leaving the key standing over nothing — the rule
`descriptions:` has followed since 0.8.0. Releasing a whole view is one
operation and therefore one undo, not one per node.

Sharing one view lookup between pinning and unpinning fixed a small thing on
the way: the staleness guard's own copy of it compared a deployment overview's
empty `scope:` against the canvas's `None` and matched neither, so a stale
overview file was never the reason an operation was refused. It is now.

Both halves are on the diagram, because that is where the pinning happened:
the box's menu releases the box, and the canvas's own menu — right-click on
empty space, a surface that did not exist — releases the view, counting what it
is about to release. Agents get the variant too, so the schema branch and the
enum stay in step (`tests/mcp.rs` asserts the count).

*Exit met*: `unpinning_everything_removes_the_layout_key` asserts the whole
file byte-for-byte after the reset — comments and `include-context:` intact,
no `layout:` — and that one undo restores it exactly; the e2e drags a node on
this repo's own L3 view, takes **Back to auto-layout**, and asserts the
arrangement returns to the never-pinned one by pairwise distance, to within two
pixels rather than the grid cell the settle test has to allow.

**Found on the way, and worth more than the feature: 0.8.0's settle test never
dragged anything — for two independent reasons.** It performed a canvas **pan**,
and a pan moves every node equally, which satisfies both of its assertions (the
dragged node moved; no two other nodes moved relative to each other). It could
not have failed for the reason it exists.

The first reason is the fixture: `canvas.spec.mjs` loads `/index.html`, whose
mock git fixture carries a merge conflict, and a conflicted workspace is
read-only — `canPin()` is false and `beginNodeDrag` returns on the first
pointer event. Proven rather than assumed: on `/index.html` the app carries no
`can-edit` class and the Add button is hidden; on `/index.html?nogit` both flip.

The second only appeared once the first was fixed, and only when the file ran
as a file rather than the test alone: the test navigated by typing into the
command palette and pressing Enter, which reaches the intended L3 view when the
result list is quick enough and stays at **L1** when it is not. L1 has no view
file in the mock, and the mock's `pin` — unlike the engine — cannot author one,
so the pins were dropped on the floor and the drag was a pan again. Both tests
now dive to the view instead of searching for it.

What makes either of them fail now is a guard rather than a comment: after the
drop, the undo button must be enabled. A pan produces no transaction, so a test
that panned instead of dragging says so. With that in place the 0.8.0
behaviour is proven real — only its proof was not. Two lessons worth keeping:
a test that edits must say `?nogit`, and a test that asserts *nothing moved*
has to prove something happened first.

**A — Unpin, and get a view back to auto-layout.** There is no `Unpin`
operation: `sync::Operation` has none, and nothing in `ui/` mentions one. That
was survivable while pinning was per-node and rare (ADR-0006: "pinning is the
exception"), and 0.8.0 ended it — the first drag in a view now pins *every*
node, deliberately, so that nothing else moves. The consequence was not
recorded at the time: one drag converts a view to fully manual, and there is
no way back inside the app except undo while it is still on the stack. Ships:
`unpin` (single) and a view-level "back to auto-layout" as one transaction,
reachable from the box and from the view panel (D); pinned boxes readable as
pinned. Small, and it closes a trap we shipped ourselves.

### B — shipped 2026-08-29: a context menu that can model

Right-clicking a box now offers everything the canvas can do to it: **Connect
to…**, **Rename…**, show or hide its description (or write one), **Add a …
inside…**, release its pin or the whole view's, and **Delete…**. Before this,
drawing a relation was bound to `R` and advertised in no menu, tooltip or
button; delete was the `Delete` key; and the menu itself carried exactly one
item. Every one of those operations already existed — a user who had not read
the shortcuts page simply could not reach them.

Two of the items are new behaviour rather than a new route to old behaviour.
**Add a … inside…** creates a child of the box you clicked rather than of the
current scope, names the kind when the model format allows only one (a
container in a system, a component in a container) and defers to the dialog
when it allows two (a deployment node holds either more nodes or the containers
that run on them) — and then **dives**, because the new element lives one
altitude below the view you are looking at and reporting success into thin air
is not a result. **Rename…** hands over to the inspector's name field the way
"Add a description…" already handed over to the description field: the name is
a model field, and the field is where it is edited.

The rules live in `ui/js/menu.js`, a pure module with no DOM and no state, so
what appears and when is testable in node while `app.js` only binds actions to
ids. That is what makes the exit's gate possible.

*Exit met.* `ui/tests/menu.test.mjs` reads `pub enum Operation` straight out of
`crates/blastradius-core/src/sync.rs`, and asserts every variant is either
offered by the menu in some context or listed in `NOT_ON_THE_BOX` **with a
reason** — plus the converse, that nothing is both offered and excused, and
that no excuse names an operation that no longer exists. Adding a variant to
the engine now fails a test until someone decides whether the diagram offers
it. Four operations are deliberately excused: `pin` (dragging the box *is* the
pin), `set-field` (the inspector edits fields, beside the text), and the two
relation operations (a relation is chosen by clicking the edge, and is not a
box). The e2e side drives connect, rename, add-a-child, and delete with the
mouse alone, and asserts a leaf is not offered children it cannot have.

The menu also grew the things a seven-item menu needs and a one-item menu did
not: separators between what the element *is*, where it *sits*, and removing
it; arrow-key navigation; and a position of its own when raised from the
keyboard, where there is no pointer and 0,0 is the corner of the window rather
than an answer.

**Not on the box, deliberately: derived L4 elements get no menu at all.** They
accept no operations — the code is the source of truth — and a menu of things
that would all be refused is worse than none.

**B — A context menu that can model.** The app's only context menu carries one
item (`openNodeMenu`, 0.8.0) — show/hide description — while the operations
for the rest already exist. Drawing a relation is bound to the `R` key on a
selected element and appears nowhere else in the UI; delete is the `Delete`
key; rename is an inspector field. Every one of those is a modelling action a
user looks for on the box. Ships: connect, rename, delete, pin/unpin (A),
show/hide description, add a child, open the source file — same ops, a surface
that admits they exist. Small.

### C — shipped 2026-08-29: an inspector that writes the whole element

`set-field` accepts `group`, `replicas` and `external` alongside `name`,
`description` and `tech`, and a new `set-source` operation writes a component's
`source:` mapping — which is a mapping with two sequences in it and so could
never have ridden `set-field`. The inspector offers each where the format
allows it: `tech` and `group` on anything, `replicas` on a deployment node or
container instance, *outside your control* on a system, and **Code level** on a
component, with **Run introspection** beside it. So grouping and code-level
detail stop being YAML-only, and `tech` — which every box in the product
renders in brackets — is finally typeable in the product that renders it.

**An emptied field is now removed rather than blanked.** The MCP schema has
promised "empty string removes the field" since 0.6.0 and the inspector's own
comment said the same; neither was true. `set_field` wrote `description: ""`,
which is a description that says nothing rather than no description. Clearing
now removes the key — in a block mapping by dropping the line, and inside a
one-line flow mapping (`db: { name: Database, tech: Postgres }`) by cutting the
field and the comma that joined it, because removing a *line* there would take
the whole element with it. Clearing what is already absent writes nothing and
leaves no undo entry.

Kind rules are checked in the operation rather than left to whole-workspace
validation, so a refusal says "`replicas` says how many of a deployment node or
container instance run, not of a container" instead of surfacing as a parse
error in a file. `replicas: 0` is refused with the reason the parser gives, and
`replicas: 1` and `external: false` clear their keys, because both are the
default and writing them states nothing.

**Introspection is reachable from the app**: `introspect_component` runs the
extractor for one component and writes its facts under `model/derived/`, then
reloads — the same output the CLI writes, so nothing about it is app-only and
hand-authoring stays a first-class route (spec/l4-introspection.md). The
snapshot carries `source` now, which is what lets an editing surface show a
mapping it did not write.

**Two format bugs fell out of writing the tests**, both older than this work:

- **`external: true` on a system could not be loaded at all.** The spec has
  documented it since §3 and `parse_system` has always read it — but the
  file-shape check treated the *presence* of an `external:` key as "this is a
  context file", so a system file carrying the flag was rejected as trying to
  be both. It is a context *section* only when it is a mapping of elements; as
  a scalar it is the flag. Nothing in this repository used it, which is why
  four releases of drift detection and a dogfood gate never noticed.
- **A system's `group:` was read as `None`, always.** Every other element reads
  `group_of(body)`; `parse_system` hardcoded the field, so a group written on a
  system parsed clean and drew nothing. At L1 its siblings are the people and
  externals, which have always grouped.

*Exit met*: a workspace gains a group, a `replicas` count, a `tech` and a
`source:` mapping without the YAML being opened — asserted in WebKit against
the real inspector, and in Rust against the bytes of the file, including that
the mapping's removal leaves the component byte-identical to before it was ever
mapped. The mapping's **Run introspection** reaches the extractor command; the
mock harness has no compilers, and says so rather than pretending.

**The B gate did its job on the way.** Adding `SetSource` to the engine failed
`menu.test.mjs` — "neither on the box nor listed in NOT_ON_THE_BOX" — which
forced the question rather than letting the box quietly fall behind the model.
The answer: a component with no code behind it is offered *Point at its code…*,
which opens the dialog the way "Add a description…" opens the field, and
`set-source` is listed as something the box starts but the inspector edits.

**Found and fixed alongside**: a latent flake in the description e2e — `#nodes`
is emptied and rebuilt on every edit, so a single `boundingBox()` read could
land on a detached handle. It polls for the measurement now. It had never
failed until the snapshot grew.

**C — An inspector that can write the whole element.** `SetField` whitelists
`name | description | tech` (`sync.rs:513`) and the inspector exposes two of
them. `tech:` is unreachable from the app although every box in the product
renders it — `[Container: Rust]` — since 0.6.0. Worse, four keys have no
operation at all: `group:` (§3c, 0.5.0), `replicas:` (§3b, 0.7.0),
`external: true`, and `source:` — the L4 opt-in. So grouping is reachable only
by hand-editing or by importing a Structurizr workspace that already had it,
and turning on code-level detail means reading `spec/l4-introspection.md` and
writing YAML, which is precisely the step a first-time user is least equipped
for. Ships: the whitelist extended, inspector fields for each, and a `source:`
editor that can run `introspect` when it is saved. Medium, and the highest
ratio of format-we-document to format-you-can-reach.

### D — shipped 2026-08-29: view settings you can see

A third panel mode beside *Inspect* and *Source*. Inspect is about an element,
Source is about a file, and **View** is about the diagram — which had no home at
all: nothing in the app said a view file existed, what was in it, or how to turn
on any of the three flags §4 defines. `descriptions:` had gained a right-click
in 0.8.0 and that was the whole of it.

It holds what the diagram says rather than what the model says: **draw group
boundaries**, **include context**, and — in a deployment view — **nested
boxes**, plus the list of what is pinned and what is drawing a description, each
releasable in place, with **Back to auto-layout** under the pins. When the level
and scope have no view file yet, the first setting changed writes one, the way
pinning has since 0.8.0 — the same `view_file_target`, so the two cannot
disagree about the file name or the header.

`SetViewFlag` follows the rule C established for element fields: **setting a
flag to its default removes the key**. `show-groups: false` and
`include-context: true` say precisely what their absence says, and a file
stating them is a file to keep in step with a default that might move — so
turning a flag off again leaves the file as it was, and turning one *to* its
default in a workspace with no view file writes nothing at all rather than
authoring a file to say nothing. `nested` outside `LD` is refused with the
reason ADR-0018 gives: everywhere else, the answer to "what is inside this" is
to dive.

The panel redraws from `renderCanvas`, not from the selection: its subject is
whatever is on screen, and that changes on a dive, a level button, or an edit —
where the inspector's subject only changes when the selection does. That was a
real bug for exactly one test cycle: switching altitude with the level segment
left the panel describing the view you had left.

*Exit met*: the e2e writes `group: People` onto two L1 elements through the
inspector — where before C there was no way to write one at all — confirms
nothing is drawn, then ticks **Draw group boundaries** in the View panel and
watches the boundary appear. L1 has no view file in this workspace, so the flag
authors one, and the panel stops saying "no view file yet" and starts naming the
file it wrote. Rust-side, four tests pin the file bytes: authoring, removal on
return-to-default, landing in an existing file with its pins and comments
untouched, and the two refusals.

**The B gate fired again**, as designed: `SetViewFlag` failed `menu.test.mjs`
until the box had an answer. It is excused — a flag belongs to the whole
diagram, not to the box you happen to be right-clicking.

**D — View settings you can see.** `show-groups`, `include-context`, `nested`
and `descriptions` are per-view keys (§4); only `descriptions` has an
affordance, and only since 0.8.0. Nothing in the app says a view file exists,
what is in it, or how to turn the rest on — so C's groups would be written and
still invisible. Ships: a view panel — level, scope, the three toggles, the
pin list with per-pin and whole-view release (A), and authoring the file when
the level+scope has none, which the sync engine already does for pins and
descriptions. Small-to-medium, and it is what makes C visible.

**E — Relations you can repair without deleting them.** The relation inspector
edits `label` and `protocol` and offers delete. `direction: both | none` is a
model field (§3, `model.rs`) with no operation. Changing either endpoint means
delete-and-recreate, which loses the label, the protocol and the direction.
And the one relation mistake this project has documented on itself is a
*backwards* relation — `model-service -> sync-engine`, written as a data flow,
caught by drift detection in 0.5.0 — for which the app's answer is: delete it,
find both boxes again, redraw it the other way, retype both fields. Ships:
`direction` on `set-relation-field`, "reverse this relation" as one
transaction, re-point an endpoint, and add-a-relation from the inspector with
the `search.js` ranker doing the picking instead of hunting for a box.

### F — shipped 2026-08-29: drift on the canvas, with the fix one click away

Drift detection found three real problems in this repository's own model the
first time it ran (0.5.0), and then reported them the way it has ever since: as
warning strings in a chip. `drift::detect` returns structure — from, to, kind,
and the file that evidences it — and `drift::diagnose` flattened every one of
them into a sentence. A sentence cannot carry the arguments its own remedy
needs, and the remedy for an undeclared dependency is one `add-relation` call
the app has had since Phase 3.

The findings now ride in the snapshot as structure (`drift`, omitted entirely
when there are none, so every clean snapshot is byte-identical to before). On
the canvas:

- **An undeclared dependency is a ghost edge** — dashed, warning-coloured, and
  deliberately unlike a relation, because it is not one: nothing in the model
  says this yet. Selecting it gives evidence rather than fields — the file the
  reference was found in, openable — and one button, **Declare this relation**,
  which asks for a label and writes it at the ids the *finding* carries rather
  than the ids it happens to be drawn between.
- **An unbacked relation marks the edge it is about**, and the inspector offers
  **Reverse it**: delete and re-add the other way round in one transaction, with
  the label and protocol carried across, so one undo puts it back. That is the
  fix this project's own first run needed — `model-service -> sync-engine` had
  been written as a data flow while the code dependency ran the other way.

Two rendering decisions worth recording. A finding whose endpoints lift into
the *same* box is not drawn: there is no line between a box and itself, and
nothing about that picture is wrong. But a finding whose direction the model
contradicts **is** drawn, alongside the declaration it disagrees with — the
first cut suppressed a ghost whenever any edge joined those two boxes, which
hid exactly the case worth seeing. And drift is not drawn while diffing or
time-travelling: it is a fact about the code as it is now, and a revision's
ghosts would be about a tree nobody has.

The diagnostics list became clickable while we were in there. Every diagnostic
has named a file and a line since Phase 0, and none of them went anywhere.

*Exit met*: `?drift` seeds one finding of each kind — the dogfood model is
drift-free by policy and `conformance.rs` fails the build otherwise, so seeding
is the only honest way to exercise the canvas side — and the e2e declares the
undeclared one, watches the ghost turn into a relation, reverses the unbacked
one, and undoes it in a single step. Six node tests pin the lifting rules, and
a Rust test asserts the snapshot carries the findings whole.

**Deliberately not done**: the exported viewer draws no ghosts. It shares
`computeView`, which takes the findings as an argument that the export does not
pass, so an export is unchanged — an export is a picture of the model, and
whether it should also carry a live claim about code is a question rather than
an oversight. The snapshot it embeds does carry the field, so the answer can be
yes later without a format change.

**F — Drift on the canvas, with the fix one click away.** `drift::detect`
returns structured findings — from, to, kind, and the file that evidences it —
and `drift::diagnose` immediately flattens each one into a warning string
(`drift.rs:135`). The app shows the count as a chip and the strings as a plain
list with no jump and no action (`renderDiagnostics`). The canvas draws
nothing. Yet the remedy for an *undeclared* dependency is one existing
operation (`add-relation`), and the remedy for an *unbacked* one is usually
the reverse action from E. Ships: drift carried structurally into the
snapshot; undeclared dependencies drawn as ghost edges with "declare this";
unbacked relations marked with "reverse" and "delete"; the diagnostics list
made clickable while we are in there. Medium. This is the product thesis —
documentation checked against reality — turned from a CI warning into
something you can act on where you are looking.

**G — Documents and ADRs from inside the app.** The inspector's Documents
section is read-only links. Doc frontmatter (`elements:`) is the link, the
skill teaches "attach ADRs to the elements they govern", and there is no
operation for docs of any kind — so recording a decision means leaving the
app. Ships: "new ADR for this element" scaffolded with correct frontmatter,
and attach/detach an existing doc as a frontmatter splice. Medium: markdown
frontmatter is a new splice surface, though `docs.rs` already parses it.

**H — Move an element (needs an ADR).** Ids are immutable (ADR-0003) and
there is no reparent operation, so when the understanding improves and a
component belongs in a different container, the route is delete and recreate —
losing its relations, its pins, its description, its `descriptions:` entries
and its doc links. Restructuring is the normal course of modelling, and it is
the one thing the product punishes. Ships: a `move` that rewrites the id and
every reference in one transaction, `blast_radius` shown before it runs.
Large, and it needs ADR-0003 amended rather than ignored: a move changes an
element's *path*, and the honest question is whether identity was ever the
path.

**Carried, not picked: E, G and H.** E's general endpoint repair loses to the
narrow reverse F needs; G and H stay in the pool for the release after this
one, H behind the ADR-0003 amendment it requires.

**Not in this release, and deliberately**: the PRD five-minute-stranger run
(still owed, carried), macOS distribution (deferred five times), and the
`writable()` open question from 0.7.0.

## 0.10.0 — planned (2026-08-29): what a user actually meets

0.9.0 made the app able to write the model it can draw; this pool is
about whether that works for someone who is not the person who built it. Three
of the items below are not features at all, which is the honest state of things:
the product has never been measured against its own PRD metric, and its
thesis feature is inert on the language its owner uses most.

**Picked 2026-08-29** (owner): **3, 4 and 9**, plus **2** as the theme. Sequence
**9 → 4 → 3 → 2**, which is cheapest-first and then dependency order: 4 protects
everything written after it, 3 protects the packaged app — and specifically the
extractor path, which is where every install-only bug this project has shipped
lived — and 2 is the one that touches that path.

**1, the five-minute-stranger run, was considered and not picked — a sixth
time.** It is worth saying plainly rather than letting it slide down a list
again: the PRD's own success metric has been owed since 0.4.0, nothing in this
release makes it easier, and the three items picked here are all *proxies* for
it — ways of asking "does this work for someone who did not build it" that can
be answered without finding that someone. They are worth having and they are not
the same answer.

Draft exits:

- **9** — a first-run canvas that says what it can do: the hint names
  right-click and the View panel, not just dive and rise, and an empty or
  freshly-scaffolded workspace says what to do next rather than showing an empty
  frame. Asserted in e2e against the mock's `?emptyfolder` path.
- **4** — one operation list runs through the real engine (Rust) and through the
  e2e mock (JS), and the resulting snapshots are compared field by field. Every
  divergence 0.9.0 introduced by hand — unpin's key removal, `external: false`,
  `replicas: 1`, view authoring — is covered by it, and a new operation that
  only one side implements fails the build.
- **3** — CI drives the *packaged* app over CDP the way 0.6.1 did by hand:
  open a repository with no workspace, take the offer, and reach a rendered
  model, out of an installed layout. `introspect_component` runs there against a
  real repository, which is the only place it has ever been exercised outside a
  mock that answers "needs the real app".
- **2** — a C# fixture where a type in one project references a type in another
  records an `outbound` entry naming the defining file, so `drift::detect`
  reports the same undeclared and unbacked findings it reports for Rust; the
  canvas draws them with no C#-specific code, because there is none to write.

### 9 — shipped 2026-08-29: the canvas says what it can do

The hint read "Double-click to dive · Esc to rise" and had since Phase 1, which
by 0.9.0 was a minority of what the canvas does: renaming, describing, adding a
child, pointing a component at its code and every view setting live behind a
right-click or the View tab, and nothing on screen said so. The help page said
so — one click and one decision away from where the user is looking.

Four surfaces now do:

- **The hint names the menu**: *Double-click to dive · Right-click for actions ·
  Esc to rise*. One string, one place, and the connect-mode hint restores it.
- **A first-run card** on the canvas names the three things worth knowing: the
  box's right-click menu, the View tab beside Inspect, and dive/rise. It is
  dismissed for good — and it also **retires itself the moment the menu it
  teaches is opened**, which is the same evidence and does not cost the user a
  click. Overlay chrome sits on top of the drawing, so the card is
  `pointer-events: none` with `auto` only on its two buttons: a hint must never
  eat a drag aimed at a node beneath it, and there is an e2e test that clicks
  straight through it.
- **A view with nothing in it is a state, not an empty frame.** It says which
  level is empty and what to do about it — right-click, Esc, or the add button
  it offers when editing is allowed. At L4 it says the honest thing instead:
  code elements come from the source, so run introspection.
- **Diving into something with nothing inside says so.** It used to `return`
  silently. On a *scaffolded* model that is exactly what double-clicking the
  starter database does, so the first exploratory gesture of a first run was
  answered with nothing at all.

*Exit met*: eight e2e, including the `?emptyfolder` scaffold path — take the
offer, close the hand-off, and the card is there — and the click-through test
that keeps the card from becoming the bug it is meant to prevent.

**Deliberately not done**: the card is stored per browser profile, not per
workspace (`br.tour-seen` in `localStorage`), so it is shown once per person
rather than once per repository. Someone who dismisses it and later opens a
second workspace does not see it again — which is the right answer for a hint
about the app, and the wrong one for a hint about a model. Nothing here is
about a model.

**1 — The five-minute-stranger run.** The PRD's metric: a platform engineer who
has never seen the product installs it and reaches a rendered model of their own
repository in under five minutes, unassisted and timed. Owed since 0.4.0 and
carried through five releases; 0.5.0 came close (a real outsider tried it and
liked it) and was correctly *not* counted, because none of the conditions were
measured. It is the only item in any pool that measures the user experience
rather than asserting it, and it needs a person and a stopwatch rather than a
commit.

**2 — Drift is blind on C#.** `extractors/dotnet/` records no `outbound`
entries at all — the field exists in the facts schema, the Rust and TypeScript
extractors fill it, and the C# one never has. So the product's central claim,
documentation that cannot quietly rot, is inert on the stack ADR-0016 named
*first* ("language priority is C#/.NET and JavaScript/TypeScript — the user's
stack"). ADR-0019 records the reason honestly: at syntax level C# resolves
namespaces rather than paths, so there is no file to point at. Semantic mode has
resolved symbols and therefore does have one. Nothing in 0.9.0's drift work —
ghost edges, declare, reverse — does anything for a C# user today.

**3 — Nothing has ever run the *app* from an installed layout.**
`tools/smoke-install.ps1` takes a finished *CLI* and puts it through a new
user's flow, which is why 0.7.0 stopped the run of install-only bugs. The app
has no such gate. 0.9.0 added three sync operations and a new IPC command
(`introspect_component`) whose only exercise anywhere is a mock that answers
"introspection needs the real app". Every release that shipped app-side
features without an installed run has shipped a bug that only that layout could
show (0.6.0, 0.6.1, 0.6.2). The technique exists: 0.6.1 drove the packaged app
over CDP (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port`) and
reproduced a Tauri-only bug the e2e suite is structurally blind to.

**4 — The mock harness can lie, and has.** The e2e suite runs against a
hand-written mock of the sync engine (ADR-0011), and every operation's semantics
are mirrored into it by hand. 0.9.0 alone added four such mirrors — unpin
removing the `layout:` key, `external: false` clearing rather than setting,
`replicas: 1` clearing, and a view file being authored when none exists. Each is
a place the suite can agree with itself while disagreeing with the engine, which
is exactly what 0.8.0's settle test did for a whole release. Worth considering:
a contract test that runs one operation list through both the real engine and
the mock and compares the resulting snapshots, so a divergence fails a build
instead of hiding one.

**5 — Relation repair (E, carried).** F shipped *Reverse it* for the one case
drift can prove; everything else about a relation is still delete-and-retype.
`direction: both | none` is a model field with no operation, re-pointing an
endpoint means losing the label and protocol, and adding a relation still means
finding both boxes on the canvas — while `search.js` already ranks every element
for the palette.

**6 — A problems panel.** Validation errors and drift findings now share a chip
that opens a list of strings, clickable since 0.9.0 to open the offending file.
But drift is structured now: a finding knows its two elements, and the canvas
knows how to fly to either. A list that is element-shaped — click a finding, land
on it, fix it there — is the difference between a report and a workflow. It is
also where a C# user would first notice item 2, once there is something to show.

**7 — Documents and ADRs from inside the app (G, carried).** The inspector's
Documents section is read-only links; there is no operation for docs of any
kind, so recording a decision means leaving the app. The skill teaches
"attach ADRs to the elements they govern" and the app cannot do it.

**8 — Move an element (H, carried).** Still the one modelling operation the
product punishes: delete and recreate, losing relations, pins, description,
`descriptions:` entries and doc links. Needs ADR-0003 amended rather than
ignored — a move changes an element's *path*, and the question is whether
identity was ever the path.

**9 — First-run discoverability.** The canvas hint reads "Double-click to dive ·
Esc to rise" and has since Phase 1. Since 0.9.0 the box's right-click menu is
where most editing lives, the View panel is a new tab, and neither announces
itself; the help page says so, which is one click and one decision away from
where the user is. Cheapest item here by a wide margin, and untested against
anyone.

**The shape of the list.** 1 and 3 are the same question asked two ways — does
this work for someone who did not build it — and 2 is the same question asked
about a language. 5, 6 and 9 are what a user feels once it does.

## 0.7.0 — candidate pool (2026-08-25)

**The hold resolved itself.** This pool was 0.6.0's, held (2026-08-25)
because 0.5.0 had reached a first outside user and not one item below was
backed by anyone who had actually used the product — "a single real
reaction is worth more than re-ranking the list". The reaction arrived
first, so 0.6.0 became the reaction and the list moved down a version,
unranked and still unbacked by use. The same caution applies to picking
from it now: the second tester has not run yet.

Held back from 0.5.0 to keep it shippable, not rejected:

- **Finding things in the model** — in-app search or a command palette:
  *(ranked first on a hunch, not on evidence — the hold above is aimed
  squarely at this assumption)*
  jump to any element, doc, or relation by name. Agents already have
  `find_elements` over MCP; a human in the app has nothing at all, which
  bites hardest on the monorepo-scale models this is built for.
- **The rest of the deployment follow-ups** (ADR-0018) — nested-box
  rendering as an optional display mode, now **substantially cheaper than
  when it was deferred**: 0.5.0's grouped elements built the containment
  renderer it needed (compound ELK layout, absolute-ising, draw order), so
  what was the expensive half of ADR-0018 is largely done already; instance multiplicity
  (`replicas`) as a field rather than repeated elements; and importing
  Structurizr's `deploymentEnvironment`/`deploymentNode` blocks, parsed
  and discarded today.
- **Recorded debts** — the exported viewer has no L4 handling at all
  (spec/l4-introspection.md), C# semantic mode could name dependencies by
  assembly rather than by namespace root, and a render-a-view MCP tool
  for agents that need pixels rather than JSON.
- **Carried from 0.6.0, and owed on a real machine** — the two checks no
  checkout can make: `introspect` against a C# repository from the
  *installed* package (the read-only install directory and the
  runtime-not-SDK claim are only truly exercised there), and pointing the
  installed app at a repository with no workspace to follow the offer
  through to a model. Two 0.6.0 bugs existed precisely because a checkout
  cannot see them.
- **macOS distribution** — deferred four times; the $99/year Apple
  Developer ID and a Mac in the loop remain the decision.
- **Going-public launch dressing** — public-audience README,
  CONTRIBUTING, issue templates, sponsor setup.

## v2 themes (not scheduled)

**Bundled in-app help** (idea 2026-08-23, follows dropping Pages
hosting; scope sharpened 2026-08-24): ship a curated *user-facing* doc
set inside the app, rendered by the existing docs-panel machinery
(offline, versioned with the binary, no hosting). The content must be
**feature-usage documentation — how to actually use the app** — and it
does not exist yet; writing it is the bulk of this theme, not the
rendering. Coverage: getting started, navigating/diving the canvas,
editing and pinning, git diff and conflict resolution, L4 introspection
setup (`source:` mappings), export/share, MCP/agent setup, keyboard
shortcuts, model-format reference, privacy policy. Deliberately not the
dogfood ADR/spec site: that is contributor material aimed at *building*
the app, already readable in-app by opening this repo, and stays a CI
artifact — none of it doubles as user help.

Pool pruned 2026-08-24 — shipped since it was written: in-app conflict
resolution (0.2.0), PR-bot diff rendering (0.2.0), headless SVG/PNG
export (0.2.0), L4 source-derived elements (0.3.0); deployment views
moved to 0.4.0. Still parked:

- **Hosted share links** (ADR-0009's payload) — demoted from primary
  commercial thesis by ADR-0017 and hosting is currently unwanted;
  parked, not scheduled.
- **Going-public execution** — the repo itself went public 2026-08-24
  (which also made Actions minutes free, ending the macOS-minutes
  rationing pressure); what remains unscheduled is the launch dressing:
  public-audience README, CONTRIBUTING, issue templates,
  sponsor/donation setup. Considered for 0.4.0 and left unscheduled.
- **Render-a-view MCP tool** — deferred from 0.3.0; revisit when an
  agent task actually needs pixels.
- **macOS/Linux distribution** — deferred at 0.2.0, 0.3.0, and 0.4.0;
  the $99/year Apple Developer ID and Mac-hardware loop remain the
  open decision, Linux undecided.

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
