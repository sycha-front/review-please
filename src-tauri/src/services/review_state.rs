use chrono::{Local, NaiveDate};

use crate::models::{EventKind, GithubEvent, ReviewRequest, ReviewStatus};

const REVIEW_REQUEST_REASON: &str = "review_requested";
const MENTION_REASON: &str = "mention";
const TEAM_MENTION_REASON: &str = "team_mention";

pub fn is_bot_login(login: &str) -> bool {
    login.contains("[bot]")
}

pub fn update_activity_label(
    event: &GithubEvent,
    github_username: &str,
    pr_author_login: Option<&str>,
    github_related_updates_only: bool,
) -> Option<&'static str> {
    if event
        .actor_login
        .as_deref()
        .map(is_bot_login)
        .unwrap_or(false)
    {
        return None;
    }

    let is_my_pr = pr_author_login
        .map(|login| login.eq_ignore_ascii_case(github_username))
        .unwrap_or(false);

    if github_related_updates_only {
        return update_directly_related_activity_label(event, is_my_pr);
    }

    match event.notification_reason.as_str() {
        REVIEW_REQUEST_REASON => Some("새 리뷰 요청"),
        MENTION_REASON | TEAM_MENTION_REASON => Some("새 멘션"),
        _ => reviewer_activity_label(event, is_my_pr, true),
    }
}

fn update_directly_related_activity_label(
    event: &GithubEvent,
    is_my_pr: bool,
) -> Option<&'static str> {
    match event.notification_reason.as_str() {
        MENTION_REASON => Some("새 멘션"),
        _ => reviewer_activity_label(event, is_my_pr, false),
    }
}

pub fn matches_slack_username(candidate: &str, slack_username: &str) -> bool {
    let candidate = normalize_slack_username(candidate);
    let slack_username = normalize_slack_username(slack_username);
    !candidate.is_empty() && candidate == slack_username
}

pub fn matches_slack_user_id(candidate: &str, slack_user_id: &str) -> bool {
    let candidate = candidate.trim();
    let slack_user_id = slack_user_id.trim();
    !candidate.is_empty() && candidate == slack_user_id
}

pub fn classify_review_request(
    request: &ReviewRequest,
    events: &[GithubEvent],
    github_username: &str,
    slack_user_id: &str,
    slack_username: &str,
) -> Option<ReviewStatus> {
    if matches_slack_user_id(&request.requester_slack_user_id, slack_user_id) {
        return None;
    }
    if matches_slack_username(&request.requester_display_name, slack_username) {
        return None;
    }

    let is_my_pr = request
        .pr_author_login
        .as_deref()
        .map(|login| login.eq_ignore_ascii_case(github_username))
        .unwrap_or(false);
    let has_my_approval = events.iter().any(is_my_approval_event);
    let is_closed = request
        .pr_state
        .as_deref()
        .map(|state| state.eq_ignore_ascii_case("closed"))
        .unwrap_or(false)
        || request.pr_closed_at.is_some();
    let is_draft = request.pr_is_draft;
    let is_merged = request.pr_merged_at.is_some();
    let overdue_by_three_days = request
        .deadline_date
        .as_deref()
        .and_then(parse_deadline)
        .map(|deadline| (Local::now().date_naive() - deadline).num_days() >= 3)
        .unwrap_or(false);

    if overdue_by_three_days || has_my_approval || is_merged || is_closed {
        return Some(ReviewStatus::Done);
    }
    if is_draft {
        return None;
    }
    if events.iter().any(|event| is_update_event(event, is_my_pr)) {
        return Some(ReviewStatus::Update);
    }
    if !is_my_pr {
        return Some(ReviewStatus::Pending);
    }

    None
}

fn parse_deadline(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

fn normalize_slack_username(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('@')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn reviewer_activity_label(
    event: &GithubEvent,
    is_my_pr: bool,
    include_changes_requested: bool,
) -> Option<&'static str> {
    if !is_my_pr || event.actor_is_me {
        return None;
    }
    if event.event_kind == EventKind::Approved.as_str() {
        return Some("새 approve");
    }
    if include_changes_requested && event.event_kind == EventKind::ChangesRequested.as_str() {
        return Some("changes requested");
    }
    if is_comment_event_kind(&event.event_kind) {
        return Some("새 comment");
    }

    None
}

fn is_my_approval_event(event: &GithubEvent) -> bool {
    event.actor_is_me && event.event_kind == EventKind::Approved.as_str()
}

fn is_comment_event_kind(event_kind: &str) -> bool {
    event_kind == EventKind::Commented.as_str() || event_kind == EventKind::ReviewCommented.as_str()
}

fn is_update_event(event: &GithubEvent, is_my_pr: bool) -> bool {
    if matches!(
        event.notification_reason.as_str(),
        MENTION_REASON | TEAM_MENTION_REASON
    ) {
        return true;
    }

    is_my_pr
        && !event.actor_is_me
        && (is_comment_event_kind(&event.event_kind)
            || event.event_kind == EventKind::ChangesRequested.as_str())
}

#[cfg(test)]
#[path = "review_state_tests.rs"]
mod tests;
