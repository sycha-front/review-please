import { useState } from "react";
import type { ReviewDump } from "../../hooks/useReviewDump";
import cn from "../../utils/cn";
import PrList from "../pr/prList";
import UpdateFeedList from "../pr/updateList";
import Tabs from "./components/tab";
import s from "./components/tab.module.css";

type Props = {
  data: ReviewDump;
  markUpdateRead: (eventIds: string[]) => Promise<void>;
  markAllUpdateRead: () => Promise<void>;
};

export default function Main({
  data,
  markUpdateRead,
  markAllUpdateRead,
}: Props) {
  const [tab, setTab] = useState(0);
  const unreadUpdateCount = data.update_feed.filter(
    (item) => !item.is_read,
  ).length;

  return (
    <section>
      <Tabs
        tab={tab}
        setTab={setTab}
        counts={[data.pending.length, unreadUpdateCount, data.done.length]}
      />
      <article className={s.tabContent}>
        <PrList
          className={cn(tab === 0 ? s.visible : s.hidden)}
          items={data.pending}
        />
        <UpdateFeedList
          className={cn(tab === 1 ? s.visible : s.hidden)}
          items={data.update_feed}
          markUpdateRead={markUpdateRead}
          markAllUpdateRead={markAllUpdateRead}
        />
        <PrList
          className={cn(tab === 2 ? s.visible : s.hidden)}
          items={data.done}
        />
      </article>
    </section>
  );
}
