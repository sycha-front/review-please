import { useState } from "react";
import type { ReviewDump } from "../../hooks/useReviewDump";
import cn from "../../utils/cn";
import { default as PrList } from "./components/prItem";
import Tabs from "./components/tab";
import s from "./components/tab.module.css";

type Props = {
  data: ReviewDump;
};

export default function Main({ data }: Props) {
  const [tab, setTab] = useState(0);

  return (
    <section>
      <Tabs
        tab={tab}
        setTab={setTab}
        counts={[data.pending.length, data.done.length, data.update.length]}
      />
      <article className={s.tabContent}>
        <PrList
          className={cn(tab === 0 ? s.visible : s.hidden)}
          items={data.pending}
        />
        <PrList
          className={cn(tab === 1 ? s.visible : s.hidden)}
          items={data.done}
        />
        <PrList
          className={cn(tab === 2 ? s.visible : s.hidden)}
          items={data.update}
        />
      </article>
    </section>
  );
}
