use std::{path::Path, process::Command};

use anyhow::{anyhow, Context, Result};

pub fn spawn_update_process(repo_path: &str) -> Result<()> {
    let repo_path = repo_path.trim();
    if repo_path.is_empty() {
        return Err(anyhow!("repo_path is required"));
    }

    let repo_dir = Path::new(repo_path);
    if !repo_dir.exists() {
        return Err(anyhow!("repo path does not exist: {}", repo_dir.display()));
    }
    if !repo_dir.join(".git").exists() {
        return Err(anyhow!(
            "repo path is not a git repository: {}",
            repo_dir.display()
        ));
    }

    let update_script = repo_dir.join("scripts/update-app.sh");
    if !update_script.exists() {
        return Err(anyhow!(
            "could not find update script at {}",
            update_script.display()
        ));
    }

    Command::new("sh")
        .args([
            "-lc",
            "sleep 1; ./scripts/update-app.sh >/tmp/review-please-update.log 2>&1",
        ])
        .current_dir(repo_dir)
        .spawn()
        .with_context(|| format!("failed to start update from {}", repo_dir.display()))?;

    Ok(())
}
