---
doc: adr-0003
type: adr
status: accepted
elements: [blastradius.core.model-service]
---

# ADR-0003: Stable element ids, separate display names

## Status
Accepted — 2026-08-22

## Context
Relations, view layouts, doc links, and git history all reference elements. If
the reference key is the display name, a rename rewrites every referencing
line, severs line-history, and breaks any link the app did not know about.
This decision is effectively permanent: changing identity schemes later breaks
every workspace in existence.

## Decision
Every element has an **immutable id** — its YAML key, a lowercase slug
(`[a-z0-9-]+`), unique among siblings, globally addressed by dotted path
(`blastradius.core.sync-engine`). Display name is a `name:` field, defaulting
to the titleized id when absent. All references — relations, layouts, doc
frontmatter — use ids only.

Rename is therefore a one-line diff by construction. The app never offers to
change an id; the canvas rename affordance edits `name:`.

## Consequences
- Renames are safe from any editor, not just the app — no refactor tooling
  needed for the common case.
- Ids are forever: a badly chosen id (`api2`, `new-service`) survives its
  embarrassment. Mitigation: the create-element dialog derives the id from the
  first-typed name and shows it for one-time confirmation.
- Hand-authors must not reuse a deleted element's id for a *different* concept
  while old branches reference it; the git diff surface treats same-id as
  same-element. Documented in the schema spec; not machine-enforced in v1.
