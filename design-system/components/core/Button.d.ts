export interface ButtonProps {
  /** 'primary' | 'secondary' | 'ghost' | 'danger' — primary is the one solid accent object */
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  /** Optional leading icon (Lucide, stroke 1.5) */
  icon?: React.ReactNode;
  /** Full-width */
  block?: boolean;
  disabled?: boolean;
  onClick?: () => void;
  children?: React.ReactNode;
}

/** Grouped independent actions (zoom, undo/redo). Not a value selector — use Segmented for that. */
export interface ButtonGroupProps {
  children?: React.ReactNode;
}
