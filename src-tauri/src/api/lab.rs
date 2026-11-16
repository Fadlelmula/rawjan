use tauri::State;

use crate::error::AppResult;
use crate::services::lab::{
    self, CompoundInput, CompoundReport, InflationInput, InflationReport, LoanInput, LoanReport,
};
use crate::state::AppState;

#[tauri::command]
pub fn lab_compound(
    state: State<'_, AppState>,
    input: CompoundInput,
) -> AppResult<CompoundReport> {
    let conn = state.conn();
    lab::compound(&conn, &state.tenant_id, &input)
}

#[tauri::command]
pub fn lab_loan(state: State<'_, AppState>, input: LoanInput) -> AppResult<LoanReport> {
    let conn = state.conn();
    lab::loan(&conn, &state.tenant_id, &input)
}

#[tauri::command]
pub fn lab_inflation(
    state: State<'_, AppState>,
    input: InflationInput,
) -> AppResult<InflationReport> {
    let conn = state.conn();
    lab::inflation(&conn, &state.tenant_id, &input)
}
