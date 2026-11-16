use tauri::State;

use crate::error::AppResult;
use crate::services::analytics::{self, AnalyticsReport};
use crate::state::AppState;

#[tauri::command]
pub fn analytics_report(
    state: State<'_, AppState>,
    from: Option<String>,
    to: Option<String>,
) -> AppResult<AnalyticsReport> {
    let conn = state.conn();
    analytics::analytics(&conn, &state.tenant_id, from.as_deref(), to.as_deref())
}
