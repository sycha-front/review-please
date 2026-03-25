import Button from "../../../common/button";
import { P3 } from "../../../common/typo";
import cn from "../../../utils/cn";
import s from "./tab.module.css";

type Props = {
  tab: number;
  setTab: (arg0: number) => void;
  counts: [number, number, number];
};

export default function Tabs({ tab, setTab, counts }: Props) {
  return (
    <div className={s.tabs}>
      <Button
        className={cn(s.tab, tab === 0 ? s.active : "")}
        onClick={() => setTab(0)}
      >
        <P3>Pending</P3>
        <span className={s.count}>
          <P3>{counts[0]}</P3>
        </span>
        <span
          className={s.current}
          style={{ transform: `translateX(${100 * tab}%)` }}
        />
      </Button>
      <Button
        className={cn(s.tab, tab === 1 ? s.active : "")}
        onClick={() => setTab(1)}
      >
        <P3>Update</P3>
        <span className={s.count}>
          <P3>{counts[1]}</P3>
        </span>
      </Button>
      <Button
        className={cn(s.tab, tab === 2 ? s.active : "")}
        onClick={() => setTab(2)}
      >
        <P3>Done</P3>
      </Button>
    </div>
  );
}
