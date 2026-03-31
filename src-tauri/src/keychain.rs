use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::config::read_dotenv_map;

pub const GITHUB_TOKEN_ACCOUNT: &str = "github_token";
pub const SLACK_TOKEN_ACCOUNT: &str = "slack_user_token";
pub const SLACK_ACCESS_TOKEN_ACCOUNT: &str = "slack_access_token";
const SERVICE_NAME: &str = "com.review-please.app";
const LEGACY_SERVICE_NAME: &str = "com.pr-please.app";

pub trait CredentialStore: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>>;
    fn set(&self, account: &str, secret: &str) -> Result<()>;
    fn delete(&self, account: &str) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct SecurityCredentialStore;

impl SecurityCredentialStore {
    fn run_security(args: &[&str]) -> Result<String> {
        let output = Command::new("security")
            .args(args)
            .output()
            .with_context(|| format!("failed to run security {:?}", args))?;
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(anyhow!(stderr))
    }

    fn find_value(service_name: &str, account: &str) -> Result<Option<String>> {
        match Self::run_security(&[
            "find-generic-password",
            "-a",
            account,
            "-s",
            service_name,
            "-w",
        ]) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.to_string().contains("could not be found") => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn fallback_env(account: &str) -> Result<Option<String>> {
        let dotenv = read_dotenv_map()?;
        let key = match account {
            GITHUB_TOKEN_ACCOUNT => "GITHUB_TOKEN",
            SLACK_TOKEN_ACCOUNT => "SLACK_TOKEN",
            _ => return Ok(None),
        };
        Ok(dotenv
            .get(key)
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string))
    }
}

impl CredentialStore for SecurityCredentialStore {
    fn get(&self, account: &str) -> Result<Option<String>> {
        if let Some(value) = Self::find_value(SERVICE_NAME, account)? {
            return Ok(Some(value));
        }
        if let Some(value) = Self::find_value(LEGACY_SERVICE_NAME, account)? {
            return Ok(Some(value));
        }
        Self::fallback_env(account)
    }

    fn set(&self, account: &str, secret: &str) -> Result<()> {
        Self::run_security(&[
            "add-generic-password",
            "-a",
            account,
            "-s",
            SERVICE_NAME,
            "-w",
            secret,
            "-U",
        ])?;
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<()> {
        match Self::run_security(&["delete-generic-password", "-a", account, "-s", SERVICE_NAME]) {
            Ok(_) => Ok(()),
            Err(error) if error.to_string().contains("could not be found") => Ok(()),
            Err(error) => Err(error),
        }
    }
}

pub fn effective_slack_token(credentials: &dyn CredentialStore) -> Result<Option<String>> {
    if let Some(value) = credentials.get(SLACK_ACCESS_TOKEN_ACCOUNT)? {
        return Ok(Some(value));
    }
    credentials.get(SLACK_TOKEN_ACCOUNT)
}
