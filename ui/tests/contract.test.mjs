// The mock half of the mock/engine contract (0.10.0 item 4).
//
// `crates/blastradius-core/tests/contract.rs` runs one operation list through
// the real sync engine and commits the result. This runs the same list through
// `ui/js/mockops.js` — the semantics the e2e suite actually tests against — and
// compares. The e2e suite has been able to agree with itself while disagreeing
// with the engine since ADR-0011; this is where that stops being possible.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { applyMockOperation } from '../js/mockops.js';

const here = dirname(fileURLToPath(import.meta.url));
const read = (p) => JSON.parse(readFileSync(join(here, p), 'utf8'));

const start = read('../mock/snapshot.json');
const ops = read('contract/operations.json');
const expected = read('contract/after.json');

/** Both sides key their collections by id; neither renders them in order.
 *
 * The engine holds elements in a BTreeMap and relations and views in file
 * order, and the mock appends. Order is therefore not part of the contract —
 * but everything in the collections is, so this normalises the ordering and
 * nothing else. */
function normalise(snap) {
  const by = (...keys) => (a, b) => {
    for (const k of keys) {
      const x = String(a[k] ?? ''), y = String(b[k] ?? '');
      if (x !== y) return x < y ? -1 : 1;
    }
    return 0;
  };
  return {
    name: snap.name,
    elements: [...snap.elements].sort(by('id')),
    relations: [...snap.relations].sort(by('from', 'to', 'label')),
    views: [...snap.views].sort(by('id')),
    derived: [...(snap.derived ?? [])].sort(by('component')),
  };
}

test('the mock applies the contract operations exactly as the engine does', () => {
  const got = structuredClone(start);
  for (const [i, op] of ops.entries()) {
    try {
      applyMockOperation(got, op);
    } catch (e) {
      assert.fail(`operation ${i} (${op.op}) threw in the mock: ${e.message}`);
    }
  }

  const a = normalise(got);
  const b = normalise(expected);

  // Compared collection by collection so a failure names the half that
  // diverged rather than printing two whole models.
  assert.deepEqual(a.name, b.name, 'workspace name');
  assert.deepEqual(a.elements, b.elements, 'elements');
  assert.deepEqual(a.relations, b.relations, 'relations');
  assert.deepEqual(a.views, b.views, 'views');
  assert.deepEqual(a.derived, b.derived, 'derived graphs');
});

test('every operation in the fixture actually does something in the mock', () => {
  // applyMockOperation falls through silently on an operation it does not
  // know, so the comparison above could be satisfied by a mock that does
  // nothing at all for a whole variant — which is the exact failure this file
  // exists to catch. Run the list cumulatively, and require each step to move
  // the snapshot.
  const snap = structuredClone(start);
  for (const [i, op] of ops.entries()) {
    const before = JSON.stringify(snap);
    applyMockOperation(snap, op);
    assert.notEqual(
      JSON.stringify(snap), before,
      `operation ${i} (${op.op}) changed nothing in the mock: ${JSON.stringify(op)}`
    );
  }
});
