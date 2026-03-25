import { useState } from "react";
import { P3 } from "../../../common/typo";
import s from "./inputs.module.css";

type Props = {
  checked: boolean;
  label?: string;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => Promise<void>;
};

export default function StatusCheckbox({
  checked,
  label = "확인?",
  disabled = false,
  onCheckedChange,
}: Props) {
  const [isSavingStatus, setIsSavingStatus] = useState(false);

  async function handleStatusChange(
    event: React.ChangeEvent<HTMLInputElement>,
  ) {
    const nextChecked = event.target.checked;

    setIsSavingStatus(true);
    try {
      await onCheckedChange(nextChecked);
    } catch (error) {
      window.alert(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSavingStatus(false);
    }
  }

  return (
    <label className={s.checkboxLabel}>
      <P3>{label}</P3>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled || isSavingStatus}
        onChange={handleStatusChange}
      />
    </label>
  );
}
