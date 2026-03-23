import { inputStyle, labelStyle, checkboxLabelStyle } from "./styles";

type TextFieldProps = {
  label: string;
  value: string;
  type?: "text" | "password";
  onChange: (value: string) => void;
};

export function SettingsTextField({
  label,
  value,
  type = "text",
  onChange,
}: TextFieldProps) {
  return (
    <label style={labelStyle}>
      {label}
      <input
        style={inputStyle}
        type={type}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
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
    <label style={labelStyle}>
      {label}
      <input
        style={inputStyle}
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
    <label style={checkboxLabelStyle}>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      {label}
    </label>
  );
}
