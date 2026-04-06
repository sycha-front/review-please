export type SlackAuthMode = "oauth" | "manual" | "disconnected";
export type SlackKeywordMatchMode = "or" | "and";

export type SettingsPayload = {
  slackMentionKeyword: string;
  slackKeywordMatchMode: SlackKeywordMatchMode;
  slackUsername: string;
  lookbackDays: number;
  slackPollIntervalSeconds: number;
  githubMinPollIntervalSeconds: number;
  doneMenuLimit: number;
  githubReviewRequestsEnabled: boolean;
  notifyOnNewPending: boolean;
  notifyOnNewUpdates: boolean;
  notifyOnDone: boolean;
  notifyOnErrors: boolean;
  hideOnlyOnClose: boolean;
  slackAuthMode: SlackAuthMode;
  slackConnected: boolean;
  slackConnectedUser: string | null;
  slackConnectedWorkspace: string | null;
  slackToken: string;
  githubToken: string;
};
