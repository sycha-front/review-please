use std::{
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
};

use anyhow::{Context, Result};
use tauri::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, PhysicalPosition, Runtime, Wry,
};

use crate::{
    config::AppConfig, db::ReviewStore, models::TrayState, services::sync::SyncCoordinator,
};

const MENU_PENDING_ID: &str = "pending";
const MENU_DONE_ID: &str = "done";
const MENU_UPDATE_ID: &str = "update";
const MENU_LAST_SYNC_ID: &str = "last_sync";
const MENU_STATUS_ID: &str = "status";
const MENU_SYNC_NOW_ID: &str = "sync_now";
const MENU_SETTINGS_ID: &str = "settings";
const MENU_SHOW_LAST_ERROR_ID: &str = "show_last_error";
const MENU_OPEN_DATA_DIR_ID: &str = "open_data_dir";
const MENU_QUIT_ID: &str = "quit";

pub struct TrayController {
    pending_item: MenuItem<Wry>,
    done_item: MenuItem<Wry>,
    update_item: MenuItem<Wry>,
    last_sync_item: MenuItem<Wry>,
    status_item: MenuItem<Wry>,
    show_last_error_item: MenuItem<Wry>,
    data_dir: PathBuf,
}

impl TrayController {
    pub fn create(app: &mut App<Wry>, data_dir: PathBuf) -> Result<Arc<Self>> {
        let app_handle = app.handle().clone();
        let pending_item =
            MenuItem::with_id(app, MENU_PENDING_ID, "Pending: 0", false, None::<&str>)?;
        let done_item = MenuItem::with_id(app, MENU_DONE_ID, "Done: 0", false, None::<&str>)?;
        let update_item = MenuItem::with_id(app, MENU_UPDATE_ID, "Update: 0", false, None::<&str>)?;
        let last_sync_item = MenuItem::with_id(
            app,
            MENU_LAST_SYNC_ID,
            "Last Sync: never",
            false,
            None::<&str>,
        )?;
        let status_item =
            MenuItem::with_id(app, MENU_STATUS_ID, "Status: OK", false, None::<&str>)?;
        let sync_now_item =
            MenuItem::with_id(app, MENU_SYNC_NOW_ID, "Sync Now", true, None::<&str>)?;
        let show_last_error_item = MenuItem::with_id(
            app,
            MENU_SHOW_LAST_ERROR_ID,
            "Show Last Error",
            false,
            None::<&str>,
        )?;
        let open_data_dir_item = MenuItem::with_id(
            app,
            MENU_OPEN_DATA_DIR_ID,
            "Open Data Directory",
            true,
            None::<&str>,
        )?;
        let quit_item = MenuItem::with_id(app, MENU_QUIT_ID, "Quit", true, None::<&str>)?;
        let separator = PredefinedMenuItem::separator(app)?;
        let menu = Menu::with_items(
            app,
            &[
                &pending_item,
                &done_item,
                &update_item,
                &last_sync_item,
                &status_item,
                &separator,
                &sync_now_item,
                &show_last_error_item,
                &open_data_dir_item,
                &quit_item,
            ],
        )?;

        let mut builder = TrayIconBuilder::with_id("main")
            .menu(&menu)
            .tooltip("review-please")
            .show_menu_on_left_click(false)
            .icon_as_template(true)
            .on_menu_event(move |app, event| {
                let _ = handle_menu_event(app, &event);
            });
        let tray_app_handle = app_handle.clone();
        builder = builder.on_tray_icon_event(move |_, event| {
            let _ = handle_tray_icon_event(&tray_app_handle, &event);
        });
        if let Some(icon) = app.default_window_icon().cloned() {
            builder = builder.icon(icon);
        }
        builder.build(app)?;

        Ok(Arc::new(Self {
            pending_item,
            done_item,
            update_item,
            last_sync_item,
            status_item,
            show_last_error_item,
            data_dir,
        }))
    }

    pub fn update(&self, tray_state: &TrayState) -> Result<()> {
        self.pending_item
            .set_text(format!("Pending: {}", tray_state.pending_count))?;
        self.done_item
            .set_text(format!("Done: {}", tray_state.done_count))?;
        self.update_item
            .set_text(format!("Update: {}", tray_state.update_count))?;
        self.last_sync_item.set_text(format!(
            "Last Sync: {}",
            tray_state
                .last_sync_at
                .clone()
                .unwrap_or_else(|| "never".to_string())
        ))?;
        self.status_item
            .set_text(format!("Status: {}", tray_state.status))?;
        self.show_last_error_item
            .set_enabled(tray_state.last_error.is_some())?;
        Ok(())
    }

    pub fn open_data_dir(&self) -> Result<()> {
        Command::new("open")
            .arg(&self.data_dir)
            .output()
            .with_context(|| format!("failed to open {}", self.data_dir.display()))?;
        Ok(())
    }
}

fn handle_tray_icon_event<R: Runtime>(app: &AppHandle<R>, event: &TrayIconEvent) -> Result<()> {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        position,
        ..
    } = event
    {
        toggle_main_window(app, position)?;
    }
    Ok(())
}

fn toggle_main_window<R: Runtime>(
    app: &AppHandle<R>,
    position: &PhysicalPosition<f64>,
) -> Result<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    if window.is_visible()? {
        window.hide()?;
        return Ok(());
    }

    let size = window.outer_size()?;
    let x = (position.x.round() as i32 - (size.width as i32 / 2)).max(0);
    let y = (position.y.round() as i32 + 12).max(0);
    window.set_position(PhysicalPosition::new(x, y))?;
    window.show()?;
    window.set_focus()?;
    Ok(())
}

fn show_main_window_near_cursor<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    if let Ok(position) = app.cursor_position() {
        let size = window.outer_size()?;
        let x = (position.x.round() as i32 - (size.width as i32 / 2)).max(0);
        let y = (position.y.round() as i32 + 12).max(0);
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    window.show()?;
    window.set_focus()?;
    Ok(())
}

pub struct AppState {
    pub coordinator: Arc<dyn SyncCoordinator>,
    pub tray: Arc<TrayController>,
    pub store: Arc<dyn ReviewStore>,
    pub runtime_config: Arc<RwLock<AppConfig>>,
    pub is_quitting: AtomicBool,
}

impl AppState {
    pub fn mark_quitting(&self) {
        self.is_quitting.store(true, Ordering::SeqCst);
    }

    pub fn is_quitting(&self) -> bool {
        self.is_quitting.load(Ordering::SeqCst)
    }
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: &MenuEvent) -> Result<()> {
    let id = event.id().0.as_str();
    let state = app.state::<AppState>();
    match id {
        MENU_SYNC_NOW_ID => {
            state.coordinator.sync_now()?;
        }
        MENU_SETTINGS_ID => {
            show_main_window_near_cursor(app)?;
        }
        MENU_SHOW_LAST_ERROR_ID => {
            if let Some(last_error) = state.coordinator.last_error() {
                let script = format!(
                    "display dialog \"{}\" with title \"review-please\" buttons {{\"OK\"}} default button \"OK\"",
                    last_error.replace('"', "\\\"")
                );
                let _ = Command::new("osascript").args(["-e", &script]).output();
            }
        }
        MENU_OPEN_DATA_DIR_ID => {
            state.tray.open_data_dir()?;
        }
        MENU_QUIT_ID => {
            state.mark_quitting();
            app.exit(0);
        }
        _ => {}
    }
    Ok(())
}
