// Find-anything ranking (docs/roadmap.md 0.7.0). The palette itself is a few
// lines of DOM; the part worth pinning is the order results come back in, and
// that it reaches everything — including the two things the sidebar tree
// cannot show at all: relations, and code-level detail.
//
// Run: node --test ui/tests/search.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { searchModel, searchIndex } from '../js/search.js';

const here = dirname(fileURLToPath(import.meta.url));
const snapshot = JSON.parse(readFileSync(join(here, '../mock/snapshot.json'), 'utf8'));

const kinds = (rs) => new Set(rs.map((r) => r.kind));

test('an exact name beats a partial one', () => {
  const rs = searchModel(snapshot, 'core');
  assert.ok(rs.length, 'no results for "core"');
  assert.equal(rs[0].title.toLowerCase(), 'core');
});

test('ids are searchable, not just names', () => {
  const rs = searchModel(snapshot, 'blastradius.core.git-service');
  assert.equal(rs[0].id, 'blastradius.core.git-service');
});

test('search is case-insensitive and trims', () => {
  const a = searchModel(snapshot, 'CORE');
  const b = searchModel(snapshot, '  core  ');
  assert.deepEqual(a.map((r) => r.id), b.map((r) => r.id));
});

test('documents are reachable', () => {
  const rs = searchModel(snapshot, 'adr');
  assert.ok(kinds(rs).has('doc'), 'no document matched "adr"');
});

test('relations are reachable — the tree has no row for an edge', () => {
  const withLabel = snapshot.relations.find((r) => r.label);
  assert.ok(withLabel, 'the mock model has no labelled relation to search for');
  const rs = searchModel(snapshot, withLabel.label);
  assert.ok(
    rs.some((r) => r.kind === 'relation' && r.relation.from === withLabel.from),
    `no relation matched ${withLabel.label}`
  );
});

test('derived (L4) elements are reachable', () => {
  const graph = (snapshot.derived ?? [])[0];
  assert.ok(graph?.elements?.length, 'the mock model has no derived graph');
  const rs = searchModel(snapshot, graph.elements[0].name, 100);
  assert.ok(kinds(rs).has('derived'), 'code-level detail is not searchable');
});

test('an empty query opens on the context altitude, not a blank list', () => {
  const rs = searchModel(snapshot, '');
  assert.ok(rs.length, 'the palette would open empty');
  const top = new Set(['person', 'system', 'external', 'environment']);
  const byId = new Map(snapshot.elements.map((e) => [e.id, e]));
  assert.ok(rs.every((r) => top.has(byId.get(r.id).kind)));
});

test('no match answers with nothing rather than everything', () => {
  assert.deepEqual(searchModel(snapshot, 'zzzzzznotathing'), []);
});

test('ranking is deterministic', () => {
  const a = searchModel(snapshot, 'e');
  const b = searchModel(snapshot, 'e');
  assert.deepEqual(a.map((r) => [r.kind, r.id]), b.map((r) => [r.kind, r.id]));
});

test('the limit is honoured', () => {
  assert.ok(searchModel(snapshot, 'e', 3).length <= 3);
});

test('every indexed row can be rendered and acted on', () => {
  for (const r of searchIndex(snapshot)) {
    assert.ok(r.title, `${r.kind} ${r.id} has no title to show`);
    assert.ok(r.tag, `${r.kind} ${r.id} has no kind label`);
    if (r.kind === 'relation') assert.ok(r.relation.from && r.relation.to);
  }
});
