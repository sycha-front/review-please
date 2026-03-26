use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::{Update, Updater, UpdaterExt};
use url::Url;

const DEFAULT_UPDATER_ENDPOINT: &str =
    "https://github.com/sycha-front/pr-review-please/releases/latest/download/latest.json";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub latest_release_url: Option<String>,
    pub published_at: Option<String>,
    pub is_update_available: bool,
    pub error: Option<String>,
}

pub async fn fetch_release_status<R: Runtime>(app: &AppHandle<R>) -> ReleaseStatus {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    if compiled_pubkey().is_none() {
        return ReleaseStatus {
            current_version,
            latest_version: None,
            latest_release_url: None,
            published_at: None,
            is_update_available: false,
            error: None,
        };
    }

    match build_updater(app) {
        Ok(updater) => match updater.check().await {
            Ok(Some(update)) => {
                let latest_release_url = extract_string_field(&update, "release_url");
                let published_at = extract_string_field(&update, "pub_date");
                ReleaseStatus {
                    current_version,
                    latest_version: Some(update.version),
                    latest_release_url,
                    published_at,
                    is_update_available: true,
                    error: None,
                }
            }
            Ok(None) => ReleaseStatus {
                current_version,
                latest_version: None,
                latest_release_url: None,
                published_at: None,
                is_update_available: false,
                error: None,
            },
            Err(error) => ReleaseStatus {
                current_version,
                latest_version: None,
                latest_release_url: None,
                published_at: None,
                is_update_available: false,
                error: Some(error.to_string()),
            },
        },
        Err(error) => ReleaseStatus {
            current_version,
            latest_version: None,
            latest_release_url: None,
            published_at: None,
            is_update_available: false,
            error: Some(error.to_string()),
        },
    }
}

pub fn build_updater<R: Runtime>(app: &AppHandle<R>) -> Result<Updater> {
    let endpoint_url = updater_endpoint();
    let endpoint = Url::parse(endpoint_url)
        .with_context(|| format!("failed to parse updater endpoint {endpoint_url}"))?;
    app.updater_builder()
        .pubkey(required_pubkey()?)
        .endpoints(vec![endpoint])?
        .build()
        .context("failed to build updater")
}

fn updater_endpoint() -> &'static str {
    option_env!("TAURI_UPDATER_ENDPOINT").unwrap_or(DEFAULT_UPDATER_ENDPOINT)
}

fn required_pubkey() -> Result<&'static str> {
    let Some(value) = compiled_pubkey() else {
        return Err(anyhow!(
            "TAURI_UPDATER_PUBLIC_KEY was not set when building the app"
        ));
    };
    Ok(value)
}

fn compiled_pubkey() -> Option<&'static str> {
    option_env!("TAURI_UPDATER_PUBLIC_KEY").and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn extract_string_field(update: &Update, key: &str) -> Option<String> {
    update
        .raw_json
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

pub async fn download_and_install<R: Runtime>(app: &AppHandle<R>) -> Result<bool> {
    let Some(update) = build_updater(app)?.check().await? else {
        return Ok(false);
    };

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .context("failed to download and install update")?;

    Ok(true)
}
