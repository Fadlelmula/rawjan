use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::dates::{iso, local_instant, now_iso, parse_date, today_local};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::money::{convert_minor, format_amount, parse_amount, parse_rate, MoneyView};
use crate::services::currency::{add_manual_rate, exponent, primary_currency, require_rate};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitInput {
    pub category_id: String,
    pub amount: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxInput {
    pub kind: String,
    pub status: Option<String>,
    pub data_status: Option<String>,
    pub description: String,
    pub payee: Option<String>,
    pub notes: Option<String>,
    pub date: String,
    pub time: Option<String>,
    pub account_id: String,
    pub counter_account_id: Option<String>,
    pub category_id: Option<String>,
    pub income_source_id: Option<String>,
    pub amount: String,
    pub currency: Option<String>,
    pub counter_amount: Option<String>,
    pub fx_rate: Option<String>,
    pub splits: Option<Vec<SplitInput>>,
    pub tags: Option<Vec<String>>,
    pub external_ref: Option<String>,
    pub direction: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostResult {
    pub id: String,
    pub possible_duplicate_id: Option<String>,
    pub amount: MoneyView,
    pub base_amount: MoneyView,
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub id: String,
    pub name: String,
    pub currency: String,
    pub account_type: String,
    pub is_liability: bool,
    pub is_active: bool,
}

pub fn load_account(
    conn: &Connection,
    tenant_id: &str,
    account_id: &str,
) -> AppResult<AccountInfo> {
    conn.query_row(
        "SELECT id, name, currency_code, type, is_liability, is_active
         FROM accounts WHERE id = ?1 AND tenant_id = ?2",
        params![account_id, tenant_id],
        |row| {
            Ok(AccountInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                currency: row.get(2)?,
                account_type: row.get(3)?,
                is_liability: row.get::<_, i64>(4)? == 1,
                is_active: row.get::<_, i64>(5)? == 1,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("Account not found.".into()))
}

fn category_kind(conn: &Connection, tenant_id: &str, category_id: &str) -> AppResult<String> {
    conn.query_row(
        "SELECT kind FROM categories WHERE id = ?1 AND tenant_id = ?2",
        params![category_id, tenant_id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("Category not found.".into()))
}

fn find_category_by_name(
    conn: &Connection,
    tenant_id: &str,
    name: &str,
    kind: &str,
) -> AppResult<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT id FROM categories WHERE tenant_id = ?1 AND name = ?2 AND kind = ?3 AND parent_id IS NULL LIMIT 1",
            params![tenant_id, name, kind],
            |row| row.get(0),
        )
        .optional()?)
}

fn normalize_text(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

pub(crate) fn dedupe_hash(
    account_id: &str,
    date: &str,
    amount_minor: i64,
    description: &str,
    kind: &str,
) -> String {
    let mut hasher = Sha256::new();
    for part in [
        account_id,
        date,
        &amount_minor.to_string(),
        &normalize_text(description),
        kind,
    ] {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    hex::encode(hasher.finalize())
}

struct Leg {
    account_id: Option<String>,
    category_id: Option<String>,
    entry_type: &'static str,
    amount_minor: i64,
    currency: String,
    base_minor: i64,
    fx_rate_id: Option<String>,
    memo: Option<String>,
}

fn insert_leg(
    conn: &Connection,
    tenant_id: &str,
    transaction_id: &str,
    base_currency: &str,
    leg: &Leg,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ledger_entries
         (id, tenant_id, transaction_id, account_id, category_id, entry_type, amount_minor, currency_code,
          base_amount_minor, base_currency, fx_rate_id, memo, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            uuid::Uuid::new_v4().to_string(),
            tenant_id,
            transaction_id,
            leg.account_id,
            leg.category_id,
            leg.entry_type,
            leg.amount_minor,
            leg.currency,
            leg.base_minor,
            base_currency,
            leg.fx_rate_id,
            leg.memo,
            now_iso()
        ],
    )?;
    Ok(())
}

fn resolve_tags(conn: &Connection, tenant_id: &str, values: &[String]) -> AppResult<Vec<String>> {
    let mut ids = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM tags WHERE tenant_id = ?1 AND (id = ?2 OR lower(name) = lower(?2)) LIMIT 1",
                params![tenant_id, trimmed],
                |r| r.get(0),
            )
            .optional()?;
        let id = match existing {
            Some(id) => id,
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO tags (id, tenant_id, name, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![id, tenant_id, trimmed, now_iso()],
                )?;
                id
            }
        };
        ids.push(id);
    }
    Ok(ids)
}

pub fn create_transaction(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    input: &TxInput,
) -> AppResult<PostResult> {
    let db_tx = conn.unchecked_transaction()?;
    let result = create_transaction_inner(&db_tx, tenant_id, user_id, None, None, input)?;
    db_tx.commit()?;
    Ok(result)
}

/// Write a transaction on a caller-owned connection/transaction. Used when a
/// larger operation (an edit, an import batch, an account opening) must commit
/// several writes atomically. The caller owns the transaction boundary.
pub(crate) fn create_transaction_on(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    input: &TxInput,
) -> AppResult<PostResult> {
    create_transaction_inner(conn, tenant_id, user_id, None, None, input)
}

