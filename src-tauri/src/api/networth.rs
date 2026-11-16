use tauri::State;

use crate::error::AppResult;
use crate::services::networth::{self, NetWorthOverview, SnapshotDto};
use crate::state::AppState;

#[tauri::command]
pub fn net_worth_overview(state: State<'_, AppState>) -> AppResult<NetWorthOverview> {
    let conn = state.conn();
    networth::overview(&conn, &state.tenant_id)
}

#[tauri::command]
pub fn capture_net_worth_snapshot(state: State<'_, AppState>) -> AppResult<SnapshotDto> {
    let conn = state.conn();
    networth::capture_snapshot(&conn, &state.tenant_id, &state.user_id)
}

#[tauri::command]
pub fn delete_net_worth_snapshot(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    networth::delete_snapshot(&conn, &state.tenant_id, &state.user_id, &id)
}
