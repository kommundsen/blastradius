// Blastradius Phase 1 frontend: read-only rendering of one workspace.
// The Core owns truth; this file owns pixels. No write path exists here.

import { computeView, findViewDef, docsFor, treeModel, rootOf, depthOf, liftTo, resolvePins } from './data.js';
import { layoutView, GRID } from './layout.js';

// ---- shell bridge -----------------------------------------------------------
// Real IPC under Tauri; mock (fetch of a committed snapshot) in a plain
// browser, so the frontend is developable and testable headless.
const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke
  ? (cmd, args) => tauri.core.invoke(cmd, args)
  : async (cmd, args) => {
      if (cmd === 'workspace_snapshot') {
        const res = await fetch('mock/snapshot.json');
        return res.json();
      }
      if (cmd === 'workspace_root') return '(mock)';
      // git commands answer from an optional fixture; absent = no repo.
      const git = await fetch('mock/git.json').then((r) => (r.ok ? r.json() : null)).catch(() => null);
      if (cmd === 'git_status') return git?.status ?? null;
      if (cmd === 'git_diff') return git?.diff ?? null;
      if (cmd === 'git_history') return git?.history ?? [];
      if (cmd === 'git_conflicts') return git?.conflicts ?? null;
      if (cmd === 'snapshot_at') return git?.snapshots?.[args?.refspec] ?? null;
      throw new Error('unknown command ' + cmd);
    };
const listen = tauri?.event?.listen ? (ev, cb) => tauri.event.listen(ev, cb) : () => {};

// ---- state ------------------------------------------------------------------
const state = {
  snapshot: null,
  level: 'L1',
  scope: null,        // system id at L2, container id at L3
  selected: null,     // element id
  zoom: 1,            // user zoom on top of the fitted camera
  pan: { x: 0, y: 0 },
  layout: null,       // last layout result
  doc: null,          // open doc id in the side panel, else null
  // ── git (phase 2) ──
  git: null,          // git_status payload | null (no repo)
  conflicts: null,    // git_conflicts payload | null
  diff: null,         // git_diff payload when diff mode is on
  diffOn: false,
  diffBase: null,     // explicit base ref, else server default (merge-base)
  showLayoutDiff: false,
  history: null,      // commit list when the History panel is open
  travel: null,       // { refspec, snapshot } during time-travel
};

const $ = (id) => document.getElementById(id);
const els = {
  breadcrumb: $('breadcrumb'), tree: $('tree'), camera: $('camera'),
  nodes: $('nodes'), edges: $('edges'), edgeLayer: $('edge-layer'),
  canvas: $('canvas'), sideTitle: $('side-title'), sideBody: $('side-body'),
  sideBack: $('side-back'), levelSeg: $('level-seg'), diagChips: $('diag-chips'),
  hint: $('hint'), themeBtn: $('theme-btn'),
  gitChips: $('git-chips'), diffBtn: $('diff-btn'), historyBtn: $('history-btn'),
};

let elk = null;

// ---- boot -------------------------------------------------------------------
window.addEventListener('DOMContentLoaded', async () => {
  elk = new ELK();
  await reload();
  listen('workspace-changed', () => reload());
  wireChrome();
});

async function reload() {
  if (state.travel) {
    // Time-travelling renders a fixed revision; external edits only refresh
    // the git chrome so the user sees new commits/conflicts appear.
    await refreshGit();
    renderGitChrome();
    return;
  }
  try {
    state.snapshot = await invoke('workspace_snapshot');
  } catch (e) {
    els.breadcrumb.textContent = 'No workspace — launch as: blastradius-app <workspace-dir>';
    return;
  }
  await refreshGit();
  // default scope: first system
  if (!state.scopeInit) {
    const sys = state.snapshot.elements.find((e) => e.kind === 'system');
    state.defaultSystem = sys?.id ?? null;
    state.scopeInit = true;
  }
  renderDiagnostics();
  renderGitChrome();
  renderTree();
  await renderCanvas({ animate: false });
  renderSide();
}

