use tauri::State;

use crate::error::AppResult;
use crate::services::income::{self, IncomeReport};
use crate::state::AppState;

#[tauri::command]
pub fn income_report(
    state: State<'_, AppState>,
    from: Option<String>,
    to: Option<String>,
) -> AppResult<IncomeReport> {
    let conn = state.conn();
    income::income_report(&conn, &state.tenant_id, from.as_deref(), to.as_deref())
}
