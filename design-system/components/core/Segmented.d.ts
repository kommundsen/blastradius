export interface SegmentedProps {
  /** Short labels; the level switcher is Segmented with ['L1','L2','L3','L4'] */
  options: string[];
  value?: string;
  onChange?: (value: string) => void;
  /** Radio group name — required when more than one Segmented is on screen */
  name?: string;
  /** Accessible name for the group, e.g. "Detail level" */
  label?: string;
}
