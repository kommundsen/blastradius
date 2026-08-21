let uid = 0;

export function Input({ label, multiline, error, id, ...rest }) {
  const fieldId = id || 'in' + (++uid);
  const errId = fieldId + '-err';
  const cls = 'input' + (error ? ' is-invalid' : '');
  const control = multiline
    ? <textarea id={fieldId} className={cls} aria-invalid={error ? true : undefined}
        aria-describedby={error ? errId : undefined} {...rest} />
    : <input id={fieldId} className={cls} aria-invalid={error ? true : undefined}
        aria-describedby={error ? errId : undefined} {...rest} />;

  if (!label && !error) return control;
  return <div className="field">
    {label && <label htmlFor={fieldId}>{label}</label>}
    {control}
    {error && <span id={errId} className="field-error" role="alert">{error}</span>}
  </div>;
}
