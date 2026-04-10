use crate::models::{GithubEvent, GithubPullRef, ReviewRequest};

use super::{
    classify_review_request, matches_slack_user_id, matches_slack_username, update_activity_label,
};

#[test]
fn marks_done_from_my_approval_event() {
    let pull = GithubPullRef {
        owner: "owner".to_string(),
        repo: "repo".to_string(),
        number: 1,
    };
    let request = ReviewRequest::new(
        &pull,
        "PR".to_string(),
        Some("author".to_string()),
        None,
        "U123".to_string(),
        "requester".to_string(),
        None,
        "1".to_string(),
        None,
        "hello".to_string(),
        None,
    );

    let events = vec![GithubEvent {
        id: "event-1".to_string(),
        pr_key: request.pr_key.clone(),
        pr_title: None,
        repo_owner: None,
        repo_name: None,
        pr_number: None,
        pr_author_login: None,
        notification_thread_id: "thread-1".to_string(),
        notification_reason: "review_requested".to_string(),
        event_kind: "approved".to_string(),
        actor_login: Some("sample-dev".to_string()),
        actor_is_me: true,
        related_to_me: true,
        event_at: "2026-03-23T00:00:00Z".to_string(),
        payload_json: "{}".to_string(),
        created_at: "2026-03-23T00:00:00Z".to_string(),
        read_at: None,
    }];

    assert_eq!(
        classify_review_request(&request, &events, "sample-dev", "", "review-bot")
            .map(|value| value.as_str().to_string()),
        Some("done".to_string())
    );
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
