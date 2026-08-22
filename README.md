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
crates/blastradius-core/   Rust library: workspace loading, validation, semantic diff, snapshot
crates/blastradius-cli/    `blastradius` binary: validate, diff, snapshot
crates/blastradius-app/    Tauri desktop shell (Phase 1: read-only canvas)
ui/                        WebView frontend — vanilla ES modules, no bundler
design-system/             tokens, components, specimen cards (plain CSS + JSX refs)
docs/                      product docs + the dogfood workspace
```

## Tooling setup

### Both platforms

- **Rust** (stable, ≥ 1.98) via [rustup](https://rustup.rs)
- **Git**
- Node.js ≥ 20 will be needed from Phase 1 (WebView UI); not required yet.

### Windows

1. Install **Visual Studio Build Tools** with the *Desktop development with
   C++* workload (the MSVC linker; Rust's default Windows target needs it):
   ```
   winget install Microsoft.VisualStudio.2022.BuildTools
   ```
2. Install rustup:
   ```
   winget install Rustlang.Rustup
   ```
   then restart the shell so `%USERPROFILE%\.cargo\bin` is on `PATH`.
3. Line endings: the repo pins **LF** via `.gitattributes` (the model core
   hashes sources; CRLF checkouts would break them). No autocrlf tweaking
   needed — the attributes file wins.
4. From Phase 1: Tauri on Windows additionally needs **WebView2** (preinstalled
   on Windows 11) — see the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

### macOS

1. Install the Xcode command-line tools (compiler + linker):
   ```
   xcode-select --install
   ```
2. Install rustup:
   ```
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
3. From Phase 1: Tauri uses the system WebKit — no extra install, but features
   must pass WebKit, which is the constraining renderer (ADR-0005).

## Build, test, validate

```
cargo build                    # everything
cargo test                     # unit + seeded-fault + conformance suites
cargo run -p blastradius-cli -- validate docs
cargo run -p blastradius-cli -- diff <base-workspace> <current-workspace>
cargo run -p blastradius-app -- docs      # the desktop app on the dogfood workspace
node --test ui/tests/determinism.test.mjs # ELK layout determinism (needs Node >= 20)
```

## Frontend development

`ui/` is plain ES modules — no bundler, no node_modules. Open it in any static
server and it runs against `ui/mock/snapshot.json` instead of Tauri IPC, so the
whole frontend is developable in a browser. Regenerate the mock after model
changes:

```
cargo run -p blastradius-cli -- snapshot docs > ui/mock/snapshot.json
```

`ui/ds/` is a build-artifact copy of `design-system/` — edit the design system,
then run `python tools/sync-ds.py`. `ui/vendor/` holds elkjs (EPL-2.0) and
marked (MIT).

One Windows note: **Smart App Control** blocks freshly compiled cargo build
scripts (unsigned binaries). Rust development on Windows effectively requires
it off — it can only be disabled in Windows Security, and re-enabling requires
a reset.

`validate docs` is the dogfood gate and runs in CI
([.github/workflows/ci.yml](.github/workflows/ci.yml));
the conformance test (`crates/blastradius-core/tests/conformance.rs`) asserts
the same thing from inside the test suite, so `cargo test` alone catches a
broken workspace.

## Design system

`design-system/` is plain CSS + HTML specimen cards — no build step. To browse
the cards locally, serve the repo root (any static server) and open
`design-system/ui_kits/app/index.html`. The system is also synced to the
Claude Design project "Blastradius Design System".
