//! Number-Sequence-Tabelle (Phase 1.7.1).
//!
//! Pro `(scope, year)` exakt eine Zeile. `year` ist `0` fuer
//! nicht-fiscal-year-resettende Sequenzen (Composite-PK braucht
//! NOT-NULL), sonst der Fiskaljahr-Wert (z.B. 2026).
//!
//! `current` ist die zuletzt vergebene Nummer; `next_number()` liefert
//! `current + 1` und increment in derselben Transaktion (gapless durch
//! SQLite's globalen Write-Lock + BEGIN IMMEDIATE).
//!
//! `format_template` ist ein Tera-Style-Template — siehe
//! [`crate::sequences::format`]. Beispiele:
//! - `"{seq:06}"`           → `"000042"`
//! - `"INV-{year}-{seq:06}"` → `"INV-2026-000042"`
//!
//! Phase-0.7-konform: `tenant_id` als nullable Spalte reserviert; heutige
//! Default-Pflege ist `NULL` (single-tenant).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "number_sequences")]
pub struct Model {
    /// Logischer Sequenz-Name (z.B. `"invoice"`, `"order"`,
    /// `"datev_entry"`). Bildet zusammen mit `year` den Composite-PK.
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub scope: String,
    /// Fiskaljahr oder `0` fuer kein-Reset. Composite-PK-Teil.
    #[sea_orm(primary_key, auto_increment = false)]
    pub year: i32,
    /// Zuletzt vergebene Nummer; `next_number` increment um 1.
    pub current: i64,
    /// Format-Template (Tera-Style: `{scope}`, `{year}`, `{seq[:NN]}`).
    #[sea_orm(column_type = "Text")]
    pub format_template: String,
    /// Multi-Tenancy-Vorbereitung (Phase 0.7 — heute immer NULL).
    #[sea_orm(column_type = "Text", nullable)]
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
