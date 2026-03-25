import { H4, P3 } from "../../common/typo";
import { useReviewActions } from "../../context/ReviewActionsContext";
import { ReviewItem } from "../../hooks/useReviewDump";
import { getGithubProps } from "../../utils";
import cn from "../../utils/cn";
import { bracketRegex } from "../../utils/regex";
import DateInput from "./components/dateInput";
import StatusCheckbox from "./components/statusCheckbox";
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
  const { updateStatus } = useReviewActions();
  const prLink = getGithubProps(item.pr_url);
  const repoLink = getGithubProps(item.repo_name);

  return (
    <li className={s.item}>
      <H4 className={s.title}>
        <a {...prLink}>{item.pr_title}</a>
      </H4>
      <P3 className={s.desc}>
        {item.slack_text.replace(bracketRegex, "").split("\n")[0]}
      </P3>
      <DateInput item={item} />
      <div className={s.credit}>
        <P3>
          {item.requester_display_name} <a {...repoLink}>@{item.repo_name}</a>
        </P3>
        <StatusCheckbox
          checked={item.status}
          onCheckedChange={(checked) => updateStatus(item.id, checked)}
        />
      </div>
    </li>
  );
}
