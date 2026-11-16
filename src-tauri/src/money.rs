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

pub fn pow10(exponent: u32) -> AppResult<i64> {
    10i64
        .checked_pow(exponent)
        .ok_or_else(|| AppError::Validation("Currency exponent is not supported.".into()))
}

/// Validate a human-entered decimal and return it in a canonical form that
/// `Decimal::from_str` accepts. Characters are never stripped: grouping commas
/// are allowed only in groups of three, there may be one decimal point, and the
/// sign may appear only at the front. Anything else is rejected outright.
fn normalize_number(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (sign, rest) = match trimmed.as_bytes()[0] {
        b'-' => ("-", &trimmed[1..]),
        b'+' => ("", &trimmed[1..]),
        _ => ("", trimmed),
    };
    if rest.is_empty() {
        return None;
    }

    let mut parts = rest.split('.');
    let whole = parts.next()?;
    let fraction = parts.next();
    if parts.next().is_some() {
        return None;
    }
    if let Some(frac) = fraction {
        if frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }

    // A bare fraction such as ".5" is unambiguous and becomes "0.5".
    let whole_digits = if whole.is_empty() {
        fraction?;
        "0".to_string()
    } else {
        normalize_whole(whole)?
    };

    Some(match fraction {
        Some(frac) => format!("{sign}{whole_digits}.{frac}"),
        None => format!("{sign}{whole_digits}"),
    })
}

fn normalize_whole(whole: &str) -> Option<String> {
    if !whole.contains(',') {
        return if whole.bytes().all(|b| b.is_ascii_digit()) {
            Some(whole.to_string())
        } else {
            None
        };
    }
    let groups: Vec<&str> = whole.split(',').collect();
    let first = groups[0];
    if first.is_empty() || first.len() > 3 || !first.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    for group in &groups[1..] {
        if group.len() != 3 || !group.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    Some(groups.concat())
}

/// Parse a strictly validated decimal. No floating point is involved.
pub fn parse_decimal(input: &str) -> AppResult<Decimal> {
    let normalized = normalize_number(input)
        .ok_or_else(|| AppError::Validation(format!("'{input}' is not a valid number.")))?;
    Decimal::from_str(&normalized)
        .map_err(|_| AppError::Validation(format!("'{input}' is not a valid number.")))
}

/// Parse a human-entered amount into minor units using half-away-from-zero rounding.
/// Input is never evaluated as floating point, and invalid characters are never
/// silently removed.
pub fn parse_amount(input: &str, exponent: u32) -> AppResult<i64> {
    if input.trim().is_empty() {
        return Err(AppError::Validation("Enter an amount.".into()));
    }
    let value = parse_decimal(input)
        .map_err(|_| AppError::Validation(format!("'{input}' is not a valid amount.")))?;
    amount_to_minor(value, exponent)
}

pub fn amount_to_minor(value: Decimal, exponent: u32) -> AppResult<i64> {
    let scaled = value
        .checked_mul(Decimal::from(pow10(exponent)?))
        .ok_or_else(|| AppError::Validation("Amount is too large.".into()))?;
    let rounded = scaled.round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero);
    rounded
        .try_into()
        .map_err(|_| AppError::Validation("Amount is too large or too precise.".into()))
}

pub fn to_decimal(minor: i64, exponent: u32) -> Decimal {
    Decimal::from_i128_with_scale(minor as i128, exponent)
}

pub fn parse_rate(input: &str) -> AppResult<Decimal> {
    let rate = parse_decimal(input)
        .map_err(|_| AppError::Validation(format!("'{input}' is not a valid exchange rate.")))?;
    if rate <= Decimal::ZERO {
        return Err(AppError::Validation(
            "Exchange rate must be greater than zero.".into(),
        ));
    }
    Ok(rate)
}

pub fn rate_to_string(rate: Decimal) -> String {
    rate.normalize().to_string()
}

/// Convert minor units between currencies: 1 unit of `from` = `rate` units of `to`.
/// Overflow is reported rather than saturated, so a broken rate can never turn
/// into a silently wrong balance.
pub fn convert_minor(
    amount_minor: i64,
    from_exp: u32,
    to_exp: u32,
    rate: Decimal,
) -> AppResult<i64> {
    let value = to_decimal(amount_minor, from_exp)
        .checked_mul(rate)
        .ok_or_else(|| AppError::Validation("Converted amount is too large.".into()))?;
    let scaled = value
        .checked_mul(Decimal::from(pow10(to_exp)?))
        .ok_or_else(|| AppError::Validation("Converted amount is too large.".into()))?;
    scaled
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_i64()
        .ok_or_else(|| AppError::Validation("Converted amount is too large.".into()))
}

