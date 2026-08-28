// Deterministic layout (ADR-0006): ELK layered with a fixed seed and
// model-order constraints, pins from the views file honoured as absolute grid
// positions. Pure module — the ELK instance is injected, so node tests can
// require elk.bundled.js and assert run-to-run determinism.

import { descriptionHeight, edgeLabelLines } from './labels.js';

export const GRID = 26; // px per grid unit at 1x — the canvas dot pitch

const SIZES = {
  person: { width: 150, height: 66 },
  system: { width: 170, height: 66 },
  external: { width: 160, height: 62 },
  container: { width: 170, height: 70 },
  component: { width: 150, height: 62 },
  // Deployment (ADR-0018): infrastructure boxes run wider than components
  // because their names are machine names, not one-word identifiers.
  environment: { width: 180, height: 68 },
  'deployment-node': { width: 176, height: 66 },
  'container-instance': { width: 164, height: 62 },
};

/** Estimated box size. `description` is the text the box will draw at the
 *  bottom, when this view asks for it (spec §4) — passing it reserves the
 *  wrapped height, since a described box is materially taller. */
export function nodeSize(el, description = null) {
  const base = SIZES[el.kind] ?? SIZES.container;
  // meta line (tech) adds a row
  let height = base.height + (el.tech ? 14 : 0);
  if (description) height += descriptionHeight(description, base.width);
  return { width: base.width, height };
}

/** ELK options: deterministic via the fixed seed; model order is a
 * preference, not a constraint — forcing it measurably *added* crossings
 * (see docs/roadmap.md layout-polish note), and the seed alone keeps
 * run-to-run determinism. Spacing values are the auto-layout minimum
 * distances between nodes. */
const LAYOUT_OPTIONS = {
  'elk.algorithm': 'layered',
  'elk.direction': 'DOWN',
  'elk.randomSeed': '1',
  'elk.layered.considerModelOrder.strategy': 'PREFER_NODES',
  'elk.layered.spacing.nodeNodeBetweenLayers': '80',
  'elk.spacing.nodeNode': '48',
  'elk.edgeRouting': 'POLYLINE',
  'elk.layered.nodePlacement.strategy': 'BRANDES_KOEPF',
};

/**
 * Wrapping turns one very long chain into a snake of shorter columns
 * (`elk.layered.wrapping.strategy`). It is applied on a second pass, and only
 * when the first comes back far taller than it is wide with enough nodes for
 * that to hurt: a small diagram reading straight down is the C4 convention and
 * worth keeping, but sixteen chained components in a single 2400px column are
 * correct and unreadable at once.
 */
const WRAP_MIN_NODES = 8;
const WRAP_ASPECT = 2.5;
const WRAP_OPTIONS = {
  'elk.layered.wrapping.strategy': 'SINGLE_EDGE',
  'elk.aspectRatio': '1.6',
};

/**
 * Layout a computed view (from data.computeView) with optional pins
 * ({id: [gx, gy]} in grid units).
 *
 * Pin policy: pinned nodes sit exactly at their pinned grid position; unpinned
 * nodes are ELK-laid-out as a block, then placed at the *least* displacement
 * that clears every pinned box. It used to be unconditionally below the pinned
 * bounding box, which meant pinning one node near the bottom shoved the entire
 * rest of the diagram underneath it. Deterministic by construction: the
 * candidate order is fixed and the tie-break is on displacement.
 *
 * Returns { nodes: [{id, x, y, width, height}], edges: [{from, to, points, label, labelAt, direction, exact}],
 * groups: [{id, label, x, y, width, height, members}] }.
 */
