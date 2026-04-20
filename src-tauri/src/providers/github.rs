use std::{
    collections::HashMap,
    fs,
    process::Command,
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Context, Error, Result};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;

use crate::{
    keychain::{CredentialStore, GITHUB_TOKEN_ACCOUNT},
    models::{
        utc_now_string, EventKind, GithubEvent, GithubNotificationThread, GithubPullRef,
        NotificationsPollResult, PullRequestMetadata, SyncState,
    },
};

pub fn is_related_reason(reason: &str) -> bool {
    matches!(
        reason,
        "review_requested" | "mention" | "team_mention" | "author"
    )
}

pub fn is_access_denied_error(error: &Error) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .contains("github access denied")
}

pub struct LocalGithubProvider {
    credentials: Arc<dyn CredentialStore>,
    current_user_login: Mutex<Option<String>>,
}

impl LocalGithubProvider {
    pub fn new(credentials: Arc<dyn CredentialStore>) -> Self {
        Self {
            credentials,
            current_user_login: Mutex::new(None),
        }
    }

    fn token(&self) -> Result<String> {
        self.credentials
            .get(GITHUB_TOKEN_ACCOUNT)?
            .ok_or_else(|| anyhow!("missing GitHub token; run `review-please setup`"))
    }

    fn base_headers(token: &str) -> [String; 2] {
        [
            format!("Authorization: Bearer {token}"),
            "Accept: application/vnd.github+json".to_string(),
        ]
    }

