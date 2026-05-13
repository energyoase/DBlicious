//! Permission-Modell (Phase 0.7).
//!
//! `Subject → Resource → Op → Effect` mit Group-/Role-Vererbung. Der Server
//! ist die Single Source of Truth fuer die Auswertung; dieses Modul
//! definiert nur das Wire-Format, das Server und Client gleichermassen
//! konsumieren.
//!
//! Verhaeltnis zum bestehenden [`crate::security`]-Modul: das alte Modell
//! (`Permission` mit `entity_type` + `can_*`-Flags) bleibt waehrend der
//! Uebergangsphase parallel bestehen und wird in Phase 0.7.4/0.7.5
//! abgeloest. Aufrufer, die das neue Modell konsumieren, nutzen den vollen
//! Pfad `shared::auth::Permission`, weil `crate::security::Permission` als
//! Top-Level reexportiert ist.
//!
//! Konventionen:
//! - Alle tagged Enums benutzen `#[serde(tag = "kind", rename_all = "camelCase")]`
//!   fuer die Tag-Werte, aber das **innere** Layout bleibt snake_case (gleiches
//!   Verhalten wie bei [`crate::FieldType`] — siehe `shared/tests/field_type_wire_format.rs`).
//! - `Op` und `Effect` sind keine tagged Enums; sie serialisieren als
//!   camelCase-String (`"create"`, `"approve"`, `"allow"`, `"deny"`).

use serde::{Deserialize, Serialize};

// =============================================================================
// Subject — wer haelt eine Permission
// =============================================================================

/// Subjekt einer Permission. Permissions koennen direkt einem User, einer
/// Group (organisatorische Mitgliedschaft) oder einer Role (Rechte-Buendel)
/// zugewiesen sein. Group und Role sind bewusst orthogonal — beide werden in
/// der Praxis gleichzeitig gebraucht.
///
/// Vererbung: ein User erbt Permissions
///   1. die ihm direkt zugewiesen sind,
///   2. die irgendeiner seiner Groups zugewiesen sind,
///   3. die einer Role zugewiesen sind, die ihm oder einer seiner Groups
///      zugewiesen ist.
///
/// Die Aufloesung passiert im Resolver (Phase 0.7.3), nicht in diesem Typ.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Subject {
    User { id: String },
    Group { id: String },
    Role { id: String },
}

impl Subject {
    pub fn id(&self) -> &str {
        match self {
            Subject::User { id } | Subject::Group { id } | Subject::Role { id } => id,
        }
    }
}

// =============================================================================
// Resource — worauf bezieht sich eine Permission
// =============================================================================

/// Resource, auf die sich eine Permission bezieht. Die Varianten decken alle
/// Berechtigungs-Ebenen ab, die die Roadmap vorsieht — auch wenn nicht alle
/// in Phase 0.7 enforced sind:
///
/// - [`Resource::EntityType`] und [`Resource::EntityProperty`]: enforced ab 0.7.4.
/// - [`Resource::EntityInstance`] (Row-Level): im Schema vorhanden, aber in 0.7
///   wird der Resolver `not_implemented` werfen, solange `permissions.enable_row_level`
///   in der Server-Config nicht aktiviert ist. Schrittweise Aktivierung spaeter.
/// - [`Resource::Action`]: benannte Aktionen ohne Entity-Bezug (`"exportCsv"`,
///   `"rebuildIndex"`).
/// - [`Resource::ImplementationId`]: Phase 1.5 — wer darf welchen Filter/
///   Editor/Formatter waehlen.
/// - [`Resource::Migration`]: Phase 3 — wer darf eine Migration approven,
///   cutovern, contracten oder rollbacken.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Resource {
    /// Ein gesamter Entity-Typ ("product", "order", ...).
    EntityType { name: String },
    /// Eine einzelne Property eines Entity-Typs ("product.price").
    EntityProperty { entity_type: String, property: String },
    /// Eine konkrete Zeile (Row-Level). Phase 0.7 enforced das nicht — der
    /// Resolver liefert `not_implemented`, solange die Server-Config das
    /// nicht freischaltet.
    EntityInstance { entity_type: String, id: String },
    /// Eine benannte Aktion ohne Entity-Bezug.
    Action { name: String },
    /// Eine Implementations-ID (Filter, Editor, Formatter, …). `registry`
    /// ist die Registry-Art ("filter", "editor", "formatter", "action");
    /// `id` ist die konkrete Implementations-ID innerhalb der Registry.
    /// Wildcard `id = "*"` ist im Datensatz erlaubt und wird vom Resolver
    /// als "alle IDs der Registry" interpretiert. Das Feld heisst absichtlich
    /// `registry` statt `kind`, weil `kind` mit dem `#[serde(tag = "kind")]`
    /// der Enum-Ebene kollidieren wuerde.
    ImplementationId { registry: String, id: String },
    /// Eine konkrete Migration (Phase 3).
    Migration { id: String },
}

