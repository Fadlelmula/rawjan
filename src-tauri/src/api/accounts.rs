use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::State;

use crate::api::{list_categories, CategoryDto};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::services::accounts::{AccountDto, AccountInput};
use crate::state::AppState;

#[tauri::command]
pub fn list_accounts(
    state: State<'_, AppState>,
    include_archived: Option<bool>,
) -> AppResult<Vec<AccountDto>> {
    let conn = state.conn();
    crate::services::accounts::list_accounts(
        &conn,
        &state.tenant_id,
        include_archived.unwrap_or(false),
    )
}

#[tauri::command]
pub fn create_account(state: State<'_, AppState>, input: AccountInput) -> AppResult<String> {
    let conn = state.conn();
    crate::services::accounts::create_account(&conn, &state.tenant_id, &state.user_id, &input)
}

#[tauri::command]
pub fn update_account(
    state: State<'_, AppState>,
    id: String,
    input: AccountInput,
) -> AppResult<()> {
    let conn = state.conn();
    crate::services::accounts::update_account(&conn, &state.tenant_id, &state.user_id, &id, &input)
}

#[tauri::command]
pub fn archive_account(state: State<'_, AppState>, id: String, archive: bool) -> AppResult<()> {
    let conn = state.conn();
    crate::services::accounts::archive_account(
        &conn,
        &state.tenant_id,
        &state.user_id,
        &id,
        archive,
    )
}

