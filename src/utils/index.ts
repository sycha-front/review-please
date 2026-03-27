import { AnchorHTMLAttributes } from "react";

export function getGithubProps(
  repo: string,
): AnchorHTMLAttributes<HTMLAnchorElement> {
  if (repo.startsWith("https://")) {
    return {
      href: repo,
      target: "_blank",
      rel: "noreferrer",
    };
  }
  return {
    href: "https://github.com/" + repo,
    target: "_blank",
    rel: "noreferrer",
  };
}
