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
    let has_my_approval = events.iter().any(is_my_approval_event);
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
    event_kind == EventKind::Commented.as_str()
        || event_kind == EventKind::ReviewCommented.as_str()
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
mod tests {
    use crate::models::{GithubEvent, GithubPullRef, ReviewRequest};

    use super::{
        classify_review_request, matches_slack_user_id, matches_slack_username, should_mark_done,
        update_activity_label,
    };

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
            Some("sample-dev".to_string()),
            None,
            "U123".to_string(),
            "requester".to_string(),
            None,
            "1".to_string(),
            None,
            "hello".to_string(),
            None,
        );
        request.pr_author_login = Some("sample-dev".to_string());

        let events = vec![GithubEvent {
            id: "event-1".to_string(),
            pr_key: request.pr_key.clone(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
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
            classify_review_request(&request, &events, "sample-dev", "", "review-bot")
                .map(|value| value.as_str().to_string()),
            Some("update".to_string())
        );
    }

    #[test]
    fn keeps_github_review_request_as_pending() {
        let pull = GithubPullRef {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 1,
        };
        let request = ReviewRequest::new_github_review_request(
            &pull,
            "PR".to_string(),
            Some("author".to_string()),
            None,
            "1".to_string(),
            "GitHub에서 리뷰 요청이 왔습니다.".to_string(),
        );

        let events = vec![GithubEvent {
            id: "event-1".to_string(),
            pr_key: request.pr_key.clone(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: Some("author".to_string()),
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "review_requested".to_string(),
            event_kind: "unknown".to_string(),
            actor_login: None,
            actor_is_me: false,
            related_to_me: true,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        }];

        assert_eq!(
            classify_review_request(&request, &events, "sample-dev", "", "review-bot")
                .map(|value| value.as_str().to_string()),
            Some("pending".to_string())
        );
    }

    #[test]
    fn identifies_visible_update_notifications() {
        let event = GithubEvent {
            id: "event-1".to_string(),
            pr_key: "owner/repo#1".to_string(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "author".to_string(),
            event_kind: "commented".to_string(),
            actor_login: Some("reviewer".to_string()),
            actor_is_me: false,
            related_to_me: true,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        };

        assert_eq!(
            update_activity_label(&event, "sample-dev", Some("sample-dev"), false),
            Some("새 comment")
        );
        assert_eq!(
            update_activity_label(&event, "sample-dev", Some("other"), false),
            None
        );
    }

    #[test]
    fn hides_unrelated_updates_when_related_only_mode_is_enabled() {
        let event = GithubEvent {
            id: "event-1".to_string(),
            pr_key: "owner/repo#1".to_string(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "review_requested".to_string(),
            event_kind: "commented".to_string(),
            actor_login: Some("reviewer".to_string()),
            actor_is_me: false,
            related_to_me: false,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        };

        assert_eq!(
            update_activity_label(&event, "sample-dev", Some("other"), true),
            None
        );
    }

    #[test]
    fn keeps_direct_mentions_when_related_only_mode_is_enabled() {
        let event = GithubEvent {
            id: "event-1".to_string(),
            pr_key: "owner/repo#1".to_string(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "mention".to_string(),
            event_kind: "commented".to_string(),
            actor_login: Some("reviewer".to_string()),
            actor_is_me: false,
            related_to_me: true,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        };

        assert_eq!(
            update_activity_label(&event, "sample-dev", Some("other"), true),
            Some("새 멘션")
        );
    }

    #[test]
    fn hides_team_mentions_when_related_only_mode_is_enabled() {
        let event = GithubEvent {
            id: "event-1".to_string(),
            pr_key: "owner/repo#1".to_string(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "team_mention".to_string(),
            event_kind: "commented".to_string(),
            actor_login: Some("reviewer".to_string()),
            actor_is_me: false,
            related_to_me: true,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        };

        assert_eq!(
            update_activity_label(&event, "sample-dev", Some("other"), true),
            None
        );
    }

    #[test]
    fn keeps_my_pr_approvals_when_related_only_mode_is_enabled() {
        let event = GithubEvent {
            id: "event-1".to_string(),
            pr_key: "owner/repo#1".to_string(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "author".to_string(),
            event_kind: "approved".to_string(),
            actor_login: Some("reviewer".to_string()),
            actor_is_me: false,
            related_to_me: true,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        };

        assert_eq!(
            update_activity_label(&event, "sample-dev", Some("sample-dev"), true),
            Some("새 approve")
        );
    }

    #[test]
    fn keeps_my_pr_comments_when_related_only_mode_is_enabled() {
        let event = GithubEvent {
            id: "event-1".to_string(),
            pr_key: "owner/repo#1".to_string(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "author".to_string(),
            event_kind: "review_commented".to_string(),
            actor_login: Some("reviewer".to_string()),
            actor_is_me: false,
            related_to_me: true,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        };

        assert_eq!(
            update_activity_label(&event, "sample-dev", Some("sample-dev"), true),
            Some("새 comment")
        );
    }

    #[test]
    fn hides_changes_requested_when_related_only_mode_is_enabled() {
        let event = GithubEvent {
            id: "event-1".to_string(),
            pr_key: "owner/repo#1".to_string(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "author".to_string(),
            event_kind: "changes_requested".to_string(),
            actor_login: Some("reviewer".to_string()),
            actor_is_me: false,
            related_to_me: true,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        };

        assert_eq!(
            update_activity_label(&event, "sample-dev", Some("sample-dev"), true),
            None
        );
    }

    #[test]
    fn hides_review_requested_when_related_only_mode_is_enabled() {
        let event = GithubEvent {
            id: "event-1".to_string(),
            pr_key: "owner/repo#1".to_string(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "review_requested".to_string(),
            event_kind: "unknown".to_string(),
            actor_login: None,
            actor_is_me: false,
            related_to_me: true,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        };

        assert_eq!(
            update_activity_label(&event, "sample-dev", Some("other"), true),
            None
        );
    }

    #[test]
    fn ignores_bot_updates() {
        let event = GithubEvent {
            id: "event-1".to_string(),
            pr_key: "owner/repo#1".to_string(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
            notification_thread_id: "thread-1".to_string(),
            notification_reason: "review_requested".to_string(),
            event_kind: "commented".to_string(),
            actor_login: Some("vercel[bot]".to_string()),
            actor_is_me: false,
            related_to_me: true,
            event_at: "2026-03-23T00:00:00Z".to_string(),
            payload_json: "{}".to_string(),
            created_at: "2026-03-23T00:00:00Z".to_string(),
            read_at: None,
        };

        assert_eq!(
            update_activity_label(&event, "sample-dev", Some("sample-dev"), false),
            None
        );
    }

    #[test]
    fn matches_slack_user_id_exactly() {
        assert!(matches_slack_user_id("U123", "U123"));
        assert!(matches_slack_user_id(" U123 ", "U123"));
        assert!(!matches_slack_user_id("U123", "U999"));
    }

    #[test]
    fn matches_slack_username_ignores_at_sign_and_spacing() {
        assert!(matches_slack_username("Sample User", "@Sample User"));
        assert!(matches_slack_username("  SampleUser ", "sampleuser"));
        assert!(matches_slack_username("Sample User", " sample user "));
        assert!(!matches_slack_username("Sample User", "Another User"));
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
            "Sample User".to_string(),
            None,
            "1".to_string(),
            None,
            "hello".to_string(),
            None,
        );

        assert_eq!(
            classify_review_request(&request, &[], "sample-dev", "", "@Sample User"),
            None
        );
    }

    #[test]
    fn excludes_requests_sent_by_me_on_slack_user_id() {
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
            "requester".to_string(),
            None,
            "1".to_string(),
            None,
            "hello".to_string(),
            None,
        );

        assert_eq!(
            classify_review_request(&request, &[], "sample-dev", "U123", "Sample User"),
            None
        );
    }

    #[test]
    fn treats_github_login_as_case_insensitive() {
        let pull = GithubPullRef {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 1,
        };
        let mut request = ReviewRequest::new(
            &pull,
            "PR".to_string(),
            Some("sample-dev".to_string()),
            None,
            "U123".to_string(),
            "requester".to_string(),
            None,
            "1".to_string(),
            None,
            "hello".to_string(),
            None,
        );
        request.pr_author_login = Some("sample-dev".to_string());

        let events = vec![GithubEvent {
            id: "event-1".to_string(),
            pr_key: request.pr_key.clone(),
            pr_title: None,
            repo_owner: None,
            repo_name: None,
            pr_number: None,
            pr_author_login: None,
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
            classify_review_request(&request, &events, "Sample-Dev", "", "review-bot")
                .map(|value| value.as_str().to_string()),
            Some("update".to_string())
        );
    }
}