export async function layoutView(elk, view, pins = {}, options = {}) {
  const pinned = view.nodes.filter((n) => pins[n.id]);
  const unpinned = view.nodes.filter((n) => !pins[n.id]);
  // Which boxes draw their description at the bottom (spec §4). Only sizing
  // needs it here: the renderers read `describe` off the finished node, so the
  // box that reserved the height is exactly the box that fills it.
  const describe = options.descriptions ?? new Set();
  // Real rendered sizes where the caller could measure them (the canvas can;
  // headless SVG rendering cannot). A `.node` is content-sized, so a name that
  // wraps to three lines — or a description — is taller than any per-kind
  // estimate, and layout that reserved the estimate leaves the overflow to
  // collide with whatever is below. Falls back to the estimate, which is what
  // the estimate is for.
  const sizeOf = (el) => {
    // Only the outer box: `sizes` entries also carry the measured breakdown of
    // a node's own chrome (below), which is nobody else's business — every
    // consumer spreads this into a laid-out node.
    const m = options.sizes?.get(el.id);
    if (m) return { width: m.width, height: m.height };
    return nodeSize(el, describe.has(el.id) ? el.description : null);
  };

  // Containment mode (ADR-0018): a deployment view drawn as boxes inside
  // boxes instead of one altitude at a time. The tree comes from the elements'
  // own `parent`, restricted to what is visible, and a node with children is
  // laid out by ELK as a compound and rendered as a node that happens to have
  // room inside it.
  const nestChildren = options.nested ? childrenByParent(unpinned) : null;

  // Groups that ELK can lay out as real compounds: every member unpinned.
  // A group with a pinned member cannot be one — pinned nodes never enter the
  // ELK graph — so it falls back to a box drawn round the finished geometry.
  const compoundGroups = options.groups ? compoundable(unpinned, pins, view) : new Map();

  const inCompound = new Set([...compoundGroups.values()].flat().map((el) => el.id));
  // A factory, not a literal: ELK writes coordinates into the graph it is
  // given, so the wrapping pass below needs its own untouched copy.
  const buildGraph = (extraOptions) => ({
    id: 'root',
    layoutOptions: {
      ...LAYOUT_OPTIONS,
      ...(compoundGroups.size || nestChildren?.kids.size
        ? { 'elk.hierarchyHandling': 'INCLUDE_CHILDREN' }
        : {}),
      ...extraOptions,
    },
    children: nestChildren
      ? unpinned
          .filter((el) => !nestChildren.parentOf.has(el.id))
          .map((el) => nestedNode(el, nestChildren, sizeOf, describe, options.sizes))
      : [
      ...unpinned.filter((el) => !inCompound.has(el.id)).map((el) => ({ id: el.id, ...sizeOf(el) })),
      ...[...compoundGroups.entries()].map(([label, members]) => ({
        id: groupId(label),
        layoutOptions: {
          'elk.padding': `[top=${GROUP_PAD.top},left=${GROUP_PAD.side},bottom=${GROUP_PAD.bottom},right=${GROUP_PAD.side}]`,
        },
        children: members.map((el) => ({ id: el.id, ...sizeOf(el) })),
      })),
    ],
    edges: view.edges
      .filter((e) => !pins[e.from] && !pins[e.to])
      .map((e, i) => ({ id: 'e' + i, sources: [e.from], targets: [e.to] })),
  });

  const laid = unpinned.length ? await runLayout(elk, buildGraph) : { children: [] };

  const nodes = [];
  for (const el of pinned) {
    const [gx, gy] = pins[el.id];
    nodes.push({ id: el.id, x: gx * GRID, y: gy * GRID, ...sizeOf(el) });
  }

  // Place the auto-laid block at the least displacement that clears every
  // pinned box, grid-snapped.
  const { x: offsetX, y: offsetY } = placeBlock(nodes, blockRects(laid.children));
  // ELK reports compound children *relative to their parent*, so the tree is
  // walked and absolutised. `origins` keeps each container's absolute origin,
  // which the edge sections below also need — a section is relative to its
  // own `container`, not to root.
  const origins = new Map([['root', { x: offsetX, y: offsetY }]]);
  const laidGroups = [];
  const absorb = (children, ox, oy) => {
    for (const child of children ?? []) {
      const x = child.x + ox;
      const y = child.y + oy;
      origins.set(child.id, { x, y });
      if (child.children?.length) {
        if (nestChildren) {
          // A container, not a boundary: it keeps its kicker, its name and its
          // dive. Pushed before its children so the DOM order paints it behind
          // them.
          nodes.push({ id: child.id, x, y, width: child.width, height: child.height, contains: true });
        } else {
          laidGroups.push({ id: child.id, x, y, width: child.width, height: child.height });
        }
        absorb(child.children, x, y);
      } else {
        nodes.push({ id: child.id, x, y, width: child.width, height: child.height });
      }
    }
  };
  absorb(laid.children, offsetX, offsetY);

  const nodeAt = new Map(nodes.map((n) => [n.id, n]));

  // Edge geometry. ELK sections cover auto-auto edges; edges touching a pinned
  // node get straight center-to-center lines clipped to node borders.
  const edges = view.edges.map((e) => {
    const from = nodeAt.get(e.from);
    const to = nodeAt.get(e.to);
    const points = straightEdge(from, to);
    const mid = points[Math.floor(points.length / 2) - 1];
    const mid2 = points[Math.floor(points.length / 2)];
    return {
      ...e,
      points,
      labelAt: {
        x: (mid.x + mid2.x) / 2,
        y: (mid.y + mid2.y) / 2 - 6,
      },
    };
  });

  // Prefer ELK's routed sections where available (auto-auto edges).
  const elkEdges = new Map();
  for (const edge of laid.edges ?? []) {
    const sec = edge.sections?.[0];
    if (!sec) continue;
    // Sections are relative to the edge's container, which is root for
    // cross-group edges but the group itself for edges wholly inside one.
    const o = origins.get(edge.container ?? 'root') ?? origins.get('root');
    const pts = [sec.startPoint, ...(sec.bendPoints ?? []), sec.endPoint].map(
      (p) => ({ x: p.x + o.x, y: p.y + o.y })
    );
    elkEdges.set(edge.sources[0] + '|' + edge.targets[0], pts);
  }
  for (const e of edges) {
    const routed = elkEdges.get(e.from + '|' + e.to);
    if (routed) {
      e.points = routed;
      const mid = midpointOf(routed);
      e.labelAt = { x: mid.x, y: mid.y - 6 };
    }
  }

  // Groups are computed from the finished geometry, so a group holds pinned
  // and auto-laid members alike (spec §3c). They are *not* nodes: every
  // consumer assumes a node is one element, one opaque content-sized box, and
  // a boundary is none of those.
  const groups = options.groups ? collectGroups(view, nodes, laidGroups) : [];

  routeEdges(edges, nodes);
  placeLabels(edges, nodes, groups);

  // Reframe around the content. Pins may be negative — a diagram has no
  // top-left corner, and clamping them to one made it a wall to pile things
  // against — so the finished geometry is translated to start at one grid
  // margin. `origin` is that translation: the canvas subtracts it again when
  // writing a pin, so what lands in the YAML stays in the model's own
  // coordinates and never drifts as the drawing grows.
  const extents = [...nodes, ...groups];
  const minX = Math.min(0, ...extents.map((n) => n.x));
  const minY = Math.min(0, ...extents.map((n) => n.y));
  const origin = { x: minX < 0 ? GRID - minX : 0, y: minY < 0 ? GRID - minY : 0 };
  if (origin.x || origin.y) {
    for (const n of extents) {
      n.x += origin.x;
      n.y += origin.y;
    }
    for (const e of edges) {
      for (const p of e.points) {
        p.x += origin.x;
        p.y += origin.y;
      }
      e.labelAt.x += origin.x;
      e.labelAt.y += origin.y;
    }
  }

  const width = Math.max(...extents.map((n) => n.x + n.width), 0) + GRID;
  const height = Math.max(...extents.map((n) => n.y + n.height), 0) + GRID;
  for (const n of nodes) {
    if (describe.has(n.id)) n.describe = true;
  }

  return { nodes, edges, groups, width, height, origin };
}

