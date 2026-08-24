// Bundled user help (docs/roadmap.md 0.4.0 theme 3). Feature-usage pages for
// people *using* Blastradius — deliberately not the ADR/spec set, which is
// contributor material about building it.
//
// The pages are real markdown under ui/help/, fetched same-origin: Tauri
// compiles the whole ui/ tree into the binary (tauri.conf.json frontendDist),
// so help ships offline and versioned with the app, and the same fetch works
// unchanged in the browser test harness. No IPC — a new command would need a
// mock branch in every Playwright run (ADR-0011).

export const HELP_PAGES = [
  { id: 'getting-started', title: 'Getting started', blurb: 'Open a workspace and read your first model.' },
  { id: 'canvas', title: 'Navigating the canvas', blurb: 'Altitudes, diving, and the camera.' },
  { id: 'editing', title: 'Editing the model', blurb: 'Create, connect, rename, pin — and how files stay in sync.' },
  { id: 'deployment', title: 'Deployment views', blurb: 'Model where your containers actually run.' },
  { id: 'code-level', title: 'Code-level detail (L4)', blurb: 'Derive modules and types from real source.' },
  { id: 'git', title: 'Git: diff, history, conflicts', blurb: 'Review architecture changes like code.' },
  { id: 'export', title: 'Sharing and export', blurb: 'Self-contained HTML, images, and CI renders.' },
  { id: 'agents', title: 'Coding agents (MCP)', blurb: 'Let an agent read and edit the model.' },
  { id: 'model-format', title: 'Model format reference', blurb: 'The YAML, field by field.' },
  { id: 'shortcuts', title: 'Keyboard shortcuts', blurb: 'Every binding in the app.' },
  { id: 'privacy', title: 'Privacy', blurb: 'What the app does and does not do with your data.' },
];

const cache = new Map();

/** Markdown source of one help page. Cached — the files never change at runtime. */
export async function helpBody(id) {
  if (cache.has(id)) return cache.get(id);
  const page = HELP_PAGES.find((p) => p.id === id);
  if (!page) return null;
  const res = await fetch(`help/${id}.md`);
  if (!res.ok) return null;
  const text = await res.text();
  cache.set(id, text);
  return text;
}

/**
 * Rewrite links between help pages so they open in the panel instead of
 * navigating the WebView away from the app. `[x](canvas.md)` becomes a
 * data-help button; everything else is left alone.
 */
export function helpLinkTarget(href) {
  const m = /^([a-z0-9-]+)\.md(?:#.*)?$/.exec(href ?? '');
  return m && HELP_PAGES.some((p) => p.id === m[1]) ? m[1] : null;
}
