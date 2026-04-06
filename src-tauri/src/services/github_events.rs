use std::{collections::HashSet, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};

use crate::{
    config::AppConfig,
    db::ReviewStore,
    models::{
        utc_now_string, GithubNotificationThread, GithubPullRef, PullRequestMetadata,
        ReviewRequest, SyncState,
    },
    providers::{github::is_access_denied_error, GithubProvider},
    services::review_state::{should_mark_done, update_activity_label},
};

pub const GITHUB_SYNC_SOURCE: &str = "github_notifications";
const FULL_NOTIFICATIONS_REFRESH_INTERVAL_MINUTES: i64 = 15;

#[derive(Debug, Default)]
pub struct GithubSyncOutcome {
    pub new_pending_count: u64,
    pub completed_request_count: u64,
    pub completed_pr_keys: Vec<String>,
    pub new_update_pr_keys: Vec<String>,
}

fn notification_updated_ts(thread: &GithubNotificationThread) -> String {
    thread
        .updated_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| format!("{}.000000", value.with_timezone(&Utc).timestamp()))
        .unwrap_or_else(|| format!("{}.000000", Utc::now().timestamp()))
}

fn build_github_review_request(
    thread: &GithubNotificationThread,
    pull: &GithubPullRef,
    metadata: Option<&PullRequestMetadata>,
) -> ReviewRequest {
    let pr_title = metadata
        .map(|value| value.title.clone())
        .or_else(|| thread.subject_title.clone())
        .unwrap_or_else(|| pull.key());
    let summary = if let Some(title) = &thread.subject_title {
        format!("GitHub에서 리뷰 요청이 왔습니다.\n{title}")
    } else {
        "GitHub에서 리뷰 요청이 왔습니다.".to_string()
    };

    ReviewRequest::new_github_review_request(
        pull,
        pr_title,
        metadata.and_then(|value| value.author_login.clone()),
        metadata.and_then(|value| value.merged_at.clone()),
        notification_updated_ts(thread),
        summary,
    )
}

fn should_force_full_refresh(last_full_refresh_at: Option<&str>, now: DateTime<Utc>) -> bool {
    let Some(last_full_refresh_at) = last_full_refresh_at else {
        return true;
    };
    let Ok(last_full_refresh_at) = DateTime::parse_from_rfc3339(last_full_refresh_at) else {
        return true;
    };
    now.signed_duration_since(last_full_refresh_at.with_timezone(&Utc))
        >= Duration::minutes(FULL_NOTIFICATIONS_REFRESH_INTERVAL_MINUTES)
}

