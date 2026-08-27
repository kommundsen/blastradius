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
   derivedGraphFor, layoutView, groupDivs, fitGroupBoxes, GRID, kicker, edgeLabelLines */

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

  // kicker() and edgeLabelLines() come from labels.js, concatenated ahead of
  // this file (export.rs). The viewer used to carry its own copy of kicker
  // and it had already drifted from the app's.

  /** An element by id, authored or derived (L4). The viewer used to know only
   * the authored half, which is why code-level detail was missing from an
   * export entirely — spec/l4-introspection.md carried it as a debt. */
  function anyElement(id) {
    const el = snap.elements.find((e) => e.id === id);
    if (el) return el;
    const graph = derivedGraphFor(snap, id);
    const d = graph?.elements.find((e) => e.id === id);
    return d ? { ...d, derived: true, stale: graph.stale } : null;
  }

  function childCount(el) {
    if (el.derived) {
      const graph = derivedGraphFor(snap, el.id);
      const kids = (graph?.elements ?? []).filter((e) => e.parent === el.id).length;
      return kids ? `${kids} member${kids > 1 ? 's' : ''}` : null;
    }
    const children = snap.elements.filter((e) => e.parent === el.id);
    const kids = children.length;
    if (!kids) return null;
    const deployment =
      el.kind === 'environment' || el.kind === 'deployment-node'
        ? children.every((c) => c.kind === 'container-instance')
          ? 'instance'
          : 'node'
        : null;
    const noun = deployment ?? (el.kind === 'system' ? 'container' : 'component');
    return `${kids} ${noun}${kids > 1 ? 's' : ''}`;
  }

  function nodeClass(el) {
    // Same classes the app uses, so one stylesheet dresses both.
    if (el.derived) {
      let cls = 'node is-component is-derived';
      if (el.kind === 'dependency') cls += ' is-dependency';
      if (el.stale) cls += ' is-stale';
      return cls;
    }
    const map = {
      person: 'is-person',
      system: 'is-system',
      container: 'is-container',
      component: 'is-component',
      external: 'is-system',
      environment: 'is-environment',
      'deployment-node': 'is-deployment-node',
      'container-instance': 'is-container-instance',
    };
    let cls = 'node ' + (map[el.kind] ?? 'is-system');
    if (el.external) cls += ' is-external';
    return cls;
  }

  async function renderCanvas() {
    const view = computeView(snap, state.level, state.scope);
    const viewDef = findViewDef(snap, state.level, state.scope);
    const layout = await layoutView(elk, view, resolvePins(viewDef, view), {
      groups: viewDef?.show_groups ?? false,
    });
    state.layout = layout;

    const nodes = $('nodes');
    nodes.textContent = '';
    for (const box of groupDivs(layout, document)) nodes.appendChild(box);
    // view.nodes carries the derived elements at L4; they are not in
    // snap.elements, so the map has to take both.
    const elById = new Map([...snap.elements, ...view.nodes].map((e) => [e.id, e]));
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
      const lines = edgeLabelLines(e);
      for (const [i, line] of lines.entries()) {
        const text = document.createElementNS(svgNS, 'text');
        text.setAttribute('class', 'edge-label');
        text.setAttribute('x', e.labelAt.x);
        text.setAttribute('y', e.labelAt.y - (lines.length - 1 - i) * 12);
        text.setAttribute('text-anchor', 'middle');
        text.textContent = line;
        edges.appendChild(text);
      }
    }
    fitGroupBoxes(nodes, layout);
    applyCamera();
    renderCrumb();
    syncSeg();
  }

  $('canvas').addEventListener('wheel', (ev) => {
    if (!state.layout) return;
    ev.preventDefault();
    const c = $('canvas').getBoundingClientRect();
    const p = { x: ev.clientX - c.left, y: ev.clientY - c.top };
    const l = state.layout;
    const fit = Math.min(1, (c.width - 40) / l.width, (c.height - 40) / l.height);
    const s0 = fit * state.zoom;
    const delta = ev.deltaMode === 1 ? ev.deltaY * 16 : ev.deltaY;
    const zoom1 = Math.min(8, Math.max(0.2, state.zoom * Math.exp(-delta * 0.0015)));
    const s1 = fit * zoom1;
    if (s1 === s0) return;
    const t0x = (c.width - l.width * s0) / 2 + state.pan.x;
    const t0y = (c.height - l.height * s0) / 2 + state.pan.y;
    state.zoom = zoom1;
    state.pan.x = p.x - ((p.x - t0x) / s0) * s1 - (c.width - l.width * s1) / 2;
    state.pan.y = p.y - ((p.y - t0y) / s0) * s1 - (c.height - l.height * s1) / 2;
    $('camera').classList.add('no-anim');
    applyCamera();
  }, { passive: false });

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

  /** The authored ancestors of a dotted id, outermost first. */
  function authoredCrumbs(id) {
    const out = [];
    for (let i = 1; i <= depthOf(id); i++) {
      const el = snap.elements.find((e) => e.id === liftTo(id, i));
      if (el) out.push(el.name);
    }
    return out;
  }

  function renderCrumb() {
    const parts = [esc(snap.name)];
    if (state.level === 'L4' && state.scope) {
      // Derived ids may themselves contain dots, so the trail below the
      // component is walked through `parent`, never by splitting the id.
      const graph = derivedGraphFor(snap, state.scope);
      const component = graph?.component ?? state.scope;
      const names = authoredCrumbs(component);
      const byId = new Map((graph?.elements ?? []).map((e) => [e.id, e]));
      const chain = [];
      for (let cur = byId.get(state.scope); cur; cur = cur.parent ? byId.get(cur.parent) : null) {
        chain.unshift(cur.name);
      }
      for (const n of [...names, ...chain]) parts.push(`<b>${esc(n)}</b>`);
    } else if (state.scope) {
      for (const n of authoredCrumbs(state.scope)) parts.push(`<b>${esc(n)}</b>`);
    }
    parts.push({ L1: 'Context', L2: 'Containers', L3: 'Components', L4: 'Code', LD: 'Deployment' }[state.level]);
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
    const el = anyElement(id);
    if (!el) return;
    const graph = derivedGraphFor(snap, id);
    if (el.kind === 'system' && !el.external && state.level === 'L1') {
      state.level = 'L2'; state.scope = id;
    } else if (el.kind === 'container' && state.level === 'L2') {
      if (!snap.elements.some((e) => e.parent === id)) return;
      state.level = 'L3'; state.scope = id;
    } else if (el.kind === 'component' && state.level === 'L3' && graph?.elements.length) {
      // Below L3 lies the code (spec/l4-introspection.md).
      state.level = 'L4'; state.scope = id;
    } else if (el.derived && graph?.elements.some((e) => e.parent === id)) {
      state.level = 'L4'; state.scope = id;
    } else if (el.kind === 'environment' || el.kind === 'deployment-node') {
      // The deployment tree dives like the logical one (ADR-0018).
      if (!snap.elements.some((e) => e.parent === id)) return;
      state.level = 'LD'; state.scope = id;
    } else return;
    state.zoom = 1; state.pan = { x: 0, y: 0 }; state.selected = id;
    await renderCanvas();
    renderSide();
  }

  async function rise() {
    if (state.level === 'L4') {
      state.selected = state.scope;
      const graph = derivedGraphFor(snap, state.scope);
      if (!graph || state.scope === graph.component) {
        state.scope = liftTo(state.scope, depthOf(state.scope) - 1);
        state.level = 'L3';
      } else {
        const el = graph.elements.find((e) => e.id === state.scope);
        state.scope = el?.parent ?? graph.component;
      }
    } else if (state.level === 'L3') {
      state.selected = state.scope;
      state.scope = liftTo(state.scope, depthOf(state.scope) - 1);
      state.level = 'L2';
    } else if (state.level === 'L2') {
      state.selected = state.scope;
      state.scope = null;
      state.level = 'L1';
    } else if (state.level === 'LD' && state.scope) {
      state.selected = state.scope;
      state.scope = depthOf(state.scope) > 1 ? liftTo(state.scope, depthOf(state.scope) - 1) : null;
    } else return;
    state.zoom = 1; state.pan = { x: 0, y: 0 };
    await renderCanvas();
    renderSide();
  }

  function renderTree() {
    const t = treeModel(snap);
    const rows = [`<span class="tree-label">Model</span>`];
    const row = (el, depth, glyph, extra = '') =>
      `<button class="tree-row${state.selected === el.id ? ' is-active' : ''}${extra}" data-id="${esc(el.id)}"` +
      (depth ? ` style="padding-left:${14 + depth * 14}px"` : '') +
      `><span class="glyph">${glyph}</span>${esc(el.name)}</button>`;
    for (const c of t.context) rows.push(row(c, 0, '◦'));
    for (const s of t.systems) {
      rows.push(row(s.el, 0, '▸'));
      for (const c of s.containers) {
        rows.push(row(c.el, 1, ''));
        for (const k of c.components) {
          rows.push(row(k, 2, ''));
          // Introspected code (L4) nests under its component, visibly code —
          // modules first, their types one step deeper. Same shape as the app.
          const graph = (snap.derived ?? []).find((g) => g.component === k.id);
          for (const m of graph?.elements.filter((e) => !e.parent) ?? []) {
            rows.push(row(m, 3, '', ' is-derived'));
            for (const ty of graph.elements.filter((e) => e.parent === m.id)) {
              rows.push(row(ty, 4, '', ' is-derived'));
            }
          }
        }
      }
    }
    $('tree').innerHTML = rows.join('');
    for (const btn of $('tree').querySelectorAll('[data-id]')) {
      btn.addEventListener('click', () => focusElement(btn.dataset.id));
    }
  }

  async function focusElement(id) {
    const el = snap.elements.find((e) => e.id === id);
    if (!el) {
      // A derived (L4) row: fly to its code altitude.
      const graph = derivedGraphFor(snap, id);
      const d = graph?.elements.find((e) => e.id === id);
      if (!d) return;
      if (!state.layout?.nodes.some((n) => n.id === id)) {
        state.level = 'L4';
        state.scope = d.parent ?? graph.component;
        state.zoom = 1; state.pan = { x: 0, y: 0 };
        state.selected = id;
        await renderCanvas();
        renderSide();
        renderTree();
        return;
      }
      select(id);
      return;
    }
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
    const el = anyElement(state.selected);
    if (!el) {
      body.innerHTML = `<p class="side-empty text-muted">Select an element to inspect it.</p>`;
      return;
    }
    if (el.derived) return renderDerivedSide(body, el);
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

  /** Code-level detail, read-only. The app offers to open the file in an
   * editor; an export has no machine to open it on, so it states the location
   * and stops there. */
  function renderDerivedSide(body, el) {
    const graph = derivedGraphFor(snap, el.id);
    const loc = el.line ? `${el.path}:${el.line}` : el.path;
    let html = `<div class="insp"><span class="insp-kicker">${esc(kicker(el))}</span>` +
      // Code identity is case-sensitive: `CommitInfo` must not read COMMITINFO.
      `<span class="insp-title is-code">${esc(el.name)}</span>` +
      `<span class="text-muted" style="font-family:var(--font-mono);font-size:var(--text-2xs)">${esc(el.id)}</span>`;
    if (el.path) {
      html += `<div class="insp-section">Source</div>` +
        `<div class="insp-rel"><span class="tag tag-outline">${esc(graph?.language ?? 'code')}</span> ` +
        `<span style="font-family:var(--font-mono);font-size:var(--text-2xs)">${esc(loc)}</span></div>`;
    } else {
      // Dependency rollups and namespaces own no single file
      // (spec/l4-introspection.md).
      html += `<div class="insp-section">External</div>` +
        `<p class="text-muted" style="font-size:var(--text-sm)">Resolved from imports — not part of the mapped source tree.</p>`;
    }
    if (el.stale) {
      html += `<p class="text-muted" style="font-size:var(--text-sm)">⚠ These facts lagged the source tree when this was exported.</p>`;
    }
    html += `</div>`;
    body.innerHTML = html;
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
      } else if (level === 'L4') {
        // The first introspected component — the segment is live only when
        // the export carries derived facts at all.
        const graph = (snap.derived ?? []).find((g) => g.elements.length);
        if (!graph) return;
        state.scope = graph.component; state.level = 'L4';
      } else if (level === 'LD') {
        // The overview lists every environment; no scope needed (ADR-0018).
        if (!snap.elements.some((e) => e.kind === 'environment')) return;
        state.scope = null; state.level = 'LD';
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
