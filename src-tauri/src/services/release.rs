use std::process::Command;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde::Deserialize;

const RELEASE_API_URL: &str =
    "https://api.github.com/repos/sycha-front/pr-review-please/releases/latest";

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

#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    html_url: String,
    published_at: Option<String>,
}

pub fn fetch_release_status() -> ReleaseStatus {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    match fetch_latest_release() {
        Ok(release) => {
            let latest_version = normalize_version(&release.tag_name);
            ReleaseStatus {
                is_update_available: compare_versions(&current_version, &latest_version)
                    .map(|ordering| ordering.is_lt())
                    .unwrap_or(false),
                current_version,
                latest_version: Some(latest_version),
                latest_release_url: Some(release.html_url),
                published_at: release.published_at,
                error: None,
            }
        }
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

fn fetch_latest_release() -> Result<ReleaseResponse> {
    let output = Command::new("curl")
        .args([
            "-sS",
            "-L",
            RELEASE_API_URL,
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: review-please/0.1.0",
        ])
        .output()
        .with_context(|| format!("failed to call GitHub releases API {RELEASE_API_URL}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "curl failed for GitHub releases API: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    serde_json::from_slice(&output.stdout).context("failed to decode latest GitHub release")
}

fn normalize_version(value: &str) -> String {
    value.trim().trim_start_matches('v').to_string()
}

fn compare_versions(current: &str, latest: &str) -> Option<std::cmp::Ordering> {
    let current_parts = parse_version_parts(current)?;
    let latest_parts = parse_version_parts(latest)?;
    Some(current_parts.cmp(&latest_parts))
}

fn parse_version_parts(value: &str) -> Option<Vec<u64>> {
    value
        .split('.')
        .map(|part| part.parse::<u64>().ok())
        .collect()
}
