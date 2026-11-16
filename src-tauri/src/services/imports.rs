use std::path::Path;
use std::str::FromStr;

use calamine::{open_workbook_auto, Data, Reader};
use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::dates::{iso, now_iso};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::money::{amount_to_minor, format_amount};
use crate::services::currency::exponent;
use crate::services::tx::{self, TxInput};

const MAX_ROWS: usize = 20_000;
const PREVIEW_ROWS: i64 = 200;
const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMapping {
    pub date: Option<String>,
    pub description: Option<String>,
    pub amount: Option<String>,
    pub debit: Option<String>,
    pub credit: Option<String>,
    pub payee: Option<String>,
    pub category: Option<String>,
    pub currency: Option<String>,
    pub reference: Option<String>,
    pub date_order: String,
    pub decimal_separator: String,
    pub default_kind: String,
    pub skip_duplicates: bool,
    pub status: String,
}

impl ColumnMapping {
    fn suggested(headers: &[String]) -> Self {
        let find = |candidates: &[&str]| -> Option<String> {
            for header in headers {
                let normalized = header.trim().to_lowercase();
                if candidates.iter().any(|candidate| normalized == *candidate) {
                    return Some(header.clone());
                }
            }
            for header in headers {
                let normalized = header.trim().to_lowercase();
                if candidates.iter().any(|candidate| normalized.contains(candidate)) {
                    return Some(header.clone());
                }
            }
            None
        };
        Self {
            date: find(&["date", "posted", "posting date", "transaction date", "value date"]),
            description: find(&["description", "details", "memo", "narrative", "particulars"]),
            amount: find(&["amount", "transaction amount", "value"]),
            debit: find(&["debit", "withdrawal", "withdrawals", "money out", "paid out"]),
            credit: find(&["credit", "deposit", "deposits", "money in", "paid in"]),
            payee: find(&["payee", "merchant", "name", "beneficiary"]),
            category: find(&["category"]),
            currency: find(&["currency", "ccy"]),
            reference: find(&["reference", "ref", "fitid", "transaction id", "cheque"]),
            date_order: "auto".into(),
            decimal_separator: "auto".into(),
            default_kind: "auto".into(),
            skip_duplicates: true,
            status: "posted".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreviewRow {
    pub row_index: i64,
    pub cells: Vec<String>,
    pub normalized: Option<serde_json::Value>,
    pub status: String,
    pub issue: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub job_id: String,
    pub file_name: String,
    pub format: String,
    pub file_hash: String,
    pub row_count: i64,
    pub headers: Vec<String>,
    pub rows: Vec<ImportPreviewRow>,
    pub suggested_mapping: ColumnMapping,
    pub warnings: Vec<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportJobDto {
    pub id: String,
    pub file_name: String,
    pub format: String,
    pub status: String,
    pub row_count: i64,
    pub imported_count: i64,
    pub duplicate_count: i64,
    pub error_count: i64,
    pub account_id: Option<String>,
    pub created_at: String,
    pub committed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRowIssue {
    pub row_index: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub job_id: String,
    pub imported: i64,
    pub duplicates: i64,
    pub errors: i64,
    pub skipped: i64,
    pub issues: Vec<ImportRowIssue>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NormalizedRow {
    date: String,
    description: String,
    payee: Option<String>,
    amount: Option<String>,
    debit: Option<String>,
    credit: Option<String>,
    category: Option<String>,
    currency: Option<String>,
    reference: Option<String>,
    #[serde(default)]
    issue: Option<String>,
}

struct ParsedRow {
    cells: Vec<String>,
    normalized: NormalizedRow,
}

struct ParsedFile {
    headers: Vec<String>,
    rows: Vec<ParsedRow>,
    warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Preview
// ---------------------------------------------------------------------------

pub fn preview(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    path: &str,
    format: &str,
    account_id: Option<&str>,
) -> AppResult<ImportPreview> {
    let format = format.trim().to_lowercase();
    if !matches!(format.as_str(), "csv" | "xlsx" | "ofx" | "qif") {
        return Err(AppError::Unsupported(match format.as_str() {
            "pdf" => "PDF statements are not parsed in this build. PDF layouts vary too much to import reliably; export CSV, OFX or QIF from your bank instead. Nothing is guessed from a PDF.".into(),
            "json" => "JSON import is not available. Use CSV, XLSX, OFX or QIF.".into(),
            _ => format!("Unsupported import format '{format}'. Use CSV, XLSX, OFX or QIF."),
        }));
    }
    let path_ref = Path::new(path);
    if !path_ref.is_file() {
        return Err(AppError::Validation("That file does not exist.".into()));
    }
    let metadata = std::fs::metadata(path_ref)?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(AppError::Validation(
            "That file is larger than 64 MB. Split the statement into smaller periods.".into(),
        ));
    }
    if let Some(account_id) = account_id {
        tx::load_account(conn, tenant_id, account_id)?;
    }

    let bytes = std::fs::read(path_ref)?;
    let file_hash = hex::encode(Sha256::digest(&bytes));
    let file_name = path_ref
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("statement")
        .to_string();

    let parsed = match format.as_str() {
        "csv" => parse_csv(&bytes)?,
        "xlsx" => parse_xlsx(path_ref)?,
        "ofx" => parse_ofx(&bytes)?,
        "qif" => parse_qif(&bytes)?,
        _ => unreachable!(),
    };

    let mut warnings = parsed.warnings;
    if parsed.rows.len() > MAX_ROWS {
        warnings.push(format!(
            "Only the first {MAX_ROWS} rows are read. Split the file if you need the rest."
        ));
    }
    let rows: Vec<ParsedRow> = parsed.rows.into_iter().take(MAX_ROWS).collect();
    let row_count = rows.len() as i64;

    let already_seen: Option<String> = conn
        .query_row(
            "SELECT id FROM import_jobs
             WHERE tenant_id = ?1 AND file_hash = ?2 AND status = 'committed'
             ORDER BY created_at DESC LIMIT 1",
            params![tenant_id, file_hash],
            |row| row.get(0),
        )
        .optional()?;
    if already_seen.is_some() {
        warnings.push(
            "A file with the same fingerprint was imported before. Duplicate detection still applies per row."
                .into(),
        );
    }

    let suggested_mapping = if matches!(format.as_str(), "csv" | "xlsx") {
        ColumnMapping::suggested(&parsed.headers)
    } else {
        ColumnMapping {
            date_order: "auto".into(),
            decimal_separator: "auto".into(),
            default_kind: "auto".into(),
            skip_duplicates: true,
            status: "posted".into(),
            ..ColumnMapping::default()
        }
    };

    let account_currency = match account_id {
        Some(id) => Some(tx::load_account(conn, tenant_id, id)?.currency),
        None => None,
    };
    let job_id = uuid::Uuid::new_v4().to_string();
    let now = now_iso();
    conn.execute(
        "INSERT INTO import_jobs
         (id, tenant_id, user_id, file_name, file_hash, format, status, account_id, mapping_json,
          row_count, imported_count, duplicate_count, error_count, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'parsed', ?7, ?8, ?9, 0, 0, 0, ?10)",
        params![
            job_id,
            tenant_id,
            user_id,
            file_name,
            file_hash,
            format,
            account_id,
            serde_json::to_string(&suggested_mapping)?,
            row_count,
            now
        ],
    )?;

    let db_tx = conn.unchecked_transaction()?;
    {
        let mut stmt = db_tx.prepare(
            "INSERT INTO import_rows
             (id, tenant_id, import_job_id, row_index, raw_json, normalized_json, status, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for (index, row) in rows.iter().enumerate() {
            let mapped = if matches!(format.as_str(), "csv" | "xlsx") {
                apply_mapping(&row.cells, &parsed.headers, &suggested_mapping)
            } else {
                row.normalized.clone()
            };
            let issue = mapped
                .issue
                .clone()
                .or_else(|| preview_issue(&mapped, &suggested_mapping, account_currency.as_deref()));
            let status = if issue.is_some() { "error" } else { "ok" };
            stmt.execute(params![
                uuid::Uuid::new_v4().to_string(),
                tenant_id,
                job_id,
                index as i64,
                serde_json::to_string(&row.cells)?,
                serde_json::to_string(&mapped)?,
                status,
                issue
            ])?;
        }
    }
    db_tx.commit()?;

    let mut preview_rows = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT row_index, raw_json, normalized_json, status, error_message
             FROM import_rows WHERE import_job_id = ?1 ORDER BY row_index LIMIT ?2",
        )?;
        let mut mapped = stmt.query_map(params![job_id, PREVIEW_ROWS], |row| {
            let raw: String = row.get(1)?;
            let normalized: String = row.get(2)?;
            Ok(ImportPreviewRow {
                row_index: row.get(0)?,
                cells: serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default(),
                normalized: serde_json::from_str::<serde_json::Value>(&normalized).ok(),
                status: row.get(3)?,
                issue: row.get(4)?,
            })
        })?;
        while let Some(item) = mapped.next() {
            preview_rows.push(item?);
        }
    }

    Ok(ImportPreview {
        job_id,
        file_name,
        format,
        file_hash,
        row_count,
        headers: parsed.headers,
        rows: preview_rows,
        suggested_mapping,
        warnings,
        account_id: account_id.map(str::to_string),
    })
}

fn preview_issue(row: &NormalizedRow, mapping: &ColumnMapping, account_currency: Option<&str>) -> Option<String> {
    if row.issue.is_some() {
        return row.issue.clone();
    }
    if !mapping_is_usable(row) {
        return Some("No amount or debit/credit value in this row.".into());
    }
    if parse_flexible_date(&row.date, &mapping.date_order).is_none() {
        return Some(format!("Unreadable date '{}'.", row.date));
    }
    if let Some(raw) = row.amount.as_deref().filter(|v| !v.trim().is_empty()) {
        if parse_money(raw, &mapping.decimal_separator).ok().flatten().is_none() {
            return Some(format!("Unreadable amount '{raw}'."));
        }
    }
    for raw in [row.debit.as_deref(), row.credit.as_deref()].into_iter().flatten() {
        if !raw.trim().is_empty() && parse_money(raw, &mapping.decimal_separator).ok().flatten().is_none() {
            return Some(format!("Unreadable amount '{raw}'."));
        }
    }
    if let (Some(row_currency), Some(account_currency)) = (row.currency.as_deref(), account_currency) {
        if !row_currency.trim().is_empty() && !row_currency.trim().eq_ignore_ascii_case(account_currency) {
            return Some(format!(
                "Statement currency {} does not match the account currency {account_currency}.",
                row_currency.trim()
            ));
        }
    }
    None
}

fn mapping_is_usable(row: &NormalizedRow) -> bool {
    row.amount.as_deref().map(|v| !v.trim().is_empty()).unwrap_or(false)
        || row.debit.as_deref().map(|v| !v.trim().is_empty()).unwrap_or(false)
        || row.credit.as_deref().map(|v| !v.trim().is_empty()).unwrap_or(false)
}

fn column_index(headers: &[String], name: Option<&String>) -> Option<usize> {
    let name = name?;
    if let Some(index) = headers.iter().position(|header| header == name) {
        return Some(index);
    }
    headers
        .iter()
        .position(|header| header.trim().eq_ignore_ascii_case(name.trim()))
}

fn cell_at(cells: &[String], headers: &[String], name: Option<&String>) -> Option<String> {
    let index = column_index(headers, name)?;
    cells
        .get(index)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn apply_mapping(cells: &[String], headers: &[String], mapping: &ColumnMapping) -> NormalizedRow {
    NormalizedRow {
        date: cell_at(cells, headers, mapping.date.as_ref()).unwrap_or_default(),
        description: cell_at(cells, headers, mapping.description.as_ref()).unwrap_or_default(),
        payee: cell_at(cells, headers, mapping.payee.as_ref()),
        amount: cell_at(cells, headers, mapping.amount.as_ref()),
        debit: cell_at(cells, headers, mapping.debit.as_ref()),
        credit: cell_at(cells, headers, mapping.credit.as_ref()),
        category: cell_at(cells, headers, mapping.category.as_ref()),
        currency: cell_at(cells, headers, mapping.currency.as_ref()),
        reference: cell_at(cells, headers, mapping.reference.as_ref()),
        issue: None,
    }
}

// ---------------------------------------------------------------------------
// Commit
// ---------------------------------------------------------------------------

pub fn commit(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    job_id: &str,
    mapping: &ColumnMapping,
    headers: &[String],
    account_override: Option<&str>,
) -> AppResult<ImportResult> {
    let (job_account, job_status, job_format): (Option<String>, String, String) = conn
        .query_row(
            "SELECT account_id, status, format FROM import_jobs WHERE id = ?1 AND tenant_id = ?2",
            params![job_id, tenant_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Import job not found.".into()))?;
    if job_status != "parsed" {
        return Err(AppError::Conflict(
            "This import has already been committed or cancelled. Start a new preview.".into(),
        ));
    }
    let account_id = account_override
        .map(str::to_string)
        .or(job_account)
        .ok_or_else(|| AppError::Validation("Choose the account this statement belongs to.".into()))?;
    let account = tx::load_account(conn, tenant_id, &account_id)?;
    if !account.is_active {
        return Err(AppError::Validation("The selected account is archived.".into()));
    }
    let exp = exponent(conn, &account.currency)?;
    let status = match mapping.status.as_str() {
        "pending" => "pending",
        "posted" | "" => "posted",
        other => {
            return Err(AppError::Validation(format!(
                "Imported rows must be posted or pending, not '{other}'."
            )))
        }
    };
    if !["auto", "expense", "income"].contains(&mapping.default_kind.as_str()) {
        return Err(AppError::Validation(
            "Default type must be auto, expense or income.".into(),
        ));
    }

    let expense_category = ensure_category(conn, tenant_id, user_id, "Uncategorised expense", "expense")?;
    let income_category = ensure_category(conn, tenant_id, user_id, "Uncategorised income", "income")?;
    let equity_category = ensure_category(conn, tenant_id, user_id, "Imported balance change", "equity")?;

    let mut rows: Vec<(i64, Vec<String>, NormalizedRow)> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT row_index, raw_json, normalized_json FROM import_rows
             WHERE import_job_id = ?1 ORDER BY row_index",
        )?;
        let mut mapped = stmt.query_map(params![job_id], |row| {
            let raw: String = row.get(1)?;
            let normalized: String = row.get(2)?;
            Ok((
                row.get::<_, i64>(0)?,
                serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default(),
                serde_json::from_str::<NormalizedRow>(&normalized).unwrap_or_default(),
            ))
        })?;
        while let Some(item) = mapped.next() {
            rows.push(item?);
        }
    }

    let tabular = matches!(job_format.as_str(), "csv" | "xlsx");
    let mut imported = 0i64;
    let mut duplicates = 0i64;
    let mut errors = 0i64;
    let mut skipped = 0i64;
    let mut issues: Vec<ImportRowIssue> = Vec::new();

    for (row_index, cells, stored) in rows {
        let row = if tabular {
            apply_mapping(&cells, headers, mapping)
        } else {
            stored
        };
        if row.issue.is_some() {
            skipped += 1;
            update_row(conn, job_id, row_index, "skipped", row.issue.as_deref(), None)?;
            continue;
        }
        match commit_row(
            conn,
            tenant_id,
            user_id,
            &account,
            exp,
            &row,
            mapping,
            status,
            &expense_category,
            &income_category,
            &equity_category,
        ) {
            Ok(Outcome::Imported(id)) => {
                imported += 1;
                update_row(conn, job_id, row_index, "imported", None, Some(&id))?;
            }
            Ok(Outcome::Duplicate) => {
                duplicates += 1;
                update_row(conn, job_id, row_index, "duplicate", Some("Already recorded."), None)?;
            }
            Err(error) => {
                errors += 1;
                let message = error.to_string();
                if issues.len() < 25 {
                    issues.push(ImportRowIssue {
                        row_index,
                        message: message.clone(),
                    });
                }
                update_row(conn, job_id, row_index, "error", Some(&message), None)?;
            }
        }
    }

    let committed_at = now_iso();
    conn.execute(
        "UPDATE import_jobs
         SET status = 'committed', account_id = ?1, mapping_json = ?2, imported_count = ?3,
             duplicate_count = ?4, error_count = ?5, committed_at = ?6
         WHERE id = ?7 AND tenant_id = ?8",
        params![
            account_id,
            serde_json::to_string(mapping)?,
            imported,
            duplicates,
            errors,
            committed_at,
            job_id,
            tenant_id
        ],
    )?;

    db::audit(
        conn,
        tenant_id,
        user_id,
        "import_job",
        job_id,
        "commit",
        None,
        Some(&json!({
            "imported": imported,
            "duplicates": duplicates,
            "errors": errors,
            "skipped": skipped,
            "accountId": account_id,
        })),
    )?;

    Ok(ImportResult {
        job_id: job_id.to_string(),
        imported,
        duplicates,
        errors,
        skipped,
        issues,
    })
}

enum Outcome {
    Imported(String),
    Duplicate,
}

#[allow(clippy::too_many_arguments)]
fn commit_row(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    account: &tx::AccountInfo,
    exp: u32,
    row: &NormalizedRow,
    mapping: &ColumnMapping,
    status: &str,
    expense_category: &str,
    income_category: &str,
    equity_category: &str,
) -> AppResult<Outcome> {
    let date = parse_flexible_date(&row.date, &mapping.date_order)
        .ok_or_else(|| AppError::Validation(format!("Unreadable date '{}'.", row.date)))?;
    let local_date = iso(date);

    let description = {
        let candidate = row.description.trim();
        if !candidate.is_empty() {
            candidate.to_string()
        } else if let Some(payee) = row.payee.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            payee.to_string()
        } else if let Some(reference) = row.reference.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            format!("Imported transaction {reference}")
        } else {
            "Imported transaction".to_string()
        }
    };
    if description.len() > 300 {
        return Err(AppError::Validation(
            "Description is longer than 300 characters.".into(),
        ));
    }

    if let Some(currency) = row.currency.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
        if !currency.eq_ignore_ascii_case(&account.currency) {
            return Err(AppError::Validation(format!(
                "Statement currency {currency} does not match the account currency {}.",
                account.currency
            )));
        }
    }

    let debit = parse_optional_money(row.debit.as_deref(), &mapping.decimal_separator)?.abs();
    let credit = parse_optional_money(row.credit.as_deref(), &mapping.decimal_separator)?.abs();
    let signed = parse_optional_money(row.amount.as_deref(), &mapping.decimal_separator)?;

    let (kind, magnitude, direction) = if debit > Decimal::ZERO || credit > Decimal::ZERO {
        if debit > Decimal::ZERO && credit > Decimal::ZERO {
            return Err(AppError::Validation(
                "Both debit and credit are filled in for this row.".into(),
            ));
        }
        if debit > Decimal::ZERO {
            ("expense", debit, None)
        } else {
            ("income", credit, None)
        }
    } else if signed != Decimal::ZERO {
        let negative = signed < Decimal::ZERO;
        let magnitude = signed.abs();
        if account.is_liability && !negative {
            // A positive movement on a liability account reduces what is owed. The
            // source account is unknown, so this is recorded as a balance change
            // rather than invented as income or a transfer.
            ("adjustment", magnitude, Some("in"))
        } else {
            match mapping.default_kind.as_str() {
                "expense" => ("expense", magnitude, None),
                "income" => ("income", magnitude, None),
                _ if negative => ("expense", magnitude, None),
                _ => ("income", magnitude, None),
            }
        }
    } else {
        return Err(AppError::Validation("The amount is zero or missing.".into()));
    };

    let amount_minor = amount_to_minor(magnitude, exp)?;
    if amount_minor <= 0 {
        return Err(AppError::Validation("The amount is too small.".into()));
    }

    let category_id = match kind {
        "expense" => resolve_category(conn, tenant_id, row.category.as_deref(), "expense")?
            .unwrap_or_else(|| expense_category.to_string()),
        "income" => resolve_category(conn, tenant_id, row.category.as_deref(), "income")?
            .unwrap_or_else(|| income_category.to_string()),
        "adjustment" => equity_category.to_string(),
        _ => expense_category.to_string(),
    };

    let hash = tx::dedupe_hash(&account.id, &local_date, amount_minor, &description, kind);
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM transactions
             WHERE tenant_id = ?1 AND account_id = ?2 AND dedupe_hash = ?3
               AND status NOT IN ('cancelled','reversed')
             ORDER BY created_at DESC LIMIT 1",
            params![tenant_id, account.id, hash],
            |row| row.get(0),
        )
        .optional()?;
    if existing.is_some() && mapping.skip_duplicates {
        return Ok(Outcome::Duplicate);
    }

    let input = TxInput {
        kind: kind.to_string(),
        status: Some(status.to_string()),
        data_status: Some(if status == "pending" { "pending" } else { "actual" }.into()),
        description,
        payee: row
            .payee
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        notes: Some("Imported from a statement.".to_string()),
        date: local_date,
        time: None,
        account_id: account.id.clone(),
        counter_account_id: None,
        category_id: Some(category_id),
        income_source_id: None,
        amount: format_amount(amount_minor, exp),
        currency: Some(account.currency.clone()),
        counter_amount: None,
        fx_rate: None,
        splits: None,
        tags: None,
        external_ref: row
            .reference
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string),
        direction: direction.map(str::to_string),
    };

    let result = tx::create_transaction(conn, tenant_id, user_id, &input)?;
    Ok(Outcome::Imported(result.id))
}

fn parse_optional_money(raw: Option<&str>, separator: &str) -> AppResult<Decimal> {
    match raw {
        Some(value) if !value.trim().is_empty() => {
            Ok(parse_money(value, separator)?.unwrap_or(Decimal::ZERO))
        }
        _ => Ok(Decimal::ZERO),
    }
}

fn update_row(
    conn: &Connection,
    job_id: &str,
    row_index: i64,
    status: &str,
    error: Option<&str>,
    transaction_id: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "UPDATE import_rows
         SET status = ?1, error_message = ?2, transaction_id = ?3
         WHERE import_job_id = ?4 AND row_index = ?5",
        params![status, error, transaction_id, job_id, row_index],
    )?;
    Ok(())
}

fn ensure_category(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    name: &str,
    kind: &str,
) -> AppResult<String> {
    if let Some(existing) = conn
        .query_row(
            "SELECT id FROM categories
             WHERE tenant_id = ?1 AND name = ?2 AND parent_id IS NULL LIMIT 1",
            params![tenant_id, name],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(existing);
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO categories
         (id, tenant_id, user_id, parent_id, name, kind, icon, color, is_system, is_archived, sort_order, created_at)
         VALUES (?1, ?2, ?3, NULL, ?4, ?5, NULL, NULL, 1, 0, 900, ?6)",
        params![id, tenant_id, user_id, name, kind, now_iso()],
    )?;
    Ok(id)
}

fn resolve_category(
    conn: &Connection,
    tenant_id: &str,
    raw: Option<&str>,
    kind: &str,
) -> AppResult<Option<String>> {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let cleaned = raw
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim()
        .to_string();
    if cleaned.is_empty() {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            "SELECT id FROM categories
             WHERE tenant_id = ?1 AND lower(name) = lower(?2) AND kind = ?3 AND parent_id IS NULL
             LIMIT 1",
            params![tenant_id, cleaned, kind],
            |row| row.get(0),
        )
        .optional()?)
}

