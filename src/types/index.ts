import { ReactNode } from "react";

export type Props<T> = {
  children?: ReactNode;
  className?: string;
  args?: T;
};
