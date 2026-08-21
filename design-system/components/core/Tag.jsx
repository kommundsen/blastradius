export function Tag({ variant = 'neutral', children, ...rest }) {
  return <span className={'tag tag-' + variant} {...rest}>{children}</span>;
}