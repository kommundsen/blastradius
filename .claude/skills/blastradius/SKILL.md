---
name: blastradius
description: Query and edit this repo's Blastradius C4 architecture model (YAML workspace). Use when working with the architecture model, ADRs, or when a change affects modelled components.
---

# Working with the Blastradius model

This repository contains a Blastradius workspace in `docs/` — a C4
architecture model as plain YAML (blastradius.yaml + model/ + views/),
versioned like source code.

When architecture is relevant:

- Query the model through the `blastradius` MCP tools. Start with
  `workspace_summary`; call `blast_radius` with an element id before
  changing or deleting anything it models; `doc` returns the ADRs and
  specs governing an element.
- Prefer the `apply_operation` tool for model edits — it splices the
  YAML in place (comments and formatting survive), validates before
  writing, and is undoable. If you edit the YAML by hand instead:
  never re-serialize or re-order keys, and run `blastradius validate
  docs` (or the `validate` tool) afterwards.
- Element ids (the YAML keys) are immutable — renaming means changing
  the `name:` field only.
- Markdown docs with a `doc:` frontmatter block are part of the model;
  their `elements:` links must point at real element ids.
- Keep the model in sync with reality: when you add, remove, or rewire
  a real component, mirror it in the model in the same change.
- `git_status` and `git_conflicts` read repository state. A merge
  conflict in the model resolves per element: read `git_conflicts`
  (each conflicted element carries ours/theirs field values), then
  call `resolve_conflicts` with {elements: {"<id>": "ours"|"theirs"}}
  — choices splice onto the chosen side (comments survive), files
  are validated and staged via the user's own git, and the commit
  stays the user's. Anything undecided keeps ours.
- Components with a `source:` mapping have derived L4 code elements
  (modules/types extracted from source). They answer in
  `find_elements`, `element`, and `blast_radius` (code-level
  fan-in), marked `derived: true` — read-only; edit the source
  instead, then run the `introspect` tool to refresh the committed
  facts. `blast_radius` on a derived id shows real code dependents.