    fn request_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        let (status, headers, body) = self.curl_with_headers(url, &[])?;
        if let Some(error) = Self::build_rate_limit_error(url, &headers, &body) {
            return Err(error);
        }
        if matches!(status, 403 | 404) {
            return Err(anyhow!(
                "GitHub access denied for {} (HTTP {})",
                url,
                status
            ));
        }
        if status >= 400 {
            let preview = body.chars().take(240).collect::<String>();
            return Err(anyhow!(
                "GitHub API {} returned HTTP {}: {}",
                url,
                status,
                preview
            ));
        }
        serde_json::from_str(&body).with_context(|| {
            let preview = body.chars().take(240).collect::<String>();
            format!("failed to decode GitHub response from {}: {}", url, preview)
        })
    }

    fn build_rate_limit_error(
        url: &str,
        headers: &HashMap<String, String>,
        body: &str,
    ) -> Option<Error> {
        let body_lower = body.to_ascii_lowercase();
        let remaining = headers
            .get("x-ratelimit-remaining")
            .map(String::as_str)
            .unwrap_or_default();
        if remaining != "0" && !body_lower.contains("rate limit exceeded") {
            return None;
        }
        let reset_suffix = headers
            .get("x-ratelimit-reset")
            .map(|value| format!(" until unix timestamp {value}"))
            .unwrap_or_default();
        Some(anyhow!(
            "GitHub API rate limit exceeded for {url}{reset_suffix}"
        ))
    }

    fn curl_with_headers(
        &self,
        url: &str,
        extra_headers: &[String],
    ) -> Result<(u16, HashMap<String, String>, String)> {
        let token = self.token()?;
        let header_path = std::env::temp_dir().join(format!(
            "review-please-headers-{}-{}.txt",
            std::process::id(),
            utc_now_string().replace([':', '.'], "-")
        ));
        let mut command = Command::new("curl");
        command
            .arg("-sS")
            .arg("-L")
            .arg("-D")
            .arg(&header_path)
            .arg("-o")
            .arg("-")
            .arg("-w")
            .arg("\n__CURL_STATUS__:%{http_code}")
            .arg(url)
            .arg("-H")
            .arg("User-Agent: review-please/0.1.0");
        for header in Self::base_headers(&token) {
            command.arg("-H").arg(header);
        }
        for header in extra_headers {
            command.arg("-H").arg(header);
        }
        let output = command
            .output()
            .with_context(|| format!("failed to call GitHub API {url}"))?;
        if !output.status.success() {
            let _ = fs::remove_file(&header_path);
            return Err(anyhow!(
                "curl failed for GitHub API {}: {}",
                url,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let (body, status_raw) = stdout
            .rsplit_once("\n__CURL_STATUS__:")
            .ok_or_else(|| anyhow!("missing curl status marker"))?;
        let status = status_raw
            .trim()
            .parse::<u16>()
            .context("invalid curl HTTP status")?;
        let headers_raw = fs::read_to_string(&header_path).unwrap_or_default();
        let _ = fs::remove_file(&header_path);
        let mut headers = HashMap::new();
        for line in headers_raw.lines() {
            if let Some((name, value)) = line.split_once(':') {
                headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
            }
        }
        Ok((status, headers, body.to_string()))
    }

    fn parse_pull_from_api_url(url: &str) -> Option<GithubPullRef> {
        let regex =
            Regex::new(r"https://api\.github\.com/repos/([^/\s]+)/([^/\s]+)/pulls/(\d+)").ok()?;
        let captures = regex.captures(url)?;
        Some(GithubPullRef {
            owner: captures[1].to_string(),
            repo: captures[2].to_string(),
            number: captures[3].parse::<i64>().ok()?,
        })
    }

    fn collect_review_events(
        &self,
        pull: &GithubPullRef,
        reason: &str,
        thread_id: &str,
        since: Option<&str>,
        current_user_login: &str,
    ) -> Result<Vec<GithubEvent>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/reviews?per_page=100",
            pull.owner, pull.repo, pull.number
        );
        let reviews: Vec<PullReview> = self.request_json(&url)?;
        Ok(reviews
            .into_iter()
            .filter_map(|review| {
                let event_at = review.submitted_at.clone()?;
                if since
                    .map(|value| event_at.as_str() <= value)
                    .unwrap_or(false)
                {
                    return None;
                }
                let event_kind = match review.state.as_deref() {
                    Some("APPROVED") => EventKind::Approved,
                    Some("CHANGES_REQUESTED") => EventKind::ChangesRequested,
                    Some("COMMENTED") => EventKind::Commented,
                    Some("PENDING") => return None,
                    _ => EventKind::Unknown,
                };
                let actor_login = review.user.clone().and_then(|user| user.login);
                Some(GithubEvent {
                    id: format!("review:{}", review.id),
                    pr_key: pull.key(),
                    pr_title: None,
                    repo_owner: None,
                    repo_name: None,
                    pr_number: None,
                    pr_author_login: None,
                    notification_thread_id: thread_id.to_string(),
                    notification_reason: reason.to_string(),
                    event_kind: event_kind.as_str().to_string(),
                    actor_login: actor_login.clone(),
                    actor_is_me: actor_login
                        .as_ref()
                        .map(|login| login == current_user_login)
                        .unwrap_or(false),
                    related_to_me: is_related_reason(reason),
                    event_at,
                    payload_json: json!(review).to_string(),
                    created_at: utc_now_string(),
                    read_at: None,
                })
            })
            .collect())
    }

    fn collect_issue_comment_events(
        &self,
        pull: &GithubPullRef,
        reason: &str,
        thread_id: &str,
        since: Option<&str>,
        current_user_login: &str,
    ) -> Result<Vec<GithubEvent>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues/{}/comments?per_page=100",
            pull.owner, pull.repo, pull.number
        );
        let comments: Vec<IssueComment> = self.request_json(&url)?;
        Ok(comments
            .into_iter()
            .filter_map(|comment| {
                let event_at = comment.created_at.clone()?;
                if since
                    .map(|value| event_at.as_str() <= value)
                    .unwrap_or(false)
                {
                    return None;
                }
                let actor_login = comment.user.clone().and_then(|user| user.login);
                Some(GithubEvent {
                    id: format!("issue_comment:{}", comment.id),
                    pr_key: pull.key(),
                    pr_title: None,
                    repo_owner: None,
                    repo_name: None,
                    pr_number: None,
                    pr_author_login: None,
                    notification_thread_id: thread_id.to_string(),
                    notification_reason: reason.to_string(),
                    event_kind: EventKind::Commented.as_str().to_string(),
                    actor_login: actor_login.clone(),
                    actor_is_me: actor_login
                        .as_ref()
                        .map(|login| login == current_user_login)
                        .unwrap_or(false),
                    related_to_me: is_related_reason(reason),
                    event_at,
                    payload_json: json!(comment).to_string(),
                    created_at: utc_now_string(),
                    read_at: None,
                })
            })
            .collect())
    }

    fn collect_review_comment_events(
        &self,
        pull: &GithubPullRef,
        reason: &str,
        thread_id: &str,
        since: Option<&str>,
        current_user_login: &str,
    ) -> Result<Vec<GithubEvent>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/comments?per_page=100",
            pull.owner, pull.repo, pull.number
        );
        let comments: Vec<ReviewComment> = self.request_json(&url)?;
        Ok(comments
            .into_iter()
            .filter_map(|comment| {
                let event_at = comment.created_at.clone()?;
                if since
                    .map(|value| event_at.as_str() <= value)
                    .unwrap_or(false)
                {
                    return None;
                }
                let actor_login = comment.user.clone().and_then(|user| user.login);
                Some(GithubEvent {
                    id: format!("review_comment:{}", comment.id),
                    pr_key: pull.key(),
                    pr_title: None,
                    repo_owner: None,
                    repo_name: None,
                    pr_number: None,
                    pr_author_login: None,
                    notification_thread_id: thread_id.to_string(),
                    notification_reason: reason.to_string(),
                    event_kind: EventKind::ReviewCommented.as_str().to_string(),
                    actor_login: actor_login.clone(),
                    actor_is_me: actor_login
                        .as_ref()
                        .map(|login| login == current_user_login)
                        .unwrap_or(false),
                    related_to_me: is_related_reason(reason),
                    event_at,
                    payload_json: json!(comment).to_string(),
                    created_at: utc_now_string(),
                    read_at: None,
                })
            })
            .collect())
    }
}

