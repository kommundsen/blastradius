---
doc: adr-0019
type: adr
status: accepted
elements: [blastradius.core.introspector]
---

# ADR-0019: Drift detection compares recorded references, not a shared corpus

## Status
Accepted — 2026-08-25

## Context

ADR-0016 closed with the observation that "facts edges that cross component
boundaries are recorded but not yet judged — comparing them against declared
L3 relations (architecture drift detection) is the natural follow-up this
design enables."

Building it revealed that the premise was wrong: **there were no
cross-component edges at all**, and could not be. Each component is extracted
against its own corpus — the files its `source:` mapping claims — so a
reference to code another component owns simply fails to resolve and is
dropped. In this repository, `git.rs` genuinely imports `crate::model`,
`crate::diagnostics`, `crate::vfs`, `crate::splice` and `crate::diff`; every
one of those was discarded at extraction time. The whole dogfood model
contained zero cross-component derived edges.

So drift detection needed a signal that did not exist yet.

## Decision

**Extractors record what they used to discard.** A reference that leaves the
mapped corpus but stays inside the repository is written to the facts file as
`outbound`: the element that holds it, and the **repo-relative file it points
at**. Which *component* owns that file is deliberately not decided there — a
per-component extractor cannot know, and pretending otherwise would bake one
component's view of the world into its own facts.

**The workspace resolves ownership.** At load time each recorded path is
matched against every `source:` mapping; the owning component turns the raw
reference into a code dependency between components. That is compared with the
declared relations, lifted through the hierarchy: a container-level relation
covers what its components do, the same way the canvas lifts an edge to a
coarser altitude.

**Both findings are warnings**, and `validate --strict-drift` is how CI opts
in.

The alternative — extracting each language once over the union of all mapped
roots and attributing elements afterwards — gives exact edges and is
architecturally cleaner. It was rejected for now because it changes what a
facts file *is*: extraction stops being per-component, the staleness digest has
to span the union, and all three extractors change. Recording references keeps
the existing contract and is additive to the schema.

## Consequences

- Drift is reported in two directions. **Undeclared**: code reaches somewhere
  the model never says it does. **Unbacked**: the model declares a dependency
  and no code reference supports it — which usually means the relation points
  the wrong way.
- **Unbacked is only claimed between components in the same language.** A
  TypeScript canvas calling a Rust engine over IPC is a real relation that no
  static import will ever evidence, so its silence proves nothing. Without this
  rule the feature reports every cross-process dependency as drift.
- Absence of a mapping is never drift: a file no component claims is simply not
  introspected, and a component with no facts is not evidence of anything.
- Warnings, not errors, because a team turning this on for the first time on an
  existing repository would otherwise get a red build on day one.
- **It found real drift in this repository the first time it ran**: two
  undeclared dependencies from `git-service`, and a declared
  `model-service -> sync-engine` edge with no code behind it — the code
  dependency runs the other way. The model described a data flow where a
  relation means a dependency. All three are now corrected, which is the
  behaviour the feature is supposed to produce.
- Only Rust and TypeScript record outbound references today. C# resolves
  namespaces rather than paths at syntax level, so it has no file to record
  until semantic mode is involved — recorded, not solved.
