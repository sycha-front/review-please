import type { ReactNode } from "react";
import s from "./tooltip.module.css";
import { P3 } from "./typo";

export default function Tooltip({
  children,
  message,
}: {
  children: ReactNode;
  message: string | null;
}) {
  return (
    <div className={s.tooltip}>
      {children}
      {message && (
        <span className={s.message}>
          <P3>{message}</P3>
        </span>
      )}
    </div>
  );
}
