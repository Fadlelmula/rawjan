use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::dates::now_iso;
use crate::error::{AppError, AppResult};

const SCHEMA_VERSION: i64 = 1;
const MIGRATION_0001: &str = include_str!("../migrations/0001_init.sql");

pub fn open(path: &Path, key_hex: Option<&str>) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    if let Some(key) = key_hex {
        if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(AppError::Internal("invalid database key format".into()));
        }
        conn.execute_batch(&format!(
            "PRAGMA key = \"x'{key}'\";\nPRAGMA cipher_memory_security = ON;"
        ))?;
    }
    conn.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA temp_store = MEMORY;",
    )?;
    // Verify the key and database readability before continuing.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get::<_, i64>(0))
        .map_err(|e| {
            AppError::Internal(format!(
                "The database could not be opened. It may be encrypted with a different key. ({e})"
            ))
        })?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_test() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory database");
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    apply_migrations(&mut conn).unwrap();
    conn
}

pub fn apply_migrations(conn: &mut Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if current > SCHEMA_VERSION {
        return Err(AppError::Internal(format!(
            "This database was created by a newer version of Rawjan (schema {current}, supported {SCHEMA_VERSION}). Update the application before opening it."
        )));
    }
    if current < 1 {
        let tx = conn.transaction()?;
        tx.execute_batch(MIGRATION_0001)?;
        tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        tx.commit()?;
    }
    Ok(())
}

pub fn resolve_database_key() -> AppResult<(Option<String>, bool)> {
    if let Ok(key) = std::env::var("RAWJAN_DB_KEY") {
        let trimmed = key.trim().to_string();
        if trimmed.len() == 64 && trimmed.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Ok((Some(trimmed), true));
        }
        return Err(AppError::Validation(
            "RAWJAN_DB_KEY must be a 64-character hexadecimal key".into(),
        ));
    }
    let entry = keyring::Entry::new("rawjan-finance", "database-key")?;
    match entry.get_password() {
        Ok(existing) if existing.len() == 64 => Ok((Some(existing), true)),
        Ok(_) | Err(keyring::Error::NoEntry) => {
            let mut buf = [0u8; 32];
            getrandom::fill(&mut buf)
                .map_err(|e| AppError::Internal(format!("secure random generation failed: {e}")))?;
            let generated = hex::encode(buf);
            entry.set_password(&generated)?;
            Ok((Some(generated), true))
        }
        Err(e) => Err(AppError::Keyring(e)),
    }
}

/// Copies a vault created before the rename into the new data directory.
///
/// The old folder and key are left untouched so the previous build can still open its own copy.
/// Migration only happens when the new vault does not exist yet and the old key is still present.
pub fn migrate_legacy_vault(new_dir: &Path) -> AppResult<()> {
    let new_db = new_dir.join("rawjan.db");
    if new_db.exists() {
        return Ok(());
    }
    let Some(parent) = new_dir.parent() else {
        return Ok(());
    };
    let legacy_dir = parent.join("com.aureqis.finance");
    let legacy_db = legacy_dir.join("aureqis.db");
    if !legacy_db.is_file() {
        return Ok(());
    }

    let legacy_key = match keyring::Entry::new("aureqis-finance", "database-key")?.get_password() {
        Ok(key) if key.len() == 64 => key,
        Ok(_) => {
            log::warn!(
                "A previous vault exists at {}, but its stored key has an unexpected format. Starting a new vault.",
                legacy_dir.display()
            );
            return Ok(());
        }
        Err(keyring::Error::NoEntry) => {
            log::warn!(
                "A previous vault exists at {}, but its key is missing from Windows Credential Manager. Starting a new vault and leaving the old files untouched.",
                legacy_dir.display()
            );
            return Ok(());
        }
        Err(error) => return Err(AppError::Keyring(error)),
    };

    let entry = keyring::Entry::new("rawjan-finance", "database-key")?;
    match entry.get_password() {
        Ok(existing) if existing == legacy_key => {}
        Ok(_) => {
            log::warn!(
                "A Rawjan key already exists and differs from the previous vault key. The old vault was not migrated."
            );
            return Ok(());
        }
        Err(keyring::Error::NoEntry) => entry.set_password(&legacy_key)?,
        Err(error) => return Err(AppError::Keyring(error)),
    }

    std::fs::copy(&legacy_db, &new_db)?;
    let legacy_wal = legacy_dir.join("aureqis.db-wal");
    if legacy_wal.is_file() {
        if let Err(error) = std::fs::copy(&legacy_wal, new_dir.join("rawjan.db-wal")) {
            let _ = std::fs::remove_file(&new_db);
            return Err(error.into());
        }
    }
    log::info!(
        "Migrated the previous vault from {} to {}",
        legacy_dir.display(),
        new_dir.display()
    );
    Ok(())
}

