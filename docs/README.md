# Blastradius — product documentation

This folder is two things at once, deliberately:

1. **The product documentation** — PRD, architecture decision records, subsystem
   specs, and roadmap for building Blastradius.
2. **A Blastradius workspace** — `workspace.yaml` + `model/` + `views/` form a
   valid model *of Blastradius itself*, and every document here carries
   frontmatter that registers it as a typed document inside that model.

The second point is the dogfood contract: when the app can open a workspace,
`File → Open → docs/` must work, and the diagram it renders is the architecture
described by these documents. Anything the schema cannot express about this
folder is a schema bug, discovered here first.

## Layout

```
docs/
  workspace.yaml        workspace manifest — include globs for model, views, docs
  model/                the semantic model (C4 elements, relations)
    context.yaml          people and external systems
    blastradius.yaml      the app itself: containers and components
  views/                layout and view definitions — kept OUT of model files
    containers.yaml       L2 view with pinned positions
  prd.md                product requirements        (doc type: prd)
  roadmap.md            phased delivery plan        (doc type: roadmap)
  adr/                  architecture decision records (doc type: adr)
    0001-…0010
  spec/                 subsystem specifications    (doc type: spec)
    model-format.md       the YAML schema itself — the normative reference
    sync-engine.md        bidirectional canvas ↔ YAML sync
    git-and-diff.md       git integration, semantic diff, conflicts
    export.md             Share: self-contained HTML + PNG/SVG
```

## How documents join the model

Every markdown file starts with YAML frontmatter:

```yaml
---
doc: adr-0003            # stable document id
type: adr                # prd | adr | spec | roadmap | note
status: accepted         # per-type lifecycle, see spec/model-format.md
elements: [blastradius.core.model-service]   # element ids this document governs
---
```

Element ids are dotted paths into `model/` (see `spec/model-format.md`).
The app resolves these links both ways: select an element on the canvas and its
governing documents are one click away; open a document and the elements it
names are highlightable. Broken links are validation errors, same as a relation
pointing at a missing element — docs rot is a model error here, not a wiki
problem.

## Rules for editing this folder

- The model describes **what we are building**, per the current roadmap phase —
  not the aspiration. When the architecture changes, the ADR lands in the same
  commit as the model edit.
- Decisions live in ADRs; specs describe mechanisms; the PRD owns scope and
  success metrics. If a paragraph is deciding something, it is an ADR.
- Never hand-edit an element id. Ids are permanent (ADR-0003).
- **Every finding that needs future follow-up is recorded here the moment it
  arises** — ideally in the same commit as the code that created it, never
  only in a conversation, review thread, or someone's head. Destinations, in
  order: a deferred capability → the roadmap's Phase 5 "named debts" list (or
  v2 themes); a spec that no longer matches the implementation → amend the
  spec with an explicit "v1 simplification" / "not yet implemented" note; an
  implementation-local gap → a comment at the exact site, pointing at the doc
  that will retire it. A limitation we know about but did not write down is
  treated as an undocumented model error — the same class of rot this product
  exists to kill.
