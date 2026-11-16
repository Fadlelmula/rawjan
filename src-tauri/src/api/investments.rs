use tauri::State;

use crate::error::AppResult;
use crate::services::investments::{
    self, InstrumentDto, InstrumentInput, PortfolioSummary, TradeDto, TradeInput, ValuationDto,
    ValuationInput,
};
use crate::state::AppState;

#[tauri::command]
pub fn list_instruments(state: State<'_, AppState>) -> AppResult<Vec<InstrumentDto>> {
    let conn = state.conn();
    investments::list_instruments(&conn, &state.tenant_id)
}

#[tauri::command]
pub fn portfolio_summary(state: State<'_, AppState>) -> AppResult<PortfolioSummary> {
    let conn = state.conn();
    investments::portfolio_summary(&conn, &state.tenant_id)
}

#[tauri::command]
pub fn save_instrument(
    state: State<'_, AppState>,
    id: Option<String>,
    input: InstrumentInput,
) -> AppResult<String> {
    let conn = state.conn();
    investments::save_instrument(
        &conn,
        &state.tenant_id,
        &state.user_id,
        id.as_deref(),
        &input,
    )
}

#[tauri::command]
pub fn delete_instrument(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    investments::delete_instrument(&conn, &state.tenant_id, &state.user_id, &id)
}

#[tauri::command]
pub fn list_trades(
    state: State<'_, AppState>,
    instrument_id: Option<String>,
) -> AppResult<Vec<TradeDto>> {
    let conn = state.conn();
    investments::list_trades(&conn, &state.tenant_id, instrument_id.as_deref())
}

#[tauri::command]
pub fn save_trade(
    state: State<'_, AppState>,
    id: Option<String>,
    input: TradeInput,
) -> AppResult<String> {
    let conn = state.conn();
    investments::save_trade(
        &conn,
        &state.tenant_id,
        &state.user_id,
        id.as_deref(),
        &input,
    )
}

#[tauri::command]
pub fn delete_trade(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    investments::delete_trade(&conn, &state.tenant_id, &state.user_id, &id)
}

#[tauri::command]
pub fn list_valuations(
    state: State<'_, AppState>,
    instrument_id: Option<String>,
) -> AppResult<Vec<ValuationDto>> {
    let conn = state.conn();
    investments::list_valuations(&conn, &state.tenant_id, instrument_id.as_deref())
}

#[tauri::command]
pub fn save_valuation(
    state: State<'_, AppState>,
    id: Option<String>,
    input: ValuationInput,
) -> AppResult<String> {
    let conn = state.conn();
    investments::save_valuation(
        &conn,
        &state.tenant_id,
        &state.user_id,
        id.as_deref(),
        &input,
    )
}

#[tauri::command]
pub fn delete_valuation(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    investments::delete_valuation(&conn, &state.tenant_id, &state.user_id, &id)
}
