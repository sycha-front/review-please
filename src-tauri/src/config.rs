use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

const APP_SUPPORT_DIR: &str = "Library/Application Support/review-please";
const LEGACY_APP_SUPPORT_DIR: &str = "Library/Application Support/pr-please";
pub const DEFAULT_SLACK_AUTH_SERVICE_URL: &str =
    "https://review-please-slack-auth.pepprbell.workers.dev";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SlackKeywordMatchMode {
    Or,
    And,
}

impl Default for SlackKeywordMatchMode {
    fn default() -> Self {
        Self::Or
    }
}

impl SlackKeywordMatchMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Or => "or",
            Self::And => "and",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    // These values are the user-editable settings we persist locally.
    pub slack_mention_keyword: String,
    pub slack_keyword_match_mode: SlackKeywordMatchMode,
    pub slack_username: String,
    pub slack_user_id: String,
    pub slack_team_id: String,
    pub slack_display_name: String,
    pub slack_team_name: String,
    pub github_username: String,
    pub lookback_days: u64,
    pub slack_poll_interval_seconds: u64,
    pub github_min_poll_interval_seconds: u64,
    pub done_menu_limit: usize,
    pub github_review_requests_enabled: bool,
    pub github_related_updates_only: bool,
    pub notify_on_new_pending: bool,
    pub notify_on_new_updates: bool,
    pub notify_on_done: bool,
    pub notify_on_errors: bool,
    pub hide_only_on_close: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            slack_mention_keyword: String::new(),
            slack_keyword_match_mode: SlackKeywordMatchMode::Or,
            slack_username: String::new(),
            slack_user_id: String::new(),
            slack_team_id: String::new(),
            slack_display_name: String::new(),
            slack_team_name: String::new(),
            github_username: String::new(),
            lookback_days: 7,
            slack_poll_interval_seconds: 120,
            github_min_poll_interval_seconds: 60,
            done_menu_limit: 10,
            github_review_requests_enabled: true,
            github_related_updates_only: true,
            notify_on_new_pending: true,
            notify_on_new_updates: true,
            notify_on_done: false,
            notify_on_errors: false,
            hide_only_on_close: false,
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

    pub fn load_effective() -> Result<Self> {
        // Prefer the persisted desktop config once it exists; dotenv is only a bootstrap fallback.
        if config_path()?.exists() {
            return Self::load();
        }
        Ok(config_from_dotenv().unwrap_or_default())
    }

    pub fn save(&self) -> Result<PathBuf> {
        ensure_data_dir()?;
        let path = config_path()?;
        let body = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&path, body).with_context(|| format!("failed to write {}", path.display()))?;
        Ok(path)
    }

    pub fn validate(&self) -> Result<()> {
        if self.slack_mention_keywords().is_empty() {
            return Err(anyhow!("slack_mention_keyword is required"));
        }
        Ok(())
    }

    pub fn slack_mention_keywords(&self) -> Vec<String> {
        self.slack_mention_keyword
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect()
    }

    pub fn slack_search_queries(&self, after_clause: Option<&str>) -> Vec<String> {
        let keywords = self.slack_mention_keywords();
        if keywords.is_empty() {
            return Vec::new();
        }

        let raw_queries = match self.slack_keyword_match_mode {
            SlackKeywordMatchMode::Or => keywords,
            SlackKeywordMatchMode::And => vec![keywords.join(" ")],
        };

        raw_queries
            .into_iter()
            .map(|query| match after_clause {
                Some(after) => format!("{query} after:{after}"),
                None => query,
            })
            .collect()
    }

    pub fn slack_text_matches_keywords(&self, text: &str) -> bool {
        let keywords = self.slack_mention_keywords();
        if keywords.is_empty() {
            return false;
        }

        match self.slack_keyword_match_mode {
            SlackKeywordMatchMode::Or => keywords.iter().any(|keyword| text.contains(keyword)),
            SlackKeywordMatchMode::And => keywords.iter().all(|keyword| text.contains(keyword)),
        }
    }
}

pub fn ensure_data_dir() -> Result<PathBuf> {
    migrate_legacy_data_dir()?;
    let data_dir = data_dir()?;
    fs::create_dir_all(&data_dir)
        .with_context(|| format!("failed to create {}", data_dir.display()))?;
    Ok(data_dir)
}

