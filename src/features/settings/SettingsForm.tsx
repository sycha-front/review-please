import type { FormEvent } from "react";

import type { SettingsPayload } from "../../types/settings";
import {
  footerStyle,
  formStyle,
  helperTextStyle,
  primaryButtonStyle,
  twoColumnGridStyle,
} from "./styles";
import {
  SettingsCheckboxField,
  SettingsNumberField,
  SettingsTextField,
} from "./SettingsField";

type SettingsFormProps = {
  form: SettingsPayload;
  isSaving: boolean;
  saveMessage: string | null;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onFieldChange: <K extends keyof SettingsPayload>(
    key: K,
    value: SettingsPayload[K],
  ) => void;
};

export function SettingsForm({
  form,
  isSaving,
  saveMessage,
  onSubmit,
  onFieldChange,
}: SettingsFormProps) {
  return (
    <form onSubmit={onSubmit} style={formStyle}>
      <SettingsTextField
        label="Slack Mention Keyword"
        value={form.slackMentionKeyword}
        onChange={(value) => onFieldChange("slackMentionKeyword", value)}
      />

      <SettingsTextField
        label="Slack Username"
        value={form.slackUsername}
        onChange={(value) => onFieldChange("slackUsername", value)}
      />

      <SettingsTextField
        label="GitHub Username"
        value={form.githubUsername}
        onChange={(value) => onFieldChange("githubUsername", value)}
      />

      <SettingsTextField
        label="Slack Token"
        type="password"
        value={form.slackToken}
        onChange={(value) => onFieldChange("slackToken", value)}
      />

      <SettingsTextField
        label="GitHub Token"
        type="password"
        value={form.githubToken}
        onChange={(value) => onFieldChange("githubToken", value)}
      />

      <div style={twoColumnGridStyle}>
        <SettingsNumberField
          label="Lookback Days"
          value={form.lookbackDays}
          onChange={(value) => onFieldChange("lookbackDays", value)}
        />
        <SettingsNumberField
          label="Done Menu Limit"
          value={form.doneMenuLimit}
          onChange={(value) => onFieldChange("doneMenuLimit", value)}
        />
      </div>

      <div style={twoColumnGridStyle}>
        <SettingsNumberField
          label="Slack Poll Seconds"
          value={form.slackPollIntervalSeconds}
          onChange={(value) => onFieldChange("slackPollIntervalSeconds", value)}
        />
        <SettingsNumberField
          label="GitHub Poll Seconds"
          value={form.githubMinPollIntervalSeconds}
          onChange={(value) =>
            onFieldChange("githubMinPollIntervalSeconds", value)
          }
        />
      </div>

      <SettingsCheckboxField
        label="Notify on new pending"
        checked={form.notifyOnNewPending}
        onChange={(value) => onFieldChange("notifyOnNewPending", value)}
      />

      <SettingsCheckboxField
        label="Notify on done"
        checked={form.notifyOnDone}
        onChange={(value) => onFieldChange("notifyOnDone", value)}
      />

      <SettingsCheckboxField
        label="Notify on errors"
        checked={form.notifyOnErrors}
        onChange={(value) => onFieldChange("notifyOnErrors", value)}
      />

      <div style={footerStyle}>
        <div style={helperTextStyle}>
          {saveMessage ?? "Saved values are applied to config or keychain."}
        </div>
        <button
          type="submit"
          disabled={isSaving}
          style={{
            ...primaryButtonStyle,
            cursor: isSaving ? "wait" : "pointer",
            opacity: isSaving ? 0.7 : 1,
          }}
        >
          {isSaving ? "Saving..." : "Save Settings"}
        </button>
      </div>
    </form>
  );
}
