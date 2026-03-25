use std::{
    collections::HashMap,
    fs,
    process::Command,
    sync::Arc,
};

use anyhow::{anyhow, Context, Error, Result};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;

use crate::{
    keychain::{CredentialStore, GITHUB_TOKEN_ACCOUNT},
    models::{
        EventKind, GithubEvent, GithubNotificationThread, GithubPullRef, NotificationsPollResult,
        PullRequestMetadata, SyncState, utc_now_string,
    },
};

pub fn is_related_reason(reason: &str) -> bool {
    matches!(reason, "review_requested" | "mention" | "team_mention" | "author")
}

pub fn is_access_denied_error(error: &Error) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .contains("github access denied")
}

pub struct LocalGithubProvider {
    credentials: Arc<dyn CredentialStore>,
}

impl LocalGithubProvider {
    pub fn new(credentials: Arc<dyn CredentialStore>) -> Self {
        Self { credentials }
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
        let (status, _headers, body) = self.curl_with_headers(url, &[])?;
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
            format!(
                "failed to decode GitHub response from {}: {}",
                url,
                preview
            )
        })
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
        let status = status_raw.trim().parse::<u16>().context("invalid curl HTTP status")?;
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
                if since.map(|value| event_at.as_str() <= value).unwrap_or(false) {
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
                if since.map(|value| event_at.as_str() <= value).unwrap_or(false) {
                    return None;
                }
                let actor_login = comment.user.clone().and_then(|user| user.login);
                Some(GithubEvent {
                    id: format!("issue_comment:{}", comment.id),
                    pr_key: pull.key(),
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
                if since.map(|value| event_at.as_str() <= value).unwrap_or(false) {
                    return None;
                }
                let actor_login = comment.user.clone().and_then(|user| user.login);
                Some(GithubEvent {
                    id: format!("review_comment:{}", comment.id),
                    pr_key: pull.key(),
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
        let response: CurrentUser = self.request_json("https://api.github.com/user")?;
        Ok(response.login)
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

    fn fetch_events_for_pull(
        &self,
        pull: &GithubPullRef,
        since: Option<&str>,
        current_user_login: &str,
    ) -> Result<Vec<GithubEvent>> {
        let thread_id = format!("scan:{}", pull.key());
        let reason = "direct_scan";
        let mut events = Vec::new();
        events.extend(self.collect_review_events(
            pull,
            reason,
            &thread_id,
            since,
            current_user_login,
        )?);
        events.extend(self.collect_issue_comment_events(
            pull,
            reason,
            &thread_id,
            since,
            current_user_login,
        )?);
        events.extend(self.collect_review_comment_events(
            pull,
            reason,
            &thread_id,
            since,
            current_user_login,
        )?);
        events.sort_by(|left, right| left.event_at.cmp(&right.event_at));
        Ok(events)
    }

    fn fetch_events_for_thread(
        &self,
        thread: &GithubNotificationThread,
        since: Option<&str>,
        current_user_login: &str,
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
        events.sort_by(|left, right| left.event_at.cmp(&right.event_at));
        if events.is_empty() {
            events.push(GithubEvent {
                id: format!("unknown:{}:{}", thread.id, pull.key()),
                pr_key: pull.key(),
                notification_thread_id: thread.id.clone(),
                notification_reason: thread.reason.clone(),
                event_kind: EventKind::Unknown.as_str().to_string(),
                actor_login: None,
                actor_is_me: false,
                related_to_me: is_related_reason(&thread.reason),
                event_at: thread.updated_at.clone().unwrap_or_else(utc_now_string),
                payload_json: json!(thread).to_string(),
                created_at: utc_now_string(),
                read_at: None,
            });
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
    reason: String,
    updated_at: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::{is_related_reason, LocalGithubProvider};

    #[test]
    fn related_reason_matches_expected_values() {
        assert!(is_related_reason("review_requested"));
        assert!(is_related_reason("mention"));
        assert!(!is_related_reason("comment"));
    }

    #[test]
    fn parses_pull_from_notification_url() {
        let pull = LocalGithubProvider::parse_pull_from_api_url(
            "https://api.github.com/repos/openai/app/pulls/17",
        )
        .expect("pull");
        assert_eq!(pull.key(), "openai/app#17");
    }
}
