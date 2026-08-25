---
doc: spec-model-format
type: spec
status: draft
elements: [blastradius.core.model-service]
---

# Spec: workspace and model format

Normative reference for the Blastradius YAML schema, v1. The workspace in this
`docs/` folder is the conformance example; where prose and that workspace
disagree, it is a bug in one of them and CI should have caught it.

## 1. Workspace

A workspace is a folder containing a `blastradius.yaml` manifest
(ADR-0014; the pre-0.2.0 name `workspace.yaml` still loads with a
deprecation warning, and `blastradius.yaml` wins when both exist):

```yaml
workspace:
  name: Blastradius        # display name
  version: 1               # schema version — integer, required
model:
  include: [model/*.yaml]
views:
  include: [views/*.yaml]
docs:
  include: ["*.md", "adr/*.md", "spec/*.md"]
```

- Globs are relative to the manifest, forward-slash, `*` and `**` only.
- `version` gates parsing: a loader encountering a greater version than it
  knows refuses with an upgrade message, never a partial parse. Migrations
  between versions are a roadmap deliverable.
- Files matched by no glob are ignored — a workspace can live inside a bigger
  `docs/` tree without claiming all of it.

## 2. Identity

Per ADR-0003: the YAML key of an element is its immutable id — lowercase slug
`[a-z0-9-]{1,64}`, unique among siblings. Global references use dotted paths:
`<system>.<container>.<component>` (`blastradius.core.sync-engine`). People
and external systems are addressed by bare id (`architect`, `git-repo`).

## 3. Model files

Each model file declares either context elements or one software system.

`model/context.yaml`:

```yaml
people:
  architect:
    name: Platform Architect
    description: Owns the architecture documentation for their group.
external:
  os-shell:
    name: Operating System
    description: File system, WebView, window manager.
```

`model/<system-id>.yaml` (one system per file, per ADR-0004):

```yaml
system: blastradius            # id of this system; must match one glob'd file
name: Blastradius
description: Local-first desktop app for C4 architecture models.

containers:
  ui:
    name: Canvas UI
    tech: WebView · design system
    description: Rendering and interaction; owns no domain state.
    components:                # L3, optional
      canvas:
        name: Canvas
        tech: SVG + DOM
  core:
    name: Core
    tech: Rust
    components:
      model-service: { name: Model Service }
      sync-engine:   { name: Sync Engine }

relations:
  - from: ui
    to: core.model-service     # dotted path, relative to this system
    label: load & edit model
    protocol: Tauri IPC
  - from: architect            # context elements by bare id
    to: ui
    label: models the system
```

Rules:

- `name` defaults to the titleized id. `tech`, `description` optional.
- `relations` may appear at system level (as above), nested under a
  container (then `from` defaults to that container), or in a **context
  file** (endpoints are then absolute — bare context ids or dotted paths;
  there is no scope). `from`/`to` accept bare ids (context), sibling ids, or
  dotted paths; cross-system references use full paths from the root.
  Context-file relations were silently dropped by the parser before
  2026-08-22 even though the sync engine wrote person-relations there — a
  data-loss bug found by the MCP test suite; both surfaces now agree.
- Relations are **directed** (`from` → `to`), and the direction is the
  *dependency*, not the flow of data. `direction: both | none` overrides;
  omitted means forward — an undirected relation is a deliberate choice,
  mirroring the canvas grammar.
- `label` says what the dependency is; `protocol` says what it runs on and is
  rendered **beneath the label in square brackets** (`calls` / `[JSON/HTTPS]`),
  which is how C4 writes technology. Elements carry the same convention on
  their type line: `[Container: Rust]`, `[Person]`. One implementation
  (`ui/js/labels.js`) serves the canvas, the SVG export, the exported viewer
  and the layout engine's label measuring — before 0.6.0 all four disagreed,
  and the SVG export dropped the protocol entirely whenever a label was set.
- `external: true` on a system renders the dashed external style. Scalar
  one-line form (`model-service: { name: Model Service }`) is valid YAML flow
  style and encouraged for terse L3 listings.

## 3c. Groups (0.5.0)

A `group:` label draws a boundary around the elements that share it:

```yaml
containers:
  web:    { name: Web, group: Storefront }
  api:    { name: API, group: Storefront }
  ledger: { name: Ledger, group: Finance }
```

- **Presentation, not structure.** A group is not a nesting level: ids stay
  `system.container`, no altitude is added, and no element gains a parent.
  Nothing about identity (ADR-0003), relations, or pins changes because an
  element joined a group.
- Elements group with their **siblings** — the same label under two different
  parents is two different boundaries, because a boundary is drawn inside one
  scene.
- The label is free text, and is what the boundary is titled. Blank is treated
  as absent.
- **Rendering is opt-in per view**: `show-groups: true` (§4), off by default,
  so adding a label never changes an existing diagram's shape.
