use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

const APP_SUPPORT_DIR: &str = "Library/Application Support/pr-please";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub slack_mention_keyword: String,
    pub slack_poll_interval_seconds: u64,
    pub github_min_poll_interval_seconds: u64,
    pub done_menu_limit: usize,
    pub notify_on_new_pending: bool,
    pub notify_on_done: bool,
    pub notify_on_errors: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            slack_mention_keyword: String::new(),
            slack_poll_interval_seconds: 120,
            github_min_poll_interval_seconds: 60,
            done_menu_limit: 10,
            notify_on_new_pending: true,
            notify_on_done: true,
            notify_on_errors: true,
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config at {}", path.display()))?;
        toml::from_str(&contents).context("failed to parse config.toml")
    }

    pub fn load_or_default() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        Self::load()
    }

    pub fn save(&self) -> Result<PathBuf> {
        ensure_data_dir()?;
        let path = config_path()?;
        let body = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(path)
    }

    pub fn validate(&self) -> Result<()> {
        if self.slack_mention_keyword.trim().is_empty() {
            return Err(anyhow!("slack_mention_keyword is required"));
        }
        Ok(())
    }
}

pub fn ensure_data_dir() -> Result<PathBuf> {
    let data_dir = data_dir()?;
    fs::create_dir_all(&data_dir)
        .with_context(|| format!("failed to create {}", data_dir.display()))?;
    Ok(data_dir)
}

pub fn data_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("HOME directory is not available"))?;
    Ok(home.join(APP_SUPPORT_DIR))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("config.toml"))
}

pub fn database_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("state.sqlite3"))
}

pub fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}
