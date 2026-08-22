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
| `apply_operation` | write | sync-engine transaction: create / rename / set-field (name·description·tech) / delete / add-relation / delete-relation / set-relation-field / pin — validated before writing, refused if it would invalidate the workspace |
| `undo` / `redo` | write | the shared transaction history |

Every call starts with `external_scan()` so direct file edits by the agent
(or anyone) are picked up first — the same inbound path as the app watcher,
echo suppression included. Unknown element ids answer with did-you-mean
suggestions instead of bare errors.

## Registration

Claude Code: `claude mcp add blastradius -- blastradius mcp <workspace-dir>`
(or with `cargo run -q -p blastradius-cli -- mcp` from this repo). Any
MCP-over-stdio client works the same way.

## Concurrency boundary (v1)

App and MCP server on one workspace: writes flow through each other's
external-edit path correctly, but the crash journal is shared and
interleaving makes it self-discard on next open (no restored undo; files
are never harmed). Supported mode is one writer at a time — recorded in
ADR-0012, revisit if agents become long-running daemons.
