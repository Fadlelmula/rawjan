use std::collections::HashMap;


use rust_decimal::{Decimal, RoundingStrategy};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::dates::{self, iso, today_local};
use crate::error::{AppError, AppResult};
use crate::money::{amount_to_minor, format_amount, parse_amount, to_decimal};
use crate::services::currency::{currency_meta, primary_currency};

const ASSET_CLASSES: &[&str] = &[
    "stock",
    "etf",
    "bond",
    "gold",
    "silver",
    "crypto",
    "real_estate",
    "cash_deposit",
    "other",
];
const SIDES: &[&str] = &["buy", "sell"];
const SCALE: u32 = 8;

#[derive(Debug, Clone)]
struct TradeRecord {
    id: String,
    account_id: String,
    account_name: String,
    instrument_id: String,
    symbol: String,
    name: String,
    asset_class: String,
    currency: String,
    side: String,
    trade_date: String,
    quantity: Decimal,
    price: Decimal,
    fee_minor: i64,
    notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldingDto {
    pub account_id: String,
    pub account_name: String,
    pub instrument_id: String,
    pub symbol: String,
    pub name: String,
    pub asset_class: String,
    pub currency: String,
    pub quantity: String,
    pub cost_basis_minor: i64,
    pub cost_basis: String,
    pub average_cost: Option<String>,
    pub market_value_minor: Option<i64>,
    pub market_value: Option<String>,
    pub valuation_date: Option<String>,
    pub unrealized_minor: Option<i64>,
    pub unrealized: Option<String>,
    pub realized_minor: i64,
    pub realized: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentDto {
    pub id: String,
    pub symbol: String,
    pub name: String,
    pub asset_class: String,
    pub currency: String,
    pub currency_symbol: String,
    pub exchange: Option<String>,
    pub isin: Option<String>,
    pub notes: Option<String>,
    pub trade_count: i64,
    pub holding_count: i64,
    pub quantity: String,
    pub cost_basis_minor: i64,
    pub cost_basis: String,
    pub market_value_minor: Option<i64>,
    pub market_value: Option<String>,
    pub unrealized_minor: Option<i64>,
    pub unrealized: Option<String>,
    pub realized_minor: i64,
    pub realized: String,
    pub valued_holdings: i64,
    pub holdings: Vec<HoldingDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeDto {
    pub id: String,
    pub account_id: String,
    pub account_name: String,
    pub instrument_id: String,
    pub symbol: String,
    pub instrument_name: String,
    pub side: String,
    pub trade_date: String,
    pub quantity: String,
    pub price: String,
    pub fee_minor: i64,
    pub fee: String,
    pub total_minor: i64,
    pub total: String,
    pub currency: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValuationDto {
    pub id: String,
    pub account_id: String,
    pub account_name: String,
    pub instrument_id: Option<String>,
    pub symbol: Option<String>,
    pub as_of_date: String,
    pub value_minor: i64,
    pub value: String,
    pub currency: String,
    pub source: String,
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencySummary {
    pub currency: String,
    pub symbol: String,
    pub cost_basis_minor: i64,
    pub cost_basis: String,
    pub market_value_minor: i64,
    pub market_value: String,
    pub realized_minor: i64,
    pub realized: String,
    pub holdings: i64,
    pub valued_holdings: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortfolioSummary {
    pub instrument_count: i64,
    pub trade_count: i64,
    pub valuation_count: i64,
    pub currencies: Vec<CurrencySummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentInput {
    pub symbol: String,
    pub name: String,
    pub asset_class: String,
    pub currency_code: Option<String>,
    pub exchange: Option<String>,
    pub isin: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeInput {
    pub account_id: String,
    pub instrument_id: String,
    pub side: String,
    pub trade_date: String,
    pub quantity: String,
    pub price: String,
    pub fee: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValuationInput {
    pub account_id: String,
    pub instrument_id: String,
    pub as_of_date: String,
    pub value: String,
    pub source_ref: Option<String>,
}

fn parse_scaled(input: &str, scale: u32) -> AppResult<i64> {
    if input.trim().is_empty() {
        return Err(AppError::Validation("Enter a number.".into()));
    }
    let value = crate::money::parse_decimal(input)?;
    let scaled = value
        .checked_mul(Decimal::from(crate::money::pow10(scale)?))
        .ok_or_else(|| AppError::Validation("Number is too large.".into()))?;
    scaled
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .try_into()
        .map_err(|_| AppError::Validation("Number is too large or too precise.".into()))
}

fn format_scaled(scaled: i64, scale: u32) -> String {
    Decimal::from_i128_with_scale(scaled as i128, scale)
        .normalize()
        .to_string()
}

fn validate_account(conn: &Connection, tenant_id: &str, account_id: &str) -> AppResult<(String, String)> {
    conn.query_row(
        "SELECT name, currency_code FROM accounts WHERE id = ?1 AND tenant_id = ?2",
        params![account_id, tenant_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("Account not found.".into()))
}

fn validate_instrument(
    conn: &Connection,
    tenant_id: &str,
    instrument_id: &str,
) -> AppResult<(String, String, String, String)> {
    conn.query_row(
        "SELECT symbol, name, asset_class, currency_code FROM instruments WHERE id = ?1 AND tenant_id = ?2",
        params![instrument_id, tenant_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("Instrument not found.".into()))
}

fn load_trades(
    conn: &Connection,
    tenant_id: &str,
    account_id: Option<&str>,
    instrument_id: Option<&str>,
    exclude_id: Option<&str>,
) -> AppResult<Vec<TradeRecord>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.account_id, a.name, t.instrument_id, i.symbol, i.name, i.asset_class, i.currency_code,
                t.side, t.trade_date, t.quantity_scaled, t.price_scaled, t.price_scale, t.fee_minor, t.notes
         FROM investment_trades t
         JOIN accounts a ON a.id = t.account_id
         JOIN instruments i ON i.id = t.instrument_id
         WHERE t.tenant_id = ?1
           AND (?2 IS NULL OR t.account_id = ?2)
           AND (?3 IS NULL OR t.instrument_id = ?3)
           AND (?4 IS NULL OR t.id <> ?4)
         ORDER BY t.trade_date, t.created_at",
    )?;
    let rows = stmt
        .query_map(
            params![tenant_id, account_id, instrument_id, exclude_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, i64>(12)?,
                    row.get::<_, i64>(13)?,
                    row.get::<_, Option<String>>(14)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                account_id,
                account_name,
                instrument_id,
                symbol,
                name,
                asset_class,
                currency,
                side,
                trade_date,
                quantity_scaled,
                price_scaled,
                price_scale,
                fee_minor,
                notes,
            )| {
                let quantity = Decimal::from_i128_with_scale(quantity_scaled as i128, SCALE);
                let price = Decimal::from_i128_with_scale(price_scaled as i128, price_scale.max(0) as u32);
                TradeRecord {
                    id,
                    account_id,
                    account_name,
                    instrument_id,
                    symbol,
                    name,
                    asset_class,
                    currency: currency.clone(),
                    side,
                    trade_date,
                    quantity,
                    price,
                    fee_minor,
                    notes,
                }
            },
        )
        .collect())
}

/// Validate that a sequence of trades never sells more than is held. Excludes `exclude_id`
/// so edits and deletes can be checked before committing.
fn validate_sequence(
    conn: &Connection,
    tenant_id: &str,
    account_id: &str,
    instrument_id: &str,
    exclude_id: Option<&str>,
) -> AppResult<()> {
    let trades = load_trades(
        conn,
        tenant_id,
        Some(account_id),
        Some(instrument_id),
        exclude_id,
    )?;
    let mut quantity = Decimal::ZERO;
    for trade in trades {
        if trade.side == "buy" {
            quantity += trade.quantity;
        } else {
            if trade.quantity > quantity {
                return Err(AppError::Validation(format!(
                    "This change would sell more {} than is held in that account on {}.",
                    trade.symbol, trade.trade_date
                )));
            }
            quantity -= trade.quantity;
        }
    }
    Ok(())
}

pub fn list_instruments(conn: &Connection, tenant_id: &str) -> AppResult<Vec<InstrumentDto>> {
    let mut stmt = conn.prepare(
        "SELECT id, symbol, name, asset_class, currency_code, exchange, isin, notes
         FROM instruments WHERE tenant_id = ?1 ORDER BY symbol",
    )?;
    let instruments = stmt
        .query_map([tenant_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let trades = load_trades(conn, tenant_id, None, None, None)?;

    let mut valuations: HashMap<(String, String), (i64, String)> = HashMap::new();
    let mut valuation_stmt = conn.prepare(
        "SELECT account_id, instrument_id, value_minor, as_of_date
         FROM investment_valuations
         WHERE tenant_id = ?1 AND instrument_id IS NOT NULL
         ORDER BY as_of_date DESC, created_at DESC",
    )?;
    for row in valuation_stmt.query_map([tenant_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })? {
        let (account_id, instrument_id, value, date) = row?;
        valuations.entry((account_id, instrument_id)).or_insert((value, date));
    }

    struct Position {
        account_id: String,
        account_name: String,
        instrument_id: String,
        symbol: String,
        name: String,
        asset_class: String,
        currency: String,
        quantity: Decimal,
        cost: Decimal,
        realized: Decimal,
    }

    let mut positions: HashMap<(String, String), Position> = HashMap::new();
    for trade in &trades {
        let key = (trade.account_id.clone(), trade.instrument_id.clone());
        let entry = positions.entry(key).or_insert_with(|| Position {
            account_id: trade.account_id.clone(),
            account_name: trade.account_name.clone(),
            instrument_id: trade.instrument_id.clone(),
            symbol: trade.symbol.clone(),
            name: trade.name.clone(),
            asset_class: trade.asset_class.clone(),
            currency: trade.currency.clone(),
            quantity: Decimal::ZERO,
            cost: Decimal::ZERO,
            realized: Decimal::ZERO,
        });
        let gross = trade.quantity * trade.price;
        let fee = to_decimal(trade.fee_minor, currency_meta(conn, &trade.currency)?.exponent);
        if trade.side == "buy" {
            entry.quantity += trade.quantity;
            entry.cost += gross + fee;
        } else {
            let average = if entry.quantity > Decimal::ZERO {
                entry.cost / entry.quantity
            } else {
                Decimal::ZERO
            };
            let sold_cost = average * trade.quantity;
            entry.realized += gross - fee - sold_cost;
            entry.cost -= sold_cost;
            entry.quantity -= trade.quantity;
            if entry.quantity < Decimal::ZERO {
                entry.quantity = Decimal::ZERO;
            }
            if entry.cost < Decimal::ZERO {
                entry.cost = Decimal::ZERO;
            }
        }
    }

    let mut result = Vec::with_capacity(instruments.len());
    for (id, symbol, name, asset_class, currency, exchange, isin, notes) in instruments {
        let exp = currency_meta(conn, &currency)?.exponent;
        let mut holdings = Vec::new();
        let mut total_qty = Decimal::ZERO;
        let mut total_cost_minor = 0i64;
        let mut total_market_minor = 0i64;
        let mut total_realized_minor = 0i64;
        let mut valued = 0i64;
        let mut holding_count = 0i64;
        let mut valued_cost_minor = 0i64;

        for position in positions.values().filter(|p| p.instrument_id == id) {
            if position.quantity <= Decimal::ZERO {
                continue;
            }
            holding_count += 1;
            let cost_minor = amount_to_minor(position.cost, exp)?;
            let realized_minor = amount_to_minor(position.realized, exp)?;
            let average = if position.quantity > Decimal::ZERO {
                Some(
                    (position.cost / position.quantity)
                        .round_dp(SCALE)
                        .normalize()
                        .to_string(),
                )
            } else {
                None
            };
            let valuation = valuations
                .get(&(position.account_id.clone(), position.instrument_id.clone()));
            let market_minor = valuation.map(|(value, _)| *value);
            let unrealized_minor = market_minor.map(|value| value - cost_minor);
            if market_minor.is_some() {
                valued += 1;
                valued_cost_minor += cost_minor;
                total_market_minor += market_minor.unwrap_or(0);
            }
            total_qty += position.quantity;
            total_cost_minor += cost_minor;
            total_realized_minor += realized_minor;

            holdings.push(HoldingDto {
                account_id: position.account_id.clone(),
                account_name: position.account_name.clone(),
                instrument_id: position.instrument_id.clone(),
                symbol: position.symbol.clone(),
                name: position.name.clone(),
                asset_class: position.asset_class.clone(),
                currency: position.currency.clone(),
                quantity: position.quantity.round_dp(SCALE).normalize().to_string(),
                cost_basis_minor: cost_minor,
                cost_basis: format_amount(cost_minor, exp),
                average_cost: average,
                market_value_minor: market_minor,
                market_value: market_minor.map(|value| format_amount(value, exp)),
                valuation_date: valuation.map(|(_, date)| date.clone()),
                unrealized_minor,
                unrealized: unrealized_minor.map(|value| format_amount(value, exp)),
                realized_minor,
                realized: format_amount(realized_minor, exp),
            });
        }

        let trade_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM investment_trades WHERE tenant_id = ?1 AND instrument_id = ?2",
            params![tenant_id, id],
            |r| r.get(0),
        )?;
        let unrealized_minor = if holding_count > 0 && valued == holding_count {
            Some(total_market_minor - valued_cost_minor)
        } else {
            None
        };

        result.push(InstrumentDto {
            id,
            symbol,
            name,
            asset_class,
            currency: currency.clone(),
            currency_symbol: currency_meta(conn, &currency)?.symbol,
            exchange,
            isin,
            notes,
            trade_count,
            holding_count,
            quantity: total_qty.round_dp(SCALE).normalize().to_string(),
            cost_basis_minor: total_cost_minor,
            cost_basis: format_amount(total_cost_minor, exp),
            market_value_minor: if valued > 0 { Some(total_market_minor) } else { None },
            market_value: if valued > 0 {
                Some(format_amount(total_market_minor, exp))
            } else {
                None
            },
            unrealized_minor,
            unrealized: unrealized_minor.map(|value| format_amount(value, exp)),
            realized_minor: total_realized_minor,
            realized: format_amount(total_realized_minor, exp),
            valued_holdings: valued,
            holdings,
        });
    }
    Ok(result)
}

pub fn portfolio_summary(conn: &Connection, tenant_id: &str) -> AppResult<PortfolioSummary> {
    let instruments = list_instruments(conn, tenant_id)?;
    let trade_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM investment_trades WHERE tenant_id = ?1",
        [tenant_id],
        |r| r.get(0),
    )?;
    let valuation_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM investment_valuations WHERE tenant_id = ?1",
        [tenant_id],
        |r| r.get(0),
    )?;

    let mut grouped: HashMap<String, (i64, i64, i64, i64, i64)> = HashMap::new();
    for instrument in &instruments {
        if instrument.holding_count == 0 {
            continue;
        }
        let entry = grouped.entry(instrument.currency.clone()).or_insert((0, 0, 0, 0, 0));
        entry.0 += instrument.cost_basis_minor;
        entry.1 += instrument.market_value_minor.unwrap_or(0);
        entry.2 += instrument.realized_minor;
        entry.3 += instrument.holding_count;
        entry.4 += instrument.valued_holdings;
    }

    let mut currencies = Vec::new();
    for (currency, (cost, market, realized, holdings, valued)) in grouped {
        let meta = currency_meta(conn, &currency)?;
        currencies.push(CurrencySummary {
            currency,
            symbol: meta.symbol,
            cost_basis_minor: cost,
            cost_basis: format_amount(cost, meta.exponent),
            market_value_minor: market,
            market_value: format_amount(market, meta.exponent),
            realized_minor: realized,
            realized: format_amount(realized, meta.exponent),
            holdings,
            valued_holdings: valued,
        });
    }
    currencies.sort_by(|a, b| a.currency.cmp(&b.currency));

    Ok(PortfolioSummary {
        instrument_count: instruments.len() as i64,
        trade_count,
        valuation_count,
        currencies,
    })
}

pub fn save_instrument(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: Option<&str>,
    input: &InstrumentInput,
) -> AppResult<String> {
    if input.symbol.trim().is_empty() || input.name.trim().is_empty() {
        return Err(AppError::Validation("Symbol and name are required.".into()));
    }
    if !ASSET_CLASSES.contains(&input.asset_class.as_str()) {
        return Err(AppError::Validation("Unknown asset class.".into()));
    }
    let currency_code = input
        .currency_code
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or(primary_currency(conn, tenant_id)?);
    currency_meta(conn, &currency_code)?;

    let duplicate: Option<String> = conn
        .query_row(
            "SELECT id FROM instruments
             WHERE tenant_id = ?1 AND symbol = ?2 AND COALESCE(exchange, '') = COALESCE(?3, '')
               AND (?4 IS NULL OR id <> ?4)",
            params![
                tenant_id,
                input.symbol.trim(),
                input.exchange.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                id.filter(|v| !v.is_empty())
            ],
            |r| r.get(0),
        )
        .optional()?;
    if duplicate.is_some() {
        return Err(AppError::Validation(
            "An instrument with this symbol and exchange already exists.".into(),
        ));
    }

    let now = dates::now_iso();
    let instrument_id = match id.filter(|v| !v.is_empty()) {
        Some(existing) => {
            let found: Option<String> = conn
                .query_row(
                    "SELECT id FROM instruments WHERE id = ?1 AND tenant_id = ?2",
                    params![existing, tenant_id],
                    |r| r.get(0),
                )
                .optional()?;
            if found.is_none() {
                return Err(AppError::NotFound("Instrument not found.".into()));
            }
            let trade_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM investment_trades WHERE instrument_id = ?1 AND tenant_id = ?2",
                params![existing, tenant_id],
                |r| r.get(0),
            )?;
            let valuation_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM investment_valuations WHERE instrument_id = ?1 AND tenant_id = ?2",
                params![existing, tenant_id],
                |r| r.get(0),
            )?;
            if trade_count > 0 || valuation_count > 0 {
                let existing_currency: String = conn.query_row(
                    "SELECT currency_code FROM instruments WHERE id = ?1",
                    [existing],
                    |r| r.get(0),
                )?;
                if existing_currency != currency_code {
                    return Err(AppError::Validation(
                        "This instrument already has trades or valuations. Its currency cannot change.".into(),
                    ));
                }
            }
            conn.execute(
                "UPDATE instruments SET symbol = ?1, name = ?2, asset_class = ?3, currency_code = ?4,
                    exchange = ?5, isin = ?6, notes = ?7
                 WHERE id = ?8 AND tenant_id = ?9",
                params![
                    input.symbol.trim(),
                    input.name.trim(),
                    input.asset_class,
                    currency_code,
                    input.exchange.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    input.isin.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    existing,
                    tenant_id
                ],
            )?;
            existing.to_string()
        }
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO instruments (id, tenant_id, symbol, name, asset_class, currency_code, exchange, isin, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    new_id,
                    tenant_id,
                    input.symbol.trim().to_uppercase(),
                    input.name.trim(),
                    input.asset_class,
                    currency_code,
                    input.exchange.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    input.isin.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    now
                ],
            )?;
            new_id
        }
    };

    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "instrument",
        &instrument_id,
        if id.is_some() { "update" } else { "create" },
        None,
        Some(&serde_json::json!({
            "symbol": input.symbol.trim(),
            "name": input.name.trim(),
            "assetClass": input.asset_class
        })),
    )?;
    Ok(instrument_id)
}

pub fn delete_instrument(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
) -> AppResult<()> {
    let trade_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM investment_trades WHERE instrument_id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
        |r| r.get(0),
    )?;
    let valuation_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM investment_valuations WHERE instrument_id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
        |r| r.get(0),
    )?;
    if trade_count > 0 || valuation_count > 0 {
        return Err(AppError::Validation(
            "This instrument has trades or valuations. Delete those records first.".into(),
        ));
    }
    let changed = conn.execute(
        "DELETE FROM instruments WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Instrument not found.".into()));
    }
    crate::db::audit(conn, tenant_id, user_id, "instrument", id, "delete", None, None)?;
    Ok(())
}

