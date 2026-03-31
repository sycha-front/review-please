import type { FormEvent } from "react";
import { useEffect, useState } from "react";

import { useSettings } from "../../hooks/useSettings";
import type { SettingsPayload } from "../../types/settings";
import { SettingsForm } from "./SettingsForm";
import { errorTextStyle, loadingTextStyle } from "./styles";

import i from "../../styles/index.module.css";
function areSettingsEqual(left: SettingsPayload, right: SettingsPayload) {
  return (
    left.slackMentionKeyword === right.slackMentionKeyword &&
    left.slackUsername === right.slackUsername &&
    left.githubUsername === right.githubUsername &&
    left.lookbackDays === right.lookbackDays &&
    left.slackPollIntervalSeconds === right.slackPollIntervalSeconds &&
    left.githubMinPollIntervalSeconds === right.githubMinPollIntervalSeconds &&
    left.doneMenuLimit === right.doneMenuLimit &&
    left.notifyOnNewPending === right.notifyOnNewPending &&
    left.notifyOnDone === right.notifyOnDone &&
    left.notifyOnErrors === right.notifyOnErrors &&
    left.hideOnlyOnClose === right.hideOnlyOnClose &&
    left.launchAtLogin === right.launchAtLogin &&
    left.slackToken === right.slackToken &&
    left.githubToken === right.githubToken
  );
}

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
    if (!form || !settings || areSettingsEqual(form, settings)) {
      return;
    }
    await saveSettings(form);
  }

  const isDirty = !!(form && settings && !areSettingsEqual(form, settings));

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