impl Resource {
    /// Konstruiert eine Resource fuer den haeufigsten Fall: ein ganzer
    /// Entity-Typ.
    pub fn entity_type(name: impl Into<String>) -> Self {
        Resource::EntityType { name: name.into() }
    }

    /// Konstruiert eine Resource fuer eine einzelne Property.
    pub fn entity_property(entity_type: impl Into<String>, property: impl Into<String>) -> Self {
        Resource::EntityProperty {
            entity_type: entity_type.into(),
            property: property.into(),
        }
    }
}

// =============================================================================
// Op — welche Operation
// =============================================================================

/// Operation, die auf einer Resource ausgefuehrt wird.
///
/// CRUD ([`Op::Create`]/[`Op::Read`]/[`Op::Update`]/[`Op::Delete`]) ist der
/// klassische Pfad fuer Entity-Resources. [`Op::Execute`] greift bei
/// [`Resource::Action`] und [`Resource::Migration`] (Migrationen sind
/// "ausfuehrbar"). [`Op::Choose`] ist die Wahl einer Implementations-ID
/// (Phase 1.5). Die Migration-Lifecycle-Ops ([`Op::Approve`], [`Op::Cutover`],
/// [`Op::Contract`], [`Op::Rollback`]) sind absichtlich getrennt — wer
/// approven darf, darf nicht automatisch contracten (destruktive Aktion).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum Op {
    Create,
    Read,
    Update,
    Delete,
    Execute,
    Choose,
    Approve,
    Cutover,
    Contract,
    Rollback,
}

// =============================================================================
// Effect — Erlauben oder Verbieten
// =============================================================================

/// Effekt einer Permission-Regel bzw. Ergebnis einer Resolver-Auswertung.
///
/// Bei Konflikt zwischen mehreren passenden Regeln gilt: **Deny gewinnt vor
/// Allow**. Innerhalb desselben Effekts entscheidet [`Permission::priority`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "camelCase")]
pub enum Effect {
    #[default]
    Allow,
    Deny,
}

// =============================================================================
// Permission — die eigentliche Regel
// =============================================================================

/// Eine konkrete Permission-Regel.
///
/// Eine Regel verknuepft ein [`Subject`] mit einer [`Resource`], einer
/// [`Op`] und einem [`Effect`]. Mehrere Regeln koennen auf dasselbe Tupel
/// `(Subject, Resource, Op)` zutreffen — die Aufloesung passiert im
/// Resolver (Phase 0.7.3) nach folgender Reihenfolge:
///
/// 1. Spezifischere Resource gewinnt (EntityInstance > EntityProperty >
///    EntityType).
/// 2. Bei gleicher Spezifitaet: Deny gewinnt vor Allow.
/// 3. Bei gleichem Effect: hoehere [`Permission::priority`] gewinnt.
/// 4. Bei gleicher Prioritaet: ist undefiniert — der Loader soll auf
///    Duplikate warnen, der Resolver muss aber trotzdem deterministisch
///    entscheiden (z.B. lexikographisch nach Subject-ID).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Permission {
    pub subject: Subject,
    pub resource: Resource,
    pub op: Op,
    pub effect: Effect,
    /// Bei Konflikten gleichen Effekts gewinnt hoher Wert. Default 0.
    #[serde(default)]
    pub priority: i32,
    /// Multi-Tenancy-Vorbereitung (Phase 0.7 Schema-only, kein Enforcement).
    /// `None` = global / single-tenant Default. Der Resolver beachtet dieses
    /// Feld nur, wenn Multi-Tenancy in der Server-Config aktiviert ist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
}