fn create_transaction_inner(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    fixed_id: Option<&str>,
    fixed_created_at: Option<&str>,
    input: &TxInput,
) -> AppResult<PostResult> {
    let account = load_account(conn, tenant_id, &input.account_id)?;
    if !account.is_active {
        return Err(AppError::Validation(
            "The selected account is archived.".into(),
        ));
    }
    let kind = input.kind.as_str();
    const KINDS: &[&str] = &[
        "expense",
        "income",
        "transfer",
        "refund",
        "fee",
        "investment_buy",
        "investment_sell",
        "dividend",
        "adjustment",
        "opening",
    ];
    if !KINDS.contains(&kind) {
        return Err(AppError::Validation(format!(
            "Unknown transaction kind '{kind}'."
        )));
    }
    let status = input.status.clone().unwrap_or_else(|| "posted".into());
    if !["draft", "pending", "posted", "reconciled"].contains(&status.as_str()) {
        return Err(AppError::Validation(
            "New transactions must be draft, pending, posted or reconciled.".into(),
        ));
    }
    let data_status = input.data_status.clone().unwrap_or_else(|| {
        if status == "pending" {
            "pending".into()
        } else {
            "actual".into()
        }
    });
    if !["actual", "pending", "planned", "estimated", "forecast"].contains(&data_status.as_str()) {
        return Err(AppError::Validation("Invalid data status.".into()));
    }
    if input.description.trim().is_empty() {
        return Err(AppError::Validation("A description is required.".into()));
    }

    let date = parse_date(&input.date)?;
    let local_date = iso(date);
    let occurred_at = local_instant(date, input.time.as_deref())?;
    let base_currency = primary_currency(conn, tenant_id)?;
    let tx_currency = input
        .currency
        .clone()
        .unwrap_or_else(|| account.currency.clone());
    if tx_currency != account.currency {
        return Err(AppError::Validation(
            "Amounts must be entered in the account's currency. Use a foreign-currency account for other currencies.".into(),
        ));
    }
    let tx_exp = exponent(conn, &tx_currency)?;
    let base_exp = exponent(conn, &base_currency)?;
    let amount_minor = parse_amount(&input.amount, tx_exp)?;
    if amount_minor <= 0 {
        return Err(AppError::Validation(
            "Amount must be greater than zero. To correct a posted record, use a reversal.".into(),
        ));
    }

    let rate_snapshot = if tx_currency == base_currency {
        None
    } else if let Some(manual) = input.fx_rate.as_deref().filter(|v| !v.trim().is_empty()) {
        let rate = parse_rate(manual)?;
        Some(add_manual_rate(
            conn,
            tenant_id,
            &tx_currency,
            &base_currency,
            rate,
            &local_date,
        )?)
    } else {
        Some(require_rate(
            conn,
            tenant_id,
            &tx_currency,
            &base_currency,
            &local_date,
        )?)
    };
    let rate_value = rate_snapshot
        .as_ref()
        .map(|r| r.rate)
        .unwrap_or(rust_decimal::Decimal::ONE);
    let base_amount = if tx_currency == base_currency {
        amount_minor
    } else {
        convert_minor(amount_minor, tx_exp, base_exp, rate_value)?
    };
    if base_amount <= 0 {
        return Err(AppError::Validation(
            "Converted base amount is zero. Check the exchange rate.".into(),
        ));
    }
    let fx_rate_id = rate_snapshot.as_ref().and_then(|r| r.id.clone());

    let counter = match input
        .counter_account_id
        .as_deref()
        .filter(|v| !v.is_empty())
    {
        Some(id) => Some(load_account(conn, tenant_id, id)?),
        None => None,
    };
    if matches!(kind, "transfer" | "investment_buy" | "investment_sell") && counter.is_none() {
        return Err(AppError::Validation(
            "Select the destination account.".into(),
        ));
    }
    let counter_minor = match &counter {
        Some(c) => {
            if c.id == account.id {
                return Err(AppError::Validation(
                    "Source and destination accounts must differ.".into(),
                ));
            }
            if matches!(kind, "investment_buy" | "investment_sell")
                && c.account_type != "investment"
            {
                return Err(AppError::Validation(
                    "Investment transactions require an investment account as the destination."
                        .into(),
                ));
            }
            if c.currency == account.currency {
                amount_minor
            } else {
                let cexp = exponent(conn, &c.currency)?;
                match input
                    .counter_amount
                    .as_deref()
                    .filter(|v| !v.trim().is_empty())
                {
                    Some(value) => parse_amount(value, cexp)?,
                    None => {
                        return Err(AppError::Validation(format!(
                            "Enter the amount received in {} for this cross-currency transfer.",
                            c.currency
                        )))
                    }
                }
            }
        }
        None => 0,
    };
    if counter.is_some() && counter_minor <= 0 {
        return Err(AppError::Validation(
            "Destination amount must be greater than zero.".into(),
        ));
    }

    // Category legs (splits supported for expenses and refunds).
    let mut category_legs: Vec<(String, i64, String, Option<String>)> = Vec::new();
    let mut is_split = false;
    if matches!(
        kind,
        "expense" | "income" | "refund" | "fee" | "dividend" | "adjustment" | "opening"
    ) {
        if let Some(splits) = input.splits.as_ref().filter(|s| !s.is_empty()) {
            if !matches!(kind, "expense" | "refund") {
                return Err(AppError::Validation(
                    "Split transactions are supported for expenses and refunds.".into(),
                ));
            }
            is_split = true;
            let mut total = 0i64;
            for split in splits {
                let split_minor = parse_amount(&split.amount, tx_exp)?;
                if split_minor <= 0 {
                    return Err(AppError::Validation(
                        "Each split must be greater than zero.".into(),
                    ));
                }
                if category_kind(conn, tenant_id, &split.category_id)? != "expense" {
                    return Err(AppError::Validation(
                        "Split categories must be expense categories.".into(),
                    ));
                }
                total = crate::money::checked_sum([total, split_minor].into_iter())?;
                category_legs.push((
                    split.category_id.clone(),
                    split_minor,
                    tx_currency.clone(),
                    split.note.clone(),
                ));
            }
            if total != amount_minor {
                return Err(AppError::Validation(format!(
                    "Split amounts ({}) must add up to the transaction amount ({}).",
                    format_amount(total, tx_exp),
                    format_amount(amount_minor, tx_exp)
                )));
            }
        } else {
            let category_id = match input.category_id.clone().filter(|v| !v.is_empty()) {
                Some(id) => id,
                None if kind == "fee" => find_category_by_name(conn, tenant_id, "Fees", "expense")?
                    .ok_or_else(|| {
                        AppError::Validation("Create an expense category first.".into())
                    })?,
                None if kind == "opening" => {
                    find_category_by_name(conn, tenant_id, "Opening Balance", "equity")?
                        .ok_or_else(|| {
                            AppError::Validation("Create an equity category first.".into())
                        })?
                }
                None if kind == "adjustment" => {
                    find_category_by_name(conn, tenant_id, "Balance Adjustment", "equity")?
                        .ok_or_else(|| {
                            AppError::Validation("Create an equity category first.".into())
                        })?
                }
                None => return Err(AppError::Validation("Select a category.".into())),
            };
            let ck = category_kind(conn, tenant_id, &category_id)?;
            let valid = match kind {
                "income" | "dividend" => ck == "income",
                "expense" | "refund" | "fee" => ck == "expense",
                "adjustment" | "opening" => ck == "equity",
                _ => true,
            };
            if !valid {
                return Err(AppError::Validation(format!(
                    "This transaction type cannot use a {ck} category."
                )));
            }
            category_legs.push((category_id, amount_minor, tx_currency.clone(), None));
        }
    } else if !matches!(kind, "transfer" | "investment_buy" | "investment_sell") {
        return Err(AppError::Validation("Select a category.".into()));
    }

    if let Some(income_source_id) = input.income_source_id.as_deref().filter(|v| !v.is_empty()) {
        let exists: i64 = conn.query_row(
            "SELECT count(*) FROM income_sources WHERE id = ?1 AND tenant_id = ?2",
            params![income_source_id, tenant_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(AppError::NotFound("Income source not found.".into()));
        }
    }

    // Base amounts for category legs: assign rounding remainder to the last leg.
    let mut category_bases: Vec<i64> = Vec::with_capacity(category_legs.len());
    let mut allocated = 0i64;
    for (index, (_, leg_amount, _, _)) in category_legs.iter().enumerate() {
        if index + 1 == category_legs.len() {
            category_bases.push(crate::money::checked_diff(base_amount, allocated)?);
        } else {
            let converted = if tx_currency == base_currency {
                *leg_amount
            } else {
                convert_minor(*leg_amount, tx_exp, base_exp, rate_value)?
            };
            allocated = crate::money::checked_sum([allocated, converted].into_iter())?;
            category_bases.push(converted);
        }
    }

    let direction = input.direction.clone().unwrap_or_else(|| "in".into());
    if matches!(kind, "adjustment" | "opening") && !matches!(direction.as_str(), "in" | "out") {
        return Err(AppError::Validation(
            "Direction must be 'in' or 'out'.".into(),
        ));
    }

    let mut legs: Vec<Leg> = Vec::new();
    let account_leg = |entry_type: &'static str, amount: i64, currency: &str, base: i64| Leg {
        account_id: Some(account.id.clone()),
        category_id: None,
        entry_type,
        amount_minor: amount,
        currency: currency.to_string(),
        base_minor: base,
        fx_rate_id: fx_rate_id.clone(),
        memo: None,
    };
    match kind {
        "expense" | "fee" => {
            legs.push(account_leg(
                "credit",
                amount_minor,
                &account.currency,
                base_amount,
            ));
            for ((category_id, amount, currency, note), base) in
                category_legs.iter().zip(category_bases.iter())
            {
                legs.push(Leg {
                    account_id: None,
                    category_id: Some(category_id.clone()),
                    entry_type: "debit",
                    amount_minor: *amount,
                    currency: currency.clone(),
                    base_minor: *base,
                    fx_rate_id: fx_rate_id.clone(),
                    memo: note.clone(),
                });
            }
        }
        "refund" => {
            legs.push(account_leg(
                "debit",
                amount_minor,
                &account.currency,
                base_amount,
            ));
            for ((category_id, amount, currency, note), base) in
                category_legs.iter().zip(category_bases.iter())
            {
                legs.push(Leg {
                    account_id: None,
                    category_id: Some(category_id.clone()),
                    entry_type: "credit",
                    amount_minor: *amount,
                    currency: currency.clone(),
                    base_minor: *base,
                    fx_rate_id: fx_rate_id.clone(),
                    memo: note.clone(),
                });
            }
        }
        "income" | "dividend" => {
            legs.push(account_leg(
                "debit",
                amount_minor,
                &account.currency,
                base_amount,
            ));
            for ((category_id, amount, currency, note), base) in
                category_legs.iter().zip(category_bases.iter())
            {
                legs.push(Leg {
                    account_id: None,
                    category_id: Some(category_id.clone()),
                    entry_type: "credit",
                    amount_minor: *amount,
                    currency: currency.clone(),
                    base_minor: *base,
                    fx_rate_id: fx_rate_id.clone(),
                    memo: note.clone(),
                });
            }
        }
        "transfer" | "investment_buy" | "investment_sell" => {
            let c = counter.as_ref().expect("validated counter account");
            match kind {
                "transfer" => {
                    legs.push(account_leg(
                        "credit",
                        amount_minor,
                        &account.currency,
                        base_amount,
                    ));
                    legs.push(Leg {
                        account_id: Some(c.id.clone()),
                        category_id: None,
                        entry_type: "debit",
                        amount_minor: counter_minor,
                        currency: c.currency.clone(),
                        base_minor: base_amount,
                        fx_rate_id: fx_rate_id.clone(),
                        memo: None,
                    });
                }
                "investment_buy" => {
                    legs.push(account_leg(
                        "credit",
                        amount_minor,
                        &account.currency,
                        base_amount,
                    ));
                    legs.push(Leg {
                        account_id: Some(c.id.clone()),
                        category_id: None,
                        entry_type: "debit",
                        amount_minor: counter_minor,
                        currency: c.currency.clone(),
                        base_minor: base_amount,
                        fx_rate_id: fx_rate_id.clone(),
                        memo: None,
                    });
                }
                _ => {
                    legs.push(account_leg(
                        "debit",
                        amount_minor,
                        &account.currency,
                        base_amount,
                    ));
                    legs.push(Leg {
                        account_id: Some(c.id.clone()),
                        category_id: None,
                        entry_type: "credit",
                        amount_minor: counter_minor,
                        currency: c.currency.clone(),
                        base_minor: base_amount,
                        fx_rate_id: fx_rate_id.clone(),
                        memo: None,
                    });
                }
            }
        }
        "adjustment" | "opening" => {
            let (category_id, amount, currency, note) =
                category_legs.first().expect("category leg");
            let base = category_bases.first().copied().unwrap_or(base_amount);
            if direction == "in" {
                legs.push(account_leg(
                    "debit",
                    amount_minor,
                    &account.currency,
                    base_amount,
                ));
                legs.push(Leg {
                    account_id: None,
                    category_id: Some(category_id.clone()),
                    entry_type: "credit",
                    amount_minor: *amount,
                    currency: currency.clone(),
                    base_minor: base,
                    fx_rate_id: fx_rate_id.clone(),
                    memo: note.clone(),
                });
            } else {
                legs.push(account_leg(
                    "credit",
                    amount_minor,
                    &account.currency,
                    base_amount,
                ));
                legs.push(Leg {
                    account_id: None,
                    category_id: Some(category_id.clone()),
                    entry_type: "debit",
                    amount_minor: *amount,
                    currency: currency.clone(),
                    base_minor: base,
                    fx_rate_id: fx_rate_id.clone(),
                    memo: note.clone(),
                });
            }
        }
        _ => unreachable!(),
    }

    let debit_total = crate::money::checked_sum(
        legs.iter()
            .filter(|l| l.entry_type == "debit")
            .map(|l| l.base_minor),
    )?;
    let credit_total = crate::money::checked_sum(
        legs.iter()
            .filter(|l| l.entry_type == "credit")
            .map(|l| l.base_minor),
    )?;
    if debit_total != credit_total {
        return Err(AppError::Internal(format!(
            "Ledger imbalance detected (debits {debit_total}, credits {credit_total}). The transaction was not saved."
        )));
    }

    let duplicate: Option<String> = conn
        .query_row(
            "SELECT id FROM transactions
             WHERE tenant_id = ?1 AND account_id = ?2 AND dedupe_hash = ?3 AND status NOT IN ('cancelled')
             ORDER BY created_at DESC LIMIT 1",
            params![
                tenant_id,
                account.id,
                dedupe_hash(&account.id, &local_date, amount_minor, &input.description, kind)
            ],
            |r| r.get(0),
        )
        .optional()?;

    let transaction_id = fixed_id
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let created_at = fixed_created_at.map(str::to_string).unwrap_or_else(now_iso);
    let now = now_iso();
    let hash = dedupe_hash(
        &account.id,
        &local_date,
        amount_minor,
        &input.description,
        kind,
    );
    let posted_at = if matches!(status.as_str(), "posted" | "reconciled") {
        Some(now.clone())
    } else {
        None
    };
    let reconciled_at = if status == "reconciled" {
        Some(now.clone())
    } else {
        None
    };
    let is_split_int = if is_split { 1i64 } else { 0i64 };

    // The caller owns the transaction boundary (see `create_transaction` and
    // `create_transaction_on`); everything below participates in it.
    let db_tx = conn;
    db_tx.execute(
        "INSERT INTO transactions
         (id, tenant_id, user_id, kind, status, data_status, description, payee, notes, local_date, occurred_at,
          account_id, counter_account_id, category_id, income_source_id, original_amount_minor, original_currency,
          fx_rate_id, counter_amount_minor, counter_currency, base_amount_minor, base_currency, is_split, dedupe_hash,
          external_ref, created_at, updated_at, posted_at, reconciled_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29)",
        params![
            transaction_id,
            tenant_id,
            user_id,
            kind,
            status,
            data_status,
            input.description.trim(),
            input.payee.as_deref().map(str::trim).filter(|v| !v.is_empty()),
            input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
            local_date,
            occurred_at,
            account.id,
            counter.as_ref().map(|c| c.id.clone()),
            if is_split { None } else { category_legs.first().map(|l| l.0.clone()) },
            input.income_source_id.as_deref().filter(|v| !v.is_empty()),
            amount_minor,
            tx_currency,
            fx_rate_id,
            if counter.is_some() { Some(counter_minor) } else { None },
            counter.as_ref().map(|c| c.currency.clone()),
            base_amount,
            base_currency,
            is_split_int,
            hash,
            input.external_ref.as_deref(),
            created_at,
            now,
            posted_at,
            reconciled_at
        ],
    )?;

    for leg in &legs {
        insert_leg(db_tx, tenant_id, &transaction_id, &base_currency, leg)?;
    }

    if is_split {
        let mut split_stmt = db_tx.prepare(
            "INSERT INTO transaction_splits (id, tenant_id, transaction_id, category_id, amount_minor, base_amount_minor, note, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for (index, ((category_id, amount, _currency, note), base)) in
            category_legs.iter().zip(category_bases.iter()).enumerate()
        {
            split_stmt.execute(params![
                uuid::Uuid::new_v4().to_string(),
                tenant_id,
                transaction_id,
                category_id,
                amount,
                base,
                note,
                index as i64
            ])?;
        }
    }

    if let Some(tags) = input.tags.as_ref().filter(|t| !t.is_empty()) {
        let tag_ids = resolve_tags(db_tx, tenant_id, tags)?;
        for tag_id in tag_ids {
            db_tx.execute(
                "INSERT OR IGNORE INTO transaction_tags (transaction_id, tag_id, tenant_id) VALUES (?1, ?2, ?3)",
                params![transaction_id, tag_id, tenant_id],
            )?;
        }
    }

    db::audit(
        conn,
        tenant_id,
        user_id,
        "transaction",
        &transaction_id,
        "create",
        None,
        Some(&serde_json::json!({
            "kind": kind,
            "amountMinor": amount_minor,
            "currency": tx_currency,
            "baseAmountMinor": base_amount,
            "status": status
        })),
    )?;

    Ok(PostResult {
        id: transaction_id,
        possible_duplicate_id: duplicate,
        amount: MoneyView::new(amount_minor, &tx_currency, tx_exp),
        base_amount: MoneyView::new(base_amount, &base_currency, base_exp),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRow {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub data_status: String,
    pub description: String,
    pub payee: Option<String>,
    pub notes: Option<String>,
    pub local_date: String,
    pub occurred_at: String,
    pub account_id: String,
    pub account_name: String,
    pub account_currency: String,
    pub counter_account_id: Option<String>,
    pub counter_account_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub income_source_id: Option<String>,
    pub amount_minor: i64,
    pub amount: String,
    pub base_minor: i64,
    pub base: String,
    pub base_currency: String,
    pub is_split: bool,
    pub recurring_rule_id: Option<String>,
    pub reversal_of_id: Option<String>,
    pub transfer_group_id: Option<String>,
    pub tags: Vec<String>,
    pub split_count: i64,
    pub attachment_count: i64,
    pub created_at: String,
    pub posted_at: Option<String>,
    pub reconciled_at: Option<String>,
    pub original_currency: String,
    pub fx_rate: Option<String>,
    pub fx_rate_date: Option<String>,
}

pub struct TxFilter<'a> {
    pub search: Option<&'a str>,
    pub account_id: Option<&'a str>,
    pub category_id: Option<&'a str>,
    pub kind: Option<&'a str>,
    pub status: Option<&'a str>,
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    pub limit: i64,
    pub offset: i64,
}

pub fn list_transactions(
    conn: &Connection,
    tenant_id: &str,
    filter: &TxFilter<'_>,
) -> AppResult<(Vec<TransactionRow>, i64)> {
    let base_currency = primary_currency(conn, tenant_id)?;
    let base_exp = exponent(conn, &base_currency)?;
    let mut where_sql = String::from(" WHERE t.tenant_id = ?1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(tenant_id.to_string())];

    if let Some(q) = filter.search.filter(|v| !v.trim().is_empty()) {
        let placeholder = args.len() + 1;
        where_sql.push_str(&format!(
            " AND (lower(t.description) LIKE ?{placeholder} ESCAPE '!'
                  OR lower(COALESCE(t.payee,'')) LIKE ?{placeholder} ESCAPE '!'
                  OR lower(COALESCE(t.notes,'')) LIKE ?{placeholder} ESCAPE '!')"
        ));
        args.push(Box::new(crate::db::like_contains(q)));
    }
    if let Some(v) = filter.account_id.filter(|v| !v.is_empty()) {
        let p = args.len() + 1;
        where_sql.push_str(&format!(
            " AND (t.account_id = ?{p} OR t.counter_account_id = ?{p})"
        ));
        args.push(Box::new(v.to_string()));
    }
    if let Some(v) = filter.category_id.filter(|v| !v.is_empty()) {
        let p = args.len() + 1;
        where_sql.push_str(&format!(
            " AND (t.category_id = ?{p} OR EXISTS (SELECT 1 FROM transaction_splits s WHERE s.transaction_id = t.id AND s.category_id = ?{p}))"
        ));
        args.push(Box::new(v.to_string()));
    }
    if let Some(v) = filter.kind.filter(|v| !v.is_empty()) {
        let p = args.len() + 1;
        where_sql.push_str(&format!(" AND t.kind = ?{p}"));
        args.push(Box::new(v.to_string()));
    }
    if let Some(v) = filter.status.filter(|v| !v.is_empty()) {
        if v == "effective" {
            where_sql
                .push_str(" AND t.status IN ('posted','reconciled') AND t.reversal_of_id IS NULL");
        } else if v == "needs_review" {
            where_sql.push_str(" AND (t.status IN ('draft','pending') OR t.data_status IN ('planned','estimated','forecast'))");
        } else {
            let p = args.len() + 1;
            where_sql.push_str(&format!(" AND t.status = ?{p}"));
            args.push(Box::new(v.to_string()));
        }
    }
    if let Some(v) = filter.from.filter(|v| !v.is_empty()) {
        let p = args.len() + 1;
        where_sql.push_str(&format!(" AND t.local_date >= ?{p}"));
        args.push(Box::new(v.to_string()));
    }
    if let Some(v) = filter.to.filter(|v| !v.is_empty()) {
        let p = args.len() + 1;
        where_sql.push_str(&format!(" AND t.local_date <= ?{p}"));
        args.push(Box::new(v.to_string()));
    }

    let count_sql = format!("SELECT count(*) FROM transactions t{where_sql}");
    let total: i64 = conn.query_row(
        &count_sql,
        rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())),
        |r| r.get(0),
    )?;

    let sql = format!(
        "SELECT t.id, t.kind, t.status, t.data_status, t.description, t.payee, t.notes, t.local_date, t.occurred_at,
                t.account_id, a.name, a.currency_code, t.counter_account_id, ca.name, t.category_id, c.name,
                t.income_source_id, t.original_amount_minor, t.original_currency, t.base_amount_minor, t.base_currency,
                t.is_split, t.recurring_rule_id, t.reversal_of_id, t.transfer_group_id, t.created_at, t.posted_at,
                t.reconciled_at, er.rate, er.as_of_date,
                (SELECT count(*) FROM transaction_splits s WHERE s.transaction_id = t.id),
                (SELECT count(*) FROM transaction_attachments at WHERE at.transaction_id = t.id),
                (SELECT group_concat(tg.name, '||') FROM transaction_tags tt JOIN tags tg ON tg.id = tt.tag_id WHERE tt.transaction_id = t.id)
         FROM transactions t
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN accounts ca ON ca.id = t.counter_account_id
         LEFT JOIN categories c ON c.id = t.category_id
         LEFT JOIN exchange_rates er ON er.id = t.fx_rate_id
         {where_sql}
         ORDER BY t.local_date DESC, t.created_at DESC
         LIMIT ?{} OFFSET ?{}",
        args.len() + 1,
        args.len() + 2
    );
    args.push(Box::new(filter.limit.clamp(1, 500)));
    args.push(Box::new(filter.offset.max(0)));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(
            rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())),
            |row| {
                let amount_minor: i64 = row.get(17)?;
                let base_minor: i64 = row.get(19)?;
                let account_currency: String = row.get(11)?;
                let original_currency: String = row.get(18)?;
                let account_exp = exponent(conn, &account_currency).unwrap_or(2);
                let tags_raw: Option<String> = row.get(32)?;
                Ok(TransactionRow {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    status: row.get(2)?,
                    data_status: row.get(3)?,
                    description: row.get(4)?,
                    payee: row.get(5)?,
                    notes: row.get(6)?,
                    local_date: row.get(7)?,
                    occurred_at: row.get(8)?,
                    account_id: row.get(9)?,
                    account_name: row.get(10)?,
                    account_currency,
                    counter_account_id: row.get(12)?,
                    counter_account_name: row.get(13)?,
                    category_id: row.get(14)?,
                    category_name: row.get(15)?,
                    income_source_id: row.get(16)?,
                    amount_minor,
                    amount: format_amount(amount_minor, account_exp),
                    base_minor,
                    base: format_amount(base_minor, base_exp),
                    base_currency: base_currency.clone(),
                    is_split: row.get::<_, i64>(21)? == 1,
                    recurring_rule_id: row.get(22)?,
                    reversal_of_id: row.get(23)?,
                    transfer_group_id: row.get(24)?,
                    created_at: row.get(25)?,
                    posted_at: row.get(26)?,
                    reconciled_at: row.get(27)?,
                    fx_rate: row.get(28)?,
                    fx_rate_date: row.get(29)?,
                    split_count: row.get(30)?,
                    attachment_count: row.get(31)?,
                    original_currency,
                    tags: tags_raw
                        .map(|v| v.split("||").map(str::to_string).collect())
                        .unwrap_or_default(),
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;

    Ok((rows, total))
}

pub fn get_transaction(
    conn: &Connection,
    tenant_id: &str,
    id: &str,
) -> AppResult<serde_json::Value> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.kind, t.status, t.data_status, t.description, t.payee, t.notes, t.local_date, t.occurred_at,
                t.account_id, a.name, t.counter_account_id, ca.name, t.category_id, c.name, t.original_amount_minor,
                t.original_currency, t.base_amount_minor, t.base_currency, t.is_split, t.recurring_rule_id, t.reversal_of_id,
                t.created_at, t.posted_at, t.reconciled_at, t.income_source_id, t.external_ref,
                er.rate, er.as_of_date, t.transfer_group_id,
                (SELECT count(*) FROM transaction_splits s WHERE s.transaction_id = t.id),
                (SELECT count(*) FROM transaction_attachments at WHERE at.transaction_id = t.id)
         FROM transactions t
         JOIN accounts a ON a.id = t.account_id
         LEFT JOIN accounts ca ON ca.id = t.counter_account_id
         LEFT JOIN categories c ON c.id = t.category_id
         LEFT JOIN exchange_rates er ON er.id = t.fx_rate_id
         WHERE t.id = ?1 AND t.tenant_id = ?2",
    )?;
    let mut result = stmt
        .query_row(params![id, tenant_id], |row| {
            let amount: i64 = row.get(15)?;
            let base: i64 = row.get(17)?;
            let currency: String = row.get(16)?;
            let base_currency: String = row.get(18)?;
            let exp = exponent(conn, &currency).unwrap_or(2);
            let base_exp = exponent(conn, &base_currency).unwrap_or(2);
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "kind": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "dataStatus": row.get::<_, String>(3)?,
                "description": row.get::<_, String>(4)?,
                "payee": row.get::<_, Option<String>>(5)?,
                "notes": row.get::<_, Option<String>>(6)?,
                "localDate": row.get::<_, String>(7)?,
                "occurredAt": row.get::<_, String>(8)?,
                "accountId": row.get::<_, String>(9)?,
                "accountName": row.get::<_, String>(10)?,
                "counterAccountId": row.get::<_, Option<String>>(11)?,
                "counterAccountName": row.get::<_, Option<String>>(12)?,
                "categoryId": row.get::<_, Option<String>>(13)?,
                "categoryName": row.get::<_, Option<String>>(14)?,
                "amountMinor": amount,
                "amount": format_amount(amount, exp),
                "originalCurrency": currency,
                "baseMinor": base,
                "base": format_amount(base, base_exp),
                "baseCurrency": base_currency,
                "isSplit": row.get::<_, i64>(19)? == 1,
                "recurringRuleId": row.get::<_, Option<String>>(20)?,
                "reversalOfId": row.get::<_, Option<String>>(21)?,
                "createdAt": row.get::<_, String>(22)?,
                "postedAt": row.get::<_, Option<String>>(23)?,
                "reconciledAt": row.get::<_, Option<String>>(24)?,
                "incomeSourceId": row.get::<_, Option<String>>(25)?,
                "externalRef": row.get::<_, Option<String>>(26)?,
                "fxRate": row.get::<_, Option<String>>(27)?,
                "fxRateDate": row.get::<_, Option<String>>(28)?,
                "transferGroupId": row.get::<_, Option<String>>(29)?,
                "splitCount": row.get::<_, i64>(30)?,
                "attachmentCount": row.get::<_, i64>(31)?,
            }))
        })
        .optional()?
        .ok_or_else(|| AppError::NotFound("Transaction not found.".into()))?;

    let exp = exponent(conn, result["originalCurrency"].as_str().unwrap_or("QAR"))?;
    let base_currency_text = result["baseCurrency"].as_str().unwrap_or("QAR").to_string();
    let base_exp = exponent(conn, &base_currency_text)?;
    let splits: Vec<serde_json::Value> = {
        let mut split_stmt = conn.prepare(
            "SELECT s.id, s.category_id, c.name, s.amount_minor, s.note FROM transaction_splits s
             JOIN categories c ON c.id = s.category_id WHERE s.transaction_id = ?1 ORDER BY s.sort_order",
        )?;
        let mapped = split_stmt.query_map([id], |row| {
            let amount: i64 = row.get(3)?;
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "categoryId": row.get::<_, String>(1)?,
                "categoryName": row.get::<_, String>(2)?,
                "amountMinor": amount,
                "amount": format_amount(amount, exp),
                "note": row.get::<_, Option<String>>(4)?,
            }))
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    let tags: Vec<serde_json::Value> = {
        let mut tag_stmt = conn.prepare(
            "SELECT tg.id, tg.name FROM transaction_tags tt JOIN tags tg ON tg.id = tt.tag_id WHERE tt.transaction_id = ?1",
        )?;
        let mapped = tag_stmt
            .query_map([id], |row| {
                Ok(serde_json::json!({"id": row.get::<_, String>(0)?, "name": row.get::<_, String>(1)?}))
            })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    let entries: Vec<serde_json::Value> = {
        let mut stmt = conn.prepare(
            "SELECT le.id, le.entry_type, le.amount_minor, le.currency_code, le.base_amount_minor,
                    le.account_id, a.name, le.category_id, c.name
             FROM ledger_entries le
             LEFT JOIN accounts a ON a.id = le.account_id
             LEFT JOIN categories c ON c.id = le.category_id
             WHERE le.transaction_id = ?1 ORDER BY le.entry_type DESC",
        )?;
        let mapped = stmt.query_map([id], |row| {
            let amount_minor: i64 = row.get(2)?;
            let currency: String = row.get(3)?;
            let base_minor: i64 = row.get(4)?;
            let entry_exp = exponent(conn, &currency).unwrap_or(2);
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "entryType": row.get::<_, String>(1)?,
                "amountMinor": amount_minor,
                "amount": format_amount(amount_minor, entry_exp),
                "currency": currency,
                "baseMinor": base_minor,
                "baseAmount": format_amount(base_minor, base_exp),
                "accountId": row.get::<_, Option<String>>(5)?,
                "accountName": row.get::<_, Option<String>>(6)?,
                "categoryId": row.get::<_, Option<String>>(7)?,
                "categoryName": row.get::<_, Option<String>>(8)?,
            }))
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    result["splits"] = serde_json::json!(splits);
    result["tags"] = serde_json::json!(tags);
    result["ledgerEntries"] = serde_json::json!(entries);
    Ok(result)
}

