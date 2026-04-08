use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, Local, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row, ToSql};
use serde_json::Value;

use crate::{
    config,
    models::{
        format_last_sync, utc_now_string, GithubEvent, IntegrationStatus, IntegrationsSummary,
        ReviewDump, ReviewRequest, ReviewStatus, SyncState, TrayState, UpdateFeedItem,
    },
    providers::slack::{extract_deadline, slack_ts_to_local_date},
    services::review_state::{classify_review_request, update_activity_label},
};

pub trait ReviewStore: Send + Sync {
    fn init_schema(&self) -> Result<()>;
    fn clear_state(&self) -> Result<()>;
    fn prune_history(&self, lookback_days: u64) -> Result<()>;
    fn github_event_count(&self) -> Result<u64>;
    fn upsert_review_request(&self, request: &ReviewRequest) -> Result<bool>;
    fn update_review_request_deadline(
        &self,
        review_request_id: &str,
        deadline_date: &str,
    ) -> Result<()>;
    fn set_review_request_status_manual(
        &self,
        review_request_id: &str,
        status: ReviewStatus,
    ) -> Result<()>;
    fn tracked_pr_keys(&self) -> Result<Vec<String>>;
    fn should_fetch_comment_events(&self, pr_key: &str, github_username: &str) -> Result<bool>;
    fn refresh_review_request_pr_metadata(
        &self,
        pr_key: &str,
        pr_title: &str,
        pr_author_login: Option<&str>,
        pr_merged_at: Option<&str>,
    ) -> Result<()>;
    fn latest_event_at_for_pr(&self, pr_key: &str) -> Result<Option<String>>;
    fn upsert_github_event(&self, event: &GithubEvent) -> Result<bool>;
    fn mark_github_events_read(&self, event_ids: &[String]) -> Result<()>;
    fn mark_requests_done_by_pr_key(
        &self,
        pr_key: &str,
        completion_event_id: &str,
        completed_at: &str,
    ) -> Result<u64>;
    fn get_sync_state(&self, source: &str) -> Result<SyncState>;
    fn save_sync_state(&self, sync_state: &SyncState) -> Result<()>;
    fn dump(
        &self,
        done_limit: usize,
        status: &str,
        last_error: Option<String>,
        github_username: &str,
        github_related_updates_only: bool,
        slack_user_id: &str,
        slack_username: &str,
    ) -> Result<ReviewDump>;
    fn tray_state(
        &self,
        status: &str,
        last_error: Option<String>,
        github_username: &str,
        slack_user_id: &str,
        slack_username: &str,
    ) -> Result<TrayState>;
    fn last_error_message(&self) -> Result<Option<String>>;
}

pub struct SqliteStore {
    connection: Mutex<Connection>,
}

impl SqliteStore {
    pub fn new(path: PathBuf) -> Result<Self> {
        let connection = open_connection(&path)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn from_default_location() -> Result<Self> {
        config::ensure_data_dir()?;
        Self::new(config::database_path()?)
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| anyhow!("failed to lock sqlite connection"))
    }

