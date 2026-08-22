// Standalone read-only viewer — the script half of the self-contained HTML
// export (spec/export.md). Concatenated after data.js and layout.js (export
// keywords stripped) into one classic <script>, so: no imports, no exports,
// and the only inputs are the globals SNAPSHOT, INCLUDE_DOC_BODIES, and ELK /
// marked from their vendored bundles.
//
// Deliberately not the app: no git, no editing, no sync — a sealed snapshot
// and a camera (ADR-0009).

/* global SNAPSHOT, INCLUDE_DOC_BODIES, ELK, marked,
   computeView, findViewDef, resolvePins, docsFor, treeModel, rootOf, depthOf, liftTo,
   layoutView, GRID */

(() => {
  const state = {
    level: 'L1',
    scope: null,
    selected: null,
    doc: null,
    zoom: 1,
    pan: { x: 0, y: 0 },
    layout: null,
  };
  const snap = SNAPSHOT;
  const elk = new ELK();
  const $ = (id) => document.getElementById(id);

  const esc = (s) =>
    String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  function kicker(el) {
    const kind = { person: 'Person', system: 'Software system', container: 'Container', component: 'Component', external: 'External system' }[el.kind];
    const label = el.external && el.kind === 'system' ? 'External system' : kind;
    return el.tech ? `${label} · ${el.tech}` : label;
  }

  function childCount(el) {
    const kids = snap.elements.filter((e) => e.parent === el.id).length;
    if (!kids) return null;
    const noun = el.kind === 'system' ? 'container' : 'component';
    return `${kids} ${noun}${kids > 1 ? 's' : ''}`;
  }

  function nodeClass(el) {
    const map = { person: 'is-person', system: 'is-system', container: 'is-container', component: 'is-component', external: 'is-system' };
    let cls = 'node ' + (map[el.kind] ?? 'is-system');
    if (el.external) cls += ' is-external';
    return cls;
  }

  async function renderCanvas() {
    const view = computeView(snap, state.level, state.scope);
    const viewDef = findViewDef(snap, state.level, state.scope);
    const layout = await layoutView(elk, view, resolvePins(viewDef, view));
    state.layout = layout;

    const nodes = $('nodes');
    nodes.textContent = '';
    const elById = new Map(snap.elements.map((e) => [e.id, e]));
    for (const n of layout.nodes) {
      const el = elById.get(n.id);
      const div = document.createElement('div');
      div.className = nodeClass(el) + (state.selected === n.id ? ' is-active' : '');
      div.style.cssText = `left:${n.x}px;top:${n.y}px;width:${n.width}px;position:absolute`;
      div.tabIndex = 0;
      div.dataset.id = n.id;
      div.innerHTML =
        `<span class="node-kicker">${esc(kicker(el))}</span>` +
        `<span class="node-title">${esc(el.name)}</span>` +
        (childCount(el) ? `<span class="node-meta">${childCount(el)}</span>` : '');
      div.addEventListener('click', () => select(n.id));
      div.addEventListener('dblclick', () => dive(n.id));
      nodes.appendChild(div);
    }

    const edges = $('edges');
    edges.textContent = '';
    const svgNS = 'http://www.w3.org/2000/svg';
    for (const e of layout.edges) {
      const d = e.points.map((p, i) => (i ? 'L' : 'M') + p.x + ',' + p.y).join(' ');
      const path = document.createElementNS(svgNS, 'path');
      let cls = 'edge';
      if (e.direction === 'both') cls += ' is-bidirectional';
      if (e.direction === 'none') cls += ' is-undirected';
      if (!e.exact) cls += ' is-secondary';
      path.setAttribute('class', cls);
      path.setAttribute('d', d);
      edges.appendChild(path);
      const label = e.label ?? e.protocol;
      if (label) {
        const text = document.createElementNS(svgNS, 'text');
        text.setAttribute('class', 'edge-label');
        text.setAttribute('x', e.labelAt.x);
        text.setAttribute('y', e.labelAt.y);
        text.setAttribute('text-anchor', 'middle');
        text.textContent = e.protocol && e.label ? `${e.label} · ${e.protocol}` : label;
        edges.appendChild(text);
      }
    }
    applyCamera();
    renderCrumb();
    syncSeg();
  }

  function applyCamera() {
    const c = $('canvas').getBoundingClientRect();
    const l = state.layout;
    const fit = Math.min(1, (c.width - 40) / l.width, (c.height - 40) / l.height);
    const scale = fit * state.zoom;
    const tx = (c.width - l.width * scale) / 2 + state.pan.x;
    const ty = (c.height - l.height * scale) / 2 + state.pan.y;
    const cam = $('camera');
    cam.style.transform = `translate(${tx}px, ${ty}px) scale(${scale})`;
    cam.style.setProperty('--camera-scale', scale);
    $('zoom-reset').textContent = Math.round(scale * 100) + '%';
  }

  function renderCrumb() {
    const parts = [esc(snap.name)];
    if (state.scope) {
      for (let i = 1; i <= depthOf(state.scope); i++) {
        const el = snap.elements.find((e) => e.id === liftTo(state.scope, i));
        if (el) parts.push(`<b>${esc(el.name)}</b>`);
      }
    }
    parts.push({ L1: 'Context', L2: 'Containers', L3: 'Components' }[state.level]);
    $('breadcrumb').innerHTML = parts.join(' / ');
  }

  function syncSeg() {
    for (const opt of document.querySelectorAll('#level-seg .seg-opt')) {
      const input = opt.querySelector('input');
      if (!input.disabled) {
        input.checked = input.value === state.level;
        opt.classList.toggle('is-active', input.value === state.level);
      }
    }
  }

  function select(id) {
    state.selected = id;
    state.doc = null;
    for (const div of $('nodes').children) div.classList.toggle('is-active', div.dataset.id === id);
    renderTree();
    renderSide();
  }

  async function dive(id) {
    const el = snap.elements.find((e) => e.id === id);
    if (!el) return;
    if (el.kind === 'system' && !el.external && state.level === 'L1') {
      state.level = 'L2'; state.scope = id;
    } else if (el.kind === 'container' && state.level === 'L2') {
      if (!snap.elements.some((e) => e.parent === id)) return;
      state.level = 'L3'; state.scope = id;
    } else return;
    state.zoom = 1; state.pan = { x: 0, y: 0 }; state.selected = id;
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
    } else return;
    state.zoom = 1; state.pan = { x: 0, y: 0 };
    await renderCanvas();
    renderSide();
  }

  function renderTree() {
    const t = treeModel(snap);
    const rows = [`<span class="tree-label">Model</span>`];
    const row = (el, depth, glyph) =>
      `<button class="tree-row${state.selected === el.id ? ' is-active' : ''}" data-id="${esc(el.id)}"` +
      (depth ? ` style="padding-left:${14 + depth * 14}px"` : '') +
      `><span class="glyph">${glyph}</span>${esc(el.name)}</button>`;
    for (const c of t.context) rows.push(row(c, 0, '◦'));
    for (const s of t.systems) {
      rows.push(row(s.el, 0, '▸'));
      for (const c of s.containers) {
        rows.push(row(c.el, 1, ''));
        for (const k of c.components) rows.push(row(k, 2, ''));
      }
    }
    $('tree').innerHTML = rows.join('');
    for (const btn of $('tree').querySelectorAll('[data-id]')) {
      btn.addEventListener('click', () => focusElement(btn.dataset.id));
    }
  }

  async function focusElement(id) {
    const el = snap.elements.find((e) => e.id === id);
    if (!el) return;
    if (!state.layout?.nodes.some((n) => n.id === id)) {
      const d = depthOf(id);
      if (el.kind === 'person' || el.kind === 'external' || (el.kind === 'system')) {
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

  function renderSide() {
    const body = $('side-body');
    const back = $('side-back');
    if (state.doc) {
      const d = snap.docs.find((x) => x.id === state.doc);
      back.hidden = false;
      let html = `<div class="doc-meta"><span class="tag tag-outline">${esc(d.type)}</span>` +
        (d.status ? `<span class="tag tag-neutral">${esc(d.status)}</span>` : '') + `</div>`;
      if (INCLUDE_DOC_BODIES && d.body) {
        html += `<div class="doc-body">${marked.parse(d.body)}</div>`;
      } else {
        html += `<div class="doc-body"><h1>${esc(d.title)}</h1>` +
          `<p class="text-muted">Document bodies were not included in this export.</p></div>`;
      }
      body.innerHTML = html;
      return;
    }
    back.hidden = true;
    if (!state.selected) {
      body.innerHTML = `<p class="side-empty text-muted">Select an element to inspect it.</p>`;
      return;
    }
    const el = snap.elements.find((e) => e.id === state.selected);
    const rels = snap.relations.filter((r) => r.from === el.id || r.to === el.id);
    const docs = docsFor(snap, el.id);
    const nameOf = (eid) => snap.elements.find((e) => e.id === eid)?.name ?? eid;
    let html = `<div class="insp"><span class="insp-kicker">${esc(kicker(el))}</span>` +
      `<span class="insp-title">${esc(el.name)}</span>` +
      `<span class="text-muted" style="font-family:var(--font-mono);font-size:var(--text-2xs)">${esc(el.id)}</span>`;
    if (el.description) html += `<p class="insp-desc">${esc(el.description)}</p>`;
    if (rels.length) {
      html += `<div class="insp-section">Relations</div>`;
      for (const r of rels) {
        const out = r.from === el.id;
        html += `<div class="insp-rel">${r.direction === 'both' ? '↔' : out ? '→' : '←'} ` +
          `${esc(nameOf(out ? r.to : r.from))}` +
          (r.label ? ` <span class="text-muted">· ${esc(r.label)}</span>` : '') + `</div>`;
      }
    }
    html += `<div class="insp-section">Documents</div>`;
    html += docs.length
      ? docs.map((d) => `<button class="doc-link" data-doc="${esc(d.id)}">` +
          `<span class="tag tag-outline">${esc(d.type)}</span> ${esc(d.title)}</button>`).join('')
      : `<span class="text-muted" style="font-size:var(--text-sm)">None linked.</span>`;
    html += `</div>`;
    body.innerHTML = html;
    for (const btn of body.querySelectorAll('[data-doc]')) {
      btn.addEventListener('click', () => { state.doc = btn.dataset.doc; renderSide(); });
    }
  }

  function wire() {
    $('level-seg').addEventListener('change', async (ev) => {
      if (ev.target.name !== 'lvl') return;
      const level = ev.target.value;
      if (level === 'L1') { state.level = 'L1'; state.scope = null; }
      else if (level === 'L2') {
        state.scope = state.scope ? rootOf(state.scope) : snap.elements.find((e) => e.kind === 'system')?.id;
        if (!state.scope) return;
        state.level = 'L2';
      } else if (level === 'L3') {
        const c = snap.elements.find((e) => e.kind === 'container');
        if (!c) return;
        state.scope = c.id; state.level = 'L3';
      }
      state.zoom = 1; state.pan = { x: 0, y: 0 };
      await renderCanvas();
    });
    $('zoom-in').addEventListener('click', () => { state.zoom *= 1.2; applyCamera(); });
    $('zoom-out').addEventListener('click', () => { state.zoom /= 1.2; applyCamera(); });
    $('zoom-reset').addEventListener('click', () => { state.zoom = 1; state.pan = { x: 0, y: 0 }; applyCamera(); });
    $('side-back').addEventListener('click', () => { state.doc = null; renderSide(); });

    let theme = 'auto';
    $('theme-btn').addEventListener('click', () => {
      theme = theme === 'auto' ? 'light' : theme === 'light' ? 'dark' : 'auto';
      if (theme === 'auto') document.documentElement.removeAttribute('data-theme');
      else document.documentElement.setAttribute('data-theme', theme);
      $('theme-btn').textContent = 'Theme: ' + theme;
    });

    const canvas = $('canvas');
    canvas.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape') { ev.preventDefault(); rise(); }
      if (ev.key === '+') { state.zoom *= 1.2; applyCamera(); }
      if (ev.key === '-') { state.zoom /= 1.2; applyCamera(); }
    });
    let drag = null;
    canvas.addEventListener('pointerdown', (ev) => {
      if (ev.target.closest('.node') || ev.target.closest('.canvas-overlay')) return;
      drag = { x: ev.clientX - state.pan.x, y: ev.clientY - state.pan.y };
      $('camera').classList.add('no-anim');
    });
    window.addEventListener('pointermove', (ev) => {
      if (!drag) return;
      state.pan = { x: ev.clientX - drag.x, y: ev.clientY - drag.y };
      applyCamera();
    });
    window.addEventListener('pointerup', () => { drag = null; $('camera').classList.remove('no-anim'); });
    window.addEventListener('resize', () => state.layout && applyCamera());
  }

  window.addEventListener('DOMContentLoaded', async () => {
    document.title = snap.name + ' — Blastradius';
    wire();
    renderTree();
    await renderCanvas();
    renderSide();
  });
})();
