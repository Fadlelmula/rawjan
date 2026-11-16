use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::dates::{self, iso, today_local};
use crate::error::{AppError, AppResult};
use crate::money::{format_amount, parse_amount};
use crate::services::currency::{currency_meta, primary_currency};

const STATUSES: &[&str] = &["active", "achieved", "paused", "archived"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalContributionDto {
    pub id: String,
    pub amount_minor: i64,
    pub amount: String,
    pub contributed_on: String,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub transaction_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalDto {
    pub id: String,
    pub name: String,
    pub target_amount_minor: i64,
    pub target_amount: String,
    pub currency: String,
    pub currency_symbol: String,
    pub target_date: Option<String>,
    pub priority: i64,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub monthly_contribution_minor: i64,
    pub monthly_contribution: String,
    pub start_date: String,
    pub status: String,
    pub notes: Option<String>,
    pub contributed_minor: i64,
    pub contributed: String,
    pub remaining_minor: i64,
    pub remaining: String,
    pub percent_used: i64,
    pub achieved: bool,
    pub months_remaining: Option<i64>,
    pub required_monthly_minor: Option<i64>,
    pub required_monthly: Option<String>,
    pub contributions: Vec<GoalContributionDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalInput {
    pub name: String,
    pub target_amount: String,
    pub currency_code: Option<String>,
    pub target_date: Option<String>,
    pub priority: Option<i64>,
    pub account_id: Option<String>,
    pub monthly_contribution: Option<String>,
    pub start_date: Option<String>,
    pub status: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionInput {
    pub amount: String,
    pub contributed_on: String,
    pub account_id: Option<String>,
    pub note: Option<String>,
}

fn parse_optional_date(value: Option<&str>) -> AppResult<Option<String>> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(raw) => {
            dates::parse_date(raw)?;
            Ok(Some(raw.to_string()))
        }
        None => Ok(None),
    }
}

fn validate_account(conn: &Connection, tenant_id: &str, account_id: Option<&str>) -> AppResult<()> {
    let Some(account_id) = account_id.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(());
    };
    let found: Option<String> = conn
        .query_row(
            "SELECT id FROM accounts WHERE id = ?1 AND tenant_id = ?2",
            params![account_id, tenant_id],
            |r| r.get(0),
        )
        .optional()?;
    if found.is_none() {
        return Err(AppError::NotFound("Linked account not found.".into()));
    }
    Ok(())
}

fn validate_status(status: &str) -> AppResult<()> {
    if STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(AppError::Validation("Unknown goal status.".into()))
    }
}

/// Whole calendar months from `reference` to `target`, floored at zero.
/// Used only for an estimated required monthly contribution, never for ledger math.
fn months_between(reference: NaiveDate, target: NaiveDate) -> i64 {
    if target <= reference {
        return 0;
    }
    let mut months = (target.year() - reference.year()) as i64 * 12 + target.month() as i64
        - reference.month() as i64;
    if target.day() < reference.day() {
        months -= 1;
    }
    months.max(0)
}

fn required_monthly(remaining: i64, months: Option<i64>) -> Option<i64> {
    let months = months?;
    if remaining <= 0 {
        return Some(0);
    }
    if months <= 0 {
        return Some(remaining);
    }
    Some((((remaining as i128) + months as i128 - 1) / months as i128) as i64)
}

fn contribution_rows(
    conn: &Connection,
    tenant_id: &str,
    goal_id: &str,
    exponent: u32,
) -> AppResult<Vec<GoalContributionDto>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.amount_minor, c.contributed_on, c.account_id, a.name, c.transaction_id, c.note
         FROM goal_contributions c
         LEFT JOIN accounts a ON a.id = c.account_id
         WHERE c.tenant_id = ?1 AND c.goal_id = ?2
         ORDER BY c.contributed_on DESC, c.created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![tenant_id, goal_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows
        .into_iter()
        .map(
            |(id, amount_minor, contributed_on, account_id, account_name, transaction_id, note)| {
                GoalContributionDto {
                    id,
                    amount_minor,
                    amount: format_amount(amount_minor, exponent),
                    contributed_on,
                    account_id,
                    account_name,
                    transaction_id,
                    note,
                }
            },
        )
        .collect())
}