/**
 * One ELK pass, plus a wrapping pass when the first result is a tower.
 *
 * The choice is a pure function of the first result, so it is as deterministic
 * as the layout it picks between, and the wrapped result is kept only if it is
 * actually squarer — ELK declines to wrap some graphs, and a pass that changed
 * nothing should not change which geometry ships.
 */
async function runLayout(elk, buildGraph) {
  const first = await elk.layout(buildGraph());
  const box = bboxOfRects(blockRects(first.children));
  if (countNodes(first.children) < WRAP_MIN_NODES) return first;
  if (!box.width || box.height <= box.width * WRAP_ASPECT) return first;
  const wrapped = await elk.layout(buildGraph(WRAP_OPTIONS));
  const wbox = bboxOfRects(blockRects(wrapped.children));
  if (!wbox.width || !wbox.height) return first;
  return wbox.height / wbox.width < box.height / box.width ? wrapped : first;
}

function countNodes(children) {
  let n = 0;
  for (const c of children ?? []) n += c.children?.length ? countNodes(c.children) : 1;
  return n;
}

/** Top-level rectangles of a laid-out block: leaf nodes and compound boxes
 * alike. A compound's box contains its children, so this is enough to reason
 * about where the block as a whole can sit. */
function blockRects(children) {
  return (children ?? []).map((c) => ({ x: c.x, y: c.y, w: c.width, h: c.height }));
}

