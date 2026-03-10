use std::process::Command;

use anyhow::{anyhow, Context, Result};

pub const GITHUB_TOKEN_ACCOUNT: &str = "github_token";
pub const SLACK_TOKEN_ACCOUNT: &str = "slack_user_token";
const SERVICE_NAME: &str = "com.pr-please.app";

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
}

impl CredentialStore for SecurityCredentialStore {
    fn get(&self, account: &str) -> Result<Option<String>> {
        match Self::run_security(&[
            "find-generic-password",
            "-a",
            account,
            "-s",
            SERVICE_NAME,
            "-w",
        ]) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.to_string().contains("could not be found") => Ok(None),
            Err(error) => Err(error),
        }
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
