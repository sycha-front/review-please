use chrono::{Local, NaiveDate};

use crate::models::{EventKind, GithubEvent, ReviewRequest, ReviewStatus};

pub fn is_bot_login(login: &str) -> bool {
    login.contains("[bot]")
}

pub fn update_activity_label(
    event: &GithubEvent,
    github_username: &str,
    pr_author_login: Option<&str>,
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

    match event.notification_reason.as_str() {
        "review_requested" => Some("새 리뷰 요청"),
        "mention" | "team_mention" => Some("새 멘션"),
        _ if is_my_pr && !event.actor_is_me && event.event_kind == EventKind::Approved.as_str() => {
            Some("새 approve")
        }
        _ if is_my_pr
            && !event.actor_is_me
            && event.event_kind == EventKind::ChangesRequested.as_str() =>
        {
            Some("changes requested")
        }
        _ if is_my_pr
            && !event.actor_is_me
            && matches!(
                event.event_kind.as_str(),
                value if value == EventKind::Commented.as_str()
                    || value == EventKind::ReviewCommented.as_str()
            ) =>
        {
            Some("새 comment")
        }
        _ => None,
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

pub fn should_mark_done(event_kind: &str, actor_is_me: bool) -> bool {
    actor_is_me && event_kind == EventKind::Approved.as_str()
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
    let has_my_approval = events
        .iter()
        .any(|event| event.actor_is_me && event.event_kind == EventKind::Approved.as_str());
    let is_merged = request.pr_merged_at.is_some();
    let overdue_by_three_days = request
        .deadline_date
        .as_deref()
        .and_then(parse_deadline)
        .map(|deadline| (Local::now().date_naive() - deadline).num_days() >= 3)
        .unwrap_or(false);

    if overdue_by_three_days || has_my_approval || is_merged {
        return Some(ReviewStatus::Done);
    }

    let has_update = events.iter().any(|event| is_update_event(event, is_my_pr));
    if has_update {
        return Some(ReviewStatus::Update);
    }

    if !is_my_pr && !has_my_approval && !is_merged {
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

fn is_update_event(event: &GithubEvent, is_my_pr: bool) -> bool {
    if matches!(
        event.notification_reason.as_str(),
        "review_requested" | "mention" | "team_mention"
    ) {
        return true;
    }

    is_my_pr
        && !event.actor_is_me
        && matches!(
            event.event_kind.as_str(),
            value if value == EventKind::Commented.as_str()
                || value == EventKind::ReviewCommented.as_str()
                || value == EventKind::ChangesRequested.as_str()
        )
}
