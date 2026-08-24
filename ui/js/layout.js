// Deterministic layout (ADR-0006): ELK layered with a fixed seed and
// model-order constraints, pins from the views file honoured as absolute grid
// positions. Pure module — the ELK instance is injected, so node tests can
// require elk.bundled.js and assert run-to-run determinism.

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

export function nodeSize(el) {
  const base = SIZES[el.kind] ?? SIZES.container;
  // meta line (tech) adds a row
  return el.tech ? { width: base.width, height: base.height + 14 } : { ...base };
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
 * Layout a computed view (from data.computeView) with optional pins
 * ({id: [gx, gy]} in grid units).
 *
 * Pin policy (Phase 1): pinned nodes sit exactly at their pinned grid
 * position; unpinned nodes are ELK-laid-out as a block, offset to start below
 * the pinned bounding box so the two groups never collide. Deterministic by
 * construction. The spec's softer "interactive hints" refinement arrives with
 * editing (Phase 3), when pins become writable.
 *
 * Returns { nodes: [{id, x, y, width, height}], edges: [{from, to, points, label, labelAt, direction, exact}] }.
 */
export async function layoutView(elk, view, pins = {}) {
  const pinned = view.nodes.filter((n) => pins[n.id]);
  const unpinned = view.nodes.filter((n) => !pins[n.id]);

  const graph = {
    id: 'root',
    layoutOptions: LAYOUT_OPTIONS,
    children: unpinned.map((el) => ({ id: el.id, ...nodeSize(el) })),
    edges: view.edges
      .filter((e) => !pins[e.from] && !pins[e.to])
      .map((e, i) => ({ id: 'e' + i, sources: [e.from], targets: [e.to] })),
  };

  const laid = unpinned.length ? await elk.layout(graph) : { children: [] };

  const nodes = [];
  let pinnedMaxY = 0;
  let pinnedMaxX = 0;
  for (const el of pinned) {
    const [gx, gy] = pins[el.id];
    const size = nodeSize(el);
    const x = gx * GRID;
    const y = gy * GRID;
    nodes.push({ id: el.id, x, y, ...size });
    pinnedMaxY = Math.max(pinnedMaxY, y + size.height);
    pinnedMaxX = Math.max(pinnedMaxX, x + size.width);
  }

  // Offset the auto block clear of the pinned block (below it), grid-snapped.
  const offsetY = pinned.length
    ? Math.ceil((pinnedMaxY + GRID * 2) / GRID) * GRID
    : GRID;
  const offsetX = GRID;
  const autoPos = new Map();
  for (const child of laid.children ?? []) {
    const x = child.x + offsetX;
    const y = child.y + offsetY;
    autoPos.set(child.id, { x, y });
    nodes.push({ id: child.id, x, y, width: child.width, height: child.height });
  }

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
    const pts = [sec.startPoint, ...(sec.bendPoints ?? []), sec.endPoint].map(
      (p) => ({ x: p.x + offsetX, y: p.y + offsetY })
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

  routeEdges(edges, nodes);
  placeLabels(edges, nodes);

  const width = Math.max(pinnedMaxX, ...nodes.map((n) => n.x + n.width), 0) + GRID;
  const height = Math.max(...nodes.map((n) => n.y + n.height), 0) + GRID;
  return { nodes, edges, width, height };
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
    const obstacles = nodes
      .filter((n) => n.id !== e.from && n.id !== e.to)
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
function placeLabels(edges, nodes) {
  const placed = [];
  const boxes = nodes.map((n) => ({ x: n.x, y: n.y, w: n.width, h: n.height }));
  const overlap = (a, b) => {
    const w = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
    const h = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
    return w > 0 && h > 0 ? w * h : 0;
  };
  for (const e of edges) {
    const text = e.protocol && e.label ? `${e.label} \u00b7 ${e.protocol}` : e.label ?? e.protocol;
    if (!text) continue;
    const w = text.length * 5.4 + 8; // ~10px font, worst-case advance
    const h = 14;
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
