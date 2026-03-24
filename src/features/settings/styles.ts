import type { CSSProperties } from "react";

export const pageStyle: CSSProperties = {
  minHeight: "100vh",
  margin: 0,
  padding: "16px",
  boxSizing: "border-box",
};

export const reviewStatusStyle: CSSProperties = {
  textAlign: "right",
  color: "#4b5563",
  fontSize: "12px",
};

export const inputStyle: CSSProperties = {
  width: "100%",
  border: "1px solid #d1d5db",
  borderRadius: "10px",
  padding: "10px 12px",
  fontSize: "14px",
  boxSizing: "border-box",
  background: "#ffffff",
  color: "#111827",
};

export const labelStyle: CSSProperties = {
  display: "grid",
  gap: "6px",
  fontSize: "12px",
  color: "#374151",
};

export const formStyle: CSSProperties = {
  display: "grid",
  gap: "14px",
};

export const twoColumnGridStyle: CSSProperties = {
  display: "grid",
  gridTemplateColumns: "1fr 1fr",
  gap: "12px",
};

export const checkboxLabelStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: "10px",
  fontSize: "13px",
  color: "#374151",
};

export const footerStyle: CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  marginTop: "4px",
};

export const helperTextStyle: CSSProperties = {
  color: "#6b7280",
  fontSize: "12px",
  lineHeight: "14px",
};

export const primaryButtonStyle: CSSProperties = {
  border: "none",
  borderRadius: "10px",
  background: "#111827",
  color: "#ffffff",
  padding: "10px 14px",
  fontSize: "13px",
  fontWeight: 600,
};

export const loadingTextStyle: CSSProperties = {
  color: "#6b7280",
  fontSize: "13px",
};

export const errorTextStyle: CSSProperties = {
  color: "#b91c1c",
  fontSize: "13px",
};
