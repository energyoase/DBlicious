//! Typen fuer den Visual-Builder (Phase 1).
//!
//! Stand: Phase 1.1/1.2. Dieses Modul haelt die **Wire-Format-Typen** des
//! Builder-Vertrags — `EventTrigger`, `EventKind`, `TriggerTarget`, `GuardExpr`.
//! Die laufzeitigen Tree-Strukturen (`UiTree`, `UiNode`) leben heute in
//! `client/src/builder/`; sollte Phase 1.6 (Persistenz via GraphQL) ergeben,
//! dass auch der Tree als serde-Wire benoetigt wird, kann er hierher
//! gehoben werden, ohne dass dieses Modul aufgebrochen werden muss.
//!
//! Konvention wie bei [`crate::FieldType`]: tagged Enums mit
//! `#[serde(tag = "kind", rename_all = "camelCase")]`. `rename_all` greift
//! dabei nur auf die Variantennamen — innere Felder einer Struct-Variante
//! bleiben snake_case (gepinnt in `shared/tests/builder_wire_format.rs`).

use serde::{Deserialize, Serialize};

pub mod guard;

pub use guard::GuardExpr;

/// Vertrag: was loest in welchem Kontext etwas aus?
///
/// Ein `EventTrigger` haengt am `UiNode` (Client-Builder) oder an einem
/// Entity-Hook (Server, ueber Phase 2 Plugin-Trigger). Die einheitliche
/// Form erlaubt es, denselben Vertrag in beiden Welten zu konsumieren —
/// der Host entscheidet anhand der [`EventKind`]-Variante, ob das Ereignis
/// client- oder serverseitig anliegt.
///
/// `guard` ist optional und liefert eine boolesche Mini-Expression ueber
/// `fields.*` (Parser in [`guard`]-Modul). `debounce_ms` ist UI-only und
/// wird serverseitig ignoriert.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EventTrigger {
    pub event: EventKind,
    pub target: TriggerTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard: Option<GuardExpr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debounce_ms: Option<u32>,
}

/// Art des ausloesenden Ereignisses.
///
/// Greift entweder client- oder serverseitig — der Trigger-Vertrag aus der
/// Roadmap (Architektur-Vertrag Abschnitt 3) ordnet jede Variante einer
/// Runtime zu:
///
/// - Client-seitig: [`EventKind::Click`], [`EventKind::Change`], [`EventKind::Submit`]
/// - Server-seitig: [`EventKind::BeforeSave`], [`EventKind::AfterSave`],
///   [`EventKind::BeforeDelete`]
/// - Beide moeglich: [`EventKind::Custom`] (Runtime im Plugin-Manifest)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EventKind {
    /// Click auf einen UiNode (Client).
    Click,
    /// Wert eines benannten Feldes hat sich geaendert (Client).
    Change { field: String },
    /// Formular-Submit (Client).
    Submit,
    /// Vor dem Persistieren einer Entity (Server-CRUD).
    BeforeSave,
    /// Nach dem Persistieren einer Entity (Server-CRUD, async).
    AfterSave,
    /// Vor dem Loeschen einer Entity (Server-CRUD).
    BeforeDelete,
    /// Beliebiges benutzerdefiniertes Ereignis (`runtime` aus Plugin-Manifest).
    Custom { name: String },
}

/// Ziel eines Triggers — wohin wird das Ereignis geleitet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TriggerTarget {
    /// Aufruf einer Plugin-Funktion (Phase 2). `args` wird dem Plugin als
    /// JSON gereicht.
    Plugin {
        id: String,
        function: String,
        #[serde(default)]
        args: serde_json::Value,
    },
    /// Eingebaute, vom Host bereitgestellte Aktion (z.B. `"save"`, `"reload"`).
    BuiltinAction {
        name: String,
        #[serde(default)]
        args: serde_json::Value,
    },
    /// Navigation zu einer Client-Route.
    Navigate { route: String },
}
