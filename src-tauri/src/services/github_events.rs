use std::{collections::HashSet, sync::Arc};

use anyhow::Result;

use crate::{
    config::AppConfig,
    db::ReviewStore,
    models::{utc_now_string, GithubEvent, GithubPullRef, PullRequestMetadata, SyncState},
    providers::{github::is_access_denied_error, GithubProvider},
    services::review_state::{should_mark_done, update_activity_label},
};

pub const GITHUB_SYNC_SOURCE: &str = "github_notifications";

#[derive(Debug, Default)]
pub struct GithubSyncOutcome {
    pub completed_request_count: u64,
    pub completed_pr_keys: Vec<String>,
    pub new_update_pr_keys: Vec<String>,
}

fn parse_pull_key(pr_key: &str) -> Option<GithubPullRef> {
    let (repo_path, number) = pr_key.rsplit_once('#')?;
    let (owner, repo) = repo_path.split_once('/')?;
    Some(GithubPullRef {
        owner: owner.to_string(),
        repo: repo.to_string(),
        number: number.parse().ok()?,
    })
}

fn record_event(
    store: &Arc<dyn ReviewStore>,
    tracked: &HashSet<String>,
    current_user_login: &str,
    metadata: Option<&PullRequestMetadata>,
    event: &mut GithubEvent,
    outcome: &mut GithubSyncOutcome,
) -> Result<()> {
    if let Some(metadata) = metadata {
        event.pr_title = Some(metadata.title.clone());
        event.pr_author_login = metadata.author_login.clone();
    }

    let inserted = store.upsert_github_event(event)?;
    if inserted
        && update_activity_label(
            event,
            current_user_login,
            metadata
                .and_then(|value| value.author_login.as_deref())
                .or(event.pr_author_login.as_deref()),
        )
        .is_some()
        && !outcome.new_update_pr_keys.contains(&event.pr_key)
    {
        outcome.new_update_pr_keys.push(event.pr_key.clone());
    }
    if tracked.contains(&event.pr_key) && should_mark_done(&event.event_kind, event.actor_is_me) {
        let completed =
            store.mark_requests_done_by_pr_key(&event.pr_key, &event.id, &event.event_at)?;
        if completed > 0 {
            outcome.completed_request_count += completed;
            if !outcome.completed_pr_keys.contains(&event.pr_key) {
                outcome.completed_pr_keys.push(event.pr_key.clone());
            }
        }
    }
    Ok(())
}

fn scan_tracked_prs(
    store: &Arc<dyn ReviewStore>,
    github_provider: &Arc<dyn GithubProvider>,
    tracked: &HashSet<String>,
    current_user_login: &str,
    outcome: &mut GithubSyncOutcome,
) -> Result<()> {
    for pr_key in tracked {
        let Some(pull) = parse_pull_key(pr_key) else {
            eprintln!("Skipping invalid tracked PR key: {pr_key}");
            continue;
        };

        let metadata = match github_provider.fetch_pr_metadata(&pull) {
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
                eprintln!("Skipping inaccessible tracked PR {}: {error}", pull.key());
                continue;
            }
            Err(error) => {
                eprintln!("GitHub metadata refresh failed for tracked {}: {error}", pull.key());
                None
            }
        };

        let Some(mut event) =
            github_provider.fetch_latest_approval_event(&pull, current_user_login)?
        else {
            continue;
        };
        record_event(
            store,
            tracked,
            current_user_login,
            metadata.as_ref(),
            &mut event,
            outcome,
        )?;
    }
    Ok(())
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

    let current_user_login = github_provider.current_user_login()?;
    let mut outcome = GithubSyncOutcome::default();

    if poll_result.not_modified {
        scan_tracked_prs(
            &store,
            &github_provider,
            &tracked,
            &current_user_login,
            &mut outcome,
        )?;
        next_state.last_success_at = next_state.last_polled_at.clone();
        store.save_sync_state(&next_state)?;
        return Ok(outcome);
    }

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
            record_event(
                &store,
                &tracked,
                &current_user_login,
                metadata.as_ref(),
                &mut event,
                &mut outcome,
            )?;
        }
    }

    scan_tracked_prs(
        &store,
        &github_provider,
        &tracked,
        &current_user_login,
        &mut outcome,
    )?;

    next_state.last_success_at = next_state.last_polled_at.clone();
    store.save_sync_state(&next_state)?;
    Ok(outcome)
}
