use std::{fs, path::PathBuf, process::Command};

use anyhow::{anyhow, Context, Result};
use serde::de::DeserializeOwned;

use crate::{
    config,
    models::{
        format_last_sync, utc_now_string, GithubEvent, ReviewDump, ReviewRequest, ReviewStatus,
        SyncState, TrayState,
    },
    providers::slack::{extract_deadline, slack_ts_to_local_date},
    services::review_state::classify_review_request,
};

pub trait ReviewStore: Send + Sync {
    fn init_schema(&self) -> Result<()>;
    fn clear_state(&self) -> Result<()>;
    fn review_request_exists(&self, slack_message_ts: &str, pr_key: &str) -> Result<bool>;
    fn upsert_review_request(&self, request: &ReviewRequest) -> Result<()>;
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
    fn refresh_review_request_pr_metadata(
        &self,
        pr_key: &str,
        pr_title: &str,
        pr_author_login: Option<&str>,
        pr_merged_at: Option<&str>,
    ) -> Result<()>;
    fn latest_event_at_for_pr(&self, pr_key: &str) -> Result<Option<String>>;
    fn github_event_exists(&self, event_id: &str) -> Result<bool>;
    fn insert_github_event(&self, event: &GithubEvent) -> Result<()>;
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
    ) -> Result<ReviewDump>;
    fn tray_state(
        &self,
        status: &str,
        last_error: Option<String>,
        github_username: &str,
    ) -> Result<TrayState>;
    fn last_sync_at(&self) -> Result<Option<String>>;
    fn last_error_message(&self) -> Result<Option<String>>;
}

#[derive(Debug, Clone)]
pub struct SqliteStore {
    db_path: PathBuf,
}

impl SqliteStore {
    pub fn new(path: PathBuf) -> Self {
        Self { db_path: path }
    }

