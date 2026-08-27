## Blastradius architecture model

This repository contains a Blastradius workspace in `docs/` — a C4
architecture model as plain YAML (blastradius.yaml + model/ + views/),
versioned like source code.

## Reading the model

Use the `blastradius` MCP tools. Start with `workspace_summary`; call
`blast_radius` with an element id before changing or deleting anything
it models; `doc` returns the ADRs and specs governing an element.

## Editing the model

**Edit through `apply_operation`, or `apply_operations` for several
changes at once.** Not a style preference: those tools splice the YAML
in place so comments, key order and formatting survive, they validate
before writing and refuse anything that would break the workspace, and
they are undoable. Hand-written YAML has none of that, and the
operation shapes are published in each tool's input schema.

Modelling a repository from scratch is a single `apply_operations`
call with the whole list — create parents before children, elements
before the relations between them. It applies as one transaction: if
any operation is refused the rest roll back, and one `undo` reverts
the lot.

**Never guess the schema from an existing file.** Call `model_format`
(or run `blastradius format`) for the authoritative reference: every
element kind, what may nest in what, relations, views, docs
frontmatter, deployment, and a complete example. If you do edit YAML
by hand, read that first, never re-serialize or re-order a file, and
run `validate` immediately — before moving on to anything else.

## Modelling rules

Getting the format right is not the same as modelling well.

- **Stop at components.** Below them is derived from source by `introspect`.
  Hand-modelling classes and functions is the classic way to build a model
  nobody maintains.
- **A relation is a dependency, not a data flow.** Blastradius's own model got
  this wrong — a relation labelled "parse results" pointed one way while the
  code dependency ran the other, and drift detection caught it. Ask "which one
  would break if the other changed?" and point from the answer.
- **Model what a reader needs to reason about**, not everything that exists. A
  container diagram with forty boxes has failed at its job.
- **Name things after what they are**, and put technology in `tech:` — a
  container called "Redis Cache" should be "Cache" with `tech: Redis`.
- **One system per file**, named for the system id.
- **Attach documents to the elements they govern.** A doc naming an element
  that no longer exists is a model error, and validation says so.
- **Run `validate` before claiming to be done**, and `blast_radius` on
  anything you are about to change or delete.

## Keeping it honest

- Element ids (the YAML keys) are immutable — renaming means changing
  the `name:` field only.
- Markdown docs with a `doc:` frontmatter block are part of the model;
  their `elements:` links must point at real element ids.
- When you add, remove, or rewire a real component, mirror it in the
  model in the same change.
- Components with a `source:` mapping have derived L4 code elements
  (modules/types extracted from source). They answer in
  `find_elements`, `element`, and `blast_radius` (code-level
  fan-in), marked `derived: true` — read-only; edit the source
  instead, then run the `introspect` tool to refresh the committed
  facts. `blast_radius` on a derived id shows real code dependents.
- `git_status` and `git_conflicts` read repository state. A merge
  conflict in the model resolves per element: read `git_conflicts`
  (each conflicted element carries ours/theirs field values), then
  call `resolve_conflicts` with {elements: {"<id>": "ours"|"theirs"}}
  — choices splice onto the chosen side (comments survive), files
  are validated and staged via the user's own git, and the commit
  stays the user's. Anything undecided keeps ours.

If the `blastradius` MCP tools are not available, say so rather than
working around it: `blastradius validate docs` and `blastradius
format` still work from the command line.
