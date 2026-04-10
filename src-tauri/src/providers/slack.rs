use std::{process::Command, sync::Arc};

use anyhow::{anyhow, Context, Result};
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Weekday};
use regex::Regex;
use serde::Deserialize;

use crate::{
    keychain::{effective_slack_token, CredentialStore},
    models::{GithubPullRef, SlackMessageRef},
};

const SEARCH_HIGHLIGHT_START: char = '\u{E000}';
const SEARCH_HIGHLIGHT_END: char = '\u{E001}';

pub fn extract_pull_requests(text: &str) -> Vec<GithubPullRef> {
    let regex =
        Regex::new(r"https://github\.com/([^/\s>]+)/([^/\s>]+)/pull/(\d+)").expect("valid regex");
    let mut pulls: Vec<GithubPullRef> = Vec::new();
    for captures in regex.captures_iter(text) {
        let pull = GithubPullRef {
            owner: captures[1].to_string(),
            repo: captures[2].to_string(),
            number: captures[3].parse::<i64>().unwrap_or_default(),
        };
        if !pulls.iter().any(|existing| existing.key() == pull.key()) {
            pulls.push(pull);
        }
    }
    pulls
}

pub fn extract_deadline(text: &str, base_date: NaiveDate) -> Option<String> {
    let bracket_regex = Regex::new(r"\[([^\]]+)\]").expect("valid regex");
    for captures in bracket_regex.captures_iter(text) {
        let content = captures.get(1)?.as_str();
        if let Some(date) = extract_numeric_deadline(content, base_date.year()) {
            return Some(date.to_string());
        }
        if let Some(date) = extract_relative_deadline(content, base_date) {
            return Some(date.to_string());
        }
    }
    None
}

pub fn slack_ts_to_local_date(ts: &str) -> Option<NaiveDate> {
    let (seconds, nanos) = match ts.split_once('.') {
        Some((seconds, fraction)) => {
            let seconds = seconds.parse::<i64>().ok()?;
            let mut nanos = fraction.to_string();
            nanos.truncate(9);
            while nanos.len() < 9 {
                nanos.push('0');
            }
            let nanos = nanos.parse::<u32>().ok()?;
            (seconds, nanos)
        }
        None => (ts.parse::<i64>().ok()?, 0),
    };
    Local
        .timestamp_opt(seconds, nanos)
        .single()
        .map(|value| value.date_naive())
}

