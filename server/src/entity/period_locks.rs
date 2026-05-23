//! Period-Locks-Tabelle (Phase 1.7.3).
//!
//! Pro `scope` (üblicherweise `entity_type` wie `"datev_entry"`, oder ein
//! anderer fachlicher Schlüssel) genau ein Lock-Eintrag. `locked_through_date`
//! ist inklusiv: alles `<= locked_through_date` ist im Lock.
//!
//! Mehrere Scopes nebeneinander erlaubt (z.B. `"invoice"` bis 2024-12-31,
//! `"order"` bis 2024-09-30). Wer auf zwei Scopes gleichzeitig sperren
//! will, legt zwei Einträge an.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "period_locks")]
pub struct Model {
    /// Fachlicher Scope-Schluessel. Konvention: `entity_type`.
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub scope: String,
    /// Inklusive Datums-Grenze (`YYYY-MM-DD`). Alles `<=` davon ist gesperrt.
    #[sea_orm(column_type = "Text")]
    pub locked_through_date: String,
    /// User, der das Lock gesetzt hat (Audit).
    #[sea_orm(column_type = "Text", nullable)]
    pub locked_by: Option<String>,
    /// Wann gesetzt — ISO-8601 (UTC).
    #[sea_orm(column_type = "Text", nullable)]
    pub locked_at: Option<String>,
    /// Frei-Form Begruendung (z.B. "Q3-Abschluss").
    #[sea_orm(column_type = "Text", nullable)]
    pub reason: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