function bboxOfRects(rects) {
  if (!rects.length) return { x: 0, y: 0, width: 0, height: 0 };
  const x = Math.min(...rects.map((r) => r.x));
  const y = Math.min(...rects.map((r) => r.y));
  const right = Math.max(...rects.map((r) => r.x + r.w));
  const bottom = Math.max(...rects.map((r) => r.y + r.h));
  return { x, y, width: right - x, height: bottom - y };
}

/**
 * Where to put the auto-laid block relative to the pinned nodes.
 *
 * Five fixed candidates — leave it where ELK put it, or push it clear below,
 * right, above or left — and the cheapest that collides with nothing wins.
 * Pushing below always clears (every auto node then starts under every pinned
 * one), so there is always an answer; it is simply no longer the *only*
 * answer, which is what made pinning a single low node relocate the whole
 * diagram underneath it.
 */
function placeBlock(pinnedNodes, rects) {
  const natural = { x: GRID, y: GRID };
  if (!pinnedNodes.length || !rects.length) return natural;
  const pinnedRects = pinnedNodes.map((n) => ({ x: n.x, y: n.y, w: n.width, h: n.height }));
  const p = bboxOfRects(pinnedRects);
  const box = bboxOfRects(rects);
  const gap = GRID * 2;
  const up = (v) => Math.ceil(v / GRID) * GRID;
  const down = (v) => Math.floor(v / GRID) * GRID;
  const below = { x: natural.x, y: up(p.y + p.height + gap - box.y) };
  const candidates = [
    natural,
    below,
    { x: up(p.x + p.width + gap - box.x), y: natural.y },
    { x: natural.x, y: down(p.y - gap - (box.y + box.height)) },
    { x: down(p.x - gap - (box.x + box.width)), y: natural.y },
  ];
  let best = null;
  let bestCost = Infinity;
  for (const c of candidates) {
    if (rects.some((r) => pinnedRects.some((q) => rectsOverlap(shift(r, c), q)))) continue;
    const cost = Math.abs(c.x - natural.x) + Math.abs(c.y - natural.y);
    if (cost < bestCost) {
      bestCost = cost;
      best = c;
    }
  }
  return best ?? below;
}

const shift = (r, o) => ({ x: r.x + o.x, y: r.y + o.y, w: r.w, h: r.h });

function rectsOverlap(a, b) {
  return (
    Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x) > 0 &&
    Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y) > 0
  );
}

/**
 * DOM for the group boundaries of a layout. Shared by the app and the exported
 * viewer so the two cannot drift. Deliberately not `.node`: a group is one
 * label over many elements, sized to its members, drawn behind them, and
 * inert to pointer events — none of the node contract applies.
 */
export function groupDivs(layout, doc) {
  return (layout.groups ?? []).map((g) => {
    const div = doc.createElement('div');
    div.className = 'group-box';
    div.dataset.group = g.label;
    div.style.cssText =
      `left:${g.x}px;top:${g.y}px;width:${g.width}px;height:${g.height}px`;
    const label = doc.createElement('span');
    label.className = 'group-label';
    label.textContent = g.label;
    div.appendChild(label);
    return div;
  });
}

export const groupId = (label) => `group:${label}`;

/**
 * Grow each rendered boundary to cover its members' *actual* boxes.
 *
 * Layout sizes nodes from per-kind estimates, but a `.node` in the DOM is
 * content-sized: a long name wraps and the real box is taller. That mismatch
 * predates groups and is harmless until something has to enclose a node — so
 * the boundary is measured against the DOM once the members exist. The SVG
 * path needs none of this: there, nodes and boundaries both use the layout
 * numbers, so they agree by construction.
 */