pub fn seed_if_empty(conn: &mut Connection) -> AppResult<(String, String)> {
    let tenant_id: Option<String> = conn
        .query_row("SELECT id FROM tenants LIMIT 1", [], |r| r.get(0))
        .optional()?;

    let (tenant_id, user_id) = if let Some(t) = tenant_id {
        let u: String = conn.query_row(
            "SELECT id FROM users WHERE tenant_id = ?1 ORDER BY created_at LIMIT 1",
            [&t],
            |r| r.get(0),
        )?;
        (t, u)
    } else {
        let now = now_iso();
        let tenant_id = uuid::Uuid::new_v4().to_string();
        let user_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO tenants (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![tenant_id, "Local profile", now],
        )?;
        conn.execute(
            "INSERT INTO users (id, tenant_id, display_name, is_local_owner, created_at)
             VALUES (?1, ?2, ?3, 1, ?4)",
            params![user_id, tenant_id, "Owner", now],
        )?;
        seed_settings(conn, &tenant_id)?;
        seed_categories(conn, &tenant_id, &user_id)?;
        (tenant_id, user_id)
    };

    seed_currencies(conn)?;
    ensure_default_settings(conn, &tenant_id)?;
    Ok((tenant_id, user_id))
}

fn seed_currencies(conn: &Connection) -> AppResult<()> {
    const CURRENCIES: &[(&str, &str, &str, i64)] = &[
        ("QAR", "Qatari Riyal", "ر.ق", 2),
        ("USD", "US Dollar", "$", 2),
        ("EUR", "Euro", "€", 2),
        ("GBP", "British Pound", "£", 2),
        ("SAR", "Saudi Riyal", "ر.س", 2),
        ("AED", "UAE Dirham", "د.إ", 2),
        ("KWD", "Kuwaiti Dinar", "د.ك", 3),
        ("BHD", "Bahraini Dinar", "د.ب", 3),
        ("OMR", "Omani Rial", "ر.ع", 3),
        ("JOD", "Jordanian Dinar", "د.ا", 3),
        ("EGP", "Egyptian Pound", "E£", 2),
        ("INR", "Indian Rupee", "₹", 2),
        ("PKR", "Pakistani Rupee", "₨", 2),
        ("TRY", "Turkish Lira", "₺", 2),
        ("CNY", "Chinese Yuan", "¥", 2),
        ("JPY", "Japanese Yen", "¥", 0),
        ("KRW", "South Korean Won", "₩", 0),
        ("CHF", "Swiss Franc", "Fr", 2),
        ("CAD", "Canadian Dollar", "C$", 2),
        ("AUD", "Australian Dollar", "A$", 2),
        ("NZD", "New Zealand Dollar", "NZ$", 2),
        ("SGD", "Singapore Dollar", "S$", 2),
        ("MYR", "Malaysian Ringgit", "RM", 2),
        ("HKD", "Hong Kong Dollar", "HK$", 2),
        ("SEK", "Swedish Krona", "kr", 2),
        ("NOK", "Norwegian Krone", "kr", 2),
        ("DKK", "Danish Krone", "kr", 2),
        ("ZAR", "South African Rand", "R", 2),
        ("BRL", "Brazilian Real", "R$", 2),
        ("MXN", "Mexican Peso", "MX$", 2),
        ("RUB", "Russian Ruble", "₽", 2),
        ("THB", "Thai Baht", "฿", 2),
        ("IDR", "Indonesian Rupiah", "Rp", 2),
        ("PHP", "Philippine Peso", "₱", 2),
        ("VND", "Vietnamese Dong", "₫", 0),
        ("BDT", "Bangladeshi Taka", "৳", 2),
        ("LKR", "Sri Lankan Rupee", "Rs", 2),
        ("NPR", "Nepalese Rupee", "Rs", 2),
        ("ILS", "Israeli New Shekel", "₪", 2),
        ("LBP", "Lebanese Pound", "ل.ل", 2),
        ("MAD", "Moroccan Dirham", "د.م.", 2),
        ("TND", "Tunisian Dinar", "د.ت", 3),
        ("KES", "Kenyan Shilling", "KSh", 2),
        ("NGN", "Nigerian Naira", "₦", 2),
        ("GHS", "Ghanaian Cedi", "₵", 2),
        ("XOF", "West African CFA Franc", "CFA", 0),
        ("XAF", "Central African CFA Franc", "FCFA", 0),
        ("CLP", "Chilean Peso", "CL$", 0),
        ("COP", "Colombian Peso", "CO$", 2),
        ("PEN", "Peruvian Sol", "S/", 2),
        ("ARS", "Argentine Peso", "AR$", 2),
        ("ISK", "Icelandic Króna", "kr", 0),
        ("CZK", "Czech Koruna", "Kč", 2),
        ("PLN", "Polish Złoty", "zł", 2),
        ("HUF", "Hungarian Forint", "Ft", 2),
        ("RON", "Romanian Leu", "lei", 2),
        ("UAH", "Ukrainian Hryvnia", "₴", 2),
        ("BGN", "Bulgarian Lev", "лв", 2),
        ("RSD", "Serbian Dinar", "дин", 2),
        ("GEL", "Georgian Lari", "₾", 2),
        ("AMD", "Armenian Dram", "֏", 2),
        ("AZN", "Azerbaijani Manat", "₼", 2),
        ("KZT", "Kazakhstani Tenge", "₸", 2),
        ("UZS", "Uzbekistani Som", "so'm", 2),
        ("TWD", "New Taiwan Dollar", "NT$", 2),
        ("BND", "Brunei Dollar", "B$", 2),
        ("FJD", "Fijian Dollar", "FJ$", 2),
    ];
    let mut stmt = conn.prepare(
        "INSERT INTO currencies (code, name, symbol, exponent, is_active)
         VALUES (?1, ?2, ?3, ?4, 1)
         ON CONFLICT(code) DO UPDATE SET name = excluded.name, symbol = excluded.symbol, exponent = excluded.exponent",
    )?;
    for (code, name, symbol, exp) in CURRENCIES {
        stmt.execute(params![code, name, symbol, exp])?;
    }
    Ok(())
}

