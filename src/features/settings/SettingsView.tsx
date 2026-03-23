import type { FormEvent } from "react";
import { useEffect, useState } from "react";

import type { UseReviewDumpResult } from "../../hooks/useReviewDump";
import type { UseSettingsResult } from "../../hooks/useSettings";
import type { SettingsPayload } from "../../types/settings";
import { SettingsForm } from "./SettingsForm";
import { SettingsHeader } from "./SettingsHeader";
import {
  errorTextStyle,
  loadingTextStyle,
  pageStyle,
  panelStyle,
} from "./styles";

type SettingsViewProps = {
  reviewState: UseReviewDumpResult;
  settingsState: UseSettingsResult;
};

export function SettingsView({
  reviewState,
  settingsState,
}: SettingsViewProps) {
  const { error, isLoading, snapshot } = reviewState;
  const {
    settings,
    error: settingsError,
    isLoading: isSettingsLoading,
    isSaving,
    saveSettings,
  } = settingsState;
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

  const reviewSummary = isLoading
    ? "Loading review data..."
    : error
      ? `Review error: ${error}`
      : snapshot
        ? `Pending ${snapshot.pending.length} / Done ${snapshot.done.length}`
        : "No review data";

  return (
    <main style={pageStyle}>
      <div style={panelStyle}>
        <SettingsHeader reviewSummary={reviewSummary} />

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
      </div>
    </main>
  );
}
