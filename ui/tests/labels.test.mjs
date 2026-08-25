// How technology is written on a diagram (spec/model-format.md §3): C4's
// square-bracket convention, in one module because four surfaces draw these
// strings and used to disagree about them.
//
// Run: node --test ui/tests/labels.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { kicker, edgeLabelLines, edgeLabelText } from '../js/labels.js';

const here = dirname(fileURLToPath(import.meta.url));

test('an element writes its type and technology the way C4 does', () => {
  assert.equal(kicker({ kind: 'container', tech: 'Rust' }), '[Container: Rust]');
  assert.equal(kicker({ kind: 'person' }), '[Person]');
  assert.equal(kicker({ kind: 'system', external: true }), '[External system]');
  assert.equal(kicker({ kind: 'container-instance' }), '[Container instance]');
  // Derived elements say so where the technology would go.
  assert.equal(kicker({ kind: 'module', derived: true }), '[Module: derived]');
  assert.equal(kicker({ kind: 'dependency', derived: true }), '[Dependency: external]');
});

test('a relation shows its label and its protocol, never one or the other', () => {
  // The bug this replaces: ui/js/svg.js rendered `e.label ?? e.protocol`, so
  // every exported diagram silently dropped the protocol of any relation that
  // also had a label — while the in-app canvas showed both.
  assert.deepEqual(edgeLabelLines({ label: 'calls', protocol: 'JSON/HTTPS' }), [
    'calls',
    '[JSON/HTTPS]',
  ]);
  assert.deepEqual(edgeLabelLines({ label: 'calls' }), ['calls']);
  assert.deepEqual(edgeLabelLines({ protocol: 'SQL' }), ['[SQL]']);
  assert.deepEqual(edgeLabelLines({}), []);
  assert.equal(edgeLabelText({ label: 'calls', protocol: 'gRPC' }), 'calls [gRPC]');
});

test('every surface draws these strings from this module', () => {
  // The exported viewer carried its own copy of kicker() and it had already
  // drifted; layout.js measured a fifth variant when placing labels.
  for (const rel of ['../js/app.js', '../js/svg.js', '../js/viewer.js', '../js/layout.js']) {
    const full = readFileSync(join(here, rel), 'utf8');
    // Comments may quote the old code to explain why it went.
    const src = full
      .split('\n')
      .filter((l) => !/^\s*(\/\/|\*|\/\*)/.test(l))
      .join('\n');
    assert.ok(
      /edgeLabelLines|labels\.js/.test(full),
      `${rel} does not go through labels.js`
    );
    assert.ok(
      !/label\s*\?\?\s*e?\.?protocol/.test(src),
      `${rel} still picks one of label/protocol instead of showing both`
    );
    assert.ok(
      !/\$\{[^}]*label\}\s*·\s*\$\{[^}]*protocol\}/.test(src),
      `${rel} still joins label and protocol with a separator`
    );
  }
});
