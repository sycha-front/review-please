export type SettingsPayload = {
  slackMentionKeyword: string;
  slackUsername: string;
  githubUsername: string;
  lookbackDays: number;
  slackPollIntervalSeconds: number;
  githubMinPollIntervalSeconds: number;
  doneMenuLimit: number;
  notifyOnNewPending: boolean;
  notifyOnDone: boolean;
  notifyOnErrors: boolean;
  slackToken: string;
  githubToken: string;
};