async function refreshGit() {
  try {
    state.git = await invoke('git_status');
    state.conflicts = state.git ? await invoke('git_conflicts') : null;
    if (state.diffOn && state.git) {
      state.diff = await invoke('git_diff', { base: state.diffBase });
    } else if (!state.diffOn) {
      state.diff = null;
    }
  } catch (e) {
    state.git = null; state.conflicts = null; state.diff = null;
  }
}

// ---- canvas -----------------------------------------------------------------
async function renderCanvas({ animate = true } = {}) {
  const snap = effectiveSnapshot();
  const view = computeView(snap, state.level, state.scope);
  const viewDef = findViewDef(snap, state.level, state.scope);
  const layout = await layoutView(elk, view, resolvePins(viewDef, view));
  state.layout = layout;

  els.camera.classList.toggle('no-anim', !animate);

  // nodes
  els.nodes.textContent = '';
  const elById = new Map(snap.elements.map((e) => [e.id, e]));
  const changeById = diffChangeMap();
  const conflictById = conflictMap();
  const movedPins = state.showLayoutDiff ? movedPinIds(viewDef) : new Set();
  for (const n of layout.nodes) {
    const el = elById.get(n.id);
    const div = document.createElement('div');
    div.className = nodeClass(el);
    const conflict = conflictById.get(n.id);
    const change = changeById.get(n.id);
    if (conflict) div.classList.add('is-conflict');
    else if (change) div.classList.add('is-' + change);
    const badge = conflict ? ['!', 'Merge conflict']
      : change === 'added' ? ['+', 'Added vs base']
      : change === 'removed' ? ['−', 'Removed vs base']
      : change === 'changed' ? ['~', 'Modified vs base']
      : movedPins.has(n.id) ? ['⌖', 'Pin moved (layout only)']
      : null;
    div.style.cssText = `left:${n.x}px;top:${n.y}px;width:${n.width}px;position:absolute`;
    div.tabIndex = 0;
    div.setAttribute('role', 'button');
    div.dataset.id = n.id;
    if (state.selected === n.id) div.classList.add('is-active');
    div.innerHTML =
      `<span class="node-kicker">${esc(kicker(el))}</span>` +
      `<span class="node-title">${esc(el.name)}</span>` +
      (childCount(el) ? `<span class="node-meta">${childCount(el)}</span>` : '');
    if (badge) {
      const b = document.createElement('span');
      b.className = 'node-badge';
      b.title = badge[1];
      b.innerHTML = `<span aria-hidden="true">${badge[0]}</span><span class="sr-only">${badge[1]}</span>`;
      div.appendChild(b);
    }
    div.addEventListener('click', () => select(n.id));
    div.addEventListener('dblclick', () => dive(n.id));
    div.addEventListener('keydown', (ev) => {
      if (ev.key === 'Enter') { ev.preventDefault(); dive(n.id); }
      if (ev.key === ' ') { ev.preventDefault(); select(n.id); }
    });
    els.nodes.appendChild(div);
  }

  // edges
  els.edges.textContent = '';
  const svgNS = 'http://www.w3.org/2000/svg';
  for (const e of state.layout.edges) {
    const d = e.points.map((p, i) => (i ? 'L' : 'M') + p.x + ',' + p.y).join(' ');
    const path = document.createElementNS(svgNS, 'path');
    let cls = 'edge';
    if (e.direction === 'both') cls += ' is-bidirectional';
    if (e.direction === 'none') cls += ' is-undirected';
    if (!e.exact) cls += ' is-secondary';
    const relChange = diffRelChange(e.from, e.to);
    if (relChange === 'added') cls += ' is-added';
    if (relChange === 'removed') cls += ' is-removed';
    path.setAttribute('class', cls);
    path.setAttribute('d', d);
    els.edges.appendChild(path);
    const label = e.label ?? e.protocol;
    if (label) {
      const text = document.createElementNS(svgNS, 'text');
      text.setAttribute('class', 'edge-label');
      text.setAttribute('x', e.labelAt.x);
      text.setAttribute('y', e.labelAt.y);
      text.setAttribute('text-anchor', 'middle');
      text.textContent = e.protocol && e.label ? `${e.label} · ${e.protocol}` : label;
      els.edges.appendChild(text);
    }
  }

  applyCamera();
  renderBreadcrumb();
  syncLevelSeg();
}

