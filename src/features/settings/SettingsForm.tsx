import type { FormEvent } from "react";

import Button from "../../common/button";
import { H1, H4, P3 } from "../../common/typo";
import type { SettingsPayload } from "../../types/settings";
import s from "./settings.module.css";
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
  onSubmit,
  onFieldChange,
}: SettingsFormProps) {
  return (
    <form onSubmit={onSubmit} className={s.form}>
      <H1>설정</H1>
      <SettingsTextField
        label="Slack 멘션 키워드"
        value={form.slackMentionKeyword}
        placeholder="@my-name, team-review"
        description="콤마(,)로 구분해서 여러 키워드를 입력할 수 있어요."
        onChange={(value) => onFieldChange("slackMentionKeyword", value)}
      />

      <SettingsTextField
        label="Slack 유저명"
        value={form.slackUsername}
        onChange={(value) => onFieldChange("slackUsername", value)}
      />

      <SettingsTextField
        label="GitHub 유저명"
        value={form.githubUsername}
        onChange={(value) => onFieldChange("githubUsername", value)}
      />

      <SettingsTextField
        label="로컬 repo 경로"
        value={form.repoPath}
        onChange={(value) => onFieldChange("repoPath", value)}
      />

      <SettingsTextField
        label="Slack 유저 토큰"
        type="password"
        value={form.slackToken}
        onChange={(value) => onFieldChange("slackToken", value)}
      />

      <SettingsTextField
        label="GitHub 토큰"
        type="password"
        value={form.githubToken}
        onChange={(value) => onFieldChange("githubToken", value)}
      />

      <div className={s.doubleColumn}>
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

      <div className={s.doubleColumn}>
        <SettingsNumberField
          label="Slack 불러오기 간격(초)"
          value={form.slackPollIntervalSeconds}
          onChange={(value) => onFieldChange("slackPollIntervalSeconds", value)}
        />
        <SettingsNumberField
          label="GitHub 불러오기 간격(초)"
          value={form.githubMinPollIntervalSeconds}
          onChange={(value) =>
            onFieldChange("githubMinPollIntervalSeconds", value)
          }
        />
      </div>
      <SettingsCheckboxField
        label="리뷰 대기 중인 PR이 생겼을 때 알림"
        checked={form.notifyOnNewPending}
        onChange={(value) => onFieldChange("notifyOnNewPending", value)}
      />
      <SettingsCheckboxField
        label="리뷰 완료 PR이 생겼을 때 알림"
        checked={form.notifyOnDone}
        onChange={(value) => onFieldChange("notifyOnDone", value)}
      />
      <SettingsCheckboxField
        label="오류가 생겼을 때 알림"
        checked={form.notifyOnErrors}
        onChange={(value) => onFieldChange("notifyOnErrors", value)}
      />
      <SettingsCheckboxField
        label="로그인 시 자동 실행"
        checked={form.launchAtLogin}
        onChange={(value) => onFieldChange("launchAtLogin", value)}
      />
      <P3>설정은 로컬에만 저장됩니다.</P3>
      <Button
        type="submit"
        disabled={isSaving}
        style={{
          cursor: isSaving ? "wait" : "pointer",
          opacity: isSaving ? 0.7 : 1,
        }}
      >
        <H4>{isSaving ? "저장 중..." : "설정 저장"}</H4>
      </Button>
    </form>
  );
}
