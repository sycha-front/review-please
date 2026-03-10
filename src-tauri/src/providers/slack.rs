use std::{process::Command, sync::Arc};

use anyhow::{anyhow, Context, Result};
use chrono::NaiveDate;
use regex::Regex;
use serde::Deserialize;

use crate::{
    keychain::{CredentialStore, SLACK_TOKEN_ACCOUNT},
    models::{GithubPullRef, SlackMessageRef},
};

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

pub fn extract_deadline(text: &str, year: i32) -> Option<String> {
    let regex = Regex::new(r"\[(\d{1,2})/(\d{1,2})\]").expect("valid regex");
    let captures = regex.captures(text)?;
    let month = captures.get(1)?.as_str().parse::<u32>().ok()?;
    let day = captures.get(2)?.as_str().parse::<u32>().ok()?;
    NaiveDate::from_ymd_opt(year, month, day).map(|date| date.to_string())
}

pub struct LocalSlackProvider {
    credentials: Arc<dyn CredentialStore>,
}

impl LocalSlackProvider {
    pub fn new(credentials: Arc<dyn CredentialStore>) -> Self {
        Self { credentials }
    }

    fn token(&self) -> Result<String> {
        self.credentials
            .get(SLACK_TOKEN_ACCOUNT)?
            .ok_or_else(|| anyhow!("missing Slack user token; run `pr-please setup`"))
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
            command.arg("--data-urlencode").arg(format!("{key}={value}"));
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
            &[("query", keyword), ("sort", "timestamp"), ("sort_dir", "desc"), ("count", "100")],
        )?;
        if !response.ok {
            return Err(anyhow!(
                "Slack search.messages failed: {}",
                response.error.unwrap_or_else(|| "unknown error".to_string())
            ));
        }
        Ok(response
            .messages
            .unwrap_or_default()
            .matches
            .into_iter()
            .filter_map(|item| {
                Some(SlackMessageRef {
                    ts: item.ts?,
                    channel_id: item.channel.and_then(|channel| channel.id),
                    text: item.text.unwrap_or_default(),
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
                response.error.unwrap_or_else(|| "unknown error".to_string())
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
                response.error.unwrap_or_else(|| "unknown error".to_string())
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
    use super::{extract_deadline, extract_pull_requests};

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
    fn extracts_deadline() {
        assert_eq!(extract_deadline("[3/11] review", 2026).as_deref(), Some("2026-03-11"));
    }

    #[test]
    fn ignores_missing_deadline() {
        assert!(extract_deadline("[soon] review", 2026).is_none());
    }

    #[test]
    fn accepts_two_digit_deadline() {
        assert_eq!(extract_deadline("[12/01]", 2026).as_deref(), Some("2026-12-01"));
    }
}
