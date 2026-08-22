// Blastradius Phase 1 frontend: read-only rendering of one workspace.
// The Core owns truth; this file owns pixels. No write path exists here.

import { computeView, findViewDef, docsFor, treeModel, rootOf, depthOf, liftTo, resolvePins } from './data.js';
import { layoutView, GRID } from './layout.js';

// ---- shell bridge -----------------------------------------------------------
// Real IPC under Tauri; mock (fetch of a committed snapshot) in a plain
// browser, so the frontend is developable and testable headless.
const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke
  ? (cmd) => tauri.core.invoke(cmd)
  : async (cmd) => {
      if (cmd === 'workspace_snapshot') {
        const res = await fetch('mock/snapshot.json');
        return res.json();
      }
      if (cmd === 'workspace_root') return '(mock)';
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
};

const $ = (id) => document.getElementById(id);
const els = {
  breadcrumb: $('breadcrumb'), tree: $('tree'), camera: $('camera'),
  nodes: $('nodes'), edges: $('edges'), edgeLayer: $('edge-layer'),
  canvas: $('canvas'), sideTitle: $('side-title'), sideBody: $('side-body'),
  sideBack: $('side-back'), levelSeg: $('level-seg'), diagChips: $('diag-chips'),
  hint: $('hint'), themeBtn: $('theme-btn'),
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
  try {
    state.snapshot = await invoke('workspace_snapshot');
  } catch (e) {
    els.breadcrumb.textContent = 'No workspace — launch as: blastradius-app <workspace-dir>';
    return;
  }
  // default scope: first system
  if (!state.scopeInit) {
    const sys = state.snapshot.elements.find((e) => e.kind === 'system');
    state.defaultSystem = sys?.id ?? null;
    state.scopeInit = true;
  }
  renderDiagnostics();
  renderTree();
  await renderCanvas({ animate: false });
  renderSide();
}

// ---- canvas -----------------------------------------------------------------
async function renderCanvas({ animate = true } = {}) {
  const snap = state.snapshot;
  const view = computeView(snap, state.level, state.scope);
  const viewDef = findViewDef(snap, state.level, state.scope);
  const layout = await layoutView(elk, view, resolvePins(viewDef, view));
  state.layout = layout;

  els.camera.classList.toggle('no-anim', !animate);

  // nodes
  els.nodes.textContent = '';
  const elById = new Map(snap.elements.map((e) => [e.id, e]));
  for (const n of layout.nodes) {
    const el = elById.get(n.id);
    const div = document.createElement('div');
    div.className = nodeClass(el);
    div.style.cssText = `left:${n.x}px;top:${n.y}px;width:${n.width}px;position:absolute`;
    div.tabIndex = 0;
    div.setAttribute('role', 'button');
    div.dataset.id = n.id;
    if (state.selected === n.id) div.classList.add('is-active');
    div.innerHTML =
      `<span class="node-kicker">${esc(kicker(el))}</span>` +
      `<span class="node-title">${esc(el.name)}</span>` +
      (childCount(el) ? `<span class="node-meta">${childCount(el)}</span>` : '');
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
  els.sideBack.addEventListener('click', () => { state.doc = null; renderSide(); });

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
  const t = treeModel(state.snapshot);
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
  const active = state.selected === el.id ? ' is-active' : '';
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
  if (state.doc) return renderDoc(state.doc);
  els.sideBack.hidden = true;
  const id = state.selected;
  if (!id) {
    els.sideTitle.textContent = 'Inspector';
    els.sideBody.innerHTML = `<p class="side-empty text-muted">Select an element to inspect it.</p>`;
    return;
  }
  const snap = state.snapshot;
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
  html += `</div>`;
  els.sideBody.innerHTML = html;
  for (const btn of els.sideBody.querySelectorAll('[data-doc]')) {
    btn.addEventListener('click', () => { state.doc = btn.dataset.doc; renderSide(); });
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

// ---- util -------------------------------------------------------------------
function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
