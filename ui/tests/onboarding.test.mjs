// The onboarding dialog offers the same pieces `blastradius init` does, and
// names the same agents core knows about.
//
// Run: node --test ui/tests/onboarding.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '../js/app.js'), 'utf8');
const onboardRs = readFileSync(join(here, '../../crates/blastradius-core/src/onboard.rs'), 'utf8');

test('the agent ids the dialog offers are the ones core can write', () => {
  // core: pub const AGENTS: [&str; 4] = ["claude", "copilot", "cursor", "codex"];
  const decl = onboardRs.match(/pub const AGENTS[^=]*=\s*\[([^\]]*)\]/);
  assert.ok(decl, 'could not find AGENTS in onboard.rs');
  const fromRust = [...decl[1].matchAll(/"([a-z]+)"/g)].map((m) => m[1]).sort();

  const uiBlock = appJs.match(/const AGENTS = \[([\s\S]*?)\];/);
  assert.ok(uiBlock, 'could not find the AGENTS list in app.js');
  const fromUi = [...uiBlock[1].matchAll(/id: '([a-z]+)'/g)].map((m) => m[1]).sort();

  assert.deepEqual(
    fromUi,
    fromRust,
    'the dialog and core::onboard disagree about which agents exist — an id the UI sends that core does not know is silently logged as an error'
  );
});

test('an existing file is never presented as a failure', () => {
  // The 0.6.0 bug: the starter set includes README.md and any existing file
  // was fatal, so the offer failed on every real repository.
  const scaffoldRs = readFileSync(
    join(here, '../../crates/blastradius-core/src/scaffold.rs'),
    'utf8'
  );
  assert.ok(
    /pub fn scaffold_into/.test(scaffoldRs),
    'scaffold_into is the one place that decides what happens to an existing file'
  );
  for (const rel of [
    '../../crates/blastradius-app/src/main.rs',
    '../../crates/blastradius-cli/src/main.rs',
  ]) {
    const src = readFileSync(join(here, rel), 'utf8');
    const live = src
      .split('\n')
      .filter((l) => !/^\s*(\/\/|\/\*|\*)/.test(l))
      .join('\n');
    assert.ok(
      !/refusing to overwrite/.test(live),
      `${rel} still treats an existing file as fatal instead of going through scaffold_into`
    );
  }
});