pub fn set_status(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
    new_status: &str,
) -> AppResult<()> {
    let current: String = conn
        .query_row(
            "SELECT status FROM transactions WHERE id = ?1 AND tenant_id = ?2",
            params![id, tenant_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Transaction not found.".into()))?;
    let allowed = matches!(
        (current.as_str(), new_status),
        ("draft", "pending")
            | ("draft", "posted")
            | ("pending", "posted")
            | ("posted", "reconciled")
            | ("reconciled", "posted")
            | ("draft", "cancelled")
            | ("pending", "cancelled")
    );
    if !allowed {
        return Err(AppError::Conflict(format!(
            "Cannot change a {current} transaction to {new_status}. Posted records must be reversed, not cancelled."
        )));
    }
    let now = now_iso();
    let mut sets = vec!["status = ?1".to_string(), "updated_at = ?2".to_string()];
    let mut args: Vec<Box<dyn rusqlite::ToSql>> =
        vec![Box::new(new_status.to_string()), Box::new(now.clone())];
    if new_status == "posted" {
        sets.push(format!(
            "posted_at = COALESCE(posted_at, ?{})",
            args.len() + 1
        ));
        args.push(Box::new(now.clone()));
    }
    if new_status == "reconciled" {
        sets.push(format!("reconciled_at = ?{}", args.len() + 1));
        args.push(Box::new(now.clone()));
    }
    if new_status == "cancelled" {
        sets.push(format!("cancelled_at = ?{}", args.len() + 1));
        args.push(Box::new(now.clone()));
    }
    let sql = format!(
        "UPDATE transactions SET {} WHERE id = ?{} AND tenant_id = ?{}",
        sets.join(", "),
        args.len() + 1,
        args.len() + 2
    );
    args.push(Box::new(id.to_string()));
    args.push(Box::new(tenant_id.to_string()));
    conn.execute(
        &sql,
        rusqlite::params_from_iter(args.iter().map(|b| b.as_ref())),
    )?;
    db::audit(
        conn,
        tenant_id,
        user_id,
        "transaction",
        id,
        &format!("status:{new_status}"),
        None,
        None,
    )?;
    Ok(())
}

pub fn reverse_transaction(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
) -> AppResult<String> {
    let original_status: String = conn
        .query_row(
            "SELECT status FROM transactions WHERE id = ?1 AND tenant_id = ?2",
            params![id, tenant_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Transaction not found.".into()))?;
    if !matches!(original_status.as_str(), "posted" | "reconciled") {
        return Err(AppError::Conflict(
            "Only posted or reconciled transactions can be reversed.".into(),
        ));
    }
    let already: i64 = conn.query_row(
        "SELECT count(*) FROM transactions WHERE reversal_of_id = ?1 AND status <> 'cancelled'",
        [id],
        |r| r.get(0),
    )?;
    if already > 0 {
        return Err(AppError::Conflict(
            "This transaction has already been reversed.".into(),
        ));
    }

    #[derive(Clone)]
    struct LegRow {
        account_id: Option<String>,
        category_id: Option<String>,
        entry_type: String,
        amount_minor: i64,
        currency: String,
        base_minor: i64,
        fx_rate_id: Option<String>,
        memo: Option<String>,
    }

    let legs: Vec<LegRow> = {
        let mut stmt = conn.prepare(
            "SELECT account_id, category_id, entry_type, amount_minor, currency_code, base_amount_minor, fx_rate_id, memo
             FROM ledger_entries WHERE transaction_id = ?1",
        )?;
        let mapped = stmt.query_map([id], |row| {
            Ok(LegRow {
                account_id: row.get(0)?,
                category_id: row.get(1)?,
                entry_type: row.get(2)?,
                amount_minor: row.get(3)?,
                currency: row.get(4)?,
                base_minor: row.get(5)?,
                fx_rate_id: row.get(6)?,
                memo: row.get(7)?,
            })
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    let splits: Vec<(String, i64, i64, Option<String>, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT category_id, amount_minor, base_amount_minor, note, sort_order FROM transaction_splits WHERE transaction_id = ?1",
        )?;
        let mapped = stmt.query_map([id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    let tags: Vec<String> = {
        let mut stmt =
            conn.prepare("SELECT tag_id FROM transaction_tags WHERE transaction_id = ?1")?;
        let mapped = stmt.query_map([id], |row| row.get(0))?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };

    let new_id = uuid::Uuid::new_v4().to_string();
    let now = now_iso();
    let today = iso(today_local());

    let db_tx = conn.unchecked_transaction()?;
    db_tx.execute(
        "INSERT INTO transactions
         (id, tenant_id, user_id, kind, status, data_status, description, payee, notes, local_date, occurred_at,
          account_id, counter_account_id, category_id, income_source_id, original_amount_minor, original_currency,
          fx_rate_id, counter_amount_minor, counter_currency, base_amount_minor, base_currency, is_split, dedupe_hash,
          external_ref, reversal_of_id, created_at, updated_at, posted_at)
         SELECT ?1, tenant_id, ?2, kind, 'posted', data_status, 'Reversal: ' || description, payee, notes,
                ?3, ?4, account_id, counter_account_id, category_id, income_source_id, original_amount_minor,
                original_currency, fx_rate_id, counter_amount_minor, counter_currency, base_amount_minor,
                base_currency, is_split, NULL, external_ref, id, ?5, ?5, ?5
         FROM transactions WHERE id = ?6 AND tenant_id = ?7",
        params![new_id, user_id, today, now, now, id, tenant_id],
    )?;

    for leg in &legs {
        db_tx.execute(
            "INSERT INTO ledger_entries
             (id, tenant_id, transaction_id, account_id, category_id, entry_type, amount_minor, currency_code,
              base_amount_minor, base_currency, fx_rate_id, memo, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                uuid::Uuid::new_v4().to_string(),
                tenant_id,
                new_id,
                leg.account_id,
                leg.category_id,
                if leg.entry_type == "debit" { "credit" } else { "debit" },
                leg.amount_minor,
                leg.currency,
                leg.base_minor,
                crate::services::currency::primary_currency(&db_tx, tenant_id)?,
                leg.fx_rate_id,
                leg.memo,
                now
            ],
        )?;
    }
    for (category_id, amount, base, note, sort_order) in &splits {
        db_tx.execute(
            "INSERT INTO transaction_splits (id, tenant_id, transaction_id, category_id, amount_minor, base_amount_minor, note, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                uuid::Uuid::new_v4().to_string(),
                tenant_id,
                new_id,
                category_id,
                amount,
                base,
                note,
                sort_order
            ],
        )?;
    }
    for tag_id in &tags {
        db_tx.execute(
            "INSERT INTO transaction_tags (transaction_id, tag_id, tenant_id) VALUES (?1, ?2, ?3)",
            params![new_id, tag_id, tenant_id],
        )?;
    }

    db_tx.execute(
        "UPDATE transactions SET status = 'reversed', updated_at = ?1, reconciled_at = NULL WHERE id = ?2 AND tenant_id = ?3",
        params![now, id, tenant_id],
    )?;
    let transfer_group = uuid::Uuid::new_v4().to_string();
    db_tx.execute(
        "UPDATE transactions SET transfer_group_id = ?1 WHERE id IN (?2, ?3)",
        params![transfer_group, id, new_id],
    )?;
    db_tx.commit()?;

    db::audit(
        conn,
        tenant_id,
        user_id,
        "transaction",
        id,
        "reverse",
        None,
        Some(&serde_json::json!({"reversalId": new_id})),
    )?;
    Ok(new_id)
}

pub fn update_transaction(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
    input: &TxInput,
) -> AppResult<()> {
    let (status, old_category, created_at): (String, Option<String>, String) = conn
        .query_row(
            "SELECT status, category_id, created_at FROM transactions WHERE id = ?1 AND tenant_id = ?2",
            params![id, tenant_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Transaction not found.".into()))?;

    if matches!(status.as_str(), "posted" | "reconciled") {
        if !input.amount.trim().is_empty() {
            let stored: i64 = conn.query_row(
                "SELECT original_amount_minor FROM transactions WHERE id = ?1",
                [id],
                |r| r.get(0),
            )?;
            let account_currency: String = conn.query_row(
                "SELECT a.currency_code FROM transactions t JOIN accounts a ON a.id = t.account_id WHERE t.id = ?1",
                [id],
                |r| r.get(0),
            )?;
            let requested = parse_amount(&input.amount, exponent(conn, &account_currency)?)?;
            if requested != stored {
                return Err(AppError::Conflict(
                    "Posted amounts cannot be edited. Reverse the transaction and record a corrected one.".into(),
                ));
            }
        }
        // Metadata and category edits must land together: a partial failure
        // would leave `transactions.category_id` and `ledger_entries.category_id`
        // disagreeing.
        let db_tx = conn.unchecked_transaction()?;
        db_tx.execute(
            "UPDATE transactions SET description = ?1, payee = ?2, notes = ?3, updated_at = ?4 WHERE id = ?5",
            params![
                input.description.trim(),
                input.payee.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                input.notes.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                now_iso(),
                id
            ],
        )?;
        if let Some(new_category) = input.category_id.as_deref().filter(|v| !v.is_empty()) {
            let is_split: i64 = db_tx.query_row(
                "SELECT is_split FROM transactions WHERE id = ?1",
                [id],
                |r| r.get(0),
            )?;
            if is_split == 0 && Some(new_category.to_string()) != old_category {
                let kind: String =
                    db_tx.query_row("SELECT kind FROM transactions WHERE id = ?1", [id], |r| {
                        r.get(0)
                    })?;
                if matches!(
                    kind.as_str(),
                    "expense" | "income" | "refund" | "fee" | "dividend"
                ) {
                    category_kind(&db_tx, tenant_id, new_category)?;
                    db_tx.execute(
                        "UPDATE ledger_entries SET category_id = ?1 WHERE transaction_id = ?2 AND category_id IS NOT NULL",
                        params![new_category, id],
                    )?;
                    db_tx.execute(
                        "UPDATE transactions SET category_id = ?1 WHERE id = ?2",
                        params![new_category, id],
                    )?;
                    db_tx.execute(
                        "INSERT INTO categorization_history (id, tenant_id, transaction_id, previous_category_id, new_category_id, actor, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, 'user', ?6)",
                        params![uuid::Uuid::new_v4().to_string(), tenant_id, id, old_category, new_category, now_iso()],
                    )?;
                }
            }
        }
        db::audit(
            &db_tx,
            tenant_id,
            user_id,
            "transaction",
            id,
            "update-metadata",
            None,
            None,
        )?;
        db_tx.commit()?;
        return Ok(());
    }

    if matches!(status.as_str(), "cancelled" | "reversed") {
        return Err(AppError::Conflict(
            "Cancelled or reversed transactions cannot be edited.".into(),
        ));
    }

    // Drafts and pending records can be fully re-entered. Delete only their own
    // rows, and re-create inside the same transaction: if re-creation fails
    // validation, the original record must survive untouched.
    let mut new_input = input.clone();
    new_input.status = Some(status);
    let db_tx = conn.unchecked_transaction()?;
    db_tx.execute(
        "DELETE FROM transaction_splits WHERE transaction_id = ?1",
        [id],
    )?;
    db_tx.execute("DELETE FROM ledger_entries WHERE transaction_id = ?1", [id])?;
    db_tx.execute(
        "DELETE FROM transaction_tags WHERE transaction_id = ?1",
        [id],
    )?;
    db_tx.execute(
        "DELETE FROM transactions WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
    )?;
    create_transaction_inner(
        &db_tx,
        tenant_id,
        user_id,
        Some(id),
        Some(&created_at),
        &new_input,
    )?;
    db_tx.commit()?;
    Ok(())
}

pub fn delete_draft(conn: &Connection, tenant_id: &str, user_id: &str, id: &str) -> AppResult<()> {
    let status: String = conn
        .query_row(
            "SELECT status FROM transactions WHERE id = ?1 AND tenant_id = ?2",
            params![id, tenant_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Transaction not found.".into()))?;
    if status != "draft" {
        return Err(AppError::Conflict(
            "Only draft transactions can be deleted. Posted records must be reversed.".into(),
        ));
    }
    conn.execute(
        "DELETE FROM transactions WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
    )?;
    db::audit(
        conn,
        tenant_id,
        user_id,
        "transaction",
        id,
        "delete-draft",
        None,
        None,
    )?;
    Ok(())
}

pub fn check_duplicate(
    conn: &Connection,
    tenant_id: &str,
    account_id: &str,
    date: &str,
    amount: &str,
    description: &str,
    kind: &str,
) -> AppResult<Option<String>> {
    let account = load_account(conn, tenant_id, account_id)?;
    let exp = exponent(conn, &account.currency)?;
    let minor = parse_amount(amount, exp)?;
    let hash = dedupe_hash(account_id, date, minor, description, kind);
    Ok(conn
        .query_row(
            "SELECT id FROM transactions WHERE tenant_id = ?1 AND dedupe_hash = ?2 AND status NOT IN ('cancelled','reversed') LIMIT 1",
            params![tenant_id, hash],
            |r| r.get(0),
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_test;

    fn setup() -> (Connection, String, String, String, String) {
        let mut conn = open_test();
        let (tenant, user) = db::seed_if_empty(&mut conn).unwrap();
        let account_id = uuid::Uuid::new_v4().to_string();
        let now = now_iso();
        conn.execute(
            "INSERT INTO accounts (id, tenant_id, user_id, name, type, currency_code, is_liability, is_liquid, opening_balance_minor, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'Main bank', 'bank', 'QAR', 0, 1, 0, ?4, ?4)",
            params![account_id, tenant, user, now],
        ).unwrap();
        let category_id: String = conn
            .query_row(
                "SELECT id FROM categories WHERE tenant_id = ?1 AND name = 'Groceries'",
                [&tenant],
                |r| r.get(0),
            )
            .unwrap();
        (conn, tenant, user, account_id, category_id)
    }

    fn input(account_id: &str, category_id: &str, amount: &str) -> TxInput {
        TxInput {
            kind: "expense".into(),
            status: Some("posted".into()),
            data_status: None,
            description: "Weekly groceries".into(),
            payee: Some("Market".into()),
            notes: None,
            date: "2026-11-14".into(),
            time: None,
            account_id: account_id.to_string(),
            counter_account_id: None,
            category_id: Some(category_id.to_string()),
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
    fn posting_an_expense_creates_balanced_entries() {
        let (conn, tenant, user, account, category) = setup();
        let result =
            create_transaction(&conn, &tenant, &user, &input(&account, &category, "250.00"))
                .unwrap();
        let (debits, credits): (i64, i64) = conn
            .query_row(
                "SELECT
               COALESCE(SUM(CASE WHEN entry_type='debit' THEN base_amount_minor END),0),
               COALESCE(SUM(CASE WHEN entry_type='credit' THEN base_amount_minor END),0)
             FROM ledger_entries WHERE transaction_id = ?1",
                [&result.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(debits, credits);
        assert_eq!(debits, 25000);
    }

    #[test]
    fn editing_a_draft_that_fails_validation_keeps_the_original() {
        let (conn, tenant, user, account, category) = setup();
        let mut draft = input(&account, &category, "250.00");
        draft.status = Some("draft".into());
        let created = create_transaction(&conn, &tenant, &user, &draft).unwrap();

        // Re-entering the draft against a nonexistent account must fail
        // validation without destroying the original record.
        let mut bad = input("no-such-account", &category, "250.00");
        bad.status = Some("draft".into());
        assert!(update_transaction(&conn, &tenant, &user, &created.id, &bad).is_err());

        let rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions WHERE id = ?1",
                [&created.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rows, 1, "the original draft must survive a failed edit");
        let legs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ledger_entries WHERE transaction_id = ?1",
                [&created.id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(legs > 0, "the original ledger entries must survive too");

        // A valid edit still works and keeps the id and creation time.
        let mut good = input(&account, &category, "300.00");
        good.status = Some("draft".into());
        good.description = "Updated".into();
        update_transaction(&conn, &tenant, &user, &created.id, &good).unwrap();
        let (description, amount): (String, i64) = conn
            .query_row(
                "SELECT description, original_amount_minor FROM transactions WHERE id = ?1",
                [&created.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(description, "Updated");
        assert_eq!(amount, 30_000);
    }

    #[test]
    fn account_balance_is_negative_after_expense() {
        let (conn, tenant, user, account, category) = setup();
        create_transaction(&conn, &tenant, &user, &input(&account, &category, "250.00")).unwrap();
        let balance: i64 = conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN le.entry_type='debit' THEN le.amount_minor ELSE -le.amount_minor END),0)
             FROM ledger_entries le JOIN transactions t ON t.id = le.transaction_id
             WHERE le.account_id = ?1 AND t.status IN ('posted','reconciled','reversed')",
            [&account],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(balance, -25000);
    }

    #[test]
    fn reversal_nets_to_zero_and_preserves_history() {
        let (conn, tenant, user, account, category) = setup();
        let first =
            create_transaction(&conn, &tenant, &user, &input(&account, &category, "100.00"))
                .unwrap();
        let reversal = reverse_transaction(&conn, &tenant, &user, &first.id).unwrap();
        assert_ne!(reversal, first.id);
        let original_status: String = conn
            .query_row(
                "SELECT status FROM transactions WHERE id = ?1",
                [&first.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(original_status, "reversed");
        let sum: i64 = conn.query_row(
            "SELECT COALESCE(SUM(CASE WHEN le.entry_type='debit' THEN le.amount_minor ELSE -le.amount_minor END),0)
             FROM ledger_entries le JOIN transactions t ON t.id=le.transaction_id
             WHERE le.account_id = ?1 AND t.status IN ('posted','reconciled','reversed')",
            [&account],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(sum, 0);
        let total_count: i64 = conn
            .query_row(
                "SELECT count(*) FROM transactions WHERE tenant_id = ?1",
                [&tenant],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(total_count, 2);
    }

    #[test]
    fn split_transactions_must_balance() {
        let (conn, tenant, user, account, category) = setup();
        let mut tx = input(&account, &category, "100.00");
        tx.splits = Some(vec![
            SplitInput {
                category_id: category.clone(),
                amount: "60.00".into(),
                note: None,
            },
            SplitInput {
                category_id: category,
                amount: "30.00".into(),
                note: None,
            },
        ]);
        let err = create_transaction(&conn, &tenant, &user, &tx).unwrap_err();
        assert!(err.to_string().contains("must add up"));
    }
}
