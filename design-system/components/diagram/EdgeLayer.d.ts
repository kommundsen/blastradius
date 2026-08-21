/**
 * The single SVG surface all Edges render into. Carries the #br-arrow marker in its
 * own <defs> — markers do not resolve across document fragments, so there must be
 * exactly one EdgeLayer per Canvas and every Edge must be inside it.
 */
export interface EdgeLayerProps {
  children?: React.ReactNode;
}
