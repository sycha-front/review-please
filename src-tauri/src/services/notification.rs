use std::process::Command;

use anyhow::{Context, Result};

pub trait NotificationService: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct MacNotificationService;

impl NotificationService for MacNotificationService {
    fn notify(&self, title: &str, body: &str) -> Result<()> {
        let escaped_title = title.replace('"', "\\\"");
        let escaped_body = body.replace('"', "\\\"");
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            escaped_body, escaped_title
        );
        Command::new("osascript")
            .args(["-e", &script])
            .output()
            .context("failed to show macOS notification")?;
        Ok(())
    }
}
