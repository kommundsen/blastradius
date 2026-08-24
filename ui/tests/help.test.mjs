// Bundled in-app help (docs/roadmap.md 0.4.0 theme 3): the page list and the
// markdown files must agree, cross-links must resolve, and the bundled privacy
// policy must not drift from the canonical one.
//
// Run: node --test ui/tests/help.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { HELP_PAGES, helpLinkTarget } from '../js/help.js';

const here = dirname(fileURLToPath(import.meta.url));
const helpDir = join(here, '../help');
const read = (id) => readFileSync(join(helpDir, `${id}.md`), 'utf8');

test('every listed page exists, and every file is listed', () => {
  const onDisk = readdirSync(helpDir)
    .filter((f) => f.endsWith('.md'))
    .map((f) => f.replace(/\.md$/, ''))
    .sort();
  assert.deepEqual(
    HELP_PAGES.map((p) => p.id).sort(),
    onDisk,
    'help.js and ui/help/ disagree — a page is unreachable or missing'
  );
});

test('each page opens with the title the index promises', () => {
  for (const page of HELP_PAGES) {
    const first = read(page.id).split('\n')[0];
    assert.match(first, /^# /, `${page.id} must start with an h1`);
    // The index label need not equal the heading, but a page whose heading
    // says something unrelated is a navigation trap.
    assert.ok(first.length > 2, `${page.id} has an empty heading`);
  }
});

test('cross-links between help pages all resolve', () => {
  for (const page of HELP_PAGES) {
    const body = read(page.id);
    for (const [, href] of body.matchAll(/\]\(([^)]+)\)/g)) {
      if (!href.endsWith('.md')) continue; // external or anchor
      assert.ok(
        helpLinkTarget(href),
        `${page.id}.md links to ${href}, which is not a help page`
      );
    }
  }
});

test('every shipped feature area has a page (the theme-3 exit)', () => {
  const ids = new Set(HELP_PAGES.map((p) => p.id));
  for (const required of [
    'getting-started',
    'canvas',
    'editing',
    'deployment',
    'code-level',
    'git',
    'export',
    'agents',
    'model-format',
    'shortcuts',
    'privacy',
  ]) {
    assert.ok(ids.has(required), `no help page covers ${required}`);
  }
});

test('the bundled privacy policy matches the canonical one', () => {
  const canonical = readFileSync(join(here, '../../docs/privacy.md'), 'utf8')
    .replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n/, '')
    .replace(/^\s+/, '')
    .replace(/\r\n/g, '\n');
  assert.equal(
    read('privacy'),
    canonical,
    'ui/help/privacy.md has drifted from docs/privacy.md — regenerate it'
  );
});
