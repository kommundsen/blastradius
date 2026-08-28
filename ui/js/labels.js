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
 * Instance multiplicity (ADR-0018 follow-up): `x3` for three of the same
 * thing. A field rather than repeated elements, so it is a suffix on one box
 * rather than three boxes and three copies of every relation.
 *
 * Returns null when there is nothing to say — one of something is the
 * default, and writing `x1` on it is noise.
 */
export function multiplicity(el) {
  return el.replicas && el.replicas > 1 ? `×${el.replicas}` : null;
}

/** Description text at 11px, the size `.node-desc` renders it at. */
const DESC_LINE_HEIGHT = 15;
const DESC_CHAR_WIDTH = 5.1;
const DESC_PAD = 20; // `.node` padding, both sides

/**
 * How an element's description wraps inside a box `width` px wide.
 *
 * Only the surfaces with no DOM need this: the SVG export draws these lines
 * literally, and layout reserves their height. The canvas measures the real
 * markup instead (app.js `measureNodes`), which is always truer than an
 * estimate and is why the estimate only has to be close.
 */
export function descriptionLines(text, width) {
  const max = Math.max(8, Math.floor((width - DESC_PAD) / DESC_CHAR_WIDTH));
  const lines = [];
  let line = '';
  for (const word of String(text).split(/\s+/).filter(Boolean)) {
    const next = line ? `${line} ${word}` : word;
    if (next.length > max && line) {
      lines.push(line);
      line = word;
    } else {
      line = next;
    }
  }
  if (line) lines.push(line);
  return lines;
}

/** Height `.node-desc` adds to a box: the rule's own 6px margin, 5px padding
 *  and 1px rule, plus a line box per wrapped line. */
export function descriptionHeight(text, width) {
  return 12 + descriptionLines(text, width).length * DESC_LINE_HEIGHT;
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
