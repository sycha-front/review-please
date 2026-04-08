use std::path::PathBuf;

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Local, Utc};

use crate::{
    db::{ReviewStore, SqliteStore},
    models::{GithubEvent, GithubPullRef, ReviewRequest, ReviewStatus},
};

fn temp_db_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "review-please-{name}-{}.sqlite3",
        uuid::Uuid::new_v4()
    ))
}

#[test]
fn prunes_requests_and_events_outside_lookback_window() -> Result<()> {
    let store = SqliteStore::new(temp_db_path("prune-history"))?;
    store.init_schema()?;

    let pull = GithubPullRef {
        owner: "owner".to_string(),
        repo: "repo".to_string(),
        number: 1,
    };

    let recent_ts = (Local::now() - ChronoDuration::days(1))
        .timestamp()
        .to_string();
    let old_ts = (Local::now() - ChronoDuration::days(10))
        .timestamp()
        .to_string();

    let recent_request = ReviewRequest::new(
        &pull,
        "Recent".to_string(),
        Some("other-user".to_string()),
        None,
        "U123".to_string(),
        "requester".to_string(),
        None,
        recent_ts,
        None,
        "hello".to_string(),
        None,
    );
    let mut old_pull = pull.clone();
    old_pull.number = 2;
    let old_request = ReviewRequest::new(
        &old_pull,
        "Old".to_string(),
        Some("other-user".to_string()),
        None,
        "U999".to_string(),
        "requester".to_string(),
        None,
        old_ts,
        None,
        "hello".to_string(),
        None,
    );

    assert!(store.upsert_review_request(&recent_request)?);
    assert!(store.upsert_review_request(&old_request)?);

    let recent_event = GithubEvent {
        id: "event-recent".to_string(),
        pr_key: recent_request.pr_key.clone(),
        pr_title: None,
        repo_owner: None,
        repo_name: None,
        pr_number: None,
        pr_author_login: None,
        notification_thread_id: "thread-1".to_string(),
        notification_reason: "author".to_string(),
        event_kind: "commented".to_string(),
        actor_login: Some("someone".to_string()),
        actor_is_me: false,
        related_to_me: true,
        event_at: (Utc::now() - ChronoDuration::days(1)).to_rfc3339(),
        payload_json: "{}".to_string(),
        created_at: Utc::now().to_rfc3339(),
        read_at: None,
    };
    let old_event = GithubEvent {
        id: "event-old".to_string(),
        pr_key: old_request.pr_key.clone(),
        pr_title: None,
        repo_owner: None,
        repo_name: None,
        pr_number: None,
        pr_author_login: None,
        notification_thread_id: "thread-2".to_string(),
        notification_reason: "author".to_string(),
        event_kind: "commented".to_string(),
        actor_login: Some("someone".to_string()),
        actor_is_me: false,
        related_to_me: true,
        event_at: (Utc::now() - ChronoDuration::days(10)).to_rfc3339(),
        payload_json: "{}".to_string(),
        created_at: Utc::now().to_rfc3339(),
        read_at: None,
    };

    store.upsert_github_event(&recent_event)?;
    store.upsert_github_event(&old_event)?;

    store.prune_history(7)?;

    let dump = store.dump(10, "OK", None, "me", false, "", "")?;
    assert_eq!(dump.pending.len(), 1);
    assert!(dump
        .pending
        .iter()
        .all(|item| item.pr_key == recent_request.pr_key));
    assert!(dump
        .recent_events
        .iter()
        .all(|event| event.id == "event-recent"));
    Ok(())
}

#[test]
fn keeps_recent_github_events_without_review_requests() -> Result<()> {
    let store = SqliteStore::new(temp_db_path("keep-github-only-events"))?;
    store.init_schema()?;

    let event = GithubEvent {
        id: "event-recent".to_string(),
        pr_key: "owner/repo#99".to_string(),
        pr_title: Some("PR".to_string()),
        repo_owner: Some("owner".to_string()),
        repo_name: Some("repo".to_string()),
        pr_number: Some(99),
        pr_author_login: Some("someone".to_string()),
        notification_thread_id: "thread-1".to_string(),
        notification_reason: "review_requested".to_string(),
        event_kind: "unknown".to_string(),
        actor_login: None,
        actor_is_me: false,
        related_to_me: true,
        event_at: (Utc::now() - ChronoDuration::days(1)).to_rfc3339(),
        payload_json: "{}".to_string(),
        created_at: Utc::now().to_rfc3339(),
        read_at: None,
    };

    store.upsert_github_event(&event)?;
    store.prune_history(7)?;

    assert_eq!(store.github_event_count()?, 1);
    let dump = store.dump(10, "OK", None, "me", false, "", "")?;
    assert_eq!(dump.recent_events.len(), 1);
    assert_eq!(dump.recent_events[0].id, "event-recent");
    Ok(())
}

#[test]
fn creates_new_pending_request_after_previous_one_is_done() -> Result<()> {
    let store = SqliteStore::new(temp_db_path("new-after-done"))?;
    store.init_schema()?;

    let pull = GithubPullRef {
        owner: "owner".to_string(),
        repo: "repo".to_string(),
        number: 8,
    };

    let first = ReviewRequest::new(
        &pull,
        "Initial title".to_string(),
        Some("author".to_string()),
        None,
        "U111".to_string(),
        "first requester".to_string(),
        None,
        "1711930000.100000".to_string(),
        None,
        "first message".to_string(),
        None,
    );
    assert!(store.upsert_review_request(&first)?);
    store.set_review_request_status_manual(&first.id, ReviewStatus::Done)?;

    let second = ReviewRequest::new(
        &pull,
        "Follow-up title".to_string(),
        Some("author".to_string()),
        None,
        "U222".to_string(),
        "second requester".to_string(),
        None,
        "1711933600.200000".to_string(),
        None,
        "second message".to_string(),
        None,
    );
    assert!(store.upsert_review_request(&second)?);

    let dump = store.dump(10, "OK", None, "me", false, "", "")?;
    assert_eq!(dump.pending.len(), 1);
    assert_eq!(dump.done.len(), 1);
    assert_eq!(dump.pending[0].id, second.id);
    assert_eq!(dump.done[0].id, first.id);
    Ok(())
}
