use tauri::State;

use crate::error::AppResult;
use crate::services::fi::{self, FiInput, FiReport};
use crate::state::AppState;

#[tauri::command]
pub fn fi_calculate(state: State<'_, AppState>, input: FiInput) -> AppResult<FiReport> {
    let conn = state.conn();
    fi::calculate(&conn, &state.tenant_id, &input)
}
