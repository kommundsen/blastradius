export interface TagProps {
  /**
   * 'accent' | 'neutral' | 'outline' are decorative.
   * 'danger' | 'warning' | 'success' are semantic and render a leading glyph,
   * so the state survives greyscale and colour-blindness.
   */
  variant?: 'accent' | 'neutral' | 'outline' | 'danger' | 'warning' | 'success';
  children?: React.ReactNode;
}
