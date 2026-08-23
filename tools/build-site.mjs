// Docs site generator (Phase 5). Renders docs/ — which is itself a valid
// Blastradius workspace — into a static site under site/, styled by the
// design system and using the vendored marked (no npm dependencies).
//
//   node tools/build-site.mjs
//
// If architecture.html exists at the repo root (CI builds it via
// `blastradius export docs -o architecture.html`), it is bundled in and the
// site links to the live model — the dogfood, published.
import { createRequire } from 'module';
import { cpSync, mkdirSync, readFileSync, writeFileSync, existsSync, readdirSync, rmSync } from 'fs';
import { join, dirname, relative } from 'path';
import { fileURLToPath } from 'url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'site');
const marked = createRequire(import.meta.url)(join(root, 'ui/vendor/marked.min.js'));

// same globs as docs/blastradius.yaml declares for its docs
const sources = ['', 'adr', 'spec']
  .flatMap((dir) =>
    readdirSync(join(root, 'docs', dir), { withFileTypes: true })
      .filter((e) => e.isFile() && e.name.endsWith('.md'))
      .map((e) => join(dir, e.name).replaceAll('\\', '/')),
  );

function frontmatter(text) {
  const m = text.match(/^---\n([\s\S]*?)\n---\n/);
  if (!m) return [{}, text];
  const meta = {};
  for (const line of m[1].split('\n')) {
    const kv = line.match(/^(\w[\w-]*):\s*(.*)$/);
    if (kv) meta[kv[1]] = kv[2].replace(/^\[|\]$/g, '');
  }
  return [meta, text.slice(m[0].length)];
}

