export interface CardProps {
  kicker?: string;
  title?: string;
  meta?: React.ReactNode;
  /** Registration-mark frame; default true */
  blueprint?: boolean;
  children?: React.ReactNode;
}