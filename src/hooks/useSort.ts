import { useEffect, useMemo, useState } from "react";

export type SortField = "latest" | "deadline";
export type SortDirection = "asc" | "desc";
export type SortOptionConfig<T> = {
  label: string;
  value: SortField;
  defaultDirection?: SortDirection;
  getValue: (item: T) => string;
};

export type SortOption = {
  label: string;
  value: SortField;
  direction: SortDirection;
};

type SortState = {
  currentField: SortField;
  sortOptions: SortOption[];
};

export function useSort<T>({
  items,
  storageKey,
  options,
  defaultField,
  tieBreaker,
}: {
  items: T[];
  storageKey: string;
  options: SortOptionConfig<T>[];
  defaultField?: SortField;
  tieBreaker?: (item: T) => string;
}) {
  const fallback = useMemo(() => {
    const currentField =
      options.find((option) => option.value === defaultField)?.value ??
      options[0].value;

    return {
      currentField,
      sortOptions: options.map(({ defaultDirection, label, value }) => ({
        label,
        value,
        direction: defaultDirection ?? "desc",
      })),
    };
  }, [defaultField, options]);

  const [sortState, setSortState] = useState<SortState>(() => {
    if (typeof window === "undefined") {
      return fallback;
    }

    try {
      const saved = JSON.parse(window.localStorage.getItem(storageKey) ?? "null");
      if (
        !saved ||
        (saved.currentField !== "latest" &&
          saved.currentField !== "deadline" &&
          saved.field !== "latest" &&
          saved.field !== "deadline")
      ) {
        return fallback;
      }

      return {
        currentField: options.some(
          (option) => option.value === (saved.currentField ?? saved.field),
        )
          ? (saved.currentField ?? saved.field)
          : fallback.currentField,
        sortOptions: fallback.sortOptions.map((option) => ({
          ...option,
          direction:
            saved.sortOptions?.find(
              (savedOption: SortOption) => savedOption.value === option.value,
            )?.direction ??
            (saved.value === option.value ? saved.direction : undefined) ??
            saved.directions?.[option.value] ??
            option.direction,
        })),
      };
    } catch {
      return fallback;
    }
  });

  useEffect(() => {
    setSortState((current) => ({
      currentField: options.some(
        (option) => option.value === current.currentField,
      )
        ? current.currentField
        : fallback.currentField,
      sortOptions: fallback.sortOptions.map((option) => ({
        ...option,
        direction:
          current.sortOptions.find((currentOption) => currentOption.value === option.value)
            ?.direction ?? option.direction,
      })),
    }));
  }, [fallback, options]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    window.localStorage.setItem(storageKey, JSON.stringify(sortState));
  }, [sortState, storageKey]);

  const activeConfig = useMemo(
    () =>
      options.find((option) => option.value === sortState.currentField) ??
      options[0],
    [options, sortState.currentField],
  );

  const sortedItems = useMemo(() => {
    const nextItems = [...items];

    nextItems.sort((left, right) => {
      const compared = activeConfig
        .getValue(left)
        .localeCompare(activeConfig.getValue(right));

      if (compared !== 0 || !tieBreaker) {
        return compared;
      }

      return tieBreaker(left).localeCompare(tieBreaker(right));
    });

    return nextItems;
  }, [activeConfig, items, tieBreaker]);

  function handleSortChange(field: SortField) {
    setSortState((current) => {
      return {
        currentField: field,
        sortOptions: current.sortOptions.map((option) =>
          option.value !== field
            ? option
            : {
                ...option,
                direction:
                  current.currentField === field
                    ? option.direction === "asc"
                      ? "desc"
                      : "asc"
                    : option.direction,
              },
        ),
      };
    });
  }

  return {
    items: sortedItems,
    currentField: sortState.currentField,
    sortOptions: sortState.sortOptions,
    onSortChange: handleSortChange,
  };
}
