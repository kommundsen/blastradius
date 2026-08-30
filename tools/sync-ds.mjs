// Keep ui/ds in step with design-system (ADR-0020).
//
// `design-system/` is the source; `ui/ds/` is the subset that ships inside the
// app and, via tools/build-site.mjs, inside the docs site. Until now that
// subset was a hand-maintained copy with nothing checking it — a product whose
// thesis is that documentation cannot quietly rot, carrying a copy of its own
// design system that could.
//
// The shape of the fix is the one the product already uses for facts:
// regenerate, or `--check` and fail the build. `blastradius introspect
// --check` is the same idea pointed at source code.
//
// What ships is *derived*, not listed: whatever `styles.css` imports,
// transitively, plus `assets/` (fonts and the brand marks, which the CSS and
// index.html reference by path). Adding a file to the design system therefore
// does not need this script edited — importing it does the work.
//
//   node tools/sync-ds.mjs           # copy design-system -> ui/ds
//   node tools/sync-ds.mjs --check   # fail if they differ (CI)

import { readFileSync, writeFileSync, mkdirSync, readdirSync, statSync, existsSync } from 'node:fs';
import { dirname, join, relative, posix } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const SRC = join(root, 'design-system');
const DST = join(root, 'ui/ds');
const ENTRY = 'styles.css';

/** Every stylesheet reachable from the entry point, entry first, no repeats. */
function importGraph(entry) {
  const seen = new Set();
  const out = [];
  const walk = (rel) => {
    if (seen.has(rel)) return;
    seen.add(rel);
    const text = readFileSync(join(SRC, rel), 'utf8');
    out.push(rel);
    for (const m of text.matchAll(/@import\s+url\(['"]?([^'")]+)['"]?\)/g)) {
      // Relative to the file doing the importing, as CSS resolves it.
      walk(posix.normalize(posix.join(posix.dirname(rel), m[1])));
    }
  };
  walk(entry);
  return out;
}

/** Every file under a directory, workspace-relative to SRC. */
function filesUnder(rel) {
  const abs = join(SRC, rel);
  if (!existsSync(abs)) return [];
  return readdirSync(abs).flatMap((name) => {
    const child = posix.join(rel, name);
    return statSync(join(SRC, child)).isDirectory() ? filesUnder(child) : [child];
  });
}

const shipped = [...importGraph(ENTRY), ...filesUnder('assets')];

const check = process.argv.includes('--check');
const drifted = [];
const missing = [];

for (const rel of shipped) {
  const src = readFileSync(join(SRC, rel));
  const dstPath = join(DST, rel);
  if (!existsSync(dstPath)) {
    missing.push(rel);
    if (!check) {
      mkdirSync(dirname(dstPath), { recursive: true });
      writeFileSync(dstPath, src);
    }
    continue;
  }
  if (!readFileSync(dstPath).equals(src)) {
    drifted.push(rel);
    if (!check) writeFileSync(dstPath, src);
  }
}

// A file in the copy that the source no longer ships is drift too: it is dead
// weight in the bundle and, worse, something a stylesheet might still import.
const shippedSet = new Set(shipped);
const extra = existsSync(DST)
  ? (function walk(rel) {
      return readdirSync(join(DST, rel || '.')).flatMap((name) => {
        const child = rel ? posix.join(rel, name) : name;
        return statSync(join(DST, child)).isDirectory() ? walk(child) : [child];
      });
    })('').filter((rel) => !shippedSet.has(rel))
  : [];

if (check) {
  const bad = [...missing.map((f) => `missing:  ${f}`),
               ...drifted.map((f) => `differs:  ${f}`),
               ...extra.map((f) => `orphaned: ${f}`)];
  if (bad.length) {
    console.error('ui/ds has drifted from design-system:\n  ' + bad.join('\n  '));
    console.error('\nRun `node tools/sync-ds.mjs` and commit the result.');
    process.exit(1);
  }
  console.log(`ui/ds: up to date (${shipped.length} files)`);
} else {
  const changed = missing.length + drifted.length;
  console.log(changed
    ? `ui/ds: synced ${changed} file(s) — ${[...missing, ...drifted].join(', ')}`
    : `ui/ds: already up to date (${shipped.length} files)`);
  if (extra.length) {
    // Not deleted automatically: removing a file from the shipped set is a
    // decision, and a script that quietly deletes is a script nobody runs.
    console.log(`ui/ds: ${extra.length} orphaned file(s) — ${extra.join(', ')}`);
    console.log('Nothing imports these. Delete them by hand if that is intended.');
  }
}
