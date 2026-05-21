//! Geteilte Typen fuer die GraphQL-Kommunikation zwischen Server und Client.
//!
//! Diese Strukturen werden auf Serverseite mit `async-graphql` annotiert
//! (siehe `server/src/schema.rs`) und auf Clientseite ueber reine
//! `serde`-Deserialisierung wiederverwendet.

use serde::{Deserialize, Serialize};

pub mod auth;
pub mod builder;
pub mod editor;
pub mod error;
pub mod header;
pub mod menu;
pub mod mutation;
pub mod ops;
pub mod plugin;
pub mod security;
pub mod settings;
pub mod source;
pub mod tabs;
pub mod translatable;
pub mod validation;
pub mod view;

pub use auth::EffectivePermission;
pub use builder::{EventKind, EventTrigger, GuardExpr, TriggerTarget};
pub use editor::{ControlKind, EditorMeta, EditorPropertyMeta};
// AuditRole / ColumnGenerated leben in dieser Datei, werden aber als
// Top-Level-API mit-exportiert, damit Server/Client sie ohne Modul-Pfad
// verwenden koennen.
pub use error::{AppError, AppResult};
pub use header::{compute_hash, EntityHeader, EntityLoadState};
pub use menu::MenuAction;
pub use mutation::{EntityChangeResult, EntityCreate, EntityDelete, EntityUpdate};
pub use ops::{ops_for, ops_for_named, FieldOps};
pub use security::{
    effective_permissions, is_allowed, property_access_for, AuthFailure, AuthSession, Permission,
    PermissionOp, PropertyAccessLevel, PropertyPermission, SecurityGroup, SecurityUser,
    SecurityUser2Group,
};
pub use source::{
    default_binding_for, BindingLocator, EntityBinding, EntityId, COMPOSITE_KEY_SEPARATOR,
};
pub use settings::{
    Access, EntitySettings, FieldTypeDefaults, LoadMethod, PropertyAccess, PropertySettings,
    SettingsBundle, Visibility,
};
pub use tabs::TabInfo;
pub use translatable::{
    message_id_for_key, TranslatableBundle, TranslatableEntry, TranslatableLanguage,
    TranslatableValue,
};
pub use validation::{Severity, ValidationMessage, ValidationResult};
pub use view::{EntityView, ResolvedLayerRef, ViewLayer, ViewPropertyOverride};

/// Knoten in der rekursiven Navigationshierarchie.
///
/// Eine `route` ist optional, weil reine Gruppierungen (z.B. Menue-Ueberschriften)
/// kein Ziel besitzen. Lokalisierung erfolgt ausschliesslich ueber `label_key`,
/// niemals durch fertige Strings auf Serverseite.
///
/// `action` ist die generalisierte Form von `route` (siehe [`MenuAction`]).
/// `route` bleibt aus Kompatibilitaetsgruenden erhalten — wenn `action`
/// `None` ist, ergibt sich die Aktion implizit aus `route`. Komponenten
/// sollten [`NavigationNode::resolved_action`] benutzen, statt direkt auf
/// eines der Felder zuzugreifen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NavigationNode {
    pub id: String,
    pub label_key: String,
    pub route: Option<String>,
    pub icon: Option<String>,
    pub children: Vec<NavigationNode>,
    #[serde(default)]
    pub action: Option<MenuAction>,
}

impl NavigationNode {
    /// Liefert die wirksame Aktion eines Knotens.
    ///
    /// Bevorzugt das explizite `action`-Feld; faellt sonst auf das
    /// `route`-Feld zurueck.
    pub fn resolved_action(&self) -> MenuAction {
        if let Some(a) = &self.action {
            return a.clone();
        }
        MenuAction::from_route(self.route.clone())
    }
}

/// Beschreibt den fachlichen Typ einer Tabellenspalte.
///
/// Diese Aufzaehlung wird vom Client benutzt, um den richtigen Formatter
/// und in spaeteren Versionen den richtigen Filter-Editor zu waehlen.
/// In der ersten Iteration wird sie clientseitig hartkodiert; die Struktur
/// ist aber identisch zu dem, was der Server kuenftig liefern soll.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FieldType {
    Text,
    Integer,
    Decimal { precision: u8 },
    Boolean,
    Date,
    DateTime,
    /// Geldbetrag. `currency_code_field` verweist auf ein anderes Feld
    /// derselben Entitaet, das den ISO-4217-Code enthaelt.
    Money { currency_code_field: Option<String> },
    /// Verweis auf eine andere Entitaet (1:1).
    Reference { entity: String },
    /// Sammlung von Verweisen (1:n).
    Collection { entity: String },
    /// Aufzaehlungstyp mit fest definierten Werten.
    Enum { values: Vec<String> },
}

