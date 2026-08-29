// Blastradius Phase 1 frontend: read-only rendering of one workspace.
// The Core owns truth; this file owns pixels. No write path exists here.

import { computeView, findViewDef, docsFor, treeModel, rootOf, depthOf, liftTo, resolvePins, resolveDescriptions, derivedGraphFor, environments } from './data.js';
import { layoutView, GRID, groupDivs, fitGroupBoxes, nodeSize } from './layout.js';
import { viewSvg, kicker, metaLine } from './svg.js';
import { edgeLabelLines, multiplicity } from './labels.js';
import { HELP_PAGES, helpBody, helpLinkTarget } from './help.js';
import { searchModel } from './search.js';
import { boxMenuItems, canvasMenuItems, CHILD_KINDS } from './menu.js';

// ---- shell bridge -----------------------------------------------------------
// Real IPC under Tauri; mock (fetch of a committed snapshot) in a plain
// browser, so the frontend is developable and testable headless.
const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke
  ? (cmd, args) => tauri.core.invoke(cmd, args)
  : async (cmd, args) => {
      if (cmd === 'workspace_snapshot') {
        if (location.search.includes('noworkspace') && !mockState.opened) {
          throw new Error('no workspace open');
        }
        const res = await fetch('mock/snapshot.json');
        return res.json();
      }
      if (cmd === 'workspace_root') return '(mock)';
      // git commands answer from an optional fixture; absent = no repo.
      // `?nogit` simulates a plain folder (and lets edit tests run unconflicted).
      const git = location.search.includes('nogit')
        ? null
        : await fetch('mock/git.json').then((r) => (r.ok ? r.json() : null)).catch(() => null);
      if (cmd === 'git_status') {
        const st = git?.status ?? null;
        return st && mockState.resolved ? { ...st, conflicted: [] } : st;
      }
      if (cmd === 'git_diff') return git?.diff ?? null;
      if (cmd === 'git_history') return git?.history ?? [];
      if (cmd === 'git_conflicts') return mockState.resolved ? null : (git?.conflicts ?? null);
      if (cmd === 'snapshot_at') return git?.snapshots?.[args?.refspec] ?? null;
      return mockSync(cmd, args);
    };
const listen = tauri?.event?.listen ? (ev, cb) => tauri.event.listen(ev, cb) : () => {};

// Mock sync layer: applies operations to the fetched snapshot in memory so the
// full editing UX is exercisable (and Playwright-testable) without Tauri.
const mockState = { undo: [], redo: [] }; // [{label, snap}] — snapshot clones
function mockSync(cmd, args) {
  const snap = state.snapshot;
  const clone = () => JSON.parse(JSON.stringify(state.snapshot));
  if (cmd === 'sync_status') {
    return { stale: [], staleModel: [], staleViewIds: [],
      canUndo: mockState.undo.length > 0, canRedo: mockState.redo.length > 0,
      undoLabel: mockState.undo.at(-1)?.label ?? null, redoLabel: mockState.redo.at(-1)?.label ?? null,
      files: ['model/context.yaml', 'model/blastradius.yaml', 'views/containers.yaml'] };
  }
  if (cmd === 'file_text') {
    return '# mock harness: source editing needs the real app\n# (files are served read-only here)\n';
  }
  if (cmd === 'buffer_update') return true;
  if (cmd === 'undo_op') {
    const t = mockState.undo.pop();
    if (!t) return null;
    mockState.redo.push({ label: t.label, snap: clone() });
    state.snapshot = t.snap;
    return t.label;
  }
  if (cmd === 'redo_op') {
    const t = mockState.redo.pop();
    if (!t) return null;
    mockState.undo.push({ label: t.label, snap: clone() });
    state.snapshot = t.snap;
    return t.label;
  }
  if (cmd === 'open_in_editor') return null;
  // Extraction runs real compilers over a real repository — there is nothing
  // for the mock to do but say so.
  if (cmd === 'introspect_component') {
    return { elements: 0, edges: 0, warnings: ['mock harness: introspection needs the real app'] };
  }
  if (cmd === 'open_source') return null;
  if (cmd === 'resolve_conflicts') {
    // the fixture is re-fetched per invoke; persist resolution in mockState
    mockState.resolved = true;
    return ['model/blastradius.yaml'];
  }
  if (cmd === 'pick_folder') return '(mock)';
  if (cmd === 'workspace_open') {
    // `?emptyfolder` plays the case that matters most on a first run: the
    // user picked their own repository and there is no workspace in it yet.
    if (location.search.includes('emptyfolder') && !mockState.initialised) {
      // `?hasdoc` plays a project that already keeps documentation in doc/,
      // where the recommendation follows what is there rather than making a
      // near-duplicate.
      return {
        empty: '/home/dev/my-repo',
        git: true,
        suggest: location.search.includes('hasdoc') ? 'doc' : 'docs',
      };
    }
    mockState.opened = true;
    return { opened: '(mock)' };
  }
  if (cmd === 'workspace_init') {
    mockState.opened = true;
    mockState.initialised = true;
    // Mirrors the real command: one log line per thing actually written, and
    // the files that were already there are kept rather than being an error.
    const mcp = args?.agents?.mcp ?? [];
    const skills = args?.agents?.skills ?? [];
    mockState.location = args?.location ?? '.';
    return {
      opened: '(mock)',
      scaffolded: true,
      location: mockState.location,
      created: ['blastradius.yaml', 'model/context.yaml', 'views/containers.yaml'],
      kept: location.search.includes('hasreadme') ? ['README.md'] : [],
      log: [
        ...mcp.map((a) => `wrote mcp config (${a})`),
        ...skills.map((a) => `wrote skill (${a})`),
      ],
      // Mirrors core: the hand-off depends on what was selected, and names
      // the workflows when any were written (core::onboard).
      prompt: skills.length
        ? "Model this repository's architecture into the Blastradius workspace at `docs` by running the blastradius model workflow — /blastradius:model in Claude Code. Interview me first."
        : 'Read this repository and model its architecture in the Blastradius workspace at `docs`. Use the blastradius MCP tools.',
      workflows: skills.length
        ? [
            'model — build or extend the model, interviewing you first: /blastradius:model in Claude Code',
            'sync — bring the model back in step with the code since a commit: /blastradius:sync in Claude Code',
            'review — judge the model against the repository, changing nothing: /blastradius:review in Claude Code',
          ]
        : [],
    };
  }
  if (cmd === 'workspace_demo') {
    // the mock has exactly one workspace: "opening" simply leaves the
    // welcome screen and serves the committed snapshot
    mockState.opened = true;
    return '(mock)';
  }
  if (cmd === 'export_html') return '(mock: export needs the real app)';
  if (cmd === 'save_export') {
    // in a plain browser, hand the file to the browser's own download path
    const blob = args.base64
      ? new Blob([Uint8Array.from(atob(args.data), (c) => c.charCodeAt(0))])
      : new Blob([args.data]);
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = args.name;
    a.click();
    URL.revokeObjectURL(a.href);
    return '(browser download)';
  }
  // A batch is one transaction with one undo entry — the same contract as
  // core::sync::apply_batch, which is what the canvas relies on when the first
  // drag in a view settles every other node too.
  if (cmd === 'apply_operations') {
    const before = clone();
    let label = '';
    for (const op of args.ops) label = mockSync('apply_operation', { op }).label;
    // Collapse the per-op history the loop just built into one entry.
    mockState.undo.length = Math.max(0, mockState.undo.length - args.ops.length);
    mockState.undo.push({ label: `${args.ops.length} operations`, snap: before });
    mockState.redo.length = 0;
    return { label };
  }
  if (cmd !== 'apply_operation') throw new Error('unknown command ' + cmd);

  const before = clone();
  const op = args.op;
  const label = op.op + ' ' + (op.id ?? (op.from ? op.from + ' -> ' + op.to : ''));
  if (op.op === 'rename') {
    const el = snap.elements.find((e) => e.id === op.id);
    if (!el) throw new Error('unknown element');
    el.name = op.name;
  } else if (op.op === 'create') {
    const id = op.parent ? op.parent + '.' + op.id : op.id;
    if (snap.elements.some((e) => e.id === id)) throw new Error('id exists');
    snap.elements.push({ id, kind: op.kind, parent: op.parent ?? undefined, name: op.name });
  } else if (op.op === 'delete') {
    snap.elements = snap.elements.filter((e) => e.id !== op.id && !e.id.startsWith(op.id + '.'));
    snap.relations = snap.relations.filter((r) => r.from !== op.id && r.to !== op.id);
  } else if (op.op === 'add-relation') {
    snap.relations.push({ from: op.from, to: op.to, label: op.label ?? null,
      protocol: op.protocol ?? null, direction: 'forward' });
  } else if (op.op === 'delete-relation') {
    snap.relations = snap.relations.filter((r) =>
      !(r.from === op.from && r.to === op.to && (op.label == null || r.label === op.label)));
  } else if (op.op === 'set-relation-field') {
    const r = snap.relations.find((r) => r.from === op.from && r.to === op.to);
    if (r) r[op.field] = op.value;
  } else if (op.op === 'set-field') {
    const el = snap.elements.find((e) => e.id === op.id);
    if (!el) throw new Error('unknown element');
    // Mirrors compute_set_field: false is not a value of `external`, 1 is not
    // a value of `replicas`, and an empty string is not a value of anything.
    const clears = !op.value
      || (op.field === 'external' && op.value !== 'true')
      || (op.field === 'replicas' && Number(op.value) === 1);
    if (clears) delete el[op.field];
    else el[op.field] = op.field === 'replicas' ? Number(op.value) : op.value;
  } else if (op.op === 'pin') {
    const v = snap.views.find((v) => op.view ? v.id === op.view :
      (v.level === op.level && (op.level === 'L1' || v.scope === op.scope)));
    if (v) {
      const key = op.scope && op.id.startsWith(op.scope + '.') ? op.id.slice(op.scope.length + 1) : op.id;
      v.layout[key] = [op.x, op.y];
    }
  } else if (op.op === 'set-view-flag') {
    const key = { 'show-groups': 'show_groups', 'include-context': 'include_context', nested: 'nested' }[op.flag];
    let v = snap.views.find((v) => op.view ? v.id === op.view :
      (v.level === op.level && (op.level === 'L1' || v.scope === op.scope)));
    if (!v) {
      // The engine authors the file here; the mock has no filesystem, so it
      // authors the view the file would have declared.
      v = { id: (op.scope ?? op.level).split('.').pop() + '-' + op.level.toLowerCase(),
        scope: op.scope ?? '', level: op.level, layout: {}, descriptions: [],
        include_context: true, show_groups: false, nested: false };
      snap.views.push(v);
    }
    v[key] = op.value;
  } else if (op.op === 'set-source') {
    const el = snap.elements.find((e) => e.id === op.id);
    if (!el) throw new Error('unknown element');
    if (op.source) el.source = { include: [], exclude: [], ...op.source };
    else delete el.source;
  } else if (op.op === 'unpin') {
    const v = snap.views.find((v) => op.view ? v.id === op.view :
      (v.level === op.level && (op.level === 'L1' || v.scope === op.scope)));
    if (v && op.id == null) {
      v.layout = {};
    } else if (v) {
      const key = op.scope && op.id.startsWith(op.scope + '.') ? op.id.slice(op.scope.length + 1) : op.id;
      delete v.layout[key];
      delete v.layout[op.id];
    }
  } else if (op.op === 'show-description') {
    // The real engine writes a view file when there is none; the mock has no
    // filesystem, so it can only edit a view the fixture already declares.
    const v = snap.views.find((v) => op.view ? v.id === op.view :
      (v.level === op.level && (op.level === 'L1' || v.scope === op.scope)));
    if (v) {
      const key = op.scope && op.id.startsWith(op.scope + '.') ? op.id.slice(op.scope.length + 1) : op.id;
      const list = new Set(v.descriptions ?? []);
      if (op.show) list.add(key); else list.delete(key);
      v.descriptions = [...list].sort();
    }
  }
  mockState.undo.push({ label, snap: before });
  mockState.redo.length = 0;
  return { label };
}

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
  help: null,         // bundled help: null = closed, '' = index, else a page id
  // ── git (phase 2) ──
  git: null,          // git_status payload | null (no repo)
  conflicts: null,    // git_conflicts payload | null
  conflictChoices: {}, // pending per-element resolution (id -> 'ours'|'theirs')
  diff: null,         // git_diff payload when diff mode is on
  diffOn: false,
  diffBase: null,     // explicit base ref, else server default (merge-base)
  showLayoutDiff: false,
  history: null,      // commit list when the History panel is open
  travel: null,       // { refspec, snapshot } during time-travel
  // ── editing (phase 3) ──
  sync: null,         // sync_status payload
  sideMode: 'inspect',
  srcFile: null,      // open file in the Source panel
  srcSuppress: false, // don't clobber the textarea while the user types
  connectFrom: null,  // relation-draw mode: source element id
  selectedRel: null,  // selected relation {from,to,label}
  dialog: null,
  palette: false,   // the find-anything overlay is open
};

const $ = (id) => document.getElementById(id);
const els = {
  breadcrumb: $('breadcrumb'), tree: $('tree'), camera: $('camera'),
  nodes: $('nodes'), edges: $('edges'), edgeLayer: $('edge-layer'),
  canvas: $('canvas'), sideTitle: $('side-title'), sideBody: $('side-body'),
  sideBack: $('side-back'), levelSeg: $('level-seg'), diagChips: $('diag-chips'),
  helpBtn: $('help-btn'),
  hint: $('hint'), themeBtn: $('theme-btn'),
  gitChips: $('git-chips'), diffBtn: $('diff-btn'), historyBtn: $('history-btn'),
  undoBtn: $('undo-btn'), redoBtn: $('redo-btn'), addBtn: $('add-btn'),
  sideMode: $('side-mode'), srcStatus: $('src-status'), findBtn: $('find-btn'),
};

let elk = null;

