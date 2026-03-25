import { useEffect, useState } from "react";
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

  async function handleDeadlineChange(
    event: React.ChangeEvent<HTMLInputElement>,
  ) {
    const nextDeadlineDate = event.target.value;
    if (!nextDeadlineDate) {
      return;
    }

    setIsSavingDeadline(true);
    try {
      await updateDeadline(item.id, nextDeadlineDate);
    } catch (error) {
      window.alert(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingDeadline(false);
    }

    useEffect(() => {
      console.log("item.deadline_date:", item.deadline_date);
    }, [item.deadline_date]);
  }

  return (
    <input
      className={cn(s.deadline, !item.deadline_date ? "" : s.able)}
      type="date"
      value={item.deadline_date ?? undefined}
      disabled={isSavingDeadline}
      onChange={handleDeadlineChange}
    />
  );
}
