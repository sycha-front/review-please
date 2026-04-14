import { invoke } from "@tauri-apps/api/core";
import { startTransition, useEffect, useState } from "react";

import type { SettingsPayload } from "../types/settings";

export type { SettingsPayload };

export type UseSettingsResult = {
  settings: SettingsPayload | null;
  error: string | null;
  isLoading: boolean;
  isSaving: boolean;
  isSlackConnecting: boolean;
  slackAuthorizeUrl: string | null;
  saveSettings: (next: SettingsPayload) => Promise<SettingsPayload>;
  connectSlack: () => Promise<void>;
  disconnectSlack: () => Promise<void>;
};

type StartSlackOauthResponse = {
  sessionId: string;
  sessionSecret: string;
  authorizeUrl: string;
  expiresAt: string;
};

type PollSlackOauthResponse = {
  status: "pending" | "completed" | "expired" | "failed";
  error: string | null;
  settings: SettingsPayload | null;
};

function wait(ms: number) {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

export function useSettings(): UseSettingsResult {
  const [settings, setSettings] = useState<SettingsPayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [isSlackConnecting, setIsSlackConnecting] = useState(false);
  const [slackAuthorizeUrl, setSlackAuthorizeUrl] = useState<string | null>(null);

  useEffect(() => {
    let isMounted = true;

    async function loadSettings(showLoading = true) {
      if (showLoading && isMounted) {
        setIsLoading(true);
      }
      try {
        const next = await invoke<SettingsPayload>("get_settings");
        if (!isMounted) {
          return null;
        }
        startTransition(() => {
          setSettings(next);
          setError(null);
        });
        return next;
      } catch (loadError) {
        if (!isMounted) {
          return null;
        }
        const message =
          loadError instanceof Error ? loadError.message : String(loadError);
        console.error("[review-please] failed to load settings", message);
        startTransition(() => {
          setError(message);
        });
        return null;
      } finally {
        if (showLoading && isMounted) {
          setIsLoading(false);
        }
      }
    }

    void loadSettings();

    return () => {
      isMounted = false;
    };
  }, []);

  async function saveSettings(next: SettingsPayload) {
    setIsSaving(true);
    try {
      const saved = await invoke<SettingsPayload>("save_settings", {
        payload: next,
      });
      // console.log("[review-please] saved settings", saved);
      startTransition(() => {
        setSettings(saved);
        setError(null);
      });
      return saved;
    } catch (saveError) {
      const message =
        saveError instanceof Error ? saveError.message : String(saveError);
      console.error("[review-please] failed to save settings", message);
      startTransition(() => {
        setError(message);
      });
      throw saveError;
    } finally {
      setIsSaving(false);
    }
  }

  async function connectSlack() {
    setIsSlackConnecting(true);
    try {
      startTransition(() => {
        setError(null);
      });
      const started = await invoke<StartSlackOauthResponse>("start_slack_oauth");
      setSlackAuthorizeUrl(started.authorizeUrl);
      const expiresAt = Date.parse(started.expiresAt);

      while (Date.now() < expiresAt + 5000) {
        await wait(1500);
        const polled = await invoke<PollSlackOauthResponse>("poll_slack_oauth", {
          payload: {
            sessionId: started.sessionId,
            sessionSecret: started.sessionSecret,
          },
        });

        if (polled.status === "pending") {
          continue;
        }

        if (polled.status === "completed" && polled.settings) {
          startTransition(() => {
            setSettings(polled.settings);
            setError(null);
          });
          setSlackAuthorizeUrl(null);
          return;
        }

        throw new Error(
          polled.error ?? "Slack 연결에 실패했어요. 다시 시도해주세요.",
        );
      }

      throw new Error("Slack 연결 시간이 만료되었어요. 다시 시도해주세요.");
    } catch (connectError) {
      const message =
        connectError instanceof Error
          ? connectError.message
          : String(connectError);
      console.error("[review-please] failed to connect slack", message);
      startTransition(() => {
        setError(message);
      });
      throw connectError;
    } finally {
      setIsSlackConnecting(false);
    }
  }

  async function disconnectSlack() {
    setIsSlackConnecting(true);
    try {
      const next = await invoke<SettingsPayload>("disconnect_slack_oauth");
      startTransition(() => {
        setSettings(next);
        setError(null);
      });
      setSlackAuthorizeUrl(null);
    } catch (disconnectError) {
      const message =
        disconnectError instanceof Error
          ? disconnectError.message
          : String(disconnectError);
      console.error("[review-please] failed to disconnect slack", message);
      startTransition(() => {
        setError(message);
      });
      throw disconnectError;
    } finally {
      setIsSlackConnecting(false);
    }
  }

  return {
    settings,
    error,
    isLoading,
    isSaving,
    isSlackConnecting,
    slackAuthorizeUrl,
    saveSettings,
    connectSlack,
    disconnectSlack,
  };
}
