//! `UiNode` — der Baustein des Builder-Trees (Phase 1.2).
//!
//! Ein `UiNode` beschreibt einen platzierten, optional gebundenen UI-Knoten
//! im Designer. Die Felder spiegeln 1:1 die Roadmap-Spezifikation und sind
//! bewusst so geschnitten, dass die spaetere Phase 4 (Codegen) sie nahezu
//! ohne Indirektion in einen Leptos-Component-View uebersetzen kann:
//!
//! - [`Transform`]: Position und Groesse auf der Designer-Leinwand,
//! - [`Style`]: Verweis auf einen semantischen Design-Token,
//! - [`BoundField`]: optionale Verbindung zu einem Entity-Property,
//! - `event_trigger`: optionaler Ereignis-Vertrag (siehe [`shared::EventTrigger`]),
//! - `draggable`: ob der Knoten im Builder bewegt werden darf,
//! - `children`: rekursive Komposition.
//!
//! Der Tree haelt diese Knoten flach in einer `Vec<UiNode>` (siehe
//! [`super::tree::UiTree`]). Performance-Profil ist „Designer-Session":
//! kleine bis mittlere Mengen (5–500 Knoten). Eine indexierte Variante
//! (`HashMap<NodeId, UiNode>`) ist in der Roadmap als nachgezogener Schritt
//! vorgesehen, sobald reale Designs das brauchen.

use serde::{Deserialize, Serialize};
use shared::{EventTrigger, FieldType};

/// Stabile, kompakte Knoten-ID innerhalb eines Trees.
///
/// `u64` ist heute ausreichend: der Designer vergibt IDs monoton steigend
/// (siehe [`super::tree::UiTree::next_id`]). Wire-Format ist die nackte Zahl
/// (`#[serde(transparent)]`), damit das Roadmap-Beispiel
/// (`"id": 42`) der direkte Vertrag bleibt.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
#[serde(transparent)]
pub struct NodeId(pub u64);

impl NodeId {
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl From<u64> for NodeId {
    fn from(value: u64) -> Self {
        NodeId(value)
    }
}

impl core::fmt::Display for NodeId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Position und Groesse eines Knotens auf der Designer-Leinwand
/// (CSS-Pixel; Origin oben links).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct Transform {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Stil-Referenz eines Knotens.
///
/// Aktuell als reiner Token-Verweis modelliert (`tokenRef`), parallel zum
/// `DesignSystem`-Trait des Clients. Inline-Overrides bleiben absichtlich
/// draussen — der Designer beschreibt *Intent* (welcher semantische Slot),
/// nicht konkretes CSS.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    /// Name eines semantischen Tokens (`"surface"`, `"table_cell"`, …).
    /// `None` bedeutet "keine zugewiesene Style-Klasse".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_ref: Option<String>,
}

/// Bindung an ein Entity-Property.
///
/// `key` referenziert eine Spalte ([`shared::ColumnMeta::key`]).
/// Die uebrigen Felder sind **optionale** Projektions-Overrides fuer
/// Phase 1.3 (`UiTree` → `Vec<ColumnMeta>`): wenn der Nutzer im Designer
/// etwas explizit eingestellt hat, fliesst es so in die `ColumnMeta`.
/// Sonst trifft die Projektion typabhaengige Defaults
/// (`field_type=Text`, `label_key=column.<key>`, `sortable/filterable`
/// abhaengig vom Field-Type).
///
/// Alle Override-Felder serialisieren nur, wenn gesetzt — der minimale
/// Designer-Stand bleibt damit das von Phase 1.2 gepinnte
/// `{"key": "..."}`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BoundField {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_type: Option<FieldType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sortable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filterable: Option<bool>,
}

impl BoundField {
    /// Bequemer Konstruktor fuer den haeufigen Fall "nur ein Key".
    pub fn key(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            field_type: None,
            label_key: None,
            sortable: None,
            filterable: None,
        }
    }
}

/// Spezialisierte Knoten-Variante. Default `Generic` haelt die heutige
/// nicht-getaggte Form (`{"id": 42, ...}`) am Vertrag. Neue Varianten
/// werden additiv ergaenzt — `skip_serializing_if = "NodeKind::is_generic"`
/// laesst die alte Wire-Form unveraendert.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum NodeKind {
    Generic,
    Script(shared::script::ScriptNodeRef),
}

impl Default for NodeKind {
    fn default() -> Self {
        NodeKind::Generic
    }
}

impl NodeKind {
    pub fn is_generic(&self) -> bool {
        matches!(self, NodeKind::Generic)
    }
}

/// Knoten des UI-Trees.
///
/// Die Feldreihenfolge entspricht dem Roadmap-Beispiel-JSON. Optional-Felder
/// werden bei `None` weggelassen (`skip_serializing_if`), damit das Wire-
/// Format kompakt bleibt und Diff-Diffs in `entity_designs` (Phase 1.6)
/// lesbar bleiben.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UiNode {
    pub id: NodeId,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default)]
    pub style: Style,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound_field: Option<BoundField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_trigger: Option<EventTrigger>,
    #[serde(default)]
    pub draggable: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<UiNode>,
    /// Spezialisierte Variante (Q0009). Default ist `NodeKind::Generic`,
    /// damit das bisherige Wire-Format unveraendert bleibt — das Feld
    /// wird dann ueber `skip_serializing_if` weggelassen.
    #[serde(default, skip_serializing_if = "NodeKind::is_generic")]
    pub kind: NodeKind,
}

