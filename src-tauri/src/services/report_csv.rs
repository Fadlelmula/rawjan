//! CSV export of the reports shown in the interface.
//!
//! The export writes the same display strings the UI renders — no amounts are
//! recomputed here. Files land in the managed `exports` folder next to the
//! encrypted exports.

use std::path::Path;

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::services::analytics;
use crate::services::budgets;
use crate::services::income;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportExportResult {
    pub path: String,
    pub file_name: String,
    pub row_count: i64,
    pub byte_size: i64,
}

fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn csv_row<I, S>(cells: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    cells
        .into_iter()
        .map(|cell| csv_field(cell.as_ref()))
        .collect::<Vec<_>>()
        .join(",")
}

fn write_report(dir: &Path, name: &str, lines: &[String]) -> AppResult<ReportExportResult> {
    std::fs::create_dir_all(dir)?;
    let file_name = format!(
        "rawjan-report-{name}-{}.csv",
        crate::dates::now_iso().replace(':', "")
    );
    let path = dir.join(&file_name);
    let mut body = lines.join("\n");
    body.push('\n');
    std::fs::write(&path, &body)?;
    Ok(ReportExportResult {
        path: path.display().to_string(),
        file_name,
        row_count: lines.len().max(1) as i64 - 1,
        byte_size: body.len() as i64,
    })
}

