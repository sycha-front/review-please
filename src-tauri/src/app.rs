use std::sync::Arc;

use anyhow::Result;
use tauri::{ActivationPolicy, Manager};

use crate::{
    commands,
    config::{self, AppConfig},
    db::{ReviewStore, SqliteStore},
    keychain::{CredentialStore, SecurityCredentialStore},
    providers::{github::LocalGithubProvider, slack::LocalSlackProvider},
    services::{notification::MacNotificationService, sync::{LocalSyncCoordinator, SyncCoordinator}},
    tray::{AppState, TrayController},
};

pub fn run_tray_app() -> Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![commands::get_review_dump])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(ActivationPolicy::Accessory);
                app.set_dock_visibility(false);
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }

            let config = AppConfig::load_or_default()?;
            let store = build_store()?;
            let credentials = build_credentials();
            let tray = TrayController::create(app, config::data_dir()?)?;
            let coordinator: Arc<dyn SyncCoordinator> = Arc::new(LocalSyncCoordinator::new(
                config,
                store.clone(),
                Arc::new(LocalSlackProvider::new(credentials.clone())),
                Arc::new(LocalGithubProvider::new(credentials)),
                Arc::new(MacNotificationService),
                tray.clone(),
            ));
            app.manage(AppState {
                coordinator: coordinator.clone(),
                tray,
                store,
                done_menu_limit: AppConfig::load_or_default()?.done_menu_limit,
            });
            coordinator.refresh_tray()?;
            coordinator.start()?;
            Ok(())
        })
        .build(tauri::generate_context!())?
        .run(|app, event| {
            match event {
                tauri::RunEvent::ExitRequested { .. } => {
                    let state = app.state::<AppState>();
                    let _ = state.coordinator.stop();
                }
                tauri::RunEvent::WindowEvent { label, event, .. } => {
                    if label == "main" {
                        if let tauri::WindowEvent::Focused(false) = event {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                    }
                }
                _ => {}
            }
        });
    Ok(())
}

fn build_store() -> Result<Arc<dyn ReviewStore>> {
    let store = SqliteStore::from_default_location()?;
    store.init_schema()?;
    Ok(Arc::new(store))
}

fn build_credentials() -> Arc<dyn CredentialStore> {
    Arc::new(SecurityCredentialStore)
}
