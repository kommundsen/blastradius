# Blastradius

Local-first desktop app for beautiful, interactive C4 architecture models —
YAML-driven, git-versioned, with a zooming canvas from system context (L1)
down to code (L4).

- **[docs/prd.md](docs/prd.md)** — what we are building and why
- **[docs/roadmap.md](docs/roadmap.md)** — phased plan with exit gates
- **[docs/adr/](docs/adr/)** — architecture decisions
- **[docs/spec/](docs/spec/)** — subsystem specifications (the model format
  lives in [docs/spec/model-format.md](docs/spec/model-format.md))
- **[design-system/](design-system/)** — the brand and component system

`docs/` is itself a valid Blastradius workspace modelling Blastradius — the
dogfood gate: every phase must be demonstrable against this folder, and CI
validates it on every push.

## Repository layout

```
crates/blastradius-core/   Rust library: loading, validation, sync engine, git, diff, export, import
crates/blastradius-cli/    `blastradius` binary: init, validate, diff, snapshot, gitdiff, export, import
crates/blastradius-app/    Tauri desktop shell (WebView renders; Rust owns truth)
ui/                        WebView frontend — vanilla ES modules, no bundler
design-system/             tokens, components, specimen cards (plain CSS + JSX refs)
docs/                      product docs + the dogfood workspace
tools/                     sync-ds.py (design-system -> ui/ds), build-site.mjs (docs site)
```

## Getting started from a fresh clone

Everything below assumes nothing but a clean OS install. The build is fully
self-contained: no system libraries beyond the platform toolchain (libgit2 is
vendored; the WebView is the OS one).

### 1. Install the toolchain

**Both platforms need:**

