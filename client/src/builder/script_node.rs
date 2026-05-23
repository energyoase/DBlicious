//! Builder-Integration fuer `UiNode { kind: NodeKind::Script(_) }` (Q0009 Phase 5.3).
//!
//! Zwei Bausteine:
//!   - `new_script_node(id, placeholder_script_id)` — Konstruktor fuer einen
//!     frischen Skript-Knoten im Canvas. Wird vom Toolbar-Button "Add Script"
//!     in `canvas.rs` aufgerufen.
//!   - `is_script_node(node)` / `script_ref(node)` — Helfer fuer den
//!     Inspector-Panel.
//!
//! Phase-5-Scope: nur Platzierung + Inspect-Surface. Ein vollstaendiger
//! Skript-Editor (Rhai-Quelltext mit Autocomplete, Capability-Editor,
//! UI-Primitive-Picker) ist explizit out-of-scope der Q0009-Spec (§14:
//! "Editor-Experience" verbleibt fuer eine spaetere Phase).

use shared::script::{ScriptId, ScriptNodeRef};

use super::node::{BoundField, NodeId, NodeKind, Style as NodeStyle, Transform, UiNode};

/// Default-Groesse eines Skript-Knotens im Canvas. Etwas hoeher als ein
/// generischer Field-Knoten, damit der "Script: <id>"-Label-Header passt.
pub const SCRIPT_DEFAULT_W: f64 = 240.0;
pub const SCRIPT_DEFAULT_H: f64 = 120.0;

/// Default-Platzhalter, mit dem ein neu gesetzter Skript-Knoten startet.
/// Der Inspector erlaubt dem Author dann, einen echten Skript zu binden.
pub const SCRIPT_PLACEHOLDER_ID: &str = "unbound";

/// Baut einen Skript-Knoten. `at` ist die Top-Left-Position auf der
/// Designer-Leinwand. `script_id` darf `None` sein — dann wird der
/// Placeholder gesetzt.
pub fn new_script_node(id: NodeId, at: (f64, f64), script_id: Option<ScriptId>) -> UiNode {
    UiNode {
        id,
        transform: Transform {
            x: at.0,
            y: at.1,
            w: SCRIPT_DEFAULT_W,
            h: SCRIPT_DEFAULT_H,
        },
        style: NodeStyle {
            token_ref: Some("surface".into()),
        },
        // Skript-Knoten sind nicht an ein Feld gebunden — der Skript-Run
        // emittiert seinen eigenen Subtree.
        bound_field: Option::<BoundField>::None,
        event_trigger: None,
        draggable: true,
        children: Vec::new(),
        kind: NodeKind::Script(ScriptNodeRef {
            script_id: script_id.unwrap_or_else(|| ScriptId(SCRIPT_PLACEHOLDER_ID.into())),
            version_pin: None,
        }),
    }
}

/// Liefert `Some(ref)`, wenn der Knoten ein Skript-Knoten ist.
pub fn script_ref(node: &UiNode) -> Option<&ScriptNodeRef> {
    match &node.kind {
        NodeKind::Script(r) => Some(r),
        _ => None,
    }
}

/// `true`, wenn `node.kind == Script(_)`.
pub fn is_script_node(node: &UiNode) -> bool {
    matches!(node.kind, NodeKind::Script(_))
}

/// `true`, wenn die `ScriptNodeRef` noch den Platzhalter haelt — der
/// Inspector zeigt dann einen Hinweis "Skript binden".
pub fn is_unbound(r: &ScriptNodeRef) -> bool {
    r.script_id.0 == SCRIPT_PLACEHOLDER_ID
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_script_node_sets_kind_script_with_placeholder_id() {
        let n = new_script_node(NodeId(1), (10.0, 20.0), None);
        assert!(is_script_node(&n));
        let r = script_ref(&n).unwrap();
        assert_eq!(r.script_id, ScriptId(SCRIPT_PLACEHOLDER_ID.into()));
        assert!(is_unbound(r));
        assert_eq!(n.transform.w, SCRIPT_DEFAULT_W);
        assert_eq!(n.transform.h, SCRIPT_DEFAULT_H);
    }

    #[test]
    fn new_script_node_uses_explicit_script_id_when_provided() {
        let n = new_script_node(
            NodeId(2),
            (0.0, 0.0),
            Some(ScriptId("dashboard-widget".into())),
        );
        let r = script_ref(&n).unwrap();
        assert_eq!(r.script_id, ScriptId("dashboard-widget".into()));
        assert!(!is_unbound(r));
    }

    #[test]
    fn generic_node_is_not_a_script_node() {
        let n = UiNode::new(NodeId(3));
        assert!(!is_script_node(&n));
        assert!(script_ref(&n).is_none());
    }

    #[test]
    fn new_script_node_serializes_with_kind_script_in_wire_format() {
        let n = new_script_node(NodeId(7), (0.0, 0.0), Some(ScriptId("disc".into())));
        let v = serde_json::to_value(&n).unwrap();
        // Phase-1.2-Vertrag pinned in `node.rs::ui_node_script_variant_serializes_with_kind_script`:
        // kind.type = "script", kind.scriptId = ...
        assert_eq!(v["kind"]["type"], serde_json::json!("script"));
        assert_eq!(v["kind"]["scriptId"], serde_json::json!("disc"));
    }

    #[test]
    fn placeholder_node_round_trips_through_wire_format() {
        // Phase-5-Smoketest: neuer Skript-Knoten -> JSON -> zurueck nach UiTree.
        let mut tree = super::super::tree::UiTree::empty();
        let id = tree.allocate_id();
        tree.push_root(new_script_node(id, (0.0, 0.0), None));
        let s = serde_json::to_string(&tree).unwrap();
        let back: super::super::tree::UiTree = serde_json::from_str(&s).unwrap();
        assert_eq!(back.nodes.len(), 1);
        assert!(is_script_node(&back.nodes[0]));
    }
}
