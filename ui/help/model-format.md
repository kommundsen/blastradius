# Model format reference

A workspace is a folder with a `blastradius.yaml` manifest. Everything else is
ordinary YAML and markdown, matched by globs.

## Manifest

```yaml
workspace:
  name: Acme
  version: 1
model:
  include: [model/*.yaml]
views:
  include: [views/*.yaml]
docs:
  include: ["*.md", "adr/*.md", "spec/*.md"]
```

`version: 1` is the schema gate. New fields may be added within a version;
anything that changes what existing files *mean* bumps it, with a migration.

## Ids

**The YAML key is the id.** Lowercase slug, unique among its siblings, and
immutable — relations, layout pins, and document links all point at it. The
full address is the dotted path: `shop.api.router`. Display names come from
`name:`, which defaults to a titleized id.

## Model files

A model file declares **one** of: context elements, one system, or deployment
environments.

Context — the people and systems around yours:

```yaml
people:
  architect: { name: Platform Architect }
external:
  payments: { name: Payment Provider }
relations:
  - from: architect
    to: shop
    label: maintains
```

A system, with containers and components:

```yaml
system: shop
name: Shop
description: Storefront and its services.
containers:
  api:
    name: API
    tech: Go
    components:
      router: { name: Router }
      store:  { name: Store, tech: Postgres }
relations:
  - from: web
    to: api
    label: calls
    protocol: JSON/HTTPS
```

Fields: `name`, `tech`, `description` everywhere; `external: true` on a system
draws it in the external style.

## Relations

```yaml
relations:
  - from: web
    to: api
    label: calls
    protocol: JSON/HTTPS
    direction: both    # forward | both | none
```

Relations may sit at system level, nested under a container (where `from`
defaults to that container), or in a context file (where both endpoints are
absolute). Endpoints accept a sibling id, a dotted path, or a bare context id.

## Deployment

See [Deployment views](deployment.md) for `model/deployment.yaml`:
`environments:` → `nodes:` (nesting) → `instances:` pointing at containers.

## Views

One file per view, under `views/`:

```yaml
view: containers
name: Containers
scope: shop
level: L2              # L1 | L2 | L3 | LD
layout:
  api: [4, 2]          # grid units, not px
  web: [10, 4]
include-context: true  # related people/externals
```

Views contribute **pins and options only** — never the element list, which
comes from the model. A workspace with no view files is valid; everything is
auto-laid-out.

## Documents

Any markdown file matched by `docs.include` that starts with frontmatter joins
the model:

```markdown
---
doc: adr-0007
type: adr
status: accepted
elements: [shop.api]
---

# Use Postgres for the store
```

`type` is `adr`, `spec`, `prd`, `note`, or `roadmap`; `status` is checked
against the vocabulary for that type. Every id in `elements:` must resolve —
so a document that talks about something you have deleted becomes a model
error rather than a stale page nobody notices.

## Code-level mappings

A component may carry a `source:` mapping to derive its internals from real
code — see [Code-level detail](code-level.md).

## What validation catches

Duplicate ids, dangling relation endpoints, layout pins and document links
pointing at nothing, unknown schema versions, malformed YAML, and container
instances naming containers that do not exist. Errors carry file and line.

```
blastradius validate .
```
