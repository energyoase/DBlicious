//! Period-Locks-Service (Phase 1.7.3).
//!
//! Aufgabe: pruefen, ob ein gegebener `(scope, date)` durch einen
//! Period-Lock gesperrt ist. CRUD-Resolver, die Datum-bewusste Entities
//! schreiben (Buchungen mit Buchungsdatum), rufen
//! [`guard_or_period_locked`] vor jedem Insert/Update/Delete und
//! propagieren `Err(PeriodLockedError)` als `period_locked`-GraphQL-Error.
//!
//! Mutationen (`set_lock`/`clear_lock`) brauchen separate Permissions —
//! der GraphQL-Resolver enforced sie.

use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, EntityTrait};
use thiserror::Error;

use crate::entity::period_locks;

#[derive(Debug, Error)]
pub enum PeriodLockError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
}

/// Liest den Lock-Eintrag fuer `scope`, falls vorhanden.
pub async fn get_lock(
    conn: &DatabaseConnection,
    scope: &str,
) -> Result<Option<period_locks::Model>, PeriodLockError> {
    Ok(period_locks::Entity::find_by_id(scope.to_string())
        .one(conn)
        .await?)
}

/// Setzt den Lock auf `locked_through_date` (upsert). Beim Verschaerfen
/// (z.B. von 2024-06-30 auf 2024-12-31) wird der alte Wert ueberschrieben.
pub async fn set_lock(
    conn: &DatabaseConnection,
    scope: &str,
    locked_through_date: &str,
    locked_by: Option<&str>,
    reason: Option<&str>,
) -> Result<(), PeriodLockError> {
    let now = chrono::Utc::now().to_rfc3339();
    match get_lock(conn, scope).await? {
        Some(m) => {
            let mut am: period_locks::ActiveModel = m.into();
            am.locked_through_date = ActiveValue::Set(locked_through_date.to_string());
            am.locked_by = ActiveValue::Set(locked_by.map(String::from));
            am.locked_at = ActiveValue::Set(Some(now));
            am.reason = ActiveValue::Set(reason.map(String::from));
            am.update(conn).await?;
        }
        None => {
            period_locks::ActiveModel {
                scope: ActiveValue::Set(scope.to_string()),
                locked_through_date: ActiveValue::Set(locked_through_date.to_string()),
                locked_by: ActiveValue::Set(locked_by.map(String::from)),
                locked_at: ActiveValue::Set(Some(now)),
                reason: ActiveValue::Set(reason.map(String::from)),
                tenant_id: ActiveValue::Set(None),
            }
            .insert(conn)
            .await?;
        }
    }
    Ok(())
}

/// Loest den Lock fuer einen scope vollstaendig auf. Idempotent.
pub async fn clear_lock(conn: &DatabaseConnection, scope: &str) -> Result<bool, PeriodLockError> {
    let res = period_locks::Entity::delete_by_id(scope.to_string())
        .exec(conn)
        .await?;
    Ok(res.rows_affected > 0)
}

/// `true`, wenn `date` durch einen aktiven Lock fuer `scope` gesperrt ist.
/// `date` ist `YYYY-MM-DD`. Lexikographischer Vergleich genuegt fuer
/// ISO-8601-Dates.
pub async fn is_locked(
    conn: &DatabaseConnection,
    scope: &str,
    date: &str,
) -> Result<bool, PeriodLockError> {
    Ok(match get_lock(conn, scope).await? {
        Some(m) => date <= m.locked_through_date.as_str(),
        None => false,
    })
}

/// Convenience: gibt `Err(PeriodLockedError)` zurueck, wenn der Lock
/// greift. Aufrufer in CRUD-Pfaden:
/// `period_locks::guard_or_period_locked(conn, et, date).await?;`
pub async fn guard_or_period_locked(
    conn: &DatabaseConnection,
    scope: &str,
    date: &str,
) -> Result<(), PeriodLockedError> {
    let lock = get_lock(conn, scope)
        .await
        .map_err(|e| PeriodLockedError::Db(format!("{e}")))?;
    if let Some(m) = lock {
        if date <= m.locked_through_date.as_str() {
            return Err(PeriodLockedError::Locked {
                scope: scope.to_string(),
                attempted_date: date.to_string(),
                locked_through_date: m.locked_through_date,
            });
        }
    }
    Ok(())
}

/// Wird vom Guard zurueckgegeben. GraphQL-Resolver bildet das als
/// `Error::new("period_locked")` ab.
#[derive(Debug, Error)]
pub enum PeriodLockedError {
    #[error("period_locked: '{scope}' is locked through {locked_through_date} (attempted: {attempted_date})")]
    Locked {
        scope: String,
        attempted_date: String,
        locked_through_date: String,
    },
    #[error("database error: {0}")]
    Db(String),
}