pub fn list_trades(
    conn: &Connection,
    tenant_id: &str,
    instrument_id: Option<&str>,
) -> AppResult<Vec<TradeDto>> {
    let trades = load_trades(conn, tenant_id, None, instrument_id, None)?;
    let mut result = Vec::with_capacity(trades.len());
    for trade in trades {
        let exp = currency_meta(conn, &trade.currency)?.exponent;
        let gross_minor = amount_to_minor(trade.quantity * trade.price, exp)?;
        result.push(TradeDto {
            id: trade.id,
            account_id: trade.account_id,
            account_name: trade.account_name,
            instrument_id: trade.instrument_id,
            symbol: trade.symbol,
            instrument_name: trade.name,
            side: trade.side,
            trade_date: trade.trade_date,
            quantity: trade.quantity.normalize().to_string(),
            price: trade.price.normalize().to_string(),
            fee_minor: trade.fee_minor,
            fee: format_amount(trade.fee_minor, exp),
            total_minor: gross_minor,
            total: format_amount(gross_minor, exp),
            currency: trade.currency,
            notes: trade.notes,
        });
    }
    result.sort_by(|a, b| b.trade_date.cmp(&a.trade_date));
    Ok(result)
}

pub fn save_trade(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: Option<&str>,
    input: &TradeInput,
) -> AppResult<String> {
    if !SIDES.contains(&input.side.as_str()) {
        return Err(AppError::Validation("Trade side must be buy or sell.".into()));
    }
    dates::parse_date(&input.trade_date)?;
    validate_account(conn, tenant_id, &input.account_id)?;
    let (symbol, _, _, currency) = validate_instrument(conn, tenant_id, &input.instrument_id)?;
    let exp = currency_meta(conn, &currency)?.exponent;

    let quantity_scaled = parse_scaled(&input.quantity, SCALE)?;
    if quantity_scaled <= 0 {
        return Err(AppError::Validation("Quantity must be greater than zero.".into()));
    }
    let price_scaled = parse_scaled(&input.price, SCALE)?;
    if price_scaled < 0 {
        return Err(AppError::Validation("Price cannot be negative.".into()));
    }
    let fee_minor = match input.fee.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        Some(value) => {
            let parsed = parse_amount(value, exp)?;
            if parsed < 0 {
                return Err(AppError::Validation("Fee cannot be negative.".into()));
            }
            parsed
        }
        None => 0,
    };

    // Existing trades for this account/instrument must remain a valid sequence.
    validate_sequence(
        conn,
        tenant_id,
        &input.account_id,
        &input.instrument_id,
        id.filter(|v| !v.is_empty()),
    )?;
    if input.side == "sell" {
        let existing = load_trades(
            conn,
            tenant_id,
            Some(&input.account_id),
            Some(&input.instrument_id),
            id.filter(|v| !v.is_empty()),
        )?;
        let mut held = Decimal::ZERO;
        for trade in existing {
            if trade.side == "buy" {
                held += trade.quantity;
            } else {
                held -= trade.quantity;
            }
        }
        let requested = Decimal::from_i128_with_scale(quantity_scaled as i128, SCALE);
        if requested > held {
            return Err(AppError::Validation(format!(
                "Cannot sell {requested} {symbol}: only {held} is held in that account."
            )));
        }
    }

    let trade_id = match id.filter(|v| !v.is_empty()) {
        Some(existing) => {
            let found: Option<String> = conn
                .query_row(
                    "SELECT id FROM investment_trades WHERE id = ?1 AND tenant_id = ?2",
                    params![existing, tenant_id],
                    |r| r.get(0),
                )
                .optional()?;
            if found.is_none() {
                return Err(AppError::NotFound("Trade not found.".into()));
            }
            conn.execute(
                "UPDATE investment_trades SET account_id = ?1, instrument_id = ?2, side = ?3, trade_date = ?4,
                    quantity_scaled = ?5, price_scaled = ?6, price_scale = ?7, fee_minor = ?8, notes = ?9
                 WHERE id = ?10 AND tenant_id = ?11",
                params![
                    input.account_id,
                    input.instrument_id,
                    input.side,
                    input.trade_date,
                    quantity_scaled,
                    price_scaled,
                    SCALE as i64,
                    fee_minor,
                    input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    existing,
                    tenant_id
                ],
            )?;
            existing.to_string()
        }
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO investment_trades (id, tenant_id, user_id, account_id, instrument_id, side, trade_date,
                    quantity_scaled, price_scaled, price_scale, fee_minor, currency_code, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    new_id,
                    tenant_id,
                    user_id,
                    input.account_id,
                    input.instrument_id,
                    input.side,
                    input.trade_date,
                    quantity_scaled,
                    price_scaled,
                    SCALE as i64,
                    fee_minor,
                    currency,
                    input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    dates::now_iso()
                ],
            )?;
            new_id
        }
    };

    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "investment_trade",
        &trade_id,
        if id.is_some() { "update" } else { "create" },
        None,
        Some(&serde_json::json!({
            "instrumentId": input.instrument_id,
            "side": input.side,
            "quantityScaled": quantity_scaled,
            "priceScaled": price_scaled,
            "feeMinor": fee_minor
        })),
    )?;
    Ok(trade_id)
}

