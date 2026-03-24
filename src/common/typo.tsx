import type { ReactNode } from "react";
import { Props } from "../types";
import cn from "../utils/cn";
import s from "./typo.module.css";

export function H1({
  children = "",
  className = "",
  ...args
}: Props<HTMLHeadingElement>) {
  return (
    <p className={cn(s.h1, className)} {...args}>
      {children}
    </p>
  );
}

export function H4({
  children = "",
  className = "",
  ...args
}: Props<HTMLHeadingElement>) {
  return (
    <p className={cn(s.h4, className)} {...args}>
      {children}
    </p>
  );
}

export function P1({
  children = "",
  className = "",
  ...args
}: Props<HTMLParagraphElement>) {
  return (
    <p className={cn(s.p1, className)} {...args}>
      {children}
    </p>
  );
}

export function P3({
  children = "",
  className = "",
}: {
  children?: ReactNode;
  className?: string;
}) {
  return <p className={cn(s.p3, className)}>{children}</p>;
}
