// The mock sync engine's semantics, as a module the tests can hold.
//
// The e2e suite runs against a hand-written mock of the sync engine (ADR-0011)
// and every operation's meaning is mirrored into it *by hand*: unpin removing
// the `layout:` key, `external: false` clearing rather than setting,
// `replicas: 1` clearing, a view file being authored when none exists. Each is
// a place the suite can agree with itself while disagreeing with the engine,
// which is exactly what 0.8.0's settle test did for a whole release.
//
// So the mirror lives here rather than inside app.js, and
// `ui/tests/contract.test.mjs` runs the same operation list through it that
// `crates/blastradius-core/tests/contract.rs` runs through the real engine,
// comparing the two snapshots field by field. A divergence fails a build
// instead of hiding one.

/** Apply one operation to a snapshot, in place. Throws the way the engine
 *  refuses: an unknown element, a duplicate id. */
export function applyMockOperation(snap, op) {
  if (op.op === 'rename') {
    const el = snap.elements.find((e) => e.id === op.id);
    if (!el) throw new Error('unknown element');
    el.name = op.name;
  } else if (op.op === 'create') {
    const id = op.parent ? op.parent + '.' + op.id : op.id;
    if (snap.elements.some((e) => e.id === id)) throw new Error('id exists');
    snap.elements.push({ id, kind: op.kind, parent: op.parent ?? undefined, name: op.name });
    // The engine keys elements by id (a BTreeMap), so a new one sorts into
    // place rather than landing at the end.
    snap.elements.sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  } else if (op.op === 'delete') {
    snap.elements = snap.elements.filter((e) => e.id !== op.id && !e.id.startsWith(op.id + '.'));
    snap.relations = snap.relations.filter((r) => r.from !== op.id && r.to !== op.id);
  } else if (op.op === 'add-relation') {
    // An absent label or protocol is *absent*: the engine skips serialising
    // None, so writing null here would be a field the engine never emits.
    const rel = { from: op.from, to: op.to, direction: 'forward' };
    if (op.label != null) rel.label = op.label;
    if (op.protocol != null) rel.protocol = op.protocol;
    snap.relations.push(rel);
    snap.drift = (snap.drift ?? []).filter((d) =>
      !(d.kind === 'undeclared' && d.from === op.from && d.to === op.to));
  } else if (op.op === 'delete-relation') {
    snap.relations = snap.relations.filter((r) =>
      !(r.from === op.from && r.to === op.to && (op.label == null || r.label === op.label)));
    snap.drift = (snap.drift ?? []).filter((d) =>
      !(d.kind === 'unbacked' && d.from === op.from && d.to === op.to));
  } else if (op.op === 'reverse-relation') {
    // Mirrors compute_reverse_relation: the two endpoints swap and nothing
    // else about the relation is touched — which is the whole difference from
    // the delete-and-re-add this replaced.
    const r = snap.relations.find((r) => r.from === op.from && r.to === op.to
      && (op.label == null || r.label === op.label));
    if (!r) throw new Error('unknown relation');
    [r.from, r.to] = [r.to, r.from];
  } else if (op.op === 'set-relation-field') {
    // Label disambiguates a pair with several relations, exactly as
    // `find_relation` does — matching on the pair alone would edit whichever
    // one happened to be first.
    const r = snap.relations.find((r) => r.from === op.from && r.to === op.to
      && (op.label == null || r.label === op.label));
    if (!r) throw new Error('unknown relation');
    // Mirrors compute_set_relation_field: forward is the absence of
    // `direction:` and the snapshot spells that absence "forward", an emptied
    // label or protocol removes the key, and an endpoint is spliced in place
    // so everything else on the relation stays exactly as it was.
    if (op.field === 'direction') {
      r.direction = op.value === '' ? 'forward' : op.value;
    } else if (op.field === 'from' || op.field === 'to') {
      r[op.field] = op.value;
    } else if (op.value === '') {
      delete r[op.field];
    } else {
      r[op.field] = op.value;
    }
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
      const id = (op.scope ?? op.level).split('.').pop() + '-' + op.level.toLowerCase();
      v = { id, file: `views/${id}.yaml`,
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
  return snap;
}

/** The undo label the engine gives this operation. */
export function mockOpLabel(op) {
  return op.op + ' ' + (op.id ?? (op.from ? op.from + ' -> ' + op.to : ''));
}
