import { invoke } from "@tauri-apps/api/core";
import { startTransition, useEffect, useRef, useState } from "react";

export type ReleaseStatus = {
  currentVersion: string;
  latestVersion: string | null;
  latestReleaseUrl: string | null;
  publishedAt: string | null;
  isUpdateAvailable: boolean;
  error: string | null;
};

type UseReleaseStatusResult = {
  releaseStatus: ReleaseStatus | null;
  isLoading: boolean;
  refresh: () => Promise<void>;
};

export function useReleaseStatus(
  pollIntervalMs = 1000 * 60 * 10,
): UseReleaseStatusResult {
  const [releaseStatus, setReleaseStatus] = useState<ReleaseStatus | null>(
    null,
  );
  const [isLoading, setIsLoading] = useState(true);
  const isMountedRef = useRef(false);

  async function loadStatus(showLoading = false) {
    if (showLoading) {
      setIsLoading(true);
    }

    try {
      const next = await invoke<ReleaseStatus>("get_release_status");
      if (!isMountedRef.current) {
        return;
      }
      startTransition(() => {
        setReleaseStatus(next);
      });
    } catch (error) {
      if (!isMountedRef.current) {
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      console.error("[review-please] failed to load release status", message);
      startTransition(() => {
        setReleaseStatus({
          currentVersion: "",
          latestVersion: null,
          latestReleaseUrl: null,
          publishedAt: null,
          isUpdateAvailable: false,
          error: message,
        });
      });
    } finally {
      if (showLoading && isMountedRef.current) {
        setIsLoading(false);
      }
    }
  }

  useEffect(() => {
    isMountedRef.current = true;
    void loadStatus(true);

    const intervalId = window.setInterval(() => {
      void loadStatus();
    }, pollIntervalMs);

    return () => {
      isMountedRef.current = false;
      window.clearInterval(intervalId);
    };
  }, [pollIntervalMs]);

  return {
    releaseStatus,
    isLoading,
    refresh: () => loadStatus(),
  };
}
