use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use tauri::Manager;

use crate::db;
use crate::error::{AppError, AppResult};

pub struct AppState {
    conn: Mutex<Connection>,
    pub data_dir: std::path::PathBuf,
    pub tenant_id: String,
    pub user_id: String,
    pub primary_currency: String,
    pub database_encrypted: bool,
}

impl AppState {
    pub fn initialize(app: &tauri::AppHandle) -> AppResult<Self> {
        let override_dir = std::env::var("RAWJAN_DATA_DIR")
            .ok()
            .filter(|dir| !dir.trim().is_empty());
        let using_override = override_dir.is_some();
        let data_dir = match override_dir {
            Some(dir) => {
                let path = std::path::PathBuf::from(dir);
                std::fs::create_dir_all(&path)?;
                path
            }
            None => {
                let path = app.path().app_data_dir().map_err(|e| {
                    AppError::Internal(format!("cannot resolve app data directory: {e}"))
                })?;
                std::fs::create_dir_all(&path)?;
                path
            }
        };

        if !using_override {
            db::migrate_legacy_vault(&data_dir)?;
        }

        let db_path = data_dir.join("rawjan.db");
        let (key, encrypted) = db::resolve_database_key()?;
        let mut conn = db::open(&db_path, key.as_deref())?;
        db::apply_migrations(&mut conn)?;
        let (tenant_id, user_id) = db::seed_if_empty(&mut conn)?;
        let primary_currency = db::get_setting_string(&conn, &tenant_id, "primary_currency")?
            .unwrap_or_else(|| "QAR".to_string());

        log::info!(
            "Rawjan data directory: {} (encrypted: {})",
            data_dir.display(),
            encrypted
        );

        Ok(Self {
            conn: Mutex::new(conn),
            data_dir,
            tenant_id,
            user_id,
            primary_currency,
            database_encrypted: encrypted,
        })
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
