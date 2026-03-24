use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Pending,
    Done,
    Update,
}

impl ReviewStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Done => "done",
            Self::Update => "update",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Commented,
    ReviewCommented,
    Approved,
    ChangesRequested,
    Merged,
    Closed,
    Unknown,
}

impl EventKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Commented => "commented",
            Self::ReviewCommented => "review_commented",
            Self::Approved => "approved",
            Self::ChangesRequested => "changes_requested",
            Self::Merged => "merged",
            Self::Closed => "closed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubPullRef {
    pub owner: String,
    pub repo: String,
    pub number: i64,
}

impl GithubPullRef {
    pub fn from_key(value: &str) -> Option<Self> {
        let (repo_path, number) = value.split_once('#')?;
        let (owner, repo) = repo_path.split_once('/')?;
        Some(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
            number: number.parse::<i64>().ok()?,
        })
    }

    pub fn key(&self) -> String {
        format!("{}/{}#{}", self.owner, self.repo, self.number)
    }

    pub fn html_url(&self) -> String {
        format!(
            "https://github.com/{}/{}/pull/{}",
            self.owner, self.repo, self.number
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub id: String,
    pub pr_key: String,
    pub pr_url: String,
    pub pr_title: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub pr_number: i64,
    pub pr_author_login: Option<String>,
    pub pr_merged_at: Option<String>,
    pub requester_slack_user_id: String,
    pub requester_display_name: String,
    pub slack_channel_id: Option<String>,
    pub slack_message_ts: String,
    pub slack_permalink: Option<String>,
    pub slack_text: String,
    pub deadline_date: Option<String>,
    pub status: String,
    #[serde(default, deserialize_with = "deserialize_bool_from_any")]
    pub is_status_manual: bool,
    pub completed_at: Option<String>,
    pub completion_event_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl ReviewRequest {
    pub fn new(
        pull: &GithubPullRef,
        pr_title: String,
        pr_author_login: Option<String>,
        pr_merged_at: Option<String>,
        requester_slack_user_id: String,
        requester_display_name: String,
        slack_channel_id: Option<String>,
        slack_message_ts: String,
        slack_permalink: Option<String>,
        slack_text: String,
        deadline_date: Option<String>,
    ) -> Self {
        let now = utc_now_string();
        Self {
            id: Uuid::new_v4().to_string(),
            pr_key: pull.key(),
            pr_url: pull.html_url(),
            pr_title,
            repo_owner: pull.owner.clone(),
            repo_name: pull.repo.clone(),
            pr_number: pull.number,
            pr_author_login,
            pr_merged_at,
            requester_slack_user_id,
            requester_display_name,
            slack_channel_id,
            slack_message_ts,
            slack_permalink,
            slack_text,
            deadline_date,
            status: ReviewStatus::Pending.as_str().to_string(),
            is_status_manual: false,
            completed_at: None,
            completion_event_id: None,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequestMetadata {
    pub title: String,
    pub author_login: Option<String>,
    pub merged_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubEvent {
    pub id: String,
    pub pr_key: String,
    pub notification_thread_id: String,
    pub notification_reason: String,
    pub event_kind: String,
    pub actor_login: Option<String>,
    #[serde(deserialize_with = "deserialize_bool_from_any")]
    pub actor_is_me: bool,
    #[serde(deserialize_with = "deserialize_bool_from_any")]
    pub related_to_me: bool,
    pub event_at: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub source: String,
    pub last_polled_at: Option<String>,
    pub last_seen_slack_ts: Option<String>,
    pub github_last_modified: Option<String>,
    pub github_etag: Option<String>,
    pub github_poll_interval_seconds: Option<u64>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub consecutive_failures: u64,
}

impl SyncState {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            last_polled_at: None,
            last_seen_slack_ts: None,
            github_last_modified: None,
            github_etag: None,
            github_poll_interval_seconds: None,
            last_success_at: None,
            last_error: None,
            consecutive_failures: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayState {
    pub pending_count: u64,
    pub done_count: u64,
    pub update_count: u64,
    pub last_sync_at: Option<String>,
    pub status: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDump {
    pub pending: Vec<ReviewRequest>,
    pub done: Vec<ReviewRequest>,
    pub update: Vec<ReviewRequest>,
    pub recent_events: Vec<GithubEvent>,
    pub tray_state: TrayState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackMessageRef {
    pub ts: String,
    pub channel_id: Option<String>,
    pub text: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubNotificationThread {
    pub id: String,
    pub reason: String,
    pub subject_type: String,
    pub subject_title: Option<String>,
    pub pull: Option<GithubPullRef>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsPollResult {
    pub threads: Vec<GithubNotificationThread>,
    pub not_modified: bool,
    pub poll_interval_seconds: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

pub fn utc_now_string() -> String {
    Utc::now().to_rfc3339()
}

pub fn parse_ts_number(ts: &str) -> f64 {
    ts.parse::<f64>().unwrap_or_default()
}

pub fn newer_ts(candidate: &str, current: Option<&str>) -> bool {
    current
        .map(|existing| parse_ts_number(candidate) > parse_ts_number(existing))
        .unwrap_or(true)
}

pub fn format_last_sync(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        DateTime::parse_from_rfc3339(raw).ok().map(|dt| {
            dt.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
    })
}

fn deserialize_bool_from_any<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Bool(value) => Ok(value),
        Value::Number(value) => Ok(value.as_i64().unwrap_or_default() != 0),
        Value::String(value) => match value.as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" | "" => Ok(false),
            _ => Err(serde::de::Error::custom(format!(
                "invalid boolean string: {value}"
            ))),
        },
        Value::Null => Ok(false),
        other => Err(serde::de::Error::custom(format!(
            "invalid boolean value: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::GithubEvent;

    #[test]
    fn github_event_deserializes_integer_booleans() {
        let json = r#"{
            "id":"event-1",
            "pr_key":"owner/repo#1",
            "notification_thread_id":"thread-1",
            "notification_reason":"mention",
            "event_kind":"approved",
            "actor_login":"sycha-front",
            "actor_is_me":1,
            "related_to_me":0,
            "event_at":"2026-03-23T00:00:00Z",
            "payload_json":"{}",
            "created_at":"2026-03-23T00:00:00Z"
        }"#;

        let event: GithubEvent =
            serde_json::from_str(json).expect("github event should deserialize");

        assert!(event.actor_is_me);
        assert!(!event.related_to_me);
    }
}
