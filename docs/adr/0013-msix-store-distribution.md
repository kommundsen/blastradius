---
doc: adr-0013
type: adr
status: accepted
elements: [blastradius.shell, blastradius.cli]
---

# ADR-0013: Windows distribution via Microsoft Store MSIX

## Context

Phase 5 deferred packaging because installers implied real costs: a Windows
code-signing certificate runs hundreds of dollars a year, and an unsigned
NSIS/MSI download trips SmartScreen and Smart App Control on every user's
machine. Meanwhile Microsoft made Store developer registration free
(individual accounts September 2025, company accounts May 2026), and the
Store signs submitted MSIX packages itself — a Store-distributed build
needs no certificate at all.

Tauri's bundler still does not emit MSIX (only NSIS EXE and MSI). Tauri's
own Microsoft Store guide works around this by listing an *unpackaged* EXE
installer in the Store, which forfeits Store signing, Store auto-updates,
and clean install/uninstall — the three things we want.

Microsoft now ships the `winapp` CLI (winget `microsoft.winappcli`), with a
first-party guide for Tauri apps: `winapp init` scaffolds a
`Package.appxmanifest` + assets, `winapp pack` wraps staged release
binaries into an MSIX. Because Tauri embeds the `frontendDist` assets into
the release exe, the staged payload is just our two binaries.

## Decision

Ship the Windows build as an MSIX in the Microsoft Store, packaged with the
`winapp` CLI over the exes `cargo build --release` already produces.
Tauri's bundler stays off (`bundle.active: false`). The package carries both
`blastradius-app.exe` (the entry point) and `blastradius.exe` with an app
execution alias, so a Store install also puts the CLI — and with it `init`
and the MCP server — on PATH.

Updates flow through the Store; we never add a self-updater to this build
(Store policy forbids self-updating apps anyway). The MSIX submitted to the
Store is uploaded unsigned — the Store signs it. Local installs for testing
use a `winapp cert generate` dev certificate.

The full procedure — one-time Partner Center setup, per-release packaging,
submission checklist — lives in [spec/msix-store-packaging.md](../spec/msix-store-packaging.md).

## Alternatives rejected

- **Signed NSIS installer, direct download** — the cert cost and yearly
  renewal are exactly what we deferred packaging to avoid; SmartScreen
  reputation still takes weeks to build even with a standard (non-EV) cert.
- **Store listing of an unpackaged EXE installer** (Tauri's documented
  path) — requires the cert anyway (Store mandates signed Win32 installers),
  plus our own update mechanism.
- **Community MSIX bundlers** (`tauri-windows-bundle` npm package) — works,
  but adds a Node build dependency for what `winapp` does first-party and
  supported.

## Consequences

- Windows packaging cost drops to zero; the open cost question narrows to
  macOS (Apple Developer ID, $99/year), which stays deferred.
- The manifest identity (Package name, `CN=`-GUID publisher) comes from
  Partner Center after name reservation — the committed
  `Package.appxmanifest` must carry those values before a Store pack.
- MSIX pins minimum Windows 10 1809 (build 17763); fine, Tauri 2 needs
  WebView2 which effectively assumes the same era.
- WebView2 is not bundled: Windows 11 ships it, Windows 10 has it via Edge
  servicing on effectively all updated machines. Recorded as a residual
  risk in the spec; revisit only if Store reviews or users hit it.
