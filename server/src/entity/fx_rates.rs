//! FX-Rate-Store (Phase 1.7.2).
//!
//! Historische Wechselkurse pro `(date, from_currency, to_currency)`.
//! `rate` ist `from -> to` (z.B. EUR→USD am 2024-06-15 ist 1.078); der
//! Service inverts beim umgekehrten Lookup, statt die Tabelle doppelt zu
//! pflegen.
//!
//! `source` ist ein freier String (`"ecb"`, `"manual"`, `"plugin:abc"`,
//! …) — nicht enforced, dient Audit + Diagnostik.
//!
//! Multi-Tenancy: `tenant_id` nullable als Vorbereitung; aktuell immer
//! `NULL` (Phase 0.7-konform).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "fx_rates")]
pub struct Model {
    /// ISO-Datum (`YYYY-MM-DD`).
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub date: String,
    /// ISO-4217-Code, drei Buchstaben (`EUR`, `USD`, `CHF`, …).
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub from_currency: String,
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub to_currency: String,
    /// Faktor `from → to`. SQLite speichert REAL; fuer Banker-Rounding
    /// rechnen wir trotzdem via `rust_decimal` im Service.
    pub rate: f64,
    /// Frei: Provider-Name oder Plugin-ID.
    #[sea_orm(column_type = "Text")]
    pub source: String,
    #[sea_orm(column_type = "Text", nullable)]
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