// ---------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------

pub fn list_jobs(conn: &Connection, tenant_id: &str, limit: Option<i64>) -> AppResult<Vec<ImportJobDto>> {
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let mut stmt = conn.prepare(
        "SELECT id, file_name, format, status, row_count, imported_count, duplicate_count,
                error_count, account_id, created_at, committed_at
         FROM import_jobs WHERE tenant_id = ?1
         ORDER BY created_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![tenant_id, limit], |row| {
        Ok(ImportJobDto {
            id: row.get(0)?,
            file_name: row.get(1)?,
            format: row.get(2)?,
            status: row.get(3)?,
            row_count: row.get(4)?,
            imported_count: row.get(5)?,
            duplicate_count: row.get(6)?,
            error_count: row.get(7)?,
            account_id: row.get(8)?,
            created_at: row.get(9)?,
            committed_at: row.get(10)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn cancel(conn: &Connection, tenant_id: &str, job_id: &str) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE import_jobs SET status = 'cancelled'
         WHERE id = ?1 AND tenant_id = ?2 AND status = 'parsed'",
        params![job_id, tenant_id],
    )?;
    if changed == 0 {
        return Err(AppError::Conflict(
            "Only a preview that has not been committed can be cancelled.".into(),
        ));
    }
    conn.execute(
        "UPDATE import_rows SET status = 'skipped'
         WHERE import_job_id = ?1 AND status NOT IN ('imported','duplicate','error')",
        params![job_id],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// CSV / XLSX parsing
// ---------------------------------------------------------------------------

fn decode_text(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes).to_string();
    text.trim_start_matches('\u{feff}').to_string()
}

fn detect_delimiter(sample: &str) -> u8 {
    let line = sample.lines().find(|line| !line.trim().is_empty()).unwrap_or("");
    let candidates = [b',', b';', b'\t', b'|'];
    let mut best = (b',', 0usize);
    for delimiter in candidates {
        let count = line.bytes().filter(|byte| *byte == delimiter).count();
        if count > best.1 {
            best = (delimiter, count);
        }
    }
    best.0
}

fn parse_csv(bytes: &[u8]) -> AppResult<ParsedFile> {
    let text = decode_text(bytes);
    if text.trim().is_empty() {
        return Err(AppError::Validation("The file is empty.".into()));
    }
    let delimiter = detect_delimiter(&text);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter)
        .from_reader(text.as_bytes());

    let mut records: Vec<Vec<String>> = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|error| AppError::Validation(format!("Could not read the CSV: {error}")))?;
        records.push(record.iter().map(|cell| cell.trim().to_string()).collect());
        if records.len() > MAX_ROWS + 1 {
            break;
        }
    }
    if records.is_empty() {
        return Err(AppError::Validation("The file has no rows.".into()));
    }

    let headers = dedupe_headers(&records.remove(0));
    let mut warnings = Vec::new();
    if headers.iter().all(|header| header.trim().is_empty()) {
        warnings.push("The first row is empty; column names were generated.".into());
    }
    let rows = records
        .into_iter()
        .enumerate()
        .map(|(index, mut cells)| {
            cells.resize(headers.len(), String::new());
            let empty = cells.iter().all(|cell| cell.trim().is_empty());
            ParsedRow {
                normalized: NormalizedRow {
                    issue: if empty {
                        Some(format!("Row {} is empty.", index + 2))
                    } else {
                        None
                    },
                    ..NormalizedRow::default()
                },
                cells,
            }
        })
        .collect();

    Ok(ParsedFile {
        headers,
        rows,
        warnings,
    })
}

fn dedupe_headers(raw: &[String]) -> Vec<String> {
    let mut headers: Vec<String> = Vec::new();
    for (index, value) in raw.iter().enumerate() {
        let base = if value.trim().is_empty() {
            format!("Column {}", index + 1)
        } else {
            value.trim().to_string()
        };
        let mut candidate = base.clone();
        let mut suffix = 2;
        while headers.iter().any(|existing| existing == &candidate) {
            candidate = format!("{base} ({suffix})");
            suffix += 1;
        }
        headers.push(candidate);
    }
    headers
}

fn parse_xlsx(path: &Path) -> AppResult<ParsedFile> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|error| AppError::Validation(format!("Could not open the workbook: {error}")))?;
    let range = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| AppError::Validation("The workbook has no worksheets.".into()))?
        .map_err(|error| AppError::Validation(format!("Could not read the first worksheet: {error}")))?;

    let mut rows_raw: Vec<Vec<String>> = range
        .rows()
        .map(|row| row.iter().map(cell_to_string).collect())
        .collect();
    if rows_raw.is_empty() {
        return Err(AppError::Validation("The first worksheet is empty.".into()));
    }
    let headers = dedupe_headers(&rows_raw.remove(0));
    let mut warnings = vec![
        "Only the first worksheet is read. Dates are interpreted from the cell's own value.".into(),
    ];
    if rows_raw.len() > MAX_ROWS {
        warnings.push(format!("Only the first {MAX_ROWS} rows are read."));
    }

    let rows = rows_raw
        .into_iter()
        .take(MAX_ROWS)
        .enumerate()
        .map(|(index, mut cells)| {
            cells.resize(headers.len(), String::new());
            let empty = cells.iter().all(|cell| cell.trim().is_empty());
            ParsedRow {
                normalized: NormalizedRow {
                    issue: if empty {
                        Some(format!("Row {} is empty.", index + 2))
                    } else {
                        None
                    },
                    ..NormalizedRow::default()
                },
                cells,
            }
        })
        .collect();

    Ok(ParsedFile {
        headers,
        rows,
        warnings,
    })
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.trim().to_string(),
        Data::Float(value) => {
            if value.fract() == 0.0 {
                format!("{}", *value as i64)
            } else {
                format!("{value}")
            }
        }
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value
            .as_datetime()
            .map(format_excel_datetime)
            .unwrap_or_else(|| format!("{value:?}")),
        Data::DateTimeIso(value) => value.clone(),
        Data::DurationIso(value) => value.clone(),
        Data::Error(error) => format!("{error:?}"),
    }
}

