use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
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

const REVIEW_REQUEST_REASON: &str = "review_requested";
const MENTION_REASON: &str = "mention";
const TEAM_MENTION_REASON: &str = "team_mention";
const AUTHOR_REASON: &str = "author";
const NOTIFICATIONS_URL: &str = "https://api.github.com/notifications?all=true&per_page=100";
const USER_AGENT: &str = "review-please/0.1.0";
const PULL_URL_PATTERN: &str = r"https://api\.github\.com/repos/([^/\s]+)/([^/\s]+)/pulls/(\d+)";

pub fn is_related_reason(reason: &str) -> bool {
    matches!(
        reason,
        REVIEW_REQUEST_REASON | MENTION_REASON | TEAM_MENTION_REASON | AUTHOR_REASON
    )
}

pub fn is_access_denied_error(error: &Error) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .contains("github access denied")
}

fn response_preview(body: &str) -> String {
    body.chars().take(240).collect::<String>()
}

fn access_denied_error(url: &str, status: u16) -> Error {
    anyhow!("GitHub access denied for {} (HTTP {})", url, status)
}

fn api_error(url: &str, status: u16, body: &str) -> Error {
    anyhow!(
        "GitHub API {} returned HTTP {}: {}",
        url,
        status,
        response_preview(body)
    )
}

fn decode_error(url: &str, body: &str) -> String {
    format!(
        "failed to decode GitHub response from {}: {}",
        url,
        response_preview(body)
    )
}

fn curl_error(url: &str, stderr: &[u8]) -> Error {
    anyhow!(
        "curl failed for GitHub API {}: {}",
        url,
        String::from_utf8_lossy(stderr).trim()
    )
}

fn call_api_error(url: &str) -> String {
    format!("failed to call GitHub API {url}")
}

fn temp_header_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "review-please-headers-{}-{}.txt",
        std::process::id(),
        utc_now_string().replace([':', '.'], "-")
    ))
}

fn header_name_value(line: &str) -> Option<(String, String)> {
    let (name, value) = line.split_once(':')?;
    Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
}

fn collect_response_headers(headers_raw: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    for line in headers_raw.lines() {
        if let Some((name, value)) = header_name_value(line) {
            headers.insert(name, value);
        }
    }
    headers
}

fn curl_status(stdout: &str) -> Result<(String, u16)> {
    let (body, status_raw) = stdout
        .rsplit_once("\n__CURL_STATUS__:")
        .ok_or_else(|| anyhow!("missing curl status marker"))?;
    let status = status_raw
        .trim()
        .parse::<u16>()
        .context("invalid curl HTTP status")?;
    Ok((body.to_string(), status))
}

fn notification_url_regex() -> Option<Regex> {
    Regex::new(PULL_URL_PATTERN).ok()
}

fn parse_pull_number(value: &str) -> Option<i64> {
    value.parse::<i64>().ok()
}

fn pull_from_captures(captures: regex::Captures<'_>) -> Option<GithubPullRef> {
    Some(GithubPullRef {
        owner: captures[1].to_string(),
        repo: captures[2].to_string(),
        number: parse_pull_number(&captures[3])?,
    })
}

fn is_after_since(event_at: &str, since: Option<&str>) -> bool {
    since.map(|value| event_at > value).unwrap_or(true)
}

fn actor_login_and_is_me(
    actor: Option<GithubActor>,
    current_user_login: &str,
) -> (Option<String>, bool) {
    let actor_login = actor.and_then(|user| user.login);
    let actor_is_me = actor_login
        .as_ref()
        .map(|login| login == current_user_login)
        .unwrap_or(false);
    (actor_login, actor_is_me)
}

fn build_github_event(
    pull: &GithubPullRef,
    thread_id: &str,
    reason: &str,
    id: String,
    event_kind: String,
    actor_login: Option<String>,
    actor_is_me: bool,
    event_at: String,
    payload_json: String,
) -> GithubEvent {
    GithubEvent {
        id,
        pr_key: pull.key(),
        pr_title: None,
        repo_owner: None,
        repo_name: None,
        pr_number: None,
        pr_author_login: None,
        notification_thread_id: thread_id.to_string(),
        notification_reason: reason.to_string(),
        event_kind,
        actor_login,
        actor_is_me,
        related_to_me: is_related_reason(reason),
        event_at,
        payload_json,
        created_at: utc_now_string(),
        read_at: None,
    }
}