/// Export one of the displayed reports. `from`/`to` apply to the ranged
/// reports (analytics, income); budgets uses its reference date.
pub fn export_report(
    conn: &Connection,
    tenant_id: &str,
    dir: &Path,
    report: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> AppResult<ReportExportResult> {
    match report {
        "analytics" => {
            let data = analytics::analytics(conn, tenant_id, from, to)?;
            let mut lines = vec![csv_row([
                "Month",
                "Income",
                "Expense",
                "Net",
                "Income entries",
                "Expense entries",
                "General entries",
            ])];
            for month in &data.months {
                lines.push(csv_row([
                    month.label.clone(),
                    month.income.clone(),
                    month.expense.clone(),
                    month.net.clone(),
                    month.income_count.to_string(),
                    month.expense_count.to_string(),
                    month.general_count.to_string(),
                ]));
            }
            lines.push(String::new());
            lines.push(csv_row(["Category", "Amount", "Percent", "Entries"]));
            for slice in data
                .expense_categories
                .iter()
                .chain(data.income_categories.iter())
            {
                lines.push(csv_row([
                    slice.name.clone(),
                    slice.amount.clone(),
                    format!("{}%", slice.percent),
                    slice.transaction_count.to_string(),
                ]));
            }
            write_report(dir, "analytics", &lines)
        }
        "income" => {
            let data = income::income_report(conn, tenant_id, from, to)?;
            let mut lines = vec![csv_row([
                "Source",
                "Type",
                "Schedule",
                "Expected",
                "Received",
                "Variance",
                "Entries",
                "Last received",
                "Note",
            ])];
            for source in &data.sources {
                lines.push(csv_row([
                    source.name.clone(),
                    source.source_type.clone(),
                    source.schedule.clone(),
                    source.expected.clone().unwrap_or_else(|| "—".into()),
                    source.received.clone(),
                    source.variance.clone().unwrap_or_else(|| "—".into()),
                    source.transaction_count.to_string(),
                    source
                        .last_received_date
                        .clone()
                        .unwrap_or_else(|| "—".into()),
                    source.estimate_note.clone().unwrap_or_default(),
                ]));
            }
            lines.push(String::new());
            lines.push(csv_row([
                "Month",
                "Income",
                "Expense",
                "Net",
                "Income entries",
                "Expense entries",
                "General entries",
            ]));
            for month in &data.months {
                lines.push(csv_row([
                    month.label.clone(),
                    month.income.clone(),
                    month.expense.clone(),
                    month.net.clone(),
                    month.income_count.to_string(),
                    month.expense_count.to_string(),
                    month.general_count.to_string(),
                ]));
            }
            write_report(dir, "income", &lines)
        }
        "budgets" => {
            let reference = to.map(str::to_string);
            let data = budgets::list_budgets(conn, tenant_id, reference.as_deref())?;
            let mut lines = vec![csv_row([
                "Budget",
                "Period start",
                "Period end",
                "Category",
                "Allocated",
                "Spent",
                "Remaining",
                "Percent used",
                "Conversion",
            ])];
            for budget in &data {
                for allocation in &budget.allocations {
                    lines.push(csv_row([
                        budget.name.clone(),
                        budget.period_start.clone(),
                        budget.period_end.clone(),
                        allocation.category_name.clone(),
                        allocation.amount.clone(),
                        allocation
                            .spent
                            .clone()
                            .unwrap_or_else(|| "unavailable".into()),
                        allocation
                            .remaining
                            .clone()
                            .unwrap_or_else(|| "unavailable".into()),
                        allocation
                            .percent_used
                            .map(|value| format!("{value}%"))
                            .unwrap_or_else(|| "—".into()),
                        if allocation.fx_unavailable {
                            "no exchange rate".into()
                        } else {
                            "converted".into()
                        },
                    ]));
                }
            }
            write_report(dir, "budgets", &lines)
        }
        "transactions" => {
            let mut stmt = conn.prepare(
                "SELECT t.local_date, t.kind, t.status, t.description,
                        COALESCE(t.payee, ''), a.name, COALESCE(c.name, ''),
                        t.original_amount_minor, t.original_currency, t.base_amount_minor
                 FROM transactions t
                 JOIN accounts a ON a.id = t.account_id
                 LEFT JOIN categories c ON c.id = t.category_id
                 WHERE t.tenant_id = ?1
                   AND (?2 IS NULL OR t.local_date >= ?2)
                   AND (?3 IS NULL OR t.local_date <= ?3)
                 ORDER BY t.local_date, t.created_at",
            )?;
            let rows = stmt.query_map(params![tenant_id, from, to], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            })?;
            let mut lines = vec![csv_row([
                "Date",
                "Kind",
                "Status",
                "Description",
                "Payee",
                "Account",
                "Category",
                "Amount",
                "Currency",
                "Base amount",
            ])];
            for row in rows {
                let (
                    date,
                    kind,
                    status,
                    description,
                    payee,
                    account,
                    category,
                    original_minor,
                    currency,
                    base_minor,
                ) = row?;
                let original_exp = crate::services::currency::exponent(conn, &currency)?;
                let base = crate::services::currency::primary_currency(conn, tenant_id)?;
                let base_exp = crate::services::currency::exponent(conn, &base)?;
                lines.push(csv_row([
                    date,
                    kind,
                    status,
                    description,
                    payee,
                    account,
                    category,
                    crate::money::format_amount(original_minor, original_exp),
                    currency,
                    crate::money::format_amount(base_minor, base_exp),
                ]));
            }
            write_report(dir, "transactions", &lines)
        }
        other => Err(AppError::Validation(format!(
            "Unknown report '{other}'. Use analytics, income, budgets or transactions."
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, open_test};
    use crate::services::tx::{create_transaction, TxInput};

    fn expense(account: &str, category: &str, amount: &str, description: &str) -> TxInput {
        TxInput {
            kind: "expense".into(),
            status: Some("posted".into()),
            data_status: None,
            description: description.into(),
            payee: Some("Acme, \"Q3\" store".into()),
            notes: None,
            date: "2026-11-14".into(),
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
    fn csv_exports_contain_the_displayed_values_and_escape_correctly() {
        let mut conn = open_test();
        let (tenant, user) = db::seed_if_empty(&mut conn).unwrap();
        let account = uuid::Uuid::new_v4().to_string();
        let now = crate::dates::now_iso();
        conn.execute(
            "INSERT INTO accounts (id, tenant_id, user_id, name, type, currency_code, is_liability, is_liquid, opening_balance_minor, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'Main bank', 'bank', 'QAR', 0, 1, 0, ?4, ?4)",
            params![account, tenant, user, now],
        )
        .unwrap();
        let category: String = conn
            .query_row(
                "SELECT id FROM categories WHERE tenant_id = ?1 AND name = 'Groceries'",
                [&tenant],
                |r| r.get(0),
            )
            .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &expense(&account, &category, "250.00", "Weekly, big \"shop\""),
        )
        .unwrap();

        let dir = tempfile::tempdir().unwrap();

        let analytics = export_report(
            &conn,
            &tenant,
            dir.path(),
            "analytics",
            Some("2026-11-01"),
            Some("2026-11-30"),
        )
        .unwrap();
        let body = std::fs::read_to_string(&analytics.path).unwrap();
        assert!(body.starts_with("Month,Income,Expense,Net,"));
        assert!(body.contains("Nov 2026"));
        assert!(body.contains("250.00"));

        let income = export_report(
            &conn,
            &tenant,
            dir.path(),
            "income",
            Some("2026-11-01"),
            Some("2026-11-30"),
        )
        .unwrap();
        let body = std::fs::read_to_string(&income.path).unwrap();
        assert!(body.starts_with("Source,Type,Schedule,"));

        let budgets = export_report(
            &conn,
            &tenant,
            dir.path(),
            "budgets",
            None,
            Some("2026-11-14"),
        )
        .unwrap();
        let body = std::fs::read_to_string(&budgets.path).unwrap();
        assert!(body.starts_with("Budget,Period start,Period end,Category,"));

        let transactions = export_report(
            &conn,
            &tenant,
            dir.path(),
            "transactions",
            Some("2026-11-01"),
            Some("2026-11-30"),
        )
        .unwrap();
        let body = std::fs::read_to_string(&transactions.path).unwrap();
        // Descriptions and payees containing commas and quotes must be escaped,
        // never silently truncated or unquoted.
        assert!(body.contains("\"Weekly, big \"\"shop\"\"\""));
        assert!(body.contains("\"Acme, \"\"Q3\"\" store\""));
        assert!(body.contains("250.00"));

        assert!(export_report(&conn, &tenant, dir.path(), "nope", None, None).is_err());
    }
}
