use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::dates::{self, now_iso};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntryDto {
    pub id: String,
    pub title: String,
    pub body: String,
    pub entry_date: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalInput {
    pub title: String,
    pub body: Option<String>,
    pub entry_date: String,
}

pub fn list_entries(
    conn: &Connection,
    tenant_id: &str,
    search: Option<&str>,
    limit: Option<i64>,
) -> AppResult<Vec<JournalEntryDto>> {
    let limit = limit.unwrap_or(200).clamp(1, 1000);
    let pattern = search
        .filter(|value| !value.trim().is_empty())
        .map(|value| format!("%{}%", value.trim().to_lowercase()));

    let mut stmt = conn.prepare(
        "SELECT id, title, body, entry_date, created_at, updated_at
         FROM journal_entries
         WHERE tenant_id = ?1
           AND (?2 IS NULL OR lower(title) LIKE ?2 OR lower(body) LIKE ?2)
         ORDER BY entry_date DESC, created_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![tenant_id, pattern, limit], |row| {
        Ok(JournalEntryDto {
            id: row.get(0)?,
            title: row.get(1)?,
            body: row.get(2)?,
            entry_date: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn save_entry(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: Option<&str>,
    input: &JournalInput,
) -> AppResult<String> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(AppError::Validation("Give the journal entry a title.".into()));
    }
    if title.len() > 200 {
        return Err(AppError::Validation(
            "Keep the journal title under 200 characters.".into(),
        ));
    }
    let body = input.body.as_deref().unwrap_or("").trim().to_string();
    let entry_date = dates::iso(dates::parse_date(&input.entry_date)?);
    let now = now_iso();

    match id {
        Some(existing) => {
            let before: Option<JournalEntryDto> = conn
                .query_row(
                    "SELECT id, title, body, entry_date, created_at, updated_at
                     FROM journal_entries WHERE id = ?1 AND tenant_id = ?2",
                    params![existing, tenant_id],
                    |row| {
                        Ok(JournalEntryDto {
                            id: row.get(0)?,
                            title: row.get(1)?,
                            body: row.get(2)?,
                            entry_date: row.get(3)?,
                            created_at: row.get(4)?,
                            updated_at: row.get(5)?,
                        })
                    },
                )
                .optional()?;
            let Some(before) = before else {
                return Err(AppError::NotFound("Journal entry not found.".into()));
            };
            conn.execute(
                "UPDATE journal_entries SET title = ?1, body = ?2, entry_date = ?3, updated_at = ?4
                 WHERE id = ?5 AND tenant_id = ?6",
                params![title, body, entry_date, now, existing, tenant_id],
            )?;
            crate::db::audit(
                conn,
                tenant_id,
                user_id,
                "journal_entry",
                existing,
                "update",
                Some(&serde_json::to_value(&before)?),
                Some(&serde_json::json!({
                    "title": title,
                    "body": body,
                    "entryDate": entry_date
                })),
            )?;
            Ok(existing.to_string())
        }
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO journal_entries (id, tenant_id, user_id, title, body, entry_date, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![new_id, tenant_id, user_id, title, body, entry_date, now],
            )?;
            crate::db::audit(
                conn,
                tenant_id,
                user_id,
                "journal_entry",
                &new_id,
                "create",
                None,
                Some(&serde_json::json!({
                    "title": title,
                    "body": body,
                    "entryDate": entry_date
                })),
            )?;
            Ok(new_id)
        }
    }
}

pub fn delete_entry(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
) -> AppResult<()> {
    let changed = conn.execute(
        "DELETE FROM journal_entries WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Journal entry not found.".into()));
    }
    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "journal_entry",
        id,
        "delete",
        None,
        None,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, open_test};

    fn setup() -> (Connection, String, String) {
        let mut conn = open_test();
        let (tenant, user) = db::seed_if_empty(&mut conn).unwrap();
        (conn, tenant, user)
    }

    fn input(title: &str, body: &str, date: &str) -> JournalInput {
        JournalInput {
            title: title.into(),
            body: Some(body.into()),
            entry_date: date.into(),
        }
    }

    #[test]
    fn journal_entries_round_trip_and_search() {
        let (conn, tenant, user) = setup();
        let id = save_entry(&conn, &tenant, &user, None, &input("Rent decision", "Renegotiated the lease.", "2026-11-02"))
            .unwrap();
        save_entry(&conn, &tenant, &user, None, &input("Bonus plan", "Save half of the bonus.", "2026-11-10"))
            .unwrap();

        let all = list_entries(&conn, &tenant, None, None).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].title, "Bonus plan");

        let found = list_entries(&conn, &tenant, Some("lease"), None).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, id);
    }

    #[test]
    fn journal_update_changes_fields_and_delete_removes() {
        let (conn, tenant, user) = setup();
        let id = save_entry(&conn, &tenant, &user, None, &input("Draft", "First thought.", "2026-11-01")).unwrap();
        save_entry(
            &conn,
            &tenant,
            &user,
            Some(&id),
            &input("Final", "Revised thought.", "2026-11-02"),
        )
        .unwrap();

        let entries = list_entries(&conn, &tenant, None, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Final");
        assert_eq!(entries[0].entry_date, "2026-11-02");

        delete_entry(&conn, &tenant, &user, &id).unwrap();
        assert!(list_entries(&conn, &tenant, None, None).unwrap().is_empty());
        assert!(delete_entry(&conn, &tenant, &user, &id).is_err());
    }

    #[test]
    fn journal_requires_a_title_and_valid_date() {
        let (conn, tenant, user) = setup();
        assert!(save_entry(&conn, &tenant, &user, None, &input("  ", "body", "2026-11-01")).is_err());
        assert!(save_entry(&conn, &tenant, &user, None, &input("Title", "body", "01/11/2026")).is_err());
    }
}
