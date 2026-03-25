import { AnchorHTMLAttributes } from "react";

export function getGithubProps(
  repo: string,
): AnchorHTMLAttributes<HTMLAnchorElement> {
  return {
    href: "https://github.com/" + repo,
    target: "_blank",
    rel: "noreferrer",
  };
}
