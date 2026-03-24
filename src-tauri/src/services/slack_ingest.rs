use std::sync::Arc;

use anyhow::Result;
use chrono::{Duration, Local};

use crate::{
    config::AppConfig,
    db::ReviewStore,
    models::{ReviewRequest, SyncState, newer_ts, utc_now_string},
    providers::{GithubProvider, SlackProvider},
};

use crate::providers::slack::{extract_deadline, extract_pull_requests, slack_ts_to_local_date};

pub const SLACK_SYNC_SOURCE: &str = "slack_search";

#[derive(Debug, Default)]
pub struct SlackSyncOutcome {
    pub new_pending_count: u64,
    pub last_seen_slack_ts: Option<String>,
}

pub fn run(
    config: &AppConfig,
    store: Arc<dyn ReviewStore>,
    slack_provider: Arc<dyn SlackProvider>,
    github_provider: Arc<dyn GithubProvider>,
) -> Result<SlackSyncOutcome> {
    let sync_state = store.get_sync_state(SLACK_SYNC_SOURCE)?;
    let last_seen = sync_state.last_seen_slack_ts.as_deref();
    let mut outcome = SlackSyncOutcome::default();
    let query = if config.lookback_days > 0 {
        let after = (Local::now() - Duration::days(config.lookback_days as i64))
            .format("%Y-%m-%d")
            .to_string();
        format!("{} after:{}", config.slack_mention_keyword, after)
    } else {
        config.slack_mention_keyword.clone()
    };
    let mut messages = slack_provider.search_messages(&query)?;
    messages.sort_by(|left, right| left.ts.partial_cmp(&right.ts).unwrap_or(std::cmp::Ordering::Equal));

    for message in messages {
        if !newer_ts(&message.ts, last_seen) {
            continue;
        }
        if newer_ts(&message.ts, outcome.last_seen_slack_ts.as_deref()) {
            outcome.last_seen_slack_ts = Some(message.ts.clone());
        }
        let pulls = extract_pull_requests(&message.text);
        if pulls.is_empty() {
            continue;
        }
        let base_date = slack_ts_to_local_date(&message.ts).unwrap_or_else(|| Local::now().date_naive());
        let deadline = extract_deadline(&message.text, base_date);
        let display_name = match slack_provider.fetch_user_display_name(&message.user_id) {
            Ok(Some(value)) => value,
            Ok(None) => message.user_id.clone(),
            Err(error) => {
                eprintln!(
                    "Slack display name lookup failed for {}: {error}",
                    message.user_id
                );
                message.user_id.clone()
            }
        };
        let permalink = match message.channel_id.as_deref() {
            Some(channel_id) => match slack_provider.fetch_permalink(channel_id, &message.ts) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "Slack permalink lookup failed for {} in {}: {error}",
                        message.ts, channel_id
                    );
                    None
                }
            },
            None => None,
        };

        for pull in pulls {
            if !store.review_request_exists(&message.ts, &pull.key())? {
                outcome.new_pending_count += 1;
            }
            let metadata = github_provider.fetch_pr_metadata(&pull).ok();
            let pr_title = metadata
                .as_ref()
                .map(|value| value.title.clone())
                .unwrap_or_else(|| pull.key());
            let request = ReviewRequest::new(
                &pull,
                pr_title,
                metadata.as_ref().and_then(|value| value.author_login.clone()),
                metadata.as_ref().and_then(|value| value.merged_at.clone()),
                message.user_id.clone(),
                display_name.clone(),
                message.channel_id.clone(),
                message.ts.clone(),
                permalink.clone(),
                message.text.clone(),
                deadline.clone(),
            );
            store.upsert_review_request(&request)?;
        }
    }

    let mut next_state = SyncState::new(SLACK_SYNC_SOURCE);
    next_state.last_polled_at = Some(utc_now_string());
    next_state.last_success_at = next_state.last_polled_at.clone();
    next_state.last_seen_slack_ts = outcome
        .last_seen_slack_ts
        .clone()
        .or(sync_state.last_seen_slack_ts);
    next_state.last_error = None;
    store.save_sync_state(&next_state)?;
    Ok(outcome)
}
