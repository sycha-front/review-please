use chrono::{Local, NaiveDate};

use crate::models::{EventKind, GithubEvent, ReviewRequest, ReviewStatus};

pub fn matches_slack_username(candidate: &str, slack_username: &str) -> bool {
    let candidate = normalize_slack_username(candidate);
    let slack_username = normalize_slack_username(slack_username);
    !candidate.is_empty() && candidate == slack_username
}

pub fn should_mark_done(event_kind: &str, actor_is_me: bool) -> bool {
    actor_is_me && event_kind == EventKind::Approved.as_str()
}

pub fn classify_review_request(
    request: &ReviewRequest,
    events: &[GithubEvent],
    github_username: &str,
    slack_username: &str,
) -> Option<ReviewStatus> {
    if matches_slack_username(&request.requester_display_name, slack_username) {
        return None;
    }

    let is_my_pr = request
        .pr_author_login
        .as_deref()
        .map(|login| login == github_username)
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

#[cfg(test)]
mod tests {
    use crate::models::{GithubEvent, GithubPullRef, ReviewRequest};

    use super::{classify_review_request, matches_slack_username, should_mark_done};

    #[test]
    fn marks_done_only_for_my_approval() {
        assert!(should_mark_done("approved", true));
        assert!(!should_mark_done("commented", true));
        assert!(!should_mark_done("approved", false));
        assert!(!should_mark_done("unknown", true));
    }

    #[test]
    fn classifies_my_pr_comment_as_update() {
        let pull = GithubPullRef {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 1,
        };
        let mut request = ReviewRequest::new(
            &pull,
            "PR".to_string(),
            Some("sycha-front".to_string()),
            None,
            "U123".to_string(),
            "requester".to_string(),
            None,
            "1".to_string(),
            None,
            "hello".to_string(),
            None,
        );
        request.pr_author_login = Some("sycha-front".to_string());

        let events = vec![GithubEvent {
            id: "event-1".to_string(),
            pr_key: request.pr_key.clone(),
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "author".to_string(),
            event_kind: "commented".to_string(),
            actor_login: Some("other-user".to_string()),
            actor_is_me: false,
            related_to_me: true,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        }];

        assert_eq!(
            classify_review_request(&request, &events, "sycha-front", "review-bot")
                .map(|value| value.as_str().to_string()),
            Some("update".to_string())
        );
    }

    #[test]
    fn matches_slack_username_ignores_at_sign_and_spacing() {
        assert!(matches_slack_username("차수연", "@차수연"));
        assert!(matches_slack_username("  Chasuyeon ", "chasuyeon"));
        assert!(matches_slack_username("차수연", " 차수연 "));
        assert!(!matches_slack_username("차수연", "최진실"));
    }

    #[test]
    fn excludes_requests_sent_by_me_on_slack() {
        let pull = GithubPullRef {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 1,
        };
        let request = ReviewRequest::new(
            &pull,
            "PR".to_string(),
            Some("other-user".to_string()),
            None,
            "U123".to_string(),
            "차수연".to_string(),
            None,
            "1".to_string(),
            None,
            "hello".to_string(),
            None,
        );

        assert_eq!(
            classify_review_request(&request, &[], "sycha-front", "@차수연"),
            None
        );
    }
}
