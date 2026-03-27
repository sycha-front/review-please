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
  isDirty: boolean;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onFieldChange: <K extends keyof SettingsPayload>(
    key: K,
    value: SettingsPayload[K],
  ) => void;
};

export function SettingsForm({
  form,
  isSaving,
  isDirty,
  onSubmit,
  onFieldChange,
}: SettingsFormProps) {
  const isSaveDisabled = isSaving || !isDirty;
  const saveLabel = isSaving ? "저장 중..." : "설정 저장";

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
        description="내 이벤트 식별에 사용됩니다."
        onChange={(value) => onFieldChange("slackUsername", value)}
      />

      <SettingsTextField
        label="GitHub 유저명"
        value={form.githubUsername}
        description="내 이벤트 식별에 사용됩니다."
        onChange={(value) => onFieldChange("githubUsername", value)}
      />

      <SettingsTextField
        label="Slack 유저 토큰"
        type="password"
        description="개발자에게 문의주세요."
        value={form.slackToken}
        onChange={(value) => onFieldChange("slackToken", value)}
      />

      <SettingsTextField
        label="GitHub 토큰 (classic)"
        type="password"
        description="필요 권한: notification, repo"
        value={form.githubToken}
        onChange={(value) => onFieldChange("githubToken", value)}
      />

      <div className={s.doubleColumn}>
        <SettingsNumberField
          label="조회 가능한 이전 일수(일)"
          value={form.lookbackDays}
          onChange={(value) => onFieldChange("lookbackDays", value)}
        />
        <SettingsNumberField
          label="완료된 PR 표시 갯수"
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
        label="X를 누를 때만 숨기기"
        checked={form.hideOnlyOnClose}
        onChange={(value) => onFieldChange("hideOnlyOnClose", value)}
      />
      {/* <SettingsCheckboxField
        label="로그인 시 자동 실행"
        checked={form.launchAtLogin}
        onChange={(value) => onFieldChange("launchAtLogin", value)}
      /> */}
      <div className={s.saveButton}>
        <P3>설정은 로컬에만 저장됩니다.</P3>
        <Button
          type="submit"
          disabled={isSaveDisabled}
          style={{
            cursor: isSaving ? "wait" : isDirty ? "pointer" : "default",
            opacity: isSaveDisabled ? 0.7 : 1,
          }}
        >
          <H4>{saveLabel}</H4>
        </Button>
      </div>
    </form>
  );
}
