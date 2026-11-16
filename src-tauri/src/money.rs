use std::str::FromStr;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// A safe, display-ready money value. Financial math never happens in the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct MoneyView {
    pub minor: i64,
    pub currency: String,
    pub amount: String,
}

impl MoneyView {
    pub fn new(minor: i64, currency: &str, exponent: u32) -> Self {
        Self {
            minor,
            currency: currency.to_string(),
            amount: format_amount(minor, exponent),
        }
    }
}

pub fn pow10(exponent: u32) -> i64 {
    10i64.pow(exponent)
}

/// Parse a human-entered amount into minor units using half-away-from-zero rounding.
/// Input is never evaluated as floating point.
pub fn parse_amount(input: &str, exponent: u32) -> AppResult<i64> {
    let cleaned: String = input
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
        .collect();
    if cleaned.trim().is_empty() {
        return Err(AppError::Validation("Enter an amount.".into()));
    }
    let value = Decimal::from_str(&cleaned)
        .map_err(|_| AppError::Validation(format!("'{input}' is not a valid amount.")))?;
    amount_to_minor(value, exponent)
}

pub fn amount_to_minor(value: Decimal, exponent: u32) -> AppResult<i64> {
    let scaled = value * Decimal::from(pow10(exponent));
    let rounded = scaled.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    rounded
        .try_into()
        .map_err(|_| AppError::Validation("Amount is too large or too precise.".into()))
}

pub fn to_decimal(minor: i64, exponent: u32) -> Decimal {
    Decimal::from_i128_with_scale(minor as i128, exponent)
}

pub fn parse_rate(input: &str) -> AppResult<Decimal> {
    let cleaned: String = input
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
        .collect();
    let rate = Decimal::from_str(&cleaned)
        .map_err(|_| AppError::Validation(format!("'{input}' is not a valid exchange rate.")))?;
    if rate <= Decimal::ZERO {
        return Err(AppError::Validation("Exchange rate must be greater than zero.".into()));
    }
    Ok(rate)
}

pub fn rate_to_string(rate: Decimal) -> String {
    rate.normalize().to_string()
}

/// Convert minor units between currencies: 1 unit of `from` = `rate` units of `to`.
pub fn convert_minor(
    amount_minor: i64,
    from_exp: u32,
    to_exp: u32,
    rate: Decimal,
) -> i64 {
    let value = to_decimal(amount_minor, from_exp) * rate;
    let scaled = value * Decimal::from(pow10(to_exp));
    scaled
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .unwrap_or_else(|| {
            if amount_minor < 0 {
                i64::MIN
            } else {
                i64::MAX
            }
        })
}

pub fn format_amount(minor: i64, exponent: u32) -> String {
    let exponent = exponent.min(4);
    let negative = minor < 0;
    let absolute = minor.unsigned_abs();
    let factor = 10u64.pow(exponent);
    let whole = absolute / factor;
    let fraction = absolute % factor;

    let whole_str = whole.to_string();
    let mut grouped = String::with_capacity(whole_str.len() + whole_str.len() / 3);
    for (i, ch) in whole_str.chars().enumerate() {
        if i > 0 && (whole_str.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&grouped);
    if exponent > 0 {
        out.push('.');
        out.push_str(&format!("{:0width$}", fraction, width = exponent as usize));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_two_decimal_currency() {
        assert_eq!(parse_amount("1,234.56", 2).unwrap(), 123456);
        assert_eq!(parse_amount("0.05", 2).unwrap(), 5);
        assert_eq!(parse_amount("-12.30", 2).unwrap(), -1230);
    }

    #[test]
    fn rounds_half_away_from_zero() {
        assert_eq!(parse_amount("1.005", 2).unwrap(), 101);
        assert_eq!(parse_amount("1.004", 2).unwrap(), 100);
        assert_eq!(parse_amount("-1.005", 2).unwrap(), -101);
    }

    #[test]
    fn parses_three_decimal_currency() {
        assert_eq!(parse_amount("12.345", 3).unwrap(), 12345);
        assert_eq!(format_amount(12345, 3), "12.345");
    }

    #[test]
    fn formats_with_grouping() {
        assert_eq!(format_amount(123456789, 2), "1,234,567.89");
        assert_eq!(format_amount(-5, 2), "-0.05");
        assert_eq!(format_amount(1000, 0), "1,000");
    }

    #[test]
    fn converts_without_floating_point() {
        let rate = parse_rate("3.64").unwrap();
        assert_eq!(convert_minor(12345, 2, 2, rate), 44936);
        let rate = parse_rate("0.274725").unwrap();
        assert_eq!(convert_minor(10000, 2, 2, rate), 2747);
    }

    #[test]
    fn rejects_invalid_input() {
        assert!(parse_amount("", 2).is_err());
        assert!(parse_amount("abc", 2).is_err());
        assert!(parse_rate("0").is_err());
        assert!(parse_rate("-1.2").is_err());
    }

    #[test]
    fn decimal_arithmetic_is_exact() {
        let tenth = Decimal::from_str("0.1").unwrap();
        let twentieth = Decimal::from_str("0.2").unwrap();
        assert_eq!((tenth + twentieth).to_string(), "0.3");
    }
}
