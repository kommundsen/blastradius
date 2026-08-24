# Navigating the canvas

The canvas is one continuous space, not a set of pages. Every view shows a
single altitude, and you move between altitudes by diving into a node or
rising out of it. The camera flies; nothing crossfades.

## Altitudes

| Level | Scope | Shows |
| --- | --- | --- |
| **L1** | the whole model | systems, plus the people and external systems around them |
| **L2** | one system | its containers |
| **L3** | one container | its components |
| **L4** | one component | modules and types derived from real source |
| **D** | one environment or node | deployment nodes and container instances |

L4 is enabled only when the model has source-mapped components
([Code-level detail](code-level.md)); **D** only when it declares environments
([Deployment views](deployment.md)).

## Moving around

- **Double-click** a node — or select it and press `Enter` — to dive into it.
- **`Esc`** rises one altitude. The breadcrumb shows where you are.
- **Arrow keys** move the selection through the nodes of the current view.
- The **level buttons** jump directly. Blastradius picks a sensible scope: the
  thing you have selected, otherwise the nearest candidate.
- Click a row in the left-hand tree to fly straight to that element at its own
  altitude, wherever you currently are.

Diving stops where there is nothing below: a component with no source mapping,
or a container instance, will not open.

## Camera

- **Scroll** to zoom about the pointer; **drag empty canvas** to pan.
- `+` / `-` zoom, `0` resets zoom and centres the view.
- The zoom control sits bottom-left; the middle button resets it.

If you have "reduce motion" set at the OS level, the flights collapse to cuts —
you land in exactly the same place.

## Reading a node

Every node carries three lines: its **kind** (and technology, if the model
records one), its **name**, and how many children it has. The kind is encoded
by shape rather than colour alone — colour is reserved for git and validation
status, so the diagram stays readable in greyscale and to colour-blind readers.

Edges are directed by default. A dashed edge is *aggregated*: it stands for one
or more relations between things deeper than the current altitude, lifted up so
you can see the dependency without diving.

## Panels

The left panel lists the whole model regardless of where the camera is; the
right panel inspects the current selection, showing its fields, relations, and
any [documents](model-format.md) attached to it. Both panels can be resized by
dragging their edge, or with the arrow keys when the grip has focus.

Selecting an edge inspects the relation instead, which is the quickest way to
see exactly what a line means.