export function fitGroupBoxes(container, layout) {
  const byId = new Map(
    [...container.querySelectorAll('[data-id]')].map((n) => [n.dataset.id, n])
  );
  for (const g of layout.groups ?? []) {
    const box = [...container.querySelectorAll('.group-box')].find(
      (b) => b.dataset.group === g.label
    );
    if (!box) continue;
    const members = g.members.map((id) => byId.get(id)).filter(Boolean);
    if (!members.length) continue;
    const l = Math.max(0, Math.min(g.x, ...members.map((n) => n.offsetLeft - GROUP_PAD.side)));
    const t = Math.max(0, Math.min(g.y, ...members.map((n) => n.offsetTop - GROUP_PAD.top)));
    const r = Math.max(g.x + g.width, ...members.map((n) => n.offsetLeft + n.offsetWidth + GROUP_PAD.side));
    const b = Math.max(g.y + g.height, ...members.map((n) => n.offsetTop + n.offsetHeight + GROUP_PAD.bottom));
    box.style.cssText = `left:${l}px;top:${t}px;width:${r - l}px;height:${b - t}px`;
  }
}

/** Containment padding: the same sides as a group boundary. Top and bottom
 * are computed per container instead (see `nestedNode`) — a container's own
 * chrome is content-sized, and a constant either wastes space or gets sat on.
 */
const NEST_PAD = { side: 16, bottom: 16 };

/** Clear space between a container's own chrome and what it holds. */
const NEST_GAP = 10;

/** Chrome estimates for the headless path, where nothing can be measured:
 * `.node` padding plus a kicker and a one-line name above, one meta line
 * below. Deliberately generous — under-reserving is what the measured path
 * exists to avoid. */
const CHROME_EST = { header: 44, meta: 15 };

/** Parent -> children among the visible nodes, plus the reverse. Only real
 * containment counts: an element whose parent is not on screen is a root here,
 * so a scoped view nests what it shows and nothing above it. */
function childrenByParent(nodes) {
  const visible = new Set(nodes.map((n) => n.id));
  const kids = new Map();
  const parentOf = new Map();
  for (const el of nodes) {
    if (!el.parent || !visible.has(el.parent)) continue;
    parentOf.set(el.id, el.parent);
    if (!kids.has(el.parent)) kids.set(el.parent, []);
    kids.get(el.parent).push(el);
  }
  return { kids, parentOf };
}

/** One node of the ELK containment tree: a leaf keeps its measured size, a
 * container is sized by ELK around what it holds. */
function nestedNode(el, tree, sizeOf, describe = new Set(), sizes = null) {
  const kids = tree.kids.get(el.id);
  const own = sizeOf(el);
  if (!kids?.length) return { id: el.id, ...own };

  // A leaf box grows to fit its contents; a container is sized by ELK from its
  // children and its padding, so its own chrome has to be *asked for* as
  // padding or whatever is inside sits on top of it. This was a constant
  // (`top: 52`), which a two-line kicker — `[DEPLOYMENT NODE: POWERSHELL]` in
  // the dogfood deployment view — overran, putting the container's own name
  // underneath its first child. The measured breakdown comes from the canvas;
  // headless callers fall back to the estimates above.
  const m = sizes?.get(el.id);
  const descH =
    describe.has(el.id) && el.description
      ? m?.desc ?? descriptionHeight(el.description, own.width)
      : 0;
  const top = (m?.header ?? CHROME_EST.header) + NEST_GAP;
  const bottom = NEST_PAD.bottom + (m?.meta ?? CHROME_EST.meta) + descH;

  return {
    id: el.id,
    layoutOptions: {
      'elk.padding': `[top=${top},left=${NEST_PAD.side},bottom=${bottom},right=${NEST_PAD.side}]`,
      // ELK sizes a compound from its children alone, so a container holding
      // one small box came out narrower than its own title line — which is
      // what made that kicker wrap in the first place. Never narrower than the
      // box it would be on its own.
      'elk.nodeSize.constraints': 'MINIMUM_SIZE',
      'elk.nodeSize.minimum': `(${own.width},0)`,
    },
    children: kids.map((k) => nestedNode(k, tree, sizeOf, describe, sizes)),
  };
}

/** Padding between a group's boundary and its members; `top` leaves room for
 * the label strip. */
const GROUP_PAD = { top: 28, side: 14, bottom: 14 };

/**
 * Groups ELK can lay out as real compound nodes: every member present and
 * unpinned. Returned in label order so the graph is deterministic (ADR-0006).
 *
 * A group with any pinned member is excluded on purpose. Pinned nodes never
 * enter the ELK graph, so a compound could not hold one — and a user who has
 * pinned a member has taken manual control of where it sits, which a box drawn
 * round the result should respect rather than override.
 */