fn extract_numeric_deadline(content: &str, year: i32) -> Option<NaiveDate> {
    let slash_or_dot_regex = Regex::new(r"(\d{1,2})\s*[/.]\s*(\d{1,2})").expect("valid regex");
    if let Some(captures) = slash_or_dot_regex.captures(content) {
        let month = captures.get(1)?.as_str().parse::<u32>().ok()?;
        let day = captures.get(2)?.as_str().parse::<u32>().ok()?;
        return NaiveDate::from_ymd_opt(year, month, day);
    }

    let korean_regex = Regex::new(r"(\d{1,2})\s*월\s*(\d{1,2})\s*일").expect("valid regex");
    let captures = korean_regex.captures(content)?;
    let month = captures.get(1)?.as_str().parse::<u32>().ok()?;
    let day = captures.get(2)?.as_str().parse::<u32>().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn extract_relative_deadline(content: &str, base_date: NaiveDate) -> Option<NaiveDate> {
    if content.contains("금일") || content.contains("오늘") {
        return Some(base_date);
    }
    if content.contains("명일") || content.contains("내일") {
        return Some(base_date + Duration::days(1));
    }
    if content.contains("차주") {
        let weekday_regex = Regex::new(r"차주\s*([월화수목금토일])").expect("valid regex");
        let weekday = weekday_regex
            .captures(content)
            .and_then(|captures| captures.get(1))
            .and_then(|capture| match capture.as_str() {
                "월" => Some(Weekday::Mon),
                "화" => Some(Weekday::Tue),
                "수" => Some(Weekday::Wed),
                "목" => Some(Weekday::Thu),
                "금" => Some(Weekday::Fri),
                "토" => Some(Weekday::Sat),
                "일" => Some(Weekday::Sun),
                _ => None,
            })?;
        return Some(next_weekday(base_date, weekday));
    }
    None
}

fn next_weekday(base_date: NaiveDate, target: Weekday) -> NaiveDate {
    let current = weekday_index(base_date.weekday());
    let target = weekday_index(target);
    let offset = 7 - current + target;
    base_date + Duration::days(offset)
}

fn weekday_index(value: Weekday) -> i64 {
    match value {
        Weekday::Mon => 0,
        Weekday::Tue => 1,
        Weekday::Wed => 2,
        Weekday::Thu => 3,
        Weekday::Fri => 4,
        Weekday::Sat => 5,
        Weekday::Sun => 6,
    }
}

fn has_search_highlight_marker(text: &str) -> bool {
    text.contains(SEARCH_HIGHLIGHT_START) && text.contains(SEARCH_HIGHLIGHT_END)
}

fn strip_search_highlight_markers(text: &str) -> String {
    text.chars()
        .filter(|char| *char != SEARCH_HIGHLIGHT_START && *char != SEARCH_HIGHLIGHT_END)
        .collect()
}

pub struct LocalSlackProvider {
    credentials: Arc<dyn CredentialStore>,
}

impl LocalSlackProvider {
    pub fn new(credentials: Arc<dyn CredentialStore>) -> Self {
        Self { credentials }
    }

    fn token(&self) -> Result<String> {
        effective_slack_token(self.credentials.as_ref())?.ok_or_else(|| {
            anyhow!("missing Slack user token; connect Slack or run `review-please setup`")
        })
    }

    fn get<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: &[(&str, &str)],
    ) -> Result<T> {
        let token = self.token()?;
        let mut command = Command::new("curl");
        command
            .arg("-sS")
            .arg("-L")
            .arg("--get")
            .arg(endpoint)
            .arg("-H")
            .arg(format!("Authorization: Bearer {token}"));
        for (key, value) in params {
            command
                .arg("--data-urlencode")
                .arg(format!("{key}={value}"));
        }
        let output = command
            .output()
            .with_context(|| format!("failed to call Slack API {endpoint}"))?;
        if !output.status.success() {
            return Err(anyhow!(
                "curl failed for Slack API {}: {}",
                endpoint,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        serde_json::from_slice(&output.stdout).context("failed to decode Slack response")
    }
}

impl super::SlackProvider for LocalSlackProvider {
    fn search_messages(&self, keyword: &str) -> Result<Vec<SlackMessageRef>> {
        let response: SearchMessagesResponse = self.get(
            "https://slack.com/api/search.messages",
            &[
                ("query", keyword),
                ("highlight", "true"),
                ("sort", "timestamp"),
                ("sort_dir", "desc"),
                ("count", "100"),
            ],
        )?;
        if !response.ok {
            return Err(anyhow!(
                "Slack search.messages failed: {}",
                response
                    .error
                    .unwrap_or_else(|| "unknown error".to_string())
            ));
        }
        Ok(response
            .messages
            .unwrap_or_default()
            .matches
            .into_iter()
            .filter_map(|item| {
                let text = item.text?;
                if !has_search_highlight_marker(&text) {
                    return None;
                }
                Some(SlackMessageRef {
                    ts: item.ts?,
                    channel_id: item.channel.and_then(|channel| channel.id),
                    text: strip_search_highlight_markers(&text),
                    user_id: item.user?,
                })
            })
            .collect())
    }

    fn fetch_user_display_name(&self, user_id: &str) -> Result<Option<String>> {
        let response: UserInfoResponse =
            self.get("https://slack.com/api/users.info", &[("user", user_id)])?;
        if !response.ok {
            return Err(anyhow!(
                "Slack users.info failed: {}",
                response
                    .error
                    .unwrap_or_else(|| "unknown error".to_string())
            ));
        }
        Ok(response.user.and_then(|user| {
            user.profile.and_then(|profile| {
                profile
                    .display_name
                    .filter(|value| !value.is_empty())
                    .or(profile.real_name.filter(|value| !value.is_empty()))
            })
        }))
    }

    fn fetch_permalink(&self, channel_id: &str, message_ts: &str) -> Result<Option<String>> {
        let response: PermalinkResponse = self.get(
            "https://slack.com/api/chat.getPermalink",
            &[("channel", channel_id), ("message_ts", message_ts)],
        )?;
        if !response.ok {
            return Err(anyhow!(
                "Slack chat.getPermalink failed: {}",
                response
                    .error
                    .unwrap_or_else(|| "unknown error".to_string())
            ));
        }
        Ok(response.permalink)
    }
}

#[derive(Debug, Deserialize)]
struct SearchMessagesResponse {
    ok: bool,
    error: Option<String>,
    messages: Option<SearchMessageMatches>,
}

#[derive(Debug, Default, Deserialize)]
struct SearchMessageMatches {
    #[serde(default)]
    matches: Vec<SearchMessage>,
}

#[derive(Debug, Deserialize)]
struct SearchMessage {
    ts: Option<String>,
    text: Option<String>,
    user: Option<String>,
    channel: Option<SlackChannel>,
}

#[derive(Debug, Deserialize)]
struct SlackChannel {
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    ok: bool,
    error: Option<String>,
    user: Option<SlackUser>,
}

#[derive(Debug, Deserialize)]
struct SlackUser {
    profile: Option<SlackProfile>,
}

#[derive(Debug, Deserialize)]
struct SlackProfile {
    display_name: Option<String>,
    real_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PermalinkResponse {
    ok: bool,
    error: Option<String>,
    permalink: Option<String>,
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{
        extract_deadline, extract_pull_requests, has_search_highlight_marker,
        slack_ts_to_local_date, strip_search_highlight_markers,
    };

    #[test]
    fn extracts_single_pull_request() {
        let pulls = extract_pull_requests("review https://github.com/openai/app/pull/12");
        assert_eq!(pulls.len(), 1);
        assert_eq!(pulls[0].key(), "openai/app#12");
    }

    #[test]
    fn extracts_multiple_pull_requests_without_duplicates() {
        let text = "a https://github.com/openai/app/pull/12 and <https://github.com/openai/app/pull/13|link> and https://github.com/openai/app/pull/12";
        let pulls = extract_pull_requests(text);
        assert_eq!(pulls.len(), 2);
    }

    #[test]
    fn detects_search_highlight_markers() {
        assert!(has_search_highlight_marker(
            "hello \u{E000}@front_timespread\u{E001}"
        ));
        assert!(!has_search_highlight_marker("hello @front_timespread"));
    }

    #[test]
    fn strips_search_highlight_markers() {
        assert_eq!(
            strip_search_highlight_markers("hello \u{E000}@front_timespread\u{E001}"),
            "hello @front_timespread"
        );
    }

    #[test]
    fn extracts_deadline() {
        assert_eq!(
            extract_deadline(
                "[3/11] review",
                NaiveDate::from_ymd_opt(2026, 3, 1).expect("date")
            )
            .as_deref(),
            Some("2026-03-11")
        );
    }

    #[test]
    fn ignores_missing_deadline() {
        assert!(extract_deadline(
            "[soon] review",
            NaiveDate::from_ymd_opt(2026, 3, 1).expect("date")
        )
        .is_none());
    }

    #[test]
    fn accepts_two_digit_deadline() {
        assert_eq!(
            extract_deadline(
                "[12/01]",
                NaiveDate::from_ymd_opt(2026, 3, 1).expect("date")
            )
            .as_deref(),
            Some("2026-12-01")
        );
    }

    #[test]
    fn extracts_deadline_with_context_text() {
        assert_eq!(
            extract_deadline(
                "[팬플러스 CMS, 03/23 (월)] review",
                NaiveDate::from_ymd_opt(2026, 3, 20).expect("date")
            )
            .as_deref(),
            Some("2026-03-23")
        );
    }

    #[test]
    fn extracts_deadline_with_dot_separator() {
        assert_eq!(
            extract_deadline(
                "[리뷰 요청/2.24(화)] review",
                NaiveDate::from_ymd_opt(2026, 2, 20).expect("date")
            )
            .as_deref(),
            Some("2026-02-24")
        );
    }

    #[test]
    fn extracts_deadline_with_korean_month_day() {
        assert_eq!(
            extract_deadline(
                "[리뷰 요청/4월 3일] review",
                NaiveDate::from_ymd_opt(2026, 4, 1).expect("date")
            )
            .as_deref(),
            Some("2026-04-03")
        );
    }

    #[test]
    fn extracts_relative_deadline() {
        assert_eq!(
            extract_deadline(
                "[리뷰 요청/명일] review",
                NaiveDate::from_ymd_opt(2026, 3, 23).expect("date")
            )
            .as_deref(),
            Some("2026-03-24")
        );
        assert_eq!(
            extract_deadline(
                "[리뷰 요청/금일] review",
                NaiveDate::from_ymd_opt(2026, 3, 23).expect("date")
            )
            .as_deref(),
            Some("2026-03-23")
        );
    }

    #[test]
    fn extracts_next_weekday_deadline() {
        assert_eq!(
            extract_deadline(
                "[리뷰 요청/차주 월] review",
                NaiveDate::from_ymd_opt(2026, 3, 18).expect("date")
            )
            .as_deref(),
            Some("2026-03-23")
        );
    }

    #[test]
    fn parses_slack_timestamp_to_local_date() {
        assert_eq!(
            slack_ts_to_local_date("1773993541.736359")
                .map(|value| value.to_string())
                .as_deref(),
            Some("2026-03-20")
        );
    }
}
