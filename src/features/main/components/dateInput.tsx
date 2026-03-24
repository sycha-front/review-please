import { useState } from "react";
import Button from "../../../common/button";
import { P3 } from "../../../common/typo";
import { ReviewItem } from "../../../hooks/useReviewDump";
import { useReviewActions } from "../ReviewActionsContext";
import s from "./pr.module.css";

type Props = {
  item: ReviewItem;
};

export default function DateInput({ item }: Props) {
  const { updateDeadline } = useReviewActions();
  const [isDeadlineEditing, setIsDeadlineEditing] = useState(false);
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
      setIsDeadlineEditing(false);
    } catch (error) {
      window.alert(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingDeadline(false);
    }
  }
  return (
    <>
      {item.deadline_date ? (
        <P3 className={s.deadline}>{item.deadline_date}</P3>
      ) : isDeadlineEditing ? (
        <input
          className={s.deadlineInput}
          type="date"
          autoFocus
          disabled={isSavingDeadline}
          onBlur={() => setIsDeadlineEditing(false)}
          onChange={handleDeadlineChange}
        />
      ) : (
        <Button
          className={s.deadline}
          disabled={isSavingDeadline}
          onClick={() => setIsDeadlineEditing(true)}
        >
          기한 설정
        </Button>
      )}
    </>
  );
}
