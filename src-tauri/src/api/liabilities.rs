use tauri::State;

use crate::error::AppResult;
use crate::services::liabilities::{
    self, LiabilitiesSummary, LiabilityDto, LiabilityInput, LiabilityPaymentInput,
};
use crate::state::AppState;

#[tauri::command]
pub fn list_liabilities(state: State<'_, AppState>) -> AppResult<Vec<LiabilityDto>> {
    let conn = state.conn();
    liabilities::list_liabilities(&conn, &state.tenant_id)
}

#[tauri::command]
pub fn liabilities_summary(state: State<'_, AppState>) -> AppResult<LiabilitiesSummary> {
    let conn = state.conn();
    liabilities::liabilities_summary(&conn, &state.tenant_id)
}

#[tauri::command]
pub fn save_liability(
    state: State<'_, AppState>,
    id: Option<String>,
    input: LiabilityInput,
) -> AppResult<String> {
    let conn = state.conn();
    liabilities::save_liability(
        &conn,
        &state.tenant_id,
        &state.user_id,
        id.as_deref(),
        &input,
    )
}

#[tauri::command]
pub fn save_liability_payment(
    state: State<'_, AppState>,
    liability_id: String,
    id: Option<String>,
    input: LiabilityPaymentInput,
) -> AppResult<String> {
    let conn = state.conn();
    liabilities::save_payment(
        &conn,
        &state.tenant_id,
        &state.user_id,
        &liability_id,
        id.as_deref(),
        &input,
    )
}

#[tauri::command]
pub fn delete_liability_payment(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    liabilities::delete_payment(&conn, &state.tenant_id, &state.user_id, &id)
}

#[tauri::command]
pub fn set_liability_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> AppResult<()> {
    let conn = state.conn();
    liabilities::set_liability_status(&conn, &state.tenant_id, &state.user_id, &id, &status)
}

#[tauri::command]
pub fn delete_liability(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    liabilities::delete_liability(&conn, &state.tenant_id, &state.user_id, &id)
}
