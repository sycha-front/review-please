import { useState } from "react";
import { P3 } from "../../../common/typo";
import { ReviewItem } from "../../../hooks/useReviewDump";
import { useReviewActions } from "../ReviewActionsContext";
import s from "./pr.module.css";

type Props = {
  item: ReviewItem;
};

export default function StatusCheckbox({ item }: Props) {
  const { updateStatus } = useReviewActions();
  const [isSavingStatus, setIsSavingStatus] = useState(false);
  const isDone = item.status === "done";

  async function handleStatusChange(
    event: React.ChangeEvent<HTMLInputElement>,
  ) {
    const nextStatus = event.target.checked ? "done" : "pending";

    setIsSavingStatus(true);
    try {
      await updateStatus(item.id, nextStatus);
    } catch (error) {
      window.alert(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingStatus(false);
    }
  }

  return (
    <label className={s.checkboxLabel}>
      <P3>{isDone ? "아직?" : "리뷰 완료?"}</P3>
      <input
        type="checkbox"
        checked={isDone}
        disabled={isSavingStatus}
        onChange={handleStatusChange}
      />
    </label>
  );
}
