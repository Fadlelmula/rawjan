use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::dates::{self, iso};
use crate::error::{AppError, AppResult};
use crate::money::format_amount;
use crate::services::currency::{currency_meta, primary_currency};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDay {
    pub date: String,
    pub day: u32,
    pub income_minor: i64,
    pub income: String,
    pub expense_minor: i64,
    pub expense: String,
    pub net_minor: i64,
    pub net: String,
    pub transaction_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarMonth {
    pub month: String,
    pub label: String,
    pub from: String,
    pub to: String,
    pub days_in_month: u32,
    /// Monday = 0, Sunday = 6.
    pub first_weekday: u32,
    pub currency: String,
    pub currency_symbol: String,
    pub income_minor: i64,
    pub income: String,
    pub expense_minor: i64,
    pub expense: String,
    pub net_minor: i64,
    pub net: String,
    pub transaction_count: i64,
    pub active_days: i64,
    pub days: Vec<CalendarDay>,
}

pub fn month_calendar(
    conn: &Connection,
    tenant_id: &str,
    month: Option<&str>,
) -> AppResult<CalendarMonth> {
    let today = dates::today_local();
    let anchor = match month.filter(|value| !value.trim().is_empty()) {
        Some(value) => {
            let raw = format!("{}-01", value.trim());
            NaiveDate::parse_from_str(&raw, "%Y-%m-%d").map_err(|_| {
                AppError::Validation(format!("Invalid month '{value}'. Expected YYYY-MM."))
            })?
        }
        None => NaiveDate::from_ymd_opt(today.year(), today.month(), 1).expect("valid month"),
    };
    let (start, end) = dates::calendar_month(anchor);
    let start_iso = iso(start);
    let end_iso = iso(end);
    let currency = primary_currency(conn, tenant_id)?;
    let meta = currency_meta(conn, &currency)?;

    let mut stmt = conn.prepare(
        "SELECT t.local_date,
                SUM(CASE WHEN t.kind IN ('income','dividend') THEN t.base_amount_minor ELSE 0 END) AS income,
                SUM(CASE WHEN t.kind IN ('expense','fee') THEN t.base_amount_minor
                         WHEN t.kind = 'refund' THEN -t.base_amount_minor ELSE 0 END) AS expense,
                COUNT(*) AS count
         FROM transactions t
         WHERE t.tenant_id = ?1
           AND t.status IN ('posted','reconciled')
           AND t.reversal_of_id IS NULL
           AND t.local_date BETWEEN ?2 AND ?3
         GROUP BY t.local_date
         ORDER BY t.local_date",
    )?;
    let rows = stmt
        .query_map(params![tenant_id, start_iso, end_iso], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut days = Vec::with_capacity(rows.len());
    let mut income_total = 0i64;
    let mut expense_total = 0i64;
    let mut count_total = 0i64;
    for (date, income, expense, count) in rows {
        let net = income - expense;
        income_total += income;
        expense_total += expense;
        count_total += count;
        let day = dates::parse_date(&date)?.day();
        days.push(CalendarDay {
            date,
            day,
            income_minor: income,
            income: format_amount(income, meta.exponent),
            expense_minor: expense,
            expense: format_amount(expense, meta.exponent),
            net_minor: net,
            net: format_amount(net, meta.exponent),
            transaction_count: count,
        });
    }
    let net_total = income_total - expense_total;
    let first_weekday = start.weekday().num_days_from_monday();

    Ok(CalendarMonth {
        month: dates::month_key(start),
        label: start.format("%B %Y").to_string(),
        from: start_iso,
        to: end_iso,
        days_in_month: dates::days_in_month(start.year(), start.month()),
        first_weekday,
        currency,
        currency_symbol: meta.symbol,
        income_minor: income_total,
        income: format_amount(income_total, meta.exponent),
        expense_minor: expense_total,
        expense: format_amount(expense_total, meta.exponent),
        net_minor: net_total,
        net: format_amount(net_total, meta.exponent),
        transaction_count: count_total,
        active_days: days.len() as i64,
        days,
    })
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
    fn calendar_only_lists_days_with_effective_activity() {
        let (conn, tenant, user, account, category) = setup();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &expense(&account, &category, "50.00", "posted", "2026-11-03"),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &expense(&account, &category, "25.00", "posted", "2026-11-03"),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &expense(&account, &category, "99.00", "pending", "2026-11-04"),
        )
        .unwrap();

        let calendar = month_calendar(&conn, &tenant, Some("2026-11")).unwrap();
        assert_eq!(calendar.days_in_month, 30);
        assert_eq!(calendar.active_days, 1);
        assert_eq!(calendar.days[0].date, "2026-11-03");
        assert_eq!(calendar.days[0].expense_minor, 7_500);
        assert_eq!(calendar.days[0].transaction_count, 2);
        assert_eq!(calendar.expense_minor, 7_500);
        assert_eq!(calendar.transaction_count, 2);
    }

    #[test]
    fn calendar_starts_on_monday_index_and_rejects_bad_month() {
        let (conn, tenant, _, _, _) = setup();
        let calendar = month_calendar(&conn, &tenant, Some("2026-11")).unwrap();
        assert_eq!(calendar.first_weekday, 6, "1 Nov 2026 is a Sunday");
        assert!(month_calendar(&conn, &tenant, Some("November")).is_err());
    }
}
