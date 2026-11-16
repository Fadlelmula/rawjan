use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::money::parse_rate;
use crate::services::accounts as account_service;
use crate::services::accounts::NetWorth;
use crate::services::currency::{add_manual_rate, primary_currency};
use crate::state::AppState;

mod accounts;
mod analytics;
mod backups;
mod budgets;
mod calendar;
mod fi;
mod goals;
mod health;
mod imports;
mod income;
mod investments;
mod journal;
mod lab;
mod liabilities;
mod networth;
mod transactions;

pub use accounts::*;
pub use analytics::*;
pub use backups::*;
pub use budgets::*;
pub use calendar::*;
pub use fi::*;
pub use goals::*;
pub use health::*;
pub use imports::*;
pub use income::*;
pub use investments::*;
pub use journal::*;
pub use lab::*;
pub use liabilities::*;
pub use networth::*;
pub use transactions::*;

// ---------------------------------------------------------------------------
// Categories (shared by the account API)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDto {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub is_system: bool,
    pub is_archived: bool,
    pub sort_order: i64,
    pub usage_count: i64,
}

pub fn list_categories(
    conn: &rusqlite::Connection,
    tenant_id: &str,
    include_archived: bool,
) -> AppResult<Vec<CategoryDto>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.parent_id, c.name, c.kind, c.icon, c.color, c.is_system, c.is_archived, c.sort_order,
                (SELECT COUNT(*) FROM transactions t WHERE t.category_id = c.id)
                + (SELECT COUNT(*) FROM transaction_splits s WHERE s.category_id = c.id)
                + (SELECT COUNT(*) FROM budget_allocations b WHERE b.category_id = c.id)
         FROM categories c
         WHERE c.tenant_id = ?1 AND (?2 = 1 OR c.is_archived = 0)
         ORDER BY c.sort_order, c.name",
    )?;
    let rows = stmt.query_map(params![tenant_id, include_archived as i64], |row| {
        Ok(CategoryDto {
            id: row.get(0)?,
            parent_id: row.get(1)?,
            name: row.get(2)?,
            kind: row.get(3)?,
            icon: row.get(4)?,
            color: row.get(5)?,
            is_system: row.get::<_, i64>(6)? == 1,
            is_archived: row.get::<_, i64>(7)? == 1,
            sort_order: row.get(8)?,
            usage_count: row.get(9)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

// ---------------------------------------------------------------------------
// System health and bootstrap
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemHealth {
    pub status: String,
    pub integrity: String,
    pub encrypted: bool,
    pub data_dir: String,
}

#[tauri::command]
pub fn health(state: State<'_, AppState>) -> AppResult<SystemHealth> {
    let conn = state.conn();
    let integrity = db::integrity_check(&conn)?;
    Ok(SystemHealth {
        status: if integrity == "ok" { "ok" } else { "degraded" }.to_string(),
        integrity,
        encrypted: state.database_encrypted,
        data_dir: state.data_dir.display().to_string(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub data_dir: String,
    pub database_encrypted: bool,
    pub today: String,
    pub primary_currency: String,
    pub locale: String,
    pub schema_version: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonInfo {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyDto {
    pub code: String,
    pub name: String,
    pub symbol: String,
    pub exponent: i64,
    pub is_primary: bool,
    pub latest_rate: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub transactions: i64,
    pub accounts: i64,
    pub budgets: i64,
    pub goals: i64,
    pub instruments: i64,
    pub liabilities: i64,
    pub journal: i64,
    pub imports: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub app: AppInfo,
    pub tenant: PersonInfo,
    pub user: PersonInfo,
    pub settings: HashMap<String, Value>,
    pub currencies: Vec<CurrencyDto>,
    pub accounts: Vec<account_service::AccountDto>,
    pub categories: Vec<CategoryDto>,
    pub counts: Counts,
    pub net_worth: NetWorth,
    pub onboarding_completed: bool,
}

fn settings_map_inner(
    conn: &rusqlite::Connection,
    tenant_id: &str,
) -> AppResult<HashMap<String, Value>> {
    let mut stmt =
        conn.prepare("SELECT key, value FROM tenant_settings WHERE tenant_id = ?1 ORDER BY key")?;
    let rows = stmt.query_map([tenant_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (key, raw) = row?;
        let value = serde_json::from_str(&raw).unwrap_or(Value::String(raw));
        map.insert(key, value);
    }
    Ok(map)
}

fn count_table(conn: &rusqlite::Connection, table: &str, tenant_id: &str) -> AppResult<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE tenant_id = ?1");
    Ok(conn.query_row(&sql, [tenant_id], |row| row.get(0))?)
}

#[tauri::command]
pub fn bootstrap(state: State<'_, AppState>) -> AppResult<Bootstrap> {
    let conn = state.conn();
    let tenant_id = &state.tenant_id;
    let settings = settings_map_inner(&conn, tenant_id)?;
    let primary = primary_currency(&conn, tenant_id)?;

    let tenant_name: String = conn.query_row(
        "SELECT name FROM tenants WHERE id = ?1",
        [tenant_id],
        |row| row.get(0),
    )?;
    let user_name: String = conn.query_row(
        "SELECT display_name FROM users WHERE id = ?1",
        [&state.user_id],
        |row| row.get(0),
    )?;

    let mut currency_stmt = conn.prepare(
        "SELECT c.code, c.name, c.symbol, c.exponent,
                (SELECT r.rate FROM exchange_rates r
                 WHERE r.tenant_id = ?1 AND r.base_code = ?2 AND r.quote_code = c.code
                 ORDER BY r.as_of_date DESC, r.fetched_at DESC LIMIT 1)
         FROM currencies c
         WHERE c.is_active = 1
         ORDER BY c.code",
    )?;
    let currencies = currency_stmt
        .query_map(params![tenant_id, primary], |row| {
            Ok(CurrencyDto {
                code: row.get(0)?,
                name: row.get(1)?,
                symbol: row.get(2)?,
                exponent: row.get(3)?,
                is_primary: false,
                latest_rate: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let currencies = currencies
        .into_iter()
        .map(|mut currency| {
            currency.is_primary = currency.code == primary;
            currency
        })
        .collect::<Vec<_>>();

    let accounts = account_service::list_accounts(&conn, tenant_id, false)?;
    let categories = list_categories(&conn, tenant_id, false)?;
    let net_worth = account_service::net_worth(&conn, tenant_id, None)?;

    let counts = Counts {
        transactions: count_table(&conn, "transactions", tenant_id)?,
        accounts: count_table(&conn, "accounts", tenant_id)?,
        budgets: count_table(&conn, "budgets", tenant_id)?,
        goals: count_table(&conn, "goals", tenant_id)?,
        instruments: count_table(&conn, "instruments", tenant_id)?,
        liabilities: count_table(&conn, "liabilities", tenant_id)?,
        journal: count_table(&conn, "journal_entries", tenant_id)?,
        imports: count_table(&conn, "import_jobs", tenant_id)?,
    };

    let schema_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let locale = settings
        .get("locale")
        .and_then(Value::as_str)
        .unwrap_or("en")
        .to_string();
    let onboarding_completed = settings
        .get("onboarding_completed")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(Bootstrap {
        app: AppInfo {
            name: "Rawjan".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            data_dir: state.data_dir.display().to_string(),
            database_encrypted: state.database_encrypted,
            today: crate::dates::iso(crate::dates::today_local()),
            primary_currency: primary,
            locale,
            schema_version,
        },
        tenant: PersonInfo {
            id: tenant_id.clone(),
            display_name: tenant_name,
        },
        user: PersonInfo {
            id: state.user_id.clone(),
            display_name: user_name,
        },
        settings,
        currencies,
        accounts,
        categories,
        counts,
        net_worth,
        onboarding_completed,
    })
}

#[tauri::command]
pub fn settings_map(state: State<'_, AppState>) -> AppResult<HashMap<String, Value>> {
    let conn = state.conn();
    settings_map_inner(&conn, &state.tenant_id)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingInput {
    pub key: String,
    pub value: Value,
}

#[tauri::command]
pub fn update_setting(state: State<'_, AppState>, input: SettingInput) -> AppResult<Value> {
    let conn = state.conn();
    let key = input.key.trim();
    if key.is_empty() {
        return Err(AppError::Validation("Setting key is required.".into()));
    }
    // Settings are a closed vocabulary: unknown keys are rejected so arbitrary
    // values can never be smuggled into `tenant_settings` (and from there into
    // the audit trail and encrypted exports).
    const SETTING_KEYS: &[&str] = &[
        "primary_currency",
        "locale",
        "budget_cycle_start_day",
        "salary_pay_day",
        "onboarding_completed",
        "backup_enabled",
        "backup_retention_days",
        "backup_time",
        "app_lock",
        "emergency_fund_months",
    ];
    if !SETTING_KEYS.contains(&key) {
        return Err(AppError::Validation(format!(
            "'{key}' is not a known setting."
        )));
    }
    match key {
        "primary_currency" => {
            let code = input.value.as_str().ok_or_else(|| {
                AppError::Validation("Primary currency must be a currency code.".into())
            })?;
            let exists: Option<i64> = conn
                .query_row(
                    "SELECT 1 FROM currencies WHERE code = ?1 AND is_active = 1",
                    [code],
                    |row| row.get(0),
                )
                .optional()?;
            if exists.is_none() {
                return Err(AppError::Validation(format!("Unknown currency '{code}'.")));
            }
        }
        "budget_cycle_start_day" => {
            let day = input
                .value
                .as_i64()
                .ok_or_else(|| AppError::Validation("Budget cycle day must be a number.".into()))?;
            if !(1..=31).contains(&day) {
                return Err(AppError::Validation(
                    "Budget cycle day must be between 1 and 31.".into(),
                ));
            }
        }
        "salary_pay_day" => {
            let day = input
                .value
                .as_i64()
                .ok_or_else(|| AppError::Validation("Pay day must be a number.".into()))?;
            if !(1..=31).contains(&day) {
                return Err(AppError::Validation(
                    "Pay day must be between 1 and 31.".into(),
                ));
            }
        }
        "backup_retention_days" => {
            let days = input.value.as_i64().ok_or_else(|| {
                AppError::Validation("Retention must be a number of days.".into())
            })?;
            if !(1..=3650).contains(&days) {
                return Err(AppError::Validation(
                    "Retention must be between 1 and 3650 days.".into(),
                ));
            }
        }
        "emergency_fund_months" => {
            let months = input.value.as_i64().ok_or_else(|| {
                AppError::Validation("Emergency fund months must be a number.".into())
            })?;
            if !(1..=36).contains(&months) {
                return Err(AppError::Validation(
                    "Emergency fund months must be between 1 and 36.".into(),
                ));
            }
        }
        _ => {}
    }

    // `app_lock` may hold credential material in the future; its values are
    // never copied into the audit trail.
    let sensitive = key == "app_lock";
    let before = if sensitive {
        None
    } else {
        db::get_setting_value(&conn, &state.tenant_id, key)?
    };
    db::set_setting_value(&conn, &state.tenant_id, key, &input.value)?;
    db::audit(
        &conn,
        &state.tenant_id,
        &state.user_id,
        "setting",
        key,
        "update",
        before.as_ref(),
        if sensitive { None } else { Some(&input.value) },
    )?;
    Ok(input.value)
}

// ---------------------------------------------------------------------------
// Exchange rates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRateDto {
    pub id: String,
    pub base: String,
    pub quote: String,
    pub rate: String,
    pub as_of_date: String,
    pub source: String,
    pub fetched_at: String,
}

fn load_rate(conn: &rusqlite::Connection, tenant_id: &str, id: &str) -> AppResult<ExchangeRateDto> {
    conn.query_row(
        "SELECT id, base_code, quote_code, rate, as_of_date, source, fetched_at
         FROM exchange_rates WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
        |row| {
            Ok(ExchangeRateDto {
                id: row.get(0)?,
                base: row.get(1)?,
                quote: row.get(2)?,
                rate: row.get(3)?,
                as_of_date: row.get(4)?,
                source: row.get(5)?,
                fetched_at: row.get(6)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("Exchange rate not found.".into()))
}

#[tauri::command]
pub fn list_exchange_rates(
    state: State<'_, AppState>,
    base_code: Option<String>,
) -> AppResult<Vec<ExchangeRateDto>> {
    let conn = state.conn();
    let mut stmt = conn.prepare(
        "SELECT id, base_code, quote_code, rate, as_of_date, source, fetched_at
         FROM exchange_rates
         WHERE tenant_id = ?1 AND (?2 IS NULL OR base_code = ?2)
         ORDER BY as_of_date DESC, fetched_at DESC
         LIMIT 500",
    )?;
    let filter = base_code.filter(|value| !value.trim().is_empty());
    let rows = stmt.query_map(params![state.tenant_id, filter], |row| {
        Ok(ExchangeRateDto {
            id: row.get(0)?,
            base: row.get(1)?,
            quote: row.get(2)?,
            rate: row.get(3)?,
            as_of_date: row.get(4)?,
            source: row.get(5)?,
            fetched_at: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[tauri::command]
pub fn add_exchange_rate(
    state: State<'_, AppState>,
    base_code: String,
    quote_code: String,
    rate: String,
    as_of_date: String,
) -> AppResult<ExchangeRateDto> {
    let conn = state.conn();
    let date = crate::dates::iso(crate::dates::parse_date(&as_of_date)?);
    let parsed = parse_rate(&rate)?;
    let snapshot = add_manual_rate(
        &conn,
        &state.tenant_id,
        &base_code,
        &quote_code,
        parsed,
        &date,
    )?;
    let id = snapshot
        .id
        .ok_or_else(|| AppError::Internal("Exchange rate was saved without an id.".into()))?;
    let stored = load_rate(&conn, &state.tenant_id, &id)?;
    db::audit(
        &conn,
        &state.tenant_id,
        &state.user_id,
        "exchange_rate",
        &stored.id,
        "create",
        None,
        Some(&json!({
            "base": stored.base,
            "quote": stored.quote,
            "rate": stored.rate,
            "asOfDate": stored.as_of_date
        })),
    )?;
    Ok(stored)
}

// ---------------------------------------------------------------------------
// Audit trail
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventDto {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub action: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub created_at: String,
}

#[tauri::command]
pub fn list_audit_events(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<AuditEventDto>> {
    let conn = state.conn();
    let limit = limit.unwrap_or(50).clamp(1, 500);
    let mut stmt = conn.prepare(
        "SELECT id, entity_type, entity_id, action, before_json, after_json, created_at
         FROM audit_events WHERE tenant_id = ?1
         ORDER BY created_at DESC, rowid DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![state.tenant_id, limit], |row| {
        let before_raw: Option<String> = row.get(4)?;
        let after_raw: Option<String> = row.get(5)?;
        Ok(AuditEventDto {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            entity_id: row.get(2)?,
            action: row.get(3)?,
            before: before_raw.and_then(|raw| serde_json::from_str(&raw).ok()),
            after: after_raw.and_then(|raw| serde_json::from_str(&raw).ok()),
            created_at: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

// ---------------------------------------------------------------------------
// Integration status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    pub id: String,
    pub name: String,
    pub category: String,
    pub available: bool,
    pub reason: String,
    pub documentation: Option<String>,
}

#[tauri::command]
pub fn list_integration_status() -> Vec<IntegrationStatus> {
    let item = |id: &str,
                name: &str,
                category: &str,
                available: bool,
                reason: &str,
                documentation: Option<&str>| IntegrationStatus {
        id: id.into(),
        name: name.into(),
        category: category.into(),
        available,
        reason: reason.into(),
        documentation: documentation.map(str::to_string),
    };
    vec![
        item(
            "statement_import",
            "Statement import",
            "import",
            true,
            "CSV, XLSX, OFX and QIF statements can be previewed, mapped, validated and de-duplicated before anything is committed. PDF statements are not parsed: export CSV, OFX or QIF from your bank instead. Imports never contact a bank.",
            None,
        ),
        item(
            "market_prices",
            "Market price feed",
            "market data",
            false,
            "No market data provider is connected. Investment values come only from valuations you record manually.",
            None,
        ),
        item(
            "exchange_rates",
            "Exchange-rate feed",
            "market data",
            false,
            "Exchange rates are entered manually and stored as immutable daily snapshots. No rate feed is connected.",
            None,
        ),
        item(
            "cloud_sync",
            "Cloud sync",
            "sync",
            false,
            "Rawjan is local-first by design. There is no cloud account, no sync service and no server holding your data.",
            None,
        ),
    ]
}
