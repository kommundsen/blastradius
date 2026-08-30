---
doc: release-checklist
type: note
status: accepted
elements: [blastradius, blastradius.shell]
---

# Manual smoke test, before a release

Half an hour, on the packaged app, by a person. Everything in here is something
**the automated gates cannot reach** — that is the entry requirement for a step
being on this list, and anything that becomes automatable should be automated
and struck off rather than checked twice.

## What already runs without you

Do not repeat these; if one is red, stop and fix it rather than smoke testing
around it.

| gate | covers |
| --- | --- |
| `cargo test` | the engine, format preservation, the torture session, drift, conformance against `docs/` |
| `npm run test:node` | layout determinism, routing, rendering, the box menu, the mock/engine contract, the problems rules |
| `npx playwright test` | the whole UI in **WebKit**, plus the a11y audit in both themes and the app bar's size contract |
| `tools/smoke-install.ps1` | the **CLI** from a real install |
| `tools/smoke-app.ps1` | the **packaged app**, over CDP: open an unknown repo, take the offer, render, edit, introspect |
| `blastradius introspect docs --check` | the committed L4 facts match the source |
| `node tools/sync-ds.mjs --check` | `ui/ds` matches the design system |

**The gap those leave, and why this document exists:**

- **The engine is not the one that ships.** The Playwright gate runs WebKit
  (ADR-0011, the constraining engine); users on Windows get WebView2. Every
  rendering and input difference between them is unobserved.
- **The e2e suite never crosses the IPC boundary.** It runs against the mock
  bridge, so a native dialog, an editor launch, a folder pick and a file watcher
  are all beyond it. `smoke-app.ps1` crosses it now, but drives a fixed path.
- **Nobody sees the pixels.** Determinism is asserted; whether a diagram is
  *legible* is not a thing a test knows.
- **macOS and Linux have no runtime test at all** (0.11.0 pool item A).

## Before you start

- [ ] Build or install the **packaged** app, not `cargo tauri dev`. Install-only
      bugs shipped in 0.6.0, 0.6.1 and 0.6.2, and none of them were reproducible
      from a checkout.
- [ ] Confirm the version in the title/About matches the tag you are cutting —
      0.2.0 went to the Store with 0.1.0-era binaries.
- [ ] Have two repositories ready: **this one** (a rich model), and any repo
      that has never seen Blastradius (an empty one is fine).

---

## 1. First run, on a repo that has never seen the product

The path every new user takes, and the one that has broken most often.

- [ ] Launch the app against the unknown repo. It offers to scaffold — it does
      not show a bare welcome screen with no memory of the folder you named
      (0.6.1's dead end, and 0.10.0's fix).
- [ ] Take the offer. A model renders.
- [ ] The first-run card names the right-click menu and the View tab, and is
      gone for good once dismissed.
- [ ] Double-click the scaffolded database. It says there is nothing inside,
      rather than doing nothing at all.
- [ ] Your existing files are untouched — check a README byte-for-byte if the
      repo had one.

## 2. The native boundary the mock cannot see

Everything here is an IPC call the e2e suite answers with a stub.

- [ ] **File → Open (Ctrl+O)** opens the real OS folder picker, and choosing a
      folder loads it.
- [ ] **Open in editor** — from a problems row, or a document — actually
      launches your editor at the right file.
- [ ] **The file watcher**: edit a model file in an external editor and save.
      The canvas re-renders without being touched.
- [ ] **The race**: start an edit in the app, change the same file externally
      before committing it. The app refuses and says so; it does not merge.
- [ ] **Introspection**: point a component at real source, run it, and get code
      elements. This is the command that lived only in a mock until 0.10.0.
- [ ] **Export**: Share → save HTML. Open the saved file in a browser: it
      renders standalone, with no network.

## 3. What only eyes can judge

- [ ] Open this repository's model. Fly L1 → L2 → L3. The camera motion goes
      *into* the box you dived and continues forward; Esc reverses it.
- [ ] Nothing is illegible: no label sitting on a node, no run-on stack of edge
      labels, no edge passing under a box it should route around.
- [ ] Switch the OS theme while the app is open. Both themes are readable and
      nothing keeps a hard-coded colour.
- [ ] Turn on reduced motion in the OS. The dive becomes a cut, not a lurch.
- [ ] Resize the window down to its minimum. Nothing in the app bar becomes
      unreachable — there is a gate for this, but look anyway, because the gate
      only knows about buttons being off-screen.

## 4. Git, on a real repository

- [ ] With uncommitted model changes, the git chip says so.
- [ ] Diff against a base: added, removed and changed elements are all
      distinguishable, and the layout toggle works.
- [ ] History: travel to an older commit and come back. Editing is refused
      while travelling.
- [ ] Manufacture a merge conflict in a model file. The canvas flags it, the
      inspector offers ours/theirs, and resolving leaves a tree your own `git`
      is happy with.

## 5. Packaging, on the artefact you are about to publish

- [ ] Install the MSIX (or unpack the portable archive) **on a machine that has
      never had the app**, or after a full uninstall.
- [ ] Launch from the Start menu, not from a build directory.
- [ ] Extractors are beside the binary and introspection runs. Missing
      `extractors/` was the 0.6.0 bug, and it only shows in a shipping layout.
- [ ] Uninstall, reinstall, and confirm your workspace is untouched.
- [ ] Confirm the shipped build has **no debug port**: the window title is
      plain. A title reading "REMOTE DEBUG PORT OPEN" means a `remote-debug`
      feature build escaped, and that build must never ship.

---

## What changed in this release

Add the release's own items here before testing, and delete them after. The
point is to exercise what is new on the real app, since every gate above was
written against what was already there.

### 0.11.0

- [ ] **Relation repair** — open a relation from an element's inspector row.
      Change its direction; re-point either endpoint by name; check the label,
      protocol and direction all survive. Reverse the arrow and confirm the same.
      Then read the YAML: the relation kept its place in the file and its
      comments.
- [ ] **Add a relation by search** from the inspector, without hunting for the
      other box on the canvas.
- [ ] **Problems panel** — on a model with drift, the chip counts it; the panel
      groups validation and drift separately; a row lands you on the finding;
      Declare and Reverse work from the row; the count follows each repair.
- [ ] **The ⋯ menu** — Open, Theme, Help, Diff and History are all in it and all
      work. Diff and History are absent (not greyed) outside a git repo.

## If you find something

Write it into `docs/roadmap.md` under the release, with what you did and what
happened, before fixing it. A finding that lives only in a conversation is a
finding that gets re-found.