    fn init_schema_with_connection(&self, connection: &Connection) -> Result<()> {
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS review_requests (
              id TEXT PRIMARY KEY,
              pr_key TEXT NOT NULL,
              pr_url TEXT NOT NULL,
              pr_title TEXT NOT NULL,
              repo_owner TEXT NOT NULL,
              repo_name TEXT NOT NULL,
              pr_number INTEGER NOT NULL,
              pr_author_login TEXT,
              pr_merged_at TEXT,
              requester_slack_user_id TEXT NOT NULL,
              requester_display_name TEXT NOT NULL,
              slack_channel_id TEXT,
              slack_message_ts TEXT NOT NULL,
              slack_permalink TEXT,
              slack_text TEXT NOT NULL,
              deadline_date TEXT,
              status TEXT NOT NULL,
              is_status_manual INTEGER NOT NULL DEFAULT 0,
              completed_at TEXT,
              completion_event_id TEXT,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_review_requests_message_pr
              ON review_requests(slack_message_ts, pr_key);
            CREATE INDEX IF NOT EXISTS idx_review_requests_pr_key
              ON review_requests(pr_key);

            CREATE TABLE IF NOT EXISTS github_events (
              id TEXT PRIMARY KEY,
              pr_key TEXT NOT NULL,
              pr_title TEXT,
              repo_owner TEXT,
              repo_name TEXT,
              pr_number INTEGER,
              pr_author_login TEXT,
              notification_thread_id TEXT NOT NULL,
              notification_reason TEXT NOT NULL,
              event_kind TEXT NOT NULL,
              actor_login TEXT,
              actor_is_me INTEGER NOT NULL,
              related_to_me INTEGER NOT NULL,
              event_at TEXT NOT NULL,
              payload_json TEXT NOT NULL,
              created_at TEXT NOT NULL,
              read_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_github_events_pr_key
              ON github_events(pr_key);

            CREATE TABLE IF NOT EXISTS sync_state (
              source TEXT PRIMARY KEY,
              last_polled_at TEXT,
              last_seen_slack_ts TEXT,
              github_last_modified TEXT,
              github_etag TEXT,
              github_poll_interval_seconds INTEGER,
              last_success_at TEXT,
              last_error TEXT,
              consecutive_failures INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )?;
        add_column_if_missing(connection, "github_events", "pr_title", "TEXT")?;
        add_column_if_missing(connection, "github_events", "repo_owner", "TEXT")?;
        add_column_if_missing(connection, "github_events", "repo_name", "TEXT")?;
        add_column_if_missing(connection, "github_events", "pr_number", "INTEGER")?;
        add_column_if_missing(connection, "github_events", "pr_author_login", "TEXT")?;
        add_column_if_missing(connection, "review_requests", "pr_author_login", "TEXT")?;
        add_column_if_missing(connection, "review_requests", "pr_merged_at", "TEXT")?;
        add_column_if_missing(
            connection,
            "review_requests",
            "is_status_manual",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        add_column_if_missing(connection, "github_events", "read_at", "TEXT")?;
        Ok(())
    }

    fn fetch_review_requests(&self, connection: &Connection) -> Result<Vec<ReviewRequest>> {
        let mut statement =
            connection.prepare("SELECT * FROM review_requests ORDER BY created_at DESC;")?;
        let rows = statement.query_map([], row_to_review_request)?;
        collect_rows(rows)
    }

    fn fetch_github_events(&self, connection: &Connection) -> Result<Vec<GithubEvent>> {
        let mut statement =
            connection.prepare("SELECT * FROM github_events ORDER BY event_at DESC;")?;
        let rows = statement.query_map([], row_to_github_event)?;
        collect_rows(rows)
    }

    fn categorized_requests(
        &self,
        connection: &Connection,
        github_username: &str,
        slack_user_id: &str,
        slack_username: &str,
    ) -> Result<(
        Vec<ReviewRequest>,
        Vec<ReviewRequest>,
        Vec<ReviewRequest>,
        Vec<GithubEvent>,
    )> {
        let requests = self.fetch_review_requests(connection)?;
        let all_events = self.fetch_github_events(connection)?;
        let mut pending = Vec::new();
        let mut done = Vec::new();
        let mut update = Vec::new();

        for mut request in requests {
            if request.deadline_date.is_none() {
                if let Some(base_date) = slack_ts_to_local_date(&request.slack_message_ts) {
                    request.deadline_date = extract_deadline(&request.slack_text, base_date);
                }
            }
            if request.is_status_manual {
                match request.status.as_str() {
                    "pending" => pending.push(request),
                    "done" => done.push(request),
                    "update" => update.push(request),
                    _ => {}
                }
                continue;
            }

            let request_events = all_events
                .iter()
                .filter(|event| event.pr_key == request.pr_key)
                .cloned()
                .collect::<Vec<_>>();

            match classify_review_request(
                &request,
                &request_events,
                github_username,
                slack_user_id,
                slack_username,
            ) {
                Some(ReviewStatus::Pending) => {
                    request.status = ReviewStatus::Pending.as_str().to_string();
                    pending.push(request);
                }
                Some(ReviewStatus::Done) => {
                    request.status = ReviewStatus::Done.as_str().to_string();
                    done.push(request);
                }
                Some(ReviewStatus::Update) => {
                    request.status = ReviewStatus::Update.as_str().to_string();
                    update.push(request);
                }
                None => {}
            }
        }

        pending.sort_by(|left, right| {
            left.deadline_date
                .cmp(&right.deadline_date)
                .then_with(|| right.created_at.cmp(&left.created_at))
        });
        done.sort_by(|left, right| {
            right
                .completed_at
                .as_deref()
                .or(right.pr_merged_at.as_deref())
                .unwrap_or(&right.updated_at)
                .cmp(
                    &left
                        .completed_at
                        .as_deref()
                        .or(left.pr_merged_at.as_deref())
                        .unwrap_or(&left.updated_at),
                )
        });
        update.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

        Ok((pending, done, update, all_events))
    }

    fn build_update_feed(
        &self,
        requests: &[ReviewRequest],
        events: &[GithubEvent],
        github_username: &str,
        github_related_updates_only: bool,
    ) -> Vec<UpdateFeedItem> {
        let mut requests_by_pr = HashMap::new();
        for request in requests {
            requests_by_pr
                .entry(request.pr_key.clone())
                .or_insert_with(|| request.clone());
        }

        let items = events
            .iter()
            .filter_map(|event| {
                build_update_feed_item(
                    event,
                    requests_by_pr.get(&event.pr_key),
                    github_username,
                    github_related_updates_only,
                )
            })
            .collect::<Vec<_>>();

        group_update_feed_items(items)
    }

    fn get_sync_state_with_connection(
        &self,
        connection: &Connection,
        source: &str,
    ) -> Result<SyncState> {
        let mut statement = connection.prepare(
            "SELECT source, last_polled_at, last_seen_slack_ts, github_last_modified, github_etag, github_poll_interval_seconds, last_success_at, last_error, consecutive_failures FROM sync_state WHERE source = ?1;",
        )?;
        let row = statement
            .query_row([source], row_to_sync_state)
            .optional()?;
        Ok(row.unwrap_or_else(|| SyncState::new(source)))
    }

    fn last_sync_at_with_connection(&self, connection: &Connection) -> Result<Option<String>> {
        Ok(connection
            .query_row("SELECT MAX(last_success_at) FROM sync_state;", [], |row| {
                row.get(0)
            })
            .optional()?
            .flatten())
    }

    fn last_error_message_with_connection(
        &self,
        connection: &Connection,
    ) -> Result<Option<String>> {
        Ok(connection
            .query_row(
                "SELECT last_error FROM sync_state WHERE last_error IS NOT NULL ORDER BY last_polled_at DESC LIMIT 1;",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    fn prune_history_with_connection(
        &self,
        connection: &Connection,
        lookback_days: u64,
    ) -> Result<()> {
        if lookback_days == 0 {
            return Ok(());
        }

        let slack_cutoff =
            (Local::now() - ChronoDuration::days(lookback_days as i64)).timestamp() as f64;
        let github_cutoff = (Utc::now() - ChronoDuration::days(lookback_days as i64)).to_rfc3339();

        connection.execute(
            "DELETE FROM review_requests WHERE CAST(slack_message_ts AS REAL) < ?1;",
            params![slack_cutoff],
        )?;
        connection.execute(
            r#"
            DELETE FROM github_events
            WHERE event_at < ?1;
            "#,
            params![github_cutoff],
        )?;
        Ok(())
    }
}

fn open_connection(path: &PathBuf) -> Result<Connection> {
    config::ensure_parent(path)?;
    let connection = Connection::open(path)
        .with_context(|| format!("failed to open sqlite database at {}", path.display()))?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    Ok(connection)
}

fn add_column_if_missing(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    column_definition: &str,
) -> Result<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name});"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in rows {
        if column? == column_name {
            return Ok(());
        }
    }
    connection.execute(
        &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {column_definition};"),
        [],
    )?;
    Ok(())
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>> {
    let mut items = Vec::new();
    for row in rows {
        items.push(row?);
    }
    Ok(items)
}

fn row_to_review_request(row: &Row<'_>) -> rusqlite::Result<ReviewRequest> {
    Ok(ReviewRequest {
        id: row.get("id")?,
        pr_key: row.get("pr_key")?,
        pr_url: row.get("pr_url")?,
        pr_title: row.get("pr_title")?,
        repo_owner: row.get("repo_owner")?,
        repo_name: row.get("repo_name")?,
        pr_number: row.get("pr_number")?,
        pr_author_login: row.get("pr_author_login")?,
        pr_merged_at: row.get("pr_merged_at")?,
        requester_slack_user_id: row.get("requester_slack_user_id")?,
        requester_display_name: row.get("requester_display_name")?,
        slack_channel_id: row.get("slack_channel_id")?,
        slack_message_ts: row.get("slack_message_ts")?,
        slack_permalink: row.get("slack_permalink")?,
        slack_text: row.get("slack_text")?,
        deadline_date: row.get("deadline_date")?,
        status: row.get("status")?,
        is_status_manual: row.get::<_, i64>("is_status_manual")? != 0,
        completed_at: row.get("completed_at")?,
        completion_event_id: row.get("completion_event_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_github_event(row: &Row<'_>) -> rusqlite::Result<GithubEvent> {
    Ok(GithubEvent {
        id: row.get("id")?,
        pr_key: row.get("pr_key")?,
        pr_title: row.get("pr_title")?,
        repo_owner: row.get("repo_owner")?,
        repo_name: row.get("repo_name")?,
        pr_number: row.get("pr_number")?,
        pr_author_login: row.get("pr_author_login")?,
        notification_thread_id: row.get("notification_thread_id")?,
        notification_reason: row.get("notification_reason")?,
        event_kind: row.get("event_kind")?,
        actor_login: row.get("actor_login")?,
        actor_is_me: row.get::<_, i64>("actor_is_me")? != 0,
        related_to_me: row.get::<_, i64>("related_to_me")? != 0,
        event_at: row.get("event_at")?,
        payload_json: row.get("payload_json")?,
        created_at: row.get("created_at")?,
        read_at: row.get("read_at")?,
    })
}

fn row_to_sync_state(row: &Row<'_>) -> rusqlite::Result<SyncState> {
    let github_poll_interval_seconds = row
        .get::<_, Option<i64>>("github_poll_interval_seconds")?
        .map(|value| value as u64);
    let consecutive_failures = row.get::<_, i64>("consecutive_failures")? as u64;
    Ok(SyncState {
        source: row.get("source")?,
        last_polled_at: row.get("last_polled_at")?,
        last_seen_slack_ts: row.get("last_seen_slack_ts")?,
        github_last_modified: row.get("github_last_modified")?,
        github_etag: row.get("github_etag")?,
        github_poll_interval_seconds,
        last_success_at: row.get("last_success_at")?,
        last_error: row.get("last_error")?,
        consecutive_failures,
    })
}

fn build_update_feed_item(
    event: &GithubEvent,
    request: Option<&ReviewRequest>,
    github_username: &str,
    github_related_updates_only: bool,
) -> Option<UpdateFeedItem> {
    let pr_author_login = request
        .and_then(|value| value.pr_author_login.as_deref())
        .or(event.pr_author_login.as_deref());
    let activity_label = update_activity_label(
        event,
        github_username,
        pr_author_login,
        github_related_updates_only,
    )?;
    let is_my_pr = pr_author_login
        .map(|login| login.eq_ignore_ascii_case(github_username))
        .unwrap_or(false);

    let pr_number = request.map(|value| value.pr_number).or(event.pr_number)?;
    let pr_title = request
        .map(|value| value.pr_title.clone())
        .or_else(|| event.pr_title.clone())
        .unwrap_or_else(|| event.pr_key.clone());
    let repo_owner = request
        .map(|value| value.repo_owner.clone())
        .or_else(|| event.repo_owner.clone())?;
    let repo_name = request
        .map(|value| value.repo_name.clone())
        .or_else(|| event.repo_name.clone())?;
    let pr_url = request
        .map(|value| value.pr_url.clone())
        .unwrap_or_else(|| format!("https://github.com/{repo_owner}/{repo_name}/pull/{pr_number}"));

    let target_label = format!("PR #{pr_number} {pr_title}");
    let headline = if is_my_pr
        && !matches!(
            event.notification_reason.as_str(),
            "mention" | "team_mention" | "review_requested"
        ) {
        format!("내 PR #{pr_number}에 {activity_label}")
    } else {
        format!("{target_label}에 {activity_label}")
    };

    let payload = serde_json::from_str::<Value>(&event.payload_json).unwrap_or(Value::Null);
    let target_url = payload
        .get("html_url")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or(pr_url);
    let summary = payload
        .get("body")
        .and_then(Value::as_str)
        .and_then(summarize_body);

    Some(UpdateFeedItem {
        id: event.pr_key.clone(),
        source_event_ids: vec![event.id.clone()],
        pr_key: event.pr_key.clone(),
        target_label,
        target_url,
        headline,
        summary,
        time_label: format_time_label(&event.event_at),
        occurred_at: event.event_at.clone(),
        actor_login: event.actor_login.clone(),
        actor_context: format_actor_context(event, &repo_owner, &repo_name, pr_number),
        repo_label: format!("{repo_owner}/{repo_name}"),
        activity_label: activity_label.to_string(),
        event_kind: event.event_kind.clone(),
        event_count: 1,
        unread_count: if event.read_at.is_some() { 0 } else { 1 },
        is_read: event.read_at.is_some(),
        read_at: event.read_at.clone(),
    })
}

fn group_update_feed_items(items: Vec<UpdateFeedItem>) -> Vec<UpdateFeedItem> {
    let mut grouped = HashMap::<String, Vec<UpdateFeedItem>>::new();
    for item in items {
        grouped.entry(item.pr_key.clone()).or_default().push(item);
    }

    let mut merged = grouped
        .into_values()
        .filter_map(merge_update_group)
        .collect::<Vec<_>>();
    merged.sort_by(|left, right| {
        left.is_read
            .cmp(&right.is_read)
            .then_with(|| right.occurred_at.cmp(&left.occurred_at))
    });
    merged
}

fn merge_update_group(group: Vec<UpdateFeedItem>) -> Option<UpdateFeedItem> {
    let latest = group
        .iter()
        .max_by(|left, right| left.occurred_at.cmp(&right.occurred_at))?
        .clone();

    let mut source_event_ids = Vec::new();
    let mut seen_ids = HashSet::new();
    let mut unread_count = 0_u64;
    let mut actor_logins = Vec::new();
    let mut seen_actors = HashSet::new();

    for item in &group {
        for event_id in &item.source_event_ids {
            if seen_ids.insert(event_id.clone()) {
                source_event_ids.push(event_id.clone());
            }
        }
        if !item.is_read {
            unread_count += 1;
        }
        if let Some(actor_login) = &item.actor_login {
            if seen_actors.insert(actor_login.clone()) {
                actor_logins.push(actor_login.clone());
            }
        }
    }

    let summary = latest.summary.clone();
    let activity_label = summarize_activity_label(&group);
    let headline = summarize_headline(&latest, &activity_label, group.len() as u64);
    let actor_login = latest
        .actor_login
        .clone()
        .or_else(|| actor_logins.first().cloned())
        .map(|login| {
            if actor_logins.len() > 1 {
                format!("{login} 외 {}명", actor_logins.len() - 1)
            } else {
                login
            }
        });

    Some(UpdateFeedItem {
        id: latest.pr_key.clone(),
        source_event_ids,
        pr_key: latest.pr_key.clone(),
        target_label: latest.target_label.clone(),
        target_url: latest.target_url.clone(),
        headline,
        summary,
        time_label: latest.time_label.clone(),
        occurred_at: latest.occurred_at.clone(),
        actor_login,
        actor_context: latest.actor_context.clone(),
        repo_label: latest.repo_label.clone(),
        activity_label,
        event_kind: latest.event_kind.clone(),
        event_count: group.len() as u64,
        unread_count,
        is_read: unread_count == 0,
        read_at: if unread_count == 0 {
            latest.read_at.clone()
        } else {
            None
        },
    })
}

fn summarize_activity_label(items: &[UpdateFeedItem]) -> String {
    if items.iter().any(|item| item.activity_label == "새 멘션") {
        return "새 멘션".to_string();
    }
    if items
        .iter()
        .any(|item| item.activity_label == "changes requested")
    {
        return "changes requested".to_string();
    }
    if items.iter().any(|item| item.activity_label == "새 approve") {
        return "새 approve".to_string();
    }
    "새 comment".to_string()
}

fn summarize_headline(latest: &UpdateFeedItem, activity_label: &str, event_count: u64) -> String {
    let mut headline = if latest.headline.starts_with("내 PR") {
        let prefix = latest
            .headline
            .split("에 ")
            .next()
            .unwrap_or(latest.headline.as_str());
        format!("{prefix}에 {activity_label}")
    } else {
        format!("{}에 {}", latest.target_label, activity_label)
    };

    if event_count > 1 {
        headline.push_str(&format!(" 외 {}건", event_count - 1));
    }

    headline
}

fn integration_status_from_sync_state(sync_state: SyncState) -> IntegrationStatus {
    let status = if sync_state.last_error.is_some() {
        "error"
    } else if sync_state.last_success_at.is_some() {
        "connected"
    } else {
        "waiting"
    };

    IntegrationStatus {
        status: status.to_string(),
        last_success_label: format_last_sync(sync_state.last_success_at.as_deref()),
        last_success_at: sync_state.last_success_at,
        last_error: sync_state.last_error,
    }
}

fn summarize_body(body: &str) -> Option<String> {
    let condensed = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
        .replace('\n', " ");
    if condensed.is_empty() {
        return None;
    }
    let mut summary = condensed.chars().take(120).collect::<String>();
    if condensed.chars().count() > 120 {
        summary.push_str("...");
    }
    Some(summary)
}

fn format_actor_context(
    event: &GithubEvent,
    repo_owner: &str,
    repo_name: &str,
    pr_number: i64,
) -> String {
    let repo = format!("{repo_owner}/{repo_name}");
    match event.event_kind.as_str() {
        "commented" | "review_commented" => format!("@{repo}/pull#{pr_number}"),
        _ => format!("@{repo}"),
    }
}

fn format_time_label(value: &str) -> String {
    let Ok(event_at) = DateTime::parse_from_rfc3339(value) else {
        return value.to_string();
    };
    let event_at = event_at.with_timezone(&Local);
    let now = Local::now();
    let delta = now.signed_duration_since(event_at);

    if delta.num_hours() >= 24 {
        return format!(
            "{}. {}. {}",
            event_at.year(),
            event_at.month(),
            event_at.day()
        );
    }

    if delta.num_minutes() < 1 {
        return "방금 전".to_string();
    }
    if delta.num_hours() < 1 {
        return format!("{}분 전", delta.num_minutes());
    }
    format!("{}시간 전", delta.num_hours())
}

impl ReviewStore for SqliteStore {
    fn init_schema(&self) -> Result<()> {
        let connection = self.connection()?;
        self.init_schema_with_connection(&connection)
    }

    fn clear_state(&self) -> Result<()> {
        let connection = self.connection()?;
        connection.execute_batch(
            r#"
            DROP TABLE IF EXISTS review_requests;
            DROP TABLE IF EXISTS github_events;
            DROP TABLE IF EXISTS sync_state;
            "#,
        )?;
        self.init_schema_with_connection(&connection)
    }

    fn prune_history(&self, lookback_days: u64) -> Result<()> {
        let connection = self.connection()?;
        self.prune_history_with_connection(&connection, lookback_days)
    }

    fn github_event_count(&self) -> Result<u64> {
        let connection = self.connection()?;
        let count: i64 =
            connection.query_row("SELECT COUNT(*) FROM github_events;", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    fn upsert_review_request(&self, request: &ReviewRequest) -> Result<bool> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;

        // Reuse the latest active request for the same PR so repeated Slack nudges
        // refresh one pending card instead of stacking duplicates in the queue.
        let existing_active = transaction
            .query_row(
                r#"
                SELECT id
                FROM review_requests
                WHERE pr_key = ?1
                  AND status IN ('pending', 'update')
                  AND is_status_manual = 0
                ORDER BY updated_at DESC, created_at DESC
                LIMIT 1;
                "#,
                params![request.pr_key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(existing_id) = existing_active {
            transaction.execute(
                r#"
                UPDATE review_requests
                SET pr_url = ?1,
                    pr_title = ?2,
                    repo_owner = ?3,
                    repo_name = ?4,
                    pr_number = ?5,
                    pr_author_login = ?6,
                    pr_merged_at = ?7,
                    requester_slack_user_id = ?8,
                    requester_display_name = ?9,
                    slack_channel_id = ?10,
                    slack_message_ts = ?11,
                    slack_permalink = ?12,
                    slack_text = ?13,
                    deadline_date = ?14,
                    status = 'pending',
                    is_status_manual = 0,
                    completed_at = NULL,
                    completion_event_id = NULL,
                    updated_at = ?15
                WHERE id = ?16;
                "#,
                params![
                    request.pr_url,
                    request.pr_title,
                    request.repo_owner,
                    request.repo_name,
                    request.pr_number,
                    request.pr_author_login,
                    request.pr_merged_at,
                    request.requester_slack_user_id,
                    request.requester_display_name,
                    request.slack_channel_id,
                    request.slack_message_ts,
                    request.slack_permalink,
                    request.slack_text,
                    request.deadline_date,
                    request.updated_at,
                    existing_id,
                ],
            )?;

            transaction.execute(
                r#"
                DELETE FROM review_requests
                WHERE pr_key = ?1
                  AND status IN ('pending', 'update')
                  AND is_status_manual = 0
                  AND id <> ?2;
                "#,
                params![request.pr_key, existing_id],
            )?;

            transaction.commit()?;
            return Ok(false);
        }

        let existed_same_message: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM review_requests WHERE slack_message_ts = ?1 AND pr_key = ?2);",
            params![request.slack_message_ts, request.pr_key],
            |row| row.get::<_, i64>(0),
        )? != 0;

        transaction.execute(
            r#"
            INSERT INTO review_requests (
              id, pr_key, pr_url, pr_title, repo_owner, repo_name, pr_number,
              pr_author_login, pr_merged_at,
              requester_slack_user_id, requester_display_name, slack_channel_id,
              slack_message_ts, slack_permalink, slack_text, deadline_date, status, is_status_manual,
              completed_at, completion_event_id, created_at, updated_at
            ) VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7,
              ?8, ?9,
              ?10, ?11, ?12,
              ?13, ?14, ?15, ?16, ?17, ?18,
              ?19, ?20, ?21, ?22
            )
            ON CONFLICT(slack_message_ts, pr_key) DO UPDATE SET
              pr_url = excluded.pr_url,
              pr_title = excluded.pr_title,
              repo_owner = excluded.repo_owner,
              repo_name = excluded.repo_name,
              pr_number = excluded.pr_number,
              pr_author_login = excluded.pr_author_login,
              pr_merged_at = excluded.pr_merged_at,
              requester_slack_user_id = excluded.requester_slack_user_id,
              requester_display_name = excluded.requester_display_name,
              slack_channel_id = excluded.slack_channel_id,
              slack_permalink = excluded.slack_permalink,
              slack_text = excluded.slack_text,
              deadline_date = excluded.deadline_date,
              updated_at = excluded.updated_at;
            "#,
            params![
                request.id,
                request.pr_key,
                request.pr_url,
                request.pr_title,
                request.repo_owner,
                request.repo_name,
                request.pr_number,
                request.pr_author_login,
                request.pr_merged_at,
                request.requester_slack_user_id,
                request.requester_display_name,
                request.slack_channel_id,
                request.slack_message_ts,
                request.slack_permalink,
                request.slack_text,
                request.deadline_date,
                request.status,
                request.is_status_manual as i64,
                request.completed_at,
                request.completion_event_id,
                request.created_at,
                request.updated_at,
            ],
        )?;
        transaction.commit()?;
        Ok(!existed_same_message)
    }

    fn update_review_request_deadline(
        &self,
        review_request_id: &str,
        deadline_date: &str,
    ) -> Result<()> {
        let updated_at = utc_now_string();
        let connection = self.connection()?;
        connection.execute(
            "UPDATE review_requests SET deadline_date = ?1, updated_at = ?2 WHERE id = ?3;",
            params![deadline_date, updated_at, review_request_id],
        )?;
        Ok(())
    }

    fn set_review_request_status_manual(
        &self,
        review_request_id: &str,
        status: ReviewStatus,
    ) -> Result<()> {
        let updated_at = utc_now_string();
        let completed_at = match status {
            ReviewStatus::Done => Some(updated_at.clone()),
            _ => None,
        };
        let connection = self.connection()?;
        connection.execute(
            r#"
            UPDATE review_requests
            SET status = ?1,
                is_status_manual = 1,
                completed_at = ?2,
                completion_event_id = NULL,
                updated_at = ?3
            WHERE id = ?4;
            "#,
            params![status.as_str(), completed_at, updated_at, review_request_id],
        )?;
        Ok(())
    }

    fn tracked_pr_keys(&self) -> Result<Vec<String>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT DISTINCT pr_key FROM review_requests WHERE status IN ('pending', 'update');",
        )?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        collect_rows(rows)
    }

    fn should_fetch_comment_events(&self, pr_key: &str, github_username: &str) -> Result<bool> {
        let connection = self.connection()?;
        let needs_comment_scan: bool = connection.query_row(
            r#"
            SELECT EXISTS(
              SELECT 1
              FROM review_requests
              WHERE pr_key = ?1
                AND status IN ('pending', 'update')
                AND (
                  pr_author_login IS NULL
                  OR lower(pr_author_login) = lower(?2)
                )
            );
            "#,
            params![pr_key, github_username],
            |row| row.get::<_, i64>(0),
        )? != 0;
        Ok(needs_comment_scan)
    }

    fn refresh_review_request_pr_metadata(
        &self,
        pr_key: &str,
        pr_title: &str,
        pr_author_login: Option<&str>,
        pr_merged_at: Option<&str>,
    ) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            r#"
            UPDATE review_requests
            SET pr_title = ?1,
                pr_author_login = ?2,
                pr_merged_at = ?3
            WHERE pr_key = ?4;
            "#,
            params![pr_title, pr_author_login, pr_merged_at, pr_key],
        )?;
        Ok(())
    }

    fn latest_event_at_for_pr(&self, pr_key: &str) -> Result<Option<String>> {
        let connection = self.connection()?;
        Ok(connection
            .query_row(
                "SELECT MAX(event_at) FROM github_events WHERE pr_key = ?1;",
                [pr_key],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    fn upsert_github_event(&self, event: &GithubEvent) -> Result<bool> {
        let connection = self.connection()?;
        let exists: bool = connection
            .query_row(
                "SELECT 1 FROM github_events WHERE id = ?1 LIMIT 1;",
                [&event.id],
                |_row| Ok(true),
            )
            .optional()?
            .unwrap_or(false);

        connection.execute(
            r#"
            INSERT INTO github_events (
              id, pr_key, pr_title, repo_owner, repo_name, pr_number, pr_author_login,
              notification_thread_id, notification_reason, event_kind,
              actor_login, actor_is_me, related_to_me, event_at, payload_json, created_at, read_at
            ) VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7,
              ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17
            )
            ON CONFLICT(id) DO UPDATE SET
              pr_key = excluded.pr_key,
              pr_title = excluded.pr_title,
              repo_owner = excluded.repo_owner,
              repo_name = excluded.repo_name,
              pr_number = excluded.pr_number,
              pr_author_login = excluded.pr_author_login,
              notification_thread_id = excluded.notification_thread_id,
              notification_reason = excluded.notification_reason,
              event_kind = excluded.event_kind,
              actor_login = excluded.actor_login,
              actor_is_me = excluded.actor_is_me,
              related_to_me = excluded.related_to_me,
              event_at = excluded.event_at,
              payload_json = excluded.payload_json,
              read_at = excluded.read_at;
            "#,
            params![
                event.id,
                event.pr_key,
                event.pr_title,
                event.repo_owner,
                event.repo_name,
                event.pr_number,
                event.pr_author_login,
                event.notification_thread_id,
                event.notification_reason,
                event.event_kind,
                event.actor_login,
                event.actor_is_me as i64,
                event.related_to_me as i64,
                event.event_at,
                event.payload_json,
                event.created_at,
                event.read_at,
            ],
        )?;
        Ok(!exists)
    }

    fn mark_github_events_read(&self, event_ids: &[String]) -> Result<()> {
        if event_ids.is_empty() {
            return Ok(());
        }

        let read_at = utc_now_string();
        let placeholders = (0..event_ids.len())
            .map(|index| format!("?{}", index + 2))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "UPDATE github_events SET read_at = ?1 WHERE id IN ({placeholders}) AND read_at IS NULL;"
        );

        let mut values: Vec<&dyn ToSql> = Vec::with_capacity(event_ids.len() + 1);
        values.push(&read_at);
        for event_id in event_ids {
            values.push(event_id);
        }

        let connection = self.connection()?;
        connection.execute(&sql, values.as_slice())?;
        Ok(())
    }

    fn mark_requests_done_by_pr_key(
        &self,
        pr_key: &str,
        completion_event_id: &str,
        completed_at: &str,
    ) -> Result<u64> {
        let connection = self.connection()?;
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM review_requests WHERE pr_key = ?1 AND status = 'pending';",
            [pr_key],
            |row| row.get(0),
        )?;
        if count == 0 {
            return Ok(0);
        }

        connection.execute(
            r#"
            UPDATE review_requests
            SET status = 'done',
                is_status_manual = 0,
                completed_at = ?1,
                completion_event_id = ?2,
                updated_at = ?3
            WHERE pr_key = ?4 AND status = 'pending';
            "#,
            params![completed_at, completion_event_id, completed_at, pr_key],
        )?;
        Ok(count as u64)
    }

    fn get_sync_state(&self, source: &str) -> Result<SyncState> {
        let connection = self.connection()?;
        self.get_sync_state_with_connection(&connection, source)
    }

    fn save_sync_state(&self, sync_state: &SyncState) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            r#"
            INSERT INTO sync_state (
              source, last_polled_at, last_seen_slack_ts, github_last_modified,
              github_etag, github_poll_interval_seconds, last_success_at, last_error, consecutive_failures
            ) VALUES (
              ?1, ?2, ?3, ?4,
              ?5, ?6, ?7, ?8, ?9
            )
            ON CONFLICT(source) DO UPDATE SET
              last_polled_at = excluded.last_polled_at,
              last_seen_slack_ts = excluded.last_seen_slack_ts,
              github_last_modified = excluded.github_last_modified,
              github_etag = excluded.github_etag,
              github_poll_interval_seconds = excluded.github_poll_interval_seconds,
              last_success_at = excluded.last_success_at,
              last_error = excluded.last_error,
              consecutive_failures = excluded.consecutive_failures;
            "#,
            params![
                sync_state.source,
                sync_state.last_polled_at,
                sync_state.last_seen_slack_ts,
                sync_state.github_last_modified,
                sync_state.github_etag,
                sync_state
                    .github_poll_interval_seconds
                    .map(|value| value as i64),
                sync_state.last_success_at,
                sync_state.last_error,
                sync_state.consecutive_failures as i64,
            ],
        )?;
        Ok(())
    }

