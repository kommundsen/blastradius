# Coding agents (MCP)

Blastradius ships an MCP server, so a coding agent can read and edit the
architecture model as a first-class tool rather than by grepping YAML. If your
agent keeps the code and the model in the same change, the diagram stops
rotting.

## Setting it up

Opening a folder that has no workspace offers this along with the starter
model. Pick the pieces you want — the **MCP server**, the **skills and
instructions**, or both — and which of the four agents get them, then you are
handed a prompt to paste. Everything is on by default.

`blastradius init` does the same from the command line, for Claude Code,
GitHub Copilot, Cursor, and Codex, in each one's native format. For an
existing workspace:

```
blastradius init --agents claude,copilot --no-git
```

Existing config files are merged, never overwritten.

Your agent may need restarting to see a newly registered server, and Claude
Code asks you to approve a project's MCP server the first time.

To register by hand, the server is:

```
blastradius mcp
```

speaking MCP over stdio, with the workspace path as its argument.

## What the agent gets

**Reading** — task-shaped, not file-shaped:

- `workspace_summary` — orientation: element counts, systems, views,
  validation state. The right first call.
- `blast_radius` — everything affected by an element: dependents, dependencies,
  governing documents, the views it appears in. Worth calling *before* changing
  or deleting anything.
- `element`, `find_elements` — detail and search.
- `doc` — the ADRs and specs governing an element.
- `model_diff` — what changed between two commits, semantically.
- `model_format` — the schema, authoritative for the build in front of it, so
  an agent never has to infer the format from a sample file. `blastradius
  format` prints the same thing.

**Writing** — `apply_operation` routes through the same engine the canvas uses:
the YAML is spliced in place, comments and formatting survive, the result is
validated before writing, and it is undoable. An agent editing the model this
way cannot leave it malformed. `apply_operations` takes a whole list as one
transaction, which is how an agent models a repository from scratch without a
hundred round trips — and one undo takes it all back if you hate the result.

**Git** — `git_status`, `git_conflicts`, and `resolve_conflicts` let an agent
resolve a merge conflict per element. Files are validated and staged through
your own git; the commit stays yours.

**Code level** — `introspect` refreshes derived facts. Derived elements answer
in `find_elements`, `element`, and `blast_radius` marked `derived: true`, so an
agent asking for the blast radius of a module gets real code-level fan-in.

## Workflows, for Claude Code

Reference and workflow are different shapes, so they ship as different things.
The **skill** is reference — it loads on its own when architecture comes up,
which is exactly why it must not start asking questions. The **commands** are
yours to invoke, so they can:

- **`/blastradius:model`** — builds or extends the model by interviewing you
  first: scope, how much detail, whether to attach ADRs and other docs,
  whether components should point at their source for code-level detail, and
  whether there is a deployment story worth drawing. On a repository that
  already has code it surveys first and brings you a proposal to correct;
  on an empty one it has nothing to read, so it asks properly.
- **`/blastradius:sync`** — finds where the model has drifted from the code
  since a given commit and proposes the fixes.
- **`/blastradius:review`** — judges the model against the repository and
  reports, without changing anything.

There is also a **`blastradius-surveyor`** subagent, read-only, which reads a
whole repository and proposes a C4 structure with the evidence for each
candidate. `/blastradius:model` launches it for you; it gets its own context
window so that reading the repository does not crowd out the conversation.

Cursor, Copilot and Codex have no equivalent surfaces, so they get one
self-contained instructions file instead — the same reference, without the
workflows.

## The rules the agent is told

The generated instructions tell your agent to keep the model in step with
reality, to edit through `apply_operation` rather than by hand, to treat
element ids as immutable (rename via `name:`), and to keep document
`elements:` links pointing at real ids. If it does edit YAML directly — which
is allowed; the files are the truth — it is told to read `model_format` first
rather than guess, never to re-serialize or reorder a file, and to run
`validate` immediately.

They also carry a short set of C4 dos and don'ts, because an agent that knows
the file format can still model badly: stop at components and let `introspect`
derive what is below; a relation is a dependency, not a data flow; put
technology in `tech:` instead of in names; model what a reader needs rather
than everything that exists.

## Privacy

The server is local, over stdio. It reads your workspace and reports what you
ask it for; it does not phone anywhere. What your *agent* does with the
information is between you and your agent vendor — see [Privacy](privacy.md).
