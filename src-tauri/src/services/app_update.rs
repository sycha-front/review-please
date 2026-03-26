use anyhow::Result;
use tauri::{AppHandle, Runtime};

use crate::services::release;

pub async fn install_latest_release<R: Runtime>(app: &AppHandle<R>) -> Result<bool> {
    release::download_and_install(app).await
}
