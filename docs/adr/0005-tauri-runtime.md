---
doc: adr-0005
type: adr
status: accepted
elements: [blastradius.shell]
---

# ADR-0005: Tauri, with a Rust core

## Status
Accepted — 2026-08-22

## Context
The app is local-first and filesystem-heavy: parse and validate YAML across
many files, watch for external edits, run libgit2-grade git queries, and stay
responsive while doing all of it. Electron gives one known Chromium everywhere
at the cost of ~150MB installers and a Node backend; Tauri gives ~10MB
installers and a Rust backend at the cost of per-platform WebViews.

## Decision
Tauri 2.x. The core domain logic — model parsing/validation, sync engine
arbitration, git service, file watching, exporters — lives in Rust and is
exposed to the WebView over typed IPC commands. The WebView owns rendering
only: canvas, panels, the ELK layout worker (ADR-0006).

The WebView-divergence risk is contained deliberately: the design system is
plain CSS + SVG with no Chromium-only features, and CI runs a screenshot suite
on all three platforms.

## Consequences
- git2 (libgit2 bindings), serde_yaml, and notify give us native-speed git,
  parsing, and watching without a JS bridge in the hot path.
- The IPC boundary is a real API surface and gets a spec section
  (spec/sync-engine.md) — it is also where a future CLI (`blastradius init`,
  headless export for CI) attaches, since the Rust core is a library first.
- WebKit (macOS) is the constraining renderer; features land only when they
  pass there.
