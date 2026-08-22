---
doc: adr-0008
type: adr
status: accepted
elements: [blastradius.core.sync-engine]
---

# ADR-0008: Bidirectional sync — files are truth, edits are transactions

## Status
Accepted — 2026-08-22

## Context
v1 is a full bidirectional editor: the same model is editable on the canvas,
in the in-app YAML panel, and in any external editor simultaneously. Naive
two-way binding produces feedback loops, lost keystrokes, and corrupted files.
This is the hardest subsystem in v1 and needs its rules fixed before code.

## Decision
The **files are the single source of truth**; every editing surface is a view
that proposes transactions against them. The sync engine (Rust core)
arbitrates:

- **Canvas edits** commit at operation boundaries — drop (not drag), dialog
  confirm, delete — as targeted AST edits to the owning file, preserving
  untouched formatting and comments. Never a full-file re-serialize.
- **Text edits** (in-app panel or external, via the watcher) parse on idle
  (~150ms debounce). A valid parse updates the model and re-renders. An
  invalid parse keeps the canvas on the **last valid model**, marked stale,
  with the error surfaced inline (`.err`) and in the panel footer — the canvas
  never goes blank because the user is mid-keystroke.
- **Canvas is read-only while stale**: editing a picture of a file that no
  longer parses would fork truth. Fix the text (or undo it) and editing
  resumes.
- Undo is a single history of file-level transactions, shared by all surfaces
  — undoing a canvas drag after a YAML edit undoes the YAML edit first,
  exactly like a text editor with two panes.

## Consequences
- Comment- and format-preserving YAML editing requires a CST-based editor
  (tree-sitter-yaml or equivalent) rather than serde round-tripping — a
  significant, named engineering cost (spec/sync-engine.md).
- External and internal edits get one code path (watcher event vs buffer
  event), which kills the classic echo-loop bug by design.
- The transaction log gives crash recovery and an audit trail nearly for free.