function nodeClass(el) {
  const map = { person: 'is-person', system: 'is-system', container: 'is-container', component: 'is-component', external: 'is-system' };
  let cls = 'node ' + (map[el.kind] ?? 'is-system');
  if (el.external) cls += ' is-external';
  return cls;
}

function kicker(el) {
  const kind = { person: 'Person', system: 'Software system', container: 'Container', component: 'Component', external: 'External system' }[el.kind];
  const label = el.external && el.kind === 'system' ? 'External system' : kind;
  return el.tech ? `${label} · ${el.tech}` : label;
}

function childCount(el) {
  const kids = state.snapshot.elements.filter((e) => e.parent === el.id).length;
  if (!kids) return null;
  const noun = el.kind === 'system' ? 'container' : 'component';
  return `${kids} ${noun}${kids > 1 ? 's' : ''}`;
}

function applyCamera() {
  const c = els.canvas.getBoundingClientRect();
  const l = state.layout;
  // fit-to-content scale, then user zoom on top
  const fit = Math.min(1, (c.width - 40) / l.width, (c.height - 40) / l.height);
  const scale = fit * state.zoom;
  const tx = (c.width - l.width * scale) / 2 + state.pan.x;
  const ty = (c.height - l.height * scale) / 2 + state.pan.y;
  els.camera.style.transform = `translate(${tx}px, ${ty}px) scale(${scale})`;
  els.camera.style.setProperty('--camera-scale', scale);
  $('zoom-reset').textContent = Math.round(scale * 100) + '%';
}

// ---- navigation -------------------------------------------------------------
function select(id) {
  state.selected = id;
  state.doc = null;
  for (const div of els.nodes.children) {
    div.classList.toggle('is-active', div.dataset.id === id);
  }
  renderTree();
  renderSide();
}

async function dive(id) {
  const el = state.snapshot.elements.find((e) => e.id === id);
  if (!el) return;
  if (el.kind === 'system' && !el.external && state.level === 'L1') {
    state.level = 'L2'; state.scope = id;
  } else if (el.kind === 'container' && state.level === 'L2') {
    if (!state.snapshot.elements.some((e) => e.parent === id)) return; // nothing inside
    state.level = 'L3'; state.scope = id;
  } else {
    return;
  }
  state.zoom = 1; state.pan = { x: 0, y: 0 };
  state.selected = id;
  await renderCanvas();
  renderSide();
}

async function rise() {
  if (state.level === 'L3') {
    state.selected = state.scope;
    state.scope = liftTo(state.scope, depthOf(state.scope) - 1);
    state.level = 'L2';
  } else if (state.level === 'L2') {
    state.selected = state.scope;
    state.scope = null;
    state.level = 'L1';
  } else {
    return;
  }
  state.zoom = 1; state.pan = { x: 0, y: 0 };
  await renderCanvas();
  renderSide();
}

async function setLevel(level) {
  if (level === state.level) return;
  if (level === 'L1') { state.scope = null; }
  if (level === 'L2') {
    state.scope = state.scope
      ? rootOf(state.selected ?? state.scope)
      : rootOf(state.selected ?? state.defaultSystem ?? '');
    if (!state.scope) return;
  }
  if (level === 'L3') {
    // need a container scope: selected container, or first container of current scope
    const sel = state.selected && state.snapshot.elements.find((e) => e.id === state.selected);
    const container = sel?.kind === 'container' ? sel.id
      : state.snapshot.elements.find((e) => e.kind === 'container' && (!state.scope || e.id.startsWith(rootOf(state.scope))))?.id;
    if (!container) return;
    state.scope = container;
  }
  state.level = level;
  state.zoom = 1; state.pan = { x: 0, y: 0 };
  await renderCanvas();
}