// ---- boot -------------------------------------------------------------------
window.addEventListener('DOMContentLoaded', async () => {
  elk = new ELK();
  await reload();
  listen('workspace-changed', () => reload());
  wireChrome();
  wireEditing();
  wireResizers();
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
    renderWelcome();
    return;
  }
  document.querySelector('.welcome')?.remove();
  await refreshGit();
  // default scope: first system
  if (!state.scopeInit) {
    const sys = state.snapshot.elements.find((e) => e.kind === 'system');
    state.defaultSystem = sys?.id ?? null;
    state.scopeInit = true;
  }
  renderDiagnostics();
  renderGitChrome();
  await refreshSync();
  renderTree();
  await renderCanvas({ animate: false });
  renderSide();
}

async function refreshSync() {
  try {
    state.sync = await invoke('sync_status');
  } catch (e) {
    state.sync = null;
  }
  renderEditChrome();
}

/** Editing is allowed when we are on the live working tree with no staleness
 * and no merge conflict (ADR-0008 + spec/git-and-diff.md). */
function canEdit() {
  // Granular staleness (Phase 5): only *model* staleness freezes editing; a
  // stale views file merely disables pinning into that view.
  const staleModel = state.sync?.staleModel ?? state.sync?.stale;
  return !state.travel
    && !state.conflicts
    && (staleModel ? staleModel.length === 0 : false);
}

/** Authored or derived element by id — derived (L4) elements live in the
 * snapshot's `derived` graphs, not in `elements` (spec/l4-introspection.md). */
function anyElement(id) {
  const snap = effectiveSnapshot();
  const el = snap.elements.find((e) => e.id === id);
  if (el) return el;
  const graph = derivedGraphFor(snap, id);
  const d = graph?.elements.find((e) => e.id === id);
  return d ? { ...d, derived: true, stale: graph.stale } : null;
}

/** Pinning is per-view: disabled while the current view's file is stale.
 * L4 is never pinnable — derived layouts are pure auto-layout. */
function canPin() {
  if (state.level === 'L4') return false;
  if (!canEdit()) return false;
  const viewDef = findViewDef(effectiveSnapshot(), state.level, state.scope);
  return !viewDef || !(state.sync?.staleViewIds ?? []).includes(viewDef.id);
}

function renderEditChrome() {
  const s = state.sync;
  els.undoBtn.disabled = !s?.canUndo;
  els.redoBtn.disabled = !s?.canRedo;
  els.undoBtn.title = s?.undoLabel ? `Undo: ${s.undoLabel}` : 'Undo (Ctrl+Z)';
  els.redoBtn.title = s?.redoLabel ? `Redo: ${s.redoLabel}` : 'Redo (Ctrl+Y)';
  els.addBtn.hidden = !canEdit();
  document.getElementById('app').classList.toggle('can-edit', canEdit());
  document.querySelector('.stale-banner')?.remove();
  const staleModel = s?.staleModel ?? s?.stale ?? [];
  const staleViews = (s?.stale ?? []).filter((f) => !staleModel.includes(f));
  if (staleModel.length || staleViews.length) {
    const b = document.createElement('div');
    b.className = 'stale-banner';
    b.innerHTML = staleModel.length
      ? `<span>⚠ ${esc(staleModel.join(', '))} does not parse — canvas is read-only until fixed</span>`
      : `<span>⚠ ${esc(staleViews.join(', '))} does not parse — pinning is disabled for that view</span>`;
    els.canvas.appendChild(b);
  }
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
  els.edgeLayer.style.pointerEvents = 'none';
  const snap = effectiveSnapshot();
  const viewDef = findViewDef(snap, state.level, state.scope);
  const view = computeView(
    snap, state.level, state.scope, viewDef?.include_context ?? true, viewDef?.nested ?? false
  );
  // Hoisted: measuring the nodes before layout needs it (see measureNodes).
  const elById = new Map([...snap.elements, ...view.nodes].map((e) => [e.id, e]));
  // view.nodes carry the element objects themselves — at L4 those are
  // derived elements that exist nowhere in snap.elements.
  const childListFor = (el) =>
    el.derived ? (derivedGraphFor(snap, el.id)?.elements ?? []) : state.snapshot.elements;
  // Which boxes draw their description (spec §4). Measuring needs it before
  // layout, and layout stamps `describe` on the nodes so rendering below
  // cannot disagree with the height that was reserved.
  const describe = resolveDescriptions(viewDef, view);
  const layout = await layoutView(elk, view, resolvePins(viewDef, view), {
    groups: viewDef?.show_groups ?? false,
    nested: viewDef?.nested ?? false,
    descriptions: describe,
    sizes: measureNodes(view, elById, childListFor, describe),
  });
  state.layout = layout;

  els.camera.classList.toggle('no-anim', !animate);

  // nodes
  els.nodes.textContent = '';
  // Boundaries first: they sit behind their members (--z-group) and must not
  // intercept clicks meant for the nodes inside them.
  for (const box of groupDivs(layout, document)) els.nodes.appendChild(box);
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
    // A container (ADR-0018 nesting) is sized by the layout in both axes and
    // painted behind what it holds; every other node is content-sized.
    div.style.cssText = n.contains
      ? `left:${n.x}px;top:${n.y}px;width:${n.width}px;height:${n.height}px;position:absolute`
      : `left:${n.x}px;top:${n.y}px;width:${n.width}px;position:absolute`;
    if (n.contains) div.classList.add('is-nested');
    div.tabIndex = 0;
    div.setAttribute('role', 'button');
    div.dataset.id = n.id;
    if (state.selected === n.id) div.classList.add('is-active');
    const kids = metaLine(el, childListFor(el));
    div.innerHTML = nodeInner(el, kids, n.describe);
    if (badge) {
      const b = document.createElement('span');
      b.className = 'node-badge';
      b.title = badge[1];
      b.innerHTML = `<span aria-hidden="true">${badge[0]}</span><span class="sr-only">${badge[1]}</span>`;
      div.appendChild(b);
    }
    div.addEventListener('click', (ev) => {
      if (state.connectFrom && state.connectFrom !== n.id) {
        ev.stopPropagation();
        finishConnect(n.id);
        return;
      }
      select(n.id);
    });
    div.addEventListener('dblclick', () => dive(n.id));
    div.addEventListener('keydown', (ev) => {
      if (ev.key === 'Enter') { ev.preventDefault(); dive(n.id); }
      if (ev.key === ' ') { ev.preventDefault(); select(n.id); }
    });
    div.addEventListener('contextmenu', (ev) => openNodeMenu(ev, n.id));
    div.addEventListener('pointerdown', (ev) => beginNodeDrag(ev, n, div));
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
    path.setAttribute('data-from', e.from);
    path.setAttribute('data-to', e.to);
    if (e.exact) {
      const hit = document.createElementNS(svgNS, 'path');
      hit.setAttribute('class', 'edge-hit');
      hit.setAttribute('d', d);
      hit.addEventListener('click', (ev) => {
        ev.stopPropagation();
        selectRelation(e);
      });
      els.edges.appendChild(hit);
    }
    els.edges.appendChild(path);
    // C4 puts technology in brackets on its own line under the label.
    const lines = edgeLabelLines(e);
    for (const [i, line] of lines.entries()) {
      const text = document.createElementNS(svgNS, 'text');
      text.setAttribute('class', 'edge-label');
      text.setAttribute('x', e.labelAt.x);
      text.setAttribute('y', e.labelAt.y - (lines.length - 1 - i) * 12);
      text.setAttribute('text-anchor', 'middle');
      text.textContent = line;
      els.edges.appendChild(text);
    }
  }

  fitGroupBoxes(els.nodes, layout);
  applyCamera();
  renderBreadcrumb();
  syncLevelSeg();
  // The view panel is about whatever is on screen, and what is on screen has
  // just changed — by a dive, a level button, or an edit. The inspector needs
  // no such thing: its subject only changes when the selection does.
  if (state.sideMode === 'view') renderViewPanel();
}

/** The size each node will actually render at.
 *
 * `nodeSize` in layout.js is a per-kind estimate, and a `.node` is
 * content-sized: a name that wraps to three lines is taller than the estimate,
 * so layout reserved too little and the overflow ran into whatever sat below.
 * With few nodes the inter-layer gap hid it; with many it did not.
 *
 * Measured with the real markup and the real stylesheet, offscreen, in one
 * reflow — the same trick `fitGroupBoxes` uses on group boxes after the fact,
 * done before layout instead so the geometry is right the first time.
 */
function measureNodes(view, elById, childListFor, describe = new Set()) {
  const probe = document.createElement('div');
  probe.style.cssText =
    'position:absolute;left:-10000px;top:0;visibility:hidden;pointer-events:none';
  els.nodes.appendChild(probe);

  const divs = view.nodes.map((n) => {
    const el = elById.get(n.id) ?? n;
    const d = document.createElement('div');
    d.className = nodeClass(el);
    // Width comes from the estimate and is authoritative — it is what the
    // renderer sets. Only height is left to the content.
    d.style.cssText = `width:${nodeSize(el).width}px;position:absolute`;
    d.innerHTML = nodeInner(el, metaLine(el, childListFor(el)), describe.has(n.id));
    probe.appendChild(d);
    return [n.id, d, nodeSize(el).width];
  });

  // Height for every node, plus — for the ones that turn out to be containers
  // — the breakdown layout needs to reserve room for their own chrome: the
  // kicker+name block above what they hold, and the meta line and description
  // below it. A container is sized by ELK from its children and its padding,
  // so anything it draws itself has to be padding or it gets sat on.
  const sizes = new Map(
    divs.map(([id, d, width]) => {
      const title = d.querySelector('.node-title');
      const meta = d.querySelector('.node-meta');
      const desc = d.querySelector('.node-desc');
      return [id, {
        width,
        height: Math.ceil(d.offsetHeight),
        header: Math.ceil(title.offsetTop + title.offsetHeight + NODE_PAD),
        meta: meta ? Math.ceil(meta.offsetHeight) : 0,
        // The rule's own margin sits outside `offsetHeight`.
        desc: desc ? Math.ceil(desc.offsetHeight) + NODE_DESC_MARGIN : 0,
      }];
    })
  );
  probe.remove();
  return sizes;
}

/** `.node` padding and `.node-desc`'s margin, from components.css — the two
 *  numbers a measured height cannot see from inside the element. */
const NODE_PAD = 10;
const NODE_DESC_MARGIN = 6;

/** A box's contents: kicker, name, meta line, and — where the view asks for
 *  it — the description at the bottom. Written once because `measureNodes`
 *  measures this markup and `renderCanvas` draws it, and a difference between
 *  the two is a box laid out at the wrong height. */
function nodeInner(el, meta, describe) {
  return (
    `<span class="node-kicker">${esc(kicker(el))}</span>` +
    `<span class="node-title">${esc(el.name)}</span>` +
    (meta ? `<span class="node-meta">${meta}</span>` : '') +
    (describe && el.description ? `<span class="node-desc">${esc(el.description)}</span>` : '')
  );
}

