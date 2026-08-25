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

**Cut 2026-08-25**: all three themes shipped; version bumped across the
three surfaces and tagged `v0.5.0`, which drives both the Store
submission and — new in this release — the portable archives attached to
a GitHub Release.

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

## 0.6.0 — candidate pool (2026-08-25)

Held back from 0.5.0 to keep it shippable, not rejected:

- **Finding things in the model** — in-app search or a command palette:
  jump to any element, doc, or relation by name. Agents already have
  `find_elements` over MCP; a human in the app has nothing at all, which
  bites hardest on the monorepo-scale models this is built for.
- **The rest of the deployment follow-ups** (ADR-0018) — nested-box
  rendering as an optional display mode, which becomes cheap once
  0.5.0's containment renderer exists; instance multiplicity
  (`replicas`) as a field rather than repeated elements; and importing
  Structurizr's `deploymentEnvironment`/`deploymentNode` blocks, parsed
  and discarded today.
- **Recorded debts** — the exported viewer has no L4 handling at all
  (spec/l4-introspection.md), C# semantic mode could name dependencies by
  assembly rather than by namespace root, and a render-a-view MCP tool
  for agents that need pixels rather than JSON.
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