// ---- chrome -----------------------------------------------------------------
function wireChrome() {
  els.levelSeg.addEventListener('change', (ev) => {
    if (ev.target.name === 'lvl') setLevel(ev.target.value);
  });
  $('zoom-in').addEventListener('click', () => { state.zoom *= 1.2; applyCamera(); });
  $('zoom-out').addEventListener('click', () => { state.zoom /= 1.2; applyCamera(); });
  $('zoom-reset').addEventListener('click', () => { state.zoom = 1; state.pan = { x: 0, y: 0 }; applyCamera(); });
  els.sideBack.addEventListener('click', () => { state.doc = null; state.history = null; renderSide(); });
  els.diffBtn.addEventListener('click', toggleDiff);
  els.historyBtn.addEventListener('click', openHistory);

  // theme cycle: auto -> light -> dark
  let theme = 'auto';
  els.themeBtn.addEventListener('click', () => {
    theme = theme === 'auto' ? 'light' : theme === 'light' ? 'dark' : 'auto';
    if (theme === 'auto') document.documentElement.removeAttribute('data-theme');
    else document.documentElement.setAttribute('data-theme', theme);
    els.themeBtn.textContent = 'Theme: ' + theme;
  });

  // keyboard on the canvas
  els.canvas.addEventListener('keydown', async (ev) => {
    const order = state.layout?.nodes.map((n) => n.id) ?? [];
    const idx = order.indexOf(state.selected);
    if (ev.key === 'ArrowRight' || ev.key === 'ArrowDown') {
      ev.preventDefault(); select(order[(idx + 1) % order.length] ?? order[0]);
    } else if (ev.key === 'ArrowLeft' || ev.key === 'ArrowUp') {
      ev.preventDefault(); select(order[(idx - 1 + order.length) % order.length] ?? order[0]);
    } else if (ev.key === 'Enter' && state.selected) {
      ev.preventDefault(); dive(state.selected);
    } else if (ev.key === 'Escape' || ev.key === 'Backspace') {
      ev.preventDefault(); rise();
    } else if (ev.key === '+' || ev.key === '=') {
      state.zoom *= 1.2; applyCamera();
    } else if (ev.key === '-') {
      state.zoom /= 1.2; applyCamera();
    } else if (ev.key === '0') {
      state.zoom = 1; state.pan = { x: 0, y: 0 }; applyCamera();
    }
  });

  // drag to pan
  let drag = null;
  els.canvas.addEventListener('pointerdown', (ev) => {
    if (ev.target.closest('.node') || ev.target.closest('.canvas-overlay')) return;
    drag = { x: ev.clientX - state.pan.x, y: ev.clientY - state.pan.y };
    els.camera.classList.add('no-anim');
  });
  window.addEventListener('pointermove', (ev) => {
    if (!drag) return;
    state.pan = { x: ev.clientX - drag.x, y: ev.clientY - drag.y };
    applyCamera();
  });
  window.addEventListener('pointerup', () => { drag = null; els.camera.classList.remove('no-anim'); });

  window.addEventListener('resize', () => state.layout && applyCamera());
}

function renderBreadcrumb() {
  const snap = state.snapshot;
  const parts = [esc(snap.name)];
  if (state.scope) {
    const segs = state.scope.split('.');
    for (let i = 1; i <= segs.length; i++) {
      const id = segs.slice(0, i).join('.');
      const el = snap.elements.find((e) => e.id === id);
      if (el) parts.push(`<b>${esc(el.name)}</b>`);
    }
  }
  parts.push({ L1: 'Context', L2: 'Containers', L3: 'Components' }[state.level]);
  els.breadcrumb.innerHTML = parts.join(' / ');
}

