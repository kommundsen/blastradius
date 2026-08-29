// What the box offers (docs/roadmap.md 0.9.0 B). Two jobs here: the rules for
// which items appear, and the gate that stops an operation from being added to
// the engine without anyone deciding whether the diagram offers it.
//
// Run: node --test ui/tests/menu.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { boxMenuItems, canvasMenuItems, NOT_ON_THE_BOX, CHILD_KINDS } from '../js/menu.js';

const here = dirname(fileURLToPath(import.meta.url));

const ctx = (over = {}) => ({
  canEdit: true,
  canPin: true,
  kind: 'container',
  pinned: false,
  pinnedCount: 0,
  hasDescription: false,
  described: false,
  hasSource: false,
  ...over,
});
const ids = (items) => items.filter((i) => !i.sep).map((i) => i.id);
const labels = (items) => items.filter((i) => !i.sep).map((i) => i.label);

test('every editing operation the canvas can perform is on the box', () => {
  const items = boxMenuItems(ctx({ pinned: true, pinnedCount: 3, hasDescription: true }));
  assert.deepEqual(ids(items), [
    'connect', 'rename', 'describe', 'child', 'unpin', 'reset-layout', 'delete',
  ]);
});

test('a menu is empty while editing is off, not partly available', () => {
  assert.deepEqual(boxMenuItems(ctx({ canEdit: false, pinned: true })), []);
});

test('layout items appear only when there is layout to release', () => {
  assert.deepEqual(ids(boxMenuItems(ctx())), ['connect', 'rename', 'add-description', 'child', 'delete']);
  // Pinned elsewhere in the view but not this box: the view can be released,
  // this box has nothing to give up.
  assert.deepEqual(
    ids(boxMenuItems(ctx({ pinnedCount: 2 }))).filter((id) => id.includes('pin') || id === 'reset-layout'),
    ['reset-layout'],
  );
  // A stale view file disables pinning without disabling editing (spec).
  assert.equal(ids(boxMenuItems(ctx({ canPin: false, pinned: true, pinnedCount: 4 })))
    .includes('reset-layout'), false);
});

test('the description item says which way it goes', () => {
  assert.ok(labels(boxMenuItems(ctx({ hasDescription: true }))).includes('Show description'));
  assert.ok(labels(boxMenuItems(ctx({ hasDescription: true, described: true }))).includes('Hide description'));
  // Nothing written yet: the menu hands over to the field rather than offering
  // to draw an empty box.
  assert.ok(labels(boxMenuItems(ctx())).includes('Add a description…'));
});

test('a component with no code behind it is offered some', () => {
  assert.ok(ids(boxMenuItems(ctx({ kind: 'component' }))).includes('map-source'));
  // Already mapped: the inspector shows the mapping, and a second offer to
  // start one would be a worse route to the same fields.
  assert.equal(ids(boxMenuItems(ctx({ kind: 'component', hasSource: true }))).includes('map-source'), false);
  // Nothing else is introspected — L4 is per component (ADR-0016).
  for (const kind of ['system', 'container', 'person', 'deployment-node']) {
    assert.equal(ids(boxMenuItems(ctx({ kind }))).includes('map-source'), false, kind);
  }
});

test('what a box may contain follows the model format, and names it', () => {
  // Ends with "inside…", not starts with "Add a": "Add a description…" is on
  // the same menu and would match first.
  const labelFor = (kind) => labels(boxMenuItems(ctx({ kind }))).find((l) => l.endsWith('inside…'));
  assert.equal(labelFor('system'), 'Add a container inside…');
  assert.equal(labelFor('container'), 'Add a component inside…');
  assert.equal(labelFor('environment'), 'Add a deployment-node inside…');
  // Two answers, so the dialog asks rather than the menu guessing.
  assert.equal(labelFor('deployment-node'), 'Add an element inside…');
  // Leaves: a component's insides are derived from source, and a person has
  // none — no child item at all, rather than one that fails on confirm.
  for (const kind of ['component', 'person', 'external', 'container-instance']) {
    assert.equal(ids(boxMenuItems(ctx({ kind }))).includes('child'), false, kind);
  }
});

test('groups are separated, and never trailing or doubled', () => {
  const items = boxMenuItems(ctx({ pinned: true, pinnedCount: 2 }));
  const shape = items.map((i) => (i.sep ? '-' : 'x')).join('');
  assert.equal(/^x+(-x+)*$/.test(shape), true, shape);
  // With nothing pinned, the layout group disappears and takes its separator.
  const bare = boxMenuItems(ctx()).map((i) => (i.sep ? '-' : 'x')).join('');
  assert.equal(/^x+(-x+)*$/.test(bare), true, bare);
  assert.equal(bare.split('-').length, 2, 'model items, then delete');
});

test('the canvas offers the view-wide release, and only when it applies', () => {
  assert.deepEqual(ids(canvasMenuItems({ canPin: true, pinnedCount: 5 })), ['reset-layout']);
  assert.deepEqual(canvasMenuItems({ canPin: true, pinnedCount: 0 }), []);
  assert.deepEqual(canvasMenuItems({ canPin: false, pinnedCount: 5 }), []);
});

// The gate. `sync::Operation` is the list of everything the model can be told
// to do; this asserts that each variant is either offered on the box or
// recorded as deliberately absent, with a reason. Adding a variant to the enum
// fails this test until someone decides which it is.
test('every sync::Operation is either offered or deliberately not', () => {
  const rust = readFileSync(join(here, '../../crates/blastradius-core/src/sync.rs'), 'utf8');
  const block = rust.split('pub enum Operation {')[1].split('\n}')[0];
  const variants = [...block.matchAll(/^ {4}([A-Z]\w+) \{/gm)]
    .map((m) => m[1].replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase());
  assert.ok(variants.length >= 9, `parsed too few variants: ${variants}`);

  // Every context the box can be in, so an op offered only in one of them
  // still counts as offered.
  const everything = new Set();
  for (const kind of Object.keys(CHILD_KINDS).concat('component')) {
    for (const pinned of [false, true]) {
      for (const hasDescription of [false, true]) {
        for (const hasSource of [false, true]) {
          const items = boxMenuItems(ctx({
            kind, pinned, pinnedCount: pinned ? 1 : 0, hasDescription, hasSource,
          }));
          for (const item of items) {
            if (!item.sep && item.op) everything.add(item.op);
          }
        }
      }
    }
  }

  for (const op of variants) {
    assert.ok(
      everything.has(op) || op in NOT_ON_THE_BOX,
      `sync::Operation::${op} is neither on the box nor listed in NOT_ON_THE_BOX with a reason`,
    );
  }
  // And no stale exemptions: an operation that has since been offered, or
  // removed from the enum, must not still be excused here.
  for (const op of Object.keys(NOT_ON_THE_BOX)) {
    assert.ok(variants.includes(op), `NOT_ON_THE_BOX names ${op}, which is not an operation`);
    assert.equal(everything.has(op), false, `${op} is both offered and excused`);
  }
});
