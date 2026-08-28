# Editing the model

Two ways in, one result: edit the YAML in your editor, or work on the canvas.
Both write the same files, and the app and your editor stay in step.

## On the canvas

- **`+ Element`** adds something at the current altitude — a system, person or
  external system at L1, a container at L2, a component at L3, and environments
  or deployment nodes in the **D** view.
- **Rename and describe** in the inspector: name, technology, and description
  are editable in place.
- **`R`**, then click a target, draws a relation from the selected element.
  Give it a label and a protocol so the line means something — the label reads
  on the line, the protocol under it in brackets, `calls` / `[JSON/HTTPS]`.
- **`Delete`** removes the selected element. You are told first what else goes
  with it — relations that pointed at it, and any layout pins.
- **`Ctrl+Z` / `Ctrl+Y`** undo and redo, including across a restart.

There is no save button. Every operation is written immediately, and the file
on disk is the only state.

## What the app writes

Edits are **splices**, not rewrites. Blastradius changes the exact bytes of the
field you touched and leaves everything else — comments, key order, blank
lines, quoting style — untouched. A one-word rename is a one-line diff.

Writes are atomic: a crash mid-write leaves either the old file or the new one,
never a half-written one. A write-ahead journal is replayed at startup, so undo
survives restarts and a torn write rolls forward.

## Editing the YAML directly

The file watcher reloads external edits live — save in your editor and the
canvas updates. You can also open the YAML inside the app: the **Source** tab
in the inspector shows the file with syntax highlighting, and validation errors
are marked on the offending line.

If a file changes on disk while you have an unsaved in-app buffer, the app
refuses to clobber it and tells you. Reload and reapply.

## Ids and names

The YAML key is the **id**, and it is immutable — it is what relations, layout
pins, and document links point at. To change what a thing is called, set
`name:`. Renaming an id by hand means updating every reference to it, and the
validator will list them for you.

## Layout and pinning

Layout is not stored in model files. Positions live in view files under
`views/`, so a diagram rearrangement never mixes into a semantic diff.

- **Drag a node** to pin it. It stays exactly there.
- **The first drag in a view settles that view.** Every other node is pinned
  where it already sits, so nothing but the node you moved appears to move.
  Without this, moving one node rearranged the diagram around it: a pinned
  node leaves the auto-layout, so what is left is a different graph and gets
  laid out afresh. It is one undo away — undo puts the whole view back to
  auto-layout, not just the node you dragged.
- Everything you have not pinned is **auto-laid-out**, deterministically —
  the same model produces the same diagram on every machine and in CI. After a
  view has settled, that applies to whatever you add next: a new element
  appears in clear space rather than reshuffling what you have arranged.
- Pins are grid units, not pixels, so they survive zoom and density changes.
- Dropping a node onto a neighbour nudges it to the nearest clear cell rather
  than overlapping.

Pin what you care about, and let the rest arrange itself — until you start
arranging, at which point that view is yours and stops moving under you.

## Validation

Problems appear as chips in the toolbar and badges on the offending node:
dangling relation targets, duplicate ids, references to elements that no longer
exist. The same check runs headlessly:

```
blastradius validate .
```

which is what you want in CI — a broken model fails the build instead of
quietly rendering wrong.
