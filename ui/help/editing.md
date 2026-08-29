# Editing the model

Two ways in, one result: edit the YAML in your editor, or work on the canvas.
Both write the same files, and the app and your editor stay in step.

## On the canvas

**Right-click a box** and everything the canvas can do to it is on the menu:

- **Connect to…** then click a target, to draw a relation. Give it a label and
  a protocol so the line means something — the label reads on the line, the
  protocol under it in brackets, `calls` / `[JSON/HTTPS]`.
- **Rename…** puts the cursor in the inspector's name field, which is where the
  name lives. Same for **Add a description…** when there is none yet.
- **Show / Hide description** draws the description inside the box, in this view
  only. See *Descriptions on the box* below.
- **Add a … inside…** creates a child — a container in a system, a component in
  a container, a node or an instance in a deployment node — and dives in after
  it, since it lives one altitude below the one you are looking at.
- **Unpin this element** and **Back to auto-layout** release layout. See
  *Layout and pinning*.
- **Delete…** removes it, telling you first what else goes with it: relations
  that pointed at it, and any layout pins.

The rest of the canvas:

- **`+ Element`** adds something at the current altitude — a system, person or
  external system at L1, a container at L2, a component at L3, and environments
  or deployment nodes in the **D** view.
- **The inspector** edits the element's own fields in place: name, technology,
  description, and the **group** it draws inside. A deployment node or
  container instance also has **replicas** — how many of it actually run — and
  a system can be marked **outside your control**, which draws it dashed.
  Emptying a field removes it rather than writing an empty value.
- **Code level** for a component is set up there too: point it at the folder
  its code lives in and run the extractor without leaving the app. See
  [Code-level detail](code-level.md).
- **`R`** and **`Delete`** are the keyboard versions of Connect and Delete, on
  the selected element.
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
- **Right-click a pinned box** to release just that one, or the canvas itself
  for **Back to auto-layout**, which releases every pin in the view in one
  action. One undo brings the whole arrangement back.
- Pins are grid units, not pixels, so they survive zoom and density changes.
- Dropping a node onto a neighbour nudges it to the nearest clear cell rather
  than overlapping.

Pin what you care about, and let the rest arrange itself — until you start
arranging, at which point that view is yours and stops moving under you.

## Descriptions on the box

An element's description is a field on the element, written once in the
inspector. Whether a *diagram* draws it is a separate choice, made per view:
right-click the box and pick **Show description**, and the text appears at the
bottom of it, under a hairline — where C4 puts it.

It is off by default and it is per view, both for the same reason. Nearly
everything in a real model has a description, so drawing them all would make
every existing diagram taller overnight; and the container that is a bare name
in the L2 overview is usually the one you want a paragraph on in the L3 view
that is about it.

Right-clicking a box with no description yet offers the inspector field
instead — there is nothing to show until there is something to say.

Like a pin, the choice is stored in the view file rather than the model, so it
never shows up in a semantic diff:

```yaml
descriptions: [core, sync-engine]
```

A described box is taller, and the layout accounts for it — on the canvas, in
the SVG and PNG exports, and in the exported HTML.

## Validation

Problems appear as chips in the toolbar and badges on the offending node:
dangling relation targets, duplicate ids, references to elements that no longer
exist. The same check runs headlessly:

```
blastradius validate .
```

which is what you want in CI — a broken model fails the build instead of
quietly rendering wrong.
