# Keyboard shortcuts

## Canvas

These apply when the canvas has focus — click it once, or dive, and it does.

| Key | Action |
| --- | --- |
| `→` `↓` | Select the next node |
| `←` `↑` | Select the previous node |
| `Enter` | Dive into the selected node |
| `Esc` / `Backspace` | Rise one altitude |
| `+` / `=` | Zoom in |
| `-` | Zoom out |
| `0` | Reset zoom and centre |

Selection wraps around at either end of the list.

## Anywhere

| Key | Action |
| --- | --- |
| `Ctrl`/`Cmd` `+K` | Find any element, relation or document |
| `?` / `F1` | Open or close this help |
| `Ctrl`/`Cmd` `+O` | Open a workspace folder |
| `Ctrl`/`Cmd` `+Z` | Undo |
| `Ctrl`/`Cmd` `+Y` or `Ctrl`/`Cmd` `+Shift` `+Z` | Redo |

In the find palette, `↑` and `↓` move through the results, `Enter` opens the
highlighted one, and `Esc` closes it.

## Editing

Only in an editable workspace, with something selected.

| Key | Action |
| --- | --- |
| `Delete` | Delete the selected element (asks first, and lists what else goes) |
| `R` | Start drawing a relation from the selected element |
| `Esc` | Cancel drawing a relation |

There is no save shortcut: edits are written to disk as you make them. See
[Editing the model](editing.md).

While the cursor is in a text field, the editing keys are inert — typing `R`
into a name field types an R.

## Mouse

| Action | Result |
| --- | --- |
| Double-click a node | Dive into it |
| Drag empty canvas | Pan |
| Scroll / pinch | Zoom about the pointer |
| Drag a node | Pin it there (the first drag in a view pins the rest where they are) |
| Click an edge | Inspect the relation |

## Panels

With a panel's resize grip focused, `←` and `→` resize it in steps. Drag it for
continuous resizing. Widths persist between sessions.
