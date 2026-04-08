use std::{collections::HashSet, sync::Arc};

use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::{
    config::AppConfig,
    db::ReviewStore,
    models::{
        utc_now_string, GithubEvent, GithubNotificationThread, GithubPullRef,
        NotificationsPollResult, PullRequestMetadata, ReviewRequest, SyncState,
    },
    providers::{github::is_access_denied_error, GithubProvider},
    services::review_state::{should_mark_done, update_activity_label},
};

pub const GITHUB_SYNC_SOURCE: &str = "github_notifications";
const REVIEW_REQUEST_REASON: &str = "review_requested";

#[derive(Debug, Default)]
pub struct GithubSyncOutcome {
    pub new_pending_count: u64,
    pub completed_request_count: u64,
    pub completed_pr_keys: Vec<String>,
    pub new_update_pr_keys: Vec<String>,
}

enum ThreadMetadata {
    Skip,
    Value(Option<PullRequestMetadata>),
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
    let summary = thread
        .subject_title
        .as_ref()
        .map(|title| format!("GitHub에서 리뷰 요청이 왔습니다.\n{title}"))
        .unwrap_or_else(|| "GitHub에서 리뷰 요청이 왔습니다.".to_string());

    ReviewRequest::new_github_review_request(
        pull,
        pr_title,
        metadata.and_then(|value| value.author_login.clone()),
        metadata.and_then(|value| value.merged_at.clone()),
        notification_updated_ts(thread),
        summary,
    )
}

fn update_review_request_metadata(
    store: &dyn ReviewStore,
    pull: &GithubPullRef,
    metadata: &PullRequestMetadata,
) {
    let _ = store.refresh_review_request_pr_metadata(
        &pull.key(),
        &metadata.title,
        metadata.author_login.as_deref(),
        metadata.merged_at.as_deref(),
    );
}

fn enrich_event_with_metadata(event: &mut GithubEvent, metadata: Option<&PullRequestMetadata>) {
    if let Some(metadata) = metadata {
        event.pr_title = Some(metadata.title.clone());
        event.pr_author_login = metadata.author_login.clone();
    }
}

fn push_unique_pr_key(keys: &mut Vec<String>, pr_key: &str) {
    if !keys.iter().any(|key| key == pr_key) {
        keys.push(pr_key.to_string());
    }
}

fn is_review_requested_thread(thread: &GithubNotificationThread) -> bool {
    thread.reason == REVIEW_REQUEST_REASON
}

fn is_notification_pr_thread(thread: &GithubNotificationThread) -> Option<&GithubPullRef> {
    match thread.pull.as_ref() {
        Some(pull) if thread.subject_type == "PullRequest" => Some(pull),
        _ => None,
    }
}

fn should_record_update(
    inserted: bool,
    created_pending_from_github: bool,
    event: &GithubEvent,
    current_user_login: &str,
    metadata: Option<&PullRequestMetadata>,
    github_related_updates_only: bool,
) -> bool {
    inserted
        && !(created_pending_from_github && event.notification_reason == REVIEW_REQUEST_REASON)
        && update_activity_label(
            event,
            current_user_login,
            metadata
                .and_then(|value| value.author_login.as_deref())
                .or(event.pr_author_login.as_deref()),
            github_related_updates_only,
        )
        .is_some()
}

fn should_include_comment_events(
    store: &dyn ReviewStore,
    pull: &GithubPullRef,
    thread: &GithubNotificationThread,
    metadata: Option<&PullRequestMetadata>,
    current_user_login: &str,
    github_username: &str,
) -> Result<bool> {
    let is_my_pr = metadata
        .and_then(|value| value.author_login.as_deref())
        .map(|login| login.eq_ignore_ascii_case(current_user_login))
        .unwrap_or(false);

    Ok(is_my_pr
        || matches!(
            thread.reason.as_str(),
            "author" | "mention" | "team_mention" | "comment"
        )
        || store.should_fetch_comment_events(&pull.key(), github_username)?)
}

fn fetch_thread_metadata(
    github_provider: &dyn GithubProvider,
    store: &dyn ReviewStore,
    pull: &GithubPullRef,
) -> Result<ThreadMetadata> {
    match github_provider.fetch_pr_metadata(pull) {
        Ok(metadata) => {
            update_review_request_metadata(store, pull, &metadata);
            Ok(ThreadMetadata::Value(Some(metadata)))
        }
        Err(error) if is_access_denied_error(&error) => {
            eprintln!(
                "Skipping inaccessible notification PR {}: {error}",
                pull.key()
            );
            Ok(ThreadMetadata::Skip)
        }
        Err(error) => {
            eprintln!("GitHub metadata refresh failed for {}: {error}", pull.key());
            Ok(ThreadMetadata::Value(None))
        }
    }
}

