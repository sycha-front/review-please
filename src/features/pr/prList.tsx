import Button from "../../common/button";
import { H4, P3 } from "../../common/typo";
import { useReviewActions } from "../../context/ReviewActionsContext";
import { useIncrementalList } from "../../hooks/useIncrementalList";
import { ReviewItem } from "../../hooks/useReviewDump";
import {
  useSort,
  type SortField,
  type SortOptionConfig,
} from "../../hooks/useSort";
import { getGithubProps } from "../../utils";
import cn from "../../utils/cn";
import { bracketRegex } from "../../utils/regex";
import Controls from "./components/controls";
import DateInput from "./components/dateInput";
import Search from "./components/search";
import StatusCheckbox from "./components/statusCheckbox";
import usePrSearch from "./hooks/usePrSearch";
import s from "./pr.module.css";

type Props = {
  item: ReviewItem;
};

const MAX_DEADLINE = "9999-12-31";

const reviewSortOptions: SortOptionConfig<ReviewItem>[] = [
  {
    value: "deadline",
    label: "마감일",
    defaultDirection: "asc",
    getValue: (item) => item.deadline_date ?? MAX_DEADLINE,
  },
  {
    value: "latest",
    label: "최신",
    defaultDirection: "desc",
    getValue: (item) =>
      item.completed_at ?? item.pr_merged_at ?? item.updated_at,
  },
];

export default function PrList({
  items,
  isVisible,
  storageKey,
  defaultSortField = "latest",
}: {
  items: ReviewItem[];
  isVisible: boolean;
  storageKey: string;
  defaultSortField?: SortField;
}) {
  const search = usePrSearch(items);
  const sorted = useSort({
    items: search.filteredItems,
    storageKey,
    options: reviewSortOptions,
    defaultField: defaultSortField,
    tieBreaker: (item) => item.created_at,
  });
  const isDescending =
    sorted.sortOptions.find((option) => option.value === sorted.currentField)
      ?.direction === "desc";
  const paginated = useIncrementalList(sorted.items, {
    resetKey: search.query,
    reverse: isDescending,
  });

  return (
    <article className={cn(s.field, isVisible ? s.visible : s.hidden)}>
      <Controls sorted={sorted}>
        <Search value={search.query} onChange={search.setQuery} />
      </Controls>
      <ul
        className={cn(
          s.list,
          sorted.sortOptions.find(
            (option) => option.value === sorted.currentField,
          )?.direction === "desc"
            ? s.listReverse
            : "",
        )}
      >
        {paginated.visibleItems.map((item) => (
          <PrItem key={item.id} item={item} />
        ))}
        {paginated.visibleItems.length === 0 && "없어용"}
      </ul>
      {paginated.hasMore && (
        <Button className={s.loadMoreButton} onClick={paginated.loadMore}>
          <P3>더 보기</P3>
        </Button>
      )}
    </article>
  );
}

export function PrItem({ item }: Props) {
  const { updateStatus } = useReviewActions();
  const prLink = getGithubProps(item.pr_url);
  const repoLink = getGithubProps(`${item.repo_owner}/${item.repo_name}`);

  return (
    <li className={cn(s.item, item.status ? s.read : "")}>
      <H4 className={s.title}>
        <a {...prLink}>{item.pr_title}</a>
      </H4>
      <P3 className={s.desc}>
        {item.slack_text.replace(bracketRegex, "").split("\n")[0]}
      </P3>
      <DateInput item={item} />
      <div className={s.credit}>
        <P3>
          {item.slack_channel_id
            ? item.requester_display_name
            : item.pr_author_login}{" "}
          <a {...repoLink}>@{item.repo_name}</a>
        </P3>
        <StatusCheckbox
          checked={item.status}
          onCheckedChange={(checked) => updateStatus(item.id, checked)}
        />
      </div>
    </li>
  );
}