pub fn delete_trade(conn: &Connection, tenant_id: &str, user_id: &str, id: &str) -> AppResult<()> {
    let trade = load_trades(conn, tenant_id, None, None, None)?
        .into_iter()
        .find(|trade| trade.id == id)
        .ok_or_else(|| AppError::NotFound("Trade not found.".into()))?;
    validate_sequence(
        conn,
        tenant_id,
        &trade.account_id,
        &trade.instrument_id,
        Some(id),
    )?;
    conn.execute(
        "DELETE FROM investment_trades WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
    )?;
    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "investment_trade",
        id,
        "delete",
        None,
        None,
    )?;
    Ok(())
}

pub fn list_valuations(
    conn: &Connection,
    tenant_id: &str,
    instrument_id: Option<&str>,
) -> AppResult<Vec<ValuationDto>> {
    let mut stmt = conn.prepare(
        "SELECT v.id, v.account_id, a.name, v.instrument_id, i.symbol, v.as_of_date, v.value_minor,
                v.currency_code, v.source, v.source_ref
         FROM investment_valuations v
         JOIN accounts a ON a.id = v.account_id
         LEFT JOIN instruments i ON i.id = v.instrument_id
         WHERE v.tenant_id = ?1 AND (?2 IS NULL OR v.instrument_id = ?2)
         ORDER BY v.as_of_date DESC, v.created_at DESC",
    )?;
    let rows = stmt
        .query_map(params![tenant_id, instrument_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut result = Vec::with_capacity(rows.len());
    for (
        id,
        account_id,
        account_name,
        instrument_id,
        symbol,
        as_of_date,
        value_minor,
        currency,
        source,
        source_ref,
    ) in rows
    {
        let exp = currency_meta(conn, &currency)?.exponent;
        result.push(ValuationDto {
            id,
            account_id,
            account_name,
            instrument_id,
            symbol,
            as_of_date,
            value_minor,
            value: format_amount(value_minor, exp),
            currency,
            source,
            source_ref,
        });
    }
    Ok(result)
}

pub fn save_valuation(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: Option<&str>,
    input: &ValuationInput,
) -> AppResult<String> {
    dates::parse_date(&input.as_of_date)?;
    validate_account(conn, tenant_id, &input.account_id)?;
    let (symbol, _, _, currency) = validate_instrument(conn, tenant_id, &input.instrument_id)?;
    let exp = currency_meta(conn, &currency)?.exponent;
    let value_minor = parse_amount(&input.value, exp)?;
    if value_minor < 0 {
        return Err(AppError::Validation("Valuation cannot be negative.".into()));
    }

    let valuation_id = match id.filter(|v| !v.is_empty()) {
        Some(existing) => {
            let found: Option<String> = conn
                .query_row(
                    "SELECT id FROM investment_valuations WHERE id = ?1 AND tenant_id = ?2",
                    params![existing, tenant_id],
                    |r| r.get(0),
                )
                .optional()?;
            if found.is_none() {
                return Err(AppError::NotFound("Valuation not found.".into()));
            }
            conn.execute(
                "UPDATE investment_valuations SET account_id = ?1, instrument_id = ?2, as_of_date = ?3,
                    value_minor = ?4, currency_code = ?5, source = 'manual', source_ref = ?6
                 WHERE id = ?7 AND tenant_id = ?8",
                params![
                    input.account_id,
                    input.instrument_id,
                    input.as_of_date,
                    value_minor,
                    currency,
                    input.source_ref.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    existing,
                    tenant_id
                ],
            )?;
            existing.to_string()
        }
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO investment_valuations (id, tenant_id, account_id, instrument_id, as_of_date,
                    quantity_scaled, price_scaled, price_scale, value_minor, currency_code, source, source_ref, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6, ?7, ?8, 'manual', ?9, ?10)",
                params![
                    new_id,
                    tenant_id,
                    input.account_id,
                    input.instrument_id,
                    input.as_of_date,
                    SCALE as i64,
                    value_minor,
                    currency,
                    input.source_ref.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                    dates::now_iso()
                ],
            )?;
            new_id
        }
    };

    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "investment_valuation",
        &valuation_id,
        if id.is_some() { "update" } else { "create" },
        None,
        Some(&serde_json::json!({
            "instrument": symbol,
            "valueMinor": value_minor,
            "asOfDate": input.as_of_date
        })),
    )?;
    Ok(valuation_id)
}

