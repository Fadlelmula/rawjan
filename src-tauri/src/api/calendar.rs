use tauri::State;

use crate::error::AppResult;
use crate::services::calendar::{self, CalendarMonth};
use crate::state::AppState;

#[tauri::command]
pub fn month_calendar(
    state: State<'_, AppState>,
    month: Option<String>,
) -> AppResult<CalendarMonth> {
    let conn = state.conn();
    calendar::month_calendar(&conn, &state.tenant_id, month.as_deref())
}
