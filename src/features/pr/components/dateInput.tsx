import { useState } from "react";
import { useReviewActions } from "../../../context/ReviewActionsContext";
import { ReviewItem } from "../../../hooks/useReviewDump";
import cn from "../../../utils/cn";
import s from "./inputs.module.css";

type Props = {
  item: ReviewItem;
};

export default function DateInput({ item }: Props) {
  const { updateDeadline } = useReviewActions();
  const [isSavingDeadline, setIsSavingDeadline] = useState(false);
  const deadlineLabel = item.deadline_date ?? "마감일 지정";

  async function handleDeadlineChange(
    event: React.ChangeEvent<HTMLInputElement>,
  ) {
    const nextDeadlineDate = event.target.value;
    if (!nextDeadlineDate) {
      return;
    }

    setIsSavingDeadline(true);
    try {
      event.target.blur();
      await updateDeadline(item.id, nextDeadlineDate);
    } catch (error) {
      window.alert(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingDeadline(false);
    }
  }

  return (
    <label className={cn(s.deadline, item.deadline_date ? s.able : "")}>
      <span>{deadlineLabel}</span>
      <input
        className={s.hiddenDateInput}
        type="date"
        value={item.deadline_date ?? ""}
        disabled={isSavingDeadline}
        onChange={handleDeadlineChange}
      />
    </label>
  );
}