impl super::GithubProvider for LocalGithubProvider {
    fn current_user_login(&self) -> Result<String> {
        if let Some(login) = self
            .current_user_login
            .lock()
            .map_err(|_| anyhow!("failed to lock GitHub login cache"))?
            .clone()
        {
            return Ok(login);
        }
        let response: CurrentUser = self.request_json("https://api.github.com/user")?;
        let login = response.login;
        *self
            .current_user_login
            .lock()
            .map_err(|_| anyhow!("failed to lock GitHub login cache"))? = Some(login.clone());
        Ok(login)
    }

    fn fetch_pr_metadata(&self, pull: &GithubPullRef) -> Result<PullRequestMetadata> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}",
            pull.owner, pull.repo, pull.number
        );
        let response: PullRequestResponse = self.request_json(&url)?;
        Ok(PullRequestMetadata {
            title: response.title,
            author_login: response.user.and_then(|user| user.login),
            merged_at: response.merged_at,
        })
    }

    fn fetch_latest_approval_event(
        &self,
        pull: &GithubPullRef,
        current_user_login: &str,
    ) -> Result<Option<GithubEvent>> {
        let mut events = self.collect_review_events(
            pull,
            "tracked_reconcile",
            &format!("tracked:{}", pull.key()),
            None,
            current_user_login,
        )?;
        events.retain(|event| {
            event.actor_is_me && event.event_kind == EventKind::Approved.as_str()
        });
        events.sort_by(|left, right| left.event_at.cmp(&right.event_at));

        let Some(mut event) = events.pop() else {
            return Ok(None);
        };
        event.repo_owner = Some(pull.owner.clone());
        event.repo_name = Some(pull.repo.clone());
        event.pr_number = Some(pull.number);
        Ok(Some(event))
    }

    fn fetch_notifications(
        &self,
        sync_state: &SyncState,
        min_poll_interval_seconds: u64,
    ) -> Result<NotificationsPollResult> {
        let mut extra_headers = Vec::new();
        if let Some(last_modified) = &sync_state.github_last_modified {
            extra_headers.push(format!("If-Modified-Since: {last_modified}"));
        }
        if let Some(etag) = &sync_state.github_etag {
            extra_headers.push(format!("If-None-Match: {etag}"));
        }
        let (status, headers, body) = self.curl_with_headers(
            "https://api.github.com/notifications?all=true&per_page=100",
            &extra_headers,
        )?;
        let poll_interval = headers
            .get("x-poll-interval")
            .and_then(|value| value.parse::<u64>().ok());
        let etag = headers.get("etag").cloned();
        let last_modified = headers.get("last-modified").cloned();

        if status == 304 {
            return Ok(NotificationsPollResult {
                threads: Vec::new(),
                not_modified: true,
                poll_interval_seconds: poll_interval.or(Some(min_poll_interval_seconds)),
                etag,
                last_modified,
            });
        }
        if let Some(error) = Self::build_rate_limit_error(
            "https://api.github.com/notifications?all=true&per_page=100",
            &headers,
            &body,
        ) {
            return Err(error);
        }
        if !(200..300).contains(&status) {
            return Err(anyhow!("GitHub notifications returned HTTP {status}"));
        }
        let threads: Vec<NotificationThreadResponse> =
            serde_json::from_str(&body).context("failed to decode notifications")?;
        Ok(NotificationsPollResult {
            threads: threads
                .into_iter()
                .map(|thread| GithubNotificationThread {
                    id: thread.id,
                    unread: thread.unread,
                    last_read_at: thread.last_read_at,
                    reason: thread.reason,
                    subject_type: thread.subject.r#type,
                    subject_title: thread.subject.title,
                    pull: thread
                        .subject
                        .url
                        .as_deref()
                        .and_then(Self::parse_pull_from_api_url),
                    updated_at: thread.updated_at,
                })
                .collect(),
            not_modified: false,
            poll_interval_seconds: poll_interval.or(Some(min_poll_interval_seconds)),
            etag,
            last_modified,
        })
    }

    fn fetch_events_for_thread(
        &self,
        thread: &GithubNotificationThread,
        since: Option<&str>,
        current_user_login: &str,
        include_comment_events: bool,
    ) -> Result<Vec<GithubEvent>> {
        let pull = thread
            .pull
            .as_ref()
            .ok_or_else(|| anyhow!("notification thread {} is missing PR metadata", thread.id))?;
        let mut events = Vec::new();
        events.extend(self.collect_review_events(
            pull,
            &thread.reason,
            &thread.id,
            since,
            current_user_login,
        )?);
        if include_comment_events {
            events.extend(self.collect_issue_comment_events(
                pull,
                &thread.reason,
                &thread.id,
                since,
                current_user_login,
            )?);
            events.extend(self.collect_review_comment_events(
                pull,
                &thread.reason,
                &thread.id,
                since,
                current_user_login,
            )?);
        }
        events.sort_by(|left, right| left.event_at.cmp(&right.event_at));
        if events.is_empty() {
            events.push(GithubEvent {
                id: format!("unknown:{}:{}", thread.id, pull.key()),
                pr_key: pull.key(),
                pr_title: thread.subject_title.clone(),
                repo_owner: Some(pull.owner.clone()),
                repo_name: Some(pull.repo.clone()),
                pr_number: Some(pull.number),
                pr_author_login: None,
                notification_thread_id: thread.id.clone(),
                notification_reason: thread.reason.clone(),
                event_kind: EventKind::Unknown.as_str().to_string(),
                actor_login: None,
                actor_is_me: false,
                related_to_me: is_related_reason(&thread.reason),
                event_at: thread.updated_at.clone().unwrap_or_else(utc_now_string),
                payload_json: json!(thread).to_string(),
                created_at: utc_now_string(),
                read_at: if thread.unread {
                    None
                } else {
                    thread
                        .last_read_at
                        .clone()
                        .or_else(|| Some(utc_now_string()))
                },
            });
        }
        for event in &mut events {
            event.pr_title = thread.subject_title.clone();
            event.repo_owner = Some(pull.owner.clone());
            event.repo_name = Some(pull.repo.clone());
            event.pr_number = Some(pull.number);
            if !thread.unread {
                event.read_at = thread
                    .last_read_at
                    .clone()
                    .or_else(|| Some(utc_now_string()));
            }
        }
        Ok(events)
    }
}

#[derive(Debug, Deserialize)]
struct CurrentUser {
    login: String,
}

#[derive(Debug, Deserialize)]
struct PullRequestResponse {
    title: String,
    merged_at: Option<String>,
    user: Option<GithubActor>,
}

#[derive(Debug, Deserialize)]
struct NotificationThreadResponse {
    id: String,
    unread: bool,
    reason: String,
    updated_at: Option<String>,
    last_read_at: Option<String>,
    subject: NotificationSubject,
}

#[derive(Debug, Deserialize)]
struct NotificationSubject {
    title: Option<String>,
    url: Option<String>,
    #[serde(rename = "type")]
    r#type: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct PullReview {
    id: i64,
    state: Option<String>,
    body: Option<String>,
    html_url: Option<String>,
    submitted_at: Option<String>,
    user: Option<GithubActor>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct IssueComment {
    id: i64,
    body: Option<String>,
    html_url: Option<String>,
    created_at: Option<String>,
    user: Option<GithubActor>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct ReviewComment {
    id: i64,
    body: Option<String>,
    html_url: Option<String>,
    created_at: Option<String>,
    user: Option<GithubActor>,
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
struct GithubActor {
    login: Option<String>,
}
