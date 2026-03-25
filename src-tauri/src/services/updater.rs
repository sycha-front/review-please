use std::{
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use anyhow::{anyhow, Context, Result};
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_updater::UpdaterExt;

const DEFAULT_UPDATE_ENDPOINT: &str =
    "https://github.com/sycha-front/pr-review-please/releases/latest/download/latest.json";

#[derive(Debug, Default)]
pub struct UpdateController {
    in_progress: AtomicBool,
}

impl UpdateController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn check_for_updates<R: Runtime + 'static>(self: &Arc<Self>, app: AppHandle<R>, interactive: bool) {
        if self
            .in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            if interactive {
                let _ = show_info_dialog(
                    "pr-please",
                    "이미 업데이트를 확인하고 있어요. 잠시 후 다시 시도해주세요.",
                );
            }
            return;
        }

        let controller = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let result = run_update_check(app, interactive).await;
            controller.in_progress.store(false, Ordering::SeqCst);

            if let Err(error) = result {
                if interactive {
                    let _ = show_info_dialog("Update failed", &error.to_string());
                }
            }
        });
    }
}

async fn run_update_check<R: Runtime>(app: AppHandle<R>, interactive: bool) -> Result<()> {
    let pubkey = updater_pubkey().ok_or_else(|| {
        anyhow!(
            "Updater public key is not configured. Build with TAURI_UPDATER_PUBKEY set."
        )
    })?;

    let update = app
        .updater_builder()
        .pubkey(pubkey)
        .endpoints(vec![update_endpoint().parse()?])?
        .build()?
        .check()
        .await?;

    let Some(update) = update else {
        if interactive {
            show_info_dialog("pr-please", "현재 최신 버전을 사용 중이에요.")?;
        }
        return Ok(());
    };

    let current_version = update.current_version.clone();
    let next_version = update.version.clone();
    let notes = update
        .body
        .as_deref()
        .map(str::trim)
        .filter(|body| !body.is_empty())
        .unwrap_or("릴리스 노트가 없습니다.");
    let prompt = format!(
        "새 버전 {next_version} 이 있어요.\\n현재 버전: {current_version}\\n\\n{notes}\\n\\n지금 다운로드하고 설치할까요?"
    );

    if !ask_install_dialog("Update available", &prompt)? {
        return Ok(());
    }

    update
        .download_and_install(
            |_chunk_length, _content_length| {},
            || {},
        )
        .await?;

    let _ = app.emit("pr-please://update-installed", ());
    show_info_dialog(
        "Update installed",
        "업데이트 설치가 끝났어요. 앱을 다시 시작합니다.",
    )?;
    app.restart();
    #[allow(unreachable_code)]
    Ok(())
}

fn updater_pubkey() -> Option<&'static str> {
    option_env!("TAURI_UPDATER_PUBKEY").and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn update_endpoint() -> &'static str {
    option_env!("PR_PLEASE_UPDATER_ENDPOINT")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_UPDATE_ENDPOINT)
}

fn ask_install_dialog(title: &str, body: &str) -> Result<bool> {
    let script = format!(
        "button returned of (display dialog \"{}\" with title \"{}\" buttons {{\"Later\", \"Install\"}} default button \"Install\")",
        escape_applescript(body),
        escape_applescript(title),
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .context("failed to show install prompt")?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.contains("Install"))
}

fn show_info_dialog(title: &str, body: &str) -> Result<()> {
    let script = format!(
        "display dialog \"{}\" with title \"{}\" buttons {{\"OK\"}} default button \"OK\"",
        escape_applescript(body),
        escape_applescript(title),
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .context("failed to show dialog")?;

    if !output.status.success() {
        return Err(anyhow!(
            "{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

fn escape_applescript(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
