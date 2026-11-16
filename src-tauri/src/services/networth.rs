use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::money::format_amount;
use crate::services::accounts::{net_worth, save_net_worth_snapshot, NetWorth};
use crate::services::currency::{currency_meta, exponent, primary_currency};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDto {
    pub id: String,
    pub as_of_date: String,
    pub currency: String,
    pub assets_minor: i64,
    pub assets: String,
    pub liabilities_minor: i64,
    pub liabilities: String,
    pub net_minor: i64,
    pub net: String,
    pub cash_minor: i64,
    pub cash: String,
    pub invested_minor: i64,
    pub invested: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthOverview {
    pub current: NetWorth,
    pub snapshots: Vec<SnapshotDto>,
    pub latest_snapshot: Option<SnapshotDto>,
    pub change_minor: Option<i64>,
    pub change: Option<String>,
}

pub fn list_snapshots(
    conn: &Connection,
    tenant_id: &str,
    limit: Option<i64>,
) -> AppResult<Vec<SnapshotDto>> {
    let limit = limit.unwrap_or(120).clamp(1, 1000);
    let mut stmt = conn.prepare(
        "SELECT id, as_of_date, currency_code, assets_minor, liabilities_minor, net_minor,
                cash_minor, invested_minor, source, created_at
         FROM net_worth_snapshots
         WHERE tenant_id = ?1
         ORDER BY as_of_date DESC, created_at DESC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![tenant_id, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut result = Vec::with_capacity(rows.len());
    for (
        id,
        as_of_date,
        currency,
        assets_minor,
        liabilities_minor,
        net_minor,
        cash_minor,
        invested_minor,
        source,
        created_at,
    ) in rows
    {
        let exp = currency_meta(conn, &currency)?.exponent;
        result.push(SnapshotDto {
            id,
            as_of_date,
            currency: currency.clone(),
            assets_minor,
            assets: format_amount(assets_minor, exp),
            liabilities_minor,
            liabilities: format_amount(liabilities_minor, exp),
            net_minor,
            net: format_amount(net_minor, exp),
            cash_minor,
            cash: format_amount(cash_minor, exp),
            invested_minor,
            invested: format_amount(invested_minor, exp),
            source,
            created_at,
        });
    }
    Ok(result)
}

pub fn overview(conn: &Connection, tenant_id: &str) -> AppResult<NetWorthOverview> {
    let current = net_worth(conn, tenant_id, None)?;
    let snapshots = list_snapshots(conn, tenant_id, Some(120))?;
    let latest = snapshots.first().cloned();

    // Only compare like with like. If the stored snapshot uses a different base
    // currency, no change is reported rather than converting it silently.
    let (change_minor, change) = match latest.as_ref() {
        Some(snapshot) if snapshot.currency == current.currency => {
            let delta = current.net_minor - snapshot.net_minor;
            let exp = exponent(conn, &current.currency)?;
            (Some(delta), Some(format_amount(delta, exp)))
        }
        _ => (None, None),
    };

    Ok(NetWorthOverview {
        current,
        snapshots,
        latest_snapshot: latest,
        change_minor,
        change,
    })
}

pub fn capture_snapshot(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
) -> AppResult<SnapshotDto> {
    let worth = save_net_worth_snapshot(conn, tenant_id)?;
    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "net_worth_snapshot",
        &worth.as_of_date,
        "capture",
        None,
        Some(&serde_json::json!({
            "asOfDate": worth.as_of_date,
            "assetsMinor": worth.assets_minor,
            "liabilitiesMinor": worth.liabilities_minor,
            "netMinor": worth.net_minor,
            "currency": worth.currency
        })),
    )?;

    let snapshot = conn
        .query_row(
            "SELECT id FROM net_worth_snapshots WHERE tenant_id = ?1 AND as_of_date = ?2",
            params![tenant_id, worth.as_of_date],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Internal("Snapshot was saved but could not be read back.".into()))?;

    let snapshots = list_snapshots(conn, tenant_id, Some(1))?;
    snapshots
        .into_iter()
        .find(|item| item.id == snapshot)
        .ok_or_else(|| AppError::Internal("Snapshot was saved but could not be read back.".into()))
}

pub fn delete_snapshot(
    conn: &Connection,
    tenant_id: &str,
    user_id: &str,
    id: &str,
) -> AppResult<()> {
    let changed = conn.execute(
        "DELETE FROM net_worth_snapshots WHERE id = ?1 AND tenant_id = ?2",
        params![id, tenant_id],
    )?;
    if changed == 0 {
        return Err(AppError::NotFound("Snapshot not found.".into()));
    }
    crate::db::audit(
        conn,
        tenant_id,
        user_id,
        "net_worth_snapshot",
        id,
        "delete",
        None,
        None,
    )?;
    Ok(())
}

pub fn latest_base_currency(conn: &Connection, tenant_id: &str) -> AppResult<String> {
    primary_currency(conn, tenant_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{self, open_test};
    use crate::services::accounts;

    fn setup() -> (Connection, String, String) {
        let mut conn = open_test();
        let (tenant, user) = db::seed_if_empty(&mut conn).unwrap();
        (conn, tenant, user)
    }

    #[test]
    fn capturing_a_snapshot_is_idempotent_for_the_day() {
        let (conn, tenant, user) = setup();
        let first = capture_snapshot(&conn, &tenant, &user).unwrap();
        let second = capture_snapshot(&conn, &tenant, &user).unwrap();
        assert_eq!(first.id, second.id);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM net_worth_snapshots", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn overview_reports_change_since_latest_snapshot() {
        let (conn, tenant, user) = setup();
        let snapshot = capture_snapshot(&conn, &tenant, &user).unwrap();
        let overview = overview(&conn, &tenant).unwrap();
        assert_eq!(overview.snapshots.len(), 1);
        assert_eq!(overview.latest_snapshot.as_ref().unwrap().id, snapshot.id);
        assert_eq!(overview.change_minor, Some(0));
        assert_eq!(overview.change.as_deref(), Some("0.00"));
    }

    #[test]
    fn deleting_a_snapshot_removes_it_from_the_history() {
        let (conn, tenant, user) = setup();
        let snapshot = capture_snapshot(&conn, &tenant, &user).unwrap();
        delete_snapshot(&conn, &tenant, &user, &snapshot.id).unwrap();
        let overview = overview(&conn, &tenant).unwrap();
        assert!(overview.snapshots.is_empty());
        assert!(overview.latest_snapshot.is_none());
        assert!(overview.change_minor.is_none());
    }

    #[test]
    fn receivables_count_as_assets_not_liabilities() {
        let (conn, tenant, user) = setup();
        let liability_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO liabilities (id, tenant_id, user_id, name, type, principal_minor, outstanding_minor,
                currency_code, interest_rate_bps, payment_minor, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'Loan to friend', 'money_lent', 100000, 100000, 'QAR', 0, 0, 'active', ?4, ?4)",
            params![liability_id, tenant, user, crate::dates::now_iso()],
        )
        .unwrap();

        let worth = accounts::net_worth(&conn, &tenant, None).unwrap();
        assert_eq!(worth.receivables_minor, 100_000);
        assert_eq!(worth.assets_minor, 100_000);
        assert_eq!(worth.liabilities_minor, 0);
        assert_eq!(worth.net_minor, 100_000);
        assert_eq!(worth.other_assets_minor, 0);
    }
}
