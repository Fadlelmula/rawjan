use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::dates::{iso, today_local};
use crate::error::{AppError, AppResult};
use crate::money::format_amount;
use crate::services::currency::{convert_to_currency, exponent, primary_currency};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub institution: Option<String>,
    pub currency: String,
    pub currency_symbol: String,
    pub is_liability: bool,
    pub is_liquid: bool,
    pub balance_minor: i64,
    pub balance: String,
    pub balance_base_minor: Option<i64>,
    pub balance_base: Option<String>,
    pub base_currency: String,
    pub rate_missing: bool,
    pub credit_limit_minor: Option<i64>,
    pub available_minor: Option<i64>,
    pub available: Option<String>,
    pub statement_day: Option<i64>,
    pub payment_due_day: Option<i64>,
    pub broker_id: Option<String>,
    pub notes: Option<String>,
    pub color: Option<String>,
    pub sort_order: i64,
    pub is_active: bool,
    pub opening_balance_minor: i64,
    pub opening_date: Option<String>,
    pub created_at: String,
    pub market_value_minor: Option<i64>,
}

pub fn balances(conn: &Connection, tenant_id: &str) -> AppResult<HashMap<String, i64>> {
    let mut stmt = conn.prepare(
        "SELECT a.id,
                COALESCE(SUM(CASE WHEN t.id IS NOT NULL THEN
                    (CASE WHEN le.entry_type = 'debit' THEN le.amount_minor ELSE -le.amount_minor END)
                ELSE 0 END), 0)
         FROM accounts a
         LEFT JOIN ledger_entries le ON le.account_id = a.id
         LEFT JOIN transactions t ON t.id = le.transaction_id AND t.status IN ('posted','reconciled','reversed')
         WHERE a.tenant_id = ?1
         GROUP BY a.id",
    )?;
    let rows = stmt.query_map([tenant_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    let mut result = HashMap::new();
    for row in rows {
        let (id, balance) = row?;
        result.insert(id, balance);
    }
    Ok(result)
}

fn latest_valuation(conn: &Connection, tenant_id: &str, account_id: &str, on_date: &str) -> AppResult<Option<(i64, String)>> {
    Ok(conn
        .query_row(
            "SELECT value_minor, currency_code FROM investment_valuations
             WHERE tenant_id = ?1 AND account_id = ?2 AND as_of_date <= ?3
             ORDER BY as_of_date DESC, created_at DESC LIMIT 1",
            params![tenant_id, account_id, on_date],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

pub fn list_accounts(conn: &Connection, tenant_id: &str, include_archived: bool) -> AppResult<Vec<AccountDto>> {
    let base_currency = primary_currency(conn, tenant_id)?;
    let base_exp = exponent(conn, &base_currency)?;
    let today = iso(today_local());
    let balances = balances(conn, tenant_id)?;

    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.type, a.institution, a.currency_code, cur.symbol, a.is_liability, a.is_liquid,
                a.credit_limit_minor, a.statement_day, a.payment_due_day, a.broker_id, a.notes, a.color,
                a.sort_order, a.is_active, a.opening_balance_minor, a.opening_date, a.created_at, a.archived_at
         FROM accounts a
         JOIN currencies cur ON cur.code = a.currency_code
         WHERE a.tenant_id = ?1 AND (?2 = 1 OR a.is_active = 1)
         ORDER BY a.is_active DESC, a.sort_order, a.name",
    )?;
    let rows = stmt
        .query_map(params![tenant_id, if include_archived { 1 } else { 0 }], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)? == 1,
                row.get::<_, i64>(7)? == 1,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<i64>>(9)?,
                row.get::<_, Option<i64>>(10)?,
                row.get::<_, Option<String>>(11)?,
                row.get::<_, Option<String>>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, i64>(14)?,
                row.get::<_, i64>(15)? == 1,
                row.get::<_, i64>(16)?,
                row.get::<_, Option<String>>(17)?,
                row.get::<_, String>(18)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut result = Vec::with_capacity(rows.len());
    for (id, name, account_type, institution, currency, symbol, is_liability, is_liquid, credit_limit,
         statement_day, payment_due_day, broker_id, notes, color, sort_order, is_active, opening_balance,
         opening_date, created_at) in rows
    {
        let account_exp = exponent(conn, &currency)?;
        let ledger_balance = *balances.get(&id).unwrap_or(&0);
        let market_value = if account_type == "investment" {
            latest_valuation(conn, tenant_id, &id, &today)?.map(|(value, _)| value)
        } else {
            None
        };
        let balance_minor = market_value.unwrap_or(ledger_balance);
        let (balance_base_minor, rate_missing) = if currency == base_currency {
            (Some(balance_minor), false)
        } else {
            match convert_to_currency(conn, tenant_id, balance_minor, &currency, &base_currency, &today) {
                Ok((converted, _)) => (Some(converted), false),
                Err(_) => (None, true),
            }
        };
        result.push(AccountDto {
            id,
            name,
            account_type: account_type.clone(),
            institution,
            currency: currency.clone(),
            currency_symbol: symbol,
            is_liability,
            is_liquid,
            balance_minor,
            balance: format_amount(balance_minor, account_exp),
            balance_base_minor,
            balance_base: balance_base_minor.map(|v| format_amount(v, base_exp)),
            base_currency: base_currency.clone(),
            rate_missing,
            credit_limit_minor: credit_limit,
            available_minor: credit_limit.map(|limit| limit + balance_minor),
            available: credit_limit.map(|limit| format_amount(limit + balance_minor, account_exp)),
            statement_day,
            payment_due_day,
            broker_id,
            notes,
            color,
            sort_order,
            is_active,
            opening_balance_minor: opening_balance,
            opening_date,
            created_at,
            market_value_minor: if account_type == "investment" { Some(balance_minor) } else { None },
        });
    }
    Ok(result)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInput {
    pub name: String,
    pub account_type: String,
    pub institution: Option<String>,
    pub currency_code: String,
    pub is_liability: Option<bool>,
    pub is_liquid: Option<bool>,
    pub opening_balance: Option<String>,
    pub opening_date: Option<String>,
    pub credit_limit: Option<String>,
    pub statement_day: Option<i64>,
    pub payment_due_day: Option<i64>,
    pub broker_id: Option<String>,
    pub notes: Option<String>,
    pub color: Option<String>,
    pub sort_order: Option<i64>,
}

pub fn create_account(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    input: &AccountInput,
) -> AppResult<String> {
    const TYPES: &[&str] = &["cash", "bank", "debit_card", "credit_card", "savings", "wallet", "investment", "loan", "other"];
    if !TYPES.contains(&input.account_type.as_str()) {
        return Err(AppError::Validation("Unknown account type.".into()));
    }
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("Account name is required.".into()));
    }
    let meta = crate::services::currency::currency_meta(conn, &input.currency_code)?;
    let is_liability = input.is_liability.unwrap_or(matches!(input.account_type.as_str(), "credit_card" | "loan"));
    let is_liquid = input.is_liquid.unwrap_or(!matches!(input.account_type.as_str(), "investment" | "loan"));
    let opening_minor = match input.opening_balance.as_deref().filter(|v| !v.trim().is_empty()) {
        Some(value) => crate::money::parse_amount(value, meta.exponent)?,
        None => 0,
    };
    let opening_date = input.opening_date.clone().unwrap_or_else(|| iso(today_local()));
    crate::dates::parse_date(&opening_date)?;
    let credit_limit = match input.credit_limit.as_deref().filter(|v| !v.trim().is_empty()) {
        Some(value) => Some(crate::money::parse_amount(value, meta.exponent)?),
        None => None,
    };
    if let Some(day) = input.statement_day {
        if !(1..=31).contains(&day) {
            return Err(AppError::Validation("Statement day must be between 1 and 31.".into()));
        }
    }
    if let Some(day) = input.payment_due_day {
        if !(1..=31).contains(&day) {
            return Err(AppError::Validation("Payment due day must be between 1 and 31.".into()));
        }
    }
    if let Some(broker) = &input.broker_id {
        let exists: i64 = conn.query_row(
            "SELECT count(*) FROM brokers WHERE id = ?1 AND tenant_id = ?2",
            params![broker, tenant_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(AppError::NotFound("Broker not found.".into()));
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = crate::dates::now_iso();
    let db_tx = conn.unchecked_transaction()?;
    db_tx.execute(
        "INSERT INTO accounts
         (id, tenant_id, user_id, name, type, institution, currency_code, is_liability, is_liquid,
          opening_balance_minor, opening_date, credit_limit_minor, statement_day, payment_due_day, broker_id,
          notes, color, sort_order, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, 1, ?19, ?19)",
        params![
            id,
            tenant_id,
            user_id,
            input.name.trim(),
            input.account_type,
            input.institution.as_deref().map(str::trim).filter(|v| !v.is_empty()),
            input.currency_code,
            if is_liability { 1 } else { 0 },
            if is_liquid { 1 } else { 0 },
            opening_minor,
            opening_date,
            credit_limit,
            input.statement_day,
            input.payment_due_day,
            input.broker_id.as_deref().filter(|v| !v.is_empty()),
            input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
            input.color.as_deref(),
            input.sort_order.unwrap_or(0),
            now
        ],
    )?;
    db_tx.commit()?;

    if opening_minor != 0 {
        let mut op = crate::services::tx::TxInput {
            kind: "opening".into(),
            status: Some("posted".into()),
            data_status: Some("actual".into()),
            description: "Opening balance".into(),
            payee: None,
            notes: Some("Created with the account".into()),
            date: opening_date.clone(),
            time: None,
            account_id: id.clone(),
            counter_account_id: None,
            category_id: None,
            income_source_id: None,
            amount: format_minor_input(opening_minor, meta.exponent),
            currency: Some(input.currency_code.clone()),
            counter_amount: None,
            fx_rate: None,
            splits: None,
            tags: None,
            external_ref: None,
            direction: Some(if opening_minor >= 0 { "in".into() } else { "out".into() }),
        };
        // Opening negatives are represented as an outflow of the absolute amount.
        if opening_minor < 0 {
            op.amount = format_minor_input(-opening_minor, meta.exponent);
        }
        match crate::services::tx::create_transaction(conn, tenant_id, user_id, &op) {
            Ok(_) => {}
            Err(e) => {
                conn.execute("DELETE FROM accounts WHERE id = ?1", [&id])?;
                return Err(e);
            }
        }
    }

    crate::db::audit(conn, tenant_id, user_id, "account", &id, "create", None, Some(&serde_json::json!({
        "name": input.name.trim(),
        "type": input.account_type,
        "currency": input.currency_code,
        "openingBalanceMinor": opening_minor
    })))?;
    Ok(id)
}

pub fn format_minor_input(minor: i64, exponent: u32) -> String {
    crate::money::format_amount(minor, exponent).replace(',', "")
}

pub fn update_account(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
    input: &AccountInput,
) -> AppResult<()> {
    let existing: Option<(String, i64)> = conn
        .query_row(
            "SELECT currency_code, is_active FROM accounts WHERE id = ?1 AND tenant_id = ?2",
            params![id, tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (existing_currency, is_active) = existing.ok_or_else(|| AppError::NotFound("Account not found.".into()))?;
    if is_active == 0 {
        return Err(AppError::Conflict("Archived accounts cannot be edited. Restore the account first.".into()));
    }
    let meta = crate::services::currency::currency_meta(conn, &input.currency_code)?;
    if input.currency_code != existing_currency {
        let entry_count: i64 = conn.query_row(
            "SELECT count(*) FROM ledger_entries WHERE account_id = ?1",
            [id],
            |r| r.get(0),
        )?;
        if entry_count > 0 {
            return Err(AppError::Conflict(
                "The currency of an account with ledger activity cannot be changed. Create a new account instead.".into(),
            ));
        }
    }
    let credit_limit = match input.credit_limit.as_deref().filter(|v| !v.trim().is_empty()) {
        Some(value) => Some(crate::money::parse_amount(value, meta.exponent)?),
        None => None,
    };
    let is_liability = input.is_liability.unwrap_or(matches!(input.account_type.as_str(), "credit_card" | "loan"));
    let is_liquid = input.is_liquid.unwrap_or(!matches!(input.account_type.as_str(), "investment" | "loan"));
    conn.execute(
        "UPDATE accounts SET name = ?1, type = ?2, institution = ?3, currency_code = ?4, is_liability = ?5,
            is_liquid = ?6, credit_limit_minor = ?7, statement_day = ?8, payment_due_day = ?9, broker_id = ?10,
            notes = ?11, color = ?12, sort_order = ?13, updated_at = ?14
         WHERE id = ?15 AND tenant_id = ?16",
        params![
            input.name.trim(),
            input.account_type,
            input.institution.as_deref().map(str::trim).filter(|v| !v.is_empty()),
            input.currency_code,
            if is_liability { 1 } else { 0 },
            if is_liquid { 1 } else { 0 },
            credit_limit,
            input.statement_day,
            input.payment_due_day,
            input.broker_id.as_deref().filter(|v| !v.is_empty()),
            input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
            input.color.as_deref(),
            input.sort_order.unwrap_or(0),
            crate::dates::now_iso(),
            id,
            tenant_id
        ],
    )?;
    crate::db::audit(conn, tenant_id, user_id, "account", id, "update", None, None)?;
    Ok(())
}

pub fn archive_account(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
    archive: bool,
) -> AppResult<()> {
    if archive {
        let balance = *balances(conn, tenant_id)?.get(id).unwrap_or(&0);
        if balance != 0 {
            return Err(AppError::Conflict(
                "Only accounts with a zero balance can be archived. Transfer the remaining balance or record an adjustment first.".into(),
            ));
        }
        let open_liabilities: i64 = conn.query_row(
            "SELECT count(*) FROM liabilities WHERE account_id = ?1 AND status = 'active' AND outstanding_minor > 0",
            [id],
            |r| r.get(0),
        )?;
        if open_liabilities > 0 {
            return Err(AppError::Conflict("This account has an active liability. Settle or unlink it before archiving.".into()));
        }
    }
    conn.execute(
        "UPDATE accounts SET is_active = ?1, archived_at = ?2, updated_at = ?2 WHERE id = ?3 AND tenant_id = ?4",
        params![if archive { 0 } else { 1 }, if archive { Some(crate::dates::now_iso()) } else { None }, id, tenant_id],
    )?;
    crate::db::audit(conn, tenant_id, user_id, "account", id, if archive { "archive" } else { "restore" }, None, None)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorth {
    pub as_of_date: String,
    pub currency: String,
    pub assets_minor: i64,
    pub liabilities_minor: i64,
    pub net_minor: i64,
    pub assets: String,
    pub liabilities: String,
    pub net: String,
    pub cash_minor: i64,
    pub cash: String,
    pub invested_minor: i64,
    pub invested: String,
    pub receivables_minor: i64,
    pub receivables: String,
    pub other_assets_minor: i64,
    pub other_assets: String,
    pub missing_rates: Vec<String>,
    pub accounts_included: i64,
}

pub fn net_worth(conn: &Connection, tenant_id: &str, on_date: Option<&str>) -> AppResult<NetWorth> {
    let base_currency = primary_currency(conn, tenant_id)?;
    let base_exp = exponent(conn, &base_currency)?;
    let date = on_date.map(str::to_string).unwrap_or_else(|| iso(today_local()));
    let balances = balances(conn, tenant_id)?;

    let mut stmt = conn.prepare(
        "SELECT a.id, a.type, a.currency_code, a.is_liability, a.is_liquid,
                (a.is_liability = 1 AND EXISTS(
                    SELECT 1 FROM liabilities l
                    WHERE l.account_id = a.id AND l.status = 'active' AND l.type <> 'money_lent'
                )) AS has_liability
         FROM accounts a WHERE a.tenant_id = ?1 AND a.is_active = 1",
    )?;
    let accounts: Vec<(String, String, String, bool, bool, bool)> = stmt
        .query_map([tenant_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get::<_, i64>(3)? == 1,
                row.get::<_, i64>(4)? == 1,
                row.get::<_, i64>(5)? == 1,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut assets = 0i64;
    let mut liabilities = 0i64;
    let mut cash = 0i64;
    let mut invested = 0i64;
    let mut receivable_total = 0i64;
    let mut missing = Vec::new();

    for (id, account_type, currency, is_liability, is_liquid, has_liability) in &accounts {
        if *has_liability {
            // The liability record is the source of truth for linked debt.
            continue;
        }
        let ledger = *balances.get(id).unwrap_or(&0);
        let balance = if account_type == "investment" {
            latest_valuation(conn, tenant_id, id, &date)?
                .map(|(v, _)| v)
                .unwrap_or(ledger)
        } else {
            ledger
        };
        let converted = if currency == &base_currency {
            balance
        } else {
            match convert_to_currency(conn, tenant_id, balance, currency, &base_currency, &date) {
                Ok((value, _)) => value,
                Err(_) => {
                    missing.push(currency.clone());
                    continue;
                }
            }
        };
        if *is_liability {
            liabilities += converted.abs();
        } else {
            assets += converted;
            if account_type == "investment" {
                invested += converted;
            } else if *is_liquid {
                cash += converted;
            }
        }
    }

    // Debt liabilities are the source of truth for linked debt accounts, and are
    // counted here for every active record so nothing is silently dropped.
    let mut liability_stmt = conn.prepare(
        "SELECT currency_code, outstanding_minor FROM liabilities
         WHERE tenant_id = ?1 AND status = 'active' AND type <> 'money_lent' AND outstanding_minor > 0",
    )?;
    let debts: Vec<(String, i64)> = liability_stmt
        .query_map([tenant_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (currency, outstanding) in debts {
        let converted = if currency == base_currency {
            outstanding
        } else {
            match convert_to_currency(conn, tenant_id, outstanding, &currency, &base_currency, &date) {
                Ok((value, _)) => value,
                Err(_) => {
                    missing.push(currency);
                    continue;
                }
            }
        };
        liabilities += converted;
    }

    // Money lent to someone is a receivable: it belongs on the asset side.
    let mut receivable_stmt = conn.prepare(
        "SELECT currency_code, outstanding_minor FROM liabilities
         WHERE tenant_id = ?1 AND status = 'active' AND type = 'money_lent' AND outstanding_minor > 0",
    )?;
    let receivables: Vec<(String, i64)> = receivable_stmt
        .query_map([tenant_id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (currency, outstanding) in receivables {
        let converted = if currency == base_currency {
            outstanding
        } else {
            match convert_to_currency(conn, tenant_id, outstanding, &currency, &base_currency, &date) {
                Ok((value, _)) => value,
                Err(_) => {
                    missing.push(currency);
                    continue;
                }
            }
        };
        assets += converted;
        receivable_total += converted;
    }

    let net = assets - liabilities;
    let other_assets = assets - cash - invested - receivable_total;
    Ok(NetWorth {
        as_of_date: date,
        currency: base_currency.clone(),
        assets_minor: assets,
        liabilities_minor: liabilities,
        net_minor: net,
        assets: format_amount(assets, base_exp),
        liabilities: format_amount(liabilities, base_exp),
        net: format_amount(net, base_exp),
        cash_minor: cash,
        cash: format_amount(cash, base_exp),
        invested_minor: invested,
        invested: format_amount(invested, base_exp),
        receivables_minor: receivable_total,
        receivables: format_amount(receivable_total, base_exp),
        other_assets_minor: other_assets,
        other_assets: format_amount(other_assets, base_exp),
        missing_rates: {
            missing.sort();
            missing.dedup();
            missing
        },
        accounts_included: accounts.len() as i64,
    })
}

pub fn save_net_worth_snapshot(conn: &Connection, tenant_id: &str) -> AppResult<NetWorth> {
    let worth = net_worth(conn, tenant_id, None)?;
    conn.execute(
        "INSERT INTO net_worth_snapshots
         (id, tenant_id, as_of_date, currency_code, assets_minor, liabilities_minor, net_minor, cash_minor,
          invested_minor, breakdown_json, source, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'computed', ?11)
         ON CONFLICT(tenant_id, as_of_date) DO UPDATE SET
           assets_minor = excluded.assets_minor, liabilities_minor = excluded.liabilities_minor,
           net_minor = excluded.net_minor, cash_minor = excluded.cash_minor,
           invested_minor = excluded.invested_minor, breakdown_json = excluded.breakdown_json,
           source = excluded.source, created_at = excluded.created_at",
        params![
            uuid::Uuid::new_v4().to_string(),
            tenant_id,
            worth.as_of_date,
            worth.currency,
            worth.assets_minor,
            worth.liabilities_minor,
            worth.net_minor,
            worth.cash_minor,
            worth.invested_minor,
            serde_json::json!({
                "missingRates": worth.missing_rates,
                "cashMinor": worth.cash_minor,
                "investedMinor": worth.invested_minor,
                "receivablesMinor": worth.receivables_minor,
                "otherAssetsMinor": worth.other_assets_minor
            })
            .to_string(),
            crate::dates::now_iso()
        ],
    )?;
    Ok(worth)
}

pub fn account_balance_in_base(
    conn: &Connection,
    tenant_id: &str,
    account_id: &str,
) -> AppResult<i64> {
    let (currency, ledger): (String, i64) = conn
        .query_row(
            "SELECT a.currency_code, COALESCE(SUM(CASE WHEN le.entry_type='debit' THEN le.amount_minor ELSE -le.amount_minor END),0)
             FROM accounts a
             LEFT JOIN ledger_entries le ON le.account_id = a.id
             LEFT JOIN transactions t ON t.id = le.transaction_id AND t.status IN ('posted','reconciled','reversed')
             WHERE a.id = ?1 AND a.tenant_id = ?2 GROUP BY a.id",
            params![account_id, tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Account not found.".into()))?;
    let base = primary_currency(conn, tenant_id)?;
    if currency == base {
        return Ok(ledger);
    }
    let (converted, _) = convert_to_currency(conn, tenant_id, ledger, &currency, &base, &iso(today_local()))?;
    Ok(converted)
}


