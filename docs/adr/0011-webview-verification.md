---
doc: adr-0011
type: adr
status: accepted
elements: [blastradius.shell, blastradius.ui]
---

# ADR-0011: WebView verification — Playwright WebKit as the CI rendering gate

## Status
Accepted — 2026-08-22

## Context
The app ships on three WebView engines (ADR-0005): WebView2/Chromium on
Windows, WebKitGTK on Linux, system WebKit on macOS — and WebKit is the
constraining renderer. The Phase 1 exit criterion said "screenshot suite green
on all three OS WebViews", which turns out to be unachievable as written:
tauri-driver (the WebDriver bridge into a real Tauri window) does not support
macOS at all, so the one engine that matters most is exactly the one a
native-window suite cannot reach in CI.

## Decision
Split verification into what each layer can actually catch:

1. **Rendering correctness runs in Playwright against a real WebKit build**,
   on Linux CI, loading the frontend through the mock harness (the same
   ES modules and CSS the Tauri window loads, minus IPC). WebKit CSS/JS/SVG
   divergence — the real risk — is caught here, on every push, with
   screenshots uploaded as CI artifacts.
2. **Shell integration compiles on all three OSes** in the CI matrix; the IPC
   and watcher paths are thin and OS-independent by design.
3. **Native-window verification is manual and staged**: Windows continuously
   during development (the dev machine), macOS and Linux at Phase 5
   packaging, when real hardware is in the loop anyway for signing and
   installers.

This promotes the mock harness from convenience to **contract**: the frontend
must remain fully runnable against `ui/mock/snapshot.json` with no Tauri
present, because that is what CI verifies rendering with.

## Consequences
- Playwright is the repo's first Node dependency (`package.json` at root,
  dev-only). The no-bundler rule for `ui/` itself still holds.
- A WebKit rendering bug in something only the shell exercises (IPC timing,
  window chrome) can still slip to Phase 5 — accepted and named, rather than
  pretended away by an unrunnable criterion.
- The Phase 1 exit criterion in the roadmap is amended accordingly, by commit,
  not silently.