function nodeClass(el) {
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

/** Zoom about a point in canvas coordinates: the model position under the
 * cursor stays put while the scale changes. */
function zoomAt(p, factor) {
  const l = state.layout;
  const c = els.canvas.getBoundingClientRect();
  const fit = Math.min(1, (c.width - 40) / l.width, (c.height - 40) / l.height);
  const s0 = fit * state.zoom;
  const zoom1 = Math.min(8, Math.max(0.2, state.zoom * factor));
  const s1 = fit * zoom1;
  if (s1 === s0) return;
  const t0x = (c.width - l.width * s0) / 2 + state.pan.x;
  const t0y = (c.height - l.height * s0) / 2 + state.pan.y;
  state.zoom = zoom1;
  state.pan.x = p.x - ((p.x - t0x) / s0) * s1 - (c.width - l.width * s1) / 2;
  state.pan.y = p.y - ((p.y - t0y) / s0) * s1 - (c.height - l.height * s1) / 2;
  els.camera.classList.add('no-anim'); // wheel must track 1:1, no glide
  applyCamera();
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
  paintGrid(tx, ty, scale);
  $('zoom-reset').textContent = Math.round(scale * 100) + '%';
}

/**
 * Align the endless dot sheet with the camera.
 *
 * The grid is painted on `.canvas`, which fills the viewport and never moves,
 * so it covers everything at every pan and zoom; these two properties are what
 * keep it part of the *drawing* rather than of the screen. The dot at model
 * (0,0) lands under the model's origin, and the pitch tracks the zoom.
 *
 * Below half scale the dots would collapse into a wash, so the pitch steps up
 * to four grid units — the behaviour the design system has always described.
 */
function paintGrid(tx, ty, scale) {
  const pitch = GRID * scale * (scale < 0.5 ? 4 : 1);
  els.canvas.style.backgroundSize = `${pitch}px ${pitch}px`;
  // Modulo keeps the offset small, which avoids losing sub-pixel accuracy in
  // the background-position once a diagram has been panned a long way.
  const at = (v) => (((v % pitch) + pitch) % pitch).toFixed(2);
  els.canvas.style.backgroundPosition = `${at(tx)}px ${at(ty)}px`;
}

// ---- navigation -------------------------------------------------------------
function select(id) {
  state.selected = id;
  state.doc = null;
  state.selectedRel = null;
  // Picking something in the model is an unambiguous request to inspect it.
  // Help used to survive this, and renderSide tests help first, so the panel
  // kept showing help while the canvas selection moved underneath — reported
  // as "no way to switch back" by the first outside user.
  state.help = null;
  for (const div of els.nodes.children) {
    div.classList.toggle('is-active', div.dataset.id === id);
  }
  renderTree();
  renderSide();
}

async function dive(id) {
  const el = anyElement(id);
  if (!el) return;
  const graph = derivedGraphFor(effectiveSnapshot(), id);
  if (el.kind === 'system' && !el.external && state.level === 'L1') {
    await glideInto(id);
    state.level = 'L2'; state.scope = id;
  } else if (el.kind === 'container' && state.level === 'L2') {
    if (!state.snapshot.elements.some((e) => e.parent === id)) return; // nothing inside
    await glideInto(id);
    state.level = 'L3'; state.scope = id;
  } else if (el.kind === 'component' && state.level === 'L3' && graph?.elements.length) {
    // Below L3 lies the code (spec/l4-introspection.md): components that opted
    // into introspection open their derived module graph.
    await glideInto(id);
    state.level = 'L4'; state.scope = id;
  } else if (el.derived && graph.elements.some((e) => e.parent === id)) {
    // Deeper into the code: module → its types/submodules. Works from the
    // canvas at L4 and from a tree row at any altitude.
    await glideInto(id);
    state.level = 'L4'; state.scope = id;
  } else if (el.kind === 'environment' || el.kind === 'deployment-node') {
    // The deployment tree dives like the logical one (ADR-0018): an
    // environment opens its nodes, a node opens whatever runs on it.
    // Container instances are leaves.
    if (!state.snapshot.elements.some((e) => e.parent === id)) return;
    await glideInto(id);
    state.level = 'LD'; state.scope = id;
  } else {
    return;
  }
  state.zoom = 1; state.pan = { x: 0, y: 0 };
  state.selected = id;
  await renderCanvas({ animate: false });
  // The node that had focus is gone after the re-render — hand focus to the
  // canvas immediately (not after the settle animation) so the next keystroke
  // in the dive/rise flow is never lost.
  els.canvas.focus({ preventScroll: true });
  await glideSettle('in');
  renderSide();
}

async function rise() {
  if (state.level === 'L4') {
    await glideOut();
    state.selected = state.scope;
    const graph = derivedGraphFor(effectiveSnapshot(), state.scope);
    if (!graph || state.scope === graph.component) {
      // Back up from the component's code to its container.
      state.scope = liftTo(state.scope, depthOf(state.scope) - 1);
      state.level = 'L3';
    } else {
      const el = graph.elements.find((e) => e.id === state.scope);
      state.scope = el?.parent ?? graph.component;
    }
  } else if (state.level === 'L3') {
    await glideOut();
    state.selected = state.scope;
    state.scope = liftTo(state.scope, depthOf(state.scope) - 1);
    state.level = 'L2';
  } else if (state.level === 'L2') {
    await glideOut();
    state.selected = state.scope;
    state.scope = null;
    state.level = 'L1';
  } else if (state.level === 'LD' && state.scope) {
    // Up the deployment tree; from an environment, out to the overview.
    await glideOut();
    state.selected = state.scope;
    state.scope = depthOf(state.scope) > 1 ? liftTo(state.scope, depthOf(state.scope) - 1) : null;
  } else {
    return;
  }
  state.zoom = 1; state.pan = { x: 0, y: 0 };
  await renderCanvas({ animate: false });
  els.canvas.focus({ preventScroll: true });
  await glideSettle('out');
  renderSide();
}

// ---- semantic dive choreography (phase 5) -----------------------------------
// The motion spec's continuous-zoom intent: diving, the camera flies *into*
// the chosen node; the deeper scene continues the forward motion by growing
// to fit. Rising is the exact inverse. --duration-camera/--ease-camera govern
// both halves, so prefers-reduced-motion collapses the glide to a cut.

function cameraMotion() {
  const css = getComputedStyle(document.documentElement);
  const raw = css.getPropertyValue('--duration-camera').trim();
  const total = raw.endsWith('ms') ? parseFloat(raw) : (parseFloat(raw) || 0) * 1000;
  return { total, ease: css.getPropertyValue('--ease-camera').trim() || 'ease' };
}

/** The applyCamera transform, with an extra scale multiplier for glides. */
function cameraTransform(m = 1) {
  const c = els.canvas.getBoundingClientRect();
  const l = state.layout;
  const fit = Math.min(1, (c.width - 40) / l.width, (c.height - 40) / l.height);
  const scale = fit * state.zoom * m;
  const tx = (c.width - l.width * scale) / 2 + state.pan.x;
  const ty = (c.height - l.height * scale) / 2 + state.pan.y;
  // The grid follows through its own CSS transition rather than the camera's
  // keyframes, which is close enough for a flight and exact once it lands.
  paintGrid(tx, ty, scale);
  return `translate(${tx}px, ${ty}px) scale(${scale})`;
}

/** First half of a dive: magnify the current scene into the target node. */
async function glideInto(id) {
  const { total, ease } = cameraMotion();
  const n = state.layout?.nodes.find((x) => x.id === id);
  if (!total || !n) return;
  const c = els.canvas.getBoundingClientRect();
  const fit = Math.min(1, (c.width - 40) / state.layout.width, (c.height - 40) / state.layout.height);
  // magnify until the node roughly fills the viewport (clamped so tiny nodes
  // do not blast the scene into pixels)
  const k = Math.min(4, Math.max(2, Math.min(c.width / (n.width * fit), c.height / (n.height * fit)) * 0.8));
  const scale = fit * state.zoom * k;
  const tx = c.width / 2 - (n.x + n.width / 2) * scale;
  const ty = c.height / 2 - (n.y + n.height / 2) * scale;
  els.camera.classList.add('no-anim');
  await els.camera.animate(
    [
      { transform: cameraTransform(), opacity: 1 },
      { transform: `translate(${tx}px, ${ty}px) scale(${scale})`, opacity: 0 },
    ],
    { duration: total / 2, easing: 'cubic-bezier(.4, 0, 1, 1)', fill: 'forwards' },
  ).finished.catch(() => {});
}

/** First half of a rise: pull back out of the current scene. */
async function glideOut() {
  const { total } = cameraMotion();
  if (!total || !state.layout) return;
  els.camera.classList.add('no-anim');
  await els.camera.animate(
    [
      { transform: cameraTransform(), opacity: 1 },
      { transform: cameraTransform(0.62), opacity: 0 },
    ],
    { duration: total / 2, easing: 'cubic-bezier(.4, 0, 1, 1)', fill: 'forwards' },
  ).finished.catch(() => {});
}

/** Second half, on the new scene: 'in' continues forward motion (grow to
 * fit); 'out' continues the pull-back (shrink to fit). */
async function glideSettle(direction) {
  const { total, ease } = cameraMotion();
  els.camera.getAnimations().forEach((a) => a.cancel());
  if (!total || !state.layout) {
    els.camera.classList.remove('no-anim');
    return;
  }
  const from = direction === 'in' ? 0.62 : 1.45;
  await els.camera.animate(
    [
      { transform: cameraTransform(from), opacity: 0 },
      { transform: cameraTransform(), opacity: 1 },
    ],
    { duration: total / 2, easing: ease },
  ).finished.catch(() => {});
  els.camera.classList.remove('no-anim');
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
  if (level === 'L4') {
    // need an introspected component: the selection if it has facts, else the
    // nearest opted-in component under the current scope, else the first one.
    const snap = effectiveSnapshot();
    const graphs = snap.derived ?? [];
    const target =
      (state.selected && graphs.find((g) => state.selected === g.component || state.selected.startsWith(g.component + '.src.'))?.component) ??
      (state.scope && graphs.find((g) => g.component.startsWith(state.scope + '.') || g.component === state.scope)?.component) ??
      graphs[0]?.component;
    if (!target) return;
    state.scope = target;
    state.selected = target;
  }
  if (level === 'LD') {
    // The deployment overview needs no scope — it lists every environment
    // (ADR-0018). Diving from there walks the tree.
    if (!environments(effectiveSnapshot()).length) return;
    state.scope = null;
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
  els.helpBtn.addEventListener('click', () => openHelp(state.help === null ? '' : null));
  els.findBtn.addEventListener('click', () => openPalette());
  // '?' opens help. Guarded like the editing handler — the Ctrl+O handler
  // below is not, and typing "?" into a field must not hijack the panel.
  window.addEventListener('keydown', (ev) => {
    const t = ev.target;
    if (t && (t.tagName === 'TEXTAREA' || t.tagName === 'INPUT' || t.tagName === 'SELECT')) return;
    if (ev.key === '?' || ev.key === 'F1') {
      ev.preventDefault();
      openHelp(state.help === null ? '' : null);
    }
  });
  els.sideBack.addEventListener('click', () => {
    // Inside help, back steps to the index rather than leaving it.
    if (state.help) { state.help = ''; renderSide(); return; }
    state.doc = null; state.history = null; state.help = null; renderSide();
  });
  els.diffBtn.addEventListener('click', toggleDiff);
  els.historyBtn.addEventListener('click', openHistory);
  document.getElementById('share-btn').addEventListener('click', openShareDialog);

  // A context menu outlives nothing: any click, key or scroll elsewhere ends
  // it. Capture phase, so a click on the node underneath still closes it.
  document.addEventListener('pointerdown', (ev) => {
    if (ctxMenu && !ctxMenu.contains(ev.target)) closeNodeMenu();
  }, true);
  document.addEventListener('keydown', (ev) => {
    if (ev.key === 'Escape') closeNodeMenu();
  }, true);
  els.canvas.addEventListener('scroll', closeNodeMenu, true);
  // Right-click on empty canvas talks about the view rather than a box.
  els.canvas.addEventListener('contextmenu', openCanvasMenu);

  // theme cycle: auto -> light -> dark
  let theme = 'auto';
  document.getElementById('open-btn').addEventListener('click', () => openWorkspaceFlow('open'));
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
  // wheel = zoom about the cursor (trackpad pinch arrives as ctrl+wheel)
  els.canvas.addEventListener('wheel', (ev) => {
    if (!state.layout) return;
    ev.preventDefault();
    const rect = els.canvas.getBoundingClientRect();
    const delta = ev.deltaMode === 1 ? ev.deltaY * 16 : ev.deltaY;
    zoomAt({ x: ev.clientX - rect.left, y: ev.clientY - rect.top }, Math.exp(-delta * 0.0015));
  }, { passive: false });
  window.addEventListener('keydown', (ev) => {
    if (!(ev.ctrlKey || ev.metaKey)) return;
    const key = ev.key.toLowerCase();
    if (key === 'o') {
      ev.preventDefault();
      openWorkspaceFlow('open');
    } else if (key === 'k') {
      // Deliberately not guarded on the focused element: Ctrl+K is how you
      // get *out* of wherever you are and into something else.
      ev.preventDefault();
      if (state.palette) closePalette(); else openPalette();
    }
  });
}

// ---- find anything (0.7.0) --------------------------------------------------
// The tree lists the authored model in model order, which is the right shape
// for reading and the wrong one for looking something up. This is the other
// half: type a few letters, get elements, code-level detail, documents and
// relations ranked together, press Enter, land on it.

function openPalette() {
  closePalette();
  state.palette = true;
  const wrap = document.createElement('div');
  wrap.className = 'dialog-backdrop palette-backdrop';
  wrap.id = 'app-palette';
  wrap.innerHTML = `<div class="palette blueprint" role="dialog" aria-modal="true" aria-label="Find in the model">
    <input class="input palette-input" id="palette-q" type="text" autocomplete="off" spellcheck="false"
      placeholder="Find an element, document or relation…" aria-controls="palette-list"
      role="combobox" aria-expanded="true" aria-autocomplete="list">
    <div class="palette-list" id="palette-list" role="listbox"></div>
    <div class="palette-foot text-muted">↑↓ to move · Enter to open · Esc to close</div>
  </div>`;
  document.body.appendChild(wrap);
  wrap.addEventListener('click', (ev) => { if (ev.target === wrap) closePalette(); });

  const input = document.getElementById('palette-q');
  const list = document.getElementById('palette-list');
  let results = [];
  let active = 0;

  const paint = () => {
    results = searchModel(effectiveSnapshot(), input.value);
    active = 0;
    list.innerHTML = results.length
      ? results
          .map(
            (r, i) =>
              `<button class="palette-row${i === 0 ? ' is-active' : ''}" role="option"` +
              ` aria-selected="${i === 0}" data-i="${i}">` +
              `<span class="palette-tag">${esc(r.tag)}</span>` +
              `<span class="palette-title">${esc(r.title)}</span>` +
              `<span class="palette-sub text-muted">${esc(r.subtitle ?? '')}</span></button>`
          )
          .join('')
      : `<div class="palette-empty text-muted">Nothing matches “${esc(input.value)}”.</div>`;
    for (const row of list.querySelectorAll('[data-i]')) {
      row.addEventListener('click', () => choose(Number(row.dataset.i)));
    }
  };

  const highlight = () => {
    for (const row of list.querySelectorAll('[data-i]')) {
      const on = Number(row.dataset.i) === active;
      row.classList.toggle('is-active', on);
      row.setAttribute('aria-selected', String(on));
      if (on) row.scrollIntoView({ block: 'nearest' });
    }
  };

  const choose = async (i) => {
    const r = results[i];
    if (!r) return;
    closePalette();
    if (r.kind === 'relation') {
      selectRelation(r.relation);
    } else if (r.kind === 'doc') {
      state.help = null;
      state.doc = r.id;
      renderSide();
    } else {
      await focusElement(r.id);
    }
  };

  input.addEventListener('input', paint);
  input.addEventListener('keydown', (ev) => {
    if (ev.key === 'ArrowDown') {
      ev.preventDefault();
      active = results.length ? (active + 1) % results.length : 0;
      highlight();
    } else if (ev.key === 'ArrowUp') {
      ev.preventDefault();
      active = results.length ? (active - 1 + results.length) % results.length : 0;
      highlight();
    } else if (ev.key === 'Enter') {
      ev.preventDefault();
      choose(active);
    } else if (ev.key === 'Escape') {
      ev.preventDefault();
      closePalette();
    }
  });

  paint();
  input.focus();
}

function closePalette() {
  document.getElementById('app-palette')?.remove();
  state.palette = false;
}

// ---- panel resize (design-system contract: JS writes --panel-*-w) -----------

function wireResizers() {
  const setup = (gripId, cssVar, storageKey, bounds, growsRight) => {
    const grip = $(gripId);
    const panel = grip.parentElement;
    const root = document.documentElement;
    const apply = (w) => {
      const clamped = Math.round(Math.min(bounds[1], Math.max(bounds[0], w)));
      root.style.setProperty(cssVar, clamped + 'px');
      grip.setAttribute('aria-valuenow', String(clamped));
      localStorage.setItem(storageKey, String(clamped));
      if (state.layout) applyCamera(); // the canvas just changed size
      // CodeMirror caches its measured width; an external resize it didn't
      // cause (this grip) leaves it showing a stale, wrong-width scrollbar
      if (cssVar === '--panel-side-w' && srcCm) srcCm.refresh();
    };
    grip.setAttribute('aria-valuemin', String(bounds[0]));
    grip.setAttribute('aria-valuemax', String(bounds[1]));
    const saved = Number(localStorage.getItem(storageKey));
    if (saved) apply(saved);
    else grip.setAttribute('aria-valuenow', String(Math.round(panel.getBoundingClientRect().width)));
    grip.addEventListener('pointerdown', (ev) => {
      if (ev.button !== 0) return;
      ev.preventDefault();
      grip.setPointerCapture(ev.pointerId);
      const startX = ev.clientX;
      const startW = panel.getBoundingClientRect().width;
      const onMove = (mv) =>
        apply(startW + (growsRight ? mv.clientX - startX : startX - mv.clientX));
      grip.addEventListener('pointermove', onMove);
      grip.addEventListener(
        'pointerup',
        () => grip.removeEventListener('pointermove', onMove),
        { once: true },
      );
    });
    grip.addEventListener('keydown', (ev) => {
      const dir = ev.key === 'ArrowLeft' ? -1 : ev.key === 'ArrowRight' ? 1 : 0;
      if (!dir) return;
      ev.preventDefault();
      apply(panel.getBoundingClientRect().width + 16 * dir * (growsRight ? 1 : -1));
    });
  };
  // bounds mirror the design-system clamp tokens (layout.css)
  setup('nav-grip', '--panel-nav-w', 'br-nav-w', [168, 320], true);
  setup('side-grip', '--panel-side-w', 'br-side-w', [260, 480], false);
}

// ---- onboarding (phase 5) ---------------------------------------------------

/** First-run screen: no workspace is open. Also the landing state after the
 * startup folder failed to resolve. */
function renderWelcome() {
  state.snapshot = null;
  els.nodes.textContent = '';
  els.edges.textContent = '';
  els.tree.textContent = '';
  els.breadcrumb.textContent = 'Blastradius';
  document.querySelector('.welcome')?.remove();
  const w = document.createElement('div');
  w.className = 'welcome';
  w.innerHTML = `<div class="dialog blueprint welcome-card">
    <i class="corner tl"></i><i class="corner tr"></i><i class="corner bl"></i><i class="corner br"></i>
    <span class="welcome-kicker">BLASTRADIUS</span>
    <span class="dialog-title">Model your architecture</span>
    <p class="text-muted">Interactive C4 models as plain YAML in your repo —
      local-first, versioned by git, diffable in PRs.</p>
    <div class="welcome-actions">
      <button class="btn btn-primary" id="welcome-open">Open a folder or repository…</button>
      <button class="btn btn-ghost" id="welcome-demo">Try a demo workspace</button>
    </div>
    <p class="text-muted welcome-foot">Point it at any folder: an existing
      workspace opens, a repository is searched, and somewhere new is offered a
      starting model.
      <button class="btn btn-ghost" id="welcome-help">Read the help</button></p>
  </div>`;
  els.canvas.appendChild(w);
  document.getElementById('welcome-open').addEventListener('click', () => openWorkspaceFlow('open'));
  document.getElementById('welcome-help').addEventListener('click', () => openHelp(''));
  document.getElementById('welcome-demo').addEventListener('click', async () => {
    try {
      await invoke('workspace_demo');
      await switchedWorkspace();
    } catch (e) {
      toast(String(e));
    }
  });
}

/** Pick a folder and go somewhere useful with it.
 *
 * One action, three outcomes: a workspace here or below opens, several offer a
 * choice, and none offers to make one — the case that used to be an error
 * naming the folder, which for someone pointing the app at their own repo for
 * the first time was the entire experience (docs/roadmap.md).
 */
async function openWorkspaceFlow() {
  try {
    const path = await invoke('pick_folder');
    if (!path) return; // dialog cancelled
    const res = await invoke('workspace_open', { path });
    if (res?.candidates) return pickWorkspaceDialog(res.candidates);
    if (res?.empty) return initWorkspaceDialog(res.empty, res.suggest ?? 'docs');
    await switchedWorkspace();
  } catch (e) {
    toast(String(e));
  }
}

/** The agents `blastradius init` knows how to wire up. Ids must match
 *  core::onboard::AGENTS — ui/tests/onboarding.test.mjs asserts they do. */
const AGENTS = [
  { id: 'claude', label: 'Claude Code' },
  { id: 'copilot', label: 'GitHub Copilot' },
  { id: 'cursor', label: 'Cursor' },
  { id: 'codex', label: 'Codex' },
];

/** No workspace in the picked folder: offer to start one, and let the user
 *  pick the same pieces `blastradius init` offers — which parts, which
 *  agents — rather than one all-or-nothing checkbox. */
function initWorkspaceDialog(path, suggest = 'docs') {
  const shown = path.length > 60 ? `…${path.slice(-59)}` : path;
  const agentBoxes = AGENTS.map(
    (a) => `<label class="dlg-check">
      <input type="checkbox" class="dlg-agent" value="${a.id}" checked> ${esc(a.label)}
    </label>`
  ).join('');
  openDialog({
    title: 'Start a model here?',
    body: `<p class="text-muted">No Blastradius workspace in
        <span style="font-family:var(--font-mono)">${esc(shown)}</span>.
        A starter model will be scaffolded — plain YAML, yours to edit or
        delete. Files that already exist are left alone.</p>
      <div class="dlg-field dlg-group">
        <label for="dlg-location">Put the workspace in</label>
        <input class="input" id="dlg-location" value="${esc(suggest)}" spellcheck="false">
        <span class="dlg-id-preview">A folder inside the project — the model is
          documentation and reads better beside it. Use <b>.</b> for the
          project root.</span>
      </div>
      <div class="dlg-group">
        <span class="dlg-group-title">Set up for coding agents</span>
        <label class="dlg-check">
          <input type="checkbox" id="dlg-mcp" checked>
          MCP server <span class="text-muted">— lets an agent query and edit the model</span>
        </label>
        <label class="dlg-check">
          <input type="checkbox" id="dlg-skills" checked>
          Skills and instructions <span class="text-muted">— teaches it the format and C4</span>
        </label>
      </div>
      <div class="dlg-group">
        <span class="dlg-group-title">For which agents</span>
        <div class="dlg-agents">${agentBoxes}</div>
      </div>
      <p class="dlg-error" id="dlg-error" hidden></p>`,
    confirm: 'Create workspace',
    onConfirm: async () => {
      const chosen = [...document.querySelectorAll('.dlg-agent:checked')].map((c) => c.value);
      const want = (id) => (document.getElementById(id).checked ? chosen : []);
      try {
        const res = await invoke('workspace_init', {
          path,
          location: document.getElementById('dlg-location').value.trim() || '.',
          agents: { mcp: want('dlg-mcp'), skills: want('dlg-skills') },
        });
        await switchedWorkspace();
        if (res?.prompt && (res.log ?? []).length) {
          // openDialog closes the current dialog on a truthy confirm, which
          // would take this one with it — false leaves the replacement up.
          startedDialog(res.prompt, res.log, res.kept ?? [], res.workflows ?? []);
          return false;
        }
        return;
      } catch (e) {
        // Inline, not only a toast: this dialog stays open on failure, and a
        // dialog that stays open without saying why is what the 0.6.0 build
        // did when the scaffold hit an existing README.
        const err = document.getElementById('dlg-error');
        if (err) {
          err.textContent = String(e);
          err.hidden = false;
        }
        toast(String(e));
        return false;
      }
    },
  });
}

/** The workspace exists and the agents are wired: hand over the prompt that
 *  turns it into a real model. "Initialised successfully" is not an answer to
 *  "now what?". */
function startedDialog(prompt, log, kept = [], workflows = []) {
  const wrote = log.filter((l) => l.startsWith('wrote ')).map((l) => l.slice(6));
  const failed = log.filter((l) => !l.startsWith('wrote ') && !l.includes('already'));
  openDialog({
    title: 'Ready — now ask your agent',
    body: `<p class="text-muted">Paste this into Claude Code, Copilot, Cursor or
        Codex in this repository:</p>
      <textarea class="input dlg-prompt" id="dlg-prompt" rows="6" readonly>${esc(prompt)}</textarea>
      ${workflows.length ? `<div class="dlg-workflows">
        <span class="dlg-group-title">Three workflows were installed</span>
        <ul>${workflows.map((w) => `<li>${esc(w)}</li>`).join('')}</ul>
        <p class="text-muted dlg-note">Modelling is the first one. The other two
          are for later, once the model exists and the code has moved on.</p>
      </div>` : ''}
      ${wrote.length ? `<p class="text-muted dlg-note">Wrote ${esc(wrote.join(', '))}.
        Your agent may need to be restarted, and Claude Code will ask you to
        approve the project's MCP server the first time.</p>` : ''}
      ${kept.length ? `<p class="text-muted dlg-note">Kept your existing
        ${esc(kept.join(', '))} — untouched.</p>` : ''}
      ${failed.length ? `<p class="dlg-error">${esc(failed.join('; '))}</p>` : ''}`,
    confirm: 'Copy prompt',
    cancel: 'Done',
    onConfirm: async () => {
      try {
        await navigator.clipboard.writeText(prompt);
        toast('Prompt copied');
      } catch {
        document.getElementById('dlg-prompt').select();
        toast('Select and copy the prompt');
        return false;
      }
    },
  });
}

/** The picked folder is a monorepo with several workspaces: let the user choose. */
function pickWorkspaceDialog(candidates) {
  const opts = candidates
    .map((p) => `<option value="${esc(p)}">${esc(p)}</option>`)
    .join('');
  openDialog({
    title: 'Choose a workspace',
    body: `<div class="dlg-field">
      <label for="dlg-ws">This folder contains ${candidates.length} workspaces</label>
      <select class="input" id="dlg-ws">${opts}</select>
    </div>`,
    confirm: 'Open',
    onConfirm: async () => {
      try {
        await invoke('workspace_open', { path: document.getElementById('dlg-ws').value });
        await switchedWorkspace();
      } catch (e) {
        toast(String(e));
        return false;
      }
    },
  });
}

/** Full state reset — a different workspace means a different everything. */
async function switchedWorkspace() {
  Object.assign(state, {
    snapshot: null, level: 'L1', scope: null, selected: null,
    zoom: 1, pan: { x: 0, y: 0 }, layout: null, doc: null,
    git: null, conflicts: null, conflictChoices: {}, diff: null, diffOn: false, diffBase: null,
    showLayoutDiff: false, history: null, travel: null,
    sync: null, srcFile: null, connectFrom: null, selectedRel: null,
    scopeInit: false,
  });
  document.querySelector('.welcome')?.remove();
  await reload();
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
    // Derived scopes (L4): walk the graph's parent chain — fact ids contain
    // dots, so the segment loop above cannot see them.
    const graph = derivedGraphFor(snap, state.scope);
    if (graph && state.scope !== graph.component) {
      const chain = [];
      const byIdMap = new Map(graph.elements.map((e) => [e.id, e]));
      for (let el = byIdMap.get(state.scope); el; el = el.parent ? byIdMap.get(el.parent) : null) {
        chain.unshift(`<b>${esc(el.name)}</b>`);
      }
      parts.push(...chain);
    }
  }
  parts.push({ L1: 'Context', L2: 'Containers', L3: 'Components', L4: 'Code', LD: 'Deployment' }[state.level]);
  if (state.level === 'L4' && derivedGraphFor(snap, state.scope)?.stale) {
    parts.push(`<span class="crumb-stale" title="The committed facts lag the source tree — run blastradius introspect">stale</span>`);
  }
  els.breadcrumb.innerHTML = parts.join(' / ');
}

function syncLevelSeg() {
  for (const input of els.levelSeg.querySelectorAll('input')) {
    input.checked = input.value === state.level;
    input.closest('.seg-opt').classList.toggle('is-active', input.value === state.level);
    if (input.value === 'L4') {
      // Live only when the model has introspected components to jump to.
      const usable = (effectiveSnapshot().derived ?? []).length > 0;
      input.disabled = !usable;
      input.closest('.seg-opt').classList.toggle('is-disabled', !usable);
    }
    if (input.value === 'LD') {
      // Live only when the model declares environments (ADR-0018).
      const usable = environments(effectiveSnapshot()).length > 0;
      input.disabled = !usable;
      input.closest('.seg-opt').classList.toggle('is-disabled', !usable);
    }
  }
}

// ---- tree -------------------------------------------------------------------
function renderTree() {
  const t = treeModel(effectiveSnapshot());
  const rows = [];
  rows.push(`<span class="tree-label">Model</span>`);
  for (const c of t.context) rows.push(treeRow(c.el ?? c, 0, '◦'));
  const snapForTree = effectiveSnapshot();
  for (const s of t.systems) {
    rows.push(treeRow(s.el, 0, '▸'));
    for (const c of s.containers) {
      rows.push(treeRow(c.el, 1, ''));
      for (const k of c.components) {
        rows.push(treeRow(k, 2, ''));
        // Introspected code (L4) nests under its component, visibly code:
        // modules first, their types one step deeper.
        const graph = (snapForTree.derived ?? []).find((g) => g.component === k.id);
        for (const m of graph?.elements.filter((e) => !e.parent) ?? []) {
          rows.push(treeRow(m, 3, '', ' is-derived'));
          for (const ty of graph.elements.filter((e) => e.parent === m.id)) {
            rows.push(treeRow(ty, 4, '', ' is-derived'));
          }
        }
      }
    }
  }
  if (t.deployment.length) {
    rows.push(`<span class="tree-label">Deployment</span>`);
    const walk = (n, depth) => {
      rows.push(treeRow(n.el, depth, depth === 0 ? '▸' : ''));
      for (const child of n.children) walk(child, depth + 1);
    };
    for (const env of t.deployment) walk(env, 0);
  }
  els.tree.innerHTML = rows.join('');
  for (const btn of els.tree.querySelectorAll('.tree-row[data-id]')) {
    btn.addEventListener('click', () => focusElement(btn.dataset.id));
    btn.addEventListener('dblclick', () => dive(btn.dataset.id));
  }
}

function treeRow(el, depth, glyph, extra = '') {
  let active = state.selected === el.id ? ' is-active' : '';
  const change = diffChangeMap().get(el.id);
  if (change === 'added') active += ' is-added';
  if (change === 'removed') active += ' is-removed';
  const pad = depth ? ` style="padding-left:${14 + depth * 14}px"` : '';
  return `<button class="tree-row${active}${extra}" data-id="${esc(el.id)}"${pad}>` +
    `<span class="glyph">${glyph}</span>${esc(el.name)}</button>`;
}

/** Select an element; if it is not visible at the current altitude, go to it. */
async function focusElement(id) {
  const el = state.snapshot.elements.find((e) => e.id === id);
  if (!el) {
    // Derived (L4) elements: jump the canvas to their code altitude.
    const graph = derivedGraphFor(effectiveSnapshot(), id);
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
  if (state.sideMode === 'source') { renderSource(); return; }
  els.srcStatus.hidden = true;
  if (state.sideMode === 'view') return renderViewPanel();
  if (state.help !== null) return renderHelp(state.help);
  if (state.history) return renderHistory();
  if (state.doc) return renderDoc(state.doc);
  if (state.selectedRel) return renderRelationSide();
  els.sideBack.hidden = true;
  const id = state.selected;
  if (!id) {
    els.sideTitle.textContent = 'Inspector';
    els.sideBody.innerHTML = `<p class="side-empty text-muted">Select an element to inspect it.</p>`;
    return;
  }
  const snap = effectiveSnapshot();
  const el = snap.elements.find((e) => e.id === id) ?? anyElement(id);
  if (!el) return;
  els.sideTitle.textContent = 'Inspector';
  if (el.derived) return renderDerivedSide(el);

  const rels = snap.relations.filter((r) => r.from === id || r.to === id);
  const docs = docsFor(snap, id);
  const nameOf = (eid) => snap.elements.find((e) => e.id === eid)?.name ?? eid;

  let html = `<div class="insp">`;
  html += `<span class="insp-kicker">${esc(kicker(el))}</span>`;
  if (canEdit()) {
    html += `<input class="input insp-name-input" id="insp-name" value="${esc(el.name)}" aria-label="Element name">`;
  } else {
    html += `<span class="insp-title">${esc(el.name)}</span>`;
  }
  html += `<span class="mono text-muted" style="font-family:var(--font-mono);font-size:var(--text-2xs)">${esc(el.id)}</span>`;
  // Multiplicity is a fact about the element, not decoration on the box
  // (ADR-0018 `replicas`) — say it here too, in words.
  const many = multiplicity(el);
  if (many) {
    html += `<p class="insp-desc text-muted">${esc(many)} — ${el.replicas} of these run.</p>`;
  }
  if (canEdit()) {
    // The description is a model field like the name, so it is edited beside
    // it. Where it is *drawn* is the diagram's business — right-click the box.
    html += `<textarea class="input insp-desc-input" id="insp-desc" rows="3"
      placeholder="What is this, in a sentence?"
      aria-label="Element description">${esc(el.description ?? '')}</textarea>`;
  } else if (el.description) {
    html += `<p class="insp-desc">${esc(el.description)}</p>`;
  }
  html += canEdit() ? propertiesHtml(el) : readOnlyProperties(el);

  if (rels.length) {
    html += `<div class="insp-section">Relations</div>`;
    for (const r of rels) {
      const out = r.from === id;
      const arrow = r.direction === 'both' ? '↔' : out ? '→' : '←';
      const other = out ? r.to : r.from;
      html += `<div class="insp-rel">${arrow} ${esc(nameOf(other))}` +
        (r.label ? ` <span class="text-muted">· ${esc(r.label)}</span>` : '') +
        (r.protocol ? ` <span class="proto">[${esc(r.protocol)}]</span>` : '') + `</div>`;
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
  for (const btn of els.sideBody.querySelectorAll('[data-conflict-choice]')) {
    btn.addEventListener('click', () => {
      state.conflictChoices[btn.dataset.conflictId] = btn.dataset.conflictChoice;
      renderSide();
    });
  }
  document.getElementById('conf-apply')?.addEventListener('click', applyResolution);
  const nameInput = document.getElementById('insp-name');
  if (nameInput) {
    nameInput.addEventListener('change', async () => {
      const name = nameInput.value.trim();
      if (name && name !== el.name) {
        await applyOp({ op: 'rename', id, name });
      }
    });
  }
  if (canEdit()) wireProperties(el);
  const descInput = document.getElementById('insp-desc');
  if (descInput) {
    descInput.addEventListener('change', async () => {
      const value = descInput.value.trim();
      // An emptied box removes the field rather than writing `description: ""`.
      if (value !== (el.description ?? '')) {
        await applyOp({ op: 'set-field', id, field: 'description', value });
      }
    });
  }
}

/**
 * The rest of the element, in the inspector.
 *
 * `set-field` has always whitelisted `tech`, and nothing in the app ever
 * offered it — while every box in the product renders it in brackets. `group`,
 * `replicas`, `external` and `source` had no operation at all before 0.9.0, so
 * grouping and code-level detail were reachable only by hand-editing YAML,
 * which is the step a first-time user is least equipped for.
 *
 * Each field is offered where the format allows it (spec §3, §3b, §3c) rather
 * than everywhere with a refusal waiting: `replicas` counts things that run,
 * `external` marks a system outside your control, `source:` is per component.
 */
function propertiesHtml(el) {
  const field = (id, label, control, hint) =>
    `<div class="insp-field"><label for="${id}">${esc(label)}</label>${control}` +
    (hint ? `<span class="insp-hint">${esc(hint)}</span>` : '') + `</div>`;
  const text = (id, value, placeholder) =>
    `<input class="input" id="${id}" value="${esc(value ?? '')}" placeholder="${esc(placeholder)}">`;

  let html = `<div class="insp-section">Properties</div>`;
  html += field('insp-tech', 'Technology', text('insp-tech', el.tech, 'Rust, React, Postgres…'),
    'Reads in brackets on the box.');
  html += field('insp-group', 'Group', text('insp-group', el.group, 'Storefront'),
    'Draws a boundary round siblings sharing it, in views that ask for one.');
  if (el.kind === 'deployment-node' || el.kind === 'container-instance') {
    html += field('insp-replicas', 'Replicas',
      `<input class="input" id="insp-replicas" type="number" min="1" step="1"
        value="${el.replicas ?? ''}" placeholder="1">`,
      'How many of it actually run. One is the default and is never drawn.');
  }
  if (el.kind === 'system') {
    html += `<label class="insp-check"><input type="checkbox" id="insp-external"` +
      `${el.external ? ' checked' : ''}> Outside your control</label>`;
  }
  if (el.kind === 'component') html += sourceHtml(el);
  return html;
}

/** The same properties where the workspace cannot be written to — conflicted,
 *  stale, or being time-travelled. What is set, with nothing to type into.
 *  `tech` is already in the kicker and `replicas` in the multiplicity line, so
 *  neither is repeated here. */
function readOnlyProperties(el) {
  const rows = [];
  if (el.group) rows.push(['Group', el.group]);
  if (el.source) rows.push(['Code', `${el.source.language} · ${el.source.root}`]);
  if (!rows.length) return '';
  return `<div class="insp-section">Properties</div>` + rows.map(([k, v]) =>
    `<div class="insp-rel"><span class="text-muted">${esc(k)}</span> ${esc(v)}</div>`).join('');
}

/** The `source:` mapping — a component's opt-in to code-level detail. */
function sourceHtml(el) {
  const src = el.source;
  const opt = (v, sel, label) => `<option value="${v}"${v === sel ? ' selected' : ''}>${label ?? v}</option>`;
  const derived = derivedGraphFor(effectiveSnapshot(), el.id);
  let html = `<div class="insp-section">Code level</div>`;
  if (!src) {
    return html + `<p class="insp-hint">Point this component at the code that implements it and
      the canvas can dive into its modules and types.</p>
      <button class="btn btn-secondary" id="map-add">Add a source mapping…</button>`;
  }
  html += `<div class="insp-field"><label for="map-language">Language</label>
    <select class="input" id="map-language">
      ${['typescript', 'csharp', 'rust'].map((l) => opt(l, src.language)).join('')}
    </select></div>`;
  html += `<div class="insp-field"><label for="map-root">Root</label>
    <input class="input" id="map-root" value="${esc(src.root)}" placeholder="crates/core/src">
    <span class="insp-hint">Relative to the repository root, not to the workspace.</span></div>`;
  html += `<div class="insp-field"><label for="map-include">Include</label>
    <input class="input" id="map-include" value="${esc((src.include ?? []).join(', '))}"
      placeholder="empty = the language's defaults">
    <span class="insp-hint">Comma-separated globs, relative to the root.</span></div>`;
  html += `<div class="insp-field"><label for="map-exclude">Exclude</label>
    <input class="input" id="map-exclude" value="${esc((src.exclude ?? []).join(', '))}"
      placeholder="**/*.test.ts"></div>`;
  html += `<div class="insp-field"><label for="map-mode">Mode</label>
    <select class="input" id="map-mode">
      ${opt('', src.mode ?? '', 'syntax (default)')}${opt('semantic', src.mode ?? '')}
    </select>
    <span class="insp-hint">Semantic resolves cross-project references; C# only.</span></div>`;
  html += `<div class="insp-actions">
    <button class="btn btn-primary" id="map-save">Save mapping</button>
    <button class="btn btn-secondary" id="map-run">Run introspection</button>
    <button class="btn btn-secondary" id="map-remove">Remove</button></div>`;
  if (derived?.elements?.length) {
    html += `<p class="insp-hint">${derived.elements.length} code elements derived${
      derived.stale ? ' — the committed facts lag the source tree.' : '.'}</p>`;
  } else {
    html += `<p class="insp-hint">No facts committed yet — run introspection.</p>`;
  }
  return html;
}

/** Commit the property fields. Each writes on change, like name and
 *  description: there is no save button because there is no buffer. */
function wireProperties(el) {
  const set = (field, value) => applyOp({ op: 'set-field', id: el.id, field, value });
  const onChange = (id, field, read = (i) => i.value.trim()) => {
    const input = document.getElementById(id);
    input?.addEventListener('change', () => set(field, read(input)));
  };
  onChange('insp-tech', 'tech');
  onChange('insp-group', 'group');
  onChange('insp-replicas', 'replicas');
  document.getElementById('insp-external')?.addEventListener('change', (ev) =>
    set('external', ev.target.checked ? 'true' : 'false'));

  document.getElementById('map-add')?.addEventListener('click', () => {
    // A mapping needs a language and a root; the dialog asks for both rather
    // than writing a half one that cannot be introspected.
    openSourceDialog(el.id);
  });
  const read = () => ({
    language: document.getElementById('map-language').value,
    root: document.getElementById('map-root').value.trim(),
    include: splitGlobs(document.getElementById('map-include').value),
    exclude: splitGlobs(document.getElementById('map-exclude').value),
    mode: document.getElementById('map-mode').value || null,
  });
  document.getElementById('map-save')?.addEventListener('click', () =>
    applyOp({ op: 'set-source', id: el.id, source: read() }));
  document.getElementById('map-remove')?.addEventListener('click', () =>
    applyOp({ op: 'set-source', id: el.id, source: null }));
  document.getElementById('map-run')?.addEventListener('click', () => runIntrospection(el.id));
}

const splitGlobs = (value) => value.split(',').map((g) => g.trim()).filter(Boolean);

/** Ask for the two things a mapping cannot do without, then write it. */
function openSourceDialog(id) {
  openDialog({
    title: 'Code for this component',
    body: `<div class="dlg-field"><label for="dlg-language">Language</label>
        <select class="input" id="dlg-language">
          <option value="typescript">typescript</option>
          <option value="csharp">csharp</option>
          <option value="rust">rust</option>
        </select></div>
      <div class="dlg-field"><label for="dlg-root">Root folder</label>
        <input class="input" id="dlg-root" placeholder="crates/core/src">
        <span class="dlg-id-preview">Relative to the repository root, not the workspace.</span></div>`,
    confirm: 'Add',
    onConfirm: async () => {
      const root = document.getElementById('dlg-root').value.trim();
      if (!root) { toast('a root folder is required'); return false; }
      return applyOp({
        op: 'set-source',
        id,
        source: { language: document.getElementById('dlg-language').value, root, include: [], exclude: [] },
      });
    },
  });
}

/** Run the extractor for one component and write its facts. The mapping says
 *  what to look at; this is what looks. */
async function runIntrospection(id) {
  toast('introspecting…');
  try {
    const res = await invoke('introspect_component', { id });
    const warnings = res?.warnings ?? [];
    toast(`${res?.elements ?? 0} code elements derived` +
      (warnings.length ? ` · ${warnings.join(' · ')}` : ''));
  } catch (e) {
    toast(String(e));
    return;
  }
  state.snapshot = await invoke('workspace_snapshot');
  renderTree();
  await renderCanvas({ animate: false });
  renderSide();
}

/**
 * The view panel: what *this diagram* says, as opposed to what the model says.
 *
 * Every one of these settings existed in the file format and only one of them
 * (descriptions, and only from a box's right-click) had any affordance at all.
 * Nothing in the app said a view file existed, what was in it, or how to turn
 * the rest on — so `show-groups` in particular was a key you could only reach
 * by opening YAML, which made the `group:` labels the inspector can now write
 * invisible in practice.
 */
function renderViewPanel() {
  els.sideBack.hidden = true;
  els.sideTitle.textContent = 'View';
  const snap = effectiveSnapshot();
  const viewDef = findViewDef(snap, state.level, state.scope);
  const nameOf = (id) => snap.elements.find((e) => e.id === id)?.name ?? id;
  const editable = canPin(); // the same gate pinning uses: stale view, no edit

  let html = `<div class="insp">`;
  html += `<span class="insp-kicker">${esc(LEVEL_NAMES[state.level] ?? state.level)}</span>`;
  html += `<span class="insp-title">${esc(viewDef?.name ?? viewDef?.id ?? 'Unsaved view')}</span>`;
  html += `<span class="mono text-muted" style="font-family:var(--font-mono);font-size:var(--text-2xs)">` +
    `${esc(state.scope ? nameOf(state.scope) : 'the whole model')}</span>`;

  if (state.level === 'L4') {
    // Derived layouts are pure auto-layout (spec/l4-introspection.md): there is
    // no view file to have settings in, and nothing here to decide.
    html += `<p class="insp-hint">Code level is derived from source and laid out
      automatically — it has no view file and nothing to set.</p></div>`;
    els.sideBody.innerHTML = html;
    return;
  }

  html += viewDef
    ? `<p class="insp-hint">Written to <button class="doc-link" data-editfile="${esc(viewDef.file ?? '')}">${esc(viewDef.file ?? viewDef.id)}</button></p>`
    : `<p class="insp-hint">No view file yet. The first setting you change writes one.</p>`;

  html += `<div class="insp-section">Drawing</div>`;
  const check = (flag, label, on, hint) =>
    `<label class="insp-check"><input type="checkbox" data-flag="${flag}"${on ? ' checked' : ''}` +
    `${editable ? '' : ' disabled'}> ${esc(label)}</label>` +
    `<span class="insp-hint">${esc(hint)}</span>`;
  html += check('show-groups', 'Draw group boundaries', Boolean(viewDef?.show_groups),
    'Elements sharing a group: label are drawn inside one box.');
  html += check('include-context', 'Include context',
    viewDef ? viewDef.include_context !== false : true,
    'People and external systems related to what this view is about.');
  if (state.level === 'LD') {
    html += check('nested', 'Nested boxes', Boolean(viewDef?.nested),
      'Draw the whole subtree in one frame instead of diving into it.');
  }

  const pins = pinnedIds();
  html += `<div class="insp-section">Pinned (${pins.size})</div>`;
  if (pins.size) {
    for (const id of [...pins].sort()) {
      html += `<div class="insp-rel">${esc(nameOf(id))}` +
        (editable ? ` <button class="btn btn-ghost" data-unpin="${esc(id)}">release</button>` : '') +
        `</div>`;
    }
    if (editable) {
      html += `<div class="insp-actions"><button class="btn btn-secondary" id="view-reset">Back to auto-layout</button></div>`;
    }
  } else {
    html += `<span class="text-muted" style="font-size:var(--text-sm)">Nothing pinned — every box is placed by the layout engine.</span>`;
  }

  const described = state.layout?.nodes.filter((n) => n.describe) ?? [];
  html += `<div class="insp-section">Descriptions on the box (${described.length})</div>`;
  if (described.length) {
    for (const n of described) {
      html += `<div class="insp-rel">${esc(nameOf(n.id))}` +
        (editable ? ` <button class="btn btn-ghost" data-undescribe="${esc(n.id)}">hide</button>` : '') +
        `</div>`;
    }
  } else {
    html += `<span class="text-muted" style="font-size:var(--text-sm)">None. Right-click a box to draw its description.</span>`;
  }
  html += `</div>`;
  els.sideBody.innerHTML = html;

  for (const box of els.sideBody.querySelectorAll('[data-flag]')) {
    box.addEventListener('change', () => applyOp({
      op: 'set-view-flag',
      view: viewDef?.id ?? null,
      level: state.level,
      scope: state.scope,
      flag: box.dataset.flag,
      value: box.checked,
    }));
  }
  for (const btn of els.sideBody.querySelectorAll('[data-unpin]')) {
    btn.addEventListener('click', () => unpin(btn.dataset.unpin));
  }
  for (const btn of els.sideBody.querySelectorAll('[data-undescribe]')) {
    btn.addEventListener('click', () => applyOp({
      op: 'show-description',
      view: viewDef?.id ?? null,
      level: state.level,
      scope: state.scope,
      id: btn.dataset.undescribe,
      show: false,
    }));
  }
  document.getElementById('view-reset')?.addEventListener('click', () => unpin(null));
  for (const btn of els.sideBody.querySelectorAll('[data-editfile]')) {
    btn.addEventListener('click', () => invoke('open_in_editor', { rel: btn.dataset.editfile }).catch(() => {}));
  }
}

/** What each altitude is called, for the panel's kicker. */
const LEVEL_NAMES = {
  L1: 'Context', L2: 'Containers', L3: 'Components', L4: 'Code', LD: 'Deployment',
};

/** Inspector for a derived (L4) element: read-only by nature — the source
 * file is the thing to edit (spec/l4-introspection.md). */
function renderDerivedSide(el) {
  const graph = derivedGraphFor(effectiveSnapshot(), el.id);
  const loc = el.line ? `${el.path}:${el.line}` : el.path;
  let html = `<div class="insp">`;
  html += `<span class="insp-kicker">${esc(kicker(el))}</span>`;
  // Code identity is case-sensitive: `CommitInfo` must not read COMMITINFO.
  html += `<span class="insp-title is-code">${esc(el.name)}</span>`;
  html += `<span class="mono text-muted" style="font-family:var(--font-mono);font-size:var(--text-2xs)">${esc(el.id)}</span>`;
  if (el.path) {
    html += `<div class="insp-section">Source</div>`;
    html += `<button class="doc-link" data-opensrc="${esc(el.path)}">` +
      `<span class="tag tag-outline">${esc(graph?.language ?? 'code')}</span> ${esc(loc)}</button>`;
    html += `<p class="text-muted" style="font-size:var(--text-sm)">Derived from source — edit the file and re-run <code>blastradius introspect</code>.</p>`;
  } else {
    // Dependency rollups (and namespaces, which own no single file) have no
    // path — there is nothing to open (spec/l4-introspection.md).
    html += `<div class="insp-section">External</div>`;
    html += `<p class="text-muted" style="font-size:var(--text-sm)">Resolved from imports — not part of the mapped source tree.</p>`;
  }
  if (el.stale) {
    html += `<p class="text-muted" style="font-size:var(--text-sm)">⚠ The committed facts lag the source tree.</p>`;
  }
  html += `</div>`;
  els.sideBody.innerHTML = html;
  for (const btn of els.sideBody.querySelectorAll('[data-opensrc]')) {
    btn.addEventListener('click', () => invoke('open_source', { rel: btn.dataset.opensrc }).catch(() => {}));
  }
}

/** Bundled help: '' shows the index, an id shows that page. */
function renderHelp(pageId) {
  els.sideBack.hidden = !pageId; // the index is the top of this stack
  if (!pageId) {
    els.sideTitle.textContent = 'Help';
    let html = `<p class="side-empty text-muted">How to use Blastradius. Everything here ships with the app — no network needed.</p>`;
    html += `<div class="doc-elements">`;
    for (const p of HELP_PAGES) {
      html += `<button class="doc-link" data-help="${esc(p.id)}">` +
        `<span>${esc(p.title)}</span> <span class="text-muted">${esc(p.blurb)}</span></button>`;
    }
    html += `</div>`;
    els.sideBody.innerHTML = html;
    wireHelpLinks();
    return;
  }
  const page = HELP_PAGES.find((p) => p.id === pageId);
  els.sideTitle.textContent = page?.title ?? 'Help';
  els.sideBody.innerHTML = `<p class="side-empty text-muted">Loading…</p>`;
  helpBody(pageId).then((md) => {
    // A slower load must not overwrite a page the reader has since moved on from.
    if (state.help !== pageId) return;
    if (md === null) {
      els.sideBody.innerHTML = `<p class="side-empty text-muted">That help page is missing from this build.</p>`;
      return;
    }
    els.sideBody.innerHTML = `<div class="doc-body">${renderHelpMarkdown(md)}</div>`;
    wireHelpLinks();
  });
}

/**
 * Help markdown with cross-page links turned into in-panel buttons. The docs
 * panel has no router, so a bare `canvas.md` href would navigate the WebView
 * off the app entirely.
 */
function renderHelpMarkdown(md) {
  const html = marked.parse(md);
  return html.replace(/<a href="([^"]+)"/g, (whole, href) => {
    const target = helpLinkTarget(href);
    return target ? `<a href="#" data-help="${esc(target)}"` : `${whole} target="_blank" rel="noreferrer"`;
  });
}

function wireHelpLinks() {
  for (const el of els.sideBody.querySelectorAll('[data-help]')) {
    el.addEventListener('click', (ev) => {
      ev.preventDefault();
      openHelp(el.dataset.help);
    });
  }
}

function openHelp(pageId = '') {
  state.help = pageId;
  state.doc = null;
  state.history = null;
  if (state.sideMode === 'source') state.sideMode = 'inspect';
  renderSide();
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
  state.help = null; // help is not a mode you can get stuck in
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

/** Conflict details for the inspector, when the selected element is
 * conflicted: both sides side-by-side, a keep-ours/theirs choice for THIS
 * element, and the apply-all action (ADR-0015 — splice-rebuilt from the
 * chosen side, validated, staged; undecided elements keep ours). */
function conflictSection(id) {
  const c = conflictMap().get(id);
  if (!c) return '';
  const row = (label, side) => side
    ? `<tr><td>${esc(label)}</td><td>${esc(side.name)}</td><td>${esc(side.tech ?? '')}</td></tr>`
    : `<tr><td>${esc(label)}</td><td colspan="2" class="text-muted">not present</td></tr>`;
  const chosen = state.conflictChoices[id] ?? 'ours';
  const pick = (side, label) =>
    `<button class="btn btn-secondary${chosen === side ? ' is-on' : ''}"
      data-conflict-choice="${side}" data-conflict-id="${esc(id)}">${label}</button>`;
  const total = state.conflicts?.elements?.length ?? 0;
  const decided = Object.keys(state.conflictChoices).length;
  const files = (state.git?.conflicted ?? [])
    .map((f) => `<button class="doc-link" data-editfile="${esc(f)}">↗ resolve ${esc(f)} in editor</button>`)
    .join('');
  return `<div class="insp-section">Merge conflict</div>
    <table class="conf-table">
      <thead><tr><th>side</th><th>name</th><th>tech</th></tr></thead>
      <tbody>${row('ours', c.ours)}${row('theirs', c.theirs)}</tbody>
    </table>
    <div class="conf-actions">${pick('ours', 'Keep ours')}${pick('theirs', 'Keep theirs')}</div>
    <button class="btn btn-primary" id="conf-apply"
      title="Undecided elements keep ours; the result is validated before anything is written, then staged">
      Resolve ${total} conflict${total === 1 ? '' : 's'} (${decided} decided)</button>
    ${files}`;
}

async function applyResolution() {
  try {
    const files = await invoke('resolve_conflicts', {
      resolution: { elements: state.conflictChoices },
    });
    toast(`Resolved & staged: ${files.join(', ')}`);
    state.conflictChoices = {};
    await reload();
  } catch (e) {
    toast(String(e));
  }
}

// ---- editing (phase 3) ------------------------------------------------------

function wireEditing() {
  els.undoBtn.addEventListener('click', () => doUndo());
  els.redoBtn.addEventListener('click', () => doRedo());
  els.addBtn.addEventListener('click', () => openCreateDialog());
  els.sideMode.addEventListener('change', (ev) => {
    if (ev.target.name === 'sidemode') {
      state.sideMode = ev.target.value;
      for (const opt of els.sideMode.querySelectorAll('.seg-opt')) {
        opt.classList.toggle('is-active', opt.querySelector('input').value === state.sideMode);
      }
      state.doc = null; state.history = null;
      renderSide();
    }
  });
  window.addEventListener('keydown', async (ev) => {
    if (ev.target.tagName === 'TEXTAREA' || ev.target.tagName === 'INPUT' || ev.target.tagName === 'SELECT') return;
    if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === 'z' && !ev.shiftKey) {
      ev.preventDefault(); doUndo();
    } else if ((ev.ctrlKey || ev.metaKey) && (ev.key.toLowerCase() === 'y' || (ev.key.toLowerCase() === 'z' && ev.shiftKey))) {
      ev.preventDefault(); doRedo();
    } else if (ev.key === 'Delete' && state.selected && canEdit()) {
      ev.preventDefault(); openDeleteDialog(state.selected);
    } else if (ev.key.toLowerCase() === 'r' && state.selected && canEdit() && !state.connectFrom) {
      ev.preventDefault(); startConnect(state.selected);
    } else if (ev.key === 'Escape' && state.connectFrom) {
      ev.preventDefault(); cancelConnect();
    }
  });
}

async function applyOp(op) {
  try {
    await invoke('apply_operation', { op });
  } catch (e) {
    toast(String(e));
    // Put the canvas back. A drag moves the node's own style.left/top for
    // feedback while the edges stay where the layout left them, so a refused
    // operation used to leave the node parked away from its own relations
    // until something else forced a render.
    await renderCanvas({ animate: false });
    return false;
  }
  if (!tauri) {
    // mock: no watcher — refresh explicitly
    await refreshSync();
    renderTree();
    await renderCanvas({ animate: false });
    renderSide();
  }
  return true;
}

/** Several operations as one transaction — one undo takes all of them back.
 *  Falls back to one-at-a-time only in the mock harness, which has no batch. */
async function applyOps(ops) {
  if (ops.length === 1) return applyOp(ops[0]);
  try {
    await invoke('apply_operations', { ops });
  } catch (e) {
    toast(String(e));
    await renderCanvas({ animate: false });
    return false;
  }
  if (!tauri) {
    await refreshSync();
    renderTree();
    await renderCanvas({ animate: false });
    renderSide();
  }
  return true;
}

async function doUndo() {
  try { await invoke('undo_op'); } catch (e) { toast(String(e)); }
  if (!tauri) { await refreshSync(); renderTree(); await renderCanvas({ animate: false }); renderSide(); }
}
async function doRedo() {
  try { await invoke('redo_op'); } catch (e) { toast(String(e)); }
  if (!tauri) { await refreshSync(); renderTree(); await renderCanvas({ animate: false }); renderSide(); }
}

function toast(message) {
  document.querySelector('.travel-banner.is-toast')?.remove();
  const b = document.createElement('div');
  b.className = 'travel-banner is-toast';
  b.innerHTML = `<span>${esc(message)}</span>`;
  els.canvas.appendChild(b);
  setTimeout(() => b.remove(), 4000);
}

// --- drag to pin -------------------------------------------------------------
function beginNodeDrag(ev, node, div) {
  if (!canPin() || ev.button !== 0) return;
  const start = { x: ev.clientX, y: ev.clientY };
  const orig = { x: node.x, y: node.y };
  let moved = false;
  const scale = parseFloat(els.camera.style.getPropertyValue('--camera-scale')) || 1;
  const onMove = (mv) => {
    const dx = (mv.clientX - start.x) / scale;
    const dy = (mv.clientY - start.y) / scale;
    if (!moved && Math.hypot(dx, dy) * scale < 4) return; // click tolerance
    moved = true;
    div.classList.add('is-dragging');
    els.camera.classList.add('no-anim');
    div.style.left = orig.x + dx + 'px';
    div.style.top = orig.y + dy + 'px';
  };
  const onUp = async (up) => {
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
    div.classList.remove('is-dragging');
    els.camera.classList.remove('no-anim');
    if (!moved) return;
    const dx = (up.clientX - start.x) / scale;
    const dy = (up.clientY - start.y) / scale;
    // The drop lands in *render* space, which is where the other nodes are,
    // so the free-spot scan happens there too.
    const rx = Math.round((orig.x + dx) / GRID);
    const ry = Math.round((orig.y + dy) / GRID);
    // minimum distance: a drop may not land a node against its neighbours —
    // nudge to the nearest clear grid cell (deterministic ring scan)
    const [fx, fy] = freePinSpot(rx, ry, node);
    // Convert to model space on the way out. Pins may be negative: a diagram
    // has no top-left corner, and clamping them to one turned it into a wall
    // to pile things against. Layout reframes around the content and reports
    // the translation it used, so what reaches the YAML stays in the model's
    // own coordinates however far the drawing grows in any direction.
    const origin = state.layout.origin ?? { x: 0, y: 0 };
    const viewDef = findViewDef(effectiveSnapshot(), state.level, state.scope);
    // resolvePins only reads node ids, and the laid-out nodes carry them.
    const pinned = new Set(Object.keys(resolvePins(viewDef, { nodes: state.layout.nodes })));
    const pin = (id, gx, gy) => ({
      op: 'pin', view: viewDef?.id ?? null, level: state.level, scope: state.scope,
      id, x: gx - Math.round(origin.x / GRID), y: gy - Math.round(origin.y / GRID),
    });
    // Moving one node used to move every other one: a pinned node leaves the
    // ELK graph, so what is left is a *different* graph and gets re-laid out.
    // On this repo's own L3 view, dragging one component moved all eight
    // others by 325-425px each, which is not "the layout settled", it is the
    // diagram rearranging itself under your hands.
    //
    // So the first drag in a view settles the whole view: the dragged node
    // where you put it, everything else exactly where it already is. Nothing
    // appears to move at all, which is the point. Later drags find the others
    // already pinned and send one operation, and it is one transaction, so one
    // undo puts the view back to auto.
    const settle = state.layout.nodes
      .filter((n) => n.id !== node.id && !n.contains && !pinned.has(n.id))
      .map((n) => pin(n.id, Math.round(n.x / GRID), Math.round(n.y / GRID)));
    await applyOps([pin(node.id, fx, fy), ...settle]);
  };
  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp);
}