function syncLevelSeg() {
  for (const input of els.levelSeg.querySelectorAll('input')) {
    input.checked = input.value === state.level;
    input.closest('.seg-opt').classList.toggle('is-active', input.value === state.level);
  }
}

// ---- tree -------------------------------------------------------------------
function renderTree() {
  const t = treeModel(effectiveSnapshot());
  const rows = [];
  rows.push(`<span class="tree-label">Model</span>`);
  for (const c of t.context) rows.push(treeRow(c.el ?? c, 0, '◦'));
  for (const s of t.systems) {
    rows.push(treeRow(s.el, 0, '▸'));
    for (const c of s.containers) {
      rows.push(treeRow(c.el, 1, ''));
      for (const k of c.components) rows.push(treeRow(k, 2, ''));
    }
  }
  els.tree.innerHTML = rows.join('');
  for (const btn of els.tree.querySelectorAll('.tree-row[data-id]')) {
    btn.addEventListener('click', () => focusElement(btn.dataset.id));
    btn.addEventListener('dblclick', () => dive(btn.dataset.id));
  }
}

function treeRow(el, depth, glyph) {
  let active = state.selected === el.id ? ' is-active' : '';
  const change = diffChangeMap().get(el.id);
  if (change === 'added') active += ' is-added';
  if (change === 'removed') active += ' is-removed';
  const pad = depth ? ` style="padding-left:${14 + depth * 14}px"` : '';
  return `<button class="tree-row${active}" data-id="${esc(el.id)}"${pad}>` +
    `<span class="glyph">${glyph}</span>${esc(el.name)}</button>`;
}

/** Select an element; if it is not visible at the current altitude, go to it. */
async function focusElement(id) {
  const el = state.snapshot.elements.find((e) => e.id === id);
  if (!el) return;
  const visible = state.layout?.nodes.some((n) => n.id === id);
  if (!visible) {
    const d = depthOf(id);
    if (el.kind === 'person' || el.kind === 'external' || el.kind === 'system') {
      state.level = 'L1'; state.scope = null;
    } else if (d === 2) {
      state.level = 'L2'; state.scope = rootOf(id);
    } else {
      state.level = 'L3'; state.scope = liftTo(id, 2);
    }
    state.zoom = 1; state.pan = { x: 0, y: 0 };
    state.selected = id;
    await renderCanvas();
    renderSide();
    renderTree();
    return;
  }
  select(id);
}

// ---- side panel -------------------------------------------------------------
function renderSide() {
  if (state.history) return renderHistory();
  if (state.doc) return renderDoc(state.doc);
  els.sideBack.hidden = true;
  const id = state.selected;
  if (!id) {
    els.sideTitle.textContent = 'Inspector';
    els.sideBody.innerHTML = `<p class="side-empty text-muted">Select an element to inspect it.</p>`;
    return;
  }
  const snap = effectiveSnapshot();
  const el = snap.elements.find((e) => e.id === id);
  if (!el) return;
  els.sideTitle.textContent = 'Inspector';

  const rels = snap.relations.filter((r) => r.from === id || r.to === id);
  const docs = docsFor(snap, id);
  const nameOf = (eid) => snap.elements.find((e) => e.id === eid)?.name ?? eid;

  let html = `<div class="insp">`;
  html += `<span class="insp-kicker">${esc(kicker(el))}</span>`;
  html += `<span class="insp-title">${esc(el.name)}</span>`;
  html += `<span class="mono text-muted" style="font-family:var(--font-mono);font-size:var(--text-2xs)">${esc(el.id)}</span>`;
  if (el.description) html += `<p class="insp-desc">${esc(el.description)}</p>`;

  if (rels.length) {
    html += `<div class="insp-section">Relations</div>`;
    for (const r of rels) {
      const out = r.from === id;
      const arrow = r.direction === 'both' ? '↔' : out ? '→' : '←';
      const other = out ? r.to : r.from;
      html += `<div class="insp-rel">${arrow} ${esc(nameOf(other))}` +
        (r.label ? ` <span class="text-muted">· ${esc(r.label)}</span>` : '') +
        (r.protocol ? ` <span class="proto">${esc(r.protocol)}</span>` : '') + `</div>`;
    }
  }

  html += `<div class="insp-section">Documents</div>`;
  if (docs.length) {
    for (const d of docs) {
      html += `<button class="doc-link" data-doc="${esc(d.id)}">` +
        `<span class="tag tag-outline">${esc(d.type)}</span> ${esc(d.title)}</button>`;
    }
  } else {
    html += `<span class="text-muted" style="font-size:var(--text-sm)">None linked.</span>`;
  }
  html += conflictSection(id);
  html += `</div>`;
  els.sideBody.innerHTML = html;
  for (const btn of els.sideBody.querySelectorAll('[data-doc]')) {
    btn.addEventListener('click', () => { state.doc = btn.dataset.doc; renderSide(); });
  }
  for (const btn of els.sideBody.querySelectorAll('[data-editfile]')) {
    btn.addEventListener('click', () => invoke('open_in_editor', { rel: btn.dataset.editfile }).catch(() => {}));
  }
}

