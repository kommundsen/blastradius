// A C4 relationship. Directed by default, because "Web App uses API" is not the same
// statement as "API uses Web App" — the previous rotated-<i> edge could not say either.
//
// Geometry is given as real points, not (x, length, angle): the router owns the shape,
// the component owns the drawing. routing="orthogonal" elbows through the midpoint,
// which is what C4 container diagrams normally want.

function buildPoints(from, to, waypoints, routing) {
  if (waypoints && waypoints.length) return [from, ...waypoints, to];
  if (routing === 'orthogonal') {
    const mx = (from.x + to.x) / 2;
    return [from, { x: mx, y: from.y }, { x: mx, y: to.y }, to];
  }
  return [from, to];
}

// Midpoint by arc length, so the label sits on the visual middle of an elbowed run.
function midpoint(pts) {
  const seg = [];
  let total = 0;
  for (let i = 1; i < pts.length; i++) {
    const len = Math.hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
    seg.push(len); total += len;
  }
  let want = total / 2;
  for (let i = 0; i < seg.length; i++) {
    if (want <= seg[i]) {
      const t = seg[i] ? want / seg[i] : 0;
      return { x: pts[i].x + (pts[i + 1].x - pts[i].x) * t,
               y: pts[i].y + (pts[i + 1].y - pts[i].y) * t };
    }
    want -= seg[i];
  }
  return pts[pts.length - 1];
}

export function Edge({
  from, to, waypoints, routing = 'straight',
  direction = 'forward', secondary, active, status,
  label, labelOffset = -4, onSelect,
}) {
  const pts = buildPoints(from, to, waypoints, routing);
  const d = pts.map((p, i) => (i ? 'L' : 'M') + p.x + ',' + p.y).join(' ');
  const mid = label ? midpoint(pts) : null;
  const cls = [
    'edge',
    direction === 'both' ? 'is-bidirectional' : '',
    direction === 'none' ? 'is-undirected' : '',
    secondary ? 'is-secondary' : '',
    active ? 'is-active' : '',
    status ? 'is-' + status : '',
  ].filter(Boolean).join(' ');

  return <g>
    {/* Fat transparent twin first — a 1px stroke is not a pointer target. */}
    {onSelect && <path className="edge-hit" d={d} onClick={onSelect} />}
    <path className={cls} d={d} />
    {label && <text className="edge-label" x={mid.x} y={mid.y + labelOffset}
      textAnchor="middle">{label}</text>}
  </g>;
}
