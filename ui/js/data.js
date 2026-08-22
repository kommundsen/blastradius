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
 * Compute the elements + relations visible at one altitude.
 *
 * level: "L1" (scope ignored), "L2" (scope = system id), "L3" (scope =
 * container id). Relations are lifted to the deepest visible ancestor of each
 * endpoint and deduplicated; self-loops after lifting are dropped.
 */
export function computeView(snapshot, level, scope) {
  const els = byId(snapshot);
  const visible = new Map(); // id -> element

  const isContext = (el) => el.kind === 'person' || el.kind === 'external';

  if (level === 'L1') {
    for (const el of snapshot.elements) {
      if (el.kind === 'system' || isContext(el)) visible.set(el.id, el);
    }
  } else {
    const scopeDepth = depthOf(scope);
    const childDepth = scopeDepth + 1;
    for (const el of snapshot.elements) {
      if (el.id.startsWith(scope + '.') && depthOf(el.id) === childDepth) {
        visible.set(el.id, el);
      }
    }
    // Context and sibling elements join only if a relation touches the scope's
    // interior (include-context handling happens in the caller via views).
    for (const r of snapshot.relations) {
      for (const [a, b] of [[r.from, r.to], [r.to, r.from]]) {
        const inScope = a === scope || a.startsWith(scope + '.');
        if (!inScope) continue;
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
        if (el) visible.set(el.id, el);
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

  // Deterministic order (ADR-0006): id order for nodes, from|to for edges.
  const nodes = [...visible.values()].sort((a, b) => a.id.localeCompare(b.id));
  edges.sort((a, b) => (a.from + '|' + a.to).localeCompare(b.from + '|' + b.to));
  return { level, scope: level === 'L1' ? null : scope, nodes, edges };
}

/** The view definition (pins) matching a level+scope, if any. */
export function findViewDef(snapshot, level, scope) {
  return (
    snapshot.views.find(
      (v) => v.level === level && (level === 'L1' || v.scope === scope)
    ) ?? null
  );
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
  return { context, systems };
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
