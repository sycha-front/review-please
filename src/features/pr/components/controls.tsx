import { ReactNode } from "react";
import { RightArrow } from "../../../assets/icons";
import { P3 } from "../../../common/typo";
import type { SortField, SortOption } from "../../../hooks/useSort";
import cn from "../../../utils/cn";
import s from "./controls.module.css";

type Props = {
  sorted: {
    sortOptions: SortOption[];
    currentField: SortField;
    onSortChange: (value: SortField) => void;
  };
  children?: ReactNode;
};

export default function Controls({ sorted, children }: Props) {
  return (
    <div className={s.controls}>
      <div className={s.group} role="group" aria-label="정렬 기준">
        {sorted.sortOptions.map((option) => (
          <button
            key={option.value}
            type="button"
            className={cn(
              s.controlButton,
              sorted.currentField === option.value ? s.active : "",
              option.direction === "asc" ? s.up : s.down,
            )}
            aria-pressed={sorted.currentField === option.value}
            onClick={() => sorted.onSortChange(option.value)}
          >
            <P3>{option.label} 순</P3>
            <RightArrow />
          </button>
        ))}
      </div>
      {children}
    </div>
  );
}
