//! Job-Run-Audit (Phase 1.7.7).
//!
//! Pro `trigger_now`-Aufruf ein Run-Eintrag. Bei Retries entsteht **eine
//! neue Run-Row** pro Versuch — die letzte mit `status="success"` wird
//! als endgueltiger Erfolg gewertet.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "job_runs")]
pub struct Model {
    /// ULID — vom Service vergeben.
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    /// FK auf `jobs.id`.
    #[sea_orm(column_type = "Text")]
    pub job_id: String,
    /// Versuch-Nummer (1-basiert): 1=erster Aufruf, 2=erster Retry, ...
    pub attempt: i32,
    /// ISO-8601-Start (UTC).
    #[sea_orm(column_type = "Text")]
    pub started_at: String,
    /// ISO-8601-Ende (UTC). NULL solange `running`.
    #[sea_orm(column_type = "Text", nullable)]
    pub finished_at: Option<String>,
    /// `running` | `success` | `failed`.
    #[sea_orm(column_type = "Text")]
    pub status: String,
    /// Dauer in Millisekunden — gesetzt beim Abschluss.
    #[sea_orm(column_type = "Integer", nullable)]
    pub duration_ms: Option<i64>,
    /// Fehlermeldung bei `failed`.
    #[sea_orm(column_type = "Text", nullable)]
    pub error_message: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
