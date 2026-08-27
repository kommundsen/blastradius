// Headless SVG rendering (0.2.0 theme 2, spec/export.md): the node script
// must render every dogfood view deterministically in both themes — CI
// publishes these, and the PR diff bot depends on byte-stable output.
//
// Run: node --test ui/tests/render.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync, rmSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '../..');
const snap = join(root, 'ui/mock/snapshot.json');

function render(extra = []) {
  const out = mkdtempSync(join(tmpdir(), 'br-render-'));
  execFileSync(process.execPath, [join(root, 'tools/render-views.mjs'), snap, '-o', out, ...extra]);
  const files = Object.fromEntries(
    readdirSync(out).sort().map((f) => [f, readFileSync(join(out, f), 'utf8')])
  );
  rmSync(out, { recursive: true, force: true });
  return files;
}

test('renders L1 plus every defined view, with real content', () => {
  const files = render();
  const names = Object.keys(files);
  assert.deepEqual(names, [
    'blastradius-L2.svg',
    'blastradius-core-L3.svg',
    'context-L1.svg',
    'deployment-LD.svg',
    'dev-machine-LD.svg',
  ]);
  assert.match(files['context-L1.svg'], /BLASTRADIUS/);
  assert.match(files['blastradius-L2.svg'], /APP SHELL/);
  assert.match(files['blastradius-core-L3.svg'], /SYNC ENGINE/);
  // Deployment renders headlessly like any other view (ADR-0018), and the
  // overview carries the delivery chain — a connector-less deployment
  // diagram would be worthless.
  assert.match(files['deployment-LD.svg'], /DEVELOPER MACHINE/);
  assert.match(files['deployment-LD.svg'], /triggers on push/);
  for (const [name, svg] of Object.entries(files)) {
    assert.ok(!/NaN|undefined/.test(svg), `${name} has broken interpolation`);
    assert.match(svg, /@font-face/, `${name} missing embedded fonts`);
  }
});

test('output is byte-identical across runs', () => {
  assert.deepEqual(render(), render());
});

test('themes differ and light is actually light', () => {
  const light = render()['context-L1.svg'];
  const dark = render(['--theme', 'dark'])['context-L1.svg'];
  assert.notEqual(light, dark);
  // the first full-canvas rect is the ground; light theme = near-white hex
  assert.match(light, /<rect width="\d+" height="\d+" fill="#f2f2f3"\/>/);
  assert.match(dark, /<rect width="\d+" height="\d+" fill="#1d1f20"\/>/);
});
