use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{anyhow, Context, Result};

const LABEL: &str = "com.review-please.app";

pub fn is_enabled() -> Result<bool> {
    Ok(plist_path()?.exists())
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    if enabled {
        enable()
    } else {
        disable()
    }
}

fn enable() -> Result<()> {
    let app_path = resolve_app_bundle_path()?;
    let plist_path = plist_path()?;
    if let Some(parent) = plist_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
      <string>/usr/bin/open</string>
      <string>{app_path}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
  </dict>
</plist>
"#,
        label = LABEL,
        app_path = app_path.display(),
    );
    fs::write(&plist_path, plist)
        .with_context(|| format!("failed to write {}", plist_path.display()))?;

    let gui = format!("gui/{}", current_uid()?);
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("{gui}/{LABEL}")])
        .arg(&plist_path)
        .output();
    let output = Command::new("launchctl")
        .args(["bootstrap", &gui])
        .arg(&plist_path)
        .output()
        .context("failed to run launchctl bootstrap")?;
    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

fn disable() -> Result<()> {
    let plist_path = plist_path()?;
    let gui = format!("gui/{}", current_uid()?);
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("{gui}/{LABEL}")])
        .arg(&plist_path)
        .output();
    if plist_path.exists() {
        fs::remove_file(&plist_path)
            .with_context(|| format!("failed to remove {}", plist_path.display()))?;
    }
    Ok(())
}

fn plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("HOME directory is not available"))?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

fn current_uid() -> Result<String> {
    env::var("UID").or_else(|_| {
        let output = Command::new("id")
            .arg("-u")
            .output()
            .context("failed to read current uid")?;
        if !output.status.success() {
            return Err(anyhow!(
                "{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    })
}

fn resolve_app_bundle_path() -> Result<PathBuf> {
    let current_exe = env::current_exe().context("failed to read current executable path")?;
    for ancestor in current_exe.ancestors() {
        if is_app_bundle(ancestor) {
            return Ok(ancestor.to_path_buf());
        }
    }
    let fallback = dirs::home_dir()
        .ok_or_else(|| anyhow!("HOME directory is not available"))?
        .join("Applications/review-please.app");
    if fallback.exists() {
        return Ok(fallback);
    }
    Err(anyhow!(
        "could not resolve installed app bundle path; install review-please.app first"
    ))
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("app"))
        .unwrap_or(false)
}
