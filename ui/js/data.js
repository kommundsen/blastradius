// View derivation: from a workspace snapshot to the node/edge set of one
// C4 altitude. Pure functions — no DOM, no ELK — so node can test them.

/** First path segment = root element id. */
export function rootOf(id) {
  return id.split('.')[0];
}

export function depthOf(id) {
  return id.split('.').length;
}

/** Lift an element id to the given ancestor depth (1 = root). */
export function liftTo(id, depth) {
  return id.split('.').slice(0, depth).join('.');
}

export function byId(snapshot) {
  const map = new Map();
  for (const el of snapshot.elements) map.set(el.id, el);
  return map;
}

/**
 * The derived (L4) graph owning an id — the component itself or any
 * `<component>.src.*` element (spec/l4-introspection.md).
 */
export function derivedGraphFor(snapshot, id) {
  if (!id) return null;
  return (
    (snapshot.derived ?? []).find(
      (g) => id === g.component || id.startsWith(g.component + '.src.')
    ) ?? null
  );
}

/**
 * The L4 view: derived elements one nesting step below the scope (the
 * component shows its top-level modules/namespaces; a module shows its
 * types and submodules). Hierarchy comes from the explicit `parent` field —
 * fact ids may themselves contain dots, so dot-depth arithmetic is wrong here.
 */
function computeDerivedView(snapshot, scope) {
  const graph = derivedGraphFor(snapshot, scope);
  if (!graph) return { level: 'L4', scope, nodes: [], edges: [] };
  const atTop = scope === graph.component;
  const visible = new Map();
  for (const el of graph.elements) {
    if ((el.parent ?? null) === (atTop ? null : scope)) {
      visible.set(el.id, { ...el, derived: true, stale: graph.stale });
    }
  }

  const parentOf = new Map(graph.elements.map((e) => [e.id, e.parent ?? null]));
  const liftEndpoint = (id) => {
    for (let cur = id; cur; cur = parentOf.get(cur)) {
      if (visible.has(cur)) return cur;
    }
    return null;
  };

  const edges = [];
  const seen = new Map();
  for (const e of graph.edges) {
    const from = liftEndpoint(e.from);
    const to = liftEndpoint(e.to);
    if (!from || !to || from === to) continue;
    const back = seen.get(to + '|' + from);
    if (back) {
      back.direction = 'both';
      continue;
    }
    const key = from + '|' + to;
    const existing = seen.get(key);
    if (existing) {
      if (existing.label !== e.kind) existing.exact = false; // mixed kinds aggregate
      continue;
    }
    const edge = {
      from,
      to,
      label: e.kind,
      protocol: null,
      direction: 'forward',
      exact: from === e.from && to === e.to,
    };
    seen.set(key, edge);
    edges.push(edge);
  }

  const nodes = [...visible.values()].sort((a, b) => a.id.localeCompare(b.id));
  edges.sort((a, b) => (a.from + '|' + a.to).localeCompare(b.from + '|' + b.to));
  return { level: 'L4', scope, nodes, edges };
}

/**
 * Compute the elements + relations visible at one altitude.
 *
 * level: "L1" (scope ignored), "L2" (scope = system id), "L3" (scope =
 * container id), "L4" (scope = an opted-in component or a derived element;
 * nodes come from the committed facts, not the authored model). Relations are
 * lifted to the deepest visible ancestor of each endpoint and deduplicated;
 * self-loops after lifting are dropped.
 */
