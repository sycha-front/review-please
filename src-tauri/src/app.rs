use std::sync::{atomic::AtomicBool, Arc, RwLock};

use anyhow::Result;
use tauri::{ActivationPolicy, Manager};
use tauri_plugin_updater::Builder as UpdaterBuilder;

use crate::{
    commands,
    config::{self, AppConfig},
    db::{ReviewStore, SqliteStore},
    keychain::{CredentialStore, SecurityCredentialStore},
    providers::{github::LocalGithubProvider, slack::LocalSlackProvider},
    services::{
        notification::MacNotificationService,
        sync::{LocalSyncCoordinator, SyncCoordinator},
    },
    tray::{AppState, TrayController},
};

pub fn run_tray_app() -> Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(UpdaterBuilder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_review_dump,
            commands::get_release_status,
            commands::run_app_update,
            commands::update_review_deadline,
            commands::update_review_status,
            commands::mark_update_events_read,
            commands::get_settings,
            commands::save_settings
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(ActivationPolicy::Accessory);
                app.set_dock_visibility(false);
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }

            let config = AppConfig::load_effective()?;
            let runtime_config = Arc::new(RwLock::new(config));
            let store = build_store()?;
            let credentials = build_credentials();
            let tray = TrayController::create(app, config::data_dir()?)?;
            let coordinator: Arc<dyn SyncCoordinator> = Arc::new(LocalSyncCoordinator::new(
                runtime_config.clone(),
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
                runtime_config,
                is_quitting: AtomicBool::new(false),
            });
            coordinator.refresh_tray()?;
            coordinator.start()?;
            Ok(())
        })
        .build(tauri::generate_context!())?
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { .. } => {
                let state = app.state::<AppState>();
                state.mark_quitting();
            }
            tauri::RunEvent::WindowEvent { label, event, .. } => {
                if label == "main" {
                    match event {
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            let state = app.state::<AppState>();
                            if !state.is_quitting() {
                                api.prevent_close();
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.hide();
                                }
                            }
                        }
                        tauri::WindowEvent::Focused(false) => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
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
