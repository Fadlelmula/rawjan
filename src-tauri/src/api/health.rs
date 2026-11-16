use tauri::State;

use crate::error::AppResult;
use crate::services::health::{self, HealthReport};
use crate::state::AppState;

#[tauri::command]
pub fn financial_health(state: State<'_, AppState>) -> AppResult<HealthReport> {
    let conn = state.conn();
    health::health_report(&conn, &state.tenant_id)
}