pub fn list_goals(
    conn: &Connection,
    tenant_id: &str,
    reference: Option<&str>,
) -> AppResult<Vec<GoalDto>> {
    let reference_date = match reference {
        Some(value) if !value.trim().is_empty() => dates::parse_date(value)?,
        _ => today_local(),
    };

    let mut totals_stmt = conn.prepare(
        "SELECT goal_id, SUM(amount_minor) FROM goal_contributions WHERE tenant_id = ?1 GROUP BY goal_id",
    )?;
    let mut totals: HashMap<String, i64> = HashMap::new();
    for row in totals_stmt.query_map([tenant_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })? {
        let (goal_id, amount) = row?;
        totals.insert(goal_id, amount);
    }

    let mut stmt = conn.prepare(
        "SELECT g.id, g.name, g.target_amount_minor, g.currency_code, cur.symbol, g.target_date, g.priority,
                g.account_id, a.name, g.monthly_contribution_minor, g.start_date, g.status, g.notes
         FROM goals g
         JOIN currencies cur ON cur.code = g.currency_code
         LEFT JOIN accounts a ON a.id = g.account_id
         WHERE g.tenant_id = ?1
         ORDER BY CASE g.status
                    WHEN 'active' THEN 0
                    WHEN 'paused' THEN 1
                    WHEN 'achieved' THEN 2
                    ELSE 3
                  END,
                  g.priority, g.name",
    )?;
    let raw = stmt
        .query_map([tenant_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, Option<String>>(12)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut goals = Vec::with_capacity(raw.len());
    for (
        id,
        name,
        target_amount_minor,
        currency,
        currency_symbol,
        target_date,
        priority,
        account_id,
        account_name,
        monthly_contribution_minor,
        start_date,
        status,
        notes,
    ) in raw
    {
        let exponent = currency_meta(conn, &currency)?.exponent;
        let contributed_minor = *totals.get(&id).unwrap_or(&0);
        let remaining_minor = target_amount_minor - contributed_minor;
        let percent_used = if target_amount_minor > 0 {
            ((contributed_minor as i128 * 100) / target_amount_minor as i128) as i64
        } else {
            0
        };
        let months_remaining = match target_date.as_deref() {
            Some(value) => Some(months_between(reference_date, dates::parse_date(value)?)),
            None => None,
        };
        let required_monthly_minor = required_monthly(remaining_minor, months_remaining);
        let contributions = contribution_rows(conn, tenant_id, &id, exponent)?;

        goals.push(GoalDto {
            id,
            name,
            target_amount_minor,
            target_amount: format_amount(target_amount_minor, exponent),
            currency,
            currency_symbol,
            target_date,
            priority,
            account_id,
            account_name,
            monthly_contribution_minor,
            monthly_contribution: format_amount(monthly_contribution_minor, exponent),
            start_date,
            status,
            notes,
            contributed_minor,
            contributed: format_amount(contributed_minor, exponent),
            remaining_minor,
            remaining: format_amount(remaining_minor, exponent),
            percent_used,
            achieved: contributed_minor >= target_amount_minor,
            months_remaining,
            required_monthly_minor,
            required_monthly: required_monthly_minor.map(|v| format_amount(v, exponent)),
            contributions,
        });
    }
    Ok(goals)
}

pub fn save_goal(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: Option<&str>,
    input: &GoalInput,
) -> AppResult<String> {
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("Goal name is required.".into()));
    }
    let currency_code = input
        .currency_code
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(primary_currency(conn, tenant_id)?);
    let meta = currency_meta(conn, &currency_code)?;
    let target_minor = parse_amount(&input.target_amount, meta.exponent)?;
    if target_minor <= 0 {
        return Err(AppError::Validation(
            "Target amount must be greater than zero.".into(),
        ));
    }
    let monthly_minor = match input.monthly_contribution.as_deref() {
        Some(value) if !value.trim().is_empty() => {
            let parsed = parse_amount(value, meta.exponent)?;
            if parsed < 0 {
                return Err(AppError::Validation(
                    "Planned monthly contribution cannot be negative.".into(),
                ));
            }
            parsed
        }
        _ => 0,
    };
    let priority = input.priority.unwrap_or(3);
    if !(1..=5).contains(&priority) {
        return Err(AppError::Validation(
            "Priority must be between 1 and 5.".into(),
        ));
    }
    let status = input.status.as_deref().unwrap_or("active");
    validate_status(status)?;
    // A target date in the past is allowed; the UI reports it as overdue instead of rewriting it.
    let target_date = parse_optional_date(input.target_date.as_deref())?;
    let start_date = match input
        .start_date
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(value) => {
            dates::parse_date(value)?;
            value.to_string()
        }
        None => iso(today_local()),
    };
    validate_account(conn, tenant_id, input.account_id.as_deref())?;

    let now = dates::now_iso();
    let goal_id = match id.filter(|v| !v.is_empty()) {
        Some(existing) => {
            let existing_currency: Option<String> = conn
                .query_row(
                    "SELECT currency_code FROM goals WHERE id = ?1 AND tenant_id = ?2",
                    params![existing, tenant_id],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(existing_currency) = existing_currency else {
                return Err(AppError::NotFound("Goal not found.".into()));
            };
            if existing_currency != currency_code {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM goal_contributions WHERE goal_id = ?1 AND tenant_id = ?2",
                    params![existing, tenant_id],
                    |r| r.get(0),
                )?;
                if count > 0 {
                    return Err(AppError::Validation(
                        "This goal already has contributions in its original currency. Create a new goal instead of changing the currency."
                            .into(),
                    ));
                }
            }
            conn.execute(
                "UPDATE goals SET name = ?1, target_amount_minor = ?2, currency_code = ?3, target_date = ?4,
                    priority = ?5, account_id = ?6, monthly_contribution_minor = ?7, start_date = ?8,
                    status = ?9, notes = ?10, updated_at = ?11
                 WHERE id = ?12 AND tenant_id = ?13",
                params![
                    input.name.trim(),
                    target_minor,
                    currency_code,
                    target_date,
                    priority,
                    input.account_id.as_deref().filter(|v| !v.trim().is_empty()),
                    monthly_minor,
                    start_date,
                    status,
                    input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    now,
                    existing,
                    tenant_id
                ],
            )?;
            existing.to_string()
        }
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO goals (id, tenant_id, user_id, name, target_amount_minor, currency_code, target_date,
                    priority, account_id, monthly_contribution_minor, start_date, status, notes, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
                params![
                    new_id,
                    tenant_id,
                    user_id,
                    input.name.trim(),
                    target_minor,
                    currency_code,
                    target_date,
                    priority,
                    input.account_id.as_deref().filter(|v| !v.trim().is_empty()),
                    monthly_minor,
                    start_date,
                    status,
                    input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    now
                ],
            )?;
            new_id
        }
    };

    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "goal",
        &goal_id,
        if id.is_some() { "update" } else { "create" },
        None,
        Some(&serde_json::json!({
            "name": input.name.trim(),
            "targetAmountMinor": target_minor,
            "currency": currency_code,
            "status": status
        })),
    )?;
    Ok(goal_id)
}

