use std::path::{Path, PathBuf};
use std::time::Duration;

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

use crate::dates::now_iso;
use crate::db;
use crate::error::{AppError, AppResult};

const MAGIC: &[u8; 8] = b"RAWJAN01";
/// Exports created before the rename still use this header. Both magics are eight bytes, so the
/// salt, nonce and ciphertext offsets are identical.
const LEGACY_MAGIC: &[u8; 8] = b"AUREQIS1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

const EXPORT_TABLES: &[&str] = &[
    "currencies",
    "accounts",
    "categories",
    "tags",
    "income_sources",
    "transactions",
    "ledger_entries",
    "transaction_splits",
    "transaction_tags",
    "budgets",
    "budget_allocations",
    "budget_periods",
    "goals",
    "goal_contributions",
    "goal_milestones",
    "instruments",
    "investment_trades",
    "investment_valuations",
    "dividends",
    "liabilities",
    "liability_payments",
    "net_worth_snapshots",
    "journal_entries",
    "journal_links",
    "exchange_rates",
    "tenant_settings",
    "recurring_rules",
    "recurring_exceptions",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDto {
    pub id: String,
    pub path: String,
    pub file_name: String,
    pub kind: String,
    pub byte_size: i64,
    pub sha256: String,
    pub encrypted: bool,
    pub schema_version: i64,
    pub verified_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableCount {
    pub table: String,
    pub rows: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupValidation {
    pub path: String,
    pub file_name: String,
    pub readable: bool,
    pub encrypted: bool,
    pub schema_version: Option<i64>,
    pub integrity: String,
    pub tables: Vec<TableCount>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub id: String,
    pub path: String,
    pub file_name: String,
    pub byte_size: i64,
    pub sha256: String,
    pub table_counts: Vec<TableCount>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub path: String,
    pub exported_at: Option<String>,
    pub schema_version: Option<i64>,
    pub tables: Vec<TableCount>,
    pub valid: bool,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Backups
// ---------------------------------------------------------------------------

fn backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

fn exports_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("exports")
}

fn timestamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

fn sha256_file(path: &Path) -> AppResult<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}

fn open_keyed(path: &Path, key: Option<&str>) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    if let Some(key) = key {
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AppError::Internal("invalid database key format".into()));
        }
        conn.execute_batch(&format!("PRAGMA key = \"x'{key}'\";"))?;
    }
    conn.execute_batch("PRAGMA busy_timeout = 5000;")?;
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })
    .map_err(|error| {
        AppError::Validation(format!(
            "The database file could not be opened with the current key. ({error})"
        ))
    })?;
    Ok(conn)
}

fn table_counts(conn: &Connection, tenant_id: &str) -> AppResult<Vec<TableCount>> {
    let mut counts = Vec::new();
    for table in EXPORT_TABLES {
        let query = if *table == "currencies" {
            format!("SELECT count(*) FROM {table}")
        } else {
            format!("SELECT count(*) FROM {table} WHERE tenant_id = ?1")
        };
        let result = if *table == "currencies" {
            conn.query_row(&query, [], |row| row.get::<_, i64>(0))
        } else {
            conn.query_row(&query, params![tenant_id], |row| row.get::<_, i64>(0))
        };
        if let Ok(rows) = result {
            counts.push(TableCount {
                table: (*table).to_string(),
                rows,
            });
        }
    }
    Ok(counts)
}

pub fn create_backup(
    conn: &Connection,
    data_dir: &Path,
    tenant_id: &str,
    user_id: &str,
    kind: &str,
) -> AppResult<BackupDto> {
    let kind = match kind {
        "manual" | "daily" | "pre_migration" => kind,
        _ => "manual",
    };
    let dir = backups_dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let file_name = format!(
        "rawjan-{}-{}.db",
        timestamp(),
        &uuid::Uuid::new_v4().to_string()[..8]
    );
    let path = dir.join(&file_name);

    let (key, encrypted) = db::resolve_database_key()?;
    {
        let mut destination = open_keyed(&path, key.as_deref())?;
        {
            let backup = rusqlite::backup::Backup::new(conn, &mut destination)?;
            backup.run_to_completion(64, Duration::from_millis(10), None)?;
        }
        let _ = destination.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }

    // Verify the copy before it is recorded as a usable backup.
    let verified_at = {
        let check = open_keyed(&path, key.as_deref())?;
        let integrity: String = check.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            let _ = std::fs::remove_file(&path);
            return Err(AppError::Internal(format!(
                "The backup failed its integrity check: {integrity}"
            )));
        }
        Some(now_iso())
    };

    let byte_size = std::fs::metadata(&path)?.len() as i64;
    let sha256 = sha256_file(&path)?;
    let schema_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = now_iso();
    conn.execute(
        "INSERT INTO backups
         (id, tenant_id, path, kind, byte_size, sha256, encrypted, schema_version, verified_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            tenant_id,
            path.display().to_string(),
            kind,
            byte_size,
            sha256,
            encrypted as i64,
            schema_version,
            verified_at,
            created_at
        ],
    )?;
    db::audit(
        conn,
        tenant_id,
        user_id,
        "backup",
        &id,
        "create",
        None,
        Some(
            &json!({ "path": path.display().to_string(), "sha256": sha256, "byteSize": byte_size }),
        ),
    )?;

    Ok(BackupDto {
        id,
        path: path.display().to_string(),
        file_name,
        kind: kind.to_string(),
        byte_size,
        sha256,
        encrypted,
        schema_version,
        verified_at,
        created_at,
    })
}

