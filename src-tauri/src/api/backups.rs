use tauri::State;

use crate::error::AppResult;
use crate::services::backups::{
    self, BackupDto, BackupValidation, ExportResult, ExportSummary,
};
use crate::state::AppState;

#[tauri::command]
pub fn create_backup(
    state: State<'_, AppState>,
    kind: Option<String>,
) -> AppResult<BackupDto> {
    let conn = state.conn();
    backups::create_backup(
        &conn,
        &state.data_dir,
        &state.tenant_id,
        &state.user_id,
        kind.as_deref().unwrap_or("manual"),
    )
}

#[tauri::command]
pub fn list_backups(state: State<'_, AppState>) -> AppResult<Vec<BackupDto>> {
    let conn = state.conn();
    backups::list_backups(&conn, &state.tenant_id)
}

#[tauri::command]
pub fn delete_backup(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    backups::delete_backup(
        &conn,
        &state.data_dir,
        &state.tenant_id,
        &state.user_id,
        &id,
    )
}

#[tauri::command]
pub fn validate_backup(path: String) -> AppResult<BackupValidation> {
    backups::validate_backup(&path)
}

#[tauri::command]
pub fn create_encrypted_export(
    state: State<'_, AppState>,
    passphrase: String,
) -> AppResult<ExportResult> {
    let conn = state.conn();
    backups::create_export(
        &conn,
        &state.data_dir,
        &state.tenant_id,
        &state.user_id,
        &passphrase,
    )
}

#[tauri::command]
pub fn verify_encrypted_export(path: String, passphrase: String) -> AppResult<ExportSummary> {
    backups::verify_export(&path, &passphrase)
}
