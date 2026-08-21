export interface Point { x: number; y: number }

export interface EdgeProps {
  /** Start point, in camera coordinates */
  from: Point;
  /** End point — the arrowhead lands here */
  to: Point;
  /** Explicit route. Overrides `routing` when present. */
  waypoints?: Point[];
  /** 'straight' | 'orthogonal' (elbow through the horizontal midpoint) */
  routing?: 'straight' | 'orthogonal';
  /**
   * C4 relationships are directed. 'forward' draws one arrowhead at `to`;
   * 'both' adds one at `from`; 'none' is for association lines only.
   */
  direction?: 'forward' | 'both' | 'none';
  /** Dashed and dimmed — implied or inferred relations */
  secondary?: boolean;
  /** Selected */
  active?: boolean;
  /** Git diff state; pairs with the same states on DiagramNode */
  status?: 'added' | 'removed';
  /** Protocol label, e.g. "JSON / HTTPS". Strokes out the grid behind itself. */
  label?: string;
  /** Perpendicular nudge of the label off the line, px. Default -4. */
  labelOffset?: number;
  /** Enables the fat invisible hit-path. Without it the edge is not clickable. */
  onSelect?: (e: any) => void;
}
