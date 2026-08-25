// How technology is written on a diagram, in one place.
//
// C4 renders technology in square brackets — `[Container: Rust]` on an
// element, `[JSON/HTTPS]` under a relation's label — and Blastradius follows
// that rather than inventing a notation (owner decision, docs/roadmap.md).
//
// One module because four surfaces draw these strings and they used to
// disagree: the canvas showed `calls · JSON/HTTPS`, the SVG export showed
// `calls` *or* `JSON/HTTPS` and never both, the exported viewer had its own
// copy, and layout measured a fifth string when deciding where a label fits.

const KINDS = {
  person: 'Person',
  system: 'Software system',
  container: 'Container',
  component: 'Component',
  external: 'External system',
  environment: 'Environment',
  'deployment-node': 'Deployment node',
  'container-instance': 'Container instance',
};

const DERIVED_KINDS = {
  module: 'Module',
  namespace: 'Namespace',
  class: 'Class',
  interface: 'Interface',
  record: 'Record',
  enum: 'Enum',
  dependency: 'Dependency',
};

/** An element's type line: `[Container: Rust]`, `[Person]`, `[Module: derived]`. */
export function kicker(el) {
  // Dependency rollups are derived, but "external" reads truer than "derived"
  // for something that lives outside the mapped source tree entirely.
  if (el.kind === 'dependency') return `[${DERIVED_KINDS.dependency}: external]`;
  if (el.derived) return `[${DERIVED_KINDS[el.kind] ?? el.kind}: derived]`;
  const kind = KINDS[el.kind] ?? el.kind;
  const label = el.external && el.kind === 'system' ? 'External system' : kind;
  return el.tech ? `[${label}: ${el.tech}]` : `[${label}]`;
}

/**
 * A relation's label, as the lines a diagram draws: the label, then the
 * technology in brackets beneath it.
 *
 * Returns `[]` for a relation carrying neither, so callers can skip it.
 */
export function edgeLabelLines(e) {
  const lines = [];
  if (e.label) lines.push(e.label);
  if (e.protocol) lines.push(`[${e.protocol}]`);
  return lines;
}

/** The same, on one line — for lists and inspectors, where a diagram's
 *  two-line stack would just be a wrapped string. */
export function edgeLabelText(e) {
  return edgeLabelLines(e).join(' ');
}
