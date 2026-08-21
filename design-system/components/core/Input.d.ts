export interface InputProps {
  /** Renders a .field wrapper with the label above, correctly associated by id */
  label?: string;
  multiline?: boolean;
  /** Validation message. Sets .is-invalid, aria-invalid and aria-describedby. */
  error?: string;
  id?: string;
  placeholder?: string;
  value?: string;
  onChange?: (e: any) => void;
}
