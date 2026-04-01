use std::{collections::HashSet, sync::Arc};

use anyhow::Result;

use crate::{
    config::AppConfig,
    db::ReviewStore,
    models::{utc_now_string, SyncState},
    providers::{github::is_access_denied_error, GithubProvider},
    services::review_state::should_mark_done,
};

pub const GITHUB_SYNC_SOURCE: &str = "github_notifications";

#[derive(Debug, Default)]
pub struct GithubSyncOutcome {
    pub completed_request_count: u64,
    pub completed_pr_keys: Vec<String>,
}

pub fn run(
    config: &AppConfig,
    store: Arc<dyn ReviewStore>,
    github_provider: Arc<dyn GithubProvider>,
) -> Result<GithubSyncOutcome> {
    store.prune_history(config.lookback_days)?;
    let sync_state = store.get_sync_state(GITHUB_SYNC_SOURCE)?;
    let tracked: HashSet<String> = store.tracked_pr_keys()?.into_iter().collect();
    let poll_result = github_provider
        .fetch_notifications(&sync_state, config.github_min_poll_interval_seconds)?;

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

    if poll_result.not_modified {
        next_state.last_success_at = next_state.last_polled_at.clone();
        store.save_sync_state(&next_state)?;
        return Ok(GithubSyncOutcome::default());
    }

    let current_user_login = github_provider.current_user_login()?;
    let mut outcome = GithubSyncOutcome::default();

    for thread in poll_result.threads {
        let pull = match thread.pull.as_ref() {
            Some(pull) if thread.subject_type == "PullRequest" && tracked.contains(&pull.key()) => {
                pull
            }
            _ => continue,
        };
        match github_provider.fetch_pr_metadata(pull) {
            Ok(metadata) => {
                let _ = store.refresh_review_request_pr_metadata(
                    &pull.key(),
                    &metadata.title,
                    metadata.author_login.as_deref(),
                    metadata.merged_at.as_deref(),
                );
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
            }
        }
        let since = store.latest_event_at_for_pr(&pull.key())?;
        let include_comment_events =
            store.should_fetch_comment_events(&pull.key(), &config.github_username)?;
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
        for event in events {
            let _ = store.upsert_github_event(&event)?;
            if should_mark_done(&event.event_kind, event.actor_is_me) {
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