pub fn run(
    config: &AppConfig,
    store: Arc<dyn ReviewStore>,
    github_provider: Arc<dyn GithubProvider>,
) -> Result<GithubSyncOutcome> {
    store.prune_history(config.lookback_days)?;
    let sync_state = store.get_sync_state(GITHUB_SYNC_SOURCE)?;
    let mut tracked: HashSet<String> = store.tracked_pr_keys()?.into_iter().collect();
    let now = Utc::now();
    let refresh_state =
        if should_force_full_refresh(sync_state.github_last_full_refresh_at.as_deref(), now) {
            SyncState::new(GITHUB_SYNC_SOURCE)
        } else {
            sync_state.clone()
        };
    let mut poll_result = github_provider
        .fetch_notifications(&refresh_state, config.github_min_poll_interval_seconds)?;
    if poll_result.not_modified && store.github_event_count()? == 0 {
        // Recover from an empty local cache even when GitHub says nothing changed
        // since the last conditional request.
        poll_result = github_provider.fetch_notifications(
            &SyncState::new(GITHUB_SYNC_SOURCE),
            config.github_min_poll_interval_seconds,
        )?;
    }

    let mut next_state = SyncState::new(GITHUB_SYNC_SOURCE);
    next_state.last_polled_at = Some(utc_now_string());
    next_state.github_etag = poll_result.etag.clone().or(sync_state.github_etag);
    next_state.github_last_modified = poll_result
        .last_modified
        .clone()
        .or(sync_state.github_last_modified);
    next_state.github_poll_interval_seconds = poll_result
        .poll_interval_seconds
        .or(sync_state.github_poll_interval_seconds);
    next_state.github_last_full_refresh_at = if poll_result.not_modified {
        sync_state.github_last_full_refresh_at
    } else {
        next_state.last_polled_at.clone()
    };

    if poll_result.not_modified {
        next_state.last_success_at = next_state.last_polled_at.clone();
        store.save_sync_state(&next_state)?;
        return Ok(GithubSyncOutcome::default());
    }

    let current_user_login = github_provider.current_user_login()?;
    let mut outcome = GithubSyncOutcome::default();

    for thread in poll_result.threads {
        let pull = match thread.pull.as_ref() {
            Some(pull) if thread.subject_type == "PullRequest" => pull,
            _ => continue,
        };
        let metadata = match github_provider.fetch_pr_metadata(pull) {
            Ok(metadata) => {
                let _ = store.refresh_review_request_pr_metadata(
                    &pull.key(),
                    &metadata.title,
                    metadata.author_login.as_deref(),
                    metadata.merged_at.as_deref(),
                );
                Some(metadata)
            }
            Err(error) if is_access_denied_error(&error) => {
                eprintln!(
                    "Skipping inaccessible notification PR {}: {error}",
                    pull.key()
                );
                continue;
            }
            Err(error) => {
                eprintln!("GitHub metadata refresh failed for {}: {error}", pull.key());
                None
            }
        };
        let mut created_pending_from_github = false;
        if config.github_review_requests_enabled
            && thread.reason == "review_requested"
            && !tracked.contains(&pull.key())
        {
            let request = build_github_review_request(&thread, pull, metadata.as_ref());
            if store.upsert_review_request(&request)? {
                outcome.new_pending_count += 1;
                created_pending_from_github = true;
            }
            tracked.insert(pull.key());
        }
        let since = store.latest_event_at_for_pr(&pull.key())?;
        let is_my_pr = metadata
            .as_ref()
            .and_then(|value| value.author_login.as_deref())
            .map(|login| login.eq_ignore_ascii_case(&current_user_login))
            .unwrap_or(false);
        let include_comment_events = is_my_pr
            || matches!(
                thread.reason.as_str(),
                "author" | "mention" | "team_mention" | "comment"
            )
            || store.should_fetch_comment_events(&pull.key(), &config.github_username)?;
        let events = match github_provider.fetch_events_for_thread(
            &thread,
            since.as_deref(),
            &current_user_login,
            include_comment_events,
        ) {
            Ok(events) => events,
            Err(error) if is_access_denied_error(&error) => {
                eprintln!(
                    "Skipping inaccessible notification events for {}: {error}",
                    pull.key()
                );
                continue;
            }
            Err(error) => return Err(error),
        };
        for mut event in events {
            if let Some(metadata) = &metadata {
                event.pr_title = Some(metadata.title.clone());
                event.pr_author_login = metadata.author_login.clone();
            }
            let inserted = store.upsert_github_event(&event)?;
            if inserted
                && !(created_pending_from_github && event.notification_reason == "review_requested")
                && update_activity_label(
                    &event,
                    &current_user_login,
                    metadata
                        .as_ref()
                        .and_then(|value| value.author_login.as_deref())
                        .or(event.pr_author_login.as_deref()),
                    config.github_related_updates_only,
                )
                .is_some()
                && !outcome.new_update_pr_keys.contains(&event.pr_key)
            {
                outcome.new_update_pr_keys.push(event.pr_key.clone());
            }
            if tracked.contains(&event.pr_key)
                && should_mark_done(&event.event_kind, event.actor_is_me)
            {
                let completed = store.mark_requests_done_by_pr_key(
                    &event.pr_key,
                    &event.id,
                    &event.event_at,
                )?;
                if completed > 0 {
                    outcome.completed_request_count += completed;
                    if !outcome.completed_pr_keys.contains(&event.pr_key) {
                        outcome.completed_pr_keys.push(event.pr_key.clone());
                    }
                }
            }
        }
    }

    next_state.last_success_at = next_state.last_polled_at.clone();
    store.save_sync_state(&next_state)?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use crate::models::{GithubNotificationThread, GithubPullRef, PullRequestMetadata};

    use super::{build_github_review_request, notification_updated_ts, should_force_full_refresh};

    #[test]
    fn builds_pending_request_from_github_review_notification() {
        let thread = GithubNotificationThread {
            id: "thread-1".to_string(),
            unread: true,
            last_read_at: None,
            reason: "review_requested".to_string(),
            subject_type: "PullRequest".to_string(),
            subject_title: Some("Feature PR".to_string()),
            pull: Some(GithubPullRef {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
                number: 42,
            }),
            updated_at: Some("2026-04-06T02:30:00Z".to_string()),
        };
        let pull = thread.pull.clone().expect("pull");
        let metadata = PullRequestMetadata {
            title: "Actual PR title".to_string(),
            author_login: Some("author".to_string()),
            merged_at: None,
        };

        let request = build_github_review_request(&thread, &pull, Some(&metadata));

        assert_eq!(request.pr_key, "owner/repo#42");
        assert_eq!(request.pr_title, "Actual PR title");
        assert_eq!(request.requester_display_name, "GitHub 리뷰 요청");
        assert_eq!(request.slack_message_ts, "1775442600.000000");
        assert!(request
            .slack_text
            .contains("GitHub에서 리뷰 요청이 왔습니다."));
    }

    #[test]
    fn converts_notification_timestamp_to_sortable_request_timestamp() {
        let thread = GithubNotificationThread {
            id: "thread-1".to_string(),
            unread: true,
            last_read_at: None,
            reason: "review_requested".to_string(),
            subject_type: "PullRequest".to_string(),
            subject_title: None,
            pull: None,
            updated_at: Some("2026-04-06T02:30:00Z".to_string()),
        };

        assert_eq!(notification_updated_ts(&thread), "1775442600.000000");
    }

    #[test]
    fn forces_full_refresh_without_previous_refresh_time() {
        let now = DateTime::parse_from_rfc3339("2026-04-06T05:45:00Z")
            .expect("timestamp")
            .with_timezone(&Utc);

        assert!(should_force_full_refresh(None, now));
    }

    #[test]
    fn skips_full_refresh_when_previous_refresh_is_recent() {
        let now = DateTime::parse_from_rfc3339("2026-04-06T05:45:00Z")
            .expect("timestamp")
            .with_timezone(&Utc);

        assert!(!should_force_full_refresh(
            Some("2026-04-06T05:35:01Z"),
            now
        ));
    }

    #[test]
    fn forces_full_refresh_when_previous_refresh_is_stale() {
        let now = DateTime::parse_from_rfc3339("2026-04-06T05:45:00Z")
            .expect("timestamp")
            .with_timezone(&Utc);

        assert!(should_force_full_refresh(Some("2026-04-06T05:29:59Z"), now));
    }
}