impl FieldType {
    /// Gibt zurueck, ob es sich um einen einfachen, direkt formatierbaren
    /// Skalar-Typ handelt. Komplexe Typen werden in der UI vorerst durch
    /// Platzhalter dargestellt.
    pub fn is_scalar(&self) -> bool {
        matches!(
            self,
            FieldType::Text
                | FieldType::Integer
                | FieldType::Decimal { .. }
                | FieldType::Boolean
                | FieldType::Date
                | FieldType::DateTime
                | FieldType::Money { .. }
                | FieldType::Enum { .. }
        )
    }

    /// Diskriminator-String, identisch zur Tag-Form im JSON-Wire-Format
    /// (`"text"`, `"integer"`, `"dateTime"`, …). Wird vom Implementations-
    /// Resolver (Phase 1.5) als Map-Key in
    /// [`settings::EntitySettings::field_type_defaults`] benutzt.
    pub fn kind_str(&self) -> &'static str {
        match self {
            FieldType::Text => "text",
            FieldType::Integer => "integer",
            FieldType::Decimal { .. } => "decimal",
            FieldType::Boolean => "boolean",
            FieldType::Date => "date",
            FieldType::DateTime => "dateTime",
            FieldType::Money { .. } => "money",
            FieldType::Reference { .. } => "reference",
            FieldType::Collection { .. } => "collection",
            FieldType::Enum { .. } => "enum",
        }
    }
}

/// Spalten-Metadaten fuer die generische Tabelle.
///
/// Implementations-IDs (`filter_id`, `editor_id`, `formatter_id`, `action_ids`)
/// folgen der Resolution-Kette aus Phase 1.5:
///   1. Server-Pflicht pro Property (dieses Feld)
///   2. Server-Default pro Entity-Typ ([`EntitySettings::field_type_defaults`])
///   3. Client-Fallback pro [`FieldType`] (hardcoded Standard)
///
/// `None`/leer = "kein Override" — die Resolution-Kette greift dann weiter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    /// Schluessel im `Entity.fields`-Map.
    pub key: String,
    /// Fluent-Schluessel fuer die Spaltenueberschrift.
    pub label_key: String,
    pub field_type: FieldType,
    pub sortable: bool,
    pub filterable: bool,
    /// Optionaler Override fuer den Sortier-/Vergleichs-Operator. Wird von
    /// [`ops::ops_for_named`] aufgeloest. Siehe [`ops::comparators`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparator_id: Option<String>,
    /// Optionaler Override fuer den Filter-Operator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_id: Option<String>,
    /// Phase 1.5: erzwungene Editor-ID fuer diese Spalte (Resolution-Stufe 1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_id: Option<String>,
    /// Phase 1.5: erzwungene Formatter-ID fuer diese Spalte.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formatter_id: Option<String>,
    /// Phase 1.5: Liste der Row-Action-IDs, die fuer Zeilen dieser Spalte
    /// angeboten werden. Leer = keine Per-Row-Aktionen aus dieser Spalte.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_ids: Vec<String>,
}

/// Generische Repraesentation einer Entitaet.
///
/// Die Felder werden bewusst als `serde_json::Value` gehalten, damit
/// dieselbe Struktur fuer beliebige Entity-Typen funktioniert.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub fields: serde_json::Map<String, serde_json::Value>,
}

/// Antwort einer paginierten Entity-Abfrage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityPage {
    pub items: Vec<Entity>,
    pub total_count: u64,
    pub page: u32,
    pub page_size: u32,
}

/// Sortierrichtung fuer Tabellenspalten.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Sortierdefinition. Wird sowohl client- als auch serverseitig verwendet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Sort {
    pub field: String,
    pub direction: SortDirection,
}

/// Typisiertes Filter-Praedikat fuer eine einzelne Spalte.
///
/// Auf der Leitung getaggt (`{"op":"textContains","value":"...","caseInsensitive":true}`),
/// analog zu [`FieldType`]. Die Variantenmenge ist bewusst auf das beschnitten,
/// was die heutigen `FieldType`-Varianten sinnvoll bedienen koennen; weitere
/// Operatoren (`Between`, `StartsWith`, ...) werden bei Bedarf ergaenzt, ohne
/// den Vertrag aufzubrechen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum FilterPredicate {
    TextContains { value: String, case_insensitive: bool },
    TextEquals { value: String, case_insensitive: bool },
    NumberEquals { value: f64 },
    NumberRange { min: Option<f64>, max: Option<f64> },
    BoolEquals { value: bool },
    DateRange { from: Option<String>, to: Option<String> },
    EnumIn { values: Vec<String> },
    IsNull,
    IsNotNull,
}