pub fn delete_valuation(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
) -> AppResult<()> {
    let changed = conn.execute(
        "DELETE FROM investment_valuations WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Valuation not found.".into()));
    }
    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "investment_valuation",
        id,
        "delete",
        None,
        None,
    )?;
    Ok(())
}

pub fn latest_portfolio_value(conn: &Connection, tenant_id: &str) -> AppResult<i64> {
    let base = primary_currency(conn, tenant_id)?;
    let summary = portfolio_summary(conn, tenant_id)?;
    let mut total = 0i64;
    for currency in summary.currencies {
        if currency.currency == base {
            total += currency.market_value_minor;
        } else {
            let converted = crate::services::currency::convert_to_currency(
                conn,
                tenant_id,
                currency.market_value_minor,
                &currency.currency,
                &base,
                &iso(today_local()),
            );
            if let Ok((value, _)) = converted {
                total += value;
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, open_test};

    fn setup() -> (Connection, String, String, String) {
        let mut conn = open_test();
        let (tenant, user) = db::seed_if_empty(&mut conn).unwrap();
        let account_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO accounts (id, tenant_id, user_id, name, type, currency_code, is_liability, is_liquid,
                opening_balance_minor, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'Brokerage', 'investment', 'QAR', 0, 0, 0, ?4, ?4)",
            params![account_id, tenant, user, dates::now_iso()],
        )
        .unwrap();
        (conn, tenant, user, account_id)
    }

    fn instrument(conn: &Connection, tenant: &str, user: &str) -> String {
        save_instrument(
            conn,
            tenant,
            user,
            None,
            &InstrumentInput {
                symbol: "TEST".into(),
                name: "Test Fund".into(),
                asset_class: "etf".into(),
                currency_code: Some("QAR".into()),
                exchange: Some("QSE".into()),
                isin: None,
                notes: None,
            },
        )
        .unwrap()
    }

    fn trade(account: &str, instrument: &str, side: &str, qty: &str, price: &str) -> TradeInput {
        TradeInput {
            account_id: account.into(),
            instrument_id: instrument.into(),
            side: side.into(),
            trade_date: "2026-11-01".into(),
            quantity: qty.into(),
            price: price.into(),
            fee: Some("5.00".into()),
            notes: None,
        }
    }

    #[test]
    fn holdings_and_cost_basis_follow_trades() {
        let (conn, tenant, user, account) = setup();
        let instrument_id = instrument(&conn, &tenant, &user);
        save_trade(
            &conn,
            &tenant,
            &user,
            None,
            &trade(&account, &instrument_id, "buy", "10", "100.00"),
        )
        .unwrap();
        save_trade(
            &conn,
            &tenant,
            &user,
            None,
            &trade(&account, &instrument_id, "sell", "4", "120.00"),
        )
        .unwrap();

        let instruments = list_instruments(&conn, &tenant).unwrap();
        assert_eq!(instruments.len(), 1);
        let item = &instruments[0];
        assert_eq!(item.quantity, "6");
        // 10 @ 100 + 5 fee = 1005; sell 4 at average 100.50 -> cost 402, realized 480 - 5 - 402 = 73
        assert_eq!(item.cost_basis_minor, 60_300);
        assert_eq!(item.realized_minor, 7_300);
        assert_eq!(item.holdings.len(), 1);
        assert_eq!(item.holdings[0].average_cost.as_deref(), Some("100.5"));
    }

    #[test]
    fn selling_more_than_held_is_rejected() {
        let (conn, tenant, user, account) = setup();
        let instrument_id = instrument(&conn, &tenant, &user);
        save_trade(
            &conn,
            &tenant,
            &user,
            None,
            &trade(&account, &instrument_id, "buy", "2", "50.00"),
        )
        .unwrap();
        let error = save_trade(
            &conn,
            &tenant,
            &user,
            None,
            &trade(&account, &instrument_id, "sell", "3", "50.00"),
        )
        .unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn manual_valuation_drives_unrealized_gain() {
        let (conn, tenant, user, account) = setup();
        let instrument_id = instrument(&conn, &tenant, &user);
        save_trade(
            &conn,
            &tenant,
            &user,
            None,
            &trade(&account, &instrument_id, "buy", "10", "100.00"),
        )
        .unwrap();
        save_valuation(
            &conn,
            &tenant,
            &user,
            None,
            &ValuationInput {
                account_id: account.clone(),
                instrument_id: instrument_id.clone(),
                as_of_date: "2026-11-14".into(),
                value: "1200.00".into(),
                source_ref: Some("broker statement".into()),
            },
        )
        .unwrap();

        let item = &list_instruments(&conn, &tenant).unwrap()[0];
        assert_eq!(item.market_value_minor, Some(120_000));
        assert_eq!(item.unrealized_minor, Some(19_500));
        assert_eq!(item.holdings[0].valuation_date.as_deref(), Some("2026-11-14"));
    }

    #[test]
    fn deleting_a_buy_that_backs_a_sale_is_rejected() {
        let (conn, tenant, user, account) = setup();
        let instrument_id = instrument(&conn, &tenant, &user);
        let buy_id = save_trade(
            &conn,
            &tenant,
            &user,
            None,
            &trade(&account, &instrument_id, "buy", "5", "10.00"),
        )
        .unwrap();
        save_trade(
            &conn,
            &tenant,
            &user,
            None,
            &trade(&account, &instrument_id, "sell", "4", "10.00"),
        )
        .unwrap();
        let error = delete_trade(&conn, &tenant, &user, &buy_id).unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn instrument_with_records_cannot_be_deleted() {
        let (conn, tenant, user, account) = setup();
        let instrument_id = instrument(&conn, &tenant, &user);
        save_trade(
            &conn,
            &tenant,
            &user,
            None,
            &trade(&account, &instrument_id, "buy", "1", "10.00"),
        )
        .unwrap();
        let error = delete_instrument(&conn, &tenant, &user, &instrument_id).unwrap_err();
        assert!(matches!(error, AppError::Validation(_)));
    }
}
