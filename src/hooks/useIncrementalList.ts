import { useEffect, useState } from "react";

type Options = {
  step?: number;
  resetKey?: string | number;
  reverse?: boolean;
};

type UseIncrementalListResult<T> = {
  visibleItems: T[];
  hasMore: boolean;
  loadMore: () => void;
};

export function useIncrementalList<T>(
  items: T[],
  { step = 10, resetKey, reverse = false }: Options = {},
): UseIncrementalListResult<T> {
  const [visibleCount, setVisibleCount] = useState(step);

  useEffect(() => {
    if (resetKey === undefined) {
      return;
    }
    setVisibleCount(step);
  }, [resetKey, step]);

  const visibleItems = reverse
    ? items.slice(-visibleCount)
    : items.slice(0, visibleCount);
  const hasMore = items.length > visibleCount;

  function loadMore() {
    setVisibleCount((current) => current + step);
  }

  return {
    visibleItems,
    hasMore,
    loadMore,
  };
}
