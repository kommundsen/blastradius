// Drift on the canvas (docs/roadmap.md 0.9.0 F). The engine decides *what* is
// drifting (ADR-0019); this file pins how a finding about two components is
// drawn at whatever altitude you happen to be looking from — which is the part
// with rules in it.
//
// Run: node --test ui/tests/drift.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { computeView } from '../js/data.js';

const here = dirname(fileURLToPath(import.meta.url));
const snapshot = JSON.parse(readFileSync(join(here, '../mock/snapshot.json'), 'utf8'));

const undeclared = (from, to, via = 'src/thing.rs') => ({ from, to, kind: 'undeclared', via });
const ghosts = (view) => view.edges.filter((e) => e.drift);

test('an undeclared dependency joins the view it belongs to', () => {
  const view = computeView(snapshot, 'L3', 'blastradius.core', false, false, [
    undeclared('blastradius.core.exporter', 'blastradius.core.git-service'),
  ]);
  assert.equal(ghosts(view).length, 1);
  const [g] = ghosts(view);
  assert.equal(g.from, 'blastradius.core.exporter');
  assert.equal(g.to, 'blastradius.core.git-service');
  // The ids the finding was made at ride along: a fix is written where the
  // code is, not where the box happens to be drawn.
  assert.equal(g.drift.from, 'blastradius.core.exporter');
  assert.equal(g.drift.via, 'src/thing.rs');
  assert.equal(g.label, null, 'a ghost has nothing to say — it is not a relation');
});

test('a finding between two components inside one box is not drawn', () => {
  // At L2 both endpoints lift to the same container: there is no line to draw
  // between a box and itself, and nothing about the picture is wrong.
  const view = computeView(snapshot, 'L2', 'blastradius', true, false, [
    undeclared('blastradius.core.exporter', 'blastradius.core.git-service'),
  ]);
  assert.equal(ghosts(view).length, 0);
});

test('a finding lifts to the boxes that are actually on screen', () => {
  // The model declares cli -> core, so at L2 these two boxes are connected —
  // but not in the direction the code goes, which is the case most worth
  // seeing rather than the case to suppress.
  const view = computeView(snapshot, 'L2', 'blastradius', true, false, [
    undeclared('blastradius.core.exporter', 'blastradius.cli.mcp-server'),
  ]);
  const [g] = ghosts(view);
  assert.equal(g.from, 'blastradius.core');
  assert.equal(g.to, 'blastradius.cli');
  assert.equal(g.exact, false, 'drawn between ancestors, so it is an aggregate');
  assert.equal(g.drift.from, 'blastradius.core.exporter', 'the finding keeps its own ids');
});

test('a dependency the model already declares is never a ghost', () => {
  // sync-engine -> model-service is a declared relation in this model, so a
  // finding naming it would be stale rather than news.
  const view = computeView(snapshot, 'L3', 'blastradius.core', false, false, [
    undeclared('blastradius.core.sync-engine', 'blastradius.core.model-service'),
  ]);
  assert.equal(ghosts(view).length, 0);
});

test('an unbacked finding marks the declaration it is about', () => {
  const view = computeView(snapshot, 'L3', 'blastradius.core', false, false, [
    { from: 'blastradius.core.sync-engine', to: 'blastradius.core.model-service', kind: 'unbacked' },
  ]);
  assert.equal(ghosts(view).length, 0, 'nothing new is drawn — the edge is already there');
  const edge = view.edges.find((e) => e.from === 'blastradius.core.sync-engine'
    && e.to === 'blastradius.core.model-service');
  assert.ok(edge.unbacked, 'the declared edge carries the finding');
  assert.equal(edge.unbacked.from, 'blastradius.core.sync-engine');
});

test('a finding about something this view cannot show is left alone', () => {
  const view = computeView(snapshot, 'L3', 'blastradius.core', false, false, [
    undeclared('nowhere.at.all', 'blastradius.core.exporter'),
  ]);
  assert.equal(ghosts(view).length, 0);
});
