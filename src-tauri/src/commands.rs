use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::{
    config::{self, AppConfig},
    keychain::{
        CredentialStore, SecurityCredentialStore, GITHUB_TOKEN_ACCOUNT, SLACK_TOKEN_ACCOUNT,
    },
    models::{ReviewDump, ReviewStatus},
    services::{launch_agent, release::ReleaseStatus},
    tray::AppState,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub slack_mention_keyword: String,
    pub slack_username: String,
    pub github_username: String,
    pub repo_path: String,
    pub lookback_days: u64,
    pub slack_poll_interval_seconds: u64,
    pub github_min_poll_interval_seconds: u64,
    pub done_menu_limit: usize,
    pub notify_on_new_pending: bool,
    pub notify_on_done: bool,
    pub notify_on_errors: bool,
    pub launch_at_login: bool,
    pub slack_token: String,
    pub github_token: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsPayload {
    pub slack_mention_keyword: String,
    pub slack_username: String,
    pub github_username: String,
    pub repo_path: String,
    pub lookback_days: u64,
    pub slack_poll_interval_seconds: u64,
    pub github_min_poll_interval_seconds: u64,
    pub done_menu_limit: usize,
    pub notify_on_new_pending: bool,
    pub notify_on_done: bool,
    pub notify_on_errors: bool,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkUpdateEventsReadPayload {
    pub event_ids: Vec<String>,
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
    let dotenv = config::read_dotenv_map().map_err(|error| error.to_string())?;
    let credentials = SecurityCredentialStore;
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

    Ok(SettingsPayload {
        slack_mention_keyword: config.slack_mention_keyword,
        slack_username: config.slack_username,
        github_username: config.github_username,
        repo_path: config.repo_path,
        lookback_days: config.lookback_days,
        slack_poll_interval_seconds: config.slack_poll_interval_seconds,
        github_min_poll_interval_seconds: config.github_min_poll_interval_seconds,
        done_menu_limit: config.done_menu_limit,
        notify_on_new_pending: config.notify_on_new_pending,
        notify_on_done: config.notify_on_done,
        notify_on_errors: config.notify_on_errors,
        launch_at_login,
        slack_token,
        github_token,
    })
}

#[tauri::command]
pub fn save_settings(
    payload: SaveSettingsPayload,
    state: State<'_, AppState>,
) -> Result<SettingsPayload, String> {
    let config = AppConfig {
        slack_mention_keyword: payload.slack_mention_keyword.trim().to_string(),
        slack_username: payload.slack_username.trim().to_string(),
        github_username: payload.github_username.trim().to_string(),
        repo_path: payload.repo_path.trim().to_string(),
        lookback_days: payload.lookback_days,
        slack_poll_interval_seconds: payload.slack_poll_interval_seconds,
        github_min_poll_interval_seconds: payload.github_min_poll_interval_seconds,
        done_menu_limit: payload.done_menu_limit,
        notify_on_new_pending: payload.notify_on_new_pending,
        notify_on_done: payload.notify_on_done,
        notify_on_errors: payload.notify_on_errors,
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

    {
        let mut runtime_config = state
            .runtime_config
            .write()
            .map_err(|_| "failed to update runtime config".to_string())?;
        *runtime_config = config.clone();
    }

    let _ = state.coordinator.refresh_tray();
    let _ = state.coordinator.sync_now();

    Ok(SettingsPayload {
        slack_mention_keyword: config.slack_mention_keyword,
        slack_username: config.slack_username,
        github_username: config.github_username,
        repo_path: config.repo_path,
        lookback_days: config.lookback_days,
        slack_poll_interval_seconds: config.slack_poll_interval_seconds,
        github_min_poll_interval_seconds: config.github_min_poll_interval_seconds,
        done_menu_limit: config.done_menu_limit,
        notify_on_new_pending: config.notify_on_new_pending,
        notify_on_done: config.notify_on_done,
        notify_on_errors: config.notify_on_errors,
        launch_at_login: payload.launch_at_login,
        slack_token: payload.slack_token.trim().to_string(),
        github_token: payload.github_token.trim().to_string(),
    })
}
