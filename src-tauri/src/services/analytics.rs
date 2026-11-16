use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::Serialize;

use crate::dates::{self, iso};
use crate::error::AppResult;
use crate::money::format_amount;
use crate::services::currency::{currency_meta, exponent, primary_currency};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsSummary {
    pub income_minor: i64,
    pub income: String,
    pub expense_minor: i64,
    pub expense: String,
    pub net_minor: i64,
    pub net: String,
    pub savings_rate: Option<i64>,
    pub transaction_count: i64,
    pub month_count: i64,
    pub average_monthly_expense_minor: i64,
    pub average_monthly_expense: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthPoint {
    pub month: String,
    pub label: String,
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
pub struct CategorySlice {
    pub category_id: String,
    pub name: String,
    pub color: Option<String>,
    pub amount_minor: i64,
    pub amount: String,
    pub percent: i64,
    pub transaction_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayeeRow {
    pub label: String,
    pub amount_minor: i64,
    pub amount: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsReport {
    pub from: String,
    pub to: String,
    pub currency: String,
    pub currency_symbol: String,
    pub summary: AnalyticsSummary,
    pub months: Vec<MonthPoint>,
    pub expense_categories: Vec<CategorySlice>,
    pub income_categories: Vec<CategorySlice>,
    pub payees: Vec<PayeeRow>,
}

struct MonthTotals {
    income: i64,
    expense: i64,
    count: i64,
}

fn month_label(month: &str) -> String {
    match NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d") {
        Ok(date) => date.format("%b %Y").to_string(),
        Err(_) => month.to_string(),
    }
}

fn percent(part: i64, total: i64) -> i64 {
    if total <= 0 {
        return 0;
    }
    ((part.max(0) as i128 * 100) / total as i128) as i64
}

/// Income and expense for each month. Only effective, non-reversed records count.
/// Transfers, investment moves, openings and adjustments are excluded: they move
/// value between accounts rather than describing income or spending.
fn monthly_totals(
    conn: &Connection,
    tenant_id: &str,
    start: &str,
    end: &str,
) -> AppResult<HashMap<String, MonthTotals>> {
    let mut stmt = conn.prepare(
        "SELECT substr(t.local_date, 1, 7) AS ym,
                SUM(CASE WHEN t.kind IN ('income','dividend') THEN t.base_amount_minor ELSE 0 END) AS income,
                SUM(CASE WHEN t.kind IN ('expense','fee') THEN t.base_amount_minor
                         WHEN t.kind = 'refund' THEN -t.base_amount_minor ELSE 0 END) AS expense,
                COUNT(*) AS count
         FROM transactions t
         WHERE t.tenant_id = ?1
           AND t.status IN ('posted','reconciled')
           AND t.reversal_of_id IS NULL
           AND t.local_date BETWEEN ?2 AND ?3
         GROUP BY ym
         ORDER BY ym",
    )?;
    let rows = stmt.query_map(params![tenant_id, start, end], |row| {
        Ok((
            row.get::<_, String>(0)?,
            MonthTotals {
                income: row.get(1)?,
                expense: row.get(2)?,
                count: row.get(3)?,
            },
        ))
    })?;
    let mut result = HashMap::new();
    for row in rows {
        let (month, totals) = row?;
        result.insert(month, totals);
    }
    Ok(result)
}

fn category_slices(
    conn: &Connection,
    tenant_id: &str,
    start: &str,
    end: &str,
    kind: &str,
) -> AppResult<Vec<CategorySlice>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.color,
                SUM(CASE WHEN c.kind = 'income'
                         THEN CASE WHEN le.entry_type = 'credit' THEN le.base_amount_minor ELSE -le.base_amount_minor END
                         ELSE CASE WHEN le.entry_type = 'debit' THEN le.base_amount_minor ELSE -le.base_amount_minor END
                    END) AS amount,
                COUNT(DISTINCT t.id) AS tx_count
         FROM ledger_entries le
         JOIN transactions t ON t.id = le.transaction_id
         JOIN categories c ON c.id = le.category_id
         WHERE le.tenant_id = ?1
           AND c.kind = ?4
           AND t.status IN ('posted','reconciled')
           AND t.reversal_of_id IS NULL
           AND t.local_date BETWEEN ?2 AND ?3
         GROUP BY c.id
         HAVING amount <> 0
         ORDER BY amount DESC
         LIMIT 12",
    )?;
    let rows = stmt.query_map(params![tenant_id, start, end, kind], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;
    let collected = rows.collect::<Result<Vec<_>, _>>()?;
    let total: i64 = collected.iter().map(|(_, _, _, amount, _)| *amount).sum();
    let exp = exponent(conn, &primary_currency(conn, tenant_id)?)?;
    Ok(collected
        .into_iter()
        .map(|(category_id, name, color, amount, tx_count)| CategorySlice {
            category_id,
            name,
            color,
            amount_minor: amount,
            amount: format_amount(amount, exp),
            percent: percent(amount, total),
            transaction_count: tx_count,
        })
        .collect())
}

fn top_payees(
    conn: &Connection,
    tenant_id: &str,
    start: &str,
    end: &str,
) -> AppResult<Vec<PayeeRow>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(NULLIF(TRIM(t.payee), ''), t.description) AS label,
                SUM(t.base_amount_minor) AS amount,
                COUNT(*) AS count
         FROM transactions t
         WHERE t.tenant_id = ?1
           AND t.status IN ('posted','reconciled')
           AND t.reversal_of_id IS NULL
           AND t.kind IN ('expense','fee')
           AND t.local_date BETWEEN ?2 AND ?3
         GROUP BY label
         ORDER BY amount DESC
         LIMIT 8",
    )?;
    let rows = stmt.query_map(params![tenant_id, start, end], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let exp = exponent(conn, &primary_currency(conn, tenant_id)?)?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|(label, amount, count)| PayeeRow {
            label,
            amount_minor: amount,
            amount: format_amount(amount, exp),
            count,
        })
        .collect())
}

