use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::{AppConfig, DEFAULT_SLACK_AUTH_SERVICE_URL};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SlackOAuthSessionStart {
    pub session_id: String,
    pub session_secret: String,
    pub authorize_url: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlackOAuthSessionState {
    Pending { expires_at: Option<String> },
    Completed(SlackOAuthCompletedSession),
    Expired,
    Failed { error: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SlackOAuthCompletedSession {
    pub access_token: String,
    pub slack_user_id: String,
    pub slack_display_name: String,
    pub team_id: String,
    pub team_name: String,
    pub scope: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerPollResponse {
    status: String,
    expires_at: Option<String>,
    error: Option<String>,
    access_token: Option<String>,
    slack_user_id: Option<String>,
    slack_display_name: Option<String>,
    team_id: Option<String>,
    team_name: Option<String>,
    scope: Option<String>,
}

pub fn create_session(config: &AppConfig) -> Result<SlackOAuthSessionStart> {
    let response = run_json_request(
        "POST",
        &format!("{}/slack/auth/session", auth_service_base_url(config)?),
        &["-H", "Content-Type: application/json", "-d", "{}"],
    )?;
    serde_json::from_slice(&response).context("failed to decode Slack OAuth session response")
}

pub fn poll_session(
    config: &AppConfig,
    session_id: &str,
    session_secret: &str,
) -> Result<SlackOAuthSessionState> {
    let response = run_json_request(
        "GET",
        &format!(
            "{}/slack/auth/session/{}",
            auth_service_base_url(config)?,
            session_id.trim()
        ),
        &[
            "-H",
            &format!("x-session-secret: {}", session_secret.trim()),
        ],
    )?;
    parse_poll_response(&response)
}

pub fn open_authorize_url(authorize_url: &str) -> Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.arg(authorize_url);
        command
    } else if cfg!(target_os = "windows") {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", authorize_url]);
        command
    } else {
        let mut command = Command::new("xdg-open");
        command.arg(authorize_url);
        command
    };

    let output = command
        .output()
        .with_context(|| format!("failed to launch browser for {authorize_url}"))?;
    if output.status.success() {
        return Ok(());
    }

    Err(anyhow!(
        "failed to open browser: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

fn auth_service_base_url(config: &AppConfig) -> Result<String> {
    let _ = config;
    Ok(DEFAULT_SLACK_AUTH_SERVICE_URL
        .trim_end_matches('/')
        .to_string())
}

fn run_json_request(method: &str, url: &str, extra_args: &[&str]) -> Result<Vec<u8>> {
    let mut command = Command::new("curl");
    command.arg("-sS").arg("-L").arg("-X").arg(method).arg(url);
    for arg in extra_args {
        command.arg(arg);
    }

    let output = command
        .output()
        .with_context(|| format!("failed to call Slack auth service {url}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "curl failed for Slack auth service {}: {}",
            url,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

fn parse_poll_response(body: &[u8]) -> Result<SlackOAuthSessionState> {
    let response: WorkerPollResponse =
        serde_json::from_slice(body).context("failed to decode Slack OAuth poll response")?;

    match response.status.as_str() {
        "pending" => Ok(SlackOAuthSessionState::Pending {
            expires_at: response.expires_at,
        }),
        "completed" => Ok(SlackOAuthSessionState::Completed(
            SlackOAuthCompletedSession {
                access_token: response
                    .access_token
                    .ok_or_else(|| anyhow!("missing accessToken in completed session"))?,
                slack_user_id: response
                    .slack_user_id
                    .ok_or_else(|| anyhow!("missing slackUserId in completed session"))?,
                slack_display_name: response.slack_display_name.unwrap_or_default(),
                team_id: response
                    .team_id
                    .ok_or_else(|| anyhow!("missing teamId in completed session"))?,
                team_name: response.team_name.unwrap_or_default(),
                scope: response.scope.unwrap_or_default(),
            },
        )),
        "expired" => Ok(SlackOAuthSessionState::Expired),
        "failed" => Ok(SlackOAuthSessionState::Failed {
            error: response
                .error
                .unwrap_or_else(|| "Slack OAuth failed".to_string()),
        }),
        other => Err(anyhow!("unsupported Slack OAuth session status `{other}`")),
    }
}