    pub fn from_default_location() -> Result<Self> {
        config::ensure_data_dir()?;
        Ok(Self::new(config::database_path()?))
    }
    fn execute(&self, sql: &str) -> Result<()> {
        config::ensure_parent(&self.db_path)?;
        let output = Command::new("sqlite3")
            .arg(&self.db_path)
            .arg(sql)
            .output()
            .with_context(|| format!("failed to run sqlite3 for {}", self.db_path.display()))?;
        if output.status.success() {
            return Ok(());
        }
        Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim().to_string()
        ))
    }

    fn query_json<T: DeserializeOwned>(&self, sql: &str) -> Result<Vec<T>> {
        config::ensure_parent(&self.db_path)?;
        let output = Command::new("sqlite3")
            .arg("-json")
            .arg(&self.db_path)
            .arg(sql)
            .output()
            .with_context(|| {
                format!("failed to run sqlite3 -json for {}", self.db_path.display())
            })?;
        if !output.status.success() {
            return Err(anyhow!(
                "{}",
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let body = if stdout.is_empty() {
            "[]".to_string()
        } else {
            stdout
        };
        serde_json::from_str(&body).context("failed to parse sqlite json output")
    }

    fn escape(value: &str) -> String {
        value.replace('\'', "''")
    }

    fn sql_string(value: &str) -> String {
        format!("'{}'", Self::escape(value))
    }

    fn sql_optional(value: Option<&str>) -> String {
        value
            .map(Self::sql_string)
            .unwrap_or_else(|| "NULL".to_string())
    }

    fn query_scalar_i64(&self, sql: &str) -> Result<i64> {
        #[derive(serde::Deserialize)]
        struct Row {
            value: i64,
        }
        Ok(self
            .query_json::<Row>(sql)?
            .into_iter()
            .next()
            .map(|row| row.value)
            .unwrap_or_default())
    }

    fn query_scalar_string(&self, sql: &str, field: &str) -> Result<Option<String>> {
        let rows = self.query_json::<serde_json::Value>(sql)?;
        Ok(rows.into_iter().next().and_then(|row| {
            row.get(field)
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        }))
    }

    fn categorized_requests(
        &self,
        github_username: &str,
    ) -> Result<(
        Vec<ReviewRequest>,
        Vec<ReviewRequest>,
        Vec<ReviewRequest>,
        Vec<GithubEvent>,
    )> {
        let requests = self.query_json::<ReviewRequest>(
            "SELECT * FROM review_requests ORDER BY created_at DESC;",
        )?;
        let all_events =
            self.query_json::<GithubEvent>("SELECT * FROM github_events ORDER BY event_at DESC;")?;
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
            let request_events: Vec<GithubEvent> = all_events
                .iter()
                .filter(|event| event.pr_key == request.pr_key)
                .cloned()
                .collect();
            let category = classify_review_request(&request, &request_events, github_username);
            match category {
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

    fn execute_allow_duplicate_column(&self, sql: &str) -> Result<()> {
        match self.execute(sql) {
            Ok(()) => Ok(()),
            Err(error) if error.to_string().contains("duplicate column name") => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl ReviewStore for SqliteStore {
    fn init_schema(&self) -> Result<()> {
        self.execute(
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
              notification_thread_id TEXT NOT NULL,
              notification_reason TEXT NOT NULL,
              event_kind TEXT NOT NULL,
              actor_login TEXT,
              actor_is_me INTEGER NOT NULL,
              related_to_me INTEGER NOT NULL,
              event_at TEXT NOT NULL,
              payload_json TEXT NOT NULL,
              created_at TEXT NOT NULL
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
        self.execute_allow_duplicate_column(
            "ALTER TABLE review_requests ADD COLUMN pr_author_login TEXT;",
        )?;
        self.execute_allow_duplicate_column(
            "ALTER TABLE review_requests ADD COLUMN pr_merged_at TEXT;",
        )?;
        self.execute_allow_duplicate_column(
            "ALTER TABLE review_requests ADD COLUMN is_status_manual INTEGER NOT NULL DEFAULT 0;",
        )
    }

    fn clear_state(&self) -> Result<()> {
        if self.db_path.exists() {
            fs::remove_file(&self.db_path)
                .with_context(|| format!("failed to remove {}", self.db_path.display()))?;
        }
        self.init_schema()
    }

    fn review_request_exists(&self, slack_message_ts: &str, pr_key: &str) -> Result<bool> {
        let sql = format!(
            "SELECT COUNT(*) AS value FROM review_requests WHERE slack_message_ts = {} AND pr_key = {};",
            Self::sql_string(slack_message_ts),
            Self::sql_string(pr_key),
        );
        Ok(self.query_scalar_i64(&sql)? > 0)
    }

    fn upsert_review_request(&self, request: &ReviewRequest) -> Result<()> {
        let sql = format!(
            "
            INSERT INTO review_requests (
              id, pr_key, pr_url, pr_title, repo_owner, repo_name, pr_number,
              pr_author_login, pr_merged_at,
              requester_slack_user_id, requester_display_name, slack_channel_id,
              slack_message_ts, slack_permalink, slack_text, deadline_date, status, is_status_manual,
              completed_at, completion_event_id, created_at, updated_at
            ) VALUES (
              {id}, {pr_key}, {pr_url}, {pr_title}, {repo_owner}, {repo_name}, {pr_number},
              {pr_author_login}, {pr_merged_at},
              {requester_slack_user_id}, {requester_display_name}, {slack_channel_id},
              {slack_message_ts}, {slack_permalink}, {slack_text}, {deadline_date}, {status}, {is_status_manual},
              {completed_at}, {completion_event_id}, {created_at}, {updated_at}
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
            ",
            id = Self::sql_string(&request.id),
            pr_key = Self::sql_string(&request.pr_key),
            pr_url = Self::sql_string(&request.pr_url),
            pr_title = Self::sql_string(&request.pr_title),
            repo_owner = Self::sql_string(&request.repo_owner),
            repo_name = Self::sql_string(&request.repo_name),
            pr_number = request.pr_number,
            pr_author_login = Self::sql_optional(request.pr_author_login.as_deref()),
            pr_merged_at = Self::sql_optional(request.pr_merged_at.as_deref()),
            requester_slack_user_id = Self::sql_string(&request.requester_slack_user_id),
            requester_display_name = Self::sql_string(&request.requester_display_name),
            slack_channel_id = Self::sql_optional(request.slack_channel_id.as_deref()),
            slack_message_ts = Self::sql_string(&request.slack_message_ts),
            slack_permalink = Self::sql_optional(request.slack_permalink.as_deref()),
            slack_text = Self::sql_string(&request.slack_text),
            deadline_date = Self::sql_optional(request.deadline_date.as_deref()),
            status = Self::sql_string(&request.status),
            is_status_manual = if request.is_status_manual { 1 } else { 0 },
            completed_at = Self::sql_optional(request.completed_at.as_deref()),
            completion_event_id = Self::sql_optional(request.completion_event_id.as_deref()),
            created_at = Self::sql_string(&request.created_at),
            updated_at = Self::sql_string(&request.updated_at),
        );
        self.execute(&sql)
    }

    fn update_review_request_deadline(
        &self,
        review_request_id: &str,
        deadline_date: &str,
    ) -> Result<()> {
        let updated_at = utc_now_string();
        let sql = format!(
            "
            UPDATE review_requests
            SET deadline_date = {deadline_date},
                updated_at = {updated_at}
            WHERE id = {review_request_id};
            ",
            deadline_date = Self::sql_string(deadline_date),
            updated_at = Self::sql_string(&updated_at),
            review_request_id = Self::sql_string(review_request_id),
        );
        self.execute(&sql)
    }

    fn set_review_request_status_manual(
        &self,
        review_request_id: &str,
        status: ReviewStatus,
    ) -> Result<()> {
        let updated_at = utc_now_string();
        let completed_at = match status {
            ReviewStatus::Done => Self::sql_string(&updated_at),
            _ => "NULL".to_string(),
        };
        let sql = format!(
            "
            UPDATE review_requests
            SET status = {status},
                is_status_manual = 1,
                completed_at = {completed_at},
                completion_event_id = NULL,
                updated_at = {updated_at}
            WHERE id = {review_request_id};
            ",
            status = Self::sql_string(status.as_str()),
            completed_at = completed_at,
            updated_at = Self::sql_string(&updated_at),
            review_request_id = Self::sql_string(review_request_id),
        );
        self.execute(&sql)
    }

    fn tracked_pr_keys(&self) -> Result<Vec<String>> {
        #[derive(serde::Deserialize)]
        struct Row {
            pr_key: String,
        }
        Ok(self
            .query_json::<Row>("SELECT DISTINCT pr_key FROM review_requests;")?
            .into_iter()
            .map(|row| row.pr_key)
            .collect())
    }

    fn refresh_review_request_pr_metadata(
        &self,
        pr_key: &str,
        pr_title: &str,
        pr_author_login: Option<&str>,
        pr_merged_at: Option<&str>,
    ) -> Result<()> {
        let sql = format!(
            "
            UPDATE review_requests
            SET pr_title = {pr_title},
                pr_author_login = {pr_author_login},
                pr_merged_at = {pr_merged_at}
            WHERE pr_key = {pr_key};
            ",
            pr_title = Self::sql_string(pr_title),
            pr_author_login = Self::sql_optional(pr_author_login),
            pr_merged_at = Self::sql_optional(pr_merged_at),
            pr_key = Self::sql_string(pr_key),
        );
        self.execute(&sql)
    }

    fn latest_event_at_for_pr(&self, pr_key: &str) -> Result<Option<String>> {
        let sql = format!(
            "SELECT MAX(event_at) AS value FROM github_events WHERE pr_key = {};",
            Self::sql_string(pr_key)
        );
        self.query_scalar_string(&sql, "value")
    }

    fn github_event_exists(&self, event_id: &str) -> Result<bool> {
        let sql = format!(
            "SELECT COUNT(*) AS value FROM github_events WHERE id = {};",
            Self::sql_string(event_id)
        );
        Ok(self.query_scalar_i64(&sql)? > 0)
    }

    fn insert_github_event(&self, event: &GithubEvent) -> Result<()> {
        let sql = format!(
            "
            INSERT OR IGNORE INTO github_events (
              id, pr_key, notification_thread_id, notification_reason, event_kind,
              actor_login, actor_is_me, related_to_me, event_at, payload_json, created_at
            ) VALUES (
              {id}, {pr_key}, {notification_thread_id}, {notification_reason}, {event_kind},
              {actor_login}, {actor_is_me}, {related_to_me}, {event_at}, {payload_json}, {created_at}
            );
            ",
            id = Self::sql_string(&event.id),
            pr_key = Self::sql_string(&event.pr_key),
            notification_thread_id = Self::sql_string(&event.notification_thread_id),
            notification_reason = Self::sql_string(&event.notification_reason),
            event_kind = Self::sql_string(&event.event_kind),
            actor_login = Self::sql_optional(event.actor_login.as_deref()),
            actor_is_me = if event.actor_is_me { 1 } else { 0 },
            related_to_me = if event.related_to_me { 1 } else { 0 },
            event_at = Self::sql_string(&event.event_at),
            payload_json = Self::sql_string(&event.payload_json),
            created_at = Self::sql_string(&event.created_at),
        );
        self.execute(&sql)
    }

    fn mark_requests_done_by_pr_key(
        &self,
        pr_key: &str,
        completion_event_id: &str,
        completed_at: &str,
    ) -> Result<u64> {
        let count = self.query_scalar_i64(&format!(
            "SELECT COUNT(*) AS value FROM review_requests WHERE pr_key = {} AND status = 'pending';",
            Self::sql_string(pr_key)
        ))?;
        if count == 0 {
            return Ok(0);
        }
        let sql = format!(
            "
            UPDATE review_requests
            SET status = 'done',
                is_status_manual = 0,
                completed_at = {completed_at},
                completion_event_id = {completion_event_id},
                updated_at = {updated_at}
            WHERE pr_key = {pr_key} AND status = 'pending';
            ",
            completed_at = Self::sql_string(completed_at),
            completion_event_id = Self::sql_string(completion_event_id),
            updated_at = Self::sql_string(completed_at),
            pr_key = Self::sql_string(pr_key),
        );
        self.execute(&sql)?;
        Ok(count as u64)
    }

    fn get_sync_state(&self, source: &str) -> Result<SyncState> {
        let sql = format!(
            "SELECT source, last_polled_at, last_seen_slack_ts, github_last_modified, github_etag, github_poll_interval_seconds, last_success_at, last_error, consecutive_failures FROM sync_state WHERE source = {};",
            Self::sql_string(source)
        );
        let row = self.query_json::<SyncState>(&sql)?.into_iter().next();
        Ok(row.unwrap_or_else(|| SyncState::new(source)))
    }

    fn save_sync_state(&self, sync_state: &SyncState) -> Result<()> {
        let sql = format!(
            "
            INSERT INTO sync_state (
              source, last_polled_at, last_seen_slack_ts, github_last_modified,
              github_etag, github_poll_interval_seconds, last_success_at, last_error, consecutive_failures
            ) VALUES (
              {source}, {last_polled_at}, {last_seen_slack_ts}, {github_last_modified},
              {github_etag}, {github_poll_interval_seconds}, {last_success_at}, {last_error}, {consecutive_failures}
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
            ",
            source = Self::sql_string(&sync_state.source),
            last_polled_at = Self::sql_optional(sync_state.last_polled_at.as_deref()),
            last_seen_slack_ts = Self::sql_optional(sync_state.last_seen_slack_ts.as_deref()),
            github_last_modified = Self::sql_optional(sync_state.github_last_modified.as_deref()),
            github_etag = Self::sql_optional(sync_state.github_etag.as_deref()),
            github_poll_interval_seconds = sync_state
                .github_poll_interval_seconds
                .map(|value| value.to_string())
                .unwrap_or_else(|| "NULL".to_string()),
            last_success_at = Self::sql_optional(sync_state.last_success_at.as_deref()),
            last_error = Self::sql_optional(sync_state.last_error.as_deref()),
            consecutive_failures = sync_state.consecutive_failures,
        );
        self.execute(&sql)
    }

    fn dump(
        &self,
        done_limit: usize,
        status: &str,
        last_error: Option<String>,
        github_username: &str,
    ) -> Result<ReviewDump> {
        let (pending, mut done, update, all_events) = self.categorized_requests(github_username)?;
        done.truncate(done_limit);
        Ok(ReviewDump {
            pending,
            done,
            update,
            recent_events: all_events.into_iter().take(20).collect(),
            tray_state: self.tray_state(status, last_error, github_username)?,
        })
    }

    fn tray_state(
        &self,
        status: &str,
        last_error: Option<String>,
        github_username: &str,
    ) -> Result<TrayState> {
        let (pending, done, update, _) = self.categorized_requests(github_username)?;
        Ok(TrayState {
            pending_count: pending.len() as u64,
            done_count: done.len() as u64,
            update_count: update.len() as u64,
            last_sync_at: format_last_sync(self.last_sync_at()?.as_deref()),
            status: status.to_string(),
            last_error,
        })
    }

    fn last_sync_at(&self) -> Result<Option<String>> {
        self.query_scalar_string(
            "SELECT MAX(last_success_at) AS value FROM sync_state;",
            "value",
        )
    }

    fn last_error_message(&self) -> Result<Option<String>> {
        self.query_scalar_string(
            "SELECT last_error AS value FROM sync_state WHERE last_error IS NOT NULL ORDER BY last_polled_at DESC LIMIT 1;",
            "value",
        )
    }
}