fn format_excel_datetime(value: NaiveDateTime) -> String {
    use chrono::Timelike;
    if value.hour() == 0 && value.minute() == 0 && value.second() == 0 {
        value.date().format("%Y-%m-%d").to_string()
    } else {
        value.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}

// ---------------------------------------------------------------------------
// OFX / QIF parsing
// ---------------------------------------------------------------------------

fn tag_value(block: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = block.find(&open)? + open.len();
    let rest = &block[start..];
    let end = rest.find('<').unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn parse_ofx(bytes: &[u8]) -> AppResult<ParsedFile> {
    let text = decode_text(bytes);
    if !text.to_uppercase().contains("<STMTTRN>") {
        return Err(AppError::Validation(
            "No OFX transactions were found. The file may use an unsupported layout.".into(),
        ));
    }
    let mut rows = Vec::new();
    for chunk in text.split("<STMTTRN>").skip(1) {
        let block = chunk.split("</STMTTRN>").next().unwrap_or(chunk);
        let posted = tag_value(block, "DTPOSTED").unwrap_or_default();
        let date = ofx_date(&posted).unwrap_or_default();
        let amount = tag_value(block, "TRNAMT");
        let name = tag_value(block, "NAME");
        let memo = tag_value(block, "MEMO");
        let description = name
            .clone()
            .or_else(|| memo.clone())
            .unwrap_or_else(|| "Imported transaction".to_string());
        let reference = tag_value(block, "FITID");
        let currency = tag_value(block, "CURSYM");
        let transaction_type = tag_value(block, "TRNTYPE").unwrap_or_default().to_uppercase();
        let issue = if date.is_empty() {
            Some("OFX row has no readable posting date.".into())
        } else if amount.is_none() {
            Some("OFX row has no amount.".into())
        } else if transaction_type == "XFER" || transaction_type == "TRANSFER" {
            Some("Transfers need two accounts and are skipped by the importer.".into())
        } else {
            None
        };
        let cells = vec![
            date.clone(),
            amount.clone().unwrap_or_default(),
            description.clone(),
            name.clone().unwrap_or_default(),
            reference.clone().unwrap_or_default(),
            currency.clone().unwrap_or_default(),
        ];
        rows.push(ParsedRow {
            cells,
            normalized: NormalizedRow {
                date,
                description,
                payee: name,
                amount,
                debit: None,
                credit: None,
                category: None,
                currency,
                reference,
                issue,
            },
        });
    }
    if rows.is_empty() {
        return Err(AppError::Validation("No OFX transactions were found.".into()));
    }
    Ok(ParsedFile {
        headers: vec![
            "date".into(),
            "amount".into(),
            "description".into(),
            "payee".into(),
            "reference".into(),
            "currency".into(),
        ],
        rows,
        warnings: vec![
            "OFX transactions are mapped automatically. Transfers and unsupported rows are skipped, never guessed."
                .into(),
        ],
    })
}

fn ofx_date(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 8 {
        return None;
    }
    let year: i32 = digits[0..4].parse().ok()?;
    let month: u32 = digits[4..6].parse().ok()?;
    let day: u32 = digits[6..8].parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day).map(iso)
}

fn parse_qif(bytes: &[u8]) -> AppResult<ParsedFile> {
    let text = decode_text(bytes);
    let mut rows = Vec::new();
    let mut current: Option<NormalizedRow> = None;
    let mut type_line = String::new();

    fn flush(current: &mut Option<NormalizedRow>, rows: &mut Vec<ParsedRow>) {
        if let Some(mut row) = current.take() {
            if row.date.is_empty() && row.amount.is_none() && row.description.is_empty() {
                return;
            }
            let transfer = row
                .category
                .as_deref()
                .map(|value| value.trim().starts_with('[') || value.eq_ignore_ascii_case("transfer"))
                .unwrap_or(false);
            if transfer {
                row.issue = Some("Transfers need two accounts and are skipped by the importer.".into());
                row.category = None;
            }
            let cells = vec![
                row.date.clone(),
                row.amount.clone().unwrap_or_default(),
                row.description.clone(),
                row.payee.clone().unwrap_or_default(),
                row.category.clone().unwrap_or_default(),
                row.reference.clone().unwrap_or_default(),
            ];
            rows.push(ParsedRow {
                cells,
                normalized: row,
            });
        }
    }

    for raw_line in text.lines() {
        let line = raw_line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            continue;
        }
        if line.starts_with('!') {
            type_line = line.to_string();
            continue;
        }
        let mut chars = line.chars();
        let code = chars.next().unwrap_or(' ');
        let value = chars.as_str().trim();
        if code == '^' {
            flush(&mut current, &mut rows);
            continue;
        }
        if type_line.to_uppercase().contains("CAT") {
            continue;
        }
        if current.is_none() {
            current = Some(NormalizedRow::default());
        }
        if let Some(row) = current.as_mut() {
            match code {
                'D' => row.date = value.to_string(),
                'T' => row.amount = Some(value.to_string()),
                'P' => {
                    row.payee = Some(value.to_string());
                    if row.description.is_empty() {
                        row.description = value.to_string();
                    }
                }
                'M' => {
                    if row.description.is_empty() {
                        row.description = value.to_string();
                    } else if !value.is_empty() {
                        row.description = format!("{} — {}", row.description, value);
                    }
                }
                'L' => row.category = Some(value.to_string()),
                'N' => row.reference = Some(value.to_string()),
                'U' => {
                    if row.amount.is_none() {
                        row.amount = Some(value.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    flush(&mut current, &mut rows);

    if rows.is_empty() {
        return Err(AppError::Validation("No QIF transactions were found.".into()));
    }
    for row in rows.iter_mut() {
        if row.normalized.issue.is_none() {
            if parse_flexible_date(&row.normalized.date, "auto").is_none() {
                row.normalized.issue = Some(format!("Unreadable QIF date '{}'.", row.normalized.date));
            } else if row.normalized.amount.is_none() {
                row.normalized.issue = Some("QIF row has no amount.".into());
            }
        }
    }

    Ok(ParsedFile {
        headers: vec![
            "date".into(),
            "amount".into(),
            "description".into(),
            "payee".into(),
            "category".into(),
            "reference".into(),
        ],
        rows,
        warnings: vec![
            "QIF records are mapped automatically. Split categories, transfers and investment actions are skipped."
                .into(),
        ],
    })
}

// ---------------------------------------------------------------------------
// Value parsing
// ---------------------------------------------------------------------------

fn parse_flexible_date(raw: &str, order: &str) -> Option<NaiveDate> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Some(date);
    }
    if let Ok(date) = NaiveDate::parse_from_str(raw, "%Y/%m/%d") {
        return Some(date);
    }
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 8 && raw.chars().all(|c| c.is_ascii_digit()) {
        let year: i32 = digits[0..4].parse().ok()?;
        let month: u32 = digits[4..6].parse().ok()?;
        let day: u32 = digits[6..8].parse().ok()?;
        if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
            return Some(date);
        }
    }

    let parts: Vec<&str> = raw
        .split(|c: char| c == '/' || c == '-' || c == '.' || c == '\'' || c == ' ')
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 3 {
        return None;
    }
    let first: u32 = parts[0].parse().ok()?;
    let second: u32 = parts[1].parse().ok()?;
    let year = normalize_year(parts[2])?;
    let resolved = match order {
        "ymd" => (year, first, second),
        "mdy" => (year, first, second),
        "dmy" => (year, second, first),
        _ => {
            if parts[0].len() == 4 {
                (year, second, first)
            } else if first > 12 {
                (year, second, first)
            } else if second > 12 {
                (year, first, second)
            } else {
                // Ambiguous numeric dates are read day-first, which matches the
                // statements this app is primarily used with. The preview makes
                // the interpretation visible before anything is committed.
                (year, second, first)
            }
        }
    };
    NaiveDate::from_ymd_opt(resolved.0, resolved.1, resolved.2)
}

fn normalize_year(raw: &str) -> Option<i32> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.len() {
        4 => digits.parse().ok(),
        2 => {
            let value: i32 = digits.parse().ok()?;
            Some(if value >= 70 { 1900 + value } else { 2000 + value })
        }
        _ => None,
    }
}

fn parse_money(raw: &str, separator: &str) -> AppResult<Option<Decimal>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    // Currency codes, symbols and signs are decoration around the number, never
    // part of it. Keeping only the span between the first and last digit means
    // "QAR 1,234.56" and "1,234.56-" still import, while "12abc34" is rejected
    // instead of silently becoming 1234.
    let (Some(first), Some(last)) = (
        trimmed.find(|c: char| c.is_ascii_digit()),
        trimmed.rfind(|c: char| c.is_ascii_digit()),
    ) else {
        return Ok(None);
    };
    let core = &trimmed[first..=last];
    if !core.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ',') {
        return Err(AppError::Validation(format!("Unreadable amount '{raw}'.")));
    }
    let decoration = format!("{}{}", &trimmed[..first], &trimmed[last + 1..]);
    let negative =
        decoration.contains('-') || (decoration.contains('(') && decoration.contains(')'));

    let mut cleaned = core.to_string();

    match separator {
        "dot" => cleaned.retain(|c| c != ','),
        "comma" => {
            cleaned.retain(|c| c != '.');
            cleaned = cleaned.replace(',', ".");
        }
        _ => {
            let dots = cleaned.matches('.').count();
            let commas = cleaned.matches(',').count();
            if dots > 0 && commas > 0 {
                let decimal_char = if cleaned.rfind('.') > cleaned.rfind(',') { '.' } else { ',' };
                let thousands_char = if decimal_char == '.' { ',' } else { '.' };
                cleaned.retain(|c| c != thousands_char);
                if decimal_char == ',' {
                    cleaned = cleaned.replace(',', ".");
                }
            } else if dots + commas > 1 {
                let decimal_char = if dots > 0 { '.' } else { ',' };
                let occurrences = cleaned.matches(decimal_char).count();
                let last_group_len = cleaned
                    .rsplit(decimal_char)
                    .next()
                    .map(|group| group.len())
                    .unwrap_or(0);
                if occurrences == 1 || last_group_len != 3 {
                    let thousands_char = if decimal_char == '.' { ',' } else { '.' };
                    cleaned.retain(|c| c != thousands_char);
                    if decimal_char == ',' {
                        cleaned = cleaned.replace(',', ".");
                    }
                } else {
                    cleaned.retain(|c| c != decimal_char);
                }
            } else if commas == 1 {
                cleaned = cleaned.replace(',', ".");
            }
        }
    }

    if cleaned.matches('.').count() > 1 {
        let last = cleaned.rfind('.').unwrap();
        cleaned = format!(
            "{}.{}",
            cleaned[..last].replace('.', ""),
            &cleaned[last + 1..]
        );
    }

    let value = Decimal::from_str(&cleaned)
        .map_err(|_| AppError::Validation(format!("Unreadable amount '{raw}'.")))?;
    Ok(Some(if negative { -value } else { value }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_test;
    use crate::services::accounts::{create_account, AccountInput};

    fn setup() -> (Connection, String, String, String) {
        let mut conn = open_test();
        let (tenant, user) = crate::db::seed_if_empty(&mut conn).unwrap();
        let account_id = create_account(
            &conn,
            &tenant,
            &user,
            &AccountInput {
                name: "Import test".into(),
                account_type: "bank".into(),
                institution: None,
                currency_code: "QAR".into(),
                is_liability: None,
                is_liquid: Some(true),
                opening_balance: None,
                opening_date: None,
                credit_limit: None,
                statement_day: None,
                payment_due_day: None,
                broker_id: None,
                notes: None,
                color: None,
                sort_order: None,
            },
        )
        .unwrap();
        (conn, tenant, user, account_id)
    }

    fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("rawjan-import-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{}-{name}", uuid::Uuid::new_v4()));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn parses_signed_amounts_and_duplicate_detection() {
        let (conn, tenant, user, account_id) = setup();
        let path = write_temp(
            "basic.csv",
            "Date,Description,Amount\n2026-01-05,Coffee,-12.50\n2026-01-06,Salary,5000.00\n",
        );
        let first = preview(
            &conn,
            &tenant,
            &user,
            path.to_str().unwrap(),
            "csv",
            Some(&account_id),
        )
        .unwrap();
        assert_eq!(first.row_count, 2);
        assert_eq!(first.rows[0].status, "ok");

        let mapping = first.suggested_mapping.clone();
        let result = commit(
            &conn,
            &tenant,
            &user,
            &first.job_id,
            &mapping,
            &first.headers,
            None,
        )
        .unwrap();
        assert_eq!(result.imported, 2);
        assert_eq!(result.errors, 0);

        let path2 = write_temp("again.csv", "Date,Description,Amount\n2026-01-05,Coffee,-12.50\n");
        let preview2 = preview(
            &conn,
            &tenant,
            &user,
            path2.to_str().unwrap(),
            "csv",
            Some(&account_id),
        )
        .unwrap();
        let result2 = commit(
            &conn,
            &tenant,
            &user,
            &preview2.job_id,
            &mapping,
            &preview2.headers,
            None,
        )
        .unwrap();
        assert_eq!(result2.duplicates, 1);
        assert_eq!(result2.imported, 0);
    }

    #[test]
    fn parses_debit_credit_columns() {
        let (conn, tenant, user, account_id) = setup();
        let path = write_temp(
            "debit-credit.csv",
            "Date,Details,Debit,Credit\n05/01/2026,Groceries,120.00,\n06/01/2026,Refund,,45.00\n",
        );
        let preview = preview(
            &conn,
            &tenant,
            &user,
            path.to_str().unwrap(),
            "csv",
            Some(&account_id),
        )
        .unwrap();
        assert_eq!(preview.suggested_mapping.debit.as_deref(), Some("Debit"));
        let result = commit(
            &conn,
            &tenant,
            &user,
            &preview.job_id,
            &preview.suggested_mapping,
            &preview.headers,
            None,
        )
        .unwrap();
        assert_eq!(result.imported, 2);
        let kinds: Vec<String> = conn
            .prepare("SELECT kind FROM transactions ORDER BY local_date")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(kinds, vec!["expense", "income"]);
    }

    #[test]
    fn parses_ofx_blocks() {
        let (conn, tenant, user, account_id) = setup();
        let path = write_temp(
            "statement.ofx",
            "<OFX><BANKMSGSRSV1><STMTTRNRS><STMTRS><CURDEF>QAR<BANKTRANLIST>
             <STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20260107120000<TRNAMT>-33.25<FITID>A1<NAME>Cafe</STMTTRN>
             <STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20260108120000<TRNAMT>900.00<FITID>A2<NAME>Client</STMTTRN>
             </BANKTRANLIST></STMTRS></STMTTRNRS></BANKMSGSRSV1></OFX>",
        );
        let preview = preview(
            &conn,
            &tenant,
            &user,
            path.to_str().unwrap(),
            "ofx",
            Some(&account_id),
        )
        .unwrap();
        assert_eq!(preview.row_count, 2);
        let result = commit(
            &conn,
            &tenant,
            &user,
            &preview.job_id,
            &preview.suggested_mapping,
            &preview.headers,
            None,
        )
        .unwrap();
        assert_eq!(result.imported, 2);
    }

    #[test]
    fn parses_qif_records_and_skips_transfers() {
        let (conn, tenant, user, account_id) = setup();
        let path = write_temp(
            "statement.qif",
            "!Type:Bank\nD1/9/2026\nT-25.00\nPCorner shop\nMBread\n^ \nD1/10/2026\nT-500.00\nPTransfer\nL[Savings]\n^\n",
        );
        let preview = preview(
            &conn,
            &tenant,
            &user,
            path.to_str().unwrap(),
            "qif",
            Some(&account_id),
        )
        .unwrap();
        assert_eq!(preview.row_count, 2);
        assert!(preview
            .rows
            .iter()
            .any(|row| row.issue.as_deref().unwrap_or("").contains("Transfers")));
    }

    #[test]
    fn parses_ambiguous_and_grouped_amounts() {
        assert_eq!(
            parse_money("1,234.56", "auto").unwrap(),
            Some(Decimal::from_str("1234.56").unwrap())
        );
        assert_eq!(
            parse_money("1.234,56", "auto").unwrap(),
            Some(Decimal::from_str("1234.56").unwrap())
        );
        assert_eq!(
            parse_money("(12.50)", "auto").unwrap(),
            Some(Decimal::from_str("-12.50").unwrap())
        );
        assert_eq!(
            parse_money("12.50-", "auto").unwrap(),
            Some(Decimal::from_str("-12.50").unwrap())
        );
        assert_eq!(parse_money("abc", "auto").unwrap(), None);
    }

    #[test]
    fn keeps_currency_decoration_but_rejects_embedded_garbage() {
        assert_eq!(
            parse_money("QAR 1,234.56", "auto").unwrap(),
            Some(Decimal::from_str("1234.56").unwrap())
        );
        assert_eq!(
            parse_money("$1,234.56-", "auto").unwrap(),
            Some(Decimal::from_str("-1234.56").unwrap())
        );
        assert_eq!(
            parse_money("1.234,56 QR", "auto").unwrap(),
            Some(Decimal::from_str("1234.56").unwrap())
        );
        assert!(parse_money("12abc34", "auto").is_err());
        assert!(parse_money("1x2.50", "auto").is_err());
        assert!(parse_money("1,234.56-78", "auto").is_err());
        assert_eq!(parse_money("-", "auto").unwrap(), None);
    }

    #[test]
    fn rejects_unsupported_pdf_with_an_honest_message() {
        let (conn, tenant, user, account_id) = setup();
        let path = write_temp("statement.pdf", "%PDF-1.4");
        let error = preview(
            &conn,
            &tenant,
            &user,
            path.to_str().unwrap(),
            "pdf",
            Some(&account_id),
        )
        .unwrap_err();
        assert!(error.to_string().contains("not parsed"));
    }

    #[test]
    fn liability_credits_are_balance_changes_not_income() {
        let (conn, tenant, user, _) = setup();
        let liability_id = create_account(
            &conn,
            &tenant,
            &user,
            &AccountInput {
                name: "Card".into(),
                account_type: "credit_card".into(),
                institution: None,
                currency_code: "QAR".into(),
                is_liability: Some(true),
                is_liquid: Some(false),
                opening_balance: None,
                opening_date: None,
                credit_limit: Some("10000".into()),
                statement_day: None,
                payment_due_day: None,
                broker_id: None,
                notes: None,
                color: None,
                sort_order: None,
            },
        )
        .unwrap();
        let path = write_temp("card.csv", "Date,Description,Amount\n2026-02-01,Payment,300.00\n");
        let preview = preview(
            &conn,
            &tenant,
            &user,
            path.to_str().unwrap(),
            "csv",
            Some(&liability_id),
        )
        .unwrap();
        let result = commit(
            &conn,
            &tenant,
            &user,
            &preview.job_id,
            &preview.suggested_mapping,
            &preview.headers,
            None,
        )
        .unwrap();
        assert_eq!(result.imported, 1);
        let kind: String = conn
            .query_row("SELECT kind FROM transactions LIMIT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(kind, "adjustment");
    }

    #[test]
    fn a_committed_job_cannot_be_committed_twice() {
        let (conn, tenant, user, account_id) = setup();
        let path = write_temp("once.csv", "Date,Description,Amount\n2026-03-01,Thing,-1.00\n");
        let preview = preview(
            &conn,
            &tenant,
            &user,
            path.to_str().unwrap(),
            "csv",
            Some(&account_id),
        )
        .unwrap();
        commit(
            &conn,
            &tenant,
            &user,
            &preview.job_id,
            &preview.suggested_mapping,
            &preview.headers,
            None,
        )
        .unwrap();
        let error = commit(
            &conn,
            &tenant,
            &user,
            &preview.job_id,
            &preview.suggested_mapping,
            &preview.headers,
            None,
        )
        .unwrap_err();
        assert!(error.to_string().contains("already been committed"));
    }

    #[test]
    fn import_rows_become_real_ledger_entries() {
        let (conn, tenant, user, account_id) = setup();
        let path = write_temp("ledger.csv", "Date,Description,Amount\n2026-04-01,Thing,-10.00\n");
        let preview = preview(
            &conn,
            &tenant,
            &user,
            path.to_str().unwrap(),
            "csv",
            Some(&account_id),
        )
        .unwrap();
        commit(
            &conn,
            &tenant,
            &user,
            &preview.job_id,
            &preview.suggested_mapping,
            &preview.headers,
            None,
        )
        .unwrap();
        let entries: i64 = conn
            .query_row("SELECT count(*) FROM ledger_entries", [], |row| row.get(0))
            .unwrap();
        assert_eq!(entries, 2);
    }
}