function renderDoc(docId) {
  const snap = state.snapshot;
  const d = snap.docs.find((x) => x.id === docId);
  if (!d) return;
  els.sideTitle.textContent = d.id;
  els.sideBack.hidden = false;

  let html = `<div class="doc-meta">` +
    `<span class="tag tag-outline">${esc(d.type)}</span>` +
    (d.status ? `<span class="tag tag-neutral">${esc(d.status)}</span>` : '') +
    `<span class="text-muted" style="font-size:var(--text-2xs);font-family:var(--font-mono)">${esc(d.file)}</span>` +
    `</div>`;
  if (d.elements.length) {
    html += `<div class="doc-elements">`;
    for (const eid of d.elements) {
      html += `<button class="doc-link" data-el="${esc(eid)}">◦ ${esc(eid)}</button>`;
    }
    html += `</div>`;
  }
  html += `<div class="doc-body">${marked.parse(d.body)}</div>`;
  els.sideBody.innerHTML = html;
  for (const btn of els.sideBody.querySelectorAll('[data-el]')) {
    btn.addEventListener('click', () => focusElement(btn.dataset.el));
  }
}

// ---- diagnostics ------------------------------------------------------------
function renderDiagnostics() {
  const diags = state.snapshot.diagnostics ?? [];
  const errs = diags.filter((d) => d.severity === 'error').length;
  const warns = diags.filter((d) => d.severity === 'warning').length;
  let html = '';
  if (errs) html += `<button class="tag tag-danger" id="diag-btn">${errs} error${errs > 1 ? 's' : ''}</button>`;
  else if (warns) html += `<button class="tag tag-warning" id="diag-btn">${warns} warning${warns > 1 ? 's' : ''}</button>`;
  els.diagChips.innerHTML = html;
  document.querySelector('.diag-list')?.remove();
  if (html) {
    $('diag-btn').addEventListener('click', () => {
      const existing = document.querySelector('.diag-list');
      if (existing) return existing.remove();
      const list = document.createElement('div');
      list.className = 'diag-list';
      list.innerHTML = diags
        .filter((d) => d.severity !== 'info')
        .map((d) => `<div>${esc(d.severity)}: ${esc(d.file)}${d.line ? ':' + d.line : ''} — ${esc(d.message)}</div>`)
        .join('');
      els.canvas.appendChild(list);
    });
  }
}

// ---- git: diff, conflicts, history, time-travel (phase 2) -------------------

/** The snapshot being rendered: the working tree, a travelled revision, and —
 * in diff mode — augmented with base-side ghosts for removed elements and
 * removed relations, so deletions stay reviewable (spec/git-and-diff.md). */