fn read_at_for_thread(thread: &GithubNotificationThread) -> Option<String> {
    if thread.unread {
        None
    } else {
        thread
            .last_read_at
            .clone()
            .or_else(|| Some(utc_now_string()))
    }
}

fn apply_thread_context(
    event: &mut GithubEvent,
    thread: &GithubNotificationThread,
    pull: &GithubPullRef,
) {
    event.pr_title = thread.subject_title.clone();
    event.repo_owner = Some(pull.owner.clone());
    event.repo_name = Some(pull.repo.clone());
    event.pr_number = Some(pull.number);
    event.read_at = read_at_for_thread(thread);
}

fn fallback_thread_event(thread: &GithubNotificationThread, pull: &GithubPullRef) -> GithubEvent {
    GithubEvent {
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
        read_at: read_at_for_thread(thread),
    }
}

fn finalize_thread_events(
    mut events: Vec<GithubEvent>,
    thread: &GithubNotificationThread,
    pull: &GithubPullRef,
) -> Vec<GithubEvent> {
    events.sort_by(|left, right| left.event_at.cmp(&right.event_at));
    if events.is_empty() {
        events.push(fallback_thread_event(thread, pull));
    }
    for event in &mut events {
        apply_thread_context(event, thread, pull);
    }
    events
}

fn review_event_kind(state: Option<&str>) -> Option<EventKind> {
    match state {
        Some("APPROVED") => Some(EventKind::Approved),
        Some("CHANGES_REQUESTED") => Some(EventKind::ChangesRequested),
        Some("COMMENTED") => Some(EventKind::Commented),
        Some("PENDING") => None,
        _ => Some(EventKind::Unknown),
    }
}

fn push_json_events<T, F>(events: &mut Vec<GithubEvent>, items: Vec<T>, build: F)
where
    F: Fn(T) -> Option<GithubEvent>,
{
    events.extend(items.into_iter().filter_map(build));
}

fn collect_headers(sync_state: &SyncState) -> Vec<String> {
    let mut extra_headers = Vec::new();
    if let Some(last_modified) = &sync_state.github_last_modified {
        extra_headers.push(format!("If-Modified-Since: {last_modified}"));
    }
    if let Some(etag) = &sync_state.github_etag {
        extra_headers.push(format!("If-None-Match: {etag}"));
    }
    extra_headers
}

fn parse_poll_headers(
    headers: &HashMap<String, String>,
) -> (Option<u64>, Option<String>, Option<String>) {
    let poll_interval = headers
        .get("x-poll-interval")
        .and_then(|value| value.parse::<u64>().ok());
    let etag = headers.get("etag").cloned();
    let last_modified = headers.get("last-modified").cloned();
    (poll_interval, etag, last_modified)
}

fn notification_poll_result(
    threads: Vec<GithubNotificationThread>,
    not_modified: bool,
    poll_interval: Option<u64>,
    min_poll_interval_seconds: u64,
    etag: Option<String>,
    last_modified: Option<String>,
) -> NotificationsPollResult {
    NotificationsPollResult {
        threads,
        not_modified,
        poll_interval_seconds: poll_interval.or(Some(min_poll_interval_seconds)),
        etag,
        last_modified,
    }
}

fn notification_thread_from_response(
    thread: NotificationThreadResponse,
) -> GithubNotificationThread {
    GithubNotificationThread {
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
            .and_then(LocalGithubProvider::parse_pull_from_api_url),
        updated_at: thread.updated_at,
    }
}

fn decode_notification_threads(body: &str) -> Result<Vec<GithubNotificationThread>> {
    let threads: Vec<NotificationThreadResponse> =
        serde_json::from_str(body).context("failed to decode notifications")?;
    Ok(threads
        .into_iter()
        .map(notification_thread_from_response)
        .collect())
}

fn cached_login(cache: &Mutex<Option<String>>) -> Result<Option<String>> {
    cache
        .lock()
        .map_err(|_| anyhow!("failed to lock GitHub login cache"))
        .map(|value| value.clone())
}

