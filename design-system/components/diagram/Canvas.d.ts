export interface CanvasProps {
  /** Force a theme on this subtree. Omit to inherit the app/OS theme. */
  theme?: 'light' | 'dark';
  /** Camera scale. Also published as --camera-scale for .screen-space children. */
  scale?: number;
  /** Camera translation in px, applied before scale. */
  x?: number;
  y?: number;
  /** Screen-space chrome: zoom control, hints, minimap. Never scales. */
  overlay?: React.ReactNode;
  style?: React.CSSProperties;
  /** The drawing: one EdgeLayer plus DiagramNodes. Scales with the camera. */
  children?: React.ReactNode;
}
