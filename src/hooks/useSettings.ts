import { invoke } from "@tauri-apps/api/core";
import { startTransition, useEffect, useState } from "react";

import type { SettingsPayload } from "../types/settings";

export type { SettingsPayload };

export type UseSettingsResult = {
  settings: SettingsPayload | null;
  error: string | null;
  isLoading: boolean;
  isSaving: boolean;
  saveSettings: (next: SettingsPayload) => Promise<SettingsPayload>;
};

export function useSettings(): UseSettingsResult {
  const [settings, setSettings] = useState<SettingsPayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    let isMounted = true;

    async function loadSettings() {
      try {
        const next = await invoke<SettingsPayload>("get_settings");
        if (!isMounted) {
          return;
        }
        console.log("[review-please] settings", next);
        startTransition(() => {
          setSettings(next);
          setError(null);
        });
      } catch (loadError) {
        if (!isMounted) {
          return;
        }
        const message =
          loadError instanceof Error ? loadError.message : String(loadError);
        console.error("[review-please] failed to load settings", message);
        startTransition(() => {
          setError(message);
        });
      } finally {
        if (isMounted) {
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
      console.log("[review-please] saved settings", saved);
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

  return {
    settings,
    error,
    isLoading,
    isSaving,
    saveSettings,
  };
}