- This mirrors Structurizr's `group`, which the importer previously flattened
  with a "groups are not modelled" diagnostic — grouped workspaces now import
  with their grouping intact.

**How a boundary is placed.** A group whose members are all auto-laid becomes a
real ELK compound, so the members are laid out *together* and ELK sizes the
box. A group holding a pinned member cannot be — pinned nodes never enter the
ELK graph — so its boundary is a box drawn round the finished geometry
instead: the user has taken manual control of where those nodes sit, and the
boundary follows rather than overrides. In the canvas the drawn box is then
grown to cover its members' real rendered heights, because a `.node` is
content-sized and a long name wraps taller than layout's per-kind estimate;
the SVG path needs no such correction, since there nodes and boundaries use
the same numbers.

## 3b. Deployment (ADR-0018)

The physical counterpart to the logical model: where the containers
actually run. One file, `model/deployment.yaml`, holding every
environment:

```yaml
environments:
  dev-machine:
    name: Developer Machine
    description: Where the app is built and dogfooded.
    nodes:
      workstation:
        name: Windows 11 Workstation
        tech: x64
        nodes:                          # deployment nodes nest arbitrarily
          dev-build:
            name: Blastradius (dev build)
            tech: cargo tauri dev
            instances:
              shell: { container: blastradius.shell }
              ui:    { container: blastradius.ui }
    relations:
      - from: workstation
        to: ci.runner
        label: push
```

- **Three kinds**: `environment` (top of a tree), `deployment-node`
  (anything a thing runs *on* — machine, runner, service, image), and
  `container-instance` (a modelled container actually running there).
- **Ids are dotted, like everything else** (ADR-0003): the key is the id,
  unique among siblings, and the full address is the path —
  `dev-machine.workstation.dev-build.shell`. Environments are top-level
  ids and must not collide with system or context ids.
- `instances:` entries carry `container:`, a reference to a `container`
  element resolved exactly like a relation endpoint. A dangling
  reference is an error — the point of modelling deployment here rather
  than in prose is that it cannot quietly drift.
- `nodes:` and `instances:` may both appear on a node. `name`, `tech`,
  `description` behave as everywhere else; an instance's `name` defaults
  to the referenced container's name.
- `relations:` may appear on an environment (endpoints relative to it)
  and works like system relations. Deployment and logical elements are
  in one id space, so a relation may cross between them.
- **Rendering is by altitude, not containment** (ADR-0018): a
  deployment view shows one level and dives, exactly like
  containers → components. Nested boxes are a recorded follow-up.

## 4. Views

`views/<view-id>.yaml`:

```yaml
view: containers            # view id
name: Containers            # optional display name
scope: blastradius          # element whose children this view shows
level: L2                   # L1 | L2 | L3 | LD — which altitude this view captures
                            # LD is a deployment view (ADR-0018); its scope is an
                            # environment or a deployment node — or omitted, for
                            # the overview of every environment, whose pins are
                            # then absolute ids
layout:                     # pinned positions — grid units (26px cells @ 1×)
  ui: [4, 2]
  core: [10, 4]
show-groups: false          # draw `group:` boundaries (§3c) — default false
include-context: true       # show people/externals related to scope (default true)
                            # (honored by the renderer since 2026-08-22 — it was
                            # parsed-but-ignored before; core-components.yaml
                            # exercises the false path)
```

- Elements absent from `layout:` are auto-placed (ADR-0006). Pinning is the
  exception, not the rule.
- Grid units, not pixels: layouts survive zoom and density changes.
- A workspace with zero view files is valid — every level renders fully
  auto-laid-out.
- `scope:` is required except on an `LD` overview, the one view whose subject
  is the whole deployment rather than one element.

## 5. Documents

Markdown files matched by `docs.include` and starting with frontmatter join
the model (ADR-0010):

```yaml
---
doc: adr-0007            # document id — same slug rules, unique workspace-wide
type: adr                # prd | adr | spec | roadmap | note
status: accepted         # see vocabularies below
elements: [blastradius.core.git-service]
supersedes: adr-0004     # optional, adr only
---
```

Status vocabularies — `adr`: proposed / accepted / superseded / rejected;
`prd`, `spec`, `roadmap`: draft / current / superseded; `note`: none.
A matched file **without** frontmatter is ignored with an info-level notice
(ordinary READMEs may share the folder).

## 6. Validation

Errors (workspace loads, marked invalid; affected elements get `is-invalid`):
duplicate id in scope; dangling reference from a relation, layout, or doc
`elements:` list; unknown schema version; malformed YAML in any included file.

Warnings: doc frontmatter with unknown `type`; empty system; relation
duplicated verbatim.

Everything is reported with file + line, in the panel footer and on the
canvas — never only in a log.

## 7. Stability contract

Additions to the schema are minor (same `version`); anything that changes the
meaning of an existing construct bumps `version` and ships with a migration.
Frontmatter counts as schema surface under the same contract.
