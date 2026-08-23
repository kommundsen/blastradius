// L4 derived-view derivation (spec/l4-introspection.md): parent-based
// nesting (fact ids contain dots — dot-depth arithmetic is wrong), edge
// lifting onto the visible altitude, read-only semantics.
//
// Run: node --test ui/tests/derived.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { computeView, derivedGraphFor } from '../js/data.js';

const here = dirname(fileURLToPath(import.meta.url));
const snapshot = JSON.parse(readFileSync(join(here, '../mock/snapshot.json'), 'utf8'));

const COMP = 'blastradius.core.git-service';

test('derivedGraphFor finds the graph for the component and its src ids', () => {
  assert.equal(derivedGraphFor(snapshot, COMP)?.language, 'rust');
  assert.equal(derivedGraphFor(snapshot, `${COMP}.src.git.GitContext`)?.component, COMP);
  assert.equal(derivedGraphFor(snapshot, 'blastradius.core.sync-engine'), null);
});

test('L4 at the component shows top-level modules with lifted edges', () => {
  const view = computeView(snapshot, 'L4', COMP);
  assert.deepEqual(
    view.nodes.map((n) => n.name),
    ['git.rs', 'resolve.rs']
  );
  assert.ok(view.nodes.every((n) => n.derived === true));
  // resolve.rs imports git.rs; type-level references lift onto the modules.
  const imp = view.edges.find((e) => e.from.endsWith('.src.resolve') && e.to.endsWith('.src.git'));
  assert.ok(imp, JSON.stringify(view.edges));
  // Intra-module edges lift to self-loops and are dropped.
  assert.ok(view.edges.every((e) => e.from !== e.to));
});

test('L4 at a module shows its types via the parent field', () => {
  const view = computeView(snapshot, 'L4', `${COMP}.src.resolve`);
  assert.deepEqual(
    view.nodes.map((n) => n.name).sort(),
    ['Resolution', 'Side']
  );
  const ref = view.edges.find(
    (e) => e.from.endsWith('.Resolution') && e.to.endsWith('.Side') && e.label === 'references'
  );
  assert.ok(ref, JSON.stringify(view.edges));
});

test('L4 with no graph (hand-modeled component) is empty, not an error', () => {
  const view = computeView(snapshot, 'L4', 'blastradius.core.sync-engine');
  assert.deepEqual(view.nodes, []);
  assert.deepEqual(view.edges, []);
});

test('derivation is deterministic', () => {
  const a = JSON.stringify(computeView(snapshot, 'L4', COMP));
  const b = JSON.stringify(computeView(snapshot, 'L4', COMP));
  assert.equal(a, b);
});
