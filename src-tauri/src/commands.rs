use tauri::State;

use crate::{models::ReviewDump, tray::AppState};

#[tauri::command]
pub fn get_review_dump(state: State<'_, AppState>) -> Result<ReviewDump, String> {
    let status = state.coordinator.status_label();
    let last_error = state
        .coordinator
        .last_error()
        .or_else(|| state.store.last_error_message().ok().flatten());
    state
        .store
        .dump(state.done_menu_limit, &status, last_error)
        .map_err(|error| error.to_string())
}
