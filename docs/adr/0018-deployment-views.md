---
doc: adr-0018
type: adr
status: accepted
elements: [blastradius]
---

# ADR-0018: Deployment views dive rather than nest

## Status
Accepted — 2026-08-24

## Context

0.4.0 adds the C4 deployment view: environments containing deployment
nodes (which nest arbitrarily) containing instances of modelled
containers. Two questions had to be settled before any code.

**How should a nested tree be drawn?** C4 deployment diagrams are
conventionally nested boxes — a region inside a data centre inside an
environment. Blastradius has never drawn containment: every existing
view shows exactly one altitude, with children collapsed to a count
("6 components") until you dive into them. `ui/js/layout.js` builds a
flat ELK graph with fixed per-kind node sizes, no `hierarchyHandling`,
and a routing pass that treats every other node as an obstacle.

**Where should deployment elements live?** ADR-0016 put L4 code
elements in a *parallel* namespace outside `Workspace::elements`,
precisely so they could stay read-only and out of sync, diff, and
validation. Deployment elements are hand-authored and editable, which
pulls the other way.

## Decision

**Deployment views dive; they do not nest.** An environment is a view
scope, its deployment nodes render as ordinary boxes, and diving into a
node shows its children — the same grammar as system → containers →
components. Nested-box rendering is explicitly not v1.

**Deployment elements are ordinary elements** in `Workspace::elements`,
with dotted ids (`dev-machine.workstation.app.shell`) and three new
kinds: `environment`, `deployment-node`, `container-instance`.

## Consequences

- **The layout engine needs no hierarchy work at all.** One altitude at
  a time is a flat graph, which is what `layout.js` already produces.
  This removes the largest and riskiest piece of the theme, along with
  the knock-on rework it would have forced: ancestor-exclusion in
  obstacle routing, label placement against containment boxes, SVG
  z-ordering and fills so a parent does not paint over its children.
- **The result is more consistent, not merely cheaper.** A user who has
  learned to dive through the logical model navigates the physical one
  identically. Nested boxes would have been the only view in the product
  that reads by containment.
- Dotted ids mean `computeView`'s existing depth arithmetic
  (`startsWith(scope + '.')` one level down) computes deployment views
  with no new algorithm — the L2/L3 branch already does exactly this.
- Because they are real elements, deployment nodes get relations,
  layout pins, blast radius, doc links, diff, and canvas editing for
  free, and the MCP server answers about them like anything else.
- The cost of that choice: four exhaustive `ElementKind` matches must
  grow (`model.rs`, `snapshot.rs`, `resolve.rs`, `sync.rs`), and the
  editing paths (`element_chain`, create, delete) need a variable-depth
  chain, because deployment nodes nest arbitrarily while containers and
  components sit at fixed depths.
- `container-instance` elements point at a modelled container. That
  reference is validated, so a deployment cannot drift into naming
  containers that no longer exist — the point of modelling it here
  rather than in prose.
- The **overview** — every environment at once — is the one view with no
  scope element, so `scope:` became optional for `LD` views and its pins are
  absolute ids. Without that, the single most useful deployment picture, the
  delivery chain between environments, could be neither pinned nor rendered
  headlessly. It also draws the people and external systems the deployment
  actually touches, chosen by relation rather than shown wholesale as L1 does.
- **Recorded follow-ups**: nested-box rendering as an optional display
  mode, should real use ask for it; instance multiplicity (`replicas`)
  as a field rather than repeated elements; and the Structurizr
  importer's `deploymentEnvironment`/`deploymentNode` blocks, which are
  currently parsed and discarded (`import.rs`) and become importable now
  that the primitives exist.