impl UiNode {
    /// Erstellt einen minimalen Knoten ohne Style/Binding/Trigger.
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            transform: Transform::default(),
            style: Style::default(),
            bound_field: None,
            event_trigger: None,
            draggable: false,
            children: Vec::new(),
            kind: NodeKind::default(),
        }
    }

    /// Iteriert ueber alle Knoten dieses Teilbaums (DFS, self zuerst).
    pub fn walk(&self) -> Walk<'_> {
        Walk { stack: vec![self] }
    }

    /// Sucht einen Knoten anhand seiner ID im gesamten Teilbaum.
    pub fn find(&self, id: NodeId) -> Option<&UiNode> {
        self.walk().find(|n| n.id == id)
    }

    /// Sucht einen mutablen Knoten anhand seiner ID im gesamten Teilbaum.
    pub fn find_mut(&mut self, id: NodeId) -> Option<&mut UiNode> {
        if self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_mut(id) {
                return Some(found);
            }
        }
        None
    }
}

/// Iterator ueber einen Teilbaum (Depth-First, Pre-Order).
pub struct Walk<'a> {
    stack: Vec<&'a UiNode>,
}

impl<'a> Iterator for Walk<'a> {
    type Item = &'a UiNode;
    fn next(&mut self) -> Option<Self::Item> {
        let node = self.stack.pop()?;
        // Kinder in umgekehrter Reihenfolge auf den Stack, damit Pre-Order
        // mit der natuerlichen Reihenfolge der `children`-Liste laeuft.
        for child in node.children.iter().rev() {
            self.stack.push(child);
        }
        Some(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use shared::{EventKind, TriggerTarget};

    #[test]
    fn ui_node_wire_format_matches_roadmap_example() {
        let node = UiNode {
            id: NodeId(42),
            transform: Transform {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 32.0,
            },
            style: Style {
                token_ref: Some("table_cell".into()),
            },
            bound_field: Some(BoundField::key("price")),
            event_trigger: Some(EventTrigger {
                event: EventKind::Click,
                target: TriggerTarget::Plugin {
                    id: "p".into(),
                    function: "f".into(),
                    args: json!({}),
                },
                guard: None,
                debounce_ms: None,
            }),
            draggable: false,
            children: Vec::new(),
            kind: NodeKind::default(),
        };
        let v = serde_json::to_value(&node).unwrap();
        // ID transparent als Zahl, eventTrigger camelCase, children weggelassen.
        assert_eq!(v["id"], json!(42));
        assert_eq!(v["transform"], json!({"x": 0.0, "y": 0.0, "w": 200.0, "h": 32.0}));
        assert_eq!(v["style"], json!({"tokenRef": "table_cell"}));
        assert_eq!(v["boundField"], json!({"key": "price"}));
        assert_eq!(v["eventTrigger"]["event"], json!({"kind": "click"}));
        assert_eq!(v["draggable"], json!(false));
        assert!(v.get("children").is_none(), "leere children weglassen: {v}");
        // Default NodeKind::Generic muss weggelassen werden, damit der
        // alte Wire-Vertrag (`{"id": 42, ...}` ohne `kind`) gilt.
        assert!(v.get("kind").is_none(), "default kind muss weggelassen werden: {v}");
    }

    #[test]
    fn ui_node_script_variant_serializes_with_kind_script() {
        use shared::script::{ScriptId, ScriptNodeRef};
        let mut node = UiNode::new(NodeId(7));
        node.kind = NodeKind::Script(ScriptNodeRef {
            script_id: ScriptId("sales-dashboard".into()),
            version_pin: None,
        });
        let v = serde_json::to_value(&node).unwrap();
        assert_eq!(v["kind"]["type"], json!("script"));
        assert_eq!(v["kind"]["scriptId"], json!("sales-dashboard"));
    }

    #[test]
    fn ui_node_minimal_skips_optional_fields() {
        let node = UiNode::new(NodeId(1));
        let v = serde_json::to_value(&node).unwrap();
        assert!(v.get("boundField").is_none());
        assert!(v.get("eventTrigger").is_none());
        assert!(v.get("children").is_none());
    }

    #[test]
    fn walk_returns_preorder_dfs() {
        let mut root = UiNode::new(NodeId(1));
        let mut a = UiNode::new(NodeId(2));
        a.children.push(UiNode::new(NodeId(4)));
        a.children.push(UiNode::new(NodeId(5)));
        root.children.push(a);
        root.children.push(UiNode::new(NodeId(3)));
        let ids: Vec<u64> = root.walk().map(|n| n.id.0).collect();
        assert_eq!(ids, vec![1, 2, 4, 5, 3]);
    }

    #[test]
    fn find_locates_nested_node() {
        let mut root = UiNode::new(NodeId(1));
        let mut a = UiNode::new(NodeId(2));
        a.children.push(UiNode::new(NodeId(99)));
        root.children.push(a);
        assert_eq!(root.find(NodeId(99)).map(|n| n.id), Some(NodeId(99)));
        assert!(root.find(NodeId(7)).is_none());
    }
}
