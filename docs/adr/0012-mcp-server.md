---
doc: adr-0012
type: adr
status: accepted
elements: [blastradius.cli.mcp-server, blastradius.core.sync-engine]
---

# ADR-0012: MCP server — coding agents attach to the core

## Context

Coding agents (Claude Code and other MCP clients) work in the same repos our
workspaces live in. They can already run the CLI, but `snapshot` dumps the
whole model — wasteful in an agent's context window — and agents editing the
YAML directly re-serialize it, destroying comments and formatting. ADR-0005
made the core a library precisely so new heads attach cheaply.

## Decision

`blastradius mcp [workspace-dir]` — an MCP server inside the CLI binary,
speaking newline-delimited JSON-RPC 2.0 over stdio.

- **Hand-rolled protocol, no SDK.** The stdio transport needs `initialize`,
  `tools/list`, `tools/call`, and `ping`; a read-line loop serves it. An SDK
  would bring an async runtime for that. Same reasoning as vendored-libgit2
  and the hand-rolled Structurizr parser.
- **Task-shaped reads**, not model dumps: `workspace_summary`,
  `find_elements`, `element`, `blast_radius` (transitive dependents with
  distance, governing docs direct-vs-inherited, affected views — the query
  the product is named for), `validate`, `model_diff` (semantic, vs a git
  ref), `doc` (docs-as-model-objects, ADR-0010, ground agents in the ADRs
  that govern an element).
- **Writes go through the sync engine** (`apply_operation`, `undo`, `redo`):
  agent edits are the same validated, CST-preserving splices the canvas
  makes — indistinguishable from hand edits in the diff, sharing one undo
  history. Direct file edits by the agent still work; every tool call runs
  `external_scan` first, exactly like the app's watcher path.

## Consequences

- Building the write path against the real engine (no mock) surfaced two
  latent core bugs the frontend mock had hidden: renaming a *system* failed
  (`set_field` had no root-mapping path), and `relations:` in a context file
  was silently dropped by the parser even though the sync engine wrote
  person-relations there. Both fixed with regression tests — the dogfood
  argument for a second real consumer of every API.
- An app instance and an MCP server on the same workspace coexist through
  the file watcher (each sees the other's writes as external edits), but
  they **share the crash journal**; interleaved sessions make the journal
  discard itself on next open — safe (files are truth), just no restored
  undo. One writer at a time is the supported mode; revisit if agents
  become long-running.
- The tool list is part of the product surface now: renaming or reshaping a
  tool breaks agent workflows the way a CLI flag rename breaks scripts.
