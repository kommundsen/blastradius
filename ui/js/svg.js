// Pure SVG assembly of a laid-out view (spec/export.md): rects + text +
// paths, no foreignObject, so the file opens in design tools standalone.
// Shared by the app's Share menu (which supplies live CSS-variable colors
// and fetched fonts) and tools/render-views.mjs (which resolves the same
// design tokens headlessly) — one renderer, two attachment points, in the
// ADR-0005 tradition.

import { GRID } from './layout.js';
import { edgeLabelLines, kicker } from './labels.js';

export { kicker };

export function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
}

export function childCount(el, elements) {
  const children = elements.filter((e) => e.parent === el.id);
  const kids = children.length;
  if (!kids) return null;
  // Deployment nodes hold nodes, instances, or both — say which, rather than
  // calling instances "nodes".
  const deployment =
    el.kind === 'environment' || el.kind === 'deployment-node'
      ? children.every((c) => c.kind === 'container-instance')
        ? 'instance'
        : 'node'
      : null;
  const noun = el.derived ? 'member' : deployment ?? (el.kind === 'system' ? 'container' : 'component');
  return `${kids} ${noun}${kids > 1 ? 's' : ''}`;
}

/**
 * Render one laid-out view to an SVG string.
 * - layout: {nodes, edges, width, height} from layout.layoutView
 * - elements: the snapshot's full element list (for kickers and counts)
 * - colors: {bg, dot, text, muted, border, fill, external, edge, key}
 * - fontCss: @font-face rules with data-URI sources ('' renders with fallbacks)
 */
export function viewSvg({ layout, elements, colors, fontCss = '', footer = true }) {
  const l = layout;
  const elById = new Map(elements.map((e) => [e.id, e]));
  const pad = GRID;
  const W = Math.ceil(l.width + pad * 2);
  const H = Math.ceil(l.height + pad * 2);

  let out = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">\n`;
  out += `<style>${fontCss}
    text{font-family:'Barlow',sans-serif}
    .t{font-family:'Barlow Condensed',sans-serif;font-weight:600;fill:${colors.text};font-size:15px;letter-spacing:.02em}
    .k{fill:${colors.key};font-size:9px;letter-spacing:1px}
    .m{fill:${colors.muted};font-size:10px}
    .lbl{fill:${colors.muted};font-size:10px;paint-order:stroke fill;stroke:${colors.bg};stroke-width:4px;stroke-linejoin:round}
  </style>\n`;
  out += `<rect width="${W}" height="${H}" fill="${colors.bg}"/>\n`;
  out += `<defs><pattern id="grid" width="${GRID}" height="${GRID}" patternUnits="userSpaceOnUse">
    <circle cx="1" cy="1" r="1" fill="${colors.dot}"/></pattern>
  <marker id="arr" viewBox="0 0 10 10" refX="9.5" refY="5" markerWidth="8" markerHeight="8"
    orient="auto-start-reverse"><path d="M1.5,1.5 L9,5 L1.5,8.5" fill="none" stroke="${colors.edge}"/></marker></defs>\n`;
  out += `<rect width="${W}" height="${H}" fill="url(#grid)"/>\n`;
  out += `<g transform="translate(${pad},${pad})">\n`;

  for (const e of l.edges) {
    const d = e.points.map((p, i) => (i ? 'L' : 'M') + p.x + ',' + p.y).join(' ');
    const dash = !e.exact ? ' stroke-dasharray="4 3"' : '';
    const marker = e.direction === 'none' ? '' : ' marker-end="url(#arr)"' +
      (e.direction === 'both' ? ' marker-start="url(#arr)"' : '');
    out += `<path d="${d}" fill="none" stroke="${colors.edge}"${dash}${marker}/>\n`;
    // Was `e.label ?? e.protocol`: an exported diagram showed one or the
    // other and never both, so every relation carrying a protocol lost it on
    // the way out while the canvas kept showing it.
    const lines = edgeLabelLines(e);
    for (const [i, line] of lines.entries()) {
      const y = e.labelAt.y - (lines.length - 1 - i) * 12;
      out += `<text class="lbl" x="${e.labelAt.x}" y="${y}" text-anchor="middle">${esc(line)}</text>\n`;
    }
  }

  for (const n of l.nodes) {
    const el = elById.get(n.id);
    const external = el.external;
    const stroke = external ? colors.external : colors.border;
    const fill = external ? 'none' : colors.fill;
    const dash = external ? ' stroke-dasharray="5 4"' : '';
    out += `<rect x="${n.x}" y="${n.y}" width="${n.width}" height="${n.height}" fill="${fill}" stroke="${stroke}"${dash}/>\n`;
    if (el.kind === 'person') {
      out += `<circle cx="${n.x + n.width / 2}" cy="${n.y - 1}" r="5" fill="${fill === 'none' ? colors.bg : fill}" stroke="${stroke}"/>\n`;
    }
    const kick = (kicker(el) || '').toUpperCase();
    out += `<text class="k" x="${n.x + 10}" y="${n.y + 18}">${esc(kick)}</text>\n`;
    out += `<text class="t" x="${n.x + 10}" y="${n.y + 36}">${esc(el.name.toUpperCase())}</text>\n`;
    const meta = childCount(el, elements);
    if (meta) out += `<text class="m" x="${n.x + 10}" y="${n.y + 52}">${esc(meta)}</text>\n`;
  }
  out += '</g>\n';
  if (footer) {
    out += '<text class="m" x="' + (W - 8) + '" y="' + (H - 8) +
      '" text-anchor="end">made with Blastradius</text>\n';
  }
  out += '</svg>\n';
  return out;
}