    fn dump(
        &self,
        done_limit: usize,
        status: &str,
        last_error: Option<String>,
        github_username: &str,
        github_related_updates_only: bool,
        slack_user_id: &str,
        slack_username: &str,
    ) -> Result<ReviewDump> {
        let connection = self.connection()?;
        let (pending, mut done, update, all_events) =
            self.categorized_requests(&connection, github_username, slack_user_id, slack_username)?;
        let all_requests = self.fetch_review_requests(&connection)?;
        let update_feed = self.build_update_feed(
            &all_requests,
            &all_events,
            github_username,
            github_related_updates_only,
        );
        let slack_sync = integration_status_from_sync_state(
            self.get_sync_state_with_connection(&connection, "slack_search")?,
        );
        let github_sync = integration_status_from_sync_state(
            self.get_sync_state_with_connection(&connection, "github_notifications")?,
        );
        done.truncate(done_limit);
        Ok(ReviewDump {
            pending,
            done,
            update,
            update_feed,
            recent_events: all_events.into_iter().take(20).collect(),
            tray_state: TrayState {
                pending_count: 0,
                done_count: 0,
                update_count: 0,
                last_sync_at: None,
                status: String::new(),
                last_error: None,
            },
            integrations: IntegrationsSummary {
                slack: slack_sync,
                github: github_sync,
            },
        })
        .and_then(|mut dump| {
            let (pending_count, done_count, update_count) = (
                dump.pending.len() as u64,
                dump.done.len() as u64,
                dump.update.len() as u64,
            );
            dump.tray_state = TrayState {
                pending_count,
                done_count,
                update_count,
                last_sync_at: format_last_sync(
                    self.last_sync_at_with_connection(&connection)?.as_deref(),
                ),
                status: status.to_string(),
                last_error,
            };
            Ok(dump)
        })
    }

