use serde::Deserialize;
use serde_json::{json, Value};
use tauri::State;

use crate::error::AppResult;
use crate::services::tx::{self, TxFilter, TxInput};
use crate::state::AppState;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn list_transactions(
    state: State<'_, AppState>,
    search: Option<String>,
    account_id: Option<String>,
    category_id: Option<String>,
    kind: Option<String>,
    status: Option<String>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<Value> {
    let conn = state.conn();
    let (rows, total) = tx::list_transactions(
        &conn,
        &state.tenant_id,
        &TxFilter {
            search: search.as_deref(),
            account_id: account_id.as_deref(),
            category_id: category_id.as_deref(),
            kind: kind.as_deref(),
            status: status.as_deref(),
            from: from.as_deref(),
            to: to.as_deref(),
            limit: limit.unwrap_or(100),
            offset: offset.unwrap_or(0),
        },
    )?;
    Ok(json!({ "rows": rows, "total": total }))
}

#[tauri::command]
pub fn get_transaction(state: State<'_, AppState>, id: String) -> AppResult<Value> {
    let conn = state.conn();
    tx::get_transaction(&conn, &state.tenant_id, &id)
}

#[tauri::command]
pub fn create_transaction(state: State<'_, AppState>, input: TxInput) -> AppResult<Value> {
    let conn = state.conn();
    let result = tx::create_transaction(&conn, &state.tenant_id, &state.user_id, &input)?;
    Ok(json!(result))
}

#[tauri::command]
pub fn update_transaction(
    state: State<'_, AppState>,
    id: String,
    input: TxInput,
) -> AppResult<()> {
    let conn = state.conn();
    tx::update_transaction(&conn, &state.tenant_id, &state.user_id, &id, &input)
}

#[tauri::command]
pub fn set_transaction_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> AppResult<()> {
    let conn = state.conn();
    tx::set_status(&conn, &state.tenant_id, &state.user_id, &id, &status)
}

#[tauri::command]
pub fn reverse_transaction(state: State<'_, AppState>, id: String) -> AppResult<String> {
    let conn = state.conn();
    tx::reverse_transaction(&conn, &state.tenant_id, &state.user_id, &id)
}

#[tauri::command]
pub fn delete_transaction(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let conn = state.conn();
    tx::delete_draft(&conn, &state.tenant_id, &state.user_id, &id)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCheckInput {
    pub account_id: String,
    pub date: String,
    pub amount: String,
    pub description: String,
    pub kind: String,
}

#[tauri::command]
pub fn check_duplicate(
    state: State<'_, AppState>,
    input: DuplicateCheckInput,
) -> AppResult<Value> {
    let conn = state.conn();
    let existing = tx::check_duplicate(
        &conn,
        &state.tenant_id,
        &input.account_id,
        &input.date,
        &input.amount,
        &input.description,
        &input.kind,
    )?;
    Ok(json!({ "possibleDuplicateId": existing }))
}
