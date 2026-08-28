// A C4 element. `type` is the C4 kind (person / system / container / component);
// `status` is git/validation state. They are independent axes — a container can be
// added, a person can be invalid — so they are separate props, not one enum.
//
// Type is encoded by geometry and status by colour+glyph, never colour alone:
// a node must stay legible in greyscale and at L1, where it is ~90px wide.

const BADGE = { added: '+', removed: '−', changed: '~', conflict: '!', invalid: '!' };
const BADGE_LABEL = {
  added: 'Added in this change', removed: 'Removed in this change',
  changed: 'Modified in this change', conflict: 'Merge conflict', invalid: 'Invalid — see model errors',
};

export function DiagramNode({
  kicker, title, meta, description, type = 'system', status,
  active, external, x, y, width, style, onSelect, ...rest
}) {
  const cls = [
    'node', 'is-' + type,
    external ? 'is-external' : '',
    active ? 'is-active' : '',
    status ? 'is-' + status : '',
  ].filter(Boolean).join(' ');

  return <div
    className={cls}
    style={{ left: x, top: y, width, ...style }}
    // The core object of the app is keyboard-reachable and announces its state.
    tabIndex={0}
    role="button"
    aria-pressed={active ? true : undefined}
    onClick={onSelect}
    onKeyDown={onSelect && (e => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onSelect(e); }
    })}
    {...rest}
  >
    {status && <span className="node-badge" title={BADGE_LABEL[status]}>
      <span aria-hidden="true">{BADGE[status]}</span>
      <span className="sr-only">{BADGE_LABEL[status]}</span>
    </span>}
    {kicker && <span className="node-kicker">{kicker}</span>}
    <span className="node-title">{title}</span>
    {meta && <span className="node-meta">{meta}</span>}
    {description && <span className="node-desc">{description}</span>}
  </div>;
}
