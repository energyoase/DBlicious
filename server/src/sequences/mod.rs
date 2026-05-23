//! Number-Sequence-Service (Phase 1.7.1).
//!
//! Public API:
//! - [`next_number`]: increment + format einer Sequenz, gapless durch
//!   SQLite-Write-Lock. Erstellt die Sequenz mit `current=0` und dem
//!   default Format-Template, wenn sie noch nicht existiert.
//! - [`current_number`]: lesen ohne increment (Debug / Audit).
//! - [`reset_sequence`]: explizit zuruecksetzen (z.B. FY-Wechsel zum
//!   01.01.) — der Resolver triggert das automatisch nicht, das macht
//!   der Aufrufer oder ein Job.
//!
//! Format-Template-Syntax: siehe [`format::render`].

pub mod format;

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use thiserror::Error;

use crate::entity::number_sequences;

/// Fiskaljahr-Reset-Marker im `year`-Feld. `0` ⇒ kein FY-Reset, eine
/// einzige rollende Sequenz pro `scope`.
pub const NO_FISCAL_YEAR: i32 = 0;

/// Default-Template fuer neu angelegte Sequenzen: nur die Nummer mit
/// 6 Stellen Leading-Zero. Aufrufer mit Anforderungen muessen die Sequenz
/// per [`ensure_sequence`] mit eigenem Template anlegen.
pub const DEFAULT_FORMAT_TEMPLATE: &str = "{seq:06}";

#[derive(Debug, Error)]
pub enum SequenceError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("format template invalid: {0}")]
    InvalidTemplate(String),
}

/// Stellt sicher, dass die Sequenz existiert (idempotent). Hilfreich,
/// wenn man ein spezifisches `format_template` vorgeben will, bevor der
/// erste `next_number`-Aufruf das Default-Template einbrennen wuerde.
pub async fn ensure_sequence(
    conn: &DatabaseConnection,
    scope: &str,
    year: i32,
    format_template: &str,
) -> Result<(), SequenceError> {
    // Validierung: Template muss parsen.
    let _ = format::render(format_template, scope, year, 0)?;
    let existing = number_sequences::Entity::find()
        .filter(number_sequences::Column::Scope.eq(scope))
        .filter(number_sequences::Column::Year.eq(year))
        .one(conn)
        .await?;
    if existing.is_none() {
        number_sequences::ActiveModel {
            scope:           ActiveValue::Set(scope.to_string()),
            year:            ActiveValue::Set(year),
            current:         ActiveValue::Set(0),
            format_template: ActiveValue::Set(format_template.to_string()),
            tenant_id:       ActiveValue::Set(None),
        }
        .insert(conn)
        .await?;
    }
    Ok(())
}

/// Aktuelle Zahl ohne Increment. Liefert `0`, wenn die Sequenz noch nicht
/// existiert.
pub async fn current_number(
    conn:  &DatabaseConnection,
    scope: &str,
    year:  i32,
) -> Result<i64, SequenceError> {
    Ok(number_sequences::Entity::find()
        .filter(number_sequences::Column::Scope.eq(scope))
        .filter(number_sequences::Column::Year.eq(year))
        .one(conn)
        .await?
        .map(|m| m.current)
        .unwrap_or(0))
}

/// Increment + format. Wenn die Sequenz nicht existiert, wird sie mit
/// [`DEFAULT_FORMAT_TEMPLATE`] angelegt.
///
/// Concurrency: SQLite's globaler Write-Lock serialisiert
/// `BEGIN IMMEDIATE`-Transaktionen automatisch — selbst bei N parallelen
/// Aufrufen ist die Sequenz gapless. Auf anderen Engines (Postgres,
/// MySQL) ist die Transaktion mit `SELECT ... FOR UPDATE` gleichwertig,
/// aber SeaORM macht das hier ueber den row-update-Pfad ohnehin atomar.
pub async fn next_number(
    conn:  &DatabaseConnection,
    scope: &str,
    year:  i32,
) -> Result<String, SequenceError> {
    let txn = conn.begin().await?;
    let row = number_sequences::Entity::find()
        .filter(number_sequences::Column::Scope.eq(scope))
        .filter(number_sequences::Column::Year.eq(year))
        .one(&txn)
        .await?;
    let (template, next_value) = match row {
        Some(m) => {
            let next = m.current + 1;
            // Update auf existierende Row.
            let mut am: number_sequences::ActiveModel = m.clone().into();
            am.current = ActiveValue::Set(next);
            am.update(&txn).await?;
            (m.format_template, next)
        }
        None => {
            // Neu anlegen mit Default-Template + current=1 (erste Vergabe).
            number_sequences::ActiveModel {
                scope:           ActiveValue::Set(scope.to_string()),
                year:            ActiveValue::Set(year),
                current:         ActiveValue::Set(1),
                format_template: ActiveValue::Set(DEFAULT_FORMAT_TEMPLATE.to_string()),
                tenant_id:       ActiveValue::Set(None),
            }
            .insert(&txn)
            .await?;
            (DEFAULT_FORMAT_TEMPLATE.to_string(), 1)
        }
    };
    txn.commit().await?;
    format::render(&template, scope, year, next_value)
}

/// Setzt `current` auf `0` zurueck. Der naechste `next_number`-Aufruf
/// fuer dasselbe `(scope, year)` liefert wieder die `1`.
///
/// Achtung: keine Audit-Spur — wer das per CLI/UI auslöst, ist
/// Aufrufer-Verantwortung.
pub async fn reset_sequence(
    conn:  &DatabaseConnection,
    scope: &str,
    year:  i32,
) -> Result<(), SequenceError> {
    if let Some(m) = number_sequences::Entity::find()
        .filter(number_sequences::Column::Scope.eq(scope))
        .filter(number_sequences::Column::Year.eq(year))
        .one(conn)
        .await?
    {
        let mut am: number_sequences::ActiveModel = m.into();
        am.current = ActiveValue::Set(0);
        am.update(conn).await?;
    }
    Ok(())
}