/** Nearest grid position where `node` keeps one grid unit of clearance from
 * every other node. Scans outward ring by ring; gives up (and honors the raw
 * drop) if nothing frees up within 8 units. */
function freePinSpot(gx, gy, node) {
  const margin = GRID;
  const others = state.layout.nodes.filter((n) => n.id !== node.id);
  const fits = (x, y) =>
    others.every((n) =>
      x * GRID + node.width + margin <= n.x ||
      n.x + n.width + margin <= x * GRID ||
      y * GRID + node.height + margin <= n.y ||
      n.y + n.height + margin <= y * GRID);
  if (fits(gx, gy)) return [gx, gy];
  for (let r = 1; r <= 8; r++) {
    for (let dy = -r; dy <= r; dy++) {
      for (let dx = -r; dx <= r; dx++) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue;
        const x = gx + dx;
        const y = gy + dy;
        // No quadrant guard: render space starts at the content, and a
        // neighbouring cell above or left of the drop is a fine answer.
        if (fits(x, y)) return [x, y];
      }
    }
  }
  return [gx, gy];
}

// --- relations ---------------------------------------------------------------
function startConnect(fromId) {
  state.connectFrom = fromId;
  els.canvas.classList.add('is-connecting');
  els.hint.textContent = 'Click a target element to connect · Esc to cancel';
}

