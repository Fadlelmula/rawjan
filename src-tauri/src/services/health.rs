use chrono::{Duration, NaiveDate};
use rusqlite::Connection;
use serde::Serialize;

use crate::dates::{self, iso};
use crate::error::AppResult;
use crate::money::format_amount;
use crate::services::accounts::net_worth;
use crate::services::analytics;
use crate::services::budgets;
use crate::services::currency::{currency_meta, primary_currency};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthComponent {
    pub key: String,
    pub label: String,
    pub weight: i64,
    pub score: Option<i64>,
    pub available: bool,
    pub detail: String,
    pub metric: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub score: i64,
    pub grade: String,
    pub label: String,
    pub currency: String,
    pub components: Vec<HealthComponent>,
    pub cash: String,
    pub debt: String,
    pub assets: String,
    pub invested: String,
    pub average_monthly_expense: Option<String>,
    pub savings_rate: Option<i64>,
    pub emergency_months: Option<String>,
    pub debt_ratio: Option<i64>,
    pub invested_share: Option<i64>,
    pub net_worth_trend: Option<i64>,
    pub budget_count: i64,
    pub budgets_within: i64,
    pub notes: Vec<String>,
}

fn clamp_score(value: i64) -> i64 {
    value.clamp(0, 100)
}

fn grade_for(score: i64) -> (&'static str, &'static str) {
    match score {
        85..=100 => ("A", "Strong"),
        70..=84 => ("B", "Steady"),
        55..=69 => ("C", "Watchful"),
        40..=54 => ("D", "Strained"),
        _ => ("E", "Critical"),
    }
}

/// Score savings over the last three complete calendar months, so a partial
/// current month does not distort the picture.
fn savings_window(today: NaiveDate) -> (NaiveDate, NaiveDate) {
    let start = dates::month_start(today, -3);
    let end = dates::month_start(today, 0) - Duration::days(1);
    (start, end)
}

fn trend_percent(conn: &Connection, tenant_id: &str, currency: &str) -> AppResult<Option<i64>> {
    let mut stmt = conn.prepare(
        "SELECT as_of_date, net_minor FROM net_worth_snapshots
         WHERE tenant_id = ?1 AND currency_code = ?2
         ORDER BY as_of_date ASC, created_at ASC",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![tenant_id, currency], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let Some(last) = rows.last() else {
        return Ok(None);
    };
    let first_date = dates::parse_date(&first.0)?;
    let last_date = dates::parse_date(&last.0)?;
    if (last_date - first_date).num_days() < 30 || first.1 == 0 {
        return Ok(None);
    }
    Ok(Some(
        ((last.1 - first.1) as i128 * 100 / first.1.abs() as i128) as i64,
    ))
}

