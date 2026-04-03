export type SlackAuthMode = "oauth" | "manual" | "disconnected";

export type SettingsPayload = {
  slackMentionKeyword: string;
  slackUsername: string;
  lookbackDays: number;
  slackPollIntervalSeconds: number;
  githubMinPollIntervalSeconds: number;
  doneMenuLimit: number;
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
