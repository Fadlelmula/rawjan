use tauri::State;

use crate::error::AppResult;
use crate::services::journal::{self, JournalEntryDto, JournalInput};
use crate::state::AppState;

#[tauri::command]
pub fn list_journal_entries(
    state: State<'_, AppState>,
    search: Option<String>,
    limit: Option<i64>,
) -> AppResult<Vec<JournalEntryDto>> {
    let conn = state.conn();
    journal::list_entries(&conn, &state.tenant_id, search.as_deref(), limit)
}

#[tauri::command]
pub fn save_journal_entry(
    state: State<'_, AppState>,
    id: Option<String>,
    input: JournalInput,
) -> AppResult<String> {
    let conn = state.conn();
    journal::save_entry(
        &conn,
        &state.tenant_id,
        &state.user_id,
        id.as_deref(),
        &input,
    )
}

#[tauri::command]
pub fn delete_journal_entry(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    journal::delete_entry(&conn, &state.tenant_id, &state.user_id, &id)
}
