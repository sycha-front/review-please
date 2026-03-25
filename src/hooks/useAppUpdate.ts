import { invoke } from "@tauri-apps/api/core";
import { startTransition, useState } from "react";

type UseAppUpdateResult = {
  isUpdating: boolean;
  error: string | null;
  runUpdate: (repoPath: string) => Promise<void>;
};

export function useAppUpdate(): UseAppUpdateResult {
  const [isUpdating, setIsUpdating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function runUpdate(repoPath: string) {
    setIsUpdating(true);
    try {
      await invoke("run_app_update", {
        payload: { repoPath },
      });
      startTransition(() => {
        setError(null);
      });
    } catch (updateError) {
      const message =
        updateError instanceof Error ? updateError.message : String(updateError);
      console.error("[review-please] failed to start app update", message);
      startTransition(() => {
        setError(message);
      });
      throw updateError;
    } finally {
      setIsUpdating(false);
    }
  }

  return {
    isUpdating,
    error,
    runUpdate,
  };
}