pub fn analytics(
    conn: &Connection,
    tenant_id: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> AppResult<AnalyticsReport> {
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
        return Err(crate::error::AppError::Validation(
            "The reporting range starts after it ends.".into(),
        ));
    }

    let currency = primary_currency(conn, tenant_id)?;
    let meta = currency_meta(conn, &currency)?;
    let start_iso = iso(start);
    let end_iso = iso(end);

    let totals = monthly_totals(conn, tenant_id, &start_iso, &end_iso)?;

    let mut months = Vec::new();
    for (month_start, _month_end) in dates::each_month(start, end) {
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
    let average_expense = expense_total / month_count;
    let savings_rate = if income_total > 0 {
        Some(((net_total as i128 * 100) / income_total as i128) as i64)
    } else {
        None
    };

    Ok(AnalyticsReport {
        from: start_iso.clone(),
        to: end_iso.clone(),
        currency: currency.clone(),
        currency_symbol: meta.symbol,
        summary: AnalyticsSummary {
            income_minor: income_total,
            income: format_amount(income_total, meta.exponent),
            expense_minor: expense_total,
            expense: format_amount(expense_total, meta.exponent),
            net_minor: net_total,
            net: format_amount(net_total, meta.exponent),
            savings_rate,
            transaction_count,
            month_count: months.len() as i64,
            average_monthly_expense_minor: average_expense,
            average_monthly_expense: format_amount(average_expense, meta.exponent),
        },
        months,
        expense_categories: category_slices(conn, tenant_id, &start_iso, &end_iso, "expense")?,
        income_categories: category_slices(conn, tenant_id, &start_iso, &end_iso, "income")?,
        payees: top_payees(conn, tenant_id, &start_iso, &end_iso)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, open_test};
    use crate::services::tx::{create_transaction, TxInput};

    fn setup() -> (Connection, String, String, String, String, String) {
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
        let expense_category: String = conn
            .query_row(
                "SELECT id FROM categories WHERE tenant_id = ?1 AND name = 'Groceries'",
                [&tenant],
                |r| r.get(0),
            )
            .unwrap();
        let income_category: String = conn
            .query_row(
                "SELECT id FROM categories WHERE tenant_id = ?1 AND name = 'Salary'",
                [&tenant],
                |r| r.get(0),
            )
            .unwrap();
        (conn, tenant, user, account_id, expense_category, income_category)
    }

    fn tx(
        kind: &str,
        account: &str,
        category: &str,
        amount: &str,
        status: &str,
        date: &str,
        payee: Option<&str>,
    ) -> TxInput {
        TxInput {
            kind: kind.into(),
            status: Some(status.into()),
            data_status: None,
            description: format!("{kind} entry"),
            payee: payee.map(str::to_string),
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
    fn monthly_summary_counts_effective_income_and_expenses_only() {
        let (conn, tenant, user, account, expense_category, income_category) = setup();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("income", &account, &income_category, "1000.00", "posted", "2026-10-05", None),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("expense", &account, &expense_category, "250.00", "posted", "2026-10-06", Some("Market")),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("expense", &account, &expense_category, "75.00", "pending", "2026-10-07", Some("Market")),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("expense", &account, &expense_category, "40.00", "posted", "2026-11-03", Some("Bakery")),
        )
        .unwrap();

        let report = analytics(&conn, &tenant, Some("2026-10-01"), Some("2026-11-30")).unwrap();
        assert_eq!(report.summary.income_minor, 100_000);
        assert_eq!(report.summary.expense_minor, 29_000);
        assert_eq!(report.summary.net_minor, 71_000);
        assert_eq!(report.summary.savings_rate, Some(71));
        assert_eq!(report.months.len(), 2);
        assert_eq!(report.months[0].label, "Oct 2026");
        assert_eq!(report.months[0].transaction_count, 2);
        assert_eq!(report.months[1].expense_minor, 4_000);
    }

    #[test]
    fn refunds_reduce_expense_totals_and_category_spend() {
        let (conn, tenant, user, account, expense_category, _) = setup();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("expense", &account, &expense_category, "120.00", "posted", "2026-11-02", Some("Shop")),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("refund", &account, &expense_category, "20.00", "posted", "2026-11-04", Some("Shop")),
        )
        .unwrap();

        let report = analytics(&conn, &tenant, Some("2026-11-01"), Some("2026-11-30")).unwrap();
        assert_eq!(report.summary.expense_minor, 10_000);
        assert_eq!(report.expense_categories.len(), 1);
        assert_eq!(report.expense_categories[0].amount_minor, 10_000);
        assert_eq!(report.expense_categories[0].percent, 100);
    }

    #[test]
    fn payees_are_grouped_from_expenses() {
        let (conn, tenant, user, account, expense_category, _) = setup();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("expense", &account, &expense_category, "30.00", "posted", "2026-11-02", Some("Cafe")),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("expense", &account, &expense_category, "45.00", "posted", "2026-11-09", Some("Cafe")),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("expense", &account, &expense_category, "10.00", "pending", "2026-11-09", Some("Cafe")),
        )
        .unwrap();

        let report = analytics(&conn, &tenant, Some("2026-11-01"), Some("2026-11-30")).unwrap();
        assert_eq!(report.payees.len(), 1);
        assert_eq!(report.payees[0].label, "Cafe");
        assert_eq!(report.payees[0].amount_minor, 7_500);
        assert_eq!(report.payees[0].count, 2);
    }

    #[test]
    fn an_inverted_range_is_rejected() {
        let (conn, tenant, _, _, _, _) = setup();
        assert!(analytics(&conn, &tenant, Some("2026-12-01"), Some("2026-11-01")).is_err());
    }
}
