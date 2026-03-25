export type SettingsPayload = {
  slackMentionKeyword: string;
  slackUsername: string;
  githubUsername: string;
  repoPath: string;
  lookbackDays: number;
  slackPollIntervalSeconds: number;
  githubMinPollIntervalSeconds: number;
  doneMenuLimit: number;
  notifyOnNewPending: boolean;
  notifyOnDone: boolean;
  notifyOnErrors: boolean;
  launchAtLogin: boolean;
  slackToken: string;
  githubToken: string;
};