function cancelConnect() {
  state.connectFrom = null;
  els.canvas.classList.remove('is-connecting');
  els.hint.textContent = 'Double-click to dive · Esc to rise';
}

async function finishConnect(toId) {
  const from = state.connectFrom;
  cancelConnect();
  openDialog({
    title: 'New relation',
    body: `<div class="dlg-field"><label for="dlg-label">Label</label><input class="input" id="dlg-label" placeholder="calls"></div>
      <div class="dlg-field"><label for="dlg-proto">Protocol (optional)</label><input class="input" id="dlg-proto" placeholder="HTTPS"></div>
      <p class="text-muted" style="font-size:var(--text-xs)">${esc(from)} → ${esc(toId)}</p>`,
    confirm: 'Create',
    onConfirm: async () => {
      const label = document.getElementById('dlg-label').value.trim() || null;
      const protocol = document.getElementById('dlg-proto').value.trim() || null;
      return applyOp({ op: 'add-relation', from, to: toId, label, protocol });
    },
  });
}

function selectRelation(edge) {
  state.selectedRel = { from: edge.from, to: edge.to, label: edge.label };
  state.selected = null;
  state.doc = null;
  state.help = null; // same as select(): the inspector wins over help
  renderSide();
}

// --- context menus -----------------------------------------------------------
// What belongs here is what is a property of *this diagram* rather than of the
// element: whether the description is drawn (spec §4), and whether the box sits
// where you put it or where the layout engine puts it. Both are decided while
// looking at the picture, which is what right-click is for; the element's own
// fields stay in the inspector.