pub fn list_backups(conn: &Connection, tenant_id: &str) -> AppResult<Vec<BackupDto>> {
    let mut stmt = conn.prepare(
        "SELECT id, path, kind, byte_size, sha256, encrypted, schema_version, verified_at, created_at
         FROM backups WHERE tenant_id = ?1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map(params![tenant_id], |row| {
        let path: String = row.get(1)?;
        let file_name = Path::new(&path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(&path)
            .to_string();
        Ok(BackupDto {
            id: row.get(0)?,
            path,
            file_name,
            kind: row.get(2)?,
            byte_size: row.get(3)?,
            sha256: row.get(4)?,
            encrypted: row.get::<_, i64>(5)? == 1,
            schema_version: row.get(6)?,
            verified_at: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn delete_backup(
    conn: &Connection,
    data_dir: &Path,
    tenant_id: &str,
    user_id: &str,
    id: &str,
) -> AppResult<()> {
    let path: String = conn
        .query_row(
            "SELECT path FROM backups WHERE id = ?1 AND tenant_id = ?2",
            params![id, tenant_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("Backup not found.".into()))?;

    // Only files inside the managed backups directory may be deleted.
    let managed = backups_dir(data_dir);
    let candidate = PathBuf::from(&path);
    let allowed = candidate
        .canonicalize()
        .ok()
        .zip(managed.canonicalize().ok())
        .map(|(file, dir)| file.starts_with(dir))
        .unwrap_or(false);
    if !allowed {
        return Err(AppError::Validation(
            "That backup is outside the managed backup folder, so Rawjan will not delete it. Remove it yourself if you are sure."
                .into(),
        ));
    }
    if candidate.exists() {
        std::fs::remove_file(&candidate)?;
    }
    conn.execute(
        "DELETE FROM backups WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
    )?;
    db::audit(
        conn,
        tenant_id,
        user_id,
        "backup",
        id,
        "delete",
        Some(&json!({ "path": path })),
        None,
    )?;
    Ok(())
}

pub fn validate_backup(path: &str) -> AppResult<BackupValidation> {
    let path_ref = Path::new(path);
    if !path_ref.is_file() {
        return Err(AppError::Validation(
            "That backup file does not exist.".into(),
        ));
    }
    let (key, encrypted) = db::resolve_database_key()?;
    let file_name = path_ref
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path)
        .to_string();
    let conn = open_keyed(path_ref, key.as_deref())?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    let schema_version: Option<i64> = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .optional()?;
    let tenant_id: Option<String> = conn
        .query_row("SELECT id FROM tenants LIMIT 1", [], |row| row.get(0))
        .optional()?;
    let tables = match tenant_id.as_deref() {
        Some(tenant) => table_counts(&conn, tenant)?,
        None => Vec::new(),
    };
    let readable = integrity == "ok" && schema_version.is_some();
    Ok(BackupValidation {
        path: path.to_string(),
        file_name,
        readable,
        encrypted,
        schema_version,
        integrity,
        tables,
        message: if readable {
            "The backup opens with the current key and passes its integrity check. Close Rawjan before replacing the live database with this file."
                .into()
        } else {
            "The backup could not be validated. Do not use it for a restore.".into()
        },
    })
}

// ---------------------------------------------------------------------------
// Encrypted exports
// ---------------------------------------------------------------------------

fn value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(number) => json!(number),
        ValueRef::Real(number) => json!(number),
        ValueRef::Text(text) => Value::String(String::from_utf8_lossy(text).to_string()),
        ValueRef::Blob(bytes) => Value::String(hex::encode(bytes)),
    }
}

fn table_json(conn: &Connection, table: &str, tenant_id: &str) -> AppResult<Vec<Value>> {
    let filtered = table != "currencies";
    let query = if filtered {
        format!("SELECT * FROM {table} WHERE tenant_id = ?1")
    } else {
        format!("SELECT * FROM {table}")
    };
    let mut stmt = conn.prepare(&query)?;
    let names: Vec<String> = stmt.column_names().map(str::to_string).collect();

    let mut rows = Vec::new();
    let collect = |row: &rusqlite::Row<'_>| -> AppResult<Value> {
        let mut object = serde_json::Map::new();
        for (index, name) in names.iter().enumerate() {
            object.insert(name.clone(), value_to_json(row.get_ref(index)?));
        }
        Ok(Value::Object(object))
    };

    if filtered {
        let mut result = stmt.query(params![tenant_id])?;
        while let Some(row) = result.next()? {
            rows.push(collect(row)?);
        }
    } else {
        let mut result = stmt.query([])?;
        while let Some(row) = result.next()? {
            rows.push(collect(row)?);
        }
    }
    Ok(rows)
}

fn build_export(conn: &Connection, tenant_id: &str) -> AppResult<(Value, Vec<TableCount>)> {
    let mut tables = serde_json::Map::new();
    let mut counts = Vec::new();
    for table in EXPORT_TABLES {
        let rows = table_json(conn, table, tenant_id)?;
        counts.push(TableCount {
            table: (*table).to_string(),
            rows: rows.len() as i64,
        });
        tables.insert((*table).to_string(), Value::Array(rows));
    }
    let schema_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let payload = json!({
        "app": "Rawjan",
        "format": "rawjan-export",
        "version": 1,
        "exportedAt": now_iso(),
        "schemaVersion": schema_version,
        "tenantId": tenant_id,
        "tables": tables,
    });
    Ok((payload, counts))
}

fn derive_key(passphrase: &str, salt: &[u8]) -> AppResult<[u8; KEY_LEN]> {
    if passphrase.trim().len() < 8 {
        return Err(AppError::Validation(
            "Use a passphrase of at least 8 characters. It is never stored.".into(),
        ));
    }
    let params = Params::new(19_456, 2, 1, Some(KEY_LEN)).map_err(|error| {
        AppError::Internal(format!("could not configure key derivation: {error}"))
    })?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|error| AppError::Internal(format!("key derivation failed: {error}")))?;
    Ok(key)
}

fn random_bytes<const N: usize>() -> AppResult<[u8; N]> {
    let mut buffer = [0u8; N];
    getrandom::fill(&mut buffer)
        .map_err(|error| AppError::Internal(format!("secure random generation failed: {error}")))?;
    Ok(buffer)
}

fn encrypt_bytes(passphrase: &str, plaintext: &[u8]) -> AppResult<Vec<u8>> {
    let salt = random_bytes::<SALT_LEN>()?;
    let nonce = random_bytes::<NONCE_LEN>()?;
    let mut key = derive_key(passphrase, &salt)?;
    let cipher = ChaCha20Poly1305::new(&Key::from(key));
    let ciphertext = cipher
        .encrypt(&Nonce::from(nonce), plaintext)
        .map_err(|_| AppError::Internal("encryption failed".into()))?;
    key.zeroize();

    let mut out = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

fn decrypt_bytes(passphrase: &str, bytes: &[u8]) -> AppResult<Vec<u8>> {
    if bytes.len() < MAGIC.len() + SALT_LEN + NONCE_LEN + 16 {
        return Err(AppError::Validation(
            "That file is too short to be a Rawjan export.".into(),
        ));
    }
    if !bytes.starts_with(MAGIC) && !bytes.starts_with(LEGACY_MAGIC) {
        return Err(AppError::Validation(
            "That file is not a Rawjan encrypted export.".into(),
        ));
    }
    let salt_start = MAGIC.len();
    let nonce_start = salt_start + SALT_LEN;
    let cipher_start = nonce_start + NONCE_LEN;
    let salt = &bytes[salt_start..nonce_start];
    let nonce = &bytes[nonce_start..cipher_start];
    let mut key = derive_key(passphrase, salt)?;
    let nonce_array: [u8; NONCE_LEN] = nonce
        .try_into()
        .map_err(|_| AppError::Validation("That file has a malformed header.".into()))?;
    let cipher = ChaCha20Poly1305::new(&Key::from(key));
    let plaintext = cipher
        .decrypt(&Nonce::from(nonce_array), &bytes[cipher_start..])
        .map_err(|_| {
            AppError::Validation(
                "The export could not be decrypted. The passphrase is wrong or the file is damaged."
                    .into(),
            )
        })?;
    key.zeroize();
    Ok(plaintext)
}

pub fn create_export(
    conn: &Connection,
    data_dir: &Path,
    tenant_id: &str,
    user_id: &str,
    passphrase: &str,
) -> AppResult<ExportResult> {
    let (payload, counts) = build_export(conn, tenant_id)?;
    let plaintext = serde_json::to_vec(&payload)?;
    let encrypted = encrypt_bytes(passphrase, &plaintext)?;

    let dir = exports_dir(data_dir);
    std::fs::create_dir_all(&dir)?;
    let file_name = format!(
        "rawjan-export-{}-{}.rawjan",
        timestamp(),
        &uuid::Uuid::new_v4().to_string()[..8]
    );
    let path = dir.join(&file_name);
    std::fs::write(&path, &encrypted)?;

    // Decrypt in memory once so a broken export is never reported as valid.
    let check = decrypt_bytes(passphrase, &std::fs::read(&path)?)?;
    if check != plaintext {
        let _ = std::fs::remove_file(&path);
        return Err(AppError::Internal(
            "The export failed its round-trip check and was removed.".into(),
        ));
    }

    let byte_size = encrypted.len() as i64;
    let sha256 = hex::encode(Sha256::digest(&encrypted));
    let schema_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = now_iso();
    conn.execute(
        "INSERT INTO backups
         (id, tenant_id, path, kind, byte_size, sha256, encrypted, schema_version, verified_at, created_at)
         VALUES (?1, ?2, ?3, 'export', ?4, ?5, 1, ?6, ?7, ?8)",
        params![
            id,
            tenant_id,
            path.display().to_string(),
            byte_size,
            sha256,
            schema_version,
            created_at,
            created_at
        ],
    )?;
    db::audit(
        conn,
        tenant_id,
        user_id,
        "export",
        &id,
        "create",
        None,
        Some(
            &json!({ "path": path.display().to_string(), "sha256": sha256, "byteSize": byte_size }),
        ),
    )?;

    Ok(ExportResult {
        id,
        path: path.display().to_string(),
        file_name,
        byte_size,
        sha256,
        table_counts: counts,
        created_at,
    })
}

pub fn verify_export(path: &str, passphrase: &str) -> AppResult<ExportSummary> {
    let path_ref = Path::new(path);
    if !path_ref.is_file() {
        return Err(AppError::Validation(
            "That export file does not exist.".into(),
        ));
    }
    let bytes = std::fs::read(path_ref)?;
    let plaintext = decrypt_bytes(passphrase, &bytes)?;
    let payload: Value = serde_json::from_slice(&plaintext)
        .map_err(|_| AppError::Validation("The decrypted file is not a Rawjan export.".into()))?;
    let tables = payload
        .get("tables")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .map(|(table, rows)| TableCount {
                    table: table.clone(),
                    rows: rows
                        .as_array()
                        .map(|values| values.len() as i64)
                        .unwrap_or(0),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(ExportSummary {
        path: path.to_string(),
        exported_at: payload
            .get("exportedAt")
            .and_then(Value::as_str)
            .map(str::to_string),
        schema_version: payload.get("schemaVersion").and_then(Value::as_i64),
        tables,
        valid: true,
        message:
            "The passphrase decrypted this export and the payload is readable. Verification does not modify your live database."
                .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_test;

    #[test]
    fn encrypted_exports_round_trip_and_reject_wrong_passphrases() {
        let encrypted = encrypt_bytes("correct horse battery", b"ledger payload").unwrap();
        assert_eq!(&encrypted[..MAGIC.len()], MAGIC);
        let plaintext = decrypt_bytes("correct horse battery", &encrypted).unwrap();
        assert_eq!(plaintext, b"ledger payload");
        let error = decrypt_bytes("wrong passphrase", &encrypted).unwrap_err();
        assert!(error.to_string().contains("could not be decrypted"));
    }

    #[test]
    fn legacy_aureqis_exports_still_decrypt() {
        let encrypted = encrypt_bytes("correct horse battery", b"ledger payload").unwrap();
        let mut legacy = encrypted.clone();
        legacy[..LEGACY_MAGIC.len()].copy_from_slice(LEGACY_MAGIC);
        let plaintext = decrypt_bytes("correct horse battery", &legacy).unwrap();
        assert_eq!(plaintext, b"ledger payload");
    }

    #[test]
    fn short_passphrases_are_rejected() {
        let error = encrypt_bytes("short", b"data").unwrap_err();
        assert!(error.to_string().contains("at least 8"));
    }

    #[test]
    fn export_payload_contains_tenant_rows() {
        let mut conn = open_test();
        let (tenant, _user) = crate::db::seed_if_empty(&mut conn).unwrap();
        let (payload, counts) = build_export(&conn, &tenant).unwrap();
        assert_eq!(payload.get("app").and_then(Value::as_str), Some("Rawjan"));
        let categories = payload
            .pointer("/tables/categories")
            .and_then(Value::as_array)
            .unwrap();
        assert!(!categories.is_empty());
        assert!(counts
            .iter()
            .any(|count| count.table == "categories" && count.rows > 0));
    }

    #[test]
    fn a_tampered_export_fails_authentication() {
        let mut encrypted = encrypt_bytes("correct horse battery", b"ledger payload").unwrap();
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0x01;
        assert!(decrypt_bytes("correct horse battery", &encrypted).is_err());
    }
}
