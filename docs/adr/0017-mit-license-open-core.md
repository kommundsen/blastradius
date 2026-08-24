---
doc: adr-0017
type: adr
status: accepted
elements: [blastradius]
---

# ADR-0017: MIT license, open-core commercial posture

## Status
Accepted — 2026-08-24

## Context

The repo is going public, primarily so a public codebase backs a
publicly distributed Store app.

## Decision

- **MIT for everything in this repository** — the core, the CLI, the
  app, the extractors, the docs tooling. Maximum adoption, minimum
  friction; the dogfooding repo itself becomes the best advertisement
  an architecture-modeling tool can have.
- **Commercial posture shifts open-core**: revenue, if pursued, comes
  from services around the code rather than the code — consulting,
  donations/sponsorship, and potentially closed-source add-ons later.
  MIT explicitly permits that split: the core stays free; anything
  proprietary is simply developed outside this repository.
- The PRD's pricing hypothesis (free tier with export footer, paid
  hosted share links) is **not repealed** — a hosted service remains
  compatible with an MIT core (the service is the product, not the
  code) — but it is no longer the primary commercial thesis.

## Consequences

- Anyone may ship a competing build, including commercially. Accepted:
  for a tool whose value grows with its ecosystem, adoption risk beats
  obscurity risk at this stage.
- Contributions arrive under MIT inbound=outbound; no CLA.
- The "Blastradius" name, Store listing, and publisher identity are not
  granted by the code license; forks must ship under their own identity
  (Store identity values are unique per publisher anyway).
- `license = "MIT"` in the workspace manifest; `LICENSE` at the root.
