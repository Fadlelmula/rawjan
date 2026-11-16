use tauri::State;

use crate::error::AppResult;
use crate::services::budgets::{self, BudgetDto, BudgetInput};
use crate::state::AppState;

#[tauri::command]
pub fn list_budgets(state: State<'_, AppState>, reference: Option<String>) -> AppResult<Vec<BudgetDto>> {
    let conn = state.conn();
    budgets::list_budgets(&conn, &state.tenant_id, reference.as_deref())
}

#[tauri::command]
pub fn save_budget(
    state: State<'_, AppState>,
    id: Option<String>,
    input: BudgetInput,
) -> AppResult<String> {
    let conn = state.conn();
    budgets::save_budget(
        &conn,
        &state.tenant_id,
        &state.user_id,
        id.as_deref(),
        &input,
    )
}

#[tauri::command]
pub fn set_budget_active(
    state: State<'_, AppState>,
    id: String,
    active: bool,
) -> AppResult<()> {
    let conn = state.conn();
    budgets::set_budget_active(&conn, &state.tenant_id, &state.user_id, &id, active)
}

#[tauri::command]
pub fn delete_budget(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    budgets::delete_budget(&conn, &state.tenant_id, &state.user_id, &id)
}