function effectiveSnapshot() {
  const snap = state.travel ? state.travel.snapshot : state.snapshot;
  if (!state.diffOn || !state.diff || state.travel) return snap;
  const have = new Set(snap.elements.map((e) => e.id));
  const ghosts = state.diff.elements
    .filter((d) => d.change === 'removed' && !have.has(d.id))
    .map((d) => d.element);
  const ghostRels = state.diff.relations
    .filter((r) => r.change === 'removed')
    .map((r) => ({ from: r.from, to: r.to, label: r.label ?? null, protocol: null, direction: 'forward' }));
  if (!ghosts.length && !ghostRels.length) return snap;
  return { ...snap, elements: [...snap.elements, ...ghosts], relations: [...snap.relations, ...ghostRels] };
}

function diffChangeMap() {
  if (!state.diffOn || !state.diff) return new Map();
  return new Map(state.diff.elements.map((d) => [d.id, d.change]));
}

function diffRelChange(from, to) {
  if (!state.diffOn || !state.diff) return null;
  // Rendered edges are lifted to the current altitude (data.computeView), so a
  // diff relation matches when each endpoint is the rendered id or one of its
  // descendants. First match wins on aggregated edges.
  const under = (id, ancestor) => id === ancestor || id.startsWith(ancestor + '.');
  const hit = state.diff.relations.find((r) => under(r.from, from) && under(r.to, to));
  return hit?.change ?? null;
}

function conflictMap() {
  const out = new Map();
  for (const c of state.conflicts?.elements ?? []) out.set(c.id, c);
  return out;
}

/** Pins listed as moved for the current view, resolved to full ids. */
function movedPinIds(viewDef) {
  const out = new Set();
  if (!state.diffOn || !state.diff || !viewDef) return out;
  const change = state.diff.layout.find((l) => l.view === viewDef.id);
  for (const pin of change?.pins ?? []) {
    out.add(viewDef.scope ? viewDef.scope + '.' + pin : pin);
    out.add(pin);
  }
  return out;
}

function renderGitChrome() {
  const g = state.git;
  els.diffBtn.hidden = !g;
  els.historyBtn.hidden = !g;
  if (!g) {
    els.gitChips.innerHTML = '';
    return;
  }
  let html = `<span class="tag tag-neutral" style="font-family:var(--font-mono)">⎇ ${esc(g.branch)}</span>`;
  if (g.conflicted.length) {
    html += `<span class="tag tag-danger">${g.conflicted.length} conflicted</span>`;
  } else if (g.dirty) {
    html += `<span class="tag tag-neutral">${g.dirty} modified</span>`;
  }
  if (g.ahead) html += `<span class="tag tag-neutral">↑${g.ahead}</span>`;
  if (g.behind) html += `<span class="tag tag-neutral">↓${g.behind}</span>`;
  if (state.diffOn && state.diff) {
    const n = (c) => state.diff.elements.filter((e) => e.change === c).length;
    if (n('added')) html += `<span class="tag tag-success">${n('added')} added</span>`;
    if (n('changed')) html += `<span class="tag tag-warning">${n('changed')} changed</span>`;
    if (n('removed')) html += `<span class="tag tag-danger">${n('removed')} removed</span>`;
    if (state.diff.layout.length) {
      const pins = state.diff.layout.reduce((a, l) => a + l.pins.length, 0);
      html += `<button class="tag ${state.showLayoutDiff ? 'tag-accent' : 'tag-neutral'}" id="layout-toggle"
        title="Layout-only changes are excluded from the diff — toggle to mark moved pins">⌖ ${pins} pin${pins > 1 ? 's' : ''}</button>`;
    }
    if (!state.diff.elements.length && !state.diff.relations.length) {
      html += `<span class="tag tag-neutral">no semantic changes</span>`;
    }
  }
  els.gitChips.innerHTML = html;
  document.getElementById('layout-toggle')?.addEventListener('click', async () => {
    state.showLayoutDiff = !state.showLayoutDiff;
    renderGitChrome();
    await renderCanvas({ animate: false });
  });
  els.diffBtn.classList.toggle('is-on', state.diffOn);
}