#[tauri::command]
pub fn account_register(
    state: State<'_, AppState>,
    account_id: String,
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> AppResult<serde_json::Value> {
    let conn = state.conn();
    let (rows, total) = crate::services::tx::list_transactions(
        &conn,
        &state.tenant_id,
        &crate::services::tx::TxFilter {
            search: None,
            account_id: Some(&account_id),
            category_id: None,
            kind: None,
            status: Some("effective"),
            from: from.as_deref(),
            to: to.as_deref(),
            limit: limit.unwrap_or(100),
            offset: offset.unwrap_or(0),
        },
    )?;
    let account = crate::services::accounts::list_accounts(&conn, &state.tenant_id, true)?
        .into_iter()
        .find(|a| a.id == account_id)
        .ok_or_else(|| AppError::NotFound("Account not found.".into()))?;
    Ok(json!({"rows": rows, "total": total, "account": account}))
}

#[tauri::command]
pub fn list_categories_cmd(
    state: State<'_, AppState>,
    include_archived: Option<bool>,
) -> AppResult<Vec<CategoryDto>> {
    let conn = state.conn();
    list_categories(&conn, &state.tenant_id, include_archived.unwrap_or(false))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInput {
    pub name: String,
    pub kind: String,
    pub parent_id: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_order: Option<i64>,
}

#[tauri::command]
pub fn create_category(state: State<'_, AppState>, input: CategoryInput) -> AppResult<String> {
    let conn = state.conn();
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("Category name is required.".into()));
    }
    if !["income", "expense", "equity"].contains(&input.kind.as_str()) {
        return Err(AppError::Validation(
            "Category kind must be income, expense or equity.".into(),
        ));
    }
    if let Some(parent_id) = input.parent_id.as_deref().filter(|v| !v.is_empty()) {
        let parent: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT kind, parent_id FROM categories WHERE id = ?1 AND tenant_id = ?2",
                params![parent_id, state.tenant_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (parent_kind, grandparent) =
            parent.ok_or_else(|| AppError::NotFound("Parent category not found.".into()))?;
        if parent_kind != input.kind {
            return Err(AppError::Validation(
                "A subcategory must have the same kind as its parent.".into(),
            ));
        }
        if grandparent.is_some() {
            return Err(AppError::Validation(
                "Rawjan supports two category levels.".into(),
            ));
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO categories (id, tenant_id, user_id, parent_id, name, kind, icon, color, is_system, sort_order, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10)",
        params![
            id,
            state.tenant_id,
            state.user_id,
            input.parent_id.as_deref().filter(|v| !v.is_empty()),
            input.name.trim(),
            input.kind,
            input.icon,
            input.color,
            input.sort_order.unwrap_or(500),
            crate::dates::now_iso()
        ],
    )
    .map_err(|e| match e {
        rusqlite::Error::SqliteFailure(err, _) if err.code == rusqlite::ErrorCode::ConstraintViolation => {
            AppError::Conflict("A category with this name already exists at this level.".into())
        }
        other => other.into(),
    })?;
    db::audit(
        &conn,
        &state.tenant_id,
        &state.user_id,
        "category",
        &id,
        "create",
        None,
        Some(&json!({
            "name": input.name.trim(), "kind": input.kind, "parentId": input.parent_id
        })),
    )?;
    Ok(id)
}

#[tauri::command]
pub fn update_category(
    state: State<'_, AppState>,
    id: String,
    input: CategoryInput,
) -> AppResult<()> {
    let conn = state.conn();
    let (old_kind, usage): (String, i64) = conn
        .query_row(
            "SELECT kind,
                (SELECT count(*) FROM transactions t WHERE t.category_id = ?1) +
                (SELECT count(*) FROM transaction_splits s WHERE s.category_id = ?1) +
                (SELECT count(*) FROM budget_allocations b WHERE b.category_id = ?1)
             FROM categories WHERE id = ?1 AND tenant_id = ?2",
            params![id, state.tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Category not found.".into()))?;
    if usage > 0 && old_kind != input.kind {
        return Err(AppError::Conflict(
            "The kind of a category in use cannot be changed.".into(),
        ));
    }
    conn.execute(
        "UPDATE categories SET name = ?1, kind = ?2, parent_id = ?3, icon = ?4, color = ?5, sort_order = ?6
         WHERE id = ?7 AND tenant_id = ?8",
        params![
            input.name.trim(),
            input.kind,
            input.parent_id.as_deref().filter(|v| !v.is_empty()),
            input.icon,
            input.color,
            input.sort_order.unwrap_or(500),
            id,
            state.tenant_id
        ],
    )?;
    db::audit(
        &conn,
        &state.tenant_id,
        &state.user_id,
        "category",
        &id,
        "update",
        None,
        None,
    )?;
    Ok(())
}

#[tauri::command]
pub fn archive_category(state: State<'_, AppState>, id: String, archive: bool) -> AppResult<()> {
    let conn = state.conn();
    if !archive {
        let parent_archived: Option<i64> = conn
            .query_row(
                "SELECT c.is_archived FROM categories c WHERE c.id = (SELECT parent_id FROM categories WHERE id = ?1 AND tenant_id = ?2)",
                params![id, state.tenant_id],
                |r| r.get(0),
            )
            .optional()?;
        if parent_archived == Some(1) {
            return Err(AppError::Conflict(
                "Restore the parent category first.".into(),
            ));
        }
    }
    conn.execute(
        "UPDATE categories SET is_archived = ?1 WHERE id = ?2 AND tenant_id = ?3",
        params![if archive { 1 } else { 0 }, id, state.tenant_id],
    )?;
    db::audit(
        &conn,
        &state.tenant_id,
        &state.user_id,
        "category",
        &id,
        if archive { "archive" } else { "restore" },
        None,
        None,
    )?;
    Ok(())
}

#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> AppResult<Vec<serde_json::Value>> {
    let conn = state.conn();
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, t.color, (SELECT count(*) FROM transaction_tags tt WHERE tt.tag_id = t.id)
         FROM tags t WHERE t.tenant_id = ?1 ORDER BY t.name",
    )?;
    let rows = stmt.query_map([&state.tenant_id], |r| {
        Ok(json!({
            "id": r.get::<_, String>(0)?,
            "name": r.get::<_, String>(1)?,
            "color": r.get::<_, Option<String>>(2)?,
            "usageCount": r.get::<_, i64>(3)?,
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeSourceDto {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub currency: String,
    pub expected_amount_minor: i64,
    pub expected_amount: String,
    pub schedule: String,
    pub pay_day: Option<i64>,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub category_id: Option<String>,
    pub is_active: bool,
    pub notes: Option<String>,
    pub received_base_minor: i64,
    pub received_base: String,
    pub base_currency: String,
    pub last_received_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeSourceInput {
    pub name: String,
    pub source_type: String,
    pub currency_code: String,
    pub expected_amount: Option<String>,
    pub schedule: String,
    pub pay_day: Option<i64>,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub is_active: Option<bool>,
    pub notes: Option<String>,
}

fn income_sources(conn: &rusqlite::Connection, tenant_id: &str) -> AppResult<Vec<IncomeSourceDto>> {
    let base = crate::services::currency::primary_currency(conn, tenant_id)?;
    let base_exp = crate::services::currency::exponent(conn, &base)?;
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.type, s.currency_code, s.expected_amount_minor, s.schedule, s.pay_day,
                s.account_id, a.name, s.category_id, s.is_active, s.notes,
                (SELECT COALESCE(SUM(t.base_amount_minor),0) FROM transactions t
                  WHERE t.income_source_id = s.id AND t.status IN ('posted','reconciled') AND t.reversal_of_id IS NULL),
                (SELECT max(t.local_date) FROM transactions t
                  WHERE t.income_source_id = s.id AND t.status IN ('posted','reconciled'))
         FROM income_sources s
         LEFT JOIN accounts a ON a.id = s.account_id
         WHERE s.tenant_id = ?1 ORDER BY s.is_active DESC, s.name",
    )?;
    let rows = stmt.query_map([tenant_id], |row| {
        let amount: i64 = row.get(4)?;
        let currency: String = row.get(3)?;
        let exp = crate::services::currency::exponent(conn, &currency).unwrap_or(2);
        Ok(IncomeSourceDto {
            id: row.get(0)?,
            name: row.get(1)?,
            source_type: row.get(2)?,
            currency: currency.clone(),
            expected_amount_minor: amount,
            expected_amount: crate::money::format_amount(amount, exp),
            schedule: row.get(5)?,
            pay_day: row.get(6)?,
            account_id: row.get(7)?,
            account_name: row.get(8)?,
            category_id: row.get(9)?,
            is_active: row.get::<_, i64>(10)? == 1,
            notes: row.get(11)?,
            received_base_minor: row.get(12)?,
            received_base: crate::money::format_amount(row.get::<_, i64>(12)?, base_exp),
            base_currency: base.clone(),
            last_received_date: row.get(13)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[tauri::command]
pub fn list_income_sources(state: State<'_, AppState>) -> AppResult<Vec<IncomeSourceDto>> {
    let conn = state.conn();
    income_sources(&conn, &state.tenant_id)
}

#[tauri::command]
pub fn save_income_source(
    state: State<'_, AppState>,
    id: Option<String>,
    input: IncomeSourceInput,
) -> AppResult<String> {
    let conn = state.conn();
    if input.name.trim().is_empty() {
        return Err(AppError::Validation(
            "Income source name is required.".into(),
        ));
    }
    if ![
        "salary",
        "bonus",
        "business",
        "freelance",
        "dividend",
        "rental",
        "interest",
        "gift",
        "other",
    ]
    .contains(&input.source_type.as_str())
    {
        return Err(AppError::Validation("Unknown income source type.".into()));
    }
    if ![
        "monthly",
        "semimonthly",
        "biweekly",
        "weekly",
        "irregular",
        "one_time",
    ]
    .contains(&input.schedule.as_str())
    {
        return Err(AppError::Validation("Unknown schedule.".into()));
    }
    if let Some(day) = input.pay_day {
        if !(1..=31).contains(&day) {
            return Err(AppError::Validation(
                "Pay day must be between 1 and 31.".into(),
            ));
        }
    }
    let exp = crate::services::currency::exponent(&conn, &input.currency_code)?;
    let amount_minor = match input
        .expected_amount
        .as_deref()
        .filter(|v| !v.trim().is_empty())
    {
        Some(value) => crate::money::parse_amount(value, exp)?,
        None => 0,
    };
    if amount_minor < 0 {
        return Err(AppError::Validation(
            "Expected amount cannot be negative.".into(),
        ));
    }
    if let Some(account_id) = input.account_id.as_deref().filter(|v| !v.is_empty()) {
        crate::services::tx::load_account(&conn, &state.tenant_id, account_id)?;
    }
    let now = crate::dates::now_iso();
    let source_id = match id.filter(|v| !v.is_empty()) {
        Some(existing) => {
            conn.execute(
                "UPDATE income_sources SET name = ?1, type = ?2, currency_code = ?3, expected_amount_minor = ?4,
                    schedule = ?5, pay_day = ?6, account_id = ?7, category_id = ?8, is_active = ?9, notes = ?10, updated_at = ?11
                 WHERE id = ?12 AND tenant_id = ?13",
                params![
                    input.name.trim(),
                    input.source_type,
                    input.currency_code,
                    amount_minor,
                    input.schedule,
                    input.pay_day,
                    input.account_id.as_deref().filter(|v| !v.is_empty()),
                    input.category_id.as_deref().filter(|v| !v.is_empty()),
                    if input.is_active.unwrap_or(true) { 1 } else { 0 },
                    input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    now,
                    existing,
                    state.tenant_id
                ],
            )?;
            existing
        }
        None => {
            let source_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO income_sources (id, tenant_id, user_id, name, type, currency_code, expected_amount_minor,
                    schedule, pay_day, account_id, category_id, is_active, notes, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
                params![
                    source_id,
                    state.tenant_id,
                    state.user_id,
                    input.name.trim(),
                    input.source_type,
                    input.currency_code,
                    amount_minor,
                    input.schedule,
                    input.pay_day,
                    input.account_id.as_deref().filter(|v| !v.is_empty()),
                    input.category_id.as_deref().filter(|v| !v.is_empty()),
                    if input.is_active.unwrap_or(true) { 1 } else { 0 },
                    input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    now
                ],
            )?;
            source_id
        }
    };
    db::audit(
        &conn,
        &state.tenant_id,
        &state.user_id,
        "income_source",
        &source_id,
        "save",
        None,
        Some(&json!({
            "name": input.name.trim(), "type": input.source_type, "expectedAmountMinor": amount_minor
        })),
    )?;
    Ok(source_id)
}

#[tauri::command]
pub fn list_brokers(state: State<'_, AppState>) -> AppResult<Vec<serde_json::Value>> {
    let conn = state.conn();
    let mut stmt = conn.prepare(
        "SELECT b.id, b.name, b.kind, b.website, b.notes,
                (SELECT count(*) FROM accounts a WHERE a.broker_id = b.id)
         FROM brokers b WHERE b.tenant_id = ?1 ORDER BY b.name",
    )?;
    let rows = stmt.query_map([&state.tenant_id], |r| {
        Ok(json!({
            "id": r.get::<_, String>(0)?,
            "name": r.get::<_, String>(1)?,
            "kind": r.get::<_, String>(2)?,
            "website": r.get::<_, Option<String>>(3)?,
            "notes": r.get::<_, Option<String>>(4)?,
            "accountCount": r.get::<_, i64>(5)?,
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[tauri::command]
pub fn save_broker(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    kind: Option<String>,
    website: Option<String>,
    notes: Option<String>,
) -> AppResult<String> {
    let conn = state.conn();
    if name.trim().is_empty() {
        return Err(AppError::Validation("Broker name is required.".into()));
    }
    let now = crate::dates::now_iso();
    let broker_id = match id.filter(|v| !v.is_empty()) {
        Some(existing) => {
            conn.execute(
                "UPDATE brokers SET name = ?1, kind = ?2, website = ?3, notes = ?4 WHERE id = ?5 AND tenant_id = ?6",
                params![name.trim(), kind.unwrap_or_else(|| "broker".into()), website, notes, existing, state.tenant_id],
            )?;
            existing
        }
        None => {
            let broker_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO brokers (id, tenant_id, name, kind, website, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    broker_id,
                    state.tenant_id,
                    name.trim(),
                    kind.unwrap_or_else(|| "broker".into()),
                    website,
                    notes,
                    now
                ],
            )?;
            broker_id
        }
    };
    Ok(broker_id)
}
