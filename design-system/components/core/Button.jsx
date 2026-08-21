export function Button({ variant = 'secondary', icon, block, children, ...rest }) {
  const cls = ['btn', 'btn-' + variant, block ? 'btn-block' : ''].filter(Boolean).join(' ');
  return <button type="button" className={cls} {...rest}>{icon}{children}</button>;
}

// Grouped actions — zoom −/100%/+, alignment, undo/redo. NOT Segmented: these are
// three independent commands, not one selected value.
export function ButtonGroup({ children, ...rest }) {
  return <span className="btn-group" {...rest}>{children}</span>;
}
