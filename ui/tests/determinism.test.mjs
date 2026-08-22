// ELK layout determinism (ADR-0006): the same model must produce identical
// geometry on every run and every fresh ELK instance — otherwise pinned
// coordinates are meaningless and every checkout renders differently.
//
// Run: node --test ui/tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { computeView, findViewDef, resolvePins } from '../js/data.js';
import { layoutView } from '../js/layout.js';

const here = dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);
const ELK = require('../vendor/elk.bundled.js');

const snapshot = JSON.parse(
  readFileSync(join(here, '../mock/snapshot.json'), 'utf8')
);

function geometry(layout) {
  return {
    nodes: layout.nodes.map((n) => [n.id, n.x, n.y, n.width, n.height]),
    edges: layout.edges.map((e) => [e.from, e.to, ...e.points.flatMap((p) => [p.x, p.y])]),
  };
}

async function lay(level, scope) {
  const view = computeView(snapshot, level, scope);
  const def = findViewDef(snapshot, level, scope);
  const elk = new ELK(); // fresh instance every time — no shared state
  return geometry(await layoutView(elk, view, resolvePins(def, view)));
}

test('L1 layout is identical across runs and instances', async () => {
  const a = await lay('L1', null);
  const b = await lay('L1', null);
  assert.deepEqual(a, b);
  assert.ok(a.nodes.length >= 5, 'L1 renders the context');
});

test('L2 layout (with pins) is identical across runs', async () => {
  const a = await lay('L2', 'blastradius');
  const b = await lay('L2', 'blastradius');
  assert.deepEqual(a, b);
});

test('L3 layout is identical across runs', async () => {
  const a = await lay('L3', 'blastradius.core');
  const b = await lay('L3', 'blastradius.core');
  assert.deepEqual(a, b);
  assert.ok(a.nodes.length >= 6, 'core components render');
});

test('pins land exactly on their grid coordinates', async () => {
  const def = findViewDef(snapshot, 'L2', 'blastradius');
  assert.ok(def, 'docs workspace pins the containers view');
  const view = computeView(snapshot, 'L2', 'blastradius');
  const pins = resolvePins(def, view);
  assert.ok(Object.keys(pins).length >= 3, 'pins resolve onto visible nodes');
  const laid = await layoutView(new ELK(), view, pins);
  for (const [id, [gx, gy]] of Object.entries(pins)) {
    const node = laid.nodes.find((n) => n.id === id);
    assert.ok(node, `pinned node ${id} present`);
    assert.equal(node.x, gx * 26, `${id} x on grid`);
    assert.equal(node.y, gy * 26, `${id} y on grid`);
  }
});

test('nodes never overlap', async () => {
  for (const [level, scope] of [['L1', null], ['L2', 'blastradius'], ['L3', 'blastradius.core']]) {
    const { nodes } = await lay(level, scope);
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        const [ai, ax, ay, aw, ah] = nodes[i];
        const [bi, bx, by, bw, bh] = nodes[j];
        const overlap = ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah;
        assert.ok(!overlap, `${level}: ${ai} overlaps ${bi}`);
      }
    }
  }
});
