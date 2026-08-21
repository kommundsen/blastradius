export interface DiagramNodeProps {
  /** Uppercase micro-label, e.g. "Container · Go" */
  kicker?: string;
  title: string;
  /** Secondary line, e.g. "8 components" */
  meta?: string;
  /** C4 element kind. Encoded by geometry, never by colour. */
  type?: 'person' | 'system' | 'container' | 'component';
  /**
   * Git or validation state. Encoded by colour AND a glyph badge with an
   * sr-only label, so it survives greyscale and colour-blindness.
   */
  status?: 'added' | 'removed' | 'changed' | 'conflict' | 'invalid';
  /** Outside the model boundary — dashed, no shadow */
  external?: boolean;
  /** Selected */
  active?: boolean;
  /** Position and size in camera coordinates */
  x?: number;
  y?: number;
  width?: number;
  style?: React.CSSProperties;
  /** Makes the node a real button: focusable, Enter/Space activated */
  onSelect?: (e: any) => void;
}
