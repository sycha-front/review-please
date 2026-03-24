import { H4, P3 } from "../../../common/typo";
import type { ReviewItem } from "../../../hooks/useReviewDump";
import cn from "../../../utils/cn";
import { bracketRegex } from "../../../utils/regex";
import s from "./pr.module.css";

type Props = {
  item: ReviewItem;
};

export default function PrList({
  items,
  className = "",
}: {
  items: ReviewItem[];
  className?: string;
}) {
  return (
    <ul className={cn(s.list, className)}>
      {items.map((item) => (
        <PrItem key={item.id} item={item} />
      ))}
      {items.length === 0 && "없어용"}
    </ul>
  );
}

export function PrItem({ item }: Props) {
  return (
    <li className={s.item}>
      <H4>
        <a href={item.pr_url} target="_blank">
          {item.pr_title}
        </a>
      </H4>
      <P3 className={s.desc}>
        {item.slack_text.replace(bracketRegex, "").split("\n")[0]}
      </P3>
      <P3>{item.deadline_date}</P3>
      <P3 className={s.credit}>
        {item.requester_display_name}{" "}
        <a href={"https://github.com/" + item.repo_name}>@{item.repo_name}</a>
      </P3>
    </li>
  );
}