pub fn health_report(conn: &Connection, tenant_id: &str) -> AppResult<HealthReport> {
    let today = dates::today_local();
    let currency = primary_currency(conn, tenant_id)?;
    let meta = currency_meta(conn, &currency)?;
    let worth = net_worth(conn, tenant_id, None)?;

    let (start, end) = savings_window(today);
    let savings = analytics::analytics(conn, tenant_id, Some(&iso(start)), Some(&iso(end)))?;
    let average_expense = savings.summary.average_monthly_expense_minor;
    let savings_rate = savings.summary.savings_rate;

    let budgets = budgets::list_budgets(conn, tenant_id, Some(&iso(today)))?;
    let active_budgets: Vec<_> = budgets.iter().filter(|budget| budget.is_active).collect();
    let budgets_within = active_budgets
        .iter()
        .filter(|budget| budget.allocated_minor > 0 && budget.percent_used <= 100)
        .count() as i64;
    let budget_count = active_budgets
        .iter()
        .filter(|budget| budget.allocated_minor > 0)
        .count() as i64;

    let emergency_months = if average_expense > 0 {
        Some((worth.cash_minor.max(0) as f64) / (average_expense as f64))
    } else {
        None
    };
    let debt_ratio = if worth.assets_minor > 0 {
        Some(
            ((worth.liabilities_minor.max(0) as f64) / (worth.assets_minor as f64) * 100.0).round()
                as i64,
        )
    } else {
        None
    };
    let invested_share = if worth.assets_minor > 0 {
        Some(
            ((worth.invested_minor.max(0) as f64) / (worth.assets_minor as f64) * 100.0).round()
                as i64,
        )
    } else {
        None
    };
    let net_worth_trend = trend_percent(conn, tenant_id, &currency)?;

    let mut notes = vec![
        "The score is a deterministic summary of recorded data. It is not a credit score, a rating or financial advice.".to_string(),
        "Savings uses the last three complete calendar months. Partial current months are excluded.".to_string(),
    ];
    if worth.missing_rates.is_empty() {
        notes.push("All balances converted with stored exchange rates.".to_string());
    } else {
        notes.push(format!(
            "Some currencies were excluded because no exchange rate is stored: {}.",
            worth.missing_rates.join(", ")
        ));
    }

    let mut components = Vec::new();

    components.push(HealthComponent {
        key: "savings_rate".into(),
        label: "Savings rate".into(),
        weight: 30,
        score: savings_rate.map(|rate| clamp_score((rate as f64 / 30.0 * 100.0).round() as i64)),
        available: savings_rate.is_some(),
        detail: match savings_rate {
            Some(rate) if rate < 0 => "Spending exceeded income across the window.".into(),
            Some(rate) => format!("{rate}% of income kept over the last three complete months."),
            None => "No income was recorded in the window, so this cannot be scored.".into(),
        },
        metric: savings_rate.map(|rate| format!("{rate}%")),
    });

    components.push(HealthComponent {
        key: "emergency_fund".into(),
        label: "Emergency fund".into(),
        weight: 25,
        score: emergency_months.map(|months| clamp_score((months / 6.0 * 100.0).round() as i64)),
        available: emergency_months.is_some(),
        detail: match emergency_months {
            Some(months) => format!(
                "{:.1} months of average expenses held as liquid cash. Six months scores full marks.",
                months
            ),
            None => "Average monthly expenses are zero, so a coverage ratio cannot be formed.".into(),
        },
        metric: emergency_months.map(|months| format!("{months:.1} mo")),
    });

    components.push(HealthComponent {
        key: "debt_load".into(),
        label: "Debt load".into(),
        weight: 20,
        score: debt_ratio
            .map(|ratio| clamp_score(((50 - ratio) as f64 / 50.0 * 100.0).round() as i64)),
        available: debt_ratio.is_some(),
        detail: match debt_ratio {
            Some(ratio) => {
                format!("Liabilities equal {ratio}% of assets. Zero debt scores full marks.")
            }
            None => "No positive asset total exists yet, so a debt ratio cannot be formed.".into(),
        },
        metric: debt_ratio.map(|ratio| format!("{ratio}%")),
    });

    components.push(HealthComponent {
        key: "investing".into(),
        label: "Invested share".into(),
        weight: 15,
        score: invested_share
            .map(|share| clamp_score((share as f64 / 40.0 * 100.0).round() as i64)),
        available: invested_share.is_some(),
        detail: match invested_share {
            Some(share) => format!(
                "Investments are {share}% of assets. Forty percent or more scores full marks."
            ),
            None => {
                "No positive asset total exists yet, so an invested share cannot be formed.".into()
            }
        },
        metric: invested_share.map(|share| format!("{share}%")),
    });

    components.push(HealthComponent {
        key: "budget_adherence".into(),
        label: "Budget adherence".into(),
        weight: 10,
        score: if budget_count > 0 {
            Some(clamp_score(
                (budgets_within as f64 / budget_count as f64 * 100.0).round() as i64,
            ))
        } else {
            None
        },
        available: budget_count > 0,
        detail: if budget_count > 0 {
            format!("{budgets_within} of {budget_count} active budgets are at or under their allocation.")
        } else {
            "No active budget with an allocation exists, so this cannot be scored.".into()
        },
        metric: if budget_count > 0 {
            Some(format!("{budgets_within}/{budget_count}"))
        } else {
            None
        },
    });

    components.push(HealthComponent {
        key: "net_worth_trend".into(),
        label: "Net worth trend".into(),
        weight: 10,
        score: net_worth_trend
            .map(|trend| clamp_score(((trend + 5) as f64 / 20.0 * 100.0).round() as i64)),
        available: net_worth_trend.is_some(),
        detail: match net_worth_trend {
            Some(trend) => format!(
                "Net worth changed {trend}% since the earliest snapshot at least 30 days old."
            ),
            None => {
                "At least two snapshots 30 days apart in the current currency are needed.".into()
            }
        },
        metric: net_worth_trend.map(|trend| format!("{trend}%")),
    });

    let available_weight: i64 = components
        .iter()
        .filter(|component| component.available)
        .map(|component| component.weight)
        .sum();
    let weighted: i64 = components
        .iter()
        .filter_map(|component| component.score.map(|score| score * component.weight))
        .sum();
    let score = if available_weight > 0 {
        (weighted as f64 / available_weight as f64).round() as i64
    } else {
        0
    };
    let (grade, label) = grade_for(score);

    Ok(HealthReport {
        score,
        grade: grade.to_string(),
        label: label.to_string(),
        currency: currency.clone(),
        components,
        cash: format_amount(worth.cash_minor, meta.exponent),
        debt: format_amount(worth.liabilities_minor, meta.exponent),
        assets: format_amount(worth.assets_minor, meta.exponent),
        invested: format_amount(worth.invested_minor, meta.exponent),
        average_monthly_expense: if average_expense > 0 {
            Some(format_amount(average_expense, meta.exponent))
        } else {
            None
        },
        savings_rate,
        emergency_months: emergency_months.map(|months| format!("{months:.1}")),
        debt_ratio,
        invested_share,
        net_worth_trend,
        budget_count,
        budgets_within,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, open_test};
    use crate::services::tx::{create_transaction, TxInput};
    use rusqlite::params;

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

    fn tx(kind: &str, account: &str, category: &str, amount: &str, date: &str) -> TxInput {
        TxInput {
            kind: kind.into(),
            status: Some("posted".into()),
            data_status: None,
            description: "Test".into(),
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
    fn health_score_uses_recorded_activity_and_reports_missing_parts() {
        let (conn, tenant, user, account, category) = setup();
        let income_category: String = conn
            .query_row(
                "SELECT id FROM categories WHERE tenant_id = ?1 AND name = 'Salary'",
                [&tenant],
                |r| r.get(0),
            )
            .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx(
                "income",
                &account,
                &income_category,
                "9000.00",
                "2026-08-05",
            ),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("expense", &account, &category, "3000.00", "2026-08-10"),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("expense", &account, &category, "3000.00", "2026-09-10"),
        )
        .unwrap();
        create_transaction(
            &conn,
            &tenant,
            &user,
            &tx("expense", &account, &category, "3000.00", "2026-10-10"),
        )
        .unwrap();

        let report = health_report(&conn, &tenant).unwrap();
        assert_eq!(report.savings_rate, Some(0));
        assert_eq!(report.budget_count, 0);
        let budget_component = report
            .components
            .iter()
            .find(|component| component.key == "budget_adherence")
            .unwrap();
        assert!(!budget_component.available);
        assert!(report.score >= 0 && report.score <= 100);
        assert!(!report.grade.is_empty());
    }

    #[test]
    fn savings_window_excludes_the_current_partial_month() {
        let today = dates::parse_date("2026-11-15").unwrap();
        let (start, end) = savings_window(today);
        assert_eq!(iso(start), "2026-08-01");
        assert_eq!(iso(end), "2026-10-31");
    }
}
