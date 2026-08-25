---
doc: spec-mcp-server
type: spec
status: current
elements: [blastradius.cli.mcp-server]
---

# Spec: MCP server

Implements ADR-0012. `blastradius mcp [dir]` — dir defaults to `.`, falling
back to `./docs` (the dogfood layout), erroring with a pointer to
`blastradius init` otherwise. Transport: newline-delimited JSON-RPC 2.0 on
stdio; logs on stderr; protocol version `2025-06-18` (the client's echoed
back). Capabilities: tools only.

## Tools

| Tool | Kind | Contract |
| --- | --- | --- |
| `workspace_summary` | read | orientation: counts by kind, systems + containers, views, docs, staleness, error count |
| `find_elements` | read | substring over id/name/description + kind filter; ≤ 50 briefs with `file:line` |
| `element` | read | one element: fields, children, resolved in/out relations, governing docs, views |
| `blast_radius` | read | transitive dependents (reverse reachability, with distance), contents, direct dependencies, docs (`directly` / `via parent`), affected views |
| `validate` | read | fresh parse from disk; PASS/FAIL + every diagnostic with file+line |
| `model_diff` | read | semantic diff vs git ref (default: merge-base), elements/relations/layout separated |
| `doc` | read | doc metadata + markdown body (frontmatter stripped) by doc id |
| `model_format` | read | the workspace format, authoritative for this build: kinds and nesting, relations, views, docs frontmatter, deployment, groups, modelling rules, and a worked example |
| `apply_operation` | write | sync-engine transaction: create / rename / set-field (name·description·tech) / delete / add-relation / delete-relation / set-relation-field / pin — validated before writing, refused if it would invalidate the workspace |
| `apply_operations` | write | a list of the same operations as one transaction: each is applied in full, so ordering matters; any refusal rolls the batch back, and one `undo` reverts all of it |
| `undo` / `redo` | write | the shared transaction history |

Every call starts with `external_scan()` so direct file edits by the agent
(or anyone) are picked up first — the same inbound path as the app watcher,
echo suppression included. Unknown element ids answer with did-you-mean
suggestions instead of bare errors.

`apply_operation`'s input schema carries one `oneOf` branch per `Operation`
variant, with the field names, enums and required keys — not a bare
`{"op": {"type": "object"}}` with the shapes in prose, which is what it was
through 0.5.0.

### Writing for an agent that has never seen a Blastradius workspace

The last three rows exist because of what the first outside user's agent
did (docs/roadmap.md, first-user findings). It queried the model through
these tools correctly, then hand-wrote YAML and looped on validation
errors — not misbehaviour: files are the source of truth (ADR-0008) and
external edits are first-class. It had nothing to write *against*.
`spec/model-format.md` is in this repository, not the user's; no tool
returned the format; the write tool's schema did not describe its own
input; and building a model from scratch meant dozens of single calls, so
writing one file was the rational choice.

The same reference is served by `model_format`, printed by `blastradius
format`, and embedded in the generated skill, from one constant in
`format_ref.rs`. Its worked example is written to disk and validated by a
test — an example that does not load is worse than none, being exactly
what an agent with no other reference will imitate.

## Registration

`blastradius init` offers to write project-scoped registration during
onboarding — `.mcp.json` (Claude Code), `.vscode/mcp.json` (Copilot/VS
Code), `.cursor/mcp.json` (Cursor), `.codex/config.toml` (Codex; loads only
for trusted projects) — plus per-agent skills/instructions. All writes are
merge-only. Manually: `claude mcp add blastradius -- blastradius mcp
<workspace-dir>`; any MCP-over-stdio client works the same way.

## Concurrency boundary (v1)

App and MCP server on one workspace: writes flow through each other's
external-edit path correctly, but the crash journal is shared and
interleaving makes it self-discard on next open (no restored undo; files
are never harmed). Supported mode is one writer at a time — recorded in
ADR-0012, revisit if agents become long-running daemons.
