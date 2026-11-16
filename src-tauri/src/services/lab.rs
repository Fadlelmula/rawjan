use rusqlite::Connection;
use rust_decimal::prelude::MathematicalOps;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::money::{amount_to_minor, format_amount, parse_amount, pow10};

use crate::services::currency::{currency_meta, primary_currency};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompoundInput {
    pub initial: String,
    pub monthly_contribution: String,
    pub annual_return: String,
    pub years: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompoundPoint {
    pub year: i64,
    pub balance_minor: i64,
    pub balance: String,
    pub contributed_minor: i64,
    pub contributed: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompoundReport {
    pub currency: String,
    pub currency_symbol: String,
    pub final_minor: i64,
    pub final_amount: String,
    pub contributed_minor: i64,
    pub contributed: String,
    pub growth_minor: i64,
    pub growth: String,
    pub points: Vec<CompoundPoint>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanInput {
    pub principal: String,
    pub annual_rate: String,
    pub months: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanRow {
    pub month: i64,
    pub payment_minor: i64,
    pub payment: String,
    pub interest_minor: i64,
    pub interest: String,
    pub principal_minor: i64,
    pub principal: String,
    pub balance_minor: i64,
    pub balance: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoanReport {
    pub currency: String,
    pub currency_symbol: String,
    pub monthly_payment_minor: i64,
    pub monthly_payment: String,
    pub total_paid_minor: i64,
    pub total_paid: String,
    pub total_interest_minor: i64,
    pub total_interest: String,
    pub months: i64,
    pub rows: Vec<LoanRow>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InflationInput {
    pub amount: String,
    pub annual_inflation: String,
    pub years: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InflationReport {
    pub currency: String,
    pub currency_symbol: String,
    pub future_cost_minor: i64,
    pub future_cost: String,
    pub real_value_minor: i64,
    pub real_value: String,
    pub increase_minor: i64,
    pub increase: String,
    pub notes: Vec<String>,
}

fn parse_percent(label: &str, raw: &str, max: i64) -> AppResult<Decimal> {
    if raw.trim().is_empty() {
        return Err(AppError::Validation(format!("Enter {label}.")));
    }
    let value = crate::money::parse_decimal(raw)
        .map_err(|_| AppError::Validation(format!("'{raw}' is not a valid {label}.")))?;
    if value < Decimal::ZERO || value > Decimal::from(max) {
        return Err(AppError::Validation(format!(
            "{label} must be between 0 and {max}%."
        )));
    }
    Ok(value)
}

fn minor_of(value: Decimal, exp: u32) -> AppResult<i64> {
    amount_to_minor(value, exp)
}

pub fn compound(
    conn: &Connection,
    tenant_id: &str,
    input: &CompoundInput,
) -> AppResult<CompoundReport> {
    let currency = primary_currency(conn, tenant_id)?;
    let meta = currency_meta(conn, &currency)?;
    let exp = meta.exponent;

    if !(1..=60).contains(&input.years) {
        return Err(AppError::Validation(
            "Years must be between 1 and 60.".into(),
        ));
    }
    let initial = parse_amount(&input.initial, exp)?;
    let monthly = parse_amount(&input.monthly_contribution, exp)?;
    if initial < 0 || monthly < 0 {
        return Err(AppError::Validation(
            "Starting balance and monthly contribution cannot be negative.".into(),
        ));
    }
    let annual = parse_percent("annual return", &input.annual_return, 100)?;

    let scale = Decimal::from(pow10(exp)?);
    let mut balance = Decimal::from(initial) / scale;
    let contribution = Decimal::from(monthly) / scale;
    let monthly_rate = annual / Decimal::from(100) / Decimal::from(12);

    let mut points = Vec::new();
    let mut contributed = Decimal::from(initial) / scale;
    points.push(CompoundPoint {
        year: 0,
        balance_minor: initial,
        balance: format_amount(initial, exp),
        contributed_minor: initial,
        contributed: format_amount(initial, exp),
    });

    let growth_factor = Decimal::ONE.checked_add(monthly_rate).ok_or_else(|| {
        AppError::Validation("The projection exceeds the supported numeric range.".into())
    })?;

    for year in 1..=input.years {
        for _ in 0..12 {
            balance = crate::money::checked_growth_step(balance, growth_factor, contribution)?;
            contributed = contributed.checked_add(contribution).ok_or_else(|| {
                AppError::Validation("The projection exceeds the supported numeric range.".into())
            })?;
        }
        let balance_minor = minor_of(balance, exp)?;
        let contributed_minor = minor_of(contributed, exp)?;
        points.push(CompoundPoint {
            year,
            balance_minor,
            balance: format_amount(balance_minor, exp),
            contributed_minor,
            contributed: format_amount(contributed_minor, exp),
        });
    }

    let final_minor = minor_of(balance, exp)?;
    let contributed_minor = minor_of(contributed, exp)?;
    let growth_minor = crate::money::checked_diff(final_minor, contributed_minor)?;

    Ok(CompoundReport {
        currency: currency.clone(),
        currency_symbol: meta.symbol,
        final_minor,
        final_amount: format_amount(final_minor, exp),
        contributed_minor,
        contributed: format_amount(contributed_minor, exp),
        growth_minor,
        growth: format_amount(growth_minor, exp),
        points,
        notes: vec![
            "Returns are applied monthly at one twelfth of the annual rate. Compounding is simplified and does not model taxes, fees or market volatility.".to_string(),
            "Contributions are assumed to be made at the end of each month and to stay level.".to_string(),
        ],
    })
}

pub fn loan(conn: &Connection, tenant_id: &str, input: &LoanInput) -> AppResult<LoanReport> {
    let currency = primary_currency(conn, tenant_id)?;
    let meta = currency_meta(conn, &currency)?;
    let exp = meta.exponent;

    if !(1..=600).contains(&input.months) {
        return Err(AppError::Validation(
            "Term must be between 1 and 600 months.".into(),
        ));
    }
    let principal = parse_amount(&input.principal, exp)?;
    if principal <= 0 {
        return Err(AppError::Validation(
            "Loan principal must be greater than zero.".into(),
        ));
    }
    let annual = parse_percent("annual interest rate", &input.annual_rate, 100)?;

    let scale = Decimal::from(pow10(exp)?);
    let principal_value = Decimal::from(principal) / scale;
    let monthly_rate = annual / Decimal::from(100) / Decimal::from(12);

    let payment = if monthly_rate == Decimal::ZERO {
        principal_value / Decimal::from(input.months)
    } else {
        let factor = (Decimal::ONE + monthly_rate).powi(input.months);
        principal_value * monthly_rate * factor / (factor - Decimal::ONE)
    };
    let payment_minor = minor_of(
        payment.round_dp_with_strategy(exp, RoundingStrategy::MidpointAwayFromZero),
        exp,
    )?
    .max(1);

    let mut rows = Vec::new();
    let mut balance_minor_dec = Decimal::from(principal);
    let mut total_interest = 0i64;
    let mut total_paid = 0i64;

    for month in 1..=input.months {
        let interest = minor_of(
            (balance_minor_dec / scale * monthly_rate)
                .round_dp_with_strategy(exp, RoundingStrategy::MidpointAwayFromZero),
            exp,
        )?;
        let is_last = month == input.months;
        let current_balance = minor_of(balance_minor_dec.max(Decimal::ZERO), exp)?;
        let principal_minor = if is_last {
            current_balance
        } else {
            (payment_minor - interest).clamp(0, current_balance)
        };
        let paid = if is_last {
            crate::money::checked_sum([principal_minor, interest].into_iter())?
        } else {
            payment_minor
        };
        balance_minor_dec = balance_minor_dec
            .checked_sub(Decimal::from(principal_minor))
            .ok_or_else(|| {
                AppError::Validation(
                    "The loan schedule exceeds the supported numeric range.".into(),
                )
            })?;
        let remaining = minor_of(balance_minor_dec.max(Decimal::ZERO), exp)?;

        total_interest = crate::money::checked_sum([total_interest, interest].into_iter())?;
        total_paid = crate::money::checked_sum([total_paid, paid].into_iter())?;

        rows.push(LoanRow {
            month,
            payment_minor: paid,
            payment: format_amount(paid, exp),
            interest_minor: interest,
            interest: format_amount(interest, exp),
            principal_minor,
            principal: format_amount(principal_minor, exp),
            balance_minor: remaining,
            balance: format_amount(remaining, exp),
        });
    }

    Ok(LoanReport {
        currency: currency.clone(),
        currency_symbol: meta.symbol,
        monthly_payment_minor: payment_minor,
        monthly_payment: format_amount(payment_minor, exp),
        total_paid_minor: total_paid,
        total_paid: format_amount(total_paid, exp),
        total_interest_minor: total_interest,
        total_interest: format_amount(total_interest, exp),
        months: input.months,
        rows,
        notes: vec![
            "The payment is calculated with the standard fixed-rate amortisation formula and rounded to the currency's minor unit. The final payment clears the remaining balance.".to_string(),
            "No fees, insurance, taxes or rate changes are modelled.".to_string(),
        ],
    })
}

pub fn inflation(
    conn: &Connection,
    tenant_id: &str,
    input: &InflationInput,
) -> AppResult<InflationReport> {
    let currency = primary_currency(conn, tenant_id)?;
    let meta = currency_meta(conn, &currency)?;
    let exp = meta.exponent;

    if !(0..=80).contains(&input.years) {
        return Err(AppError::Validation(
            "Years must be between 0 and 80.".into(),
        ));
    }
    let amount = parse_amount(&input.amount, exp)?;
    if amount <= 0 {
        return Err(AppError::Validation(
            "Amount must be greater than zero.".into(),
        ));
    }
    let rate = parse_percent("annual inflation", &input.annual_inflation, 100)?;

    let scale = Decimal::from(pow10(exp)?);
    let value = Decimal::from(amount) / scale;
    let factor = (Decimal::ONE + rate / Decimal::from(100)).powi(input.years);
    let future = value * factor;
    let real = if factor > Decimal::ZERO {
        value / factor
    } else {
        value
    };

    let future_minor = minor_of(future, exp)?;
    let real_minor = minor_of(real, exp)?;

    Ok(InflationReport {
        currency: currency.clone(),
        currency_symbol: meta.symbol,
        future_cost_minor: future_minor,
        future_cost: format_amount(future_minor, exp),
        real_value_minor: real_minor,
        real_value: format_amount(real_minor, exp),
        increase_minor: future_minor - amount,
        increase: format_amount(future_minor - amount, exp),
        notes: vec![
            "Future cost compounds the entered inflation rate yearly. Real value discounts a future amount back to today's money.".to_string(),
            "Inflation is assumed to be constant. Actual inflation varies by category and over time.".to_string(),
        ],
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

    fn compound_input(initial: &str, monthly: &str, rate: &str, years: i64) -> CompoundInput {
        CompoundInput {
            initial: initial.into(),
            monthly_contribution: monthly.into(),
            annual_return: rate.into(),
            years,
        }
    }

    #[test]
    fn extreme_compound_growth_errors_instead_of_panicking() {
        let (conn, tenant) = setup();
        // Near-i64::MAX balances compounding at 100% for 60 years overflow the
        // supported decimal range; expect an error, not a panic.
        let result = compound(
            &conn,
            &tenant,
            &compound_input("92233720368547758.07", "92233720368547758.07", "100", 60),
        );
        assert!(result.is_err());
    }

    #[test]
    fn zero_return_compound_is_contributions_only() {
        let (conn, tenant) = setup();
        let report = compound(&conn, &tenant, &compound_input("1000", "100", "0", 2)).unwrap();
        assert_eq!(report.final_minor, 100_000 + 2 * 12 * 10_000);
        assert_eq!(report.growth_minor, 0);
        assert_eq!(report.points.len(), 3);
    }

    #[test]
    fn loan_with_zero_interest_splits_evenly() {
        let (conn, tenant) = setup();
        let report = loan(
            &conn,
            &tenant,
            &LoanInput {
                principal: "1200".into(),
                annual_rate: "0".into(),
                months: 12,
            },
        )
        .unwrap();
        assert_eq!(report.monthly_payment_minor, 10_000);
        assert_eq!(report.total_interest_minor, 0);
        assert_eq!(report.rows.len(), 12);
        assert_eq!(report.rows[11].balance_minor, 0);
    }

    #[test]
    fn loan_with_interest_has_positive_interest_and_clears_balance() {
        let (conn, tenant) = setup();
        let report = loan(
            &conn,
            &tenant,
            &LoanInput {
                principal: "100000".into(),
                annual_rate: "6".into(),
                months: 60,
            },
        )
        .unwrap();
        assert!(report.total_interest_minor > 0);
        assert_eq!(report.rows[59].balance_minor, 0);
        assert_eq!(
            report.total_paid_minor,
            report.monthly_payment_minor * 59 + report.rows[59].payment_minor
        );
    }

    #[test]
    fn inflation_moves_value_in_both_directions() {
        let (conn, tenant) = setup();
        let report = inflation(
            &conn,
            &tenant,
            &InflationInput {
                amount: "100".into(),
                annual_inflation: "10".into(),
                years: 2,
            },
        )
        .unwrap();
        assert_eq!(report.future_cost_minor, 12_100);
        assert_eq!(report.real_value_minor, 8_264);
        assert_eq!(report.increase_minor, 2_100);
    }

    #[test]
    fn invalid_lab_inputs_are_rejected() {
        let (conn, tenant) = setup();
        assert!(compound(&conn, &tenant, &compound_input("0", "0", "0", 0)).is_err());
        assert!(loan(
            &conn,
            &tenant,
            &LoanInput {
                principal: "0".into(),
                annual_rate: "5".into(),
                months: 12,
            }
        )
        .is_err());
        assert!(inflation(
            &conn,
            &tenant,
            &InflationInput {
                amount: "100".into(),
                annual_inflation: "2".into(),
                years: 100,
            }
        )
        .is_err());
    }
}
