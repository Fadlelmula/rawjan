use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;

use crate::dates::now_iso;
use crate::error::{AppError, AppResult};
use crate::money::{convert_minor, parse_rate, rate_to_string};

#[derive(Debug, Clone)]
pub struct CurrencyMeta {
    pub code: String,
    pub symbol: String,
    pub exponent: u32,
    pub name: String,
}

pub fn currency_meta(conn: &Connection, code: &str) -> AppResult<CurrencyMeta> {
    conn.query_row(
        "SELECT code, symbol, exponent, name FROM currencies WHERE code = ?1",
        [code],
        |row| {
            Ok(CurrencyMeta {
                code: row.get(0)?,
                symbol: row.get(1)?,
                exponent: row.get::<_, i64>(2)? as u32,
                name: row.get(3)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::Validation(format!("Unknown currency '{code}'.")))
}

pub fn exponent(conn: &Connection, code: &str) -> AppResult<u32> {
    Ok(currency_meta(conn, code)?.exponent)
}

pub fn primary_currency(conn: &Connection, tenant_id: &str) -> AppResult<String> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM tenant_settings WHERE tenant_id = ?1 AND key = 'primary_currency'",
            [tenant_id],
            |r| r.get(0),
        )
        .optional()?;
    let code = raw
        .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok())
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "QAR".to_string());
    Ok(code)
}

#[derive(Debug, Clone)]
pub struct RateSnapshot {
    pub id: Option<String>,
    pub rate: Decimal,
    pub as_of_date: String,
    pub source: String,
    pub inverted: bool,
}