async function toggleDiff() {
  state.diffOn = !state.diffOn;
  if (state.diffOn) {
    state.diff = await invoke('git_diff', { base: state.diffBase });
    if (!state.diff) {
      state.diffOn = false; // no repo / no base
    }
  } else {
    state.diff = null;
    state.showLayoutDiff = false;
  }
  renderGitChrome();
  renderTree();
  await renderCanvas({ animate: false });
}

async function openHistory() {
  state.history = await invoke('git_history');
  state.doc = null;
  renderHistory();
}

function renderHistory() {
  els.sideTitle.textContent = 'History';
  els.sideBack.hidden = false;
  const rows = (state.history ?? []).map((c) => {
    const when = new Date(c.time * 1000).toISOString().slice(0, 10);
    const isBase = state.diffBase === c.id;
    return `<div class="hist-row${isBase ? ' is-base' : ''}">
      <span class="sum">${esc(c.summary)}</span>
      <span class="meta">${esc(c.short)} · ${esc(c.author)} · ${when}</span>
      <span class="acts">
        <button class="btn btn-ghost" data-view="${esc(c.id)}">view</button>
        <button class="btn btn-ghost" data-base="${esc(c.id)}">${isBase ? 'base ✓' : 'set as diff base'}</button>
      </span>
    </div>`;
  });
  els.sideBody.innerHTML = rows.join('') ||
    `<p class="side-empty text-muted">No commits touch this workspace yet.</p>`;
  for (const b of els.sideBody.querySelectorAll('[data-view]')) {
    b.addEventListener('click', () => travelTo(b.dataset.view));
  }
  for (const b of els.sideBody.querySelectorAll('[data-base]')) {
    b.addEventListener('click', async () => {
      state.diffBase = b.dataset.base;
      state.diffOn = true;
      state.diff = await invoke('git_diff', { base: state.diffBase });
      renderGitChrome();
      renderHistory();
      renderTree();
      await renderCanvas({ animate: false });
    });
  }
}

async function travelTo(refspec) {
  const snapshot = await invoke('snapshot_at', { refspec });
  if (!snapshot) return;
  state.travel = { refspec, snapshot };
  state.selected = null;
  renderTravelBanner();
  renderTree();
  await renderCanvas({ animate: false });
}

async function returnToPresent() {
  state.travel = null;
  document.querySelector('.travel-banner')?.remove();
  await reload();
}

function renderTravelBanner() {
  document.querySelector('.travel-banner')?.remove();
  const b = document.createElement('div');
  b.className = 'travel-banner';
  b.innerHTML = `<span>Viewing <b style="font-family:var(--font-mono)">${esc(state.travel.refspec.slice(0, 8))}</b> — read-only</span>
    <button class="btn btn-ghost" id="travel-return">Return to working tree</button>`;
  els.canvas.appendChild(b);
  document.getElementById('travel-return').addEventListener('click', returnToPresent);
}

/** Conflict details for the inspector, when the selected element is conflicted. */
function conflictSection(id) {
  const c = conflictMap().get(id);
  if (!c) return '';
  const row = (label, side) => side
    ? `<tr><td>${esc(label)}</td><td>${esc(side.name)}</td><td>${esc(side.tech ?? '')}</td></tr>`
    : `<tr><td>${esc(label)}</td><td colspan="2" class="text-muted">not present</td></tr>`;
  const files = (state.git?.conflicted ?? [])
    .map((f) => `<button class="doc-link" data-editfile="${esc(f)}">↗ resolve ${esc(f)} in editor</button>`)
    .join('');
  return `<div class="insp-section">Merge conflict</div>
    <table class="conf-table">
      <thead><tr><th>side</th><th>name</th><th>tech</th></tr></thead>
      <tbody>${row('ours', c.ours)}${row('theirs', c.theirs)}</tbody>
    </table>${files}`;
}

// ---- util -------------------------------------------------------------------
function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
