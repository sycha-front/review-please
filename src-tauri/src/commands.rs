use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    config::{self, AppConfig},
    keychain::{CredentialStore, GITHUB_TOKEN_ACCOUNT, SLACK_TOKEN_ACCOUNT, SecurityCredentialStore},
    models::ReviewDump,
    tray::AppState,
};

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
    pub slack_token: String,
    pub github_token: String,
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
        .done_menu_limit;
    state
        .store
        .dump(done_menu_limit, &status, last_error)
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

    Ok(SettingsPayload {
        slack_mention_keyword: config.slack_mention_keyword,
        slack_username: config.slack_username,
        github_username: config.github_username,
        lookback_days: config.lookback_days,
        slack_poll_interval_seconds: config.slack_poll_interval_seconds,
        github_min_poll_interval_seconds: config.github_min_poll_interval_seconds,
        done_menu_limit: config.done_menu_limit,
        notify_on_new_pending: config.notify_on_new_pending,
        notify_on_done: config.notify_on_done,
        notify_on_errors: config.notify_on_errors,
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
        lookback_days: payload.lookback_days,
        slack_poll_interval_seconds: payload.slack_poll_interval_seconds,
        github_min_poll_interval_seconds: payload.github_min_poll_interval_seconds,
        done_menu_limit: payload.done_menu_limit,
        notify_on_new_pending: payload.notify_on_new_pending,
        notify_on_done: payload.notify_on_done,
        notify_on_errors: payload.notify_on_errors,
    };
    config.save().map_err(|error| error.to_string())?;

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
        lookback_days: config.lookback_days,
        slack_poll_interval_seconds: config.slack_poll_interval_seconds,
        github_min_poll_interval_seconds: config.github_min_poll_interval_seconds,
        done_menu_limit: config.done_menu_limit,
        notify_on_new_pending: config.notify_on_new_pending,
        notify_on_done: config.notify_on_done,
        notify_on_errors: config.notify_on_errors,
        slack_token: payload.slack_token.trim().to_string(),
        github_token: payload.github_token.trim().to_string(),
    })
}