pub fn read_dotenv_map() -> Result<HashMap<String, String>> {
    let dotenv_path = std::env::current_dir().ok().and_then(|dir| {
        [
            dir.join(".env"),
            dir.parent()
                .map(|parent| parent.join(".env"))
                .unwrap_or_default(),
        ]
        .into_iter()
        .find(|path| path.exists())
    });
    let Some(path) = dotenv_path else {
        return Ok(HashMap::new());
    };
    let contents =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut values = HashMap::new();
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let parsed = value
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        values.insert(key.trim().to_string(), parsed);
    }
    Ok(values)
}

pub fn config_from_dotenv() -> Result<AppConfig> {
    let values = read_dotenv_map()?;
    let mut config = AppConfig::default();
    if let Some(value) = values.get("SLACK_MENTION_KEYWORD") {
        config.slack_mention_keyword = value.clone();
    }
    if let Some(value) = values.get("SLACK_KEYWORD_MATCH_MODE") {
        config.slack_keyword_match_mode = match value.trim().to_lowercase().as_str() {
            "and" => SlackKeywordMatchMode::And,
            _ => SlackKeywordMatchMode::Or,
        };
    }
    if let Some(value) = values.get("SLACK_USERNAME") {
        config.slack_username = value.clone();
    }
    if let Some(value) = values.get("SLACK_USER_ID") {
        config.slack_user_id = value.clone();
    }
    if let Some(value) = values.get("SLACK_TEAM_ID") {
        config.slack_team_id = value.clone();
    }
    if let Some(value) = values.get("SLACK_DISPLAY_NAME") {
        config.slack_display_name = value.clone();
    }
    if let Some(value) = values.get("SLACK_TEAM_NAME") {
        config.slack_team_name = value.clone();
    }
    if let Some(value) = values.get("GITHUB_USERNAME") {
        config.github_username = value.clone();
    }
    if let Some(value) = values
        .get("LOOKBACK_DAYS")
        .and_then(|value| value.parse::<u64>().ok())
    {
        config.lookback_days = value;
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::{AppConfig, SlackKeywordMatchMode};

    #[test]
    fn builds_or_queries_per_keyword() {
        let config = AppConfig {
            slack_mention_keyword: "@one, @two".to_string(),
            slack_keyword_match_mode: SlackKeywordMatchMode::Or,
            ..AppConfig::default()
        };

        assert_eq!(
            config.slack_search_queries(Some("2026-04-01")),
            vec!["@one after:2026-04-01", "@two after:2026-04-01"]
        );
    }

    #[test]
    fn builds_and_query_as_single_search() {
        let config = AppConfig {
            slack_mention_keyword: "@one, @two".to_string(),
            slack_keyword_match_mode: SlackKeywordMatchMode::And,
            ..AppConfig::default()
        };

        assert_eq!(
            config.slack_search_queries(Some("2026-04-01")),
            vec!["@one @two after:2026-04-01"]
        );
    }

    #[test]
    fn matches_or_keywords_using_exact_substrings() {
        let config = AppConfig {
            slack_mention_keyword: "@front_timespread, @other".to_string(),
            slack_keyword_match_mode: SlackKeywordMatchMode::Or,
            ..AppConfig::default()
        };

        assert!(config.slack_text_matches_keywords("hello @front_timespread"));
        assert!(!config.slack_text_matches_keywords("hello timespread"));
    }

    #[test]
    fn matches_and_keywords_only_when_all_are_present() {
        let config = AppConfig {
            slack_mention_keyword: "@front_timespread, @review".to_string(),
            slack_keyword_match_mode: SlackKeywordMatchMode::And,
            ..AppConfig::default()
        };

        assert!(config.slack_text_matches_keywords("@front_timespread and @review"));
        assert!(!config.slack_text_matches_keywords("@front_timespread only"));
    }
}

pub fn data_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("HOME directory is not available"))?;
    Ok(home.join(APP_SUPPORT_DIR))
}

fn legacy_data_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("HOME directory is not available"))?;
    Ok(home.join(LEGACY_APP_SUPPORT_DIR))
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

fn migrate_legacy_data_dir() -> Result<()> {
    let current = data_dir()?;
    if current.exists() {
        return Ok(());
    }

    let legacy = legacy_data_dir()?;
    if !legacy.exists() {
        return Ok(());
    }

    ensure_parent(&current)?;
    fs::rename(&legacy, &current).with_context(|| {
        format!(
            "failed to migrate legacy app support directory from {} to {}",
            legacy.display(),
            current.display()
        )
    })?;
    Ok(())
}