export function computeView(snapshot, level, scope, includeContext = true, nested = false, drift = []) {
  if (level === 'L4') return computeDerivedView(snapshot, scope);
  const els = byId(snapshot);
  const visible = new Map(); // id -> element

  const isContext = (el) => el.kind === 'person' || el.kind === 'external';

  // Containment (ADR-0018): a deployment view can show its whole subtree in
  // one frame instead of one altitude at a time. Only the depth changes —
  // everything below (relation lifting, context, ordering) is untouched, and
  // the renderer decides what nesting looks like.
  const deep = nested && level === 'LD';

  if (level === 'L1') {
    for (const el of snapshot.elements) {
      if (el.kind === 'system' || isContext(el)) visible.set(el.id, el);
    }
  } else if (level === 'LD' && !scope) {
    // The deployment overview: every environment, the physical counterpart
    // of L1 (ADR-0018). Diving into one shows its nodes.
    for (const el of snapshot.elements) {
      if (el.kind === 'environment') visible.set(el.id, el);
    }
    if (deep) {
      // Every environment plus everything inside it.
      const roots = [...visible.keys()];
      for (const el of snapshot.elements) {
        if (roots.some((r) => el.id.startsWith(r + '.'))) visible.set(el.id, el);
      }
    }
    // Plus the people and external systems the deployment actually touches —
    // unlike L1, which shows every context element, this is relation-driven:
    // an environment talking to a git host is part of the delivery picture,
    // a reviewer who never touches infrastructure is not.
    if (includeContext) {
      for (const r of snapshot.relations) {
        for (const [a, b] of [[r.from, r.to], [r.to, r.from]]) {
          if (!visible.has(rootOf(a))) continue;
          const el = els.get(rootOf(b));
          if (el && isContext(el)) visible.set(el.id, el);
        }
      }
    }
  } else {
    const scopeDepth = depthOf(scope);
    const childDepth = scopeDepth + 1;
    for (const el of snapshot.elements) {
      if (!el.id.startsWith(scope + '.')) continue;
      if (deep || depthOf(el.id) === childDepth) visible.set(el.id, el);
    }
    // Context and sibling elements join only when a relation touches the
    // scope's strict *interior* — a relation to the bare scope element has no
    // visible node to attach its edge to, so it must not pull anyone in
    // (that was the "island" bug: joined node, droppable edge).
    for (const r of snapshot.relations) {
      for (const [a, b] of [[r.from, r.to], [r.to, r.from]]) {
        if (!a.startsWith(scope + '.')) continue;
        const outside = b === scope || b.startsWith(scope + '.') ? null : b;
        if (!outside) continue;
        // Lift the outside endpoint to its most meaningful visible level:
        // same-system sibling stays at its own depth capped to scopeDepth,
        // anything else lifts to root.
        const lifted =
          rootOf(outside) === rootOf(scope)
            ? liftTo(outside, scopeDepth)
            : rootOf(outside);
        const el = els.get(lifted);
        if (el && (includeContext || !isContext(el))) visible.set(el.id, el);
      }
    }
  }

  // Lift relations onto visible elements.
  const liftEndpoint = (id) => {
    for (let d = depthOf(id); d >= 1; d--) {
      const cand = liftTo(id, d);
      if (visible.has(cand)) return cand;
    }
    return null;
  };

  const edges = [];
  const seen = new Map(); // from|to -> edge (first label wins, count aggregated)
  for (const r of snapshot.relations) {
    const from = liftEndpoint(r.from);
    const to = liftEndpoint(r.to);
    if (!from || !to || from === to) continue;
    const key = from + '|' + to;
    const back = seen.get(to + '|' + from);
    if (back && back.direction !== 'none') {
      // A lifted reverse relation makes the aggregate bidirectional.
      if (!(back.from === from && back.to === to)) back.direction = 'both';
      continue;
    }
    if (seen.has(key)) continue;
    const edge = {
      from,
      to,
      label: r.label ?? null,
      protocol: r.protocol ?? null,
      direction: r.direction,
      exact: from === r.from && to === r.to, // false = aggregated from deeper relations
    };
    seen.set(key, edge);
    edges.push(edge);
  }

  // Drift (ADR-0019): where the code and the model disagree. An **undeclared**
  // dependency joins the picture as a ghost edge — it is evidence rather than a
  // relation, and it must look unlike one until somebody declares it. An
  // **unbacked** relation is the opposite case, so it marks the edge that is
  // already there. Both carry the ids the finding was made at, which is the
  // altitude any fix has to be written at: lifting is for drawing.
  for (const d of drift) {
    const from = liftEndpoint(d.from);
    const to = liftEndpoint(d.to);
    if (!from || !to || from === to) continue;
    if (d.kind === 'unbacked') {
      const edge = seen.get(from + '|' + to) ?? seen.get(to + '|' + from);
      if (edge) edge.unbacked = { from: d.from, to: d.to };
      continue;
    }
    // Only a declaration in the *same direction* answers the finding. A
    // relation drawn the other way between the same two boxes is exactly the
    // case worth seeing — the model connects them, and not the way the code
    // does — so the ghost is drawn alongside it rather than swallowed by it.
    if (seen.has(from + '|' + to)) continue;
    const edge = {
      from, to, label: null, protocol: null, direction: 'forward',
      exact: from === d.from && to === d.to,
      drift: { from: d.from, to: d.to, via: d.via ?? null },
    };
    seen.set(from + '|' + to, edge);
    edges.push(edge);
  }

  // Deterministic order (ADR-0006): id order for nodes, from|to for edges.
  const nodes = [...visible.values()].sort((a, b) => a.id.localeCompare(b.id));
  edges.sort((a, b) => (a.from + '|' + a.to).localeCompare(b.from + '|' + b.to));
  return { level, scope: level === 'L1' ? null : scope ?? null, nodes, edges };
}

