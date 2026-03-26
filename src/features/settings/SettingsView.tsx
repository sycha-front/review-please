import type { FormEvent } from "react";
import { useEffect, useState } from "react";

import { useSettings } from "../../hooks/useSettings";
import type { SettingsPayload } from "../../types/settings";
import { SettingsForm } from "./SettingsForm";
import { errorTextStyle, loadingTextStyle } from "./styles";

import i from "../../styles/index.module.css";

export function SettingsView() {
  const {
    settings,
    error: settingsError,
    isLoading: isSettingsLoading,
    isSaving,
    saveSettings,
  } = useSettings();
  const [form, setForm] = useState<SettingsPayload | null>(null);
  const [saveMessage, setSaveMessage] = useState<string | null>(null);

  useEffect(() => {
    if (settings) {
      setForm(settings);
    }
  }, [settings]);

  useEffect(() => {
    if (!saveMessage) {
      return undefined;
    }
    const timeoutId = window.setTimeout(() => {
      setSaveMessage(null);
    }, 1500);
    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [saveMessage]);

  function updateField<K extends keyof SettingsPayload>(
    key: K,
    value: SettingsPayload[K],
  ) {
    setForm((current) => (current ? { ...current, [key]: value } : current));
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!form) {
      return;
    }
    await saveSettings(form);
    setSaveMessage("Saved");
  }

  return (
    <section className={i.panel} id="settings">
      {isSettingsLoading && (
        <div style={loadingTextStyle}>Loading settings...</div>
      )}

      {!isSettingsLoading && settingsError && (
        <div style={errorTextStyle}>{settingsError}</div>
      )}

      {!isSettingsLoading && form && (
        <SettingsForm
          form={form}
          isSaving={isSaving}
          saveMessage={saveMessage}
          onSubmit={handleSubmit}
          onFieldChange={updateField}
        />
      )}
    </section>
  );
}
