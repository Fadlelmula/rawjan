//! Income reporting.
//!
//! Every number here comes from posted or reconciled ledger records. Expected
//! amounts are taken from the saved income-source records and are only turned
//! into a period estimate when the schedule and currency make an honest
//! comparison possible. Nothing is converted, extrapolated or invented.

use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::dates::{self, iso};
use crate::error::{AppError, AppResult};
use crate::money::format_amount;
use crate::services::analytics::{category_slices, month_label, monthly_totals, MonthPoint};
use crate::services::currency::{currency_meta, exponent, primary_currency};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeSummary {
    pub income_minor: i64,
    pub income: String,
    pub previous_income_minor: i64,
    pub previous_income: String,
    /// Percentage change against the immediately preceding range of the same
    /// length. `None` when the previous range had no income to compare with.
    pub change_percent: Option<i64>,
    pub average_monthly_minor: i64,
    pub average_monthly: String,
    pub expense_minor: i64,
    pub expense: String,
    pub net_minor: i64,
    pub net: String,
    pub savings_rate: Option<i64>,
    pub transaction_count: i64,
    pub month_count: i64,
    pub unassigned_minor: i64,
    pub unassigned: String,
    pub unassigned_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeSourceReport {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub currency: String,
    pub schedule: String,
    pub pay_day: Option<i64>,
    pub account_id: Option<String>,
    pub account_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub is_active: bool,
    pub notes: Option<String>,
    /// Expected amount for one occurrence, in the source currency.
    pub expected_amount_minor: i64,
    pub expected_amount: String,
    /// Expected total for the reporting range, in the primary currency.
    /// `None` when no honest estimate is possible.
    pub expected_minor: Option<i64>,
    pub expected: Option<String>,
    pub received_minor: i64,
    pub received: String,
    pub variance_minor: Option<i64>,
    pub variance: Option<String>,
    pub occurrences: Option<i64>,
    pub transaction_count: i64,
    pub last_received_date: Option<String>,
    pub estimate_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeAccountRow {
    pub account_id: String,
    pub name: String,
    pub currency: String,
    pub income_minor: i64,
    pub income: String,
    pub transaction_count: i64,
    pub percent: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IncomeReport {
    pub from: String,
    pub to: String,
    pub currency: String,
    pub currency_symbol: String,
    pub summary: IncomeSummary,
    pub months: Vec<MonthPoint>,
    pub sources: Vec<IncomeSourceReport>,
    pub accounts: Vec<IncomeAccountRow>,
    pub income_categories: Vec<crate::services::analytics::CategorySlice>,
}

fn percent(part: i64, total: i64) -> i64 {
    if total <= 0 {
        return 0;
    }
    ((part.max(0) as i128 * 100) / total as i128) as i64
}

/// How many expected payouts fall inside the range.
///
/// Only monthly, semimonthly and one-time sources are counted. Weekly,
/// biweekly and irregular sources have no stored anchor date, so any number
/// would be a guess and is therefore withheld.
fn expected_occurrences(
    schedule: &str,
    pay_day: Option<i64>,
    start: NaiveDate,
    end: NaiveDate,
    received_all_time: i64,
) -> Option<i64> {
    match schedule {
        "monthly" => {
            let mut count = 0i64;
            for (month_start, _) in dates::each_month(start, end) {
                match pay_day {
                    Some(day) => {
                        let pay = dates::clamp_day(month_start.year(), month_start.month(), day as u32);
                        if pay >= start && pay <= end {
                            count += 1;
                        }
                    }
                    None => count += 1,
                }
            }
            Some(count)
        }
        "semimonthly" => Some(dates::each_month(start, end).len() as i64 * 2),
        "one_time" => Some(if received_all_time > 0 { 0 } else { 1 }),
        _ => None,
    }
}

/// The comparison range immediately before `start`.
///
/// Calendar-aligned ranges (first of a month to the last of a month) shift back
/// by whole months, so "this month" is compared with "last month". Anything
/// else shifts back by the same number of days.
fn previous_range(start: NaiveDate, end: NaiveDate) -> (NaiveDate, NaiveDate) {
    let previous_end = start - Duration::days(1);
    let month_aligned = start.day() == 1 && end == dates::calendar_month(end).1;
    if month_aligned {
        let months = dates::each_month(start, end).len() as i32;
        (dates::add_months_clamped(start, -months), previous_end)
    } else {
        let span_days = (end - start).num_days();
        (previous_end - Duration::days(span_days), previous_end)
    }
}

fn period_income(
    conn: &Connection,
    tenant_id: &str,
    start: &str,
    end: &str,
) -> AppResult<(i64, i64)> {
    let row = conn.query_row(
        "SELECT COALESCE(SUM(base_amount_minor), 0), COUNT(*)
         FROM transactions
         WHERE tenant_id = ?1
           AND kind IN ('income','dividend')
           AND status IN ('posted','reconciled')
           AND reversal_of_id IS NULL
           AND local_date BETWEEN ?2 AND ?3",
        params![tenant_id, start, end],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    Ok(row)
}

fn unassigned_income(
    conn: &Connection,
    tenant_id: &str,
    start: &str,
    end: &str,
) -> AppResult<(i64, i64)> {
    let row = conn.query_row(
        "SELECT COALESCE(SUM(base_amount_minor), 0), COUNT(*)
         FROM transactions
         WHERE tenant_id = ?1
           AND kind IN ('income','dividend')
           AND income_source_id IS NULL
           AND status IN ('posted','reconciled')
           AND reversal_of_id IS NULL
           AND local_date BETWEEN ?2 AND ?3",
        params![tenant_id, start, end],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    Ok(row)
}

fn source_rows(
    conn: &Connection,
    tenant_id: &str,
    start: &str,
    end: &str,
    base: &str,
    base_exp: u32,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> AppResult<Vec<IncomeSourceReport>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.type, s.currency_code, s.expected_amount_minor, s.schedule, s.pay_day,
                s.account_id, a.name, s.category_id, c.name, s.is_active,
                COALESCE((SELECT SUM(t.base_amount_minor) FROM transactions t
                   WHERE t.income_source_id = s.id AND t.tenant_id = s.tenant_id
                     AND t.kind IN ('income','dividend')
                     AND t.status IN ('posted','reconciled') AND t.reversal_of_id IS NULL
                     AND t.local_date BETWEEN ?2 AND ?3), 0),
                (SELECT COUNT(*) FROM transactions t
                   WHERE t.income_source_id = s.id AND t.tenant_id = s.tenant_id
                     AND t.kind IN ('income','dividend')
                     AND t.status IN ('posted','reconciled') AND t.reversal_of_id IS NULL
                     AND t.local_date BETWEEN ?2 AND ?3),
                (SELECT max(t.local_date) FROM transactions t
                   WHERE t.income_source_id = s.id AND t.tenant_id = s.tenant_id
                     AND t.kind IN ('income','dividend')
                     AND t.status IN ('posted','reconciled') AND t.reversal_of_id IS NULL),
                (SELECT COUNT(*) FROM transactions t
                   WHERE t.income_source_id = s.id AND t.tenant_id = s.tenant_id
                     AND t.kind IN ('income','dividend')
                     AND t.status IN ('posted','reconciled') AND t.reversal_of_id IS NULL),
                s.notes
         FROM income_sources s
         LEFT JOIN accounts a ON a.id = s.account_id
         LEFT JOIN categories c ON c.id = s.category_id
         WHERE s.tenant_id = ?1
         ORDER BY s.is_active DESC, s.name",
    )?;
    let rows = stmt.query_map(params![tenant_id, start, end], |row| {
        let currency: String = row.get(3)?;
        let expected_amount_minor: i64 = row.get(4)?;
        let schedule: String = row.get(5)?;
        let pay_day: Option<i64> = row.get(6)?;
        let received_minor: i64 = row.get(12)?;
        let transaction_count: i64 = row.get(13)?;
        let last_received_date: Option<String> = row.get(14)?;
        let received_all_time: i64 = row.get(15)?;
        let source_exp = exponent(conn, &currency).unwrap_or(2);

        let occurrences = expected_occurrences(
            &schedule,
            pay_day,
            start_date,
            end_date,
            received_all_time,
        );
        let (expected_minor, estimate_note) = if currency != base {
            (
                None,
                Some(format!(
                    "Expected amounts are saved in {currency} and are not converted, so no variance is shown."
                )),
            )
        } else {
            match occurrences {
                Some(count) if expected_amount_minor > 0 => {
                    match expected_amount_minor.checked_mul(count) {
                        Some(total) => (Some(total), None),
                        None => (
                            None,
                            Some("The expected amount is too large to total.".to_string()),
                        ),
                    }
                }
                Some(_) => (
                    None,
                    Some("No expected amount is saved for this source.".to_string()),
                ),
                None => (
                    None,
                    Some(
                        "Rawjan does not estimate weekly, biweekly or irregular income. Recorded amounts are shown exactly."
                            .to_string(),
                    ),
                ),
            }
        };
        let variance_minor = match expected_minor {
            Some(expected) if expected > 0 => Some(received_minor - expected),
            _ => None,
        };

        Ok(IncomeSourceReport {
            id: row.get(0)?,
            name: row.get(1)?,
            source_type: row.get(2)?,
            currency,
            schedule,
            pay_day,
            account_id: row.get(7)?,
            account_name: row.get(8)?,
            category_id: row.get(9)?,
            category_name: row.get(10)?,
            is_active: row.get::<_, i64>(11)? == 1,
            notes: row.get(16)?,
            expected_amount_minor,
            expected_amount: format_amount(expected_amount_minor, source_exp),
            expected_minor,
            expected: expected_minor.map(|value| format_amount(value, base_exp)),
            received_minor,
            received: format_amount(received_minor, base_exp),
            variance_minor,
            variance: variance_minor.map(|value| format_amount(value, base_exp)),
            occurrences,
            transaction_count,
            last_received_date,
            estimate_note,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn account_rows(
    conn: &Connection,
    tenant_id: &str,
    start: &str,
    end: &str,
    total: i64,
    base_exp: u32,
) -> AppResult<Vec<IncomeAccountRow>> {
    let mut stmt = conn.prepare(
        "SELECT t.account_id, a.name, a.currency_code, SUM(t.base_amount_minor), COUNT(*)
         FROM transactions t
         JOIN accounts a ON a.id = t.account_id
         WHERE t.tenant_id = ?1
           AND t.kind IN ('income','dividend')
           AND t.status IN ('posted','reconciled')
           AND t.reversal_of_id IS NULL
           AND t.local_date BETWEEN ?2 AND ?3
         GROUP BY t.account_id
         ORDER BY SUM(t.base_amount_minor) DESC",
    )?;
    let rows = stmt.query_map(params![tenant_id, start, end], |row| {
        let income_minor: i64 = row.get(3)?;
        Ok(IncomeAccountRow {
            account_id: row.get(0)?,
            name: row.get(1)?,
            currency: row.get(2)?,
            income_minor,
            income: format_amount(income_minor, base_exp),
            transaction_count: row.get(4)?,
            percent: percent(income_minor, total),
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn income_report(
    conn: &Connection,
    tenant_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> AppResult<IncomeReport> {
    let today = dates::today_local();
    let default_from = dates::month_start(today, -5);
    let start = match from.filter(|value| !value.trim().is_empty()) {
        Some(value) => dates::parse_date(value)?,
        None => default_from,
    };
    let end = match to.filter(|value| !value.trim().is_empty()) {
        Some(value) => dates::parse_date(value)?,
        None => today,
    };
    if start > end {
        return Err(AppError::Validation(
            "The reporting range starts after it ends.".into(),
        ));
    }

    let currency = primary_currency(conn, tenant_id)?;
    let meta = currency_meta(conn, &currency)?;
    let start_iso = iso(start);
    let end_iso = iso(end);

    let totals = monthly_totals(conn, tenant_id, &start_iso, &end_iso)?;
    let mut months = Vec::new();
    for (month_start, _) in dates::each_month(start, end) {
        let key = dates::month_key(month_start);
        let entry = totals.get(&key);
        let income = entry.map(|value| value.income).unwrap_or(0);
        let expense = entry.map(|value| value.expense).unwrap_or(0);
        let count = entry.map(|value| value.count).unwrap_or(0);
        let net = income - expense;
        months.push(MonthPoint {
            month: key.clone(),
            label: month_label(&key),
            income_minor: income,
            income: format_amount(income, meta.exponent),
            expense_minor: expense,
            expense: format_amount(expense, meta.exponent),
            net_minor: net,
            net: format_amount(net, meta.exponent),
            transaction_count: count,
        });
    }

    let income_total: i64 = months.iter().map(|month| month.income_minor).sum();
    let expense_total: i64 = months.iter().map(|month| month.expense_minor).sum();
    let net_total = income_total - expense_total;
    let transaction_count: i64 = months.iter().map(|month| month.transaction_count).sum();
    let month_count = months.len().max(1) as i64;
    let average_monthly = income_total / month_count;
    let savings_rate = if income_total > 0 {
        Some(((net_total as i128 * 100) / income_total as i128) as i64)
    } else {
        None
    };

    let (previous_start, previous_end) = previous_range(start, end);
    let (previous_income, _) = period_income(
        conn,
        tenant_id,
        &iso(previous_start),
        &iso(previous_end),
    )?;
    let change_percent = if previous_income > 0 {
        Some(
            (((income_total as i128 - previous_income as i128) * 100) / previous_income as i128)
                as i64,
        )
    } else {
        None
    };

    let (unassigned_minor, unassigned_count) =
        unassigned_income(conn, tenant_id, &start_iso, &end_iso)?;

    Ok(IncomeReport {
        from: start_iso.clone(),
        to: end_iso.clone(),
        currency: currency.clone(),
        currency_symbol: meta.symbol,
        summary: IncomeSummary {
            income_minor: income_total,
            income: format_amount(income_total, meta.exponent),
            previous_income_minor: previous_income,
            previous_income: format_amount(previous_income, meta.exponent),
            change_percent,
            average_monthly_minor: average_monthly,
            average_monthly: format_amount(average_monthly, meta.exponent),
            expense_minor: expense_total,
            expense: format_amount(expense_total, meta.exponent),
            net_minor: net_total,
            net: format_amount(net_total, meta.exponent),
            savings_rate,
            transaction_count,
            month_count: months.len() as i64,
            unassigned_minor,
            unassigned: format_amount(unassigned_minor, meta.exponent),
            unassigned_count,
        },
        months,
        sources: source_rows(
            conn,
            tenant_id,
            &start_iso,
            &end_iso,
            &currency,
            meta.exponent,
            start,
            end,
        )?,
        accounts: account_rows(
            conn,
            tenant_id,
            &start_iso,
            &end_iso,
            income_total,
            meta.exponent,
        )?,
        income_categories: category_slices(conn, tenant_id, &start_iso, &end_iso, "income")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, open_test};
    use crate::services::tx::{create_transaction, TxInput};

    struct Fixture {
        conn: Connection,
        tenant: String,
        user: String,
        account: String,
        income_category: String,
        expense_category: String,
    }

    fn setup() -> Fixture {
        let mut conn = open_test();
        let (tenant, user) = db::seed_if_empty(&mut conn).unwrap();
        let account = uuid::Uuid::new_v4().to_string();
        let now = dates::now_iso();
        conn.execute(
            "INSERT INTO accounts (id, tenant_id, user_id, name, type, currency_code, is_liability, is_liquid, opening_balance_minor, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'Main bank', 'bank', 'QAR', 0, 1, 0, ?4, ?4)",
            params![account, tenant, user, now],
        )
        .unwrap();
        let income_category: String = conn
            .query_row(
                "SELECT id FROM categories WHERE tenant_id = ?1 AND name = 'Salary'",
                [&tenant],
                |row| row.get(0),
            )
            .unwrap();
        let expense_category: String = conn
            .query_row(
                "SELECT id FROM categories WHERE tenant_id = ?1 AND name = 'Groceries'",
                [&tenant],
                |row| row.get(0),
            )
            .unwrap();
        Fixture {
            conn,
            tenant,
            user,
            account,
            income_category,
            expense_category,
        }
    }

    fn source(
        f: &Fixture,
        name: &str,
        currency: &str,
        expected: &str,
        schedule: &str,
        pay_day: Option<i64>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = dates::now_iso();
        let exp = crate::services::currency::exponent(&f.conn, currency).unwrap();
        let expected_minor = crate::money::parse_amount(expected, exp).unwrap();
        f.conn
            .execute(
                "INSERT INTO income_sources
                 (id, tenant_id, user_id, name, type, currency_code, expected_amount_minor,
                  schedule, pay_day, account_id, category_id, is_active, notes, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'salary', ?5, ?6, ?7, ?8, ?9, ?10, 1, NULL, ?11, ?11)",
                params![
                    id,
                    f.tenant,
                    f.user,
                    name,
                    currency,
                    expected_minor,
                    schedule,
                    pay_day,
                    f.account,
                    f.income_category,
                    now
                ],
            )
            .unwrap();
        id
    }

    fn income(
        f: &Fixture,
        amount: &str,
        date: &str,
        source_id: Option<&str>,
        status: &str,
    ) {
        create_transaction(
            &f.conn,
            &f.tenant,
            &f.user,
            &TxInput {
                kind: "income".into(),
                status: Some(status.into()),
                data_status: None,
                description: "Income entry".into(),
                payee: Some("Employer".into()),
                notes: None,
                date: date.into(),
                time: None,
                account_id: f.account.clone(),
                counter_account_id: None,
                category_id: Some(f.income_category.clone()),
                income_source_id: source_id.map(str::to_string),
                amount: amount.into(),
                currency: None,
                counter_amount: None,
                fx_rate: None,
                splits: None,
                tags: None,
                external_ref: None,
                direction: None,
            },
        )
        .unwrap();
    }

    fn expense(f: &Fixture, amount: &str, date: &str) {
        create_transaction(
            &f.conn,
            &f.tenant,
            &f.user,
            &TxInput {
                kind: "expense".into(),
                status: Some("posted".into()),
                data_status: None,
                description: "Groceries".into(),
                payee: Some("Market".into()),
                notes: None,
                date: date.into(),
                time: None,
                account_id: f.account.clone(),
                counter_account_id: None,
                category_id: Some(f.expense_category.clone()),
                income_source_id: None,
                amount: amount.into(),
                currency: None,
                counter_amount: None,
                fx_rate: None,
                splits: None,
                tags: None,
                external_ref: None,
                direction: None,
            },
        )
        .unwrap();
    }

    #[test]
    fn previous_range_aligns_calendar_months() {
        fn d(year: i32, month: u32, day: u32) -> NaiveDate {
            NaiveDate::from_ymd_opt(year, month, day).unwrap()
        }
        assert_eq!(
            previous_range(d(2026, 11, 1), d(2026, 11, 30)),
            (d(2026, 10, 1), d(2026, 10, 31))
        );
        assert_eq!(
            previous_range(d(2026, 9, 1), d(2026, 11, 30)),
            (d(2026, 6, 1), d(2026, 8, 31))
        );
        assert_eq!(
            previous_range(d(2026, 1, 10), d(2026, 1, 20)),
            (d(2025, 12, 30), d(2026, 1, 9))
        );
    }

    #[test]
    fn report_totals_and_compares_with_the_previous_range() {
        let f = setup();
        let salary = source(&f, "Salary", "QAR", "1000.00", "monthly", Some(1));
        income(&f, "1000.00", "2026-10-01", Some(&salary), "posted");
        income(&f, "900.00", "2026-11-01", Some(&salary), "posted");
        income(&f, "500.00", "2026-11-15", Some(&salary), "pending");
        expense(&f, "200.00", "2026-11-05");

        let report = income_report(&f.conn, &f.tenant, Some("2026-11-01"), Some("2026-11-30")).unwrap();
        assert_eq!(report.summary.income_minor, 90_000);
        assert_eq!(report.summary.expense_minor, 20_000);
        assert_eq!(report.summary.net_minor, 70_000);
        assert_eq!(report.summary.savings_rate, Some(77));
        assert_eq!(report.summary.previous_income_minor, 100_000);
        assert_eq!(report.summary.change_percent, Some(-10));
        assert_eq!(report.summary.month_count, 1);
        assert_eq!(report.summary.unassigned_minor, 0);
        assert_eq!(report.months.len(), 1);
        assert_eq!(report.months[0].label, "Nov 2026");
        assert_eq!(report.accounts.len(), 1);
        assert_eq!(report.accounts[0].income_minor, 90_000);
        assert_eq!(report.accounts[0].percent, 100);
        assert_eq!(report.income_categories.len(), 1);
        assert_eq!(report.income_categories[0].amount_minor, 90_000);
    }

    #[test]
    fn source_variance_uses_the_period_and_the_saved_schedule() {
        let f = setup();
        let salary = source(&f, "Salary", "QAR", "1000.00", "monthly", Some(1));
        income(&f, "900.00", "2026-11-01", Some(&salary), "posted");

        let report = income_report(&f.conn, &f.tenant, Some("2026-11-01"), Some("2026-11-30")).unwrap();
        let row = &report.sources[0];
        assert_eq!(row.occurrences, Some(1));
        assert_eq!(row.expected_minor, Some(100_000));
        assert_eq!(row.received_minor, 90_000);
        assert_eq!(row.variance_minor, Some(-10_000));
        assert_eq!(row.transaction_count, 1);
        assert_eq!(row.last_received_date.as_deref(), Some("2026-11-01"));
        assert!(row.estimate_note.is_none());
    }

    #[test]
    fn monthly_pay_day_outside_a_partial_range_is_not_counted() {
        let f = setup();
        source(&f, "Salary", "QAR", "1000.00", "monthly", Some(1));

        let report = income_report(&f.conn, &f.tenant, Some("2026-11-15"), Some("2026-11-30")).unwrap();
        let row = &report.sources[0];
        assert_eq!(row.occurrences, Some(0));
        assert_eq!(row.expected_minor, Some(0));
        assert_eq!(row.variance_minor, None);
        assert_eq!(row.received_minor, 0);
    }

    #[test]
    fn unassigned_income_is_reported_separately() {
        let f = setup();
        income(&f, "300.00", "2026-11-10", None, "posted");

        let report = income_report(&f.conn, &f.tenant, Some("2026-11-01"), Some("2026-11-30")).unwrap();
        assert_eq!(report.summary.income_minor, 30_000);
        assert_eq!(report.summary.unassigned_minor, 30_000);
        assert_eq!(report.summary.unassigned_count, 1);
        assert!(report.sources.is_empty());
    }

    #[test]
    fn foreign_currency_sources_get_no_converted_estimate() {
        let f = setup();
        source(&f, "US client", "USD", "500.00", "monthly", Some(5));

        let report = income_report(&f.conn, &f.tenant, Some("2026-11-01"), Some("2026-11-30")).unwrap();
        let row = &report.sources[0];
        assert_eq!(row.currency, "USD");
        assert_eq!(row.expected_amount_minor, 50_000);
        assert_eq!(row.expected_minor, None);
        assert_eq!(row.variance_minor, None);
        assert!(row.estimate_note.as_deref().unwrap().contains("not converted"));
    }

    #[test]
    fn weekly_sources_are_not_estimated() {
        let f = setup();
        let weekly = source(&f, "Weekly rent", "QAR", "100.00", "weekly", None);
        income(&f, "100.00", "2026-11-03", Some(&weekly), "posted");

        let report = income_report(&f.conn, &f.tenant, Some("2026-11-01"), Some("2026-11-30")).unwrap();
        let row = &report.sources[0];
        assert_eq!(row.occurrences, None);
        assert_eq!(row.expected_minor, None);
        assert_eq!(row.variance_minor, None);
        assert_eq!(row.received_minor, 10_000);
        assert!(row.estimate_note.is_some());
    }

    #[test]
    fn a_one_time_source_is_expected_until_it_is_received() {
        let f = setup();
        let bonus = source(&f, "Annual bonus", "QAR", "5000.00", "one_time", None);

        let before = income_report(&f.conn, &f.tenant, Some("2026-11-01"), Some("2026-11-30")).unwrap();
        assert_eq!(before.sources[0].occurrences, Some(1));
        assert_eq!(before.sources[0].expected_minor, Some(500_000));
        assert_eq!(before.sources[0].variance_minor, Some(-500_000));

        income(&f, "5000.00", "2026-11-20", Some(&bonus), "posted");
        let after = income_report(&f.conn, &f.tenant, Some("2026-11-01"), Some("2026-11-30")).unwrap();
        assert_eq!(after.sources[0].occurrences, Some(0));
        assert_eq!(after.sources[0].expected_minor, Some(0));
        assert_eq!(after.sources[0].variance_minor, None);
        assert_eq!(after.sources[0].received_minor, 500_000);
    }

    #[test]
    fn reversed_income_is_excluded() {
        let f = setup();
        let salary = source(&f, "Salary", "QAR", "1000.00", "monthly", Some(1));
        income(&f, "900.00", "2026-11-01", Some(&salary), "posted");
        let id: String = f
            .conn
            .query_row(
                "SELECT id FROM transactions WHERE tenant_id = ?1 ORDER BY created_at DESC LIMIT 1",
                [&f.tenant],
                |row| row.get(0),
            )
            .unwrap();
        crate::services::tx::reverse_transaction(&f.conn, &f.tenant, &f.user, &id).unwrap();

        let report = income_report(&f.conn, &f.tenant, Some("2026-11-01"), Some("2026-11-30")).unwrap();
        assert_eq!(report.summary.income_minor, 0);
        assert_eq!(report.sources[0].received_minor, 0);
    }

    #[test]
    fn an_inverted_range_is_rejected() {
        let f = setup();
        assert!(income_report(&f.conn, &f.tenant, Some("2026-12-01"), Some("2026-11-01")).is_err());
    }
}
