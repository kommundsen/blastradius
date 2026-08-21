export interface DialogProps {
  title?: string;
  /** Buttons for the actions row */
  actions?: React.ReactNode;
  /** Called on backdrop click. Owners should also wire Escape. */
  onDismiss?: () => void;
  children?: React.ReactNode;
}
