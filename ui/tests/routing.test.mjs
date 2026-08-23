// Obstacle-avoiding edge routing (0.2.0 theme 1, docs/roadmap.md): no edge
// segment may cross a node box it doesn't terminate on. Pinned-adjacent
// edges used to be straight center-to-center lines that ignored obstacles;
// ELK's own routes ignored pinned boxes. The router post-pass fixes both.
//
// Run: node --test ui/tests/routing.test.mjs
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

/** Sampled overlap of each edge polyline with each foreign node box. */
function crossings(layout) {
  const bad = [];
  for (const e of layout.edges) {
    for (const n of layout.nodes) {
      if (n.id === e.from || n.id === e.to) continue;
      // sample the polyline; strictly-inside points mean a real crossing
      const pts = e.points;
      let inside = 0;
      const samples = 64;
      let total = 0;
      const seg = [];
      for (let i = 1; i < pts.length; i++) {
        const l = Math.hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
        seg.push(l);
        total += l;
      }
      for (let s = 0; s <= samples; s++) {
        let want = (total * s) / samples;
        let p = pts[0];
        for (let i = 0; i < seg.length; i++) {
          if (want <= seg[i]) {
            const t = seg[i] ? want / seg[i] : 0;
            p = {
              x: pts[i].x + (pts[i + 1].x - pts[i].x) * t,
              y: pts[i].y + (pts[i + 1].y - pts[i].y) * t,
            };
            break;
          }
          want -= seg[i];
          p = pts[i + 1];
        }
        if (p.x > n.x + 1 && p.x < n.x + n.width - 1 && p.y > n.y + 1 && p.y < n.y + n.height - 1) {
          inside++;
        }
      }
      if (inside > 1) bad.push(`${e.from}->${e.to} crosses ${n.id} (${inside}/${samples})`);
    }
  }
  return bad;
}

test('a pinned node between two pinned endpoints is routed around', async () => {
  const container = (id, name) => ({ id, name, kind: 'container' });
  const view = {
    nodes: [container('a', 'A'), container('b', 'B'), container('block', 'Block')],
    edges: [{ from: 'a', to: 'b', label: 'talks to', exact: true }],
  };
  // block sits exactly on the straight a->b line
  const pins = { a: [0, 0], b: [0, 16], block: [0, 8] };
  const layout = await layoutView(new ELK(), view, pins);
  assert.deepEqual(crossings(layout), []);
  // pins stayed exact — routing must never move nodes
  const at = Object.fromEntries(layout.nodes.map((n) => [n.id, [n.x / 26, n.y / 26]]));
  assert.deepEqual(at, { a: [0, 0], b: [0, 16], block: [0, 8] });
});

test('detours are deterministic across runs', async () => {
  const container = (id, name) => ({ id, name, kind: 'container' });
  const view = {
    nodes: [container('a', 'A'), container('b', 'B'), container('block', 'Block')],
    edges: [{ from: 'a', to: 'b', label: 'talks to', exact: true }],
  };
  const pins = { a: [0, 0], b: [0, 16], block: [0, 8] };
  const one = await layoutView(new ELK(), view, pins);
  const two = await layoutView(new ELK(), view, pins);
  assert.deepEqual(one.edges[0].points, two.edges[0].points);
});

for (const [level, scope] of [['L1', null], ['L2', 'blastradius'], ['L3', 'blastradius.core']]) {
  test(`no edge crosses a node box in the dogfood ${level} view`, async () => {
    const view = computeView(snapshot, level, scope);
    const def = findViewDef(snapshot, level, scope);
    const layout = await layoutView(new ELK(), view, resolvePins(def, view));
    assert.deepEqual(crossings(layout), []);
  });
}
