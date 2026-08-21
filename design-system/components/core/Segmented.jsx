// Single-select. Real radios in a named group, so arrow keys, screen readers and
// form semantics all work — the previous version was click-only <label>s with no
// input, which made the L1-L4 level switcher unreachable by keyboard.
export function Segmented({ options, value, onChange, name = 'seg', label }) {
  return <span className="seg" role="radiogroup" aria-label={label}>{options.map((o) => (
    <label key={o} className={'seg-opt' + (o === value ? ' is-active' : '')}>
      <input
        type="radio" name={name} value={o}
        checked={o === value}
        onChange={() => onChange && onChange(o)}
      />
      {o}
    </label>
  ))}</span>;
}