/// Verknuepft ein [`FilterPredicate`] mit einer konkreten Spalte
/// ([`ColumnMeta::key`]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnFilter {
    pub key: String,
    pub predicate: FilterPredicate,
}

/// Filter-Zustand der Tabelle.
///
/// Zwei orthogonale Mechanismen:
///   - `global_search` ist ein optionaler Freitext, der spaltenuebergreifend
///     gegen alle textartigen Spalten matcht (siehe `FieldOps::matches_search`).
///   - `predicates` enthaelt spaltenspezifische, typisierte Filter.
///
/// Beide werden konjunktiv verknuepft (UND). Die Auswertung passiert in der
/// jeweiligen [`DataSource`](../client/src/components/table/data_source.rs)
/// — der Server darf die Kriterien heute noch ignorieren.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FilterCriteria {
    #[serde(default)]
    pub global_search: Option<String>,
    #[serde(default)]
    pub predicates: Vec<ColumnFilter>,
}

impl FilterCriteria {
    pub fn is_empty(&self) -> bool {
        let search_empty = self.global_search.as_deref().is_none_or(str::is_empty);
        search_empty && self.predicates.is_empty()
    }
}

// =============================================================================
// Visueller Datenbank-Designer
// =============================================================================
//
// Die folgenden Typen beschreiben ein clientseitig modelliertes Datenbank-
// Schema. Sie werden vom Designer (siehe `client/src/components/designer/`)
// gepflegt und in einem Stueck an den Server gesendet (`saveDbSchema`).
//
// Konvention wie bei `FieldType`: der getaggte `DbColumnType`-Enum wandert
// als JSON ueber die Leitung – sowohl der GraphQL-Endpunkt als auch der
// Client deserialisieren ihn beidseitig ueber `serde`. So bleibt
// `async-graphql` von Tagged-Unions verschont.

/// Position eines Knotens auf der Designer-Leinwand (in CSS-Pixel).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Default for Position {
    fn default() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

/// Fachlicher Spaltentyp im Designer.
///
/// Bewusst eigenstaendig zum laufzeitorientierten `FieldType` gehalten:
/// hier modelliert der Nutzer die *Schema*-Definition (DDL-naehe),
/// waehrend `FieldType` die Darstellungslogik fuer geladene Daten steuert.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DbColumnType {
    Text,
    Integer,
    BigInt,
    Decimal { precision: u8, scale: u8 },
    Boolean,
    Date,
    DateTime,
    Uuid,
    Json,
    /// Fremdschluessel-Typ. Die eigentliche Beziehung wird zusaetzlich in
    /// `DbSchema.relations` modelliert; der Spaltentyp ist hier nur ein
    /// Marker fuer die UI.
    ForeignKey,
}

impl DbColumnType {
    pub fn kind(&self) -> &'static str {
        match self {
            DbColumnType::Text => "text",
            DbColumnType::Integer => "integer",
            DbColumnType::BigInt => "bigInt",
            DbColumnType::Decimal { .. } => "decimal",
            DbColumnType::Boolean => "boolean",
            DbColumnType::Date => "date",
            DbColumnType::DateTime => "dateTime",
            DbColumnType::Uuid => "uuid",
            DbColumnType::Json => "json",
            DbColumnType::ForeignKey => "foreignKey",
        }
    }
}

/// Wie ein Spaltenwert beim Einfuegen/Aktualisieren erzeugt wird.
///
/// Entspricht `Microsoft.EntityFrameworkCore.Metadata.ValueGenerated` aus
/// dem importierten C#-Modell. `OnAdd` deckt Identity-Spalten und
/// Default-Werte ab, `OnAddOrUpdate` typischerweise Row-Version-Spalten.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ColumnGenerated {
    #[default]
    Never,
    OnAdd,
    OnAddOrUpdate,
}

/// Semantische Audit-Rolle einer Spalte.
///
/// Anders als [`ColumnGenerated`] (das die *Mechanik* beschreibt: "Wird der
/// Wert vom System erzeugt?") gibt `AuditRole` die *Bedeutung* an. Der
/// Server populiert Spalten mit explizitem Audit-Rolle in jeder Mutation
/// automatisch — der Client sendet diese Felder gar nicht (siehe
/// `EditorPropertyMeta.readonly` + Visibility-Filter).
///
/// `CreatedAt`/`UpdatedAt`-Werte sind ISO-8601-Strings;
/// `CreatedBy`/`UpdatedBy` halten die User-ID des handelnden Users.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AuditRole {
    #[default]
    None,
    CreatedAt,
    UpdatedAt,
    CreatedBy,
    UpdatedBy,
}

