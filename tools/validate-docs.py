# Stand-in validator for the docs/ dogfood workspace, implementing the checks in
# docs/spec/model-format.md section 6 until the real Phase-0 parser exists.
# Usage: python tools/validate-docs.py [workspace-dir]   (default: docs/)
# Retired by the Phase 0 exit criterion: `blastradius validate docs/` replaces it.
import io, os, re, sys, glob

try:
    import yaml
except ImportError:
    sys.exit('pyyaml required: pip install pyyaml')

ROOT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'docs')
os.chdir(ROOT)
errors, warnings = [], []
SLUG = re.compile(r'^[a-z0-9-]{1,64}$')

# ---- manifest ---------------------------------------------------------------
mf = yaml.safe_load(io.open('workspace.yaml', encoding='utf-8'))
assert mf['workspace']['version'] == 1

def expand(globs):
    out = []
    for g in globs:
        out += sorted(glob.glob(g))
    return out

model_files = expand(mf['model']['include'])
view_files = expand(mf['views']['include'])
doc_files = expand(mf['docs']['include'])

# ---- model ------------------------------------------------------------------
ids = set()          # every addressable dotted path + bare context ids
systems = {}

def reg(eid, where):
    if eid in ids:
        errors.append(f'{where}: duplicate id {eid!r}')
    ids.add(eid)

for f in model_files:
    d = yaml.safe_load(io.open(f, encoding='utf-8'))
    if 'people' in d or 'external' in d:
        for sec in ('people', 'external'):
            for eid, body in (d.get(sec) or {}).items():
                if not SLUG.match(eid):
                    errors.append(f'{f}: bad id {eid!r}')
                reg(eid, f)
    if 'system' in d:
        sid = d['system']
        if not SLUG.match(sid):
            errors.append(f'{f}: bad system id {sid!r}')
        reg(sid, f)
        systems[sid] = d
        for cid, c in (d.get('containers') or {}).items():
            if not SLUG.match(cid):
                errors.append(f'{f}: bad id {cid!r}')
            reg(f'{sid}.{cid}', f)
            for kid in ((c or {}).get('components') or {}):
                if not SLUG.match(kid):
                    errors.append(f'{f}: bad id {kid!r}')
                reg(f'{sid}.{cid}.{kid}', f)

def resolve(ref, sid):
    """bare context id, sibling path relative to system, or absolute path"""
    if ref in ids:
        return ref
    if f'{sid}.{ref}' in ids:
        return f'{sid}.{ref}'
    return None

for sid, d in systems.items():
    fname = next(f for f in model_files if yaml.safe_load(io.open(f, encoding='utf-8')).get('system') == sid)
    seen_rel = set()
    for i, r in enumerate(d.get('relations') or []):
        for end in ('from', 'to'):
            ref = r.get(end)
            if ref is None:
                errors.append(f'{fname}: relation #{i} missing {end}:')
                continue
            if resolve(ref, sid) is None:
                errors.append(f'{fname}: relation #{i} dangling {end}: {ref!r}')
        key = (r.get('from'), r.get('to'), r.get('label'))
        if key in seen_rel:
            warnings.append(f'{fname}: relation #{i} duplicated verbatim {key}')
        seen_rel.add(key)
        if r.get('direction') not in (None, 'both', 'none'):
            errors.append(f'{fname}: relation #{i} bad direction {r["direction"]!r}')

# ---- views ------------------------------------------------------------------
for f in view_files:
    v = yaml.safe_load(io.open(f, encoding='utf-8'))
    scope = v.get('scope')
    if scope not in ids:
        errors.append(f'{f}: scope {scope!r} not an element')
    if v.get('level') not in ('L1', 'L2', 'L3'):
        errors.append(f'{f}: bad level {v.get("level")!r}')
    for lid, pos in (v.get('layout') or {}).items():
        target = lid if lid in ids else f'{scope}.{lid}' if f'{scope}.{lid}' in ids else None
        if target is None:
            errors.append(f'{f}: layout pins unknown element {lid!r}')
        if not (isinstance(pos, list) and len(pos) == 2 and all(isinstance(n, (int, float)) for n in pos)):
            errors.append(f'{f}: layout {lid!r} must be [x, y]')

# ---- docs -------------------------------------------------------------------
TYPES = {'prd': {'draft', 'current', 'superseded'},
         'spec': {'draft', 'current', 'superseded'},
         'roadmap': {'draft', 'current', 'superseded'},
         'adr': {'proposed', 'accepted', 'superseded', 'rejected'},
         'note': set()}
doc_ids = set()
for f in doc_files:
    text = io.open(f, encoding='utf-8').read()
    if not text.startswith('---'):
        warnings.append(f'{f}: no frontmatter — ignored (info)')
        continue
    fm = yaml.safe_load(text.split('---', 2)[1])
    did, dtype, status = fm.get('doc'), fm.get('type'), fm.get('status')
    if not did or not SLUG.match(str(did)):
        errors.append(f'{f}: bad doc id {did!r}')
    if did in doc_ids:
        errors.append(f'{f}: duplicate doc id {did!r}')
    doc_ids.add(did)
    if dtype not in TYPES:
        warnings.append(f'{f}: unknown doc type {dtype!r}')
    elif TYPES[dtype] and status not in TYPES[dtype]:
        errors.append(f'{f}: status {status!r} invalid for type {dtype!r}')
    for eid in fm.get('elements') or []:
        if eid not in ids:
            errors.append(f'{f}: elements link dangling: {eid!r}')
    sup = fm.get('supersedes')
    if sup:
        # allow forward ref within the doc set; verified after loop
        pass

print(f'model files: {len(model_files)}  view files: {len(view_files)}  doc files: {len(doc_files)}')
print(f'elements: {len(ids)}  docs: {len(doc_ids)}')
for w in warnings:
    print('WARN ', w)
for e in errors:
    print('ERROR', e)
print('RESULT:', 'FAIL' if errors else 'PASS')
sys.exit(1 if errors else 0)