fn seed_settings(conn: &Connection, tenant_id: &str) -> AppResult<()> {
    let now = now_iso();
    let defaults = vec![
        ("primary_currency", json!("QAR")),
        ("locale", json!("en")),
        ("budget_cycle_start_day", json!(28)),
        ("salary_pay_day", json!(28)),
        ("onboarding_completed", json!(false)),
        ("backup_enabled", json!(true)),
        ("backup_retention_days", json!(30)),
        ("backup_time", json!("02:00")),
        ("app_lock", json!({"enabled": false})),
        ("health_weights", json!({
            "emergency_fund": 25,
            "debt": 20,
            "cash_flow": 20,
            "savings_rate": 15,
            "budget_adherence": 10,
            "diversification": 10
        })),
        ("fi_assumptions", json!({
            "annual_return_pct": 6.0,
            "inflation_pct": 2.5,
            "withdrawal_rate_pct": 4.0,
            "monthly_expenses": 0
        })),
        ("emergency_fund_months", json!(6)),
    ];
    let mut stmt = conn.prepare(
        "INSERT INTO tenant_settings (tenant_id, key, value, updated_at) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (key, value) in defaults {
        stmt.execute(params![tenant_id, key, value.to_string(), now])?;
    }
    Ok(())
}

fn ensure_default_settings(conn: &Connection, tenant_id: &str) -> AppResult<()> {
    let now = now_iso();
    let defaults = vec![
        ("primary_currency", json!("QAR")),
        ("locale", json!("en")),
        ("budget_cycle_start_day", json!(28)),
        ("salary_pay_day", json!(28)),
        ("onboarding_completed", json!(false)),
        ("backup_enabled", json!(true)),
        ("backup_retention_days", json!(30)),
        ("backup_time", json!("02:00")),
        ("app_lock", json!({"enabled": false})),
        ("health_weights", json!({
            "emergency_fund": 25,
            "debt": 20,
            "cash_flow": 20,
            "savings_rate": 15,
            "budget_adherence": 10,
            "diversification": 10
        })),
        ("fi_assumptions", json!({
            "annual_return_pct": 6.0,
            "inflation_pct": 2.5,
            "withdrawal_rate_pct": 4.0,
            "monthly_expenses": 0
        })),
        ("emergency_fund_months", json!(6)),
    ];
    let mut stmt = conn.prepare(
        "INSERT OR IGNORE INTO tenant_settings (tenant_id, key, value, updated_at) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (key, value) in defaults {
        stmt.execute(params![tenant_id, key, value.to_string(), now])?;
    }
    Ok(())
}