| Tool | Why | Version |
| --- | --- | --- |
| Rust (via [rustup](https://rustup.rs)) | core, CLI, desktop app | stable, ≥ 1.85 |
| Git | you are cloning a repo | any recent |
| Node.js | frontend tests, docs site — *not* needed to build or run the app | ≥ 20 |
| Python 3 | only for `tools/sync-ds.py` (design-system sync) | any 3.x |

**Windows (10/11):**

1. Visual Studio Build Tools with the *Desktop development with C++*
   workload (Rust's MSVC target needs the linker):
   ```
   winget install Microsoft.VisualStudio.2022.BuildTools
   ```
2. Rustup, then restart the shell so `%USERPROFILE%\.cargo\bin` is on `PATH`:
   ```
   winget install Rustlang.Rustup
   ```
3. Node (for the test suites and docs site): `winget install OpenJS.NodeJS.LTS`
4. WebView2 is preinstalled on Windows 11; on Windows 10 install the
   [Evergreen runtime](https://developer.microsoft.com/microsoft-edge/webview2/).
5. **Smart App Control** (Windows 11): it blocks freshly compiled cargo build
   scripts (unsigned binaries), so Rust development effectively requires it
   off — Windows Security → App & browser control. Since the April 2026
   cumulative update (KB5083769) it can be re-enabled from the same place;
   on builds without that update, re-enabling still requires a Windows
   reset.

**macOS (13+):**

1. Xcode command-line tools (compiler + linker):
   ```
   xcode-select --install
   ```
2. Rustup:
   ```
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
3. Node: `brew install node` (or the [installer](https://nodejs.org)).
4. The app uses the system WebKit — nothing else to install. WebKit is also
   the *constraining* renderer, which is why CI's rendering gate runs
   Playwright WebKit (ADR-0011).

**Linux:** builds too — the extra WebKitGTK packages are listed in the `app`
job of [.github/workflows/ci.yml](.github/workflows/ci.yml).

A note on line endings: the repo pins **LF** via `.gitattributes` (the core
hashes sources; CRLF checkouts would break them). No autocrlf tweaking needed
— the attributes file wins.

### 2. Build and run

```bash
git clone https://github.com/kommundsen/blastradius.git
```
```bash
cd blastradius && cargo build
```

First build takes a few minutes (vendored libgit2 compiles once). Then:

```bash
cargo run -p blastradius-app -- docs
```

opens the desktop app on the dogfood workspace. Launched with no argument
(and no `./docs`), the app shows the welcome screen instead — open any
folder, scaffold a new workspace into one, or spin up a throwaway demo.

To model **your own repo**:

```bash
cargo run -p blastradius-cli -- init path/to/your/repo
```

then open that folder in the app (File → Open, Ctrl+O). The scaffold is five
commented YAML files; `blastradius validate .` checks them and is CI-ready.
Run interactively, `init` also offers to `git init` a fresh folder and to
register the MCP server and agent skills for Claude Code, Copilot/VS Code,
Cursor, and Codex — project-scoped config files at the repo root, merged
into whatever already exists, never overwritten. Passing any of the flags
switches to fully non-interactive mode (for scripts and CI). Re-running
`init` on an existing workspace skips the scaffold but still offers the
extras.

### 3. Run the test battery

```bash
cargo test
```
```bash
cargo test --release -p blastradius-core --test budgets -- --include-ignored
```
```bash
node --test ui/tests/determinism.test.mjs
```
```bash
npm ci && npx playwright install --with-deps webkit && npx playwright test
```

In order: Rust unit/integration suites (includes the dogfood conformance
gate), the performance budgets (release-build contracts,
[docs/spec/sync-engine.md](docs/spec/sync-engine.md)), ELK layout determinism
(ADR-0006), and the WebKit rendering + a11y gate (ADR-0011; includes an
axe-core WCAG AA audit of every surface).

## CLI

```
blastradius init [dir] [--name <name>]      scaffold a starter workspace, offer
     [--git|--no-git] [--agents <list>]     git init + agent MCP config + skills
     [--skills <list>]                      (list: all | none | claude,copilot,cursor,codex)
blastradius validate [dir]                  parse + validate, file:line diagnostics
blastradius diff <base-dir> <current-dir>   semantic model diff
blastradius gitdiff <dir> [base] [cur]      semantic diff from git history
blastradius snapshot [dir]                  renderer snapshot as JSON
blastradius export <dir> -o out.html        self-contained interactive HTML
blastradius import <file.dsl> <out-dir>     one-way Structurizr DSL import
blastradius mcp [dir]                       MCP server over stdio (ADR-0012)
```

(Substitute `cargo run -q -p blastradius-cli --` for `blastradius` when
running from the repo.)

## Coding agents (MCP)

The model is queryable by coding agents: `blastradius mcp <workspace-dir>`
serves ten tools over stdio (MCP) — orientation, search, per-element detail,
**blast_radius** impact analysis, validation, semantic git diff, doc bodies,
and edits that go through the sync engine so they are format-preserving and
undoable. Register with Claude Code:

```bash
claude mcp add blastradius -- blastradius mcp path/to/workspace
```

Details in [docs/spec/mcp-server.md](docs/spec/mcp-server.md) and
[ADR-0012](docs/adr/0012-mcp-server.md).

## Frontend development

`ui/` is plain ES modules — no bundler, no build step. Serve it statically
and it runs against `ui/mock/snapshot.json` instead of Tauri IPC, so the
whole frontend is developable in a browser:

```bash
node ui/tests/serve.mjs 4173
```

Query params: `?nogit` simulates a plain folder (no git chrome), and
`?noworkspace` lands on the welcome screen. Regenerate the mock after model
changes:

```bash
cargo run -p blastradius-cli -- snapshot docs > ui/mock/snapshot.json
```

`ui/ds/` is a build-artifact copy of `design-system/` — edit the design
system, then run `python tools/sync-ds.py`. `ui/vendor/` holds elkjs
(EPL-2.0), marked (MIT), and CodeMirror 5 (MIT).

## Docs site

```bash
cargo run -p blastradius-cli -- export docs -o architecture.html
```
```bash
node tools/build-site.mjs
```

renders `docs/` into `site/` (design-system styling, live model bundled in).
CI builds it on every push and deploys from `master` via GitHub Pages — the
one manual prerequisite is enabling Pages once: repo **Settings → Pages →
Source: GitHub Actions**.

## Design system

`design-system/` is plain CSS + HTML specimen cards — no build step. To
browse the cards locally, serve the repo root (any static server) and open
`design-system/ui_kits/app/index.html`. The system is also synced to the
Claude Design project "Blastradius Design System".