/// Multiply and add in one checked step: `value * factor + addend`.
/// Projection loops use this so extreme assumptions produce a validation
/// error instead of a decimal overflow panic.
pub fn checked_growth_step(value: Decimal, factor: Decimal, addend: Decimal) -> AppResult<Decimal> {
    value
        .checked_mul(factor)
        .and_then(|product| product.checked_add(addend))
        .ok_or_else(|| {
            AppError::Validation("The projection exceeds the supported numeric range.".into())
        })
}

/// Sum minor-unit amounts, reporting overflow instead of wrapping.
pub fn checked_sum(values: impl Iterator<Item = i64>) -> AppResult<i64> {
    let mut total = 0i64;
    for value in values {
        total = total
            .checked_add(value)
            .ok_or_else(|| AppError::Validation("The total exceeds the supported range.".into()))?;
    }
    Ok(total)
}

/// Difference between two minor-unit amounts, reporting overflow.
pub fn checked_diff(a: i64, b: i64) -> AppResult<i64> {
    a.checked_sub(b)
        .ok_or_else(|| AppError::Validation("The total exceeds the supported range.".into()))
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
        if i > 0 && (whole_str.len() - i).is_multiple_of(3) {
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
        assert_eq!(convert_minor(12345, 2, 2, rate).unwrap(), 44936);
        let rate = parse_rate("0.274725").unwrap();
        assert_eq!(convert_minor(10000, 2, 2, rate).unwrap(), 2747);
    }

    #[test]
    fn rejects_invalid_input() {
        assert!(parse_amount("", 2).is_err());
        assert!(parse_amount("abc", 2).is_err());
        assert!(parse_rate("0").is_err());
        assert!(parse_rate("-1.2").is_err());
    }

    #[test]
    fn accepts_well_formed_amounts() {
        assert_eq!(parse_amount("1,234,567.89", 2).unwrap(), 123456789);
        assert_eq!(parse_amount("  42.10  ", 2).unwrap(), 4210);
        assert_eq!(parse_amount(".5", 2).unwrap(), 50);
        assert_eq!(parse_amount("-.5", 2).unwrap(), -50);
        assert_eq!(parse_amount("+12.34", 2).unwrap(), 1234);
        assert_eq!(parse_amount("1,000", 0).unwrap(), 1000);
        assert_eq!(
            parse_rate("3.6400").unwrap(),
            Decimal::from_str("3.64").unwrap()
        );
    }

    #[test]
    fn never_strips_invalid_characters() {
        for bad in [
            "12abc34", "1,23", "12,34.56", "1,2345", "1,234,", ",123", "1.2.3", "$100", "100 USD",
            "1 234", "1e3", "1.", "-", "+", "--1", "1,-234", "NaN", "inf", "1_000",
        ] {
            assert!(parse_amount(bad, 2).is_err(), "should reject {bad:?}");
            assert!(parse_rate(bad).is_err(), "rate should reject {bad:?}");
        }
        assert_eq!(parse_amount("1,234.56", 2).unwrap(), 123456);
    }

    #[test]
    fn rejects_out_of_range_amounts_instead_of_saturating() {
        assert!(parse_amount("99999999999999999999999", 2).is_err());
        assert!(amount_to_minor(Decimal::MAX, 2).is_err());
        assert!(pow10(20).is_err());
        assert_eq!(pow10(3).unwrap(), 1000);
        let huge = Decimal::from(1_000_000);
        assert!(convert_minor(i64::MAX, 2, 2, huge).is_err());
        assert!(convert_minor(i64::MIN, 2, 2, huge).is_err());
        // A value that still fits keeps working exactly.
        assert_eq!(
            convert_minor(i64::MAX, 2, 2, Decimal::ONE).unwrap(),
            i64::MAX
        );
    }

    #[test]
    fn decimal_arithmetic_is_exact() {
        let tenth = Decimal::from_str("0.1").unwrap();
        let twentieth = Decimal::from_str("0.2").unwrap();
        assert_eq!((tenth + twentieth).to_string(), "0.3");
    }

    #[test]
    fn checked_totals_report_overflow_instead_of_wrapping() {
        assert_eq!(checked_sum([i64::MAX, 0].into_iter()).unwrap(), i64::MAX);
        assert!(checked_sum([i64::MAX, 1].into_iter()).is_err());
        assert_eq!(checked_diff(5, 3).unwrap(), 2);
        assert!(checked_diff(i64::MIN, 1).is_err());
    }

    #[test]
    fn formatting_and_parsing_round_trip_across_the_supported_range() {
        // Deterministic property sweep: every formatted amount parses back to
        // the same minor units for all supported exponents and signs.
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state
        };
        for exponent in 0..=4u32 {
            for index in 0..500u64 {
                let magnitude = (next() >> 11) as i64 % 100_000_000_000;
                let minor = if index % 2 == 0 {
                    magnitude
                } else {
                    -magnitude
                };
                let formatted = format_amount(minor, exponent);
                let parsed = parse_amount(&formatted, exponent).unwrap();
                assert_eq!(
                    parsed, minor,
                    "round trip failed for {formatted} (exp {exponent})"
                );
            }
        }
    }
}
