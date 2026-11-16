use rusqlite::Connection;
use rust_decimal::prelude::MathematicalOps;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::dates;
use crate::error::{AppError, AppResult};
use crate::money::{amount_to_minor, format_amount, pow10};
use crate::services::currency::{currency_meta, primary_currency};

const MAX_MONTHS: i64 = 1_200;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiInput {
    pub annual_expenses: String,
    pub withdrawal_rate: String,
    pub current_invested: String,
    pub monthly_contribution: String,
    pub expected_return: String,
    pub inflation: String,
    pub years_to_retirement: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiYearPoint {
    pub year: i64,
    pub balance_minor: i64,
    pub balance: String,
    pub target_minor: i64,
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FiReport {
    pub currency: String,
    pub currency_symbol: String,
    pub fi_number_minor: i64,
    pub fi_number: String,
    pub progress_percent: i64,
    pub remaining_minor: i64,
    pub remaining: String,
    pub months_to_fi: Option<i64>,
    pub years_to_fi: Option<String>,
    pub fi_date: Option<String>,
    pub real_return_percent: String,
    pub coast_number_minor: Option<i64>,
    pub coast_number: Option<String>,
    pub coast_progress_percent: Option<i64>,
    pub projected_at_retirement: Option<String>,
    pub annual_expenses: String,
    pub current_invested: String,
    pub monthly_contribution: String,
    pub projection: Vec<FiYearPoint>,
    pub notes: Vec<String>,
}

fn parse_percent(label: &str, raw: &str) -> AppResult<Decimal> {
    if raw.trim().is_empty() {
        return Err(AppError::Validation(format!("Enter {label}.")));
    }
    crate::money::parse_decimal(raw)
        .map_err(|_| AppError::Validation(format!("'{raw}' is not a valid {label}.")))
}

fn round_minor(value: Decimal, exp: u32) -> AppResult<i64> {
    amount_to_minor(value, exp)
}

pub fn calculate(conn: &Connection, tenant_id: &str, input: &FiInput) -> AppResult<FiReport> {
    let currency = primary_currency(conn, tenant_id)?;
    let meta = currency_meta(conn, &currency)?;
    let exp = meta.exponent;

    let annual_expenses = crate::money::parse_amount(&input.annual_expenses, exp)?;
    if annual_expenses <= 0 {
        return Err(AppError::Validation(
            "Annual expenses must be greater than zero.".into(),
        ));
    }
    let current_invested = crate::money::parse_amount(&input.current_invested, exp)?;
    if current_invested < 0 {
        return Err(AppError::Validation(
            "Current invested balance cannot be negative.".into(),
        ));
    }
    let monthly_contribution = crate::money::parse_amount(&input.monthly_contribution, exp)?;
    if monthly_contribution < 0 {
        return Err(AppError::Validation(
            "Monthly contribution cannot be negative.".into(),
        ));
    }

    let withdrawal_rate = parse_percent("withdrawal rate", &input.withdrawal_rate)?;
    if withdrawal_rate <= Decimal::ZERO || withdrawal_rate > Decimal::from(100) {
        return Err(AppError::Validation(
            "Withdrawal rate must be greater than 0 and no more than 100%.".into(),
        ));
    }
    let expected_return = parse_percent("expected return", &input.expected_return)?;
    let inflation = parse_percent("inflation", &input.inflation)?;
    if expected_return < Decimal::ZERO || expected_return > Decimal::from(100) {
        return Err(AppError::Validation(
            "Expected return must be between 0 and 100%.".into(),
        ));
    }
    if inflation < Decimal::ZERO || inflation > Decimal::from(100) {
        return Err(AppError::Validation(
            "Inflation must be between 0 and 100%.".into(),
        ));
    }
    if let Some(years) = input.years_to_retirement {
        if !(0..=80).contains(&years) {
            return Err(AppError::Validation(
                "Years to retirement must be between 0 and 80.".into(),
            ));
        }
    }

    let scale = Decimal::from(pow10(exp)?);
    let annual = Decimal::from(annual_expenses) / scale;
    let current = Decimal::from(current_invested) / scale;
    let contribution = Decimal::from(monthly_contribution) / scale;

    let fi_value = annual / (withdrawal_rate / Decimal::from(100));
    let fi_minor = round_minor(fi_value, exp)?;

    let progress = if fi_value > Decimal::ZERO {
        ((current / fi_value) * Decimal::from(100))
            .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
            .to_string()
            .parse::<i64>()
            .unwrap_or(0)
    } else {
        0
    };
    let remaining_value = (fi_value - current).max(Decimal::ZERO);
    let remaining_minor = round_minor(remaining_value, exp)?;

    let real_annual = ((Decimal::ONE + expected_return / Decimal::from(100))
        / (Decimal::ONE + inflation / Decimal::from(100)))
        - Decimal::ONE;
    let monthly_factor = (Decimal::ONE + real_annual).powd(Decimal::ONE / Decimal::from(12));
    let real_return_percent = (real_annual * Decimal::from(100))
        .round_dp_with_strategy(1, RoundingStrategy::MidpointAwayFromZero)
        .to_string();

    let mut months_to_fi: Option<i64> = None;
    let mut balance = current;
    if current >= fi_value {
        months_to_fi = Some(0);
    } else {
        for month in 1..=MAX_MONTHS {
            balance = balance * monthly_factor + contribution;
            if balance >= fi_value {
                months_to_fi = Some(month);
                break;
            }
        }
    }

    let today = dates::today_local();
    let fi_date =
        months_to_fi.map(|months| dates::iso(dates::add_months_clamped(today, months as i32)));
    let years_to_fi =
        months_to_fi.map(|months| format!("{:.1}", Decimal::from(months) / Decimal::from(12)));

    let years = input.years_to_retirement;
    let (coast_value, coast_minor, coast_progress) = match years {
        Some(years) if years >= 0 => {
            let growth = (Decimal::ONE + real_annual).powi(years);
            let coast = if growth > Decimal::ZERO {
                fi_value / growth
            } else {
                fi_value
            };
            let coast_minor = round_minor(coast, exp)?;
            let progress = if coast > Decimal::ZERO {
                Some(
                    ((current / coast) * Decimal::from(100))
                        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                        .to_string()
                        .parse::<i64>()
                        .unwrap_or(0),
                )
            } else {
                None
            };
            (Some(coast), Some(coast_minor), progress)
        }
        _ => (None, None, None),
    };

    let projected_at_retirement = match years {
        Some(years) => {
            let mut value = current;
            let months = years * 12;
            for _ in 0..months {
                value = value * monthly_factor + contribution;
            }
            Some(format_amount(round_minor(value, exp)?, exp))
        }
        None => None,
    };

    let horizon_years = years.unwrap_or(30).clamp(1, 60);
    let mut projection = Vec::new();
    let mut projected = current;
    for year in 0..=horizon_years {
        if year > 0 {
            for _ in 0..12 {
                projected = projected * monthly_factor + contribution;
            }
        }
        projection.push(FiYearPoint {
            year,
            balance_minor: round_minor(projected, exp)?,
            balance: format_amount(round_minor(projected, exp)?, exp),
            target_minor: fi_minor,
            target: format_amount(fi_minor, exp),
        });
    }

    let mut notes = vec![
        "This is a projection from the assumptions you enter, not a forecast and not a record of your accounts."
            .to_string(),
        "The real return is the expected return reduced by inflation. Contributions are assumed to stay level in today's money."
            .to_string(),
        "The FI number is annual expenses divided by the withdrawal rate. It is a target, not a guarantee.".to_string(),
    ];
    if months_to_fi.is_none() {
        notes.push(
            "With these assumptions the target is not reached within 100 years. Increase contributions, the return, or lower the target."
                .to_string(),
        );
    }
    if expected_return < inflation {
        notes.push(
            "Expected return is below inflation, so the real return is negative.".to_string(),
        );
    }
    if let Some(coast) = coast_value {
        if coast > fi_value {
            notes.push(
                "A negative real return makes the coast number larger than the FI number."
                    .to_string(),
            );
        }
    }

    Ok(FiReport {
        currency: currency.clone(),
        currency_symbol: meta.symbol,
        fi_number_minor: fi_minor,
        fi_number: format_amount(fi_minor, exp),
        progress_percent: progress.clamp(0, 999),
        remaining_minor,
        remaining: format_amount(remaining_minor, exp),
        months_to_fi,
        years_to_fi,
        fi_date,
        real_return_percent,
        coast_number_minor: coast_minor,
        coast_number: coast_minor.map(|value| format_amount(value, exp)),
        coast_progress_percent: coast_progress,
        projected_at_retirement,
        annual_expenses: format_amount(annual_expenses, exp),
        current_invested: format_amount(current_invested, exp),
        monthly_contribution: format_amount(monthly_contribution, exp),
        projection,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, open_test};

    fn setup() -> (Connection, String) {
        let mut conn = open_test();
        let (tenant, _user) = db::seed_if_empty(&mut conn).unwrap();
        (conn, tenant)
    }

    fn input(
        annual: &str,
        rate: &str,
        current: &str,
        monthly: &str,
        expected: &str,
        inflation: &str,
        years: Option<i64>,
    ) -> FiInput {
        FiInput {
            annual_expenses: annual.into(),
            withdrawal_rate: rate.into(),
            current_invested: current.into(),
            monthly_contribution: monthly.into(),
            expected_return: expected.into(),
            inflation: inflation.into(),
            years_to_retirement: years,
        }
    }

    #[test]
    fn fi_number_is_annual_expenses_divided_by_withdrawal_rate() {
        let (conn, tenant) = setup();
        let report = calculate(
            &conn,
            &tenant,
            &input("48000", "4", "0", "0", "0", "0", None),
        )
        .unwrap();
        assert_eq!(report.fi_number, "1,200,000.00");
        assert_eq!(report.remaining, "1,200,000.00");
        assert_eq!(report.progress_percent, 0);
    }

    #[test]
    fn zero_return_months_are_exact() {
        let (conn, tenant) = setup();
        let report = calculate(
            &conn,
            &tenant,
            &input("48000", "4", "0", "10000", "0", "0", None),
        )
        .unwrap();
        assert_eq!(report.months_to_fi, Some(120));
        assert_eq!(report.years_to_fi.as_deref(), Some("10.0"));
    }

    #[test]
    fn already_at_target_reports_zero_months() {
        let (conn, tenant) = setup();
        let report = calculate(
            &conn,
            &tenant,
            &input("48000", "4", "1300000", "5000", "5", "2", Some(10)),
        )
        .unwrap();
        assert_eq!(report.months_to_fi, Some(0));
        assert_eq!(report.progress_percent, 108);
        assert!(report.coast_number_minor.unwrap() < report.fi_number_minor);
        assert!(report.projected_at_retirement.is_some());
    }

    #[test]
    fn invalid_assumptions_are_rejected() {
        let (conn, tenant) = setup();
        assert!(calculate(&conn, &tenant, &input("0", "4", "0", "0", "0", "0", None)).is_err());
        assert!(calculate(
            &conn,
            &tenant,
            &input("48000", "0", "0", "0", "0", "0", None)
        )
        .is_err());
        assert!(calculate(
            &conn,
            &tenant,
            &input("48000", "4", "0", "0", "5", "2", Some(120))
        )
        .is_err());
    }
}
