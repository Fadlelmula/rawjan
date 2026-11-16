use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::dates::{self, iso, today_local};
use crate::error::{AppError, AppResult};
use crate::money::{format_amount, parse_amount};
use crate::services::currency::{currency_meta, primary_currency};

const TYPES: &[&str] = &[
    "credit_card",
    "loan",
    "installment",
    "money_lent",
    "money_borrowed",
    "other",
];
const STATUSES: &[&str] = &["active", "paid", "defaulted", "archived"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiabilityPaymentDto {
    pub id: String,
    pub paid_on: String,
    pub amount_minor: i64,
    pub amount: String,
    pub principal_minor: i64,
    pub principal: String,
    pub interest_minor: i64,
    pub interest: String,
    pub fee_minor: i64,
    pub fee: String,
    pub transaction_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiabilityDto {
    pub id: String,
    pub name: String,
    pub liability_type: String,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub principal_minor: i64,
    pub principal: String,
    pub outstanding_minor: i64,
    pub outstanding: String,
    pub currency: String,
    pub currency_symbol: String,
    pub interest_rate_bps: i64,
    pub interest_rate: String,
    pub start_date: Option<String>,
    pub term_months: Option<i64>,
    pub payment_minor: i64,
    pub payment: String,
    pub due_day: Option<i64>,
    pub first_payment_date: Option<String>,
    pub status: String,
    pub notes: Option<String>,
    pub principal_repaid_minor: i64,
    pub principal_repaid: String,
    pub percent_repaid: i64,
    pub is_receivable: bool,
    pub next_due_date: Option<String>,
    pub days_until_due: Option<i64>,
    pub overdue: bool,
    pub last_payment_date: Option<String>,
    pub estimated_payoff_months: Option<i64>,
    pub estimated_payoff_date: Option<String>,
    pub payments: Vec<LiabilityPaymentDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiabilityCurrencySummary {
    pub currency: String,
    pub symbol: String,
    pub debt_minor: i64,
    pub debt: String,
    pub receivable_minor: i64,
    pub receivable: String,
    pub monthly_payment_minor: i64,
    pub monthly_payment: String,
    pub active_count: i64,
    pub overdue_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiabilitiesSummary {
    pub total_count: i64,
    pub active_count: i64,
    pub paid_count: i64,
    pub currencies: Vec<LiabilityCurrencySummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiabilityInput {
    pub name: String,
    pub liability_type: String,
    pub account_id: Option<String>,
    pub principal: String,
    pub outstanding: String,
    pub currency_code: Option<String>,
    pub interest_rate: Option<String>,
    pub start_date: Option<String>,
    pub term_months: Option<i64>,
    pub payment: Option<String>,
    pub due_day: Option<i64>,
    pub first_payment_date: Option<String>,
    pub status: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiabilityPaymentInput {
    pub paid_on: String,
    pub amount: String,
    pub principal: Option<String>,
    pub interest: Option<String>,
    pub fee: Option<String>,
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

fn parse_optional_amount(value: Option<&str>, exponent: u32, label: &str) -> AppResult<i64> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(raw) => {
            let parsed = parse_amount(raw, exponent)?;
            if parsed < 0 {
                return Err(AppError::Validation(format!("{label} cannot be negative.")));
            }
            Ok(parsed)
        }
        None => Ok(0),
    }
}

/// Interest rates are entered as a percentage (5.25) and stored as basis points (525).
fn parse_bps(value: Option<&str>) -> AppResult<i64> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        None => Ok(0),
        Some(raw) => {
            let parsed = crate::money::parse_decimal(raw).map_err(|_| {
                AppError::Validation(format!("'{raw}' is not a valid interest rate."))
            })?;
            if parsed < Decimal::ZERO {
                return Err(AppError::Validation(
                    "Interest rate cannot be negative.".into(),
                ));
            }
            let bps = parsed
                .checked_mul(Decimal::from(100))
                .ok_or_else(|| AppError::Validation("Interest rate is too large.".into()))?
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
            bps.try_into()
                .map_err(|_| AppError::Validation("Interest rate is too large.".into()))
        }
    }
}

fn format_bps(bps: i64) -> String {
    (Decimal::from(bps) / Decimal::from(100))
        .round_dp(2)
        .normalize()
        .to_string()
}

fn validate_account(
    conn: &Connection,
    tenant_id: &str,
    account_id: Option<&str>,
    currency_code: &str,
) -> AppResult<()> {
    let Some(account_id) = account_id.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(());
    };
    let account_currency: Option<String> = conn
        .query_row(
            "SELECT currency_code FROM accounts WHERE id = ?1 AND tenant_id = ?2",
            params![account_id, tenant_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(account_currency) = account_currency else {
        return Err(AppError::NotFound("Linked account not found.".into()));
    };
    if account_currency != currency_code {
        return Err(AppError::Validation(format!(
            "The linked account uses {account_currency}, but this liability is in {currency_code}. Use matching currencies so the debt is never double-counted."
        )));
    }
    Ok(())
}

fn validate_status(status: &str) -> AppResult<()> {
    if STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(AppError::Validation("Unknown liability status.".into()))
    }
}

fn next_due_date(due_day: i64, today: NaiveDate) -> NaiveDate {
    let day = due_day.clamp(1, 31) as u32;
    let this_month = dates::clamp_day(today.year(), today.month(), day);
    if this_month >= today {
        this_month
    } else {
        let next_month = dates::month_start(today, 1);
        dates::clamp_day(next_month.year(), next_month.month(), day)
    }
}

fn cycle_start(due_day: i64, today: NaiveDate) -> NaiveDate {
    let day = due_day.clamp(1, 31) as u32;
    let this_month = dates::clamp_day(today.year(), today.month(), day);
    if this_month <= today {
        this_month
    } else {
        let previous = dates::month_start(today, -1);
        dates::clamp_day(previous.year(), previous.month(), day)
    }
}

/// A deterministic payoff estimate in whole months, using Decimal money math and
/// rounded monthly interest. It assumes the rate and payment never change.
fn payoff_estimate(outstanding_minor: i64, payment_minor: i64, rate_bps: i64) -> Option<i64> {
    if outstanding_minor <= 0 {
        return Some(0);
    }
    if payment_minor <= 0 {
        return None;
    }
    let monthly_rate = Decimal::from(rate_bps) / Decimal::from(120_000i64);
    let payment = Decimal::from(payment_minor);
    let mut balance = Decimal::from(outstanding_minor);
    let mut months = 0i64;
    while balance > Decimal::ZERO {
        if months >= 1_200 {
            return None;
        }
        let interest = (balance * monthly_rate)
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
        if payment <= interest {
            return None;
        }
        let due = balance + interest;
        balance = if payment >= due {
            Decimal::ZERO
        } else {
            due - payment
        };
        months += 1;
    }
    Some(months)
}

fn payment_rows(
    conn: &Connection,
    tenant_id: &str,
    liability_id: &str,
    exponent: u32,
) -> AppResult<Vec<LiabilityPaymentDto>> {
    let mut stmt = conn.prepare(
        "SELECT id, paid_on, amount_minor, principal_minor, interest_minor, fee_minor, transaction_id, note
         FROM liability_payments
         WHERE tenant_id = ?1 AND liability_id = ?2
         ORDER BY paid_on DESC, created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![tenant_id, liability_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                paid_on,
                amount_minor,
                principal_minor,
                interest_minor,
                fee_minor,
                transaction_id,
                note,
            )| {
                LiabilityPaymentDto {
                    id,
                    paid_on,
                    amount_minor,
                    amount: format_amount(amount_minor, exponent),
                    principal_minor,
                    principal: format_amount(principal_minor, exponent),
                    interest_minor,
                    interest: format_amount(interest_minor, exponent),
                    fee_minor,
                    fee: format_amount(fee_minor, exponent),
                    transaction_id,
                    note,
                }
            },
        )
        .collect())
}

pub fn list_liabilities(conn: &Connection, tenant_id: &str) -> AppResult<Vec<LiabilityDto>> {
    let today = today_local();

    let mut stmt = conn.prepare(
        "SELECT l.id, l.name, l.type, l.account_id, a.name, l.principal_minor, l.outstanding_minor,
                l.currency_code, cur.symbol, l.interest_rate_bps, l.start_date, l.term_months,
                l.payment_minor, l.due_day, l.first_payment_date, l.status, l.notes
         FROM liabilities l
         JOIN currencies cur ON cur.code = l.currency_code
         LEFT JOIN accounts a ON a.id = l.account_id
         WHERE l.tenant_id = ?1
         ORDER BY CASE l.status
                    WHEN 'active' THEN 0
                    WHEN 'defaulted' THEN 1
                    WHEN 'paid' THEN 2
                    ELSE 3
                  END,
                  l.due_day IS NULL, l.due_day, l.name",
    )?;
    let raw = stmt
        .query_map([tenant_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<i64>>(11)?,
                row.get::<_, i64>(12)?,
                row.get::<_, Option<i64>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, String>(15)?,
                row.get::<_, Option<String>>(16)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut result = Vec::with_capacity(raw.len());
    for (
        id,
        name,
        liability_type,
        account_id,
        account_name,
        principal_minor,
        outstanding_minor,
        currency,
        currency_symbol,
        interest_rate_bps,
        start_date,
        term_months,
        payment_minor,
        due_day,
        first_payment_date,
        status,
        notes,
    ) in raw
    {
        let exp = currency_meta(conn, &currency)?.exponent;
        let payments = payment_rows(conn, tenant_id, &id, exp)?;
        let last_payment_date = payments.first().map(|p| p.paid_on.clone());
        let principal_repaid_minor = (principal_minor - outstanding_minor).max(0);
        let percent_repaid = if principal_minor > 0 {
            ((principal_repaid_minor as i128 * 100) / principal_minor as i128).clamp(0, 100) as i64
        } else {
            0
        };

        let (next_due, days_until, overdue) = match due_day {
            Some(day) if status == "active" && outstanding_minor > 0 => {
                let next = next_due_date(day, today);
                let cycle = cycle_start(day, today);
                let paid_this_cycle = payments.iter().any(|p| {
                    dates::parse_date(&p.paid_on)
                        .map(|d| d >= cycle)
                        .unwrap_or(false)
                });
                let overdue = today > cycle && !paid_this_cycle;
                (Some(iso(next)), Some((next - today).num_days()), overdue)
            }
            _ => (None, None, false),
        };

        let estimated_months = if status == "active" {
            payoff_estimate(outstanding_minor, payment_minor, interest_rate_bps)
        } else {
            None
        };
        let estimated_date = estimated_months.map(|months| {
            if months <= 0 {
                iso(today)
            } else {
                iso(dates::add_months_clamped(today, months as i32))
            }
        });

        result.push(LiabilityDto {
            id,
            name,
            liability_type: liability_type.clone(),
            account_id,
            account_name,
            principal_minor,
            principal: format_amount(principal_minor, exp),
            outstanding_minor,
            outstanding: format_amount(outstanding_minor, exp),
            currency,
            currency_symbol,
            interest_rate_bps,
            interest_rate: format_bps(interest_rate_bps),
            start_date,
            term_months,
            payment_minor,
            payment: format_amount(payment_minor, exp),
            due_day,
            first_payment_date,
            status,
            notes,
            principal_repaid_minor,
            principal_repaid: format_amount(principal_repaid_minor, exp),
            percent_repaid,
            is_receivable: liability_type == "money_lent",
            next_due_date: next_due,
            days_until_due: days_until,
            overdue,
            last_payment_date,
            estimated_payoff_months: estimated_months,
            estimated_payoff_date: estimated_date,
            payments,
        });
    }
    Ok(result)
}

pub fn liabilities_summary(conn: &Connection, tenant_id: &str) -> AppResult<LiabilitiesSummary> {
    let liabilities = list_liabilities(conn, tenant_id)?;

    struct Group {
        symbol: String,
        debt: i64,
        receivable: i64,
        monthly: i64,
        active: i64,
        overdue: i64,
    }

    let mut grouped: HashMap<String, Group> = HashMap::new();
    let mut active_count = 0i64;
    let mut paid_count = 0i64;

    for liability in &liabilities {
        if liability.status == "active" {
            active_count += 1;
        }
        if liability.status == "paid" {
            paid_count += 1;
        }
        if liability.status != "active" {
            continue;
        }
        let group = grouped
            .entry(liability.currency.clone())
            .or_insert_with(|| Group {
                symbol: liability.currency_symbol.clone(),
                debt: 0,
                receivable: 0,
                monthly: 0,
                active: 0,
                overdue: 0,
            });
        if liability.is_receivable {
            group.receivable += liability.outstanding_minor;
        } else {
            group.debt += liability.outstanding_minor;
        }
        group.monthly += liability.payment_minor;
        group.active += 1;
        if liability.overdue {
            group.overdue += 1;
        }
    }

    let mut currencies = Vec::new();
    for (currency, group) in grouped {
        let exp = currency_meta(conn, &currency)?.exponent;
        currencies.push(LiabilityCurrencySummary {
            currency,
            symbol: group.symbol,
            debt_minor: group.debt,
            debt: format_amount(group.debt, exp),
            receivable_minor: group.receivable,
            receivable: format_amount(group.receivable, exp),
            monthly_payment_minor: group.monthly,
            monthly_payment: format_amount(group.monthly, exp),
            active_count: group.active,
            overdue_count: group.overdue,
        });
    }
    currencies.sort_by(|a, b| a.currency.cmp(&b.currency));

    Ok(LiabilitiesSummary {
        total_count: liabilities.len() as i64,
        active_count,
        paid_count,
        currencies,
    })
}

pub fn save_liability(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: Option<&str>,
    input: &LiabilityInput,
) -> AppResult<String> {
    if input.name.trim().is_empty() {
        return Err(AppError::Validation("Liability name is required.".into()));
    }
    if !TYPES.contains(&input.liability_type.as_str()) {
        return Err(AppError::Validation("Unknown liability type.".into()));
    }
    let currency_code = input
        .currency_code
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(primary_currency(conn, tenant_id)?);
    let meta = currency_meta(conn, &currency_code)?;

    let principal_minor = parse_amount(&input.principal, meta.exponent)?;
    if principal_minor < 0 {
        return Err(AppError::Validation("Principal cannot be negative.".into()));
    }
    let outstanding_minor = parse_amount(&input.outstanding, meta.exponent)?;
    if outstanding_minor < 0 {
        return Err(AppError::Validation(
            "Outstanding balance cannot be negative.".into(),
        ));
    }
    let payment_minor =
        parse_optional_amount(input.payment.as_deref(), meta.exponent, "Monthly payment")?;
    let interest_rate_bps = parse_bps(input.interest_rate.as_deref())?;
    let status = input.status.as_deref().unwrap_or("active");
    validate_status(status)?;
    if status == "paid" && outstanding_minor != 0 {
        return Err(AppError::Validation(
            "Set the outstanding balance to zero before marking this liability paid.".into(),
        ));
    }
    if let Some(term) = input.term_months {
        if term <= 0 {
            return Err(AppError::Validation(
                "Term must be a positive number of months.".into(),
            ));
        }
    }
    if let Some(day) = input.due_day {
        if !(1..=31).contains(&day) {
            return Err(AppError::Validation(
                "Due day must be between 1 and 31.".into(),
            ));
        }
    }
    let start_date = parse_optional_date(input.start_date.as_deref())?;
    let first_payment_date = parse_optional_date(input.first_payment_date.as_deref())?;
    validate_account(conn, tenant_id, input.account_id.as_deref(), &currency_code)?;

    let now = dates::now_iso();
    let liability_id = match id.filter(|v| !v.is_empty()) {
        Some(existing) => {
            let existing_currency: Option<String> = conn
                .query_row(
                    "SELECT currency_code FROM liabilities WHERE id = ?1 AND tenant_id = ?2",
                    params![existing, tenant_id],
                    |r| r.get(0),
                )
                .optional()?;
            let Some(existing_currency) = existing_currency else {
                return Err(AppError::NotFound("Liability not found.".into()));
            };
            if existing_currency != currency_code {
                let payments: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM liability_payments WHERE liability_id = ?1 AND tenant_id = ?2",
                    params![existing, tenant_id],
                    |r| r.get(0),
                )?;
                if payments > 0 {
                    return Err(AppError::Validation(
                        "This liability already has payments in its original currency. Create a new liability instead of changing the currency."
                            .into(),
                    ));
                }
            }
            conn.execute(
                "UPDATE liabilities SET name = ?1, type = ?2, account_id = ?3, principal_minor = ?4,
                    outstanding_minor = ?5, currency_code = ?6, interest_rate_bps = ?7, start_date = ?8,
                    term_months = ?9, payment_minor = ?10, due_day = ?11, first_payment_date = ?12,
                    status = ?13, notes = ?14, updated_at = ?15
                 WHERE id = ?16 AND tenant_id = ?17",
                params![
                    input.name.trim(),
                    input.liability_type,
                    input.account_id.as_deref().filter(|v| !v.trim().is_empty()),
                    principal_minor,
                    outstanding_minor,
                    currency_code,
                    interest_rate_bps,
                    start_date,
                    input.term_months,
                    payment_minor,
                    input.due_day,
                    first_payment_date,
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
                "INSERT INTO liabilities (id, tenant_id, user_id, name, type, account_id, principal_minor,
                    outstanding_minor, currency_code, interest_rate_bps, start_date, term_months, payment_minor,
                    due_day, first_payment_date, status, notes, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?18)",
                params![
                    new_id,
                    tenant_id,
                    user_id,
                    input.name.trim(),
                    input.liability_type,
                    input.account_id.as_deref().filter(|v| !v.trim().is_empty()),
                    principal_minor,
                    outstanding_minor,
                    currency_code,
                    interest_rate_bps,
                    start_date,
                    input.term_months,
                    payment_minor,
                    input.due_day,
                    first_payment_date,
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
        "liability",
        &liability_id,
        if id.is_some() { "update" } else { "create" },
        None,
        Some(&serde_json::json!({
            "name": input.name.trim(),
            "type": input.liability_type,
            "principalMinor": principal_minor,
            "outstandingMinor": outstanding_minor,
            "currency": currency_code,
            "status": status
        })),
    )?;
    Ok(liability_id)
}

pub fn save_payment(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    liability_id: &str,
    id: Option<&str>,
    input: &LiabilityPaymentInput,
) -> AppResult<String> {
    let liability: Option<(String, i64, String, String)> = conn
        .query_row(
            "SELECT currency_code, outstanding_minor, status, type FROM liabilities WHERE id = ?1 AND tenant_id = ?2",
            params![liability_id, tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((currency, outstanding_minor, status, _liability_type)) = liability else {
        return Err(AppError::NotFound("Liability not found.".into()));
    };
    if status == "archived" {
        return Err(AppError::Validation(
            "This liability is archived. Restore it before recording payments.".into(),
        ));
    }
    let exp = currency_meta(conn, &currency)?.exponent;
    dates::parse_date(&input.paid_on)?;

    let amount_minor = parse_amount(&input.amount, exp)?;
    if amount_minor <= 0 {
        return Err(AppError::Validation(
            "A payment must be greater than zero.".into(),
        ));
    }
    let interest_minor = parse_optional_amount(input.interest.as_deref(), exp, "Interest")?;
    let fee_minor = parse_optional_amount(input.fee.as_deref(), exp, "Fee")?;
    let principal_minor = match input
        .principal
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        Some(raw) => {
            let parsed = parse_amount(raw, exp)?;
            if parsed < 0 {
                return Err(AppError::Validation(
                    "Principal portion cannot be negative.".into(),
                ));
            }
            parsed
        }
        None => amount_minor - interest_minor - fee_minor,
    };
    if principal_minor < 0 {
        return Err(AppError::Validation(
            "Interest and fees cannot exceed the payment amount.".into(),
        ));
    }
    if principal_minor + interest_minor + fee_minor != amount_minor {
        return Err(AppError::Validation(
            "Principal, interest and fee must add up to the payment amount.".into(),
        ));
    }
    // When editing, the previously recorded principal is already reflected in
    // the outstanding balance and is available to be re-used.
    let previous_principal: i64 = match id.filter(|v| !v.is_empty()) {
        Some(existing) => {
            let row: Option<(i64, i64)> = conn
                .query_row(
                    "SELECT principal_minor, amount_minor FROM liability_payments
                     WHERE id = ?1 AND tenant_id = ?2 AND liability_id = ?3",
                    params![existing, tenant_id, liability_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            row.map(|(principal, _)| principal)
                .ok_or_else(|| AppError::NotFound("Payment not found.".into()))?
        }
        None => 0,
    };
    if principal_minor > outstanding_minor + previous_principal {
        return Err(AppError::Validation(format!(
            "The principal portion ({}) is larger than the outstanding balance ({}).",
            format_amount(principal_minor, exp),
            format_amount(outstanding_minor, exp)
        )));
    }

    let now = dates::now_iso();
    let payment_id = match id.filter(|v| !v.is_empty()) {
        Some(existing) => {
            let previous: Option<(i64, i64)> = conn
                .query_row(
                    "SELECT principal_minor, amount_minor FROM liability_payments
                     WHERE id = ?1 AND tenant_id = ?2 AND liability_id = ?3",
                    params![existing, tenant_id, liability_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((previous_principal, _)) = previous else {
                return Err(AppError::NotFound("Payment not found.".into()));
            };
            let delta = principal_minor - previous_principal;
            let adjusted = outstanding_minor - delta;
            if adjusted < 0 {
                return Err(AppError::Validation(
                    "That change would push the outstanding balance below zero.".into(),
                ));
            }
            conn.execute(
                "UPDATE liability_payments SET paid_on = ?1, amount_minor = ?2, principal_minor = ?3,
                    interest_minor = ?4, fee_minor = ?5, note = ?6
                 WHERE id = ?7 AND tenant_id = ?8",
                params![
                    input.paid_on,
                    amount_minor,
                    principal_minor,
                    interest_minor,
                    fee_minor,
                    input.note.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    existing,
                    tenant_id
                ],
            )?;
            conn.execute(
                "UPDATE liabilities SET outstanding_minor = ?1, status = ?2, updated_at = ?3 WHERE id = ?4 AND tenant_id = ?5",
                params![
                    adjusted,
                    if adjusted == 0 {
                        "paid"
                    } else if status == "paid" {
                        "active"
                    } else {
                        status.as_str()
                    },
                    now,
                    liability_id,
                    tenant_id
                ],
            )?;
            existing.to_string()
        }
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO liability_payments (id, tenant_id, liability_id, paid_on, amount_minor,
                    principal_minor, interest_minor, fee_minor, transaction_id, note, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10)",
                params![
                    new_id,
                    tenant_id,
                    liability_id,
                    input.paid_on,
                    amount_minor,
                    principal_minor,
                    interest_minor,
                    fee_minor,
                    input.note.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    now
                ],
            )?;
            let adjusted = outstanding_minor - principal_minor;
            let next_status = if adjusted == 0 {
                "paid"
            } else {
                status.as_str()
            };
            conn.execute(
                "UPDATE liabilities SET outstanding_minor = ?1, status = ?2, updated_at = ?3
                 WHERE id = ?4 AND tenant_id = ?5",
                params![adjusted, next_status, now, liability_id, tenant_id],
            )?;
            new_id
        }
    };

    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "liability_payment",
        &payment_id,
        if id.is_some() { "update" } else { "create" },
        None,
        Some(&serde_json::json!({
            "liabilityId": liability_id,
            "paidOn": input.paid_on,
            "amountMinor": amount_minor,
            "principalMinor": principal_minor,
            "interestMinor": interest_minor,
            "feeMinor": fee_minor
        })),
    )?;
    Ok(payment_id)
}

pub fn delete_payment(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    payment_id: &str,
) -> AppResult<()> {
    let payment: Option<(String, i64)> = conn
        .query_row(
            "SELECT liability_id, principal_minor FROM liability_payments WHERE id = ?1 AND tenant_id = ?2",
            params![payment_id, tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((liability_id, principal_minor)) = payment else {
        return Err(AppError::NotFound("Payment not found.".into()));
    };

    conn.execute(
        "DELETE FROM liability_payments WHERE id = ?1 AND tenant_id = ?2",
        params![payment_id, tenant_id],
    )?;

    let (outstanding_minor, status): (i64, String) = conn.query_row(
        "SELECT outstanding_minor, status FROM liabilities WHERE id = ?1 AND tenant_id = ?2",
        params![liability_id, tenant_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let restored = outstanding_minor + principal_minor;
    let next_status = if status == "paid" && restored > 0 {
        "active"
    } else {
        status.as_str()
    };
    conn.execute(
        "UPDATE liabilities SET outstanding_minor = ?1, status = ?2, updated_at = ?3
         WHERE id = ?4 AND tenant_id = ?5",
        params![
            restored,
            next_status,
            dates::now_iso(),
            liability_id,
            tenant_id
        ],
    )?;

    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "liability_payment",
        payment_id,
        "delete",
        None,
        Some(&serde_json::json!({ "liabilityId": liability_id })),
    )?;
    Ok(())
}

pub fn set_liability_status(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
    status: &str,
) -> AppResult<()> {
    validate_status(status)?;
    let outstanding: Option<i64> = conn
        .query_row(
            "SELECT outstanding_minor FROM liabilities WHERE id = ?1 AND tenant_id = ?2",
            params![id, tenant_id],
            |r| r.get(0),
        )
        .optional()?;
    let Some(outstanding) = outstanding else {
        return Err(AppError::NotFound("Liability not found.".into()));
    };
    if status == "paid" && outstanding != 0 {
        return Err(AppError::Validation(
            "Set the outstanding balance to zero before marking this liability paid.".into(),
        ));
    }
    conn.execute(
        "UPDATE liabilities SET status = ?1, updated_at = ?2 WHERE id = ?3 AND tenant_id = ?4",
        params![status, dates::now_iso(), id, tenant_id],
    )?;
    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "liability",
        id,
        "status",
        None,
        Some(&serde_json::json!({ "status": status })),
    )?;
    Ok(())
}

pub fn delete_liability(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
) -> AppResult<()> {
    let changed = conn.execute(
        "DELETE FROM liabilities WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Liability not found.".into()));
    }
    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "liability",
        id,
        "delete",
        None,
        None,
    )?;
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

    fn liability_input(liability_type: &str, principal: &str, outstanding: &str) -> LiabilityInput {
        LiabilityInput {
            name: "Car loan".into(),
            liability_type: liability_type.into(),
            account_id: None,
            principal: principal.into(),
            outstanding: outstanding.into(),
            currency_code: Some("QAR".into()),
            interest_rate: Some("5.00".into()),
            start_date: Some("2026-01-01".into()),
            term_months: Some(36),
            payment: Some("500.00".into()),
            due_day: Some(28),
            first_payment_date: Some("2026-02-28".into()),
            status: None,
            notes: None,
        }
    }

    #[test]
    fn liability_progress_tracks_principal_repaid() {
        let (conn, tenant, user) = setup();
        let liability_id = save_liability(
            &conn,
            &tenant,
            &user,
            None,
            &liability_input("loan", "10000.00", "7500.00"),
        )
        .unwrap();

        let liabilities = list_liabilities(&conn, &tenant).unwrap();
        assert_eq!(liabilities.len(), 1);
        let item = &liabilities[0];
        assert_eq!(item.id, liability_id);
        assert_eq!(item.principal_minor, 1_000_000);
        assert_eq!(item.outstanding_minor, 750_000);
        assert_eq!(item.principal_repaid_minor, 250_000);
        assert_eq!(item.percent_repaid, 25);
        assert_eq!(item.interest_rate, "5");
        assert!(!item.is_receivable);
    }

    #[test]
    fn payment_reduces_outstanding_and_marks_paid_at_zero() {
        let (conn, tenant, user) = setup();
        let liability_id = save_liability(
            &conn,
            &tenant,
            &user,
            None,
            &liability_input("loan", "1000.00", "1000.00"),
        )
        .unwrap();

        save_payment(
            &conn,
            &tenant,
            &user,
            &liability_id,
            None,
            &LiabilityPaymentInput {
                paid_on: "2026-11-01".into(),
                amount: "400.00".into(),
                principal: Some("300.00".into()),
                interest: Some("100.00".into()),
                fee: None,
                note: None,
            },
        )
        .unwrap();

        let item = &list_liabilities(&conn, &tenant).unwrap()[0];
        assert_eq!(item.outstanding_minor, 70_000);
        assert_eq!(item.payments.len(), 1);
        assert_eq!(item.payments[0].principal_minor, 30_000);
        assert_eq!(item.payments[0].interest_minor, 10_000);
        assert_eq!(item.status, "active");

        save_payment(
            &conn,
            &tenant,
            &user,
            &liability_id,
            None,
            &LiabilityPaymentInput {
                paid_on: "2026-11-14".into(),
                amount: "700.00".into(),
                principal: Some("700.00".into()),
                interest: None,
                fee: None,
                note: None,
            },
        )
        .unwrap();

        let item = &list_liabilities(&conn, &tenant).unwrap()[0];
        assert_eq!(item.outstanding_minor, 0);
        assert_eq!(item.status, "paid");
    }

    #[test]
    fn editing_a_payment_keeps_the_status_in_sync() {
        let (conn, tenant, user) = setup();
        let liability_id = save_liability(
            &conn,
            &tenant,
            &user,
            None,
            &liability_input("loan", "1000.00", "1000.00"),
        )
        .unwrap();
        let payment_id = save_payment(
            &conn,
            &tenant,
            &user,
            &liability_id,
            None,
            &LiabilityPaymentInput {
                paid_on: "2026-11-01".into(),
                amount: "1000.00".into(),
                principal: Some("1000.00".into()),
                interest: None,
                fee: None,
                note: None,
            },
        )
        .unwrap();
        let item = &list_liabilities(&conn, &tenant).unwrap()[0];
        assert_eq!(item.status, "paid");

        // Editing the payment down reopens the liability.
        save_payment(
            &conn,
            &tenant,
            &user,
            &liability_id,
            Some(&payment_id),
            &LiabilityPaymentInput {
                paid_on: "2026-11-01".into(),
                amount: "400.00".into(),
                principal: Some("400.00".into()),
                interest: None,
                fee: None,
                note: None,
            },
        )
        .unwrap();
        let item = &list_liabilities(&conn, &tenant).unwrap()[0];
        assert_eq!(item.outstanding_minor, 60_000);
        assert_eq!(
            item.status, "active",
            "a reopened balance must not stay 'paid'"
        );

        // Editing it back up marks it paid again.
        save_payment(
            &conn,
            &tenant,
            &user,
            &liability_id,
            Some(&payment_id),
            &LiabilityPaymentInput {
                paid_on: "2026-11-01".into(),
                amount: "1000.00".into(),
                principal: Some("1000.00".into()),
                interest: None,
                fee: None,
                note: None,
            },
        )
        .unwrap();
        let item = &list_liabilities(&conn, &tenant).unwrap()[0];
        assert_eq!(item.outstanding_minor, 0);
        assert_eq!(item.status, "paid");
    }

    #[test]
    fn payment_breakdown_must_add_up() {
        let (conn, tenant, user) = setup();
        let liability_id = save_liability(
            &conn,
            &tenant,
            &user,
            None,
            &liability_input("loan", "1000.00", "1000.00"),
        )
        .unwrap();
        let error = save_payment(
            &conn,
            &tenant,
            &user,
            &liability_id,
            None,
            &LiabilityPaymentInput {
                paid_on: "2026-11-01".into(),
                amount: "400.00".into(),
                principal: Some("300.00".into()),
                interest: Some("50.00".into()),
                fee: None,
                note: None,
            },
        )
        .unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn deleting_a_payment_restores_outstanding_and_status() {
        let (conn, tenant, user) = setup();
        let liability_id = save_liability(
            &conn,
            &tenant,
            &user,
            None,
            &liability_input("loan", "500.00", "500.00"),
        )
        .unwrap();
        let payment_id = save_payment(
            &conn,
            &tenant,
            &user,
            &liability_id,
            None,
            &LiabilityPaymentInput {
                paid_on: "2026-11-01".into(),
                amount: "500.00".into(),
                principal: Some("500.00".into()),
                interest: None,
                fee: None,
                note: None,
            },
        )
        .unwrap();

        let item = &list_liabilities(&conn, &tenant).unwrap()[0];
        assert_eq!(item.status, "paid");

        delete_payment(&conn, &tenant, &user, &payment_id).unwrap();
        let item = &list_liabilities(&conn, &tenant).unwrap()[0];
        assert_eq!(item.outstanding_minor, 50_000);
        assert_eq!(item.status, "active");
        assert!(item.payments.is_empty());
    }

    #[test]
    fn money_lent_is_flagged_as_receivable() {
        let (conn, tenant, user) = setup();
        save_liability(
            &conn,
            &tenant,
            &user,
            None,
            &liability_input("money_lent", "2000.00", "1500.00"),
        )
        .unwrap();
        let item = &list_liabilities(&conn, &tenant).unwrap()[0];
        assert!(item.is_receivable);
        let summary = liabilities_summary(&conn, &tenant).unwrap();
        assert_eq!(summary.currencies[0].receivable_minor, 150_000);
        assert_eq!(summary.currencies[0].debt_minor, 0);
    }

    #[test]
    fn payoff_estimate_uses_fixed_rate_and_payment() {
        // 1000 at 12% annual (1% monthly), paying 100/month: deterministic amortization.
        assert_eq!(payoff_estimate(100_000, 10_000, 1_200), Some(11));
        // A payment that only covers interest never pays the debt off.
        assert_eq!(payoff_estimate(100_000, 1_000, 1_200), None);
        // Zero interest: straight division, rounded up.
        assert_eq!(payoff_estimate(100_000, 30_000, 0), Some(4));
    }

    #[test]
    fn paid_status_requires_zero_outstanding() {
        let (conn, tenant, user) = setup();
        let mut input = liability_input("loan", "1000.00", "500.00");
        input.status = Some("paid".into());
        let error = save_liability(&conn, &tenant, &user, None, &input).unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }
}
