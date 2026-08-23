// Headless SVG/PNG renderer (0.2.0 theme 2, spec/export.md): the same
// layout engine (ui/js/layout.js + vendored elkjs) and the same SVG
// assembly (ui/js/svg.js) the app uses, driven from node — deterministic
// by the same ADR-0006 guarantees, so CI renders are diff-stable.
//
//   node tools/render-views.mjs <snapshot.json> -o <outdir> [--theme dark]
//        [--png] [--scale N] [--no-footer]
//
// Renders the L1 context view plus every view defined in the snapshot.
// PNG needs Playwright's WebKit (already a dev dependency; skipped unless
// --png is passed, so the SVG path never requires a browser download).
import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { createRequire } from 'module';
import { fileURLToPath } from 'url';
import { dirname, join, resolve } from 'path';

import { computeView, findViewDef, resolvePins } from '../ui/js/data.js';
import { layoutView } from '../ui/js/layout.js';
import { viewSvg } from '../ui/js/svg.js';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const require = createRequire(import.meta.url);
const ELK = require(join(root, 'ui/vendor/elk.bundled.js'));

// ---- args -------------------------------------------------------------------
const args = process.argv.slice(2);
let snapPath = null, outDir = null, theme = 'light', png = false, scale = 2, footer = true;
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '-o') outDir = args[++i];
  else if (a === '--theme') theme = args[++i];
  else if (a === '--png') png = true;
  else if (a === '--scale') scale = Number(args[++i]);
  else if (a === '--no-footer') footer = false;
  else snapPath = a;
}
if (!snapPath || !outDir) {
  console.error('usage: node tools/render-views.mjs <snapshot.json> -o <outdir> [--theme dark] [--png] [--scale N]');
  process.exit(2);
}

// ---- design tokens, resolved headlessly -------------------------------------
// The palette is the single source of truth (ui/ds/tokens/colors.css). Two
// value shapes exist there: plain colors and
// `color-mix(in srgb, <color> N%, transparent)`; var() chains link them.
// That tiny grammar is evaluated here rather than dragging in a browser.
function tokenMaps(css) {
  // dark declarations live inside `@media (prefers-color-scheme: dark)`
  // (whose inner selector mentions "light", so selector text alone lies)
  // and under `[data-theme="dark"]` — find the media ranges brace-aware
  const darkRanges = [];
  for (const m of css.matchAll(/@media[^{]*prefers-color-scheme:\s*dark[^{]*\{/g)) {
    let depth = 1, i = m.index + m[0].length;
    while (i < css.length && depth) {
      if (css[i] === '{') depth++;
      else if (css[i] === '}') depth--;
      i++;
    }
    darkRanges.push([m.index, i]);
  }
  const inDark = (idx) => darkRanges.some(([a, b]) => idx >= a && idx < b);
  const light = {}, dark = {};
  for (const m of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const isDark = inDark(m.index) || /\[data-theme="dark"\]/.test(m[1]);
    for (const p of m[2].matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
      (isDark ? dark : light)[p[1]] = p[2].trim();
    }
  }
  return { light, dark: { ...light, ...dark } };
}

function resolveColor(name, map, depth = 0) {
  if (depth > 8) throw new Error(`token cycle at ${name}`);
  let v = map[name];
  if (!v) throw new Error(`unknown token ${name}`);
  v = v.replace(/\/\*.*?\*\//g, '').trim();
  const varRef = v.match(/^var\((--[\w-]+)\)$/);
  if (varRef) return resolveColor(varRef[1], map, depth + 1);
  const mix = v.match(/^color-mix\(in srgb,\s*(.+?)\s+([\d.]+)%,\s*transparent\)$/);
  if (mix) {
    let c = mix[1];
    const inner = c.match(/^var\((--[\w-]+)\)$/);
    if (inner) c = resolveColor(inner[1], map, depth + 1);
    const [r, g, b] = hexRgb(c);
    return `rgba(${r},${g},${b},${Number(mix[2]) / 100})`;
  }
  return v;
}

function hexRgb(c) {
  const m = c.match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/i);
  if (!m) throw new Error(`expected hex color, got ${c}`);
  let h = m[1];
  if (h.length === 3) h = [...h].map((x) => x + x).join('');
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16));
}

function palette(theme) {
  const css = readFileSync(join(root, 'ui/ds/tokens/colors.css'), 'utf8');
  const map = tokenMaps(css)[theme];
  const pick = (n) => resolveColor(n, map);
  return {
    bg: pick('--canvas-bg'),
    dot: pick('--canvas-dot'),
    text: pick('--color-text'),
    muted: pick('--color-text-muted'),
    border: pick('--node-border'),
    fill: pick('--node-fill'),
    external: pick('--node-external'),
    edge: pick('--edge-stroke'),
    key: pick('--code-key'),
  };
}

function fontCss() {
  const faces = [
    ['Barlow', 400, 'ui/ds/assets/fonts/barlow-400-latin.woff2'],
    ['Barlow Condensed', 600, 'ui/ds/assets/fonts/barlow-condensed-600-latin.woff2'],
  ];
  return faces.map(([family, weight, rel]) => {
    const b64 = readFileSync(join(root, rel)).toString('base64');
    return `@font-face{font-family:'${family}';font-weight:${weight};src:url(data:font/woff2;base64,${b64}) format('woff2')}`;
  }).join('');
}

// ---- render -----------------------------------------------------------------
const snapshot = JSON.parse(readFileSync(snapPath, 'utf8'));
mkdirSync(outDir, { recursive: true });
const colors = palette(theme);
const fonts = fontCss();

const targets = [{ level: 'L1', scope: null, name: 'context-L1' }];
for (const v of snapshot.views ?? []) {
  targets.push({
    level: v.level,
    scope: v.scope ?? null,
    name: `${(v.scope ?? 'context').replace(/\./g, '-')}-${v.level}`,
  });
}

const rendered = [];
for (const t of targets) {
  const view = computeView(snapshot, t.level, t.scope);
  const def = findViewDef(snapshot, t.level, t.scope);
  const layout = await layoutView(new ELK(), view, resolvePins(def, view));
  const svg = viewSvg({ layout, elements: snapshot.elements, colors, fontCss: fonts, footer });
  const file = join(outDir, `${t.name}.svg`);
  writeFileSync(file, svg);
  rendered.push({ ...t, file, svg });
  console.log(`${file}: ${layout.nodes.length} nodes, ${layout.edges.length} edges`);
}

if (png) {
  const { webkit } = await import('@playwright/test');
  const browser = await webkit.launch();
  for (const r of rendered) {
    const size = r.svg.match(/width="(\d+)" height="(\d+)"/);
    const page = await browser.newPage({
      viewport: { width: Number(size[1]), height: Number(size[2]) },
      deviceScaleFactor: scale,
    });
    await page.goto('file://' + resolve(r.file).replace(/\\/g, '/'));
    const out = r.file.replace(/\.svg$/, '.png');
    await page.screenshot({ path: out });
    await page.close();
    console.log(out);
  }
  await browser.close();
}
