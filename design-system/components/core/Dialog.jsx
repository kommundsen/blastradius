let uid = 0;

export function Dialog({ title, actions, children, onDismiss }) {
  const titleId = 'dlg' + (++uid);
  return <div className="dialog-backdrop" onClick={onDismiss}>
    <div
      className="dialog blueprint"
      role="dialog" aria-modal="true"
      aria-labelledby={title ? titleId : undefined}
      onClick={e => e.stopPropagation()}
    >
      <i className="corner tl"></i><i className="corner tr"></i><i className="corner bl"></i><i className="corner br"></i>
      {title && <span className="dialog-title" id={titleId}>{title}</span>}
      <div className="dialog-body">{children}</div>
      {actions && <div className="dialog-actions">{actions}</div>}
    </div>
  </div>;
}
