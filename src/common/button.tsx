import type { ButtonHTMLAttributes } from "react";
import cn from "../utils/cn";
import s from "./button.module.css";

export default function Button({
  children,
  color = "",
  className = "",
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      className={cn(
        s.button,
        color === "secondary" ? s.secondaryButton : "",
        className,
      )}
      {...rest}
    >
      {children}
    </button>
  );
}
