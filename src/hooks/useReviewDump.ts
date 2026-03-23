import { invoke } from "@tauri-apps/api/core";
import { startTransition, useEffect, useState } from "react";

export type ReviewItem = {
  id: string;
  pr_key: string;
  pr_url: string;
  pr_title: string;
  repo_owner: string;
  repo_name: string;
  pr_number: number;
  requester_slack_user_id: string;
  requester_display_name: string;
  slack_channel_id: string | null;
  slack_message_ts: string;
  slack_permalink: string | null;
  slack_text: string;
  deadline_date: string | null;
  status: string;
  completed_at: string | null;
  completion_event_id: string | null;
  created_at: string;
  updated_at: string;
};

export type GithubEventItem = {
  id: string;
  pr_key: string;
  notification_thread_id: string;
  notification_reason: string;
  event_kind: string;
  actor_login: string | null;
  actor_is_me: boolean;
  related_to_me: boolean;
  event_at: string;
  payload_json: string;
  created_at: string;
};

export type TrayState = {
  pending_count: number;
  done_count: number;
  last_sync_at: string | null;
  status: string;
  last_error: string | null;
};

export type ReviewDump = {
  pending: ReviewItem[];
  done: ReviewItem[];
  recent_events: GithubEventItem[];
  tray_state: TrayState;
};

export type UseReviewDumpResult = {
  snapshot: ReviewDump | null;
  error: string | null;
  isLoading: boolean;
};

export function useReviewDump(pollIntervalMs = 10000): UseReviewDumpResult {
  const [snapshot, setSnapshot] = useState<ReviewDump | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let isMounted = true;

    async function loadSnapshot() {
      try {
        const next = await invoke<ReviewDump>("get_review_dump");
        if (!isMounted) {
          return;
        }
        console.log("[pr-please] review dump", next);
        startTransition(() => {
          setSnapshot(next);
          setError(null);
        });
      } catch (loadError) {
        if (!isMounted) {
          return;
        }
        const message =
          loadError instanceof Error ? loadError.message : String(loadError);
        console.error("[pr-please] failed to load review dump", message);
        startTransition(() => {
          setError(message);
        });
      } finally {
        if (isMounted) {
          setIsLoading(false);
        }
      }
    }

    void loadSnapshot();
    const intervalId = window.setInterval(() => {
      void loadSnapshot();
    }, pollIntervalMs);

    return () => {
      isMounted = false;
      window.clearInterval(intervalId);
    };
  }, [pollIntervalMs]);

  return {
    snapshot,
    error,
    isLoading,
  };
}
