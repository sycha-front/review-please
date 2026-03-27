import { invoke } from "@tauri-apps/api/core";
import { startTransition, useEffect, useRef, useState } from "react";

export type ReviewItem = {
  id: string;
  pr_key: string;
  pr_url: string;
  pr_title: string;
  repo_owner: string;
  repo_name: string;
  pr_number: number;
  pr_author_login: string | null;
  pr_merged_at: string | null;
  requester_slack_user_id: string;
  requester_display_name: string;
  slack_channel_id: string | null;
  slack_message_ts: string;
  slack_permalink: string | null;
  slack_text: string;
  deadline_date: string | null;
  status: boolean;
  is_status_manual: boolean;
  completed_at: string | null;
  completion_event_id: string | null;
  created_at: string;
  updated_at: string;
};

type BackendReviewItem = Omit<ReviewItem, "status"> & {
  status: string;
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
  read_at: string | null;
};

export type UpdateFeedItem = {
  id: string;
  source_event_ids: string[];
  pr_key: string;
  target_label: string;
  target_url: string;
  headline: string;
  summary: string | null;
  time_label: string;
  occurred_at: string;
  actor_login: string | null;
  actor_context: string;
  repo_label: string;
  activity_label: string;
  event_kind: string;
  event_count: number;
  unread_count: number;
  is_read: boolean;
  read_at: string | null;
};

export type TrayState = {
  pending_count: number;
  done_count: number;
  update_count: number;
  last_sync_at: string | null;
  status: string;
  last_error: string | null;
};

export type IntegrationStatus = {
  status: string;
  last_success_at: string | null;
  last_success_label: string | null;
  last_error: string | null;
};

export type IntegrationsSummary = {
  slack: IntegrationStatus;
  github: IntegrationStatus;
};

export type ReviewDump = {
  pending: ReviewItem[];
  done: ReviewItem[];
  update: ReviewItem[];
  update_feed: UpdateFeedItem[];
  recent_events: GithubEventItem[];
  tray_state: TrayState;
  integrations: IntegrationsSummary;
};

type BackendReviewDump = Omit<ReviewDump, "pending" | "done" | "update"> & {
  pending: BackendReviewItem[];
  done: BackendReviewItem[];
  update: BackendReviewItem[];
};

export type UseReviewDumpResult = {
  snapshot: ReviewDump | null;
  error: string | null;
  isLoading: boolean;
  refresh: () => Promise<void>;
  updateDeadline: (
    reviewRequestId: string,
    deadlineDate: string,
  ) => Promise<void>;
  updateStatus: (reviewRequestId: string, status: boolean) => Promise<void>;
  markUpdateRead: (eventIds: string[]) => Promise<void>;
  markAllUpdateRead: () => Promise<void>;
};

export function useReviewDump(pollIntervalMs = 10000): UseReviewDumpResult {
  const [snapshot, setSnapshot] = useState<ReviewDump | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const isMountedRef = useRef(false);

  async function loadSnapshot(showLoading = false) {
    if (showLoading) {
      setIsLoading(true);
    }

    try {
      const next = await invoke<BackendReviewDump>("get_review_dump");
      if (!isMountedRef.current) {
        return;
      }
      const snapshot = normalizeReviewDump(next);
      console.log("[review-please] review dump", next);
      startTransition(() => {
        setSnapshot(snapshot);
        setError(null);
      });
    } catch (loadError) {
      if (!isMountedRef.current) {
        return;
      }
      const message =
        loadError instanceof Error ? loadError.message : String(loadError);
      console.error("[review-please] failed to load review dump", message);
      startTransition(() => {
        setError(message);
      });
    } finally {
      if (showLoading && isMountedRef.current) {
        setIsLoading(false);
      }
    }
  }

  async function refresh() {
    await loadSnapshot();
  }

  async function updateDeadline(reviewRequestId: string, deadlineDate: string) {
    await invoke("update_review_deadline", {
      payload: { reviewRequestId, deadlineDate },
    });
    await refresh();
  }

  async function updateStatus(reviewRequestId: string, status: boolean) {
    await invoke("update_review_status", {
      payload: {
        reviewRequestId,
        status: status ? "done" : "pending",
      },
    });
    await refresh();
  }

  async function markUpdateRead(eventIds: string[]) {
    if (eventIds.length === 0) {
      return;
    }
    await invoke("mark_update_events_read", {
      payload: { eventIds },
    });
    await refresh();
  }

  async function markAllUpdateRead() {
    const unreadIds =
      snapshot?.update_feed
        .filter((item) => !item.is_read)
        .flatMap((item) => item.source_event_ids) ?? [];
    await markUpdateRead(unreadIds);
  }

  useEffect(() => {
    isMountedRef.current = true;

    void loadSnapshot(true);
    const intervalId = window.setInterval(() => {
      void loadSnapshot();
    }, pollIntervalMs);

    return () => {
      isMountedRef.current = false;
      window.clearInterval(intervalId);
    };
  }, [pollIntervalMs]);

  return {
    snapshot,
    error,
    isLoading,
    refresh,
    updateDeadline,
    updateStatus,
    markUpdateRead,
    markAllUpdateRead,
  };
}

function normalizeReviewItem(item: BackendReviewItem): ReviewItem {
  return {
    ...item,
    status: item.status === "done",
  };
}

function normalizeReviewDump(snapshot: BackendReviewDump): ReviewDump {
  return {
    ...snapshot,
    pending: snapshot.pending.map(normalizeReviewItem),
    done: snapshot.done.map(normalizeReviewItem),
    update: snapshot.update.map(normalizeReviewItem),
  };
}