function compoundable(unpinned, pins, view) {
  const free = new Map(unpinned.map((el) => [el.id, el]));
  const byLabel = new Map();
  for (const el of view.nodes) {
    if (!el.group) continue;
    if (!byLabel.has(el.group)) byLabel.set(el.group, []);
    byLabel.get(el.group).push(el);
  }
  const out = new Map();
  for (const label of [...byLabel.keys()].sort((a, b) => a.localeCompare(b))) {
    const members = byLabel.get(label);
    if (members.every((m) => free.has(m.id))) {
      out.set(label, members.map((m) => free.get(m.id)));
    }
  }
  return out;
}

/**
 * The boundaries to draw: ELK's own compound rectangles where it laid one out,
 * and a bounding box round the members otherwise (a group holding a pinned
 * node). Either way a group is *not* a node — every consumer assumes a node is
 * one element, one opaque content-sized box, and a boundary is none of those.
 */
function collectGroups(view, nodes, laidGroups) {
  const nodeAt = new Map(nodes.map((n) => [n.id, n]));
  const laidAt = new Map(laidGroups.map((g) => [g.id, g]));
  const byLabel = new Map();
  for (const el of view.nodes) {
    if (!el.group || !nodeAt.has(el.id)) continue;
    if (!byLabel.has(el.group)) byLabel.set(el.group, []);
    byLabel.get(el.group).push(nodeAt.get(el.id));
  }
  return [...byLabel.keys()]
    .sort((a, b) => a.localeCompare(b))
    .map((label) => {
      const members = byLabel.get(label);
      const laid = laidAt.get(groupId(label));
      const rect = laid ?? bboxOf(members);
      return {
        id: groupId(label),
        label,
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        members: members.map((m) => m.id).sort(),
      };
    });
}

/** Clamped at the canvas origin: a member pinned at [0,0] would otherwise push
 * its boundary off-canvas. */
function bboxOf(members) {
  const x = Math.max(0, Math.min(...members.map((m) => m.x)) - GROUP_PAD.side);
  const y = Math.max(0, Math.min(...members.map((m) => m.y)) - GROUP_PAD.top);
  const right = Math.max(...members.map((m) => m.x + m.width)) + GROUP_PAD.side;
  const bottom = Math.max(...members.map((m) => m.y + m.height)) + GROUP_PAD.bottom;
  return { x, y, width: right - x, height: bottom - y };
}


// ---- obstacle-avoiding edge routing (0.2.0, docs/roadmap.md theme 1) --------
// Edges touching pinned nodes bypass ELK (straight lines), and ELK's own
// routes don't know the pinned boxes exist — either way an edge can pass
// under a node. This post-pass reroutes any offending edge through a
// visibility graph over the inflated corners of every other node: pins stay
// exactly where the user put them, only the line moves. Deterministic —
// obstacle order, corner order, and the Dijkstra tie-break are all fixed.

const ROUTE_MARGIN = 12; // clearance kept around nodes when detouring

function routeEdges(edges, nodes) {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  for (const e of edges) {
    const from = byId.get(e.from);
    const to = byId.get(e.to);
    if (!from || !to || e.from === e.to) continue;
    // A container is a region, not an obstacle: every edge to something inside
    // it necessarily crosses it, and treating it as solid would send every
    // line on a detour round the box it is already inside.
    const obstacles = nodes
      .filter((n) => n.id !== e.from && n.id !== e.to && !n.contains)
      .map((n) => ({ x: n.x, y: n.y, w: n.width, h: n.height }));
    if (!obstacles.some((r) => polylineHitsRect(e.points, r))) continue;
    const detour = findDetour(from, to, obstacles);
    if (detour) e.points = detour;
  }
}

function polylineHitsRect(pts, r) {
  for (let i = 0; i + 1 < pts.length; i++) {
    if (segHitsRect(pts[i], pts[i + 1], r)) return true;
  }
  return false;
}

/** Does the open segment p→q pass through the rect's interior? Liang-Barsky
 * with the rect deflated by eps, so grazing a border never counts. */
