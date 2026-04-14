import type { FormEvent } from "react";
import { useEffect, useState } from "react";

import { useSettings } from "../../hooks/useSettings";
import type { SettingsPayload } from "../../types/settings";
import { areFieldsEqual } from "../../utils/object";
import { SettingsForm } from "./SettingsForm";
import { errorTextStyle, loadingTextStyle } from "./styles";

import i from "../../styles/index.module.css";

const DIRTY_CHECK_FIELDS = [
  "slackMentionKeyword",
  "slackUsername",
  "lookbackDays",
  "slackPollIntervalSeconds",
  "githubMinPollIntervalSeconds",
  "doneMenuLimit",
  "notifyOnNewPending",
  "notifyOnNewUpdates",
  "notifyOnDone",
  "notifyOnErrors",
  "hideOnlyOnClose",
  "slackToken",
  "githubToken",
] satisfies ReadonlyArray<keyof SettingsPayload>;

export function SettingsView() {
  const {
    settings,
    error: settingsError,
    isLoading: isSettingsLoading,
    isSaving,
    isSlackConnecting,
    saveSettings,
    connectSlack,
    disconnectSlack,
    slackAuthorizeUrl,
  } = useSettings();
  const [form, setForm] = useState<SettingsPayload | null>(null);

  useEffect(() => {
    if (settings) {
      setForm(settings);
    }
  }, [settings]);

  function updateField<K extends keyof SettingsPayload>(
    key: K,
    value: SettingsPayload[K],
  ) {
    setForm((current) => (current ? { ...current, [key]: value } : current));
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (
      !form ||
      !settings ||
      areFieldsEqual(form, settings, DIRTY_CHECK_FIELDS)
    ) {
      return;
    }
    await saveSettings(form);
  }

  const isDirty = !!(
    form &&
    settings &&
    !areFieldsEqual(form, settings, DIRTY_CHECK_FIELDS)
  );

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
          isSlackConnecting={isSlackConnecting}
          slackAuthorizeUrl={slackAuthorizeUrl}
          isDirty={isDirty}
          onSubmit={handleSubmit}
          onFieldChange={updateField}
          onConnectSlack={connectSlack}
          onDisconnectSlack={disconnectSlack}
        />
      )}
    </section>
  );
}
