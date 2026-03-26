import Button from "../../common/button";
import { H4, P3 } from "../../common/typo";
import { useReviewActions } from "../../context/ReviewActionsContext";
import { UpdateFeedItem } from "../../hooks/useReviewDump";
import { useSort, type SortOptionConfig } from "../../hooks/useSort";
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
          sorted.sortOptions.find((option) => option.value === sorted.currentField)
            ?.direction === "desc"
            ? s.listReverse
            : "",
        )}
      >
        {sorted.items.map((item) => (
          <UpdateFeedCard
            key={item.id}
            item={item}
            onRead={() => markUpdateRead(item.source_event_ids)}
          />
        ))}
        {sorted.items.length === 0 && "없어용"}
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
  return (
    <li className={cn(s.item, item.is_read ? s.read : "")}>
      <H4 className={s.title}>
        <a href={item.target_url} target="_blank" rel="noreferrer">
          {item.headline}
        </a>
      </H4>
      {item.summary && <P3 className={s.desc}>{item.summary}</P3>}
      <P3>{item.time_label}</P3>
      <div className={s.credit}>
        <P3>
          <a
            href={"https://github.com/" + item.actor_login}
            target="_blank"
            rel="noreferrer"
          >
            {item.actor_login ?? "unknown"}
          </a>{" "}
          {item.actor_context}
          {item.event_count > 1 ? ` · ${item.event_count}개 활동` : ""}
        </P3>
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
