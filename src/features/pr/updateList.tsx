import Button from "../../common/button";
import { H4, P3 } from "../../common/typo";
import { UpdateFeedItem } from "../../hooks/useReviewDump";
import cn from "../../utils/cn";
import StatusCheckbox from "./components/statusCheckbox";
import s from "./pr.module.css";

export default function UpdateFeedList({
  items,
  markUpdateRead,
  markAllUpdateRead,
  className = "",
}: {
  items: UpdateFeedItem[];
  markUpdateRead: (eventIds: string[]) => Promise<void>;
  markAllUpdateRead: () => Promise<void>;
  className?: string;
}) {
  const unreadCount = items.filter((item) => !item.is_read).length;

  return (
    <div className={cn(className)}>
      <div className={s.updateActions}>
        <P3>
          {unreadCount > 0
            ? `${unreadCount}개 origin 안 읽음`
            : "모두 읽었어요"}
        </P3>
        <Button
          className={s.readButton}
          disabled={unreadCount === 0}
          onClick={() => void markAllUpdateRead()}
        >
          모두 읽음 처리
        </Button>
      </div>
      <ul className={s.list}>
        {items.map((item) => (
          <UpdateFeedCard
            key={item.id}
            item={item}
            onRead={() => markUpdateRead(item.source_event_ids)}
          />
        ))}
        {items.length === 0 && "없어용"}
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
    <li className={cn(s.item)}>
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
