---
doc: spec-sync-engine
type: spec
status: draft
elements: [blastradius.core.sync-engine, blastradius.core.model-service]
---

# Spec: bidirectional sync engine

Implements ADR-0008. The sync engine is the arbiter between three editing
surfaces — canvas, in-app YAML panel, external editors — and the files, which
are the only truth.

## State machine

Per workspace, the engine holds:

- **Disk state** — the files as last read.
- **Model** — the last *valid* parse of disk state (element graph + docs).
- **Staleness** — per-file flag: disk has content that does not parse.

```
            valid parse                    parse error
  IN-SYNC ──────────────▶ IN-SYNC   IN-SYNC ─────────▶ STALE(file)
     ▲                                                    │
     └────────────── next valid parse of that file ◀──────┘
```

While any model file is STALE: the canvas renders the last valid model with a
stale indicator; **canvas editing is disabled** (editing a picture of a file
that no longer parses would fork truth); the YAML panel and external editors
remain live. Staleness is **granular** (Phase 5): a stale *views* file
disables only pinning into that view — model semantics keep flowing, the
view's last-known pins are retained so the canvas holds steady, and every
other operation (including pinning other views) stays live.

## Inbound: text → model

One path for all text sources:

1. In-app panel keystrokes update an in-memory buffer; external file events
   (notify watcher) update disk state. Both feed the same debounced parse
   (~150ms idle).
2. Valid parse → new model → canvas re-renders (layout stability per
   ADR-0006).
3. Invalid parse → STALE; error with file+line to the panel footer and `.err`
   underline at the offending line.

Watcher writes that the engine itself just performed are recognised by content
hash and dropped — the echo-loop killer.

## Outbound: canvas → files

Canvas operations commit at operation boundaries only: node **drop** (never
during drag), dialog confirm, delete, relation endpoint drop.

Each operation becomes a **targeted CST edit**: the engine locates the owning
node in a lossless YAML concrete-syntax-tree (comments, key order, and
formatting preserved) and splices the minimal change. Full-file re-serialization
is forbidden — it would destroy user formatting and produce garbage diffs.

The carried-over duplicate-key requirement is **closed** (Phase 3): the loader
scans for YAML-level duplicate keys itself and reports the exact line — which
also fixed a worse latent behavior, since marked-yaml's default loader silently
kept the *last* duplicate with no diagnostic at all. The splice layer edits
text directly via markers, so no parser replacement was needed.

| Canvas operation | File touched | Edit |
| --- | --- | --- |
| Drag node to pin | views/*.yaml | upsert `layout.<id>: [x, y]` |
| Rename element | model file | set `name:` (id immutable, ADR-0003) |
| Create element | model file | insert mapping under parent, id from dialog |
| Delete element | model + views | remove node; remove layout entry; relations referencing it are removed **in the same transaction** and listed in the confirm dialog |
| Create/edit relation | model file | insert/update sequence item |

Writes are atomic (temp file + rename). If disk changed under an uncommitted
canvas operation (race with external editor), the operation is aborted with a
toast, never merged heuristically.

## Undo

One workspace-level history of file transactions, shared by all surfaces. Undo
restores the prior file content of the last transaction regardless of which
surface produced it. External edits enter history as transactions too, so
undo-past-an-external-edit is well-defined (it reverts the file, with a
distinct "external change" label in the history UI). Depth: 200 transactions.

`apply_batch` applies several operations as one undoable unit — the MCP
`apply_operations` tool, and the only sane way to model a repository from
scratch. Each operation still goes through `apply` in full, so every
intermediate state is a valid workspace and ordering matters; a refusal undoes
what already landed and truncates the redo tail, leaving the workspace exactly
as it was. On success the transactions coalesce into one history entry (per
file: the first `before`, the last `after`), so a single undo takes the batch
back.

History is journaled per workspace (JSONL under the OS cache dir) and
**replayed on open** (Phase 5): undo/redo depth survives restarts and crashes.
Every write batch is bracketed write-ahead (`intent` … `commit`); recovery
rolls a torn trailing transaction forward when disk sits part-way through its
writes. If the files changed while the app was closed, the journal is
discarded whole — files are the truth and recovery never guesses. The journal
is compacted to the adopted history on every open, bounding its size.

## The IPC surface (Tauri commands)

As shipped (v1): `workspace_snapshot` · `sync_status` (staleness, undo/redo
availability, editable files) · `apply_operation(op)` · `undo_op` / `redo_op` ·
`file_text(rel)` · `buffer_update(rel, text)` — plus the Phase 2 git surface.
One event, `workspace-changed`, prompts the WebView to re-request everything;
the originally sketched fine-grained events (`model_updated`, `file_stale`,
`transaction_applied`) were not needed at this scale and are not planned unless
profiling demands them. Phase 5 onboarding added runtime workspace switching:
`workspace_open(path)` (plus `workspace_init(path, agents)` scaffolding into an
empty folder, `workspace_demo()` for a throwaway sample, and `pick_folder()`
for the native dialog). A switch retires the old watcher by generation counter
and drops the engine; the frontend resets all state and reloads.

**Opening detects rather than fails.** `workspace_open` answers one of three
shapes: `{opened}`, `{candidates}` when a repository holds several workspaces,
or `{empty, git}` when it holds none. That last one used to be an error naming
the folder the user had just picked, which — for someone pointing the app at
their own repository for the first time — was the entire first-run experience
(docs/roadmap.md, first-user findings). The frontend turns it into an offer to
scaffold, with `agents: {mcp: [...], skills: [...]}` naming which integrations
to write through `core::onboard` — the same choice `blastradius init` offers,
per part and per agent — and returns the sample prompt to hand over
afterwards.

**Existing files are kept, never fatal** (`scaffold::scaffold_into`, shared
with the CLI). 0.6.0 shipped with the opposite: any pre-existing file aborted
the scaffold, and the starter set includes `README.md`, so the offer failed on
every real repository. The app left its dialog open having written nothing and
never reached the agent setup; `blastradius init .` wrote four files, printed
"refusing to overwrite", exited 2, and skipped it too. Reported 2026-08-26 and
fixed the same day; the response now carries `created` and `kept` so the UI
can say which of the user's files it left alone. The app writes the *absolute path* of the CLI beside
it as the server command: a Store install has an execution alias on PATH but a
portable install has nothing, and a server that cannot start is
indistinguishable from one that was never registered.

This surface is also the future CLI/CI attachment point (ADR-0005): validate
and export must be callable without a WebView.

## Performance budgets

- Parse + validate a 500-element workspace: < 50ms (release build).
- Keystroke → canvas update (valid edit): < 250ms end to end.
- Canvas drop → file write: < 30ms.

**Enforced in CI** (Phase 5) against a generated ~510-element benchmark
workspace (`scaffold::benchmark_workspace`): the `budgets` job runs
crates/blastradius-core/tests/budgets.rs on a release build (parse+validate
< 50ms, canvas drop → write < 30ms, best-of-5). The keystroke budget spans
two processes and is enforced in shares: the core share (write + reparse +
validate) at < 150ms in the same suite, and the render share (ELK layout +
DOM for the current view) at < 100ms in WebKit by ui/tests/e2e/perf.spec.mjs.
Debug builds skip the suite — budgets are release-build contracts.