function segHitsRect(p, q, r, eps = 0.5) {
  const x0 = r.x + eps, y0 = r.y + eps, x1 = r.x + r.w - eps, y1 = r.y + r.h - eps;
  if (x1 <= x0 || y1 <= y0) return false;
  const dx = q.x - p.x, dy = q.y - p.y;
  let t0 = 0, t1 = 1;
  for (const [den, num] of [[-dx, p.x - x0], [dx, x1 - p.x], [-dy, p.y - y0], [dy, y1 - p.y]]) {
    if (den === 0) { if (num < 0) return false; continue; }
    const t = num / den;
    if (den < 0) { if (t > t1) return false; if (t > t0) t0 = t; }
    else { if (t < t0) return false; if (t < t1) t1 = t; }
  }
  return t1 > t0;
}

/** Shortest clear polyline from `from` to `to` around `obstacles`: Dijkstra
 * over the visibility graph of inflated obstacle corners, with a fixed
 * per-hop penalty so fewer bends win among near-equal routes. Returns null
 * when no clear route exists (the caller keeps the straight line). */
function findDetour(from, to, obstacles) {
  const inflated = obstacles.map((r) => ({
    x: r.x - ROUTE_MARGIN, y: r.y - ROUTE_MARGIN,
    w: r.w + 2 * ROUTE_MARGIN, h: r.h + 2 * ROUTE_MARGIN,
  }));
  const start = center(from);
  const goal = center(to);
  const verts = [start, goal];
  for (const r of inflated) {
    verts.push(
      { x: r.x, y: r.y }, { x: r.x + r.w, y: r.y },
      { x: r.x, y: r.y + r.h }, { x: r.x + r.w, y: r.y + r.h },
    );
  }
  const clear = (a, b) => !inflated.some((r) => segHitsRect(a, b, r));

  const dist = new Array(verts.length).fill(Infinity);
  const prev = new Array(verts.length).fill(-1);
  const done = new Array(verts.length).fill(false);
  dist[0] = 0;
  for (;;) {
    let u = -1;
    for (let i = 0; i < verts.length; i++) {
      if (!done[i] && dist[i] < (u === -1 ? Infinity : dist[u])) u = i;
    }
    if (u === -1 || u === 1) break;
    done[u] = true;
    for (let v = 0; v < verts.length; v++) {
      if (done[v] || v === u) continue;
      if (!clear(verts[u], verts[v])) continue;
      const d = dist[u] + Math.hypot(verts[v].x - verts[u].x, verts[v].y - verts[u].y) + 30;
      if (d < dist[v]) { dist[v] = d; prev[v] = u; }
    }
  }
  if (dist[1] === Infinity) return null;

  const path = [];
  for (let i = 1; i !== -1; i = prev[i]) path.unshift(verts[i]);
  if (path.length < 2) return null;
  // arrowheads land on borders, not centers — clip the terminal segments
  path[0] = clipToBox(path[0], path[1], from);
  path[path.length - 1] = clipToBox(path[path.length - 1], path[path.length - 2], to);
  return path;
}

/** Label de-collision: try positions along each edge and keep the first one
 * whose text box clears every node and every already-placed label; when no
 * spot is fully clear (very short edges), take the least-bad one. Pure and
 * deterministic — candidate order and edge order are fixed. */
