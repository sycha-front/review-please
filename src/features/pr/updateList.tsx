import Button from "../../common/button";
import { H4, P3 } from "../../common/typo";
import { useReviewActions } from "../../context/ReviewActionsContext";
import { useIncrementalList } from "../../hooks/useIncrementalList";
import { UpdateFeedItem } from "../../hooks/useReviewDump";
import { useSort, type SortOptionConfig } from "../../hooks/useSort";
import { getGithubProps } from "../../utils";
import cn from "../../utils/cn";
import Controls from "./components/controls";
import StatusCheckbox from "./components/statusCheckbox";
import s from "./pr.module.css";

const updateSortOptions: SortOptionConfig<UpdateFeedItem>[] = [
  {
    value: "latest",
    label: "최신",
    defaultDirection: "desc",
    getValue: (item) => item.occurred_at,
  },
];

export default function UpdateList({
  items,
  isVisible,
  storageKey,
}: {
  items: UpdateFeedItem[];
  isVisible: boolean;
  storageKey: string;
}) {
  const { markAllUpdateRead, markUpdateRead } = useReviewActions();
  const unreadCount = items.filter((item) => !item.is_read).length;
  const sorted = useSort({
    items,
    storageKey,
    options: updateSortOptions,
    defaultField: "latest",
    tieBreaker: (item) => item.id,
  });
  const isDescending =
    sorted.sortOptions.find((option) => option.value === sorted.currentField)
      ?.direction === "desc";
  const paginated = useIncrementalList(sorted.items, {
    reverse: isDescending,
  });

  return (
    <div className={cn(isVisible ? s.visible : s.hidden)}>
      <Controls sorted={sorted}>
        <Button
          disabled={unreadCount === 0}
          onClick={() => void markAllUpdateRead()}
        >
          <P3>모두 읽기</P3>
        </Button>
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
          <UpdateFeedCard
            key={item.id}
            item={item}
            onRead={() => markUpdateRead(item.source_event_ids)}
          />
        ))}
        {paginated.visibleItems.length === 0 && "없어용"}
        {paginated.hasMore && (
          <Button className={s.loadMoreButton} onClick={paginated.loadMore}>
            <P3>더 보기</P3>
          </Button>
        )}
      </ul>
    </div>
  );
}

export function UpdateFeedCard({
  item,
  onRead,
}: {
  item: UpdateFeedItem;
  onRead: () => Promise<void>;
}) {
  const eventLink = getGithubProps(item.target_url);
  const repoLink = getGithubProps(`${item.repo_label}`);

  return (
    <li className={cn(s.item, item.is_read ? s.read : "")}>
      <H4 className={s.title}>
        <a {...eventLink}>{item.headline}</a>
      </H4>
      {item.summary && <P3 className={s.desc}>{item.summary}</P3>}
      <P3>{item.time_label}</P3>
      <div className={s.credit}>
        <P3>{item.actor_login ?? "unknown"} </P3>
        <P3>
          <a {...repoLink}>{item.actor_context}</a>
        </P3>
        <P3>{item.event_count > 1 ? `· ${item.event_count}개 활동` : ""}</P3>
        <StatusCheckbox
          checked={item.is_read}
          label="읽음"
          disabled={item.is_read}
          onCheckedChange={async (checked) => {
            if (!checked) {
              return;
            }
            await onRead();
          }}
        />
      </div>
    </li>
  );
}
