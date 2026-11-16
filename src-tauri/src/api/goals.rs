use tauri::State;

use crate::error::AppResult;
use crate::services::goals::{self, ContributionInput, GoalDto, GoalInput};
use crate::state::AppState;

#[tauri::command]
pub fn list_goals(state: State<'_, AppState>, reference: Option<String>) -> AppResult<Vec<GoalDto>> {
    let conn = state.conn();
    goals::list_goals(&conn, &state.tenant_id, reference.as_deref())
}

#[tauri::command]
pub fn save_goal(state: State<'_, AppState>, id: Option<String>, input: GoalInput) -> AppResult<String> {
    let conn = state.conn();
    goals::save_goal(
        &conn,
        &state.tenant_id,
        &state.user_id,
        id.as_deref(),
        &input,
    )
}

#[tauri::command]
pub fn add_goal_contribution(
    state: State<'_, AppState>,
    goal_id: String,
    input: ContributionInput,
) -> AppResult<String> {
    let conn = state.conn();
    goals::add_contribution(
        &conn,
        &state.tenant_id,
        &state.user_id,
        &goal_id,
        &input,
    )
}

#[tauri::command]
pub fn delete_goal_contribution(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let conn = state.conn();
    goals::delete_contribution(&conn, &state.tenant_id, &state.user_id, &id)
}

#[tauri::command]
pub fn set_goal_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> AppResult<()> {
    let conn = state.conn();
    goals::set_goal_status(&conn, &state.tenant_id, &state.user_id, &id, &status)
}

#[tauri::command]
pub fn delete_goal(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    goals::delete_goal(&conn, &state.tenant_id, &state.user_id, &id)
}