function placeLabels(edges, nodes, groups = []) {
  const placed = [];
  // Only a group's *label strip* is an obstacle, never its interior: the
  // interior is where its members and their edges live, and treating the whole
  // rect as occupied would stampede every intra-group label out of the box.
  const boxes = [
    // A container contributes only its label strip, for the same reason a
    // group does: its interior is where its members live.
    ...nodes.map((n) =>
      n.contains
        ? { x: n.x, y: n.y, w: n.width, h: GROUP_PAD.top }
        : { x: n.x, y: n.y, w: n.width, h: n.height }
    ),
    ...groups.map((g) => ({ x: g.x, y: g.y, w: g.width, h: GROUP_PAD.top })),
  ];
  const overlap = (a, b) => {
    const w = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
    const h = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
    return w > 0 && h > 0 ? w * h : 0;
  };
  for (const e of edges) {
    // The label and its bracketed technology stack on two lines (C4), so the
    // box is as wide as the wider line and as tall as the stack \u2014 measured
    // from the same strings the renderers draw.
    const lines = edgeLabelLines(e);
    if (!lines.length) continue;
    const w = Math.max(...lines.map((l) => l.length)) * 5.4 + 8; // ~10px font, worst-case advance
    const h = 2 + lines.length * 12;
    let best = null;
    let bestScore = Infinity;
    for (const t of [0.5, 0.42, 0.58, 0.34, 0.66, 0.26, 0.74, 0.18, 0.82]) {
      // offset 0/0 sits on the line (classic interrupt style); dy moves the
      // label beside it (short edges, where the knockout would erase the
      // whole line) and dx lets it extend sideways into open space when
      // neighbouring columns crowd a vertical edge
      for (const dy of [0, -14, 16, -26, 28, -38, 40]) {
        for (const dx of [0, -w / 2 + 8, w / 2 - 8]) {
          const at = pointAt(e.points, t);
          const p = { x: at.x + dx, y: at.y + dy };
          const box = { x: p.x - w / 2, y: p.y - h + 2, w, h };
          let score = Math.abs(t - 0.5) * 8 + Math.abs(dy) * 0.6 + Math.abs(dx) * 0.3;
          for (const b of boxes) score += overlap(box, b) * 3; // never sit on a node
          for (const b of placed) score += overlap(box, b) * 2;
          score += erasedFraction(e.points, box) * 900; // keep the line visible
          if (score < bestScore) {
            bestScore = score;
            best = { p, box };
          }
        }
      }
    }
    e.labelAt = { x: best.p.x, y: best.p.y - 6 };
    placed.push(best.box);
  }
}

/** Fraction of a polyline's length that the knockout stroke of a label box
 * would erase (box inflated by the stroke radius, path sampled). */
function erasedFraction(pts, box) {
  const pad = 4;
  const x0 = box.x - pad, y0 = box.y - pad, x1 = box.x + box.w + pad, y1 = box.y + box.h + pad;
  const samples = 24;
  let inside = 0;
  for (let i = 0; i <= samples; i++) {
    const p = pointAt(pts, i / samples);
    if (p.x > x0 && p.x < x1 && p.y > y0 && p.y < y1) inside++;
  }
  return inside / (samples + 1);
}

/** Point at arc-length fraction t of a polyline. */
function pointAt(pts, t) {
  let total = 0;
  for (let i = 0; i + 1 < pts.length; i++) total += Math.hypot(pts[i + 1].x - pts[i].x, pts[i + 1].y - pts[i].y);
  let target = total * t;
  for (let i = 0; i + 1 < pts.length; i++) {
    const seg = Math.hypot(pts[i + 1].x - pts[i].x, pts[i + 1].y - pts[i].y);
    if (target <= seg || i + 2 === pts.length) {
      const k = seg ? target / seg : 0;
      return { x: pts[i].x + (pts[i + 1].x - pts[i].x) * k, y: pts[i].y + (pts[i + 1].y - pts[i].y) * k };
    }
    target -= seg;
  }
  return pts[0];
}

function center(n) {
  return { x: n.x + n.width / 2, y: n.y + n.height / 2 };
}

/** Straight line between node borders (not centers) so arrowheads land on the box. */
function straightEdge(a, b) {
  const ca = center(a);
  const cb = center(b);
  return [clipToBox(ca, cb, a), clipToBox(cb, ca, b)];
}

/** Point where the segment from `from` (inside box) toward `to` exits the box. */
function clipToBox(from, to, box) {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  let t = 1;
  if (dx !== 0) {
    const edgeX = dx > 0 ? box.x + box.width : box.x;
    t = Math.min(t, (edgeX - from.x) / dx);
  }
  if (dy !== 0) {
    const edgeY = dy > 0 ? box.y + box.height : box.y;
    t = Math.min(t, (edgeY - from.y) / dy);
  }
  return { x: from.x + dx * t, y: from.y + dy * t };
}

/** Arc-length midpoint of a polyline (matches the design-system edge label rule). */
function midpointOf(pts) {
  let total = 0;
  const seg = [];
  for (let i = 1; i < pts.length; i++) {
    const len = Math.hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
    seg.push(len);
    total += len;
  }
  let want = total / 2;
  for (let i = 0; i < seg.length; i++) {
    if (want <= seg[i]) {
      const t = seg[i] ? want / seg[i] : 0;
      return {
        x: pts[i].x + (pts[i + 1].x - pts[i].x) * t,
        y: pts[i].y + (pts[i + 1].y - pts[i].y) * t,
      };
    }
    want -= seg[i];
  }
  return pts[pts.length - 1];
}
