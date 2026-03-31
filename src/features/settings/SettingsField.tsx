import { ReactNode } from "react";
import { P3 } from "../../common/typo";
import s from "./settings.module.css";

type TextFieldProps = {
  label: string;
  value: string;
  type?: "text" | "password";
  placeholder?: string;
  description?: string;
  children?: ReactNode;
  onChange: (value: string) => void;
};

export function SettingsTextField({
  label,
  value,
  type = "text",
  placeholder,
  description,
  children,
  onChange,
}: TextFieldProps) {
  return (
    <label className={s.label}>
      {label}
      <input
        className={s.input}
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
      {description && <span className={s.helperText}>{description}</span>}
      {children}
    </label>
  );
}

type NumberFieldProps = {
  label: string;
  value: number;
  onChange: (value: number) => void;
};

export function SettingsNumberField({
  label,
  value,
  onChange,
}: NumberFieldProps) {
  return (
    <label className={s.label}>
      {label}
      <input
        className={s.input}
        type="number"
        value={value}
        onChange={(event) => onChange(Number(event.currentTarget.value))}
      />
    </label>
  );
}

type CheckboxFieldProps = {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
};

export function SettingsCheckboxField({
  label,
  checked,
  onChange,
}: CheckboxFieldProps) {
  return (
    <label className={s.checkbox}>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      <P3>{label}</P3>
    </label>
  );
}