let ctxMenu = null;

function closeNodeMenu() {
  ctxMenu?.remove();
  ctxMenu = null;
}

function openNodeMenu(ev, id) {
  ev.preventDefault();
  closeNodeMenu();
  select(id);
  const el = anyElement(id);
  // Derived (L4) elements are read-only, and editing is off entirely while
  // the model is stale, in a conflict, or in time-travel.
  if (!el || el.derived || !canEdit()) return;

  const viewDef = findViewDef(effectiveSnapshot(), state.level, state.scope);
  const shown = state.layout?.nodes.find((n) => n.id === id)?.describe ?? false;
  const pins = pinnedIds();
  const items = boxMenuItems({
    canEdit: canEdit(),
    canPin: canPin(),
    kind: el.kind,
    pinned: pins.has(id),
    pinnedCount: pins.size,
    hasDescription: Boolean(el.description),
    described: shown,
    hasSource: Boolean(el.source),
  });
  // menu.js decides what is offered; here is what each one does.
  showMenu(ev, items, {
    connect: () => startConnect(id),
    rename: () => focusInspectorField('insp-name'),
    'add-description': () => focusInspectorField('insp-desc'),
    describe: () => applyOp({
      op: 'show-description',
      view: viewDef?.id ?? null,
      level: state.level,
      scope: state.scope,
      id,
      show: !shown,
    }),
    child: () => openCreateDialog({ parent: id, kinds: CHILD_KINDS[el.kind], into: true }),
    'map-source': () => openSourceDialog(id),
    unpin: () => unpin(id),
    'reset-layout': () => unpin(null),
    delete: () => openDeleteDialog(id),
  });
}

