// Finding things in the model (docs/roadmap.md 0.7.0). An agent talking to the
// MCP server has had `find_elements` since 0.5.0; a human in the app had the
// sidebar tree and nothing else, which stops scaling at exactly the monorepo
// size this is built for.
//
// Pure module — no DOM, no snapshot mutation — so the ranking is tested in
// node rather than through the canvas.

import { derivedGraphFor } from './data.js';

/** Rank buckets, best first. Lower is better; ties break on the sort key, so
 * the same query over the same model always lists the same order. */
const EXACT = 0;
const NAME_PREFIX = 1;
const ID_PREFIX = 2;
const NAME_PART = 3;
const ID_PART = 4;
const TEXT_PART = 5;

function rank(q, { name, id, text }) {
  const n = (name ?? '').toLowerCase();
  const i = (id ?? '').toLowerCase();
  if (n === q || i === q) return EXACT;
  if (n.startsWith(q)) return NAME_PREFIX;
  if (i.startsWith(q)) return ID_PREFIX;
  if (n.includes(q)) return NAME_PART;
  if (i.includes(q)) return ID_PART;
  if ((text ?? '').toLowerCase().includes(q)) return TEXT_PART;
  return null;
}

const KIND_LABEL = {
  person: 'Person',
  system: 'System',
  external: 'External system',
  container: 'Container',
  component: 'Component',
  environment: 'Environment',
  'deployment-node': 'Deployment node',
  'container-instance': 'Container instance',
};

/**
 * Everything in a snapshot a person might want to jump to: authored elements,
 * derived (L4) code elements, documents, and relations.
 *
 * Relations are included because "where does the API talk to the queue" is a
 * question about an edge, and an edge has no row in the tree.
 */
export function searchIndex(snapshot) {
  const out = [];
  const nameOf = new Map((snapshot.elements ?? []).map((e) => [e.id, e.name]));

  for (const el of snapshot.elements ?? []) {
    out.push({
      kind: 'element',
      id: el.id,
      title: el.name,
      subtitle: el.id,
      tag: KIND_LABEL[el.kind] ?? el.kind,
      match: { name: el.name, id: el.id, text: el.description },
    });
  }

  for (const graph of snapshot.derived ?? []) {
    for (const d of graph.elements ?? []) {
      out.push({
        kind: 'derived',
        id: d.id,
        title: d.name,
        subtitle: d.path || graph.component,
        tag: d.kind,
        match: { name: d.name, id: d.id, text: d.path },
      });
    }
  }

  for (const doc of snapshot.docs ?? []) {
    out.push({
      kind: 'doc',
      id: doc.id,
      title: doc.title,
      subtitle: doc.id,
      tag: doc.type,
      match: { name: doc.title, id: doc.id, text: (doc.elements ?? []).join(' ') },
    });
  }

  for (const r of snapshot.relations ?? []) {
    const from = nameOf.get(r.from) ?? r.from;
    const to = nameOf.get(r.to) ?? r.to;
    out.push({
      kind: 'relation',
      id: `${r.from}|${r.to}`,
      relation: { from: r.from, to: r.to, label: r.label ?? null },
      title: `${from} → ${to}`,
      subtitle: r.label ?? r.protocol ?? `${r.from} → ${r.to}`,
      tag: 'Relation',
      match: {
        name: `${from} ${to}`,
        id: `${r.from} ${r.to}`,
        text: [r.label, r.protocol].filter(Boolean).join(' '),
      },
    });
  }

  return out;
}

/** Order the kinds appear in when they rank equally: the authored model is
 * what people mean most of the time, code detail least. */
const KIND_ORDER = { element: 0, doc: 1, relation: 2, derived: 3 };

/**
 * Rank the index against a query. An empty query answers with the top of the
 * model — the context altitude — so the palette opens onto something useful
 * rather than a blank list.
 */
export function searchModel(snapshot, query, limit = 25) {
  const q = (query ?? '').trim().toLowerCase();
  const index = searchIndex(snapshot);
  if (!q) {
    const top = new Set(['person', 'system', 'external', 'environment']);
    const byId = new Map((snapshot.elements ?? []).map((e) => [e.id, e]));
    return index
      .filter((r) => r.kind === 'element' && top.has(byId.get(r.id)?.kind))
      .sort((a, b) => a.id.localeCompare(b.id))
      .slice(0, limit);
  }
  return index
    .map((r) => ({ r, score: rank(q, r.match) }))
    .filter((x) => x.score !== null)
    .sort(
      (a, b) =>
        a.score - b.score ||
        KIND_ORDER[a.r.kind] - KIND_ORDER[b.r.kind] ||
        a.r.title.localeCompare(b.r.title) ||
        a.r.id.localeCompare(b.r.id)
    )
    .slice(0, limit)
    .map((x) => x.r);
}

/**
 * Elements a relation may point at, ranked for a picker.
 *
 * Not `searchModel` with a filter over the top. An empty query there answers
 * with the context altitude, which is the right opening for "find me
 * something" and the wrong one for "which box does this edge end at" — an
 * endpoint is usually a container or a component, and those are exactly what
 * that opening leaves out. Derived (L4) elements are absent because the engine
 * refuses a relation aimed at one, and offering what will be refused is worse
 * than not offering it.
 *
 * `exclude` takes the ids that would make the relation meaningless: the
 * endpoint being replaced is fine to re-choose, the other one is not.
 */
export function searchElements(snapshot, query, exclude = [], limit = 25) {
  const skip = new Set(exclude);
  const q = (query ?? '').trim().toLowerCase();
  const index = searchIndex(snapshot).filter((r) => r.kind === 'element' && !skip.has(r.id));
  if (!q) return index.slice().sort((a, b) => a.id.localeCompare(b.id)).slice(0, limit);
  return index
    .map((r) => ({ r, score: rank(q, r.match) }))
    .filter((x) => x.score !== null)
    .sort(
      (a, b) =>
        a.score - b.score ||
        a.r.title.localeCompare(b.r.title) ||
        a.r.id.localeCompare(b.r.id)
    )
    .slice(0, limit)
    .map((x) => x.r);
}

/** Where a result lives, for the callers that have to navigate to it: the
 * derived graph an L4 hit belongs to, or null. */
export function graphOf(snapshot, result) {
  return result.kind === 'derived' ? derivedGraphFor(snapshot, result.id) : null;
}