    fn tray_state(
        &self,
        status: &str,
        last_error: Option<String>,
        github_username: &str,
        slack_user_id: &str,
        slack_username: &str,
    ) -> Result<TrayState> {
        let connection = self.connection()?;
        let (pending, done, update, _) =
            self.categorized_requests(&connection, github_username, slack_user_id, slack_username)?;
        Ok(TrayState {
            pending_count: pending.len() as u64,
            done_count: done.len() as u64,
            update_count: update.len() as u64,
            last_sync_at: format_last_sync(
                self.last_sync_at_with_connection(&connection)?.as_deref(),
            ),
            status: status.to_string(),
            last_error,
        })
    }

    fn last_error_message(&self) -> Result<Option<String>> {
        let connection = self.connection()?;
        self.last_error_message_with_connection(&connection)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use anyhow::Result;
    use chrono::{Duration as ChronoDuration, Local, Utc};

    use crate::models::{GithubEvent, GithubPullRef, ReviewRequest, ReviewStatus};

    use super::{ReviewStore, SqliteStore};

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
    fn refreshes_existing_pending_request_for_same_pr() -> Result<()> {
        let store = SqliteStore::new(temp_db_path("refresh-same-pr"))?;
        store.init_schema()?;

        let pull = GithubPullRef {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            number: 7,
        };

        let first = ReviewRequest::new(
            &pull,
            "Initial title".to_string(),
            Some("author".to_string()),
            None,
            "U111".to_string(),
            "first requester".to_string(),
            Some("C111".to_string()),
            "1711930000.100000".to_string(),
            Some("https://slack.com/archives/C111/p1711930000100000".to_string()),
            "first message".to_string(),
            Some("2026-04-02".to_string()),
        );
        assert!(store.upsert_review_request(&first)?);

        let second = ReviewRequest::new(
            &pull,
            "Updated title".to_string(),
            Some("author".to_string()),
            None,
            "U222".to_string(),
            "second requester".to_string(),
            Some("C222".to_string()),
            "1711933600.200000".to_string(),
            Some("https://slack.com/archives/C222/p1711933600200000".to_string()),
            "second message".to_string(),
            Some("2026-04-05".to_string()),
        );
        assert!(!store.upsert_review_request(&second)?);

        let dump = store.dump(10, "OK", None, "me", false, "", "")?;
        assert_eq!(dump.pending.len(), 1);

        let pending = &dump.pending[0];
        assert_eq!(pending.id, first.id);
        assert_eq!(pending.pr_title, "Updated title");
        assert_eq!(pending.requester_slack_user_id, "U222");
        assert_eq!(pending.requester_display_name, "second requester");
        assert_eq!(pending.slack_channel_id.as_deref(), Some("C222"));
        assert_eq!(pending.slack_message_ts, "1711933600.200000");
        assert_eq!(pending.slack_text, "second message");
        assert_eq!(pending.deadline_date.as_deref(), Some("2026-04-05"));
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
}