/// Find the most recent immutable rate snapshot on or before `on_date`.
pub fn find_rate(
    conn: &Connection,
    tenant_id: &str,
    base_code: &str,
    quote_code: &str,
    on_date: &str,
) -> AppResult<Option<RateSnapshot>> {
    if base_code == quote_code {
        return Ok(Some(RateSnapshot {
            id: None,
            rate: Decimal::ONE,
            as_of_date: on_date.to_string(),
            source: "identity".into(),
            inverted: false,
        }));
    }
    let direct = conn
        .query_row(
            "SELECT id, rate, as_of_date, source FROM exchange_rates
             WHERE tenant_id = ?1 AND base_code = ?2 AND quote_code = ?3 AND as_of_date <= ?4
             ORDER BY as_of_date DESC, fetched_at DESC LIMIT 1",
            params![tenant_id, base_code, quote_code, on_date],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    if let Some((id, rate, date, source)) = direct {
        let rate = parse_rate(&rate)?;
        return Ok(Some(RateSnapshot { id: Some(id), rate, as_of_date: date, source, inverted: false }));
    }

    let inverse = conn
        .query_row(
            "SELECT id, rate, as_of_date, source FROM exchange_rates
             WHERE tenant_id = ?1 AND base_code = ?2 AND quote_code = ?3 AND as_of_date <= ?4
             ORDER BY as_of_date DESC, fetched_at DESC LIMIT 1",
            params![tenant_id, quote_code, base_code, on_date],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .optional()?;
    if let Some((id, rate, date, source)) = inverse {
        let rate = parse_rate(&rate)?;
        return Ok(Some(RateSnapshot {
            id: Some(id),
            rate: Decimal::ONE / rate,
            as_of_date: date,
            source,
            inverted: true,
        }));
    }
    Ok(None)
}

pub fn require_rate(
    conn: &Connection,
    tenant_id: &str,
    from: &str,
    to: &str,
    on_date: &str,
) -> AppResult<RateSnapshot> {
    find_rate(conn, tenant_id, from, to, on_date)?.ok_or_else(|| {
        AppError::Validation(format!(
            "No exchange rate from {from} to {to} on or before {on_date}. Record a manual rate in Settings → Currencies before using {from} amounts, so converted figures remain traceable."
        ))
    })
}

pub fn convert_to_currency(
    conn: &Connection,
    tenant_id: &str,
    amount_minor: i64,
    from_code: &str,
    to_code: &str,
    on_date: &str,
) -> AppResult<(i64, RateSnapshot)> {
    let from_exp = exponent(conn, from_code)?;
    let to_exp = exponent(conn, to_code)?;
    let snapshot = require_rate(conn, tenant_id, from_code, to_code, on_date)?;
    let converted = convert_minor(amount_minor, from_exp, to_exp, snapshot.rate);
    Ok((converted, snapshot))
}

/// Insert a new manual rate snapshot. Existing snapshots for the same day and source are preserved.
pub fn add_manual_rate(
    conn: &Connection,
    tenant_id: &str,
    base_code: &str,
    quote_code: &str,
    rate: Decimal,
    as_of_date: &str,
) -> AppResult<RateSnapshot> {
    if base_code == quote_code {
        return Err(AppError::Validation("A currency cannot be rated against itself.".into()));
    }
    if rate <= Decimal::ZERO {
        return Err(AppError::Validation("Exchange rate must be greater than zero.".into()));
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO exchange_rates (id, tenant_id, base_code, quote_code, rate, as_of_date, source, fetched_at, is_manual)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'manual', ?7, 1)",
        params![id, tenant_id, base_code, quote_code, rate_to_string(rate), as_of_date, now_iso()],
    )?;
    let (id, stored_rate, source): (String, String, String) = conn.query_row(
        "SELECT id, rate, source FROM exchange_rates
         WHERE tenant_id = ?1 AND base_code = ?2 AND quote_code = ?3 AND as_of_date = ?4 AND source = 'manual'",
        params![tenant_id, base_code, quote_code, as_of_date],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(RateSnapshot {
        id: Some(id),
        rate: parse_rate(&stored_rate)?,
        as_of_date: as_of_date.to_string(),
        source,
        inverted: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_test;
    use crate::dates::parse_date;

    fn seeded() -> (Connection, String) {
        let mut conn = open_test();
        let (tenant, _) = crate::db::seed_if_empty(&mut conn).unwrap();
        (conn, tenant)
    }

    #[test]
    fn identity_rate_is_exact() {
        let (conn, tenant) = seeded();
        let snapshot = require_rate(&conn, &tenant, "QAR", "QAR", "2026-01-01").unwrap();
        assert_eq!(snapshot.rate, Decimal::ONE);
        assert!(snapshot.id.is_none());
    }

    #[test]
    fn historical_rate_is_used_and_immutable() {
        let (conn, tenant) = seeded();
        add_manual_rate(&conn, &tenant, "USD", "QAR", parse_rate("3.64").unwrap(), "2026-01-10").unwrap();
        add_manual_rate(&conn, &tenant, "USD", "QAR", parse_rate("3.70").unwrap(), "2026-02-10").unwrap();

        let january = require_rate(&conn, &tenant, "USD", "QAR", "2026-01-31").unwrap();
        assert_eq!(january.rate, parse_rate("3.64").unwrap());
        let february = require_rate(&conn, &tenant, "USD", "QAR", "2026-02-15").unwrap();
        assert_eq!(february.rate, parse_rate("3.70").unwrap());
        // Adding a newer rate never rewrites history.
        add_manual_rate(&conn, &tenant, "USD", "QAR", parse_rate("3.80").unwrap(), "2026-03-01").unwrap();
        assert_eq!(
            require_rate(&conn, &tenant, "USD", "QAR", "2026-01-31").unwrap().rate,
            parse_rate("3.64").unwrap()
        );
    }

    #[test]
    fn inverse_rate_works() {
        let (conn, tenant) = seeded();
        add_manual_rate(&conn, &tenant, "USD", "QAR", parse_rate("4").unwrap(), "2026-01-10").unwrap();
        let inverse = require_rate(&conn, &tenant, "QAR", "USD", "2026-01-15").unwrap();
        assert_eq!(inverse.rate, parse_rate("0.25").unwrap());
        assert!(inverse.inverted);
    }

    #[test]
    fn missing_rate_is_an_error() {
        let (conn, tenant) = seeded();
        let err = require_rate(&conn, &tenant, "USD", "QAR", "2026-01-01").unwrap_err();
        assert!(err.to_string().contains("No exchange rate"));
        let _ = parse_date("2026-01-01").unwrap();
    }
}
