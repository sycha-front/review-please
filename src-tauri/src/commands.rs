use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    config::{self, AppConfig},
    keychain::{
        CredentialStore, SecurityCredentialStore, GITHUB_TOKEN_ACCOUNT, SLACK_ACCESS_TOKEN_ACCOUNT,
        SLACK_TOKEN_ACCOUNT,
    },
    models::{ReviewDump, ReviewStatus},
    services::{
        launch_agent,
        release::ReleaseStatus,
        slack_oauth::{create_session, open_authorize_url, poll_session, SlackOAuthSessionState},
    },
    tray::AppState,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SlackAuthMode {
    Oauth,
    Manual,
    Disconnected,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub slack_mention_keyword: String,
    pub slack_username: String,
    pub github_username: String,
    pub lookback_days: u64,
    pub slack_poll_interval_seconds: u64,
    pub github_min_poll_interval_seconds: u64,
    pub done_menu_limit: usize,
    pub notify_on_new_pending: bool,
    pub notify_on_done: bool,
    pub notify_on_errors: bool,
    pub hide_only_on_close: bool,
    pub launch_at_login: bool,
    pub slack_auth_mode: SlackAuthMode,
    pub slack_connected: bool,
    pub slack_connected_user: Option<String>,
    pub slack_connected_workspace: Option<String>,
    pub slack_token: String,
    pub github_token: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsPayload {
    pub slack_mention_keyword: String,
    pub slack_username: String,
    pub github_username: String,
    pub lookback_days: u64,
    pub slack_poll_interval_seconds: u64,
    pub github_min_poll_interval_seconds: u64,
    pub done_menu_limit: usize,
    pub notify_on_new_pending: bool,
    pub notify_on_done: bool,
    pub notify_on_errors: bool,
    pub hide_only_on_close: bool,
    pub launch_at_login: bool,
    pub slack_token: String,
    pub github_token: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReviewDeadlinePayload {
    pub review_request_id: String,
    pub deadline_date: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReviewStatusPayload {
    pub review_request_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAppUpdatePayload {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSlackOauthResponse {
    pub session_id: String,
    pub session_secret: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollSlackOauthPayload {
    pub session_id: String,
    pub session_secret: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PollSlackOauthResponse {
    pub status: String,
    pub error: Option<String>,
    pub settings: Option<SettingsPayload>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkUpdateEventsReadPayload {
    pub event_ids: Vec<String>,
}

fn normalize_optional(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn build_settings_payload(
    config: &AppConfig,
    credentials: &SecurityCredentialStore,
) -> Result<SettingsPayload, String> {
    let dotenv = config::read_dotenv_map().map_err(|error| error.to_string())?;
    let oauth_slack_token = credentials
        .get(SLACK_ACCESS_TOKEN_ACCOUNT)
        .map_err(|error| error.to_string())?;
    let slack_token = credentials
        .get(SLACK_TOKEN_ACCOUNT)
        .map_err(|error| error.to_string())?
        .or_else(|| dotenv.get("SLACK_TOKEN").cloned())
        .unwrap_or_default();
    let github_token = credentials
        .get(GITHUB_TOKEN_ACCOUNT)
        .map_err(|error| error.to_string())?
        .or_else(|| dotenv.get("GITHUB_TOKEN").cloned())
        .unwrap_or_default();

    let launch_at_login = launch_agent::is_enabled().map_err(|error| error.to_string())?;
    let slack_auth_mode = if oauth_slack_token.is_some() {
        SlackAuthMode::Oauth
    } else if !slack_token.trim().is_empty() {
        SlackAuthMode::Manual
    } else {
        SlackAuthMode::Disconnected
    };

    Ok(SettingsPayload {
        slack_mention_keyword: config.slack_mention_keyword.clone(),
        slack_username: config.slack_username.clone(),
        github_username: config.github_username.clone(),
        lookback_days: config.lookback_days,
        slack_poll_interval_seconds: config.slack_poll_interval_seconds,
        github_min_poll_interval_seconds: config.github_min_poll_interval_seconds,
        done_menu_limit: config.done_menu_limit,
        notify_on_new_pending: config.notify_on_new_pending,
        notify_on_done: config.notify_on_done,
        notify_on_errors: config.notify_on_errors,
        hide_only_on_close: config.hide_only_on_close,
        launch_at_login,
        slack_auth_mode,
        slack_connected: oauth_slack_token.is_some(),
        slack_connected_user: normalize_optional(&config.slack_display_name)
            .or_else(|| normalize_optional(&config.slack_username)),
        slack_connected_workspace: normalize_optional(&config.slack_team_name),
        slack_token,
        github_token,
    })
}

fn runtime_config(state: &State<'_, AppState>) -> Result<AppConfig, String> {
    state
        .runtime_config
        .read()
        .map_err(|_| "failed to read runtime config".to_string())
        .map(|config| config.clone())
}

fn replace_runtime_config(state: &State<'_, AppState>, config: &AppConfig) -> Result<(), String> {
    let mut runtime_config = state
        .runtime_config
        .write()
        .map_err(|_| "failed to update runtime config".to_string())?;
    *runtime_config = config.clone();
    Ok(())
}

#[tauri::command]
pub fn get_review_dump(state: State<'_, AppState>) -> Result<ReviewDump, String> {
    let status = state.coordinator.status_label();
    let last_error = state
        .coordinator
        .last_error()
        .or_else(|| state.store.last_error_message().ok().flatten());
    let done_menu_limit = state
        .runtime_config
        .read()
        .map_err(|_| "failed to read runtime config".to_string())?
        .clone();
    state
        .store
        .dump(
            done_menu_limit.done_menu_limit,
            &status,
            last_error,
            &done_menu_limit.github_username,
            &done_menu_limit.slack_user_id,
            &done_menu_limit.slack_username,
        )
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_release_status(app: AppHandle) -> ReleaseStatus {
    crate::services::release::fetch_release_status(&app).await
}

#[tauri::command]
pub async fn run_app_update(
    _payload: RunAppUpdatePayload,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let did_install = crate::services::app_update::install_latest_release(&app)
        .await
        .map_err(|error| error.to_string())?;
    if !did_install {
        return Ok(());
    }

    state.mark_quitting();
    app.restart();
}

#[tauri::command]
pub fn start_slack_oauth(state: State<'_, AppState>) -> Result<StartSlackOauthResponse, String> {
    let config = runtime_config(&state)?;
    let session = create_session(&config).map_err(|error| error.to_string())?;
    open_authorize_url(&session.authorize_url).map_err(|error| error.to_string())?;
    Ok(StartSlackOauthResponse {
        session_id: session.session_id,
        session_secret: session.session_secret,
        expires_at: session.expires_at,
    })
}

#[tauri::command]
pub fn poll_slack_oauth(
    payload: PollSlackOauthPayload,
    state: State<'_, AppState>,
) -> Result<PollSlackOauthResponse, String> {
    let config = runtime_config(&state)?;
    let credentials = SecurityCredentialStore;
    match poll_session(&config, &payload.session_id, &payload.session_secret)
        .map_err(|error| error.to_string())?
    {
        SlackOAuthSessionState::Pending { .. } => Ok(PollSlackOauthResponse {
            status: "pending".to_string(),
            error: None,
            settings: None,
        }),
        SlackOAuthSessionState::Expired => Ok(PollSlackOauthResponse {
            status: "expired".to_string(),
            error: Some("Slack 연결 시간이 만료되었어요. 다시 시도해주세요.".to_string()),
            settings: None,
        }),
        SlackOAuthSessionState::Failed { error } => Ok(PollSlackOauthResponse {
            status: "failed".to_string(),
            error: Some(error),
            settings: None,
        }),
        SlackOAuthSessionState::Completed(session) => {
            credentials
                .set(SLACK_ACCESS_TOKEN_ACCOUNT, &session.access_token)
                .map_err(|error| error.to_string())?;

            let mut next_config = config.clone();
            next_config.slack_user_id = session.slack_user_id;
            next_config.slack_team_id = session.team_id;
            next_config.slack_display_name = session.slack_display_name.clone();
            next_config.slack_team_name = session.team_name;
            if !session.slack_display_name.trim().is_empty() {
                next_config.slack_username = session.slack_display_name;
            }
            next_config.save().map_err(|error| error.to_string())?;
            replace_runtime_config(&state, &next_config)?;

            let _ = state.coordinator.refresh_tray();
            let _ = state.coordinator.sync_now();

            Ok(PollSlackOauthResponse {
                status: "completed".to_string(),
                error: None,
                settings: Some(build_settings_payload(&next_config, &credentials)?),
            })
        }
    }
}

#[tauri::command]
pub fn disconnect_slack_oauth(state: State<'_, AppState>) -> Result<SettingsPayload, String> {
    let mut config = runtime_config(&state)?;
    let credentials = SecurityCredentialStore;
    credentials
        .delete(SLACK_ACCESS_TOKEN_ACCOUNT)
        .map_err(|error| error.to_string())?;

    config.slack_user_id.clear();
    config.slack_team_id.clear();
    config.slack_display_name.clear();
    config.slack_team_name.clear();
    config.save().map_err(|error| error.to_string())?;
    replace_runtime_config(&state, &config)?;

    let _ = state.coordinator.refresh_tray();
    let _ = state.coordinator.sync_now();

    build_settings_payload(&config, &credentials)
}

#[tauri::command]
pub fn update_review_deadline(
    payload: UpdateReviewDeadlinePayload,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let deadline_date = payload.deadline_date.trim();
    if deadline_date.is_empty() {
        return Err("deadline_date is required".to_string());
    }

    state
        .store
        .update_review_request_deadline(&payload.review_request_id, deadline_date)
        .map_err(|error| error.to_string())?;
    state
        .coordinator
        .refresh_tray()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn update_review_status(
    payload: UpdateReviewStatusPayload,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let status = match payload.status.trim() {
        "pending" => ReviewStatus::Pending,
        "done" => ReviewStatus::Done,
        _ => return Err("status must be either pending or done".to_string()),
    };

    state
        .store
        .set_review_request_status_manual(&payload.review_request_id, status)
        .map_err(|error| error.to_string())?;
    state
        .coordinator
        .refresh_tray()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn mark_update_events_read(
    payload: MarkUpdateEventsReadPayload,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .store
        .mark_github_events_read(&payload.event_ids)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_settings() -> Result<SettingsPayload, String> {
    let config = AppConfig::load_effective().map_err(|error| error.to_string())?;
    let credentials = SecurityCredentialStore;
    build_settings_payload(&config, &credentials)
}

#[tauri::command]
pub fn save_settings(
    payload: SaveSettingsPayload,
    state: State<'_, AppState>,
) -> Result<SettingsPayload, String> {
    let existing = runtime_config(&state)?;
    let config = AppConfig {
        slack_mention_keyword: payload.slack_mention_keyword.trim().to_string(),
        slack_username: payload.slack_username.trim().to_string(),
        slack_user_id: existing.slack_user_id,
        slack_team_id: existing.slack_team_id,
        slack_display_name: existing.slack_display_name,
        slack_team_name: existing.slack_team_name,
        github_username: payload.github_username.trim().to_string(),
        lookback_days: payload.lookback_days,
        slack_poll_interval_seconds: payload.slack_poll_interval_seconds,
        github_min_poll_interval_seconds: payload.github_min_poll_interval_seconds,
        done_menu_limit: payload.done_menu_limit,
        notify_on_new_pending: payload.notify_on_new_pending,
        notify_on_done: payload.notify_on_done,
        notify_on_errors: payload.notify_on_errors,
        hide_only_on_close: payload.hide_only_on_close,
        launch_at_login: payload.launch_at_login,
    };
    config.save().map_err(|error| error.to_string())?;
    launch_agent::set_enabled(payload.launch_at_login).map_err(|error| error.to_string())?;

    let credentials = SecurityCredentialStore;
    if payload.slack_token.trim().is_empty() {
        credentials
            .delete(SLACK_TOKEN_ACCOUNT)
            .map_err(|error| error.to_string())?;
    } else {
        credentials
            .set(SLACK_TOKEN_ACCOUNT, payload.slack_token.trim())
            .map_err(|error| error.to_string())?;
    }
    if payload.github_token.trim().is_empty() {
        credentials
            .delete(GITHUB_TOKEN_ACCOUNT)
            .map_err(|error| error.to_string())?;
    } else {
        credentials
            .set(GITHUB_TOKEN_ACCOUNT, payload.github_token.trim())
            .map_err(|error| error.to_string())?;
    }

    replace_runtime_config(&state, &config)?;

    let _ = state.coordinator.refresh_tray();
    let _ = state.coordinator.sync_now();

    build_settings_payload(&config, &credentials)
}
