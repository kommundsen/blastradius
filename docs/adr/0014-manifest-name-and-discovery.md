---
doc: adr-0014
type: adr
status: accepted
elements: [blastradius.core.model-service, blastradius.shell.window, blastradius.cli]
---

# ADR-0014: Self-identifying manifest and repo-root discovery

## Status
Accepted — 2026-08-23

## Context

Two ease-of-life gaps shared one root cause. Opening a workspace meant
picking the exact folder holding the manifest — pointing the app (or
`blastradius validate`) at the repo root failed, even though the MCP server
already special-cased `./docs`. And the manifest's name, `workspace.yaml`,
is claimed by half the tool ecosystem (Melos, various build systems), so
"find the workspace under this root" could not trust filenames.

The rename window is also as cheap as it will ever be: 0.1.0 shipped to
the Store days ago, user count ≈ 0, and the cost of a rename only grows.

## Decision

1. **The manifest is `blastradius.yaml`** — self-identifying, in the
   `cargo.toml`/`pnpm-workspace.yaml` tradition. The legacy
   `workspace.yaml` still loads everywhere, with a deprecation warning;
   when both exist, `blastradius.yaml` wins and the ignored file is
   flagged. Scaffold and importer emit the new name only.
2. **Every entry point discovers**: the app's Open dialog, `blastradius
   validate/snapshot/diff/gitdiff/export/mcp`, and startup args accept a
   repo root. Discovery walks breadth-first to depth 4, skips hidden and
   dependency/build directories, and **content-sniffs** each candidate for
   a top-level `workspace:` key — a filename match alone is never trusted,
   which keeps other tools' `workspace.yaml` files and model files that
   happen to be named `blastradius.yaml` (our own dogfood has one) out of
   the results.
3. **Ambiguity is surfaced, not guessed**: several workspaces under one
   root produce a picker in the app and an explicit error listing the
   candidates in the CLI. A monorepo with many models is a supported
   layout, not an error.

## Consequences

- "Open your repo" becomes the default mental model — and the app learning
  the *repo* root (not just the model folder) is the anchor the future L4
  source-derived elements need.
- ADR-0004's layout contract is otherwise unchanged; only the manifest's
  filename moved. Spec §1 (model-format.md) is the normative reference.
- The deprecation warning is deliberately a *warning*, not info: the
  dogfood gate keeps `docs/` at zero warnings, so this repo (and any repo
  holding itself to the same bar) migrates immediately.
- Legacy-name support is a v1-line commitment; removing it would be a
  schema-version event, not a quiet cleanup.
