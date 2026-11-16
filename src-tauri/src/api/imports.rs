use tauri::State;

use crate::error::AppResult;
use crate::services::imports::{self, ColumnMapping, ImportJobDto, ImportPreview, ImportResult};
use crate::state::AppState;

#[tauri::command]
pub fn import_preview(
    state: State<'_, AppState>,
    path: String,
    format: String,
    account_id: Option<String>,
) -> AppResult<ImportPreview> {
    let conn = state.conn();
    imports::preview(
        &conn,
        &state.tenant_id,
        &state.user_id,
        &path,
        &format,
        account_id.as_deref(),
    )
}

#[tauri::command]
pub fn import_commit(
    state: State<'_, AppState>,
    job_id: String,
    mapping: ColumnMapping,
    headers: Vec<String>,
    account_id: Option<String>,
) -> AppResult<ImportResult> {
    let conn = state.conn();
    imports::commit(
        &conn,
        &state.tenant_id,
        &state.user_id,
        &job_id,
        &mapping,
        &headers,
        account_id.as_deref(),
    )
}

#[tauri::command]
pub fn list_import_jobs(state: State<'_, AppState>) -> AppResult<Vec<ImportJobDto>> {
    let conn = state.conn();
    imports::list_jobs(&conn, &state.tenant_id, None)
}

#[tauri::command]
pub fn cancel_import(state: State<'_, AppState>, job_id: String) -> AppResult<()> {
    let conn = state.conn();
    imports::cancel(&conn, &state.tenant_id, &job_id)
}
