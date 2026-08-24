# Coding agents (MCP)

Blastradius ships an MCP server, so a coding agent can read and edit the
architecture model as a first-class tool rather than by grepping YAML. If your
agent keeps the code and the model in the same change, the diagram stops
rotting.

## Setting it up

`blastradius init` offers to register the server and write instructions for
Claude Code, GitHub Copilot, Cursor, and Codex, in each one's native format.
For an existing workspace:

```
blastradius init --agents claude,copilot --no-git
```

Existing config files are merged, never overwritten.

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

**Writing** — `apply_operation` routes through the same engine the canvas uses:
the YAML is spliced in place, comments and formatting survive, the result is
validated before writing, and it is undoable. An agent editing the model this
way cannot leave it malformed.

**Git** — `git_status`, `git_conflicts`, and `resolve_conflicts` let an agent
resolve a merge conflict per element. Files are validated and staged through
your own git; the commit stays yours.

**Code level** — `introspect` refreshes derived facts. Derived elements answer
in `find_elements`, `element`, and `blast_radius` marked `derived: true`, so an
agent asking for the blast radius of a module gets real code-level fan-in.

## The rules the agent is told

The generated instructions tell your agent to keep the model in step with
reality, prefer `apply_operation` over hand-editing, treat element ids as
immutable (rename via `name:`), and keep document `elements:` links pointing at
real ids. If it does edit YAML directly, it is told not to re-serialize or
reorder keys, and to run `blastradius validate` afterwards.

## Privacy

The server is local, over stdio. It reads your workspace and reports what you
ask it for; it does not phone anywhere. What your *agent* does with the
information is between you and your agent vendor — see [Privacy](privacy.md).