/** Ids pinned in the view on screen — layout's own answer, so a pin naming an
 *  element this view does not show does not count as one. */
function pinnedIds() {
  if (!state.layout) return new Set();
  const viewDef = findViewDef(effectiveSnapshot(), state.level, state.scope);
  return new Set(Object.keys(resolvePins(viewDef, { nodes: state.layout.nodes })));
}

/** Release one pin, or every pin in this view (`id === null`) — one operation
 *  either way, so one undo puts the arrangement back. */
function unpin(id) {
  const viewDef = findViewDef(effectiveSnapshot(), state.level, state.scope);
  return applyOp({
    op: 'unpin',
    view: viewDef?.id ?? null,
    level: state.level,
    scope: state.scope,
    id,
  });
}

/** Right-click on the canvas itself, where there is no box to talk about: the
 *  view's own layout is the only thing left to say something about. */
function openCanvasMenu(ev) {
  if (ev.target.closest('.node')) return;
  closeNodeMenu();
  const items = canvasMenuItems({ canPin: canPin(), pinnedCount: pinnedIds().size });
  showMenu(ev, items, { 'reset-layout': () => unpin(null) });
}

/** Build and place a menu from menu.js items, binding each id to its action. */
function showMenu(ev, items, run) {
  ev.preventDefault();
  if (!items.length) return; // nothing to offer is not an empty menu
  const menu = document.createElement('div');
  menu.className = 'ctx-menu';
  menu.setAttribute('role', 'menu');
  for (const item of items) {
    if (item.sep) {
      const sep = document.createElement('div');
      sep.className = 'ctx-sep';
      sep.setAttribute('role', 'separator');
      menu.appendChild(sep);
      continue;
    }
    const btn = document.createElement('button');
    btn.className = 'ctx-item';
    btn.setAttribute('role', 'menuitem');
    btn.dataset.item = item.id;
    btn.textContent = item.label;
    btn.addEventListener('click', () => { closeNodeMenu(); run[item.id](); });
    menu.appendChild(btn);
  }
  // Arrows walk it. One item answered to Tab and nothing else; a menu with
  // seven has to be navigable by the key people reach for.
  menu.addEventListener('keydown', (kev) => {
    if (kev.key !== 'ArrowDown' && kev.key !== 'ArrowUp') return;
    kev.preventDefault();
    const opts = [...menu.querySelectorAll('.ctx-item')];
    const at = opts.indexOf(document.activeElement);
    const step = kev.key === 'ArrowDown' ? 1 : opts.length - 1;
    opts[(Math.max(at, 0) + step) % opts.length].focus();
  });
  document.body.appendChild(menu);
  // Placed after appending so the real size is known: a menu opened near the
  // right or bottom edge folds back instead of hanging off the window.
  const box = menu.getBoundingClientRect();
  // A menu raised from the keyboard (the menu key, Shift+F10) carries no
  // pointer position, and 0,0 is the corner of the window rather than an
  // answer — so it opens on the thing it is about.
  const from = ev.clientX || ev.clientY
    ? { x: ev.clientX, y: ev.clientY }
    : anchorOf(ev.target);
  const x = Math.min(from.x, window.innerWidth - box.width - 8);
  const y = Math.min(from.y, window.innerHeight - box.height - 8);
  menu.style.left = `${Math.max(8, x)}px`;
  menu.style.top = `${Math.max(8, y)}px`;
  ctxMenu = menu;
  menu.querySelector('.ctx-item')?.focus();
}

/** Top-left of the box a keyboard-raised menu is about, for want of a cursor. */
function anchorOf(target) {
  const box = (target.closest?.('.node') ?? target).getBoundingClientRect();
  return { x: box.left + 8, y: box.top + 8 };
}

/** Put the cursor in one of the inspector's fields — where "rename" and "add a
 *  description" from the canvas have to end up, since a name and a description
 *  are model fields and these are the fields. */
function focusInspectorField(fieldId) {
  if (state.sideMode === 'source') {
    state.sideMode = 'inspect';
    for (const opt of els.sideMode.querySelectorAll('.seg-opt')) {
      opt.classList.toggle('is-active', opt.querySelector('input').value === 'inspect');
    }
  }
  renderSide();
  const input = document.getElementById(fieldId);
  input?.focus();
  input?.select?.();
}

// --- dialogs -----------------------------------------------------------------
function openDialog({ title, body, confirm, cancel = 'Cancel', danger, onConfirm }) {
  closeDialog();
  const wrap = document.createElement('div');
  wrap.className = 'dialog-backdrop';
  wrap.id = 'app-dialog';
  wrap.innerHTML = `<div class="dialog blueprint" role="dialog" aria-modal="true">
    <i class="corner tl"></i><i class="corner tr"></i><i class="corner bl"></i><i class="corner br"></i>
    <span class="dialog-title">${esc(title)}</span>
    <div class="dialog-body">${body}</div>
    <div class="dialog-actions">
      <button class="btn btn-secondary" id="dlg-cancel">${esc(cancel)}</button>
      <button class="btn ${danger ? 'btn-danger' : 'btn-primary'}" id="dlg-ok">${esc(confirm)}</button>
    </div></div>`;
  document.body.appendChild(wrap);
  wrap.addEventListener('click', (ev) => { if (ev.target === wrap) closeDialog(); });
  document.getElementById('dlg-cancel').addEventListener('click', closeDialog);
  document.getElementById('dlg-ok').addEventListener('click', async () => {
    const ok = await onConfirm();
    if (ok !== false) closeDialog();
  });
  wrap.querySelector('input, select')?.focus();
}

function closeDialog() {
  document.getElementById('app-dialog')?.remove();
}

function slugify(name) {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 64);
}

