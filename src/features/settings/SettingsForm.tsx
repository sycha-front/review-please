import type { FormEvent } from "react";
import { useState } from "react";

import { RightArrow, Slack } from "../../assets/icons";
import Button from "../../common/button";
import { H1, H4, P3 } from "../../common/typo";
import type { SettingsPayload } from "../../types/settings";
import cn from "../../utils/cn";
import s from "./settings.module.css";
import {
  SettingsCheckboxField,
  SettingsNumberField,
  SettingsTextField,
} from "./SettingsField";

type SettingsFormProps = {
  form: SettingsPayload;
  isSaving: boolean;
  isSlackConnecting: boolean;
  isDirty: boolean;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onFieldChange: <K extends keyof SettingsPayload>(
    key: K,
    value: SettingsPayload[K],
  ) => void;
  onConnectSlack: () => Promise<void>;
  onDisconnectSlack: () => Promise<void>;
};

export function SettingsForm({
  form,
  isSaving,
  isSlackConnecting,
  isDirty,
  onSubmit,
  onFieldChange,
  onConnectSlack,
  onDisconnectSlack,
}: SettingsFormProps) {
  const [showAdvancedSlackToken, setShowAdvancedSlackToken] = useState(false);
  const isSaveDisabled = isSaving || !isDirty;
  const saveLabel = isSaving ? "저장 중..." : "설정 저장";
  const oauthButtonLabel = isSlackConnecting
    ? "브라우저에서 승인 대기 중..."
    : form.slackConnected
      ? "다시 연결"
      : "Slack 연결";
  const authStatusText = form.slackConnected
    ? `${form.slackConnectedWorkspace ?? "워크스페이스"} / ${form.slackConnectedUser ?? "연결됨"}`
    : form.slackAuthMode === "manual"
      ? "고급 옵션의 수동 Slack 토큰을 사용 중이에요."
      : "Slack OAuth 연결이 필요해요.";

  return (
    <form onSubmit={onSubmit} className={s.form}>
      <H1>설정</H1>
      <div className={s.label}>
        Slack 연결
        <div className={s.authCard}>
          <Button
            color={form.slackConnected ? "secondary" : ""}
            className={s.input}
            disabled={isSlackConnecting}
            onClick={onConnectSlack}
          >
            <Slack /> {oauthButtonLabel}
          </Button>
          {form.slackConnected && (
            <Button
              color="secondary"
              className={s.input}
              disabled={isSlackConnecting}
              onClick={onDisconnectSlack}
            >
              연결 해제
            </Button>
          )}
          <Button
            color="secondary"
            className={cn(
              s.input,
              s.moreButton,
              showAdvancedSlackToken ? s.moreButtonActive : "",
            )}
            onClick={() => setShowAdvancedSlackToken((current) => !current)}
          >
            <RightArrow />
          </Button>
        </div>
        <P3 className={s.helperText}>현재: {authStatusText}</P3>
        {showAdvancedSlackToken && (
          <div className={s.advancedPanel}>
            <SettingsTextField
              label="Slack 유저 토큰"
              type="password"
              description={`비상 상황용 수동 토큰입니다.${"\n"}OAuth 연결이 있으면 자동으로 우선 사용돼요.`}
              value={form.slackToken}
              onChange={(value) => onFieldChange("slackToken", value)}
            />
            <SettingsTextField
              label="Slack 유저명"
              value={form.slackUsername}
              description="내 이벤트가 자동으로 식별되지 않을 때 입력해주세요."
              onChange={(value) => onFieldChange("slackUsername", value)}
            />
          </div>
        )}
      </div>

      <SettingsTextField
        label="알림을 받을 Slack 멘션 키워드"
        value={form.slackMentionKeyword}
        placeholder="@my-name, team-review"
        description="콤마(,)로 구분해서 여러 키워드를 입력할 수 있어요."
        onChange={(value) => onFieldChange("slackMentionKeyword", value)}
        inputNodes={[
          <Button
            value={form.slackKeywordMatchMode}
            color={form.slackKeywordMatchMode === "or" ? "secondary" : ""}
            className={s.input}
            onClick={(e) =>
              onFieldChange(
                "slackKeywordMatchMode",
                e.currentTarget.value === "or" ? "and" : "or",
              )
            }
          >
            {form.slackKeywordMatchMode === "or" ? "OR" : "AND"}
          </Button>,
        ]}
      />

      <SettingsTextField
        label="GitHub 토큰 (classic)"
        type="password"
        description="필요 권한: notification, repo"
        value={form.githubToken}
        onChange={(value) => onFieldChange("githubToken", value)}
      >
        <a
          href="https://github.com/settings/tokens"
          target="_blank"
          rel="noopener noreferrer"
        >
          Github 토큰 발급 링크
        </a>
      </SettingsTextField>

      <div className={s.doubleColumn}>
        <SettingsNumberField
          label="조회 가능한 이전 일수(일)"
          value={form.lookbackDays}
          onChange={(value) => onFieldChange("lookbackDays", value)}
        />
        <SettingsNumberField
          label="완료된 PR 표시 개수"
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
      <div className={s.checkboxes}>
        <SettingsCheckboxField
          label="GitHub 리뷰요청을 대기 중에 추가"
          checked={form.githubReviewRequestsEnabled}
          onChange={(value) =>
            onFieldChange("githubReviewRequestsEnabled", value)
          }
        />
        <SettingsCheckboxField
          label="나와 관련된 GitHub 새 소식만 보기"
          checked={form.githubRelatedUpdatesOnly}
          onChange={(value) => onFieldChange("githubRelatedUpdatesOnly", value)}
        />
        <SettingsCheckboxField
          label="리뷰 대기 중인 PR이 생겼을 때 알림"
          checked={form.notifyOnNewPending}
          onChange={(value) => onFieldChange("notifyOnNewPending", value)}
        />
        <SettingsCheckboxField
          label="새 소식이 생겼을 때 알림"
          checked={form.notifyOnNewUpdates}
          onChange={(value) => onFieldChange("notifyOnNewUpdates", value)}
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
      </div>
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
