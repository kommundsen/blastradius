// What the problems panel offers, and for which finding (0.11.0 item 6).
//
// Run: node --test ui/tests/problems.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { problemRows, problemSummary } from '../js/problems.js';

const snap = (over = {}) => ({
  elements: [
    { id: 'a.b', name: 'Exporter' },
    { id: 'a.c', name: 'Git Service' },
  ],
  diagnostics: [],
  drift: [],
  ...over,
});

const nameOf = (id) => snap().elements.find((e) => e.id === id)?.name ?? id;

test('an info diagnostic is not a problem', () => {
  // The parser saying it ignored a file without frontmatter is a fact about the
  // workspace, not a fault in it — and it is present in this very repository,
  // so a panel that counted it would always claim one problem.
  const rows = problemRows(snap({
    diagnostics: [
      { severity: 'info', file: 'README.md', line: 0, message: 'no frontmatter' },
      { severity: 'error', file: 'model/a.yaml', line: 14, message: 'dangling reference' },
    ],
  }));
  assert.equal(rows.length, 1);
  assert.equal(rows[0].kind, 'error');
});

test('a diagnostic names its file and line, and offers to open it', () => {
  const [row] = problemRows(snap({
    diagnostics: [{ severity: 'error', file: 'model/a.yaml', line: 14, message: 'dangling reference' }],
  }));
  assert.equal(row.subtitle, 'model/a.yaml:14');
  assert.deepEqual(row.fix, { op: 'open', label: 'Open' });
  // What a dangling reference should *become* is a modelling decision, so the
  // row goes to the file rather than pretending a button can take it.
  assert.equal(row.focus, null);
});

test('a line of zero is a file without a line, not line zero', () => {
  const [row] = problemRows(snap({
    diagnostics: [{ severity: 'warning', file: 'model/a.yaml', line: 0, message: 'no containers' }],
  }));
  assert.equal(row.subtitle, 'model/a.yaml');
});

test('drift rows are named after elements, not ids', () => {
  const rows = problemRows(snap({
    drift: [{ from: 'a.b', to: 'a.c', kind: 'undeclared', via: 'src/export.rs' }],
  }), { nameOf });
  assert.equal(rows[0].title, 'Exporter → Git Service');
  assert.match(rows[0].subtitle, /src\/export\.rs/);
});

test('each kind of drift offers the repair that fits it', () => {
  const rows = problemRows(snap({
    drift: [
      { from: 'a.b', to: 'a.c', kind: 'undeclared', via: 'src/export.rs' },
      { from: 'a.c', to: 'a.b', kind: 'unbacked' },
    ],
  }), { nameOf });
  assert.equal(rows[0].fix.op, 'declare');
  // Reversing is the repair drift can prove: our own model got this wrong once,
  // with the dependency running the other way.
  assert.equal(rows[1].fix.op, 'reverse');
  // Both land on an element, because both know two of them.
  assert.equal(rows[0].focus, 'a.b');
  assert.equal(rows[1].focus, 'a.c');
});

test('a read-only workspace is offered no repairs, and still reads', () => {
  const rows = problemRows(snap({
    drift: [{ from: 'a.b', to: 'a.c', kind: 'undeclared', via: 'x.rs' }],
    diagnostics: [{ severity: 'error', file: 'a.yaml', line: 2, message: 'bad' }],
  }), { canEdit: false, nameOf });
  assert.equal(rows.find((r) => r.kind === 'drift').fix, null);
  // Opening a file is not an edit, so it survives.
  assert.equal(rows.find((r) => r.kind === 'error').fix.op, 'open');
});

test('errors outrank warnings, and both outrank drift', () => {
  const rows = problemRows(snap({
    drift: [{ from: 'a.b', to: 'a.c', kind: 'unbacked' }],
    diagnostics: [
      { severity: 'warning', file: 'a.yaml', line: 1, message: 'w' },
      { severity: 'error', file: 'a.yaml', line: 2, message: 'e' },
    ],
  }), { nameOf });
  assert.deepEqual(rows.map((r) => r.kind), ['error', 'warning', 'drift']);
});

test('the chip counts every kind and takes the worst colour', () => {
  const rows = problemRows(snap({
    drift: [{ from: 'a.b', to: 'a.c', kind: 'unbacked' }],
    diagnostics: [{ severity: 'warning', file: 'a.yaml', line: 1, message: 'w' }],
  }), { nameOf });
  const sum = problemSummary(rows);
  assert.equal(sum.label, '1 warning · 1 drift');
  assert.equal(sum.tone, 'warning');

  // Drift on its own is a disagreement, not a failure, and must not shout in
  // the colour reserved for a model that cannot load.
  const driftOnly = problemSummary(problemRows(snap({
    drift: [{ from: 'a.b', to: 'a.c', kind: 'unbacked' }],
  }), { nameOf }));
  assert.equal(driftOnly.tone, 'accent');
  assert.equal(driftOnly.label, '1 drift');
});

test('a clean workspace has no chip at all', () => {
  assert.equal(problemSummary(problemRows(snap())), null);
});

test('plurals are counted, not assumed', () => {
  const sum = problemSummary(problemRows(snap({
    diagnostics: [
      { severity: 'error', file: 'a.yaml', line: 1, message: 'one' },
      { severity: 'error', file: 'a.yaml', line: 2, message: 'two' },
    ],
  })));
  assert.equal(sum.label, '2 errors');
});