function title(meta, body, rel) {
  const h1 = body.match(/^#\s+(.*)$/m);
  return h1 ? h1[1] : meta.doc ?? rel;
}

const TYPE_ORDER = ['prd', 'roadmap', 'spec', 'adr', 'note'];
const TYPE_LABEL = {
  prd: 'Product', roadmap: 'Roadmap', spec: 'Specifications',
  adr: 'Architecture decisions', note: 'Notes',
};

function page({ rel, docTitle, meta, html }) {
  const depth = rel.split('/').length - 1;
  const base = '../'.repeat(depth);
  const chips = [meta.type, meta.status].filter(Boolean)
    .map((c) => `<span class="tag tag-accent">${c}</span>`).join(' ');
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${docTitle} — Blastradius</title>
<link rel="stylesheet" href="${base}ds/styles.css">
<link rel="stylesheet" href="${base}site.css">
</head>
<body>
<header class="site-bar">
  <a class="site-brand" href="${base}index.html"><img src="${base}ds/assets/mark.svg" width="14" height="14" alt=""> Blastradius</a>
  <span class="site-bar-spacer"></span>
  <a class="btn btn-secondary" href="${base}architecture.html">Live model</a>
  <a class="btn btn-secondary" href="https://github.com/kommundsen/blastradius">GitHub</a>
</header>
<main class="prose">
  <p class="doc-chips">${chips}</p>
${html}
</main>
</body>
</html>
`;
}

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
cpSync(join(root, 'ui/ds'), join(out, 'ds'), { recursive: true });

const docs = [];
for (const rel of sources) {
  const raw = readFileSync(join(root, 'docs', rel), 'utf8');
  const [meta, body] = frontmatter(raw);
  const docTitle = title(meta, body, rel);
  let html = marked.parse(body);
  // markdown cross-links become site links
  html = html.replaceAll(/href="([^"]+)\.md"/g, 'href="$1.html"');
  const target = rel.replace(/\.md$/, '.html');
  mkdirSync(join(out, dirname(target)), { recursive: true });
  writeFileSync(join(out, target), page({ rel, docTitle, meta, html }));
  docs.push({ rel: target, docTitle, meta });
}

// index: hero + live model + docs grouped by type
const groups = TYPE_ORDER.map((t) => ({
  label: TYPE_LABEL[t],
  items: docs.filter((d) => (d.meta.type ?? 'note') === t),
})).filter((g) => g.items.length);

const hasModel = existsSync(join(root, 'architecture.html'));
if (hasModel) cpSync(join(root, 'architecture.html'), join(out, 'architecture.html'));

writeFileSync(join(out, 'index.html'), `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Blastradius — interactive C4 models, versioned by git</title>
<link rel="stylesheet" href="ds/styles.css">
<link rel="stylesheet" href="site.css">
</head>
<body>
<header class="site-bar">
  <a class="site-brand" href="index.html"><img src="ds/assets/mark.svg" width="14" height="14" alt=""> Blastradius</a>
  <span class="site-bar-spacer"></span>
  <a class="btn btn-secondary" href="https://github.com/kommundsen/blastradius">GitHub</a>
</header>
<main class="prose site-index">
  <h1>Model your architecture</h1>
  <p class="site-lede">Interactive C4 models as plain YAML in your repo —
  local-first, versioned by git, diffable in PRs. This site is generated from
  the <code>docs/</code> folder, which is itself a valid Blastradius workspace
  modelling Blastradius.</p>
  ${hasModel
    ? '<p><a class="btn btn-primary" href="architecture.html">Explore the live model →</a></p>'
    : '<p class="text-muted">(architecture.html not built — run <code>blastradius export docs -o architecture.html</code> first)</p>'}
  ${groups.map((g) => `<h2>${g.label}</h2>\n<ul class="doc-list">\n${g.items
    .map((d) => `  <li><a href="${d.rel}">${d.docTitle}</a>${d.meta.status ? ` <span class="tag tag-accent">${d.meta.status}</span>` : ''}</li>`)
    .join('\n')}\n</ul>`).join('\n')}
</main>
</body>
</html>
`);

writeFileSync(join(out, 'site.css'), `/* docs site chrome — everything else is the design system */
body { margin: 0; background: var(--color-bg); color: var(--color-text); font-family: var(--font-body); }
.site-bar { display: flex; align-items: center; gap: var(--space-3);
  padding: var(--space-3) var(--space-5); border-bottom: 1px solid var(--color-border); }
.site-brand { display: inline-flex; align-items: center; gap: var(--space-2);
  font-family: var(--font-heading); font-weight: var(--font-heading-weight); letter-spacing: .04em;
  color: var(--color-text); text-decoration: none; text-transform: uppercase; }
.site-bar-spacer { flex: 1; }
.site-bar .btn { text-decoration: none; }
.prose { max-width: 46rem; margin: 0 auto; padding: var(--space-6) var(--space-5) var(--space-8); }
.prose h1, .prose h2, .prose h3 { font-family: var(--font-heading); font-weight: var(--font-heading-weight); letter-spacing: .02em; }
.prose pre { background: var(--color-surface); border: 1px solid var(--color-border);
  padding: var(--space-3); overflow-x: auto; font-size: var(--text-xs); }
.prose code { font-family: var(--font-mono); font-size: 0.92em; }
.prose a { color: var(--color-accent-ink, var(--color-accent)); }
.prose a.btn { text-decoration: none; }
.prose a.btn-primary { color: var(--color-on-accent); }
.prose table { border-collapse: collapse; font-size: var(--text-sm); }
.prose th, .prose td { border: 1px solid var(--color-border); padding: var(--space-1) var(--space-2); text-align: left; }
.prose blockquote { border-left: 3px solid var(--color-accent); margin-left: 0;
  padding-left: var(--space-3); color: var(--color-text-muted); }
.doc-chips { display: flex; gap: var(--space-2); }
.doc-list { list-style: none; padding: 0; }
.doc-list li { padding: var(--space-1) 0; border-bottom: 1px solid var(--color-border); }
.site-lede { font-size: var(--text-lg); color: var(--color-text-muted); }
`);

console.log(`site/: index + ${docs.length} docs${hasModel ? ' + live model' : ''}`);
