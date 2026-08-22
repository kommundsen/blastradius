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
remain live. Views files being stale disables only pinning, not semantics.

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

Carried-over requirement for the CST layer: the Phase 0 reader (marked-yaml)
cannot attach a line number to a *YAML-level duplicate key* (its error variant
exposes no position — see `load_error_line` in `blastradius-core/src/yaml.rs`),
so that one diagnostic currently reports line 0. Whatever parser backs the CST
must report duplicate keys with exact positions, closing the gap rather than
inheriting it.

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
distinct "external change" label in the history UI). Depth: 200 transactions;
history is in-memory per session, with the log journaled to the workspace
cache dir for crash recovery.

## The IPC surface (Tauri commands)

`workspace_open(path)` · `workspace_snapshot() → model+docs+errors` ·
`apply_operation(op) → transaction` · `undo()` / `redo()` ·
`buffer_update(file, text)` (panel keystrokes) · events:
`model_updated`, `file_stale`, `transaction_applied`.

This surface is also the future CLI/CI attachment point (ADR-0005): validate
and export must be callable without a WebView.

## Performance budgets

- Parse + validate a 500-element workspace: < 50ms (release build).
- Keystroke → canvas update (valid edit): < 250ms end to end.
- Canvas drop → file write: < 30ms.

Budgets are CI-enforced against a generated benchmark workspace.