pub fn add_contribution(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    goal_id: &str,
    input: &ContributionInput,
) -> AppResult<String> {
    let currency: Option<String> = conn
        .query_row(
            "SELECT currency_code FROM goals WHERE id = ?1 AND tenant_id = ?2",
            params![goal_id, tenant_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(currency) = currency else {
        return Err(AppError::NotFound("Goal not found.".into()));
    };
    let status: String = conn.query_row(
        "SELECT status FROM goals WHERE id = ?1 AND tenant_id = ?2",
        params![goal_id, tenant_id],
        |r| r.get(0),
    )?;
    if status == "archived" {
        return Err(AppError::Validation(
            "This goal is archived. Restore it before adding contributions.".into(),
        ));
    }
    let exponent = currency_meta(conn, &currency)?.exponent;
    let amount_minor = parse_amount(&input.amount, exponent)?;
    if amount_minor <= 0 {
        return Err(AppError::Validation(
            "A contribution must be greater than zero.".into(),
        ));
    }
    dates::parse_date(&input.contributed_on)?;
    validate_account(conn, tenant_id, input.account_id.as_deref())?;

    let contribution_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO goal_contributions (id, tenant_id, goal_id, amount_minor, contributed_on, account_id, transaction_id, note, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)",
        params![
            contribution_id,
            tenant_id,
            goal_id,
            amount_minor,
            input.contributed_on,
            input.account_id.as_deref().filter(|v| !v.trim().is_empty()),
            input.note.as_deref().map(str::trim).filter(|v| !v.is_empty()),
            dates::now_iso()
        ],
    )?;

    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "goal_contribution",
        &contribution_id,
        "create",
        None,
        Some(&serde_json::json!({
            "goalId": goal_id,
            "amountMinor": amount_minor,
            "contributedOn": input.contributed_on
        })),
    )?;
    Ok(contribution_id)
}

pub fn delete_contribution(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    contribution_id: &str,
) -> AppResult<()> {
    let changed = conn.execute(
        "DELETE FROM goal_contributions WHERE id = ?1 AND tenant_id = ?2",
        params![contribution_id, tenant_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Contribution not found.".into()));
    }
    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "goal_contribution",
        contribution_id,
        "delete",
        None,
        None,
    )?;
    Ok(())
}

pub fn set_goal_status(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
    status: &str,
) -> AppResult<()> {
    validate_status(status)?;
    let changed = conn.execute(
        "UPDATE goals SET status = ?1, updated_at = ?2 WHERE id = ?3 AND tenant_id = ?4",
        params![status, dates::now_iso(), id, tenant_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Goal not found.".into()));
    }
    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "goal",
        id,
        "status",
        None,
        Some(&serde_json::json!({ "status": status })),
    )?;
    Ok(())
}

