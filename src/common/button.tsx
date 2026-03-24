import type { ButtonHTMLAttributes } from "react";
import cn from "../utils/cn";
import s from "./button.module.css";

export default function Button({
  children,
  className = "",
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button className={cn(s.button, className)} {...rest}>
      {children}
    </button>
  );
}