impl AuditRole {
    pub fn fills_on_create(self) -> bool {
        matches!(
            self,
            AuditRole::CreatedAt | AuditRole::UpdatedAt | AuditRole::CreatedBy | AuditRole::UpdatedBy
        )
    }
    pub fn fills_on_update(self) -> bool {
        matches!(self, AuditRole::UpdatedAt | AuditRole::UpdatedBy)
    }
}

/// Eine Spalte einer modellierten Tabelle.
///
/// `primary_key` ist eine Bequemlichkeit fuer den Designer und bleibt
/// vorerst erhalten; verbindlich ist die `DbKey`-Liste am Schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DbColumn {
    pub id: String,
    pub name: String,
    pub data_type: DbColumnType,
    pub nullable: bool,
    pub primary_key: bool,
    pub unique: bool,
    #[serde(default)]
    pub generated: ColumnGenerated,
    #[serde(default)]
    pub concurrency_token: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    /// Semantik der Spalte fuer das Audit-Trail (CreatedAt/UpdatedBy/…).
    /// Optional: keiner der bestehenden Designer-Stand muss das setzen,
    /// `None` bleibt das Default-Verhalten.
    #[serde(default)]
    pub audit_role: AuditRole,
}

/// Eine modellierte Tabelle inklusive ihrer Position auf der Leinwand.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DbTable {
    pub id: String,
    pub name: String,
    pub position: Position,
    pub columns: Vec<DbColumn>,
}

/// Kardinalitaet einer Beziehung.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RelationKind {
    OneToOne,
    OneToMany,
    ManyToOne,
    ManyToMany,
}

/// Verhalten beim Loeschen der Principal-Zeile einer Beziehung.
///
/// Spiegel von `Microsoft.EntityFrameworkCore.DeleteBehavior` aus dem
/// importierten C#-Modell. `NoAction` ist DB-seitiges Standardverhalten,
/// `Restrict` der EF-Default fuer optionale Beziehungen.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum DeleteBehavior {
    #[default]
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

/// Ein einzelnes Spaltenpaar einer (potenziell mehrspaltigen) Beziehung.
///
/// `source_column_id` zeigt auf die Spalte der abhaengigen (dependent)
/// Tabelle, `target_column_id` auf die Spalte der Principal-Tabelle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RelationColumnPair {
    pub source_column_id: String,
    pub target_column_id: String,
}

/// Beziehung zwischen zwei Tabellen ueber eine geordnete Liste von
/// Spaltenpaaren.
///
/// `name` entspricht dem DB-Constraint-Namen (`ModelForeignKey.ConstraintName`).
/// `source_table_id` ist die abhaengige (dependent) Tabelle,
/// `target_table_id` die Principal-Tabelle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DbRelation {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub kind: RelationKind,
    #[serde(default)]
    pub on_delete: DeleteBehavior,
    #[serde(default)]
    pub required: bool,
    pub source_table_id: String,
    pub target_table_id: String,
    pub column_pairs: Vec<RelationColumnPair>,
}

/// Schluessel (Primary oder Alternate) einer Tabelle.
///
/// Mehrspaltige Schluessel sind ueber die geordnete `column_ids`-Liste
/// abgebildet (Spiegel von `ModelKeyProperty.Nr` aus dem C#-Modell).
/// Genau ein `DbKey` pro Tabelle darf `is_primary = true` setzen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DbKey {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub table_id: String,
    pub is_primary: bool,
    pub column_ids: Vec<String>,
}

/// Index ueber ein oder mehrere Spalten einer Tabelle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DbIndex {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub table_id: String,
    pub unique: bool,
    pub column_ids: Vec<String>,
}

/// Gesamter Designer-Stand. Wird als JSON-Mutation an den Server geschickt.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DbSchema {
    pub id: String,
    pub name: String,
    pub tables: Vec<DbTable>,
    pub relations: Vec<DbRelation>,
    #[serde(default)]
    pub keys: Vec<DbKey>,
    #[serde(default)]
    pub indices: Vec<DbIndex>,
}

/// Antwortobjekt der `saveDbSchema`-Mutation.
///
/// In der aktuellen Ausbaustufe rein quittierend; spaetere Iterationen
/// koennen zusaetzliche Felder (Validierungsfehler, generierte IDs,
/// Diff-Zusammenfassungen) ergaenzen, ohne den Vertrag aufzubrechen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DbSchemaSaveResult {
    pub ok: bool,
    pub message: String,
    pub table_count: u32,
    pub relation_count: u32,
}