fn seed_categories(conn: &Connection, tenant_id: &str, user_id: &str) -> AppResult<()> {
    let now = now_iso();
    let roots: &[(&str, &str, &str, &str)] = &[
        ("Income", "income", "briefcase", "#C3364D"),
        ("Housing", "expense", "home", "#8E6A5A"),
        ("Groceries", "expense", "shopping-basket", "#7C8F6A"),
        ("Transport", "expense", "car", "#5F7A8E"),
        ("Utilities", "expense", "plug", "#8A7A5E"),
        ("Dining & Cafés", "expense", "utensils", "#A8756A"),
        ("Health", "expense", "heart-pulse", "#6F8A7E"),
        ("Shopping", "expense", "shopping-bag", "#8E6A8A"),
        ("Education", "expense", "graduation-cap", "#6A7A9E"),
        ("Entertainment", "expense", "clapperboard", "#8A5E6A"),
        ("Travel", "expense", "plane", "#5E8A8A"),
        ("Subscriptions", "expense", "repeat", "#7A6A8E"),
        ("Personal", "expense", "user", "#8E7A6A"),
        ("Gifts & Giving", "expense", "gift", "#A05A6A"),
        ("Taxes", "expense", "landmark", "#6A6A7A"),
        ("Fees", "expense", "receipt", "#7A7A6A"),
        ("Other", "expense", "circle-ellipsis", "#6A6A6E"),
        ("Opening Balance", "equity", "flag", "#5E6A7A"),
        ("Balance Adjustment", "equity", "scale", "#5E6A7A"),
    ];
    let children: &[(&str, &str, &str, &str)] = &[
        ("Housing", "Rent", "expense", "key"),
        ("Housing", "Mortgage", "expense", "landmark"),
        ("Housing", "Home services", "expense", "wrench"),
        ("Transport", "Fuel", "expense", "fuel"),
        ("Transport", "Public transit", "expense", "bus"),
        ("Transport", "Car costs", "expense", "car"),
        ("Utilities", "Electricity", "expense", "zap"),
        ("Utilities", "Water", "expense", "droplets"),
        ("Utilities", "Internet & phone", "expense", "wifi"),
        ("Income", "Salary", "income", "briefcase"),
        ("Income", "Business", "income", "building"),
        ("Income", "Freelance", "income", "laptop"),
        ("Income", "Dividends", "income", "trending-up"),
        ("Income", "Other income", "income", "plus-circle"),
    ];

    let mut ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut insert = conn.prepare(
        "INSERT INTO categories (id, tenant_id, user_id, parent_id, name, kind, icon, color, is_system, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9)",
    )?;
    for (index, (name, kind, icon, color)) in roots.iter().enumerate() {
        let id = uuid::Uuid::new_v4().to_string();
        insert.execute(params![id, tenant_id, user_id, Option::<String>::None, name, kind, icon, color, now])?;
        ids.insert((*name).to_string(), id.clone());
        conn.execute(
            "UPDATE categories SET sort_order = ?2 WHERE id = ?1",
            params![id, index as i64],
        )?;
    }
    for (index, (parent, name, kind, icon)) in children.iter().enumerate() {
        let Some(parent_id) = ids.get(*parent) else { continue };
        let id = uuid::Uuid::new_v4().to_string();
        insert.execute(params![id, tenant_id, user_id, parent_id, name, kind, icon, Option::<String>::None, now])?;
        conn.execute(
            "UPDATE categories SET sort_order = ?2 WHERE id = ?1",
            params![id, 100 + index as i64],
        )?;
    }
    Ok(())
}