/**
 * The New-element dialog. Called bare from the toolbar, where the altitude
 * decides what may be created and the current scope is the parent; called with
 * a parent and kinds from a box's "add something inside", where the box itself
 * decides both — and then `into` follows the new element down, since it lives
 * one altitude below the one you are looking at.
 */
function openCreateDialog({ parent: intoParent, kinds: intoKinds, into = false } = {}) {
  const level = state.level;
  const kinds = intoKinds ?? (level === 'L1' ? ['system', 'person', 'external']
    : level === 'L2' ? ['container']
    // Deployment (ADR-0018): environments at the overview, and below one
    // either more infrastructure or a container running on it.
    : level === 'LD' ? (state.scope ? ['deployment-node', 'container-instance'] : ['environment'])
    : ['component']);
  const parent = intoParent ?? (level === 'L1' || (level === 'LD' && !state.scope) ? null : state.scope);
  const kindOptions = kinds.map((k) => `<option value="${k}">${k}</option>`).join('');
  openDialog({
    title: 'New element',
    body: `<div class="dlg-field"><label for="dlg-kind">Kind</label><select class="input" id="dlg-kind">${kindOptions}</select></div>
      <div class="dlg-field"><label for="dlg-name">Name</label><input class="input" id="dlg-name" placeholder="Payment Service"></div>
      <div class="dlg-field"><label for="dlg-id">Id — immutable once created (ADR-0003)</label>
        <input class="input" id="dlg-id" style="font-family:var(--font-mono)">
        <span class="dlg-id-preview" id="dlg-id-full"></span></div>`,
    confirm: 'Create',
    onConfirm: async () => {
      const kind = document.getElementById('dlg-kind').value;
      const name = document.getElementById('dlg-name').value.trim();
      const id = document.getElementById('dlg-id').value.trim();
      if (!name || !id) { toast('name and id are required'); return false; }
      const useParent = (kind === 'person' || kind === 'external' || kind === 'system') ? null : parent;
      const ok = await applyOp({ op: 'create', parent: useParent, id, name, kind });
      // Asking for a component inside a container is asking to see inside it:
      // the new element is one altitude down and invisible from here.
      if (ok && into && useParent && state.scope !== useParent) await dive(useParent);
      return ok;
    },
  });
  const nameInput = document.getElementById('dlg-name');
  const idInput = document.getElementById('dlg-id');
  const preview = document.getElementById('dlg-id-full');
  const sync = () => {
    if (!idInput.dataset.touched) idInput.value = slugify(nameInput.value);
    const kind = document.getElementById('dlg-kind').value;
    const useParent = (kind === 'person' || kind === 'external' || kind === 'system') ? null : parent;
    preview.textContent = useParent ? `${useParent}.${idInput.value}` : idInput.value;
  };
  nameInput.addEventListener('input', sync);
  idInput.addEventListener('input', () => { idInput.dataset.touched = '1'; sync(); });
  document.getElementById('dlg-kind').addEventListener('change', sync);
  sync();
}

function openDeleteDialog(id) {
  const snap = effectiveSnapshot();
  const el = snap.elements.find((e) => e.id === id);
  if (!el) return;
  const cascading = snap.relations.filter((r) => r.from === id || r.to === id
    || r.from.startsWith(id + '.') || r.to.startsWith(id + '.'));
  const relList = cascading.length
    ? `<p>Also removes ${cascading.length} relation${cascading.length > 1 ? 's' : ''}:</p><ul>` +
      cascading.map((r) => `<li>${esc(r.from)} → ${esc(r.to)}${r.label ? ` <span class="text-muted">· ${esc(r.label)}</span>` : ''}</li>`).join('') + '</ul>'
    : '<p>No relations reference it.</p>';
  openDialog({
    title: `Delete ${el.name}?`,
    body: `<p><span style="font-family:var(--font-mono)">${esc(id)}</span> and everything inside it will be removed from the model.</p>${relList}`,
    confirm: 'Delete',
    danger: true,
    onConfirm: async () => {
      const ok = await applyOp({ op: 'delete', id });
      if (ok) { state.selected = null; }
      return ok;
    },
  });
}

// --- source panel ------------------------------------------------------------
// CodeMirror (Phase 5, vendored v5): YAML highlighting and inline `.err`
// underlines at the offending line, replacing the v1 plain textarea.
let srcDebounce = null;
let srcCm = null;
async function renderSource() {
  els.sideTitle.textContent = '';
  els.sideBack.hidden = true;
  const files = state.sync?.files ?? [];
  if (!state.srcFile && files.length) state.srcFile = files[0];
  const options = files
    .map((f) => `<option value="${esc(f)}"${f === state.srcFile ? ' selected' : ''}>${esc(f)}</option>`)
    .join('');
  els.sideBody.innerHTML = `<div class="src-wrap">
    <select class="input src-file" id="src-file" aria-label="Workspace file">${options}</select>
    <div class="src-editor" id="src-editor"></div>
    <div class="src-err" id="src-err" hidden></div>
  </div>`;
  const fileSel = document.getElementById('src-file');
  srcCm = CodeMirror(document.getElementById('src-editor'), {
    mode: 'yaml',
    lineNumbers: true,
    lineWrapping: true, // the side panel is 260-480px; unwrapped YAML lines
                         // (descriptions, tech) routinely forced an h-scroll
    indentUnit: 2,
    tabSize: 2,
    screenReaderLabel: 'YAML source editor',
  });
  // a scrollable region must be keyboard-reachable (axe: WCAG 2.1.1); a
  // focused scroller pans with the arrow keys natively
  srcCm.getScrollerElement().setAttribute('tabindex', '0');
  srcCm.getScrollerElement().setAttribute('aria-label', 'Scroll the source file');
  const load = async () => {
    let text = '';
    try {
      text = await invoke('file_text', { rel: state.srcFile });
    } catch (e) { /* keep empty */ }
    srcCm.setValue(text);
    updateSrcStatus();
  };
  fileSel.addEventListener('change', async () => {
    state.srcFile = fileSel.value;
    await load();
  });
  srcCm.on('change', (cm, change) => {
    if (change.origin === 'setValue') return; // programmatic load, not an edit
    state.srcSuppress = true;
    clearTimeout(srcDebounce);
    srcDebounce = setTimeout(async () => {
      try {
        const ok = await invoke('buffer_update', { rel: state.srcFile, text: srcCm.getValue() });
        state.srcSuppress = false;
        if (!tauri) return;
        await refreshSync();
        updateSrcStatus();
        if (ok) {
          renderTree();
          await renderCanvas({ animate: false });
        }
      } catch (e) {
        toast(String(e));
        state.srcSuppress = false;
      }
    }, 200);
  });
  await load();
}

function updateSrcStatus() {
  const stale = state.sync?.stale ?? [];
  els.srcStatus.hidden = state.sideMode !== 'source';
  const isStale = state.srcFile && stale.includes(state.srcFile);
  els.srcStatus.textContent = isStale ? 'error' : 'synced';
  els.srcStatus.className = 'tag ' + (isStale ? 'tag-danger' : 'tag-accent');
  const err = document.getElementById('src-err');
  if (err) {
    const diags = (state.snapshot?.diagnostics ?? [])
      .filter((d) => d.severity === 'error' && d.file === state.srcFile);
    err.hidden = diags.length === 0;
    err.textContent = diags.map((d) => `${d.file}:${d.line} ${d.message}`).join('\n');
    // inline underline at the offending line (spec/sync-engine.md)
    if (srcCm) {
      (srcCm._errLines ?? []).forEach((h) => srcCm.removeLineClass(h, 'wrap', 'src-errline'));
      srcCm._errLines = diags
        .filter((d) => d.line >= 1 && d.line <= srcCm.lineCount())
        .map((d) => srcCm.addLineClass(d.line - 1, 'wrap', 'src-errline'));
    }
  }
}

/** Relation inspector view. */
function renderRelationSide() {
  const r = state.selectedRel;
  els.sideTitle.textContent = '';
  els.sideBack.hidden = true;
  const editable = canEdit();
  els.sideBody.innerHTML = `<div class="insp">
    <span class="insp-kicker">Relation</span>
    <span class="insp-title">${esc(shortName(r.from))} → ${esc(shortName(r.to))}</span>
    <span style="font-family:var(--font-mono);font-size:var(--text-2xs)" class="text-muted">${esc(r.from)} → ${esc(r.to)}</span>
    <div class="insp-section">Label</div>
    <input class="input" id="rel-label" aria-label="Relation label" value="${esc(r.label ?? '')}" ${editable ? '' : 'disabled'}>
    <div class="insp-section">Protocol</div>
    <input class="input" id="rel-proto" aria-label="Relation protocol" value="${esc(relProtocol(r) ?? '')}" ${editable ? '' : 'disabled'}>
    ${editable ? '<div class="insp-section"></div><button class="btn btn-danger" id="rel-delete">Delete relation</button>' : ''}
  </div>`;
  if (!editable) return;
  const commit = (field) => async (ev) => {
    const value = ev.target.value.trim();
    await applyOp({ op: 'set-relation-field', from: r.from, to: r.to, label: r.label ?? null, field, value });
    if (field === 'label') state.selectedRel.label = value || null;
  };
  document.getElementById('rel-label').addEventListener('change', commit('label'));
  document.getElementById('rel-proto').addEventListener('change', commit('protocol'));
  document.getElementById('rel-delete').addEventListener('click', async () => {
    const ok = await applyOp({ op: 'delete-relation', from: r.from, to: r.to, label: r.label ?? null });
    if (ok) { state.selectedRel = null; renderSide(); }
  });
}

function shortName(id) {
  const snap = effectiveSnapshot();
  return snap.elements.find((e) => e.id === id)?.name ?? id;
}

function relProtocol(r) {
  const snap = effectiveSnapshot();
  return snap.relations.find((x) => x.from === r.from && x.to === r.to && x.label === r.label)?.protocol;
}

// ---- share (phase 4) --------------------------------------------------------

function openShareDialog() {
  openDialog({
    title: 'Share',
    body: `<p class="text-muted" style="font-size:var(--text-sm)">Everything is generated locally —
      nothing leaves this machine (ADR-0009).</p>
      <div class="dlg-field"><label><input type="checkbox" id="share-bodies"> Include document bodies
      <span class="text-muted">(they may be more sensitive than structure)</span></label></div>
      <div class="dlg-field" style="flex-direction:row;gap:var(--space-2);margin-top:var(--space-2)">
        <button class="btn btn-secondary" id="share-svg">SVG · current view</button>
        <button class="btn btn-secondary" id="share-png">PNG 2×</button>
      </div>`,
    confirm: 'Interactive HTML',
    onConfirm: async () => {
      const withBodies = document.getElementById('share-bodies').checked;
      try {
        const path = await invoke('export_html', { withBodies });
        toast('Exported: ' + path);
      } catch (e) {
        toast(String(e));
        return false;
      }
    },
  });
  document.getElementById('share-svg').addEventListener('click', async () => {
    try {
      const svg = await buildViewSvg();
      const path = await invoke('save_export', {
        name: exportName('svg'), data: svg, base64: false });
      toast('Exported: ' + path);
      closeDialog();
    } catch (e) { toast(String(e)); }
  });
  document.getElementById('share-png').addEventListener('click', async () => {
    try {
      const png = await buildViewPng(2);
      const path = await invoke('save_export', {
        name: exportName('png'), data: png, base64: true });
      toast('Exported: ' + path);
      closeDialog();
    } catch (e) { toast(String(e)); }
  });
}

function exportName(ext) {
  const scope = state.scope ? state.scope.replace(/\./g, '-') : 'context';
  return `${slugify(state.snapshot.name)}-${scope}-${state.level}.${ext}`;
}

function cssVar(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/** Pure SVG of the current view — colors from the live CSS variables,
 * fonts fetched and inlined, assembly shared with the headless renderer
 * (ui/js/svg.js, spec/export.md). */
async function buildViewSvg() {
  if (!state.layout) throw new Error('nothing to export');
  const colors = {
    bg: cssVar('--canvas-bg') || cssVar('--color-bg'),
    dot: cssVar('--canvas-dot'),
    text: cssVar('--color-text'),
    muted: cssVar('--color-text-muted'),
    border: cssVar('--node-border'),
    fill: cssVar('--node-fill'),
    external: cssVar('--node-external'),
    edge: cssVar('--edge-stroke'),
    key: cssVar('--code-key'),
  };
  const fontCss = await embeddedFontCss().catch(() => '');
  return viewSvg({
    layout: state.layout,
    elements: effectiveSnapshot().elements,
    colors,
    fontCss,
  });
}

let fontCssCache = null;
async function embeddedFontCss() {
  if (fontCssCache) return fontCssCache;
  const faces = [
    ['Barlow', 400, 'ds/assets/fonts/barlow-400-latin.woff2'],
    ['Barlow Condensed', 600, 'ds/assets/fonts/barlow-condensed-600-latin.woff2'],
  ];
  const parts = await Promise.all(faces.map(async ([family, weight, url]) => {
    const buf = await fetch(url).then((r) => r.arrayBuffer());
    const b64 = btoa(String.fromCharCode(...new Uint8Array(buf)));
    return `@font-face{font-family:'${family}';font-weight:${weight};src:url(data:font/woff2;base64,${b64}) format('woff2')}`;
  }));
  fontCssCache = parts.join('');
  return fontCssCache;
}

/** Rasterize the current view via the SVG export. */
async function buildViewPng(scale) {
  const svg = await buildViewSvg();
  const img = new Image();
  const url = URL.createObjectURL(new Blob([svg], { type: 'image/svg+xml' }));
  await new Promise((resolve, reject) => {
    img.onload = resolve;
    img.onerror = () => reject(new Error('SVG rasterization failed'));
    img.src = url;
  });
  const canvas = document.createElement('canvas');
  canvas.width = img.width * scale;
  canvas.height = img.height * scale;
  const ctx = canvas.getContext('2d');
  ctx.scale(scale, scale);
  ctx.drawImage(img, 0, 0);
  URL.revokeObjectURL(url);
  return canvas.toDataURL('image/png').split(',')[1];
}

// ---- util -------------------------------------------------------------------
function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}
