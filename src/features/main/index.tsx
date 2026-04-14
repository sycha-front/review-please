import { useState } from "react";
import type { ReviewDump } from "../../hooks/useReviewDump";
import PrList from "../pr/prList";
import UpdateList from "../pr/updateList";
import Tabs from "./components/tab";
import s from "./components/tab.module.css";

type Props = {
  data: ReviewDump;
};

const PENDING_SORT_STORAGE_KEY = "review-please.sort.pending";
const UPDATE_SORT_STORAGE_KEY = "review-please.sort.update";
const DONE_SORT_STORAGE_KEY = "review-please.sort.done";

export default function Main({ data }: Props) {
  const [tab, setTab] = useState(1);
  const unreadUpdateCount = data.update_feed.filter(
    (item) => !item.is_read,
  ).length;

  function handleTabClick(tab: number) {
    if (window.scrollY > 47) {
      window.scrollTo(0, 47);
    }
    setTab(tab);
  }

  return (
    <section className={s.main}>
      <Tabs
        tab={tab}
        setTab={handleTabClick}
        counts={[unreadUpdateCount, data.pending.length, data.done.length]}
      />
      <article className={s.tabContent}>
        <UpdateList
          isVisible={tab === 0}
          items={data.update_feed}
          storageKey={UPDATE_SORT_STORAGE_KEY}
        />
        <PrList
          isVisible={tab === 1}
          items={data.pending}
          storageKey={PENDING_SORT_STORAGE_KEY}
          defaultSortField="deadline"
        />
        <PrList
          isVisible={tab === 2}
          items={data.done}
          storageKey={DONE_SORT_STORAGE_KEY}
          defaultSortField="latest"
        />
      </article>
    </section>
  );
}