fn store_cached_login(cache: &Mutex<Option<String>>, login: String) -> Result<String> {
    *cache
        .lock()
        .map_err(|_| anyhow!("failed to lock GitHub login cache"))? = Some(login.clone());
    Ok(login)
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
            return Err(access_denied_error(url, status));
        }
        if status >= 400 {
            return Err(api_error(url, status, &body));
        }
        serde_json::from_str(&body).with_context(|| decode_error(url, &body))
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
        let header_path = temp_header_path();
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
            .arg(format!("User-Agent: {USER_AGENT}"));
        for header in Self::base_headers(&token) {
            command.arg("-H").arg(header);
        }
        for header in extra_headers {
            command.arg("-H").arg(header);
        }
        let output = command.output().with_context(|| call_api_error(url))?;
        if !output.status.success() {
            let _ = fs::remove_file(&header_path);
            return Err(curl_error(url, &output.stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let (body, status) = curl_status(&stdout)?;
        let headers_raw = fs::read_to_string(&header_path).unwrap_or_default();
        let _ = fs::remove_file(&header_path);
        Ok((status, collect_response_headers(&headers_raw), body))
    }

    fn parse_pull_from_api_url(url: &str) -> Option<GithubPullRef> {
        let regex = notification_url_regex()?;
        let captures = regex.captures(url)?;
        pull_from_captures(captures)
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
        let mut events = Vec::new();
        push_json_events(&mut events, reviews, |review| {
            let event_at = review.submitted_at.clone()?;
            if !is_after_since(&event_at, since) {
                return None;
            }
            let event_kind = review_event_kind(review.state.as_deref())?;
            let (actor_login, actor_is_me) =
                actor_login_and_is_me(review.user.clone(), current_user_login);
            Some(build_github_event(
                pull,
                thread_id,
                reason,
                format!("review:{}", review.id),
                event_kind.as_str().to_string(),
                actor_login,
                actor_is_me,
                event_at,
                json!(review).to_string(),
            ))
        });
        Ok(events)
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
        let mut events = Vec::new();
        push_json_events(&mut events, comments, |comment| {
            let event_at = comment.created_at.clone()?;
            if !is_after_since(&event_at, since) {
                return None;
            }
            let (actor_login, actor_is_me) =
                actor_login_and_is_me(comment.user.clone(), current_user_login);
            Some(build_github_event(
                pull,
                thread_id,
                reason,
                format!("issue_comment:{}", comment.id),
                EventKind::Commented.as_str().to_string(),
                actor_login,
                actor_is_me,
                event_at,
                json!(comment).to_string(),
            ))
        });
        Ok(events)
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
        let mut events = Vec::new();
        push_json_events(&mut events, comments, |comment| {
            let event_at = comment.created_at.clone()?;
            if !is_after_since(&event_at, since) {
                return None;
            }
            let (actor_login, actor_is_me) =
                actor_login_and_is_me(comment.user.clone(), current_user_login);
            Some(build_github_event(
                pull,
                thread_id,
                reason,
                format!("review_comment:{}", comment.id),
                EventKind::ReviewCommented.as_str().to_string(),
                actor_login,
                actor_is_me,
                event_at,
                json!(comment).to_string(),
            ))
        });
        Ok(events)
    }
}

impl super::GithubProvider for LocalGithubProvider {
    fn current_user_login(&self) -> Result<String> {
        if let Some(login) = cached_login(&self.current_user_login)? {
            return Ok(login);
        }
        let response: CurrentUser = self.request_json("https://api.github.com/user")?;
        store_cached_login(&self.current_user_login, response.login)
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
            "approval_scan",
            &format!("tracked-approval:{}", pull.key()),
            None,
            current_user_login,
        )?;
        events
            .retain(|event| event.actor_is_me && event.event_kind == EventKind::Approved.as_str());
        events.sort_by(|left, right| left.event_at.cmp(&right.event_at));
        Ok(events.pop())
    }

    fn fetch_notifications(
        &self,
        sync_state: &SyncState,
        min_poll_interval_seconds: u64,
    ) -> Result<NotificationsPollResult> {
        let extra_headers = collect_headers(sync_state);
        let (status, headers, body) = self.curl_with_headers(NOTIFICATIONS_URL, &extra_headers)?;
        let (poll_interval, etag, last_modified) = parse_poll_headers(&headers);

        if status == 304 {
            return Ok(notification_poll_result(
                Vec::new(),
                true,
                poll_interval,
                min_poll_interval_seconds,
                etag,
                last_modified,
            ));
        }
        if let Some(error) = Self::build_rate_limit_error(NOTIFICATIONS_URL, &headers, &body) {
            return Err(error);
        }
        if !(200..300).contains(&status) {
            return Err(anyhow!("GitHub notifications returned HTTP {status}"));
        }
        let threads = decode_notification_threads(&body)?;
        Ok(notification_poll_result(
            threads,
            false,
            poll_interval,
            min_poll_interval_seconds,
            etag,
            last_modified,
        ))
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
        Ok(finalize_thread_events(events, thread, pull))
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
