// Two properties the canvas depends on and nothing used to check:
//
//  - layout reserves the size a node will *actually* render at, not a per-kind
//    estimate. A `.node` is content-sized, so a wrapped name is taller than the
//    estimate; layout that reserved the estimate handed the overflow to
//    whatever sat below, which is how a dense diagram ended up with nodes on
//    top of each other (reported 2026-08-26);
//  - a pin may be negative. A diagram has no top-left corner, and clamping
//    pins to one made it a wall to pile things against.
//
// Plus the two things layout owes a diagram that has grown: pinning one node
// must not relocate every other one, and a long chain must not become a tower
// nothing can read.
//
// Run: node --test ui/tests/layout-space.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';

import { layoutView, nodeSize, GRID } from '../js/layout.js';

const require = createRequire(import.meta.url);
const ELK = require('../vendor/elk.bundled.js');
const elk = new ELK();

/** A chain of components, so ELK stacks them in separate layers. */
const chain = (n, extra = {}) => ({
  nodes: Array.from({ length: n }, (_, i) => ({
    id: `s.c.n${i}`, kind: 'component', name: `Node ${i}`, ...extra,
  })),
  edges: Array.from({ length: n - 1 }, (_, i) => ({
    from: `s.c.n${i}`, to: `s.c.n${i + 1}`, label: 'calls', direction: 'forward',
  })),
});

test('measured sizes are honoured, and the gap between layers survives them', async () => {
  const view = chain(6);
  const tall = 160; // a name wrapped to several lines
  const sizes = new Map(view.nodes.map((n) => [n.id, { width: 150, height: tall }]));

  const laid = await layoutView(elk, view, {}, { sizes });
  for (const n of laid.nodes) {
    assert.equal(n.height, tall, `${n.id} was laid out at the estimate, not the measurement`);
  }

  // The property that actually matters: nothing overlaps.
  for (let i = 0; i < laid.nodes.length; i++) {
    for (let j = i + 1; j < laid.nodes.length; j++) {
      const a = laid.nodes[i], b = laid.nodes[j];
      const w = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x);
      const h = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y);
      assert.ok(w <= 0 || h <= 0, `${a.id} overlaps ${b.id} by ${w}x${h}`);
    }
  }
});

test('without measurements it still falls back to the estimate', async () => {
  const view = chain(3);
  const laid = await layoutView(elk, view, {}, {});
  const expected = nodeSize(view.nodes[0]).height;
  assert.ok(
    laid.nodes.every((n) => n.height === expected),
    'headless callers (SVG export, exported viewer) must keep working without a DOM'
  );
});

test('a negative pin is honoured, and the drawing is reframed around it', async () => {
  const view = chain(3);
  // Dragged up and left of everything else.
  const laid = await layoutView(elk, view, { 's.c.n0': [-10, -4] }, {});

  assert.ok(laid.origin, 'layout must report the translation it used');
  assert.ok(laid.origin.x > 0 && laid.origin.y > 0, `expected a reframe, got ${JSON.stringify(laid.origin)}`);

  // Nothing renders off the top-left edge...
  for (const n of laid.nodes) {
    assert.ok(n.x >= 0 && n.y >= 0, `${n.id} renders off-canvas at ${n.x},${n.y}`);
  }
  // ...and the pinned node still sits where it was pinned, in model space.
  const pinned = laid.nodes.find((n) => n.id === 's.c.n0');
  assert.equal(Math.round((pinned.x - laid.origin.x) / GRID), -10);
  assert.equal(Math.round((pinned.y - laid.origin.y) / GRID), -4);

  // Edge geometry moves with the nodes rather than being left behind.
  for (const e of laid.edges) {
    for (const p of e.points) {
      assert.ok(p.x >= 0 && p.y >= 0, `edge point left behind at ${p.x},${p.y}`);
    }
  }
});

test('with no negative pin nothing is translated', async () => {
  const laid = await layoutView(elk, chain(3), { 's.c.n0': [2, 2] }, {});
  assert.deepEqual(laid.origin, { x: 0, y: 0 }, 'an ordinary layout must not shift');
});

test('a pin off to one side does not push the rest of the diagram below it', async () => {
  const view = chain(4);
  // Pinned well to the right of where a four-node chain lays itself out, and
  // low enough that "start below the pinned bounding box" would send the whole
  // chain down past it.
  const laid = await layoutView(elk, view, { 's.c.n3': [30, 12] }, {});
  const auto = laid.nodes.filter((n) => n.id !== 's.c.n3');
  const pin = laid.nodes.find((n) => n.id === 's.c.n3');

  assert.ok(
    Math.min(...auto.map((n) => n.y)) < pin.y,
    'the auto-laid block was pushed under a pin it never collided with'
  );
  for (const a of auto) {
    const w = Math.min(a.x + a.width, pin.x + pin.width) - Math.max(a.x, pin.x);
    const h = Math.min(a.y + a.height, pin.y + pin.height) - Math.max(a.y, pin.y);
    assert.ok(w <= 0 || h <= 0, `${a.id} overlaps the pinned node`);
  }
});

test('a pin in the way still displaces the block, and nothing overlaps', async () => {
  // Pinned right where a chain starts: there is a genuine collision here, so
  // the block has to move — the point is that it moves, not that it never does.
  const laid = await layoutView(elk, chain(4), { 's.c.n3': [1, 1] }, {});
  for (let i = 0; i < laid.nodes.length; i++) {
    for (let j = i + 1; j < laid.nodes.length; j++) {
      const a = laid.nodes[i], b = laid.nodes[j];
      const w = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x);
      const h = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y);
      assert.ok(w <= 0 || h <= 0, `${a.id} overlaps ${b.id}`);
    }
  }
});

test('a long chain wraps instead of growing a tower', async () => {
  const deep = await layoutView(elk, chain(16), {}, {});
  assert.ok(
    deep.height / deep.width < 2.5,
    `16 chained components laid out ${Math.round(deep.width)}x${Math.round(deep.height)} — still a column`
  );
  for (let i = 0; i < deep.nodes.length; i++) {
    for (let j = i + 1; j < deep.nodes.length; j++) {
      const a = deep.nodes[i], b = deep.nodes[j];
      const w = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x);
      const h = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y);
      assert.ok(w <= 0 || h <= 0, `${a.id} overlaps ${b.id}`);
    }
  }
});

test('a short chain is left reading straight down', async () => {
  // Wrapping is for diagrams that have outgrown a column, not for every
  // diagram: five boxes top to bottom is the C4 convention and stays.
  const short = await layoutView(elk, chain(5), {}, {});
  const columns = new Set(short.nodes.map((n) => n.x));
  assert.equal(columns.size, 1, 'a five-node chain was wrapped when it did not need to be');
});

test('wrapping is deterministic', async () => {
  const a = await layoutView(elk, chain(16), {}, {});
  const b = await layoutView(new ELK(), chain(16), {}, {});
  assert.deepEqual(
    a.nodes.map((n) => [n.id, n.x, n.y]),
    b.nodes.map((n) => [n.id, n.x, n.y])
  );
});
