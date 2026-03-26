import { useState } from "react";
import { useDebounce } from "use-debounce";
import type { ReviewItem } from "../../../hooks/useReviewDump";

type UsePrSearchResult = {
  query: string;
  setQuery: (value: string) => void;
  filteredItems: ReviewItem[];
};

export default function usePrSearch(items: ReviewItem[]): UsePrSearchResult {
  const [query, setQuery] = useState("");
  const [debouncedQuery] = useDebounce(query, 300);
  const normalizedQuery = debouncedQuery.trim().toLowerCase();

  const filteredItems =
    normalizedQuery.length === 0
      ? items
      : items.filter((item) =>
          [
            item.pr_title,
            item.repo_owner,
            item.repo_name,
            item.pr_key,
            item.pr_author_login,
            item.requester_display_name,
            item.slack_text,
          ]
            .filter((value): value is string => Boolean(value))
            .some((value) => value.toLowerCase().includes(normalizedQuery)),
        );

  return {
    query,
    setQuery,
    filteredItems,
  };
}