pub fn delete_goal(conn: &Connection, tenant_id: &str, user_id: &str, id: &str) -> AppResult<()> {
    let changed = conn.execute(
        "DELETE FROM goals WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Goal not found.".into()));
    }
    crate::db::audit(conn, tenant_id, user_id, "goal", id, "delete", None, None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, open_test};

    fn setup() -> (Connection, String, String) {
        let mut conn = open_test();
        let (tenant, user) = db::seed_if_empty(&mut conn).unwrap();
        (conn, tenant, user)
    }

    fn goal_input(target: &str, target_date: Option<&str>) -> GoalInput {
        GoalInput {
            name: "Emergency fund".into(),
            target_amount: target.into(),
            currency_code: Some("QAR".into()),
            target_date: target_date.map(str::to_string),
            priority: Some(1),
            account_id: None,
            monthly_contribution: Some("500.00".into()),
            start_date: Some("2026-01-01".into()),
            status: None,
            notes: None,
        }
    }

    #[test]
    fn goal_progress_tracks_contributions() {
        let (conn, tenant, user) = setup();
        let goal_id = save_goal(
            &conn,
            &tenant,
            &user,
            None,
            &goal_input("1000.00", Some("2026-05-14")),
        )
        .unwrap();
        add_contribution(
            &conn,
            &tenant,
            &user,
            &goal_id,
            &ContributionInput {
                amount: "250.00".into(),
                contributed_on: "2026-11-01".into(),
                account_id: None,
                note: None,
            },
        )
        .unwrap();
        add_contribution(
            &conn,
            &tenant,
            &user,
            &goal_id,
            &ContributionInput {
                amount: "100.00".into(),
                contributed_on: "2026-11-10".into(),
                account_id: None,
                note: Some("bonus".into()),
            },
        )
        .unwrap();

        let goals = list_goals(&conn, &tenant, Some("2026-11-14")).unwrap();
        assert_eq!(goals.len(), 1);
        let goal = &goals[0];
        assert_eq!(goal.target_amount_minor, 100_000);
        assert_eq!(goal.contributed_minor, 35_000);
        assert_eq!(goal.remaining_minor, 65_000);
        assert_eq!(goal.percent_used, 35);
        assert!(!goal.achieved);
        assert_eq!(goal.contributions.len(), 2);
        // Target date has passed, so the full remainder is due rather than an invented schedule.
        assert_eq!(goal.months_remaining, Some(0));
        assert_eq!(goal.required_monthly_minor, Some(65_000));
    }

    #[test]
    fn required_monthly_is_an_estimate_from_calendar_months() {
        let (conn, tenant, user) = setup();
        let goal_id = save_goal(
            &conn,
            &tenant,
            &user,
            None,
            &goal_input("600.00", Some("2027-02-14")),
        )
        .unwrap();
        let goals = list_goals(&conn, &tenant, Some("2026-11-14")).unwrap();
        assert_eq!(goals[0].id, goal_id);
        assert_eq!(goals[0].months_remaining, Some(3));
        assert_eq!(goals[0].required_monthly_minor, Some(20_000));
    }

    #[test]
    fn contribution_requires_positive_amount() {
        let (conn, tenant, user) = setup();
        let goal_id = save_goal(&conn, &tenant, &user, None, &goal_input("1000.00", None)).unwrap();
        let error = add_contribution(
            &conn,
            &tenant,
            &user,
            &goal_id,
            &ContributionInput {
                amount: "0.00".into(),
                contributed_on: "2026-11-14".into(),
                account_id: None,
                note: None,
            },
        )
        .unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn goal_currency_cannot_change_after_contributions() {
        let (conn, tenant, user) = setup();
        let goal_id = save_goal(&conn, &tenant, &user, None, &goal_input("1000.00", None)).unwrap();
        add_contribution(
            &conn,
            &tenant,
            &user,
            &goal_id,
            &ContributionInput {
                amount: "100.00".into(),
                contributed_on: "2026-11-14".into(),
                account_id: None,
                note: None,
            },
        )
        .unwrap();

        let mut changed = goal_input("1000.00", None);
        changed.currency_code = Some("USD".into());
        let error = save_goal(&conn, &tenant, &user, Some(&goal_id), &changed).unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn deleting_a_goal_removes_its_contributions() {
        let (conn, tenant, user) = setup();
        let goal_id = save_goal(&conn, &tenant, &user, None, &goal_input("500.00", None)).unwrap();
        add_contribution(
            &conn,
            &tenant,
            &user,
            &goal_id,
            &ContributionInput {
                amount: "50.00".into(),
                contributed_on: "2026-11-14".into(),
                account_id: None,
                note: None,
            },
        )
        .unwrap();
        delete_goal(&conn, &tenant, &user, &goal_id).unwrap();
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM goal_contributions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
    }
}