/** The view definition (pins) matching a level+scope, if any. */
export function findViewDef(snapshot, level, scope) {
  // The deployment overview carries no scope, and arrives as '' from the
  // snapshot but null from the canvas — normalize before comparing.
  const norm = (s) => s || null;
  return (
    snapshot.views.find(
      (v) => v.level === level && (level === 'L1' || norm(v.scope) === norm(scope))
    ) ?? null
  );
}

/** Every environment, in id order — the entry points to the deployment side. */
export function environments(snapshot) {
  return snapshot.elements.filter((e) => e.kind === 'environment');
}

/** Docs linked to an element id. */
export function docsFor(snapshot, elementId) {
  return snapshot.docs.filter((d) => d.elements.includes(elementId));
}

/** Elements grouped for the sidebar tree: context first, then systems. */
export function treeModel(snapshot) {
  const els = snapshot.elements;
  const context = els.filter((e) => e.kind === 'person' || e.kind === 'external');
  const systems = els
    .filter((e) => e.kind === 'system')
    .map((sys) => ({
      el: sys,
      containers: els
        .filter((e) => e.parent === sys.id)
        .map((c) => ({
          el: c,
          components: els.filter((e) => e.parent === c.id),
        })),
    }));
  // Deployment (ADR-0018) is a separate root: nodes nest arbitrarily, so the
  // tree recurses rather than assuming the model's fixed three tiers.
  const childrenOf = (id) => els.filter((e) => e.parent === id && e.kind !== 'component');
  const deployment = els.filter((e) => e.kind === 'environment').map(function walk(el) {
    return { el, children: childrenOf(el.id).map(walk) };
  });
  return { context, systems, deployment };
}

/**
 * Resolve a view's pin keys (scope-relative or absolute, per spec §4) onto the
 * full dotted ids used by computeView. Unresolvable pins are dropped here —
 * the Core already reports them as validation errors.
 */
export function resolvePins(viewDef, view) {
  if (!viewDef) return {};
  const visible = new Set(view.nodes.map((n) => n.id));
  const out = {};
  for (const [key, xy] of Object.entries(viewDef.layout ?? {})) {
    const scoped = viewDef.scope ? viewDef.scope + '.' + key : key;
    const id = visible.has(scoped) ? scoped : visible.has(key) ? key : null;
    if (id) out[id] = xy;
  }
  return out;
}

/**
 * The visible elements whose description this view draws inside their box
 * (spec §4), as a Set of full ids.
 *
 * Keys are written scope-relative exactly as pins are, so they are resolved
 * the same way — an id that resolves to nothing visible is simply not drawn,
 * which is what happens to a pin for an element this view does not show.
 */
export function resolveDescriptions(viewDef, view) {
  const out = new Set();
  if (!viewDef) return out;
  const visible = new Set(view.nodes.map((n) => n.id));
  for (const key of viewDef.descriptions ?? []) {
    const scoped = viewDef.scope ? viewDef.scope + '.' + key : key;
    const id = visible.has(scoped) ? scoped : visible.has(key) ? key : null;
    if (id) out.add(id);
  }
  return out;
}
