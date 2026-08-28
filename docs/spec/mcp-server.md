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

**Reference and workflow ship separately** (0.6.3, `core::workflows`).
Reference loads on its own when architecture comes up, so it must not
interview; workflows are invoked deliberately and therefore may. Every agent
has a surface for them, in its own format — `.claude/commands/`,
`.github/prompts/*.prompt.md`, and `.agents/skills/*/SKILL.md`, which Cursor
and Codex both discover. Claude Code and Copilot additionally take a read-only
*subagent* (`blastradius-surveyor`) for the read-the-whole-repository pass;
where there is none, the model workflow surveys inline.

The per-agent paths and frontmatter keys are **checked against each vendor's
documentation, not remembered** — this first shipped asserting that only
Claude Code had a command surface, which was wrong for all three others. A
file at the wrong path with the wrong extension does nothing and reports
nothing, so the table in `core::workflows` is the contract and the tests pin
the frontmatter each vendor requires.

The reference itself stays one self-contained document per agent: it is what
gets read before anything happens, and splitting it only makes half of it easy
to miss. It is also *our* file, always —
`.github/instructions/blastradius.instructions.md` for Copilot rather than an
append into `copilot-instructions.md`, which belongs to the project, and
`.agents/blastradius.md` for Codex. Codex has no per-repo instructions file
but `AGENTS.md`, so that one takes the only part which has to auto-load: a
five-line pointer between `<!-- blastradius:begin -->` markers, removable by
deleting the block. Nothing is ever overwritten: each file is written only if
absent, and a repo set up by an earlier version — our text already inside
`copilot-instructions.md`, or the whole primer pasted into `AGENTS.md` — is
left exactly as it is rather than told the same thing twice.

**The hand-off names the workflow, not the task** (0.7.1). The prompt the app
and the CLI print is generated from what was actually selected: an invocation
of the *model* workflow when one was written (`/blastradius:model`,
`/blastradius-model`, or the `blastradius-model` skill, for the agents chosen),
the MCP instructions when tools were registered but no workflow, and a pointer
to `blastradius format` when neither. It shipped as one fixed sentence telling
the agent to model the repository straight away — which walked past the
interview the workflows exist to run, and named MCP tools that a
skills-only setup never registered. Both surfaces also list all three
workflows and how to start them; `sync` and `review` were written to disk and
never mentioned. Pinned by tests that derive the quoted invocations from the
files `workflows::files_for` actually writes.

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
