use std::collections::HashMap;

use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::dates::{self, iso, today_local};
use crate::error::{AppError, AppResult};
use crate::money::{format_amount, parse_amount};
use crate::services::currency::{convert_to_currency, currency_meta, exponent, primary_currency};

const PERIOD_TYPES: &[&str] = &["calendar_month", "cycle_month", "weekly", "daily"];
const METHODS: &[&str] = &["custom", "zero_based", "envelope", "fifty_thirty_twenty"];
const ROLLOVER: &[&str] = &["none", "unspent", "full"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetAllocationDto {
    pub id: String,
    pub category_id: String,
    pub category_name: String,
    pub category_color: Option<String>,
    pub amount_minor: i64,
    pub amount: String,
    pub spent_minor: i64,
    pub spent: String,
    pub remaining_minor: i64,
    pub remaining: String,
    pub percent_used: i64,
    pub over_budget: bool,
    pub notes: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetDto {
    pub id: String,
    pub name: String,
    pub method: String,
    pub period_type: String,
    pub cycle_start_day: i64,
    pub rollover_policy: String,
    pub currency: String,
    pub currency_symbol: String,
    pub alert_thresholds: Vec<i64>,
    pub is_active: bool,
    pub start_date: String,
    pub allocated_minor: i64,
    pub allocated: String,
    pub spent_minor: i64,
    pub spent: String,
    pub remaining_minor: i64,
    pub remaining: String,
    pub percent_used: i64,
    pub period_start: String,
    pub period_end: String,
    pub allocations: Vec<BudgetAllocationDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetAllocationInput {
    pub category_id: String,
    pub amount: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetInput {
    pub name: String,
    pub method: Option<String>,
    pub period_type: Option<String>,
    pub cycle_start_day: Option<i64>,
    pub rollover_policy: Option<String>,
    pub currency_code: Option<String>,
    pub alert_thresholds: Option<Vec<i64>>,
    pub start_date: Option<String>,
    #[serde(default)]
    pub allocations: Vec<BudgetAllocationInput>,
}

fn percent_used(spent: i64, allocated: i64) -> i64 {
    if allocated <= 0 {
        return 0;
    }
    ((spent.max(0) as i128 * 100) / allocated as i128) as i64
}

/// Resolve the reporting window for a budget. `reference` is the date whose period is shown.
pub fn period_for(
    period_type: &str,
    cycle_start_day: u32,
    reference: NaiveDate,
) -> AppResult<(NaiveDate, NaiveDate)> {
    Ok(match period_type {
        "calendar_month" => dates::calendar_month(reference),
        "cycle_month" => dates::cycle_period(reference, cycle_start_day),
        "weekly" => {
            let offset = reference.weekday().num_days_from_monday() as i64;
            let start = reference - Duration::days(offset);
            (start, start + Duration::days(6))
        }
        "daily" => (reference, reference),
        _ => {
            return Err(AppError::Validation(
                "This budget period type is not supported yet.".into(),
            ))
        }
    })
}

/// Net spend per category in the tenant's primary currency for an inclusive date range.
/// Debits count as spending and refunds/credits reduce it. Reversed and pending rows are excluded.
fn spend_by_category(
    conn: &Connection,
    tenant_id: &str,
    start: &str,
    end: &str,
) -> AppResult<HashMap<String, i64>> {
    let mut stmt = conn.prepare(
        "SELECT le.category_id,
                SUM(CASE WHEN le.entry_type = 'debit' THEN le.base_amount_minor ELSE -le.base_amount_minor END)
         FROM ledger_entries le
         JOIN transactions t ON t.id = le.transaction_id
         WHERE le.tenant_id = ?1
           AND le.category_id IS NOT NULL
           AND t.status IN ('posted','reconciled')
           AND t.reversal_of_id IS NULL
           AND t.local_date BETWEEN ?2 AND ?3
         GROUP BY le.category_id",
    )?;
    let rows = stmt.query_map(params![tenant_id, start, end], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut result = HashMap::new();
    for row in rows {
        let (category, amount) = row?;
        result.insert(category, amount);
    }
    Ok(result)
}

fn parse_thresholds(raw: &str) -> Vec<i64> {
    serde_json::from_str::<Vec<i64>>(raw).unwrap_or_else(|_| vec![50, 75, 90, 100])
}

pub fn list_budgets(
    conn: &Connection,
    tenant_id: &str,
    reference: Option<&str>,
) -> AppResult<Vec<BudgetDto>> {
    let reference_date = match reference {
        Some(value) if !value.trim().is_empty() => dates::parse_date(value)?,
        _ => today_local(),
    };
    let base_currency = primary_currency(conn, tenant_id)?;

    let mut stmt = conn.prepare(
        "SELECT b.id, b.name, b.method, b.period_type, b.cycle_start_day, b.rollover_policy,
                b.currency_code, cur.symbol, b.alert_thresholds, b.is_active, b.start_date
         FROM budgets b
         JOIN currencies cur ON cur.code = b.currency_code
         WHERE b.tenant_id = ?1
         ORDER BY b.is_active DESC, b.name",
    )?;
    let budgets: Vec<(String, String, String, String, i64, String, String, String, String, bool, String)> = stmt
        .query_map([tenant_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get::<_, i64>(9)? == 1,
                row.get(10)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut result = Vec::with_capacity(budgets.len());
    for (
        id,
        name,
        method,
        period_type,
        cycle_start_day,
        rollover_policy,
        currency,
        symbol,
        thresholds_raw,
        is_active,
        start_date,
    ) in budgets
    {
        let (start, end) = period_for(&period_type, cycle_start_day as u32, reference_date)?;
        let (start_s, end_s) = (iso(start), iso(end));
        let currency_exp = exponent(conn, &currency)?;
        let spend_base = spend_by_category(conn, tenant_id, &start_s, &end_s)?;

        let mut alloc_stmt = conn.prepare(
            "SELECT ba.id, ba.category_id, c.name, c.color, ba.amount_minor, ba.notes, ba.sort_order
             FROM budget_allocations ba
             JOIN categories c ON c.id = ba.category_id
             WHERE ba.budget_id = ?1
             ORDER BY ba.sort_order, c.name",
        )?;
        let allocations_raw: Vec<(String, String, String, Option<String>, i64, Option<String>, i64)> = alloc_stmt
            .query_map([&id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut allocations = Vec::with_capacity(allocations_raw.len());
        let mut allocated_total = 0i64;
        let mut spent_total = 0i64;
        for (alloc_id, category_id, category_name, category_color, amount_minor, notes, sort_order) in
            allocations_raw
        {
            let spent_base = *spend_base.get(&category_id).unwrap_or(&0);
            // Allocation amounts live in the budget currency; spend is computed in the primary
            // currency. Convert only when they differ so the comparison stays traceable.
            let spent_minor = if currency == base_currency {
                spent_base
            } else {
                match convert_to_currency(
                    conn,
                    tenant_id,
                    spent_base,
                    &base_currency,
                    &currency,
                    &end_s,
                ) {
                    Ok((value, _)) => value,
                    Err(_) => spent_base,
                }
            };
            let remaining = amount_minor - spent_minor;
            allocated_total += amount_minor;
            spent_total += spent_minor;
            allocations.push(BudgetAllocationDto {
                id: alloc_id,
                category_id,
                category_name,
                category_color,
                amount_minor,
                amount: format_amount(amount_minor, currency_exp),
                spent_minor,
                spent: format_amount(spent_minor, currency_exp),
                remaining_minor: remaining,
                remaining: format_amount(remaining, currency_exp),
                percent_used: percent_used(spent_minor, amount_minor),
                over_budget: remaining < 0,
                notes,
                sort_order,
            });
        }

        let remaining_total = allocated_total - spent_total;
        result.push(BudgetDto {
            id,
            name,
            method,
            period_type,
            cycle_start_day,
            rollover_policy,
            currency,
            currency_symbol: symbol,
            alert_thresholds: parse_thresholds(&thresholds_raw),
            is_active,
            start_date,
            allocated_minor: allocated_total,
            allocated: format_amount(allocated_total, currency_exp),
            spent_minor: spent_total,
            spent: format_amount(spent_total, currency_exp),
            remaining_minor: remaining_total,
            remaining: format_amount(remaining_total, currency_exp),
            percent_used: percent_used(spent_total, allocated_total),
            period_start: start_s,
            period_end: end_s,
            allocations,
        });
    }
    Ok(result)
}

fn validate_category(conn: &Connection, tenant_id: &str, category_id: &str) -> AppResult<()> {
    let kind: Option<String> = conn
        .query_row(
            "SELECT kind FROM categories WHERE id = ?1 AND tenant_id = ?2",
            params![category_id, tenant_id],
            |r| r.get(0),
        )
        .optional()?;
    match kind.as_deref() {
        Some("expense") => Ok(()),
        Some(_) => Err(AppError::Validation(
            "Budgets can only allocate expense categories.".into(),
        )),
        None => Err(AppError::NotFound("Category not found.".into())),
    }
}

pub fn save_budget(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: Option<&str>,
    input: &BudgetInput,
) -> AppResult<String> {
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("Budget name is required.".into()));
    }
    let method = input.method.as_deref().unwrap_or("custom");
    if !METHODS.contains(&method) {
        return Err(AppError::Validation("Unknown budget method.".into()));
    }
    let period_type = input.period_type.as_deref().unwrap_or("cycle_month");
    if !PERIOD_TYPES.contains(&period_type) {
        return Err(AppError::Validation(
            "Only calendar month, salary cycle, weekly and daily budgets are supported.".into(),
        ));
    }
    let rollover = input.rollover_policy.as_deref().unwrap_or("none");
    if !ROLLOVER.contains(&rollover) {
        return Err(AppError::Validation("Unknown rollover policy.".into()));
    }
    let cycle_start_day = input.cycle_start_day.unwrap_or(28);
    if !(1..=31).contains(&cycle_start_day) {
        return Err(AppError::Validation("Cycle start day must be between 1 and 31.".into()));
    }
    let currency_code = input
        .currency_code
        .clone()
        .unwrap_or(primary_currency(conn, tenant_id)?);
    let meta = currency_meta(conn, &currency_code)?;
    let start_date = match input.start_date.as_deref().filter(|v| !v.trim().is_empty()) {
        Some(value) => {
            dates::parse_date(value)?;
            value.to_string()
        }
        None => iso(today_local()),
    };
    let thresholds = input
        .alert_thresholds
        .clone()
        .unwrap_or_else(|| vec![50, 75, 90, 100]);
    if thresholds.iter().any(|v| !(1..=1000).contains(v)) {
        return Err(AppError::Validation(
            "Alert thresholds must be positive percentages.".into(),
        ));
    }

    let mut seen = std::collections::HashSet::new();
    for allocation in &input.allocations {
        validate_category(conn, tenant_id, &allocation.category_id)?;
        if !seen.insert(allocation.category_id.clone()) {
            return Err(AppError::Validation(
                "Each category can appear only once in a budget.".into(),
            ));
        }
    }

    let now = dates::now_iso();
    let db_tx = conn.unchecked_transaction()?;
    let budget_id = match id.filter(|v| !v.is_empty()) {
        Some(existing) => {
            let found: Option<String> = db_tx
                .query_row(
                    "SELECT id FROM budgets WHERE id = ?1 AND tenant_id = ?2",
                    params![existing, tenant_id],
                    |r| r.get(0),
                )
                .optional()?;
            if found.is_none() {
                return Err(AppError::NotFound("Budget not found.".into()));
            }
            db_tx.execute(
                "UPDATE budgets SET name = ?1, method = ?2, period_type = ?3, cycle_start_day = ?4,
                    rollover_policy = ?5, currency_code = ?6, alert_thresholds = ?7, start_date = ?8, updated_at = ?9
                 WHERE id = ?10 AND tenant_id = ?11",
                params![
                    input.name.trim(),
                    method,
                    period_type,
                    cycle_start_day,
                    rollover,
                    currency_code,
                    serde_json::to_string(&thresholds).unwrap_or_else(|_| "[50,75,90,100]".into()),
                    start_date,
                    now,
                    existing,
                    tenant_id
                ],
            )?;
            db_tx.execute("DELETE FROM budget_allocations WHERE budget_id = ?1", [existing])?;
            existing.to_string()
        }
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            db_tx.execute(
                "INSERT INTO budgets (id, tenant_id, user_id, name, method, period_type, cycle_start_day,
                    rollover_policy, currency_code, alert_thresholds, is_active, start_date, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?12, ?12)",
                params![
                    new_id,
                    tenant_id,
                    user_id,
                    input.name.trim(),
                    method,
                    period_type,
                    cycle_start_day,
                    rollover,
                    currency_code,
                    serde_json::to_string(&thresholds).unwrap_or_else(|_| "[50,75,90,100]".into()),
                    start_date,
                    now
                ],
            )?;
            new_id
        }
    };

    for (index, allocation) in input.allocations.iter().enumerate() {
        let amount_minor = parse_amount(&allocation.amount, meta.exponent)?;
        if amount_minor < 0 {
            return Err(AppError::Validation("Budget amounts cannot be negative.".into()));
        }
        db_tx.execute(
            "INSERT INTO budget_allocations (id, tenant_id, budget_id, category_id, amount_minor, notes, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                uuid::Uuid::new_v4().to_string(),
                tenant_id,
                budget_id,
                allocation.category_id,
                amount_minor,
                allocation.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                index as i64
            ],
        )?;
    }
    db_tx.commit()?;

    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "budget",
        &budget_id,
        if id.is_some() { "update" } else { "create" },
        None,
        Some(&serde_json::json!({
            "name": input.name.trim(),
            "method": method,
            "periodType": period_type,
            "allocations": input.allocations.len()
        })),
    )?;
    Ok(budget_id)
}

pub fn set_budget_active(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
    active: bool,
) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE budgets SET is_active = ?1, updated_at = ?2 WHERE id = ?3 AND tenant_id = ?4",
        params![if active { 1 } else { 0 }, dates::now_iso(), id, tenant_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Budget not found.".into()));
    }
    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "budget",
        id,
        if active { "activate" } else { "deactivate" },
        None,
        None,
    )?;
    Ok(())
}

pub fn delete_budget(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
) -> AppResult<()> {
    let changed = conn.execute("DELETE FROM budgets WHERE id = ?1 AND tenant_id = ?2", params![id, tenant_id])?;
    if changed == 0 {
        return Err(AppError::NotFound("Budget not found.".into()));
    }
    crate::db::audit(conn, tenant_id, user_id, "budget", id, "delete", None, None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, open_test};
    use crate::services::tx::{create_transaction, TxInput};

    fn setup() -> (Connection, String, String, String, String) {
        let mut conn = open_test();
        let (tenant, user) = db::seed_if_empty(&mut conn).unwrap();
        let account_id = uuid::Uuid::new_v4().to_string();
        let now = dates::now_iso();
        conn.execute(
            "INSERT INTO accounts (id, tenant_id, user_id, name, type, currency_code, is_liability, is_liquid, opening_balance_minor, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'Main bank', 'bank', 'QAR', 0, 1, 0, ?4, ?4)",
            params![account_id, tenant, user, now],
        )
        .unwrap();
        let category_id: String = conn
            .query_row(
                "SELECT id FROM categories WHERE tenant_id = ?1 AND name = 'Groceries'",
                [&tenant],
                |r| r.get(0),
            )
            .unwrap();
        (conn, tenant, user, account_id, category_id)
    }

    fn expense(account: &str, category: &str, amount: &str, status: &str, date: &str) -> TxInput {
        TxInput {
            kind: "expense".into(),
            status: Some(status.into()),
            data_status: None,
            description: "Test expense".into(),
            payee: None,
            notes: None,
            date: date.into(),
            time: None,
            account_id: account.to_string(),
            counter_account_id: None,
            category_id: Some(category.to_string()),
            income_source_id: None,
            amount: amount.into(),
            currency: None,
            counter_amount: None,
            fx_rate: None,
            splits: None,
            tags: None,
            external_ref: None,
            direction: None,
        }
    }

    #[test]
    fn period_windows_are_explicit() {
        let reference = dates::parse_date("2026-02-15").unwrap();
        let (start, end) = period_for("cycle_month", 28, reference).unwrap();
        assert_eq!(iso(start), "2026-01-28");
        assert_eq!(iso(end), "2026-02-27");

        let (start, end) = period_for("calendar_month", 28, reference).unwrap();
        assert_eq!(iso(start), "2026-02-01");
        assert_eq!(iso(end), "2026-02-28");

        let (start, end) = period_for("weekly", 28, reference).unwrap();
        assert_eq!(iso(start), "2026-02-09");
        assert_eq!(iso(end), "2026-02-15");

        let (start, end) = period_for("daily", 28, reference).unwrap();
        assert_eq!(iso(start), "2026-02-15");
        assert_eq!(iso(end), "2026-02-15");

        assert!(period_for("custom", 28, reference).is_err());
    }

    #[test]
    fn budget_spend_uses_posted_expenses_only() {
        let (conn, tenant, user, account, category) = setup();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &expense(&account, &category, "250.00", "posted", "2026-11-14"),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &expense(&account, &category, "50.00", "pending", "2026-11-14"),
        )
        .unwrap();

        let budget_id = save_budget(
            &conn,
            &tenant,
            &user,
            None,
            &BudgetInput {
                name: "Essentials".into(),
                method: Some("custom".into()),
                period_type: Some("calendar_month".into()),
                cycle_start_day: None,
                rollover_policy: Some("none".into()),
                currency_code: Some("QAR".into()),
                alert_thresholds: None,
                start_date: Some("2026-11-01".into()),
                allocations: vec![BudgetAllocationInput {
                    category_id: category.clone(),
                    amount: "300.00".into(),
                    notes: None,
                }],
            },
        )
        .unwrap();

        let budgets = list_budgets(&conn, &tenant, Some("2026-11-14")).unwrap();
        assert_eq!(budgets.len(), 1);
        let budget = &budgets[0];
        assert_eq!(budget.id, budget_id);
        assert_eq!(budget.spent_minor, 25000, "pending entries must not count");
        assert_eq!(budget.remaining_minor, 5000);
        assert_eq!(budget.percent_used, 83);
        assert_eq!(budget.allocations.len(), 1);
        assert_eq!(budget.allocations[0].spent_minor, 25000);
    }
}