fn fetch_thread_events(
    github_provider: &dyn GithubProvider,
    thread: &GithubNotificationThread,
    pull: &GithubPullRef,
    since: Option<&str>,
    current_user_login: &str,
    include_comment_events: bool,
) -> Result<Option<Vec<GithubEvent>>> {
    match github_provider.fetch_events_for_thread(
        thread,
        since,
        current_user_login,
        include_comment_events,
    ) {
        Ok(events) => Ok(Some(events)),
        Err(error) if is_access_denied_error(&error) => {
            eprintln!(
                "Skipping inaccessible notification events for {}: {error}",
                pull.key()
            );
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn try_insert_review_request(
    config: &AppConfig,
    store: &dyn ReviewStore,
    thread: &GithubNotificationThread,
    pull: &GithubPullRef,
    metadata: Option<&PullRequestMetadata>,
    tracked: &mut HashSet<String>,
    outcome: &mut GithubSyncOutcome,
) -> Result<bool> {
    if !config.github_review_requests_enabled
        || !is_review_requested_thread(thread)
        || tracked.contains(&pull.key())
    {
        return Ok(false);
    }

    let request = build_github_review_request(thread, pull, metadata);
    let inserted = store.upsert_review_request(&request)?;
    if inserted {
        outcome.new_pending_count += 1;
    }
    tracked.insert(pull.key());
    Ok(inserted)
}

fn handle_event(
    config: &AppConfig,
    store: &dyn ReviewStore,
    current_user_login: &str,
    tracked: &HashSet<String>,
    outcome: &mut GithubSyncOutcome,
    metadata: Option<&PullRequestMetadata>,
    created_pending_from_github: bool,
    mut event: GithubEvent,
) -> Result<()> {
    enrich_event_with_metadata(&mut event, metadata);
    let inserted = store.upsert_github_event(&event)?;
    if should_record_update(
        inserted,
        created_pending_from_github,
        &event,
        current_user_login,
        metadata,
        config.github_related_updates_only,
    ) {
        push_unique_pr_key(&mut outcome.new_update_pr_keys, &event.pr_key);
    }
    if tracked.contains(&event.pr_key) && should_mark_done(&event.event_kind, event.actor_is_me) {
        let completed =
            store.mark_requests_done_by_pr_key(&event.pr_key, &event.id, &event.event_at)?;
        if completed > 0 {
            outcome.completed_request_count += completed;
            push_unique_pr_key(&mut outcome.completed_pr_keys, &event.pr_key);
        }
    }

    Ok(())
}

fn process_poll_result(
    config: &AppConfig,
    store: &dyn ReviewStore,
    github_provider: &dyn GithubProvider,
    current_user_login: &str,
    tracked: &mut HashSet<String>,
    outcome: &mut GithubSyncOutcome,
    poll_result: NotificationsPollResult,
) -> Result<()> {
    for thread in poll_result.threads {
        let Some(pull) = is_notification_pr_thread(&thread) else {
            continue;
        };
        let metadata = match fetch_thread_metadata(github_provider, store, pull)? {
            ThreadMetadata::Skip => continue,
            ThreadMetadata::Value(metadata) => metadata,
        };
        let created_pending_from_github = try_insert_review_request(
            config,
            store,
            &thread,
            pull,
            metadata.as_ref(),
            tracked,
            outcome,
        )?;
        let since = store.latest_event_at_for_pr(&pull.key())?;
        let include_comment_events = should_include_comment_events(
            store,
            pull,
            &thread,
            metadata.as_ref(),
            current_user_login,
            &config.github_username,
        )?;
        let Some(events) = fetch_thread_events(
            github_provider,
            &thread,
            pull,
            since.as_deref(),
            current_user_login,
            include_comment_events,
        )?
        else {
            continue;
        };

        for event in events {
            handle_event(
                config,
                store,
                current_user_login,
                tracked,
                outcome,
                metadata.as_ref(),
                created_pending_from_github,
                event,
            )?;
        }
    }

    Ok(())
}

fn update_sync_state(sync_state: SyncState, poll_result: &NotificationsPollResult) -> SyncState {
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
    next_state
}

fn finalize_sync_state(store: &dyn ReviewStore, mut next_state: SyncState) -> Result<()> {
    next_state.last_success_at = next_state.last_polled_at.clone();
    store.save_sync_state(&next_state)
}

pub fn run(
    config: &AppConfig,
    store: Arc<dyn ReviewStore>,
    github_provider: Arc<dyn GithubProvider>,
) -> Result<GithubSyncOutcome> {
    store.prune_history(config.lookback_days)?;
    let sync_state = store.get_sync_state(GITHUB_SYNC_SOURCE)?;
    let mut tracked: HashSet<String> = store.tracked_pr_keys()?.into_iter().collect();
    let mut poll_result = github_provider
        .fetch_notifications(&sync_state, config.github_min_poll_interval_seconds)?;
    if poll_result.not_modified && store.github_event_count()? == 0 {
        // Recover from an empty local cache even when GitHub says nothing changed
        // since the last conditional request.
        poll_result = github_provider.fetch_notifications(
            &SyncState::new(GITHUB_SYNC_SOURCE),
            config.github_min_poll_interval_seconds,
        )?;
    }

    let next_state = update_sync_state(sync_state, &poll_result);
    if poll_result.not_modified {
        finalize_sync_state(store.as_ref(), next_state)?;
        return Ok(GithubSyncOutcome::default());
    }

    let current_user_login = github_provider.current_user_login()?;
    let mut outcome = GithubSyncOutcome::default();
    process_poll_result(
        config,
        store.as_ref(),
        github_provider.as_ref(),
        &current_user_login,
        &mut tracked,
        &mut outcome,
        poll_result,
    )?;
    finalize_sync_state(store.as_ref(), next_state)?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use crate::models::{GithubNotificationThread, GithubPullRef, PullRequestMetadata};

    use super::{build_github_review_request, notification_updated_ts};

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
}
