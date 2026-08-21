export function Card({ kicker, title, meta, blueprint = true, children, ...rest }) {
  return <div className={blueprint ? 'card blueprint' : 'card'} {...rest}>
    {blueprint && <><i className="corner tl"></i><i className="corner tr"></i><i className="corner bl"></i><i className="corner br"></i></>}
    {kicker && <span className="card-kicker">{kicker}</span>}
    {title && <span className="card-title">{title}</span>}
    {children && <p className="card-body">{children}</p>}
    {meta && <span className="card-meta">{meta}</span>}
  </div>;
}