pub fn get_setting_string(conn: &Connection, tenant_id: &str, key: &str) -> AppResult<Option<String>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM tenant_settings WHERE tenant_id = ?1 AND key = ?2",
            params![tenant_id, key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(raw.and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok()).and_then(|v| match v {
        serde_json::Value::String(s) => Some(s),
        other => Some(other.to_string()),
    }))
}

pub fn get_setting_value(
    conn: &Connection,
    tenant_id: &str,
    key: &str,
) -> AppResult<Option<serde_json::Value>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT value FROM tenant_settings WHERE tenant_id = ?1 AND key = ?2",
            params![tenant_id, key],
            |r| r.get(0),
        )
        .optional()?;
    Ok(raw.and_then(|v| serde_json::from_str(&v).ok()))
}

pub fn set_setting_value(
    conn: &Connection,
    tenant_id: &str,
    key: &str,
    value: &serde_json::Value,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO tenant_settings (tenant_id, key, value, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(tenant_id, key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![tenant_id, key, value.to_string(), now_iso()],
    )?;
    Ok(())
}

pub fn audit(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    entity_type: &str,
    entity_id: &str,
    action: &str,
    before: Option<&serde_json::Value>,
    after: Option<&serde_json::Value>,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO audit_events (id, tenant_id, user_id, entity_type, entity_id, action, before_json, after_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            uuid::Uuid::new_v4().to_string(),
            tenant_id,
            user_id,
            entity_type,
            entity_id,
            action,
            before.map(|v| v.to_string()),
            after.map(|v| v.to_string()),
            now_iso()
        ],
    )?;
    Ok(())
}

pub fn integrity_check(conn: &Connection) -> AppResult<String> {
    let result: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn encrypted_database_round_trips_and_rejects_wrong_key() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("rawjan.db");

        {
            let mut conn = open(&path, Some(KEY)).expect("open with key");
            apply_migrations(&mut conn).expect("migrations");
            let (tenant, user) = seed_if_empty(&mut conn).expect("seed");
            assert!(!tenant.is_empty());
            assert!(!user.is_empty());
        }

        {
            let conn = open(&path, Some(KEY)).expect("reopen with same key");
            let currencies: i64 = conn
                .query_row("SELECT count(*) FROM currencies", [], |r| r.get(0))
                .expect("read currencies");
            assert!(currencies > 10);
        }

        let wrong = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
        assert!(open(&path, Some(wrong)).is_err(), "wrong key must not open the vault");
    }

    #[test]
    fn plaintext_read_is_rejected_for_encrypted_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("rawjan.db");
        {
            let mut conn = open(&path, Some(KEY)).expect("open with key");
            apply_migrations(&mut conn).expect("migrations");
        }
        // Opening the same file without a key must not silently expose plaintext.
        assert!(open(&path, None).is_err());
    }
}
