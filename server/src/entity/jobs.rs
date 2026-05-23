//! Job-Definition (Phase 1.7.7).
//!
//! Ein Job verbindet einen Handler-Schluessel mit optionalem
//! Cron-Schedule + Aktivierungs-Flag. Der eigentliche Code laeuft im
//! Handler — registriert ueber [`crate::jobs::registry`]. Spaetere
//! Iterationen koennen Q0009-Scripts als Handler-Quelle freischalten.

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "jobs")]
pub struct Model {
    /// Stabiler Job-Schluessel (z.B. `"nightly_dunning"`).
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    /// Anzeigename (i18n-Key oder freier Text).
    #[sea_orm(column_type = "Text")]
    pub name: String,
    /// Cron-Expression im 5-Felder-Format (`* * * * *`). MVP-Service
    /// trigger nicht automatisch ueber Cron — Feld ist Konfig fuer den
    /// spaeteren Scheduler-Loop.
    #[sea_orm(column_type = "Text", nullable)]
    pub schedule_cron: Option<String>,
    /// Schluessel des Handler-Eintrags in der `JobRegistry`.
    #[sea_orm(column_type = "Text")]
    pub handler: String,
    /// `false` ⇒ trigger ignoriert den Job.
    pub enabled: bool,
    /// Retries bei Fehler (0 = kein Retry).
    pub max_retries: i32,
    /// Letzter erfolgreicher Lauf (ISO-8601). NULL = noch nie.
    #[sea_orm(column_type = "Text", nullable)]
    pub last_run_at: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
