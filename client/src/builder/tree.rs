//! `UiTree` — In-Memory-Datenmodell des Visual-Builders (Phase 1.1).
//!
//! Der Tree haelt seine Knoten flach als `Vec<UiNode>` auf der Top-Ebene
//! (Multi-Root). Tiefere Ebenen liegen in `UiNode.children`. Diese Form ist
//! bewusst gewaehlt:
//!
//! - **Codegen-Naehe (Phase 4)**: ein einzelner `UiNode` uebersetzt sich in
//!   ein Leptos-Component-View; die Wurzel-`Vec` ist die Composer-Schicht.
//! - **Performance-Profil**: Designer-Sessions bewegen sich im niedrigen
//!   3-stelligen Bereich an Knoten. Eine `Vec`-Iteration ist hier billiger
//!   als ein indexierter Apparat. Sollte das Profil drehen, kann eine
//!   `HashMap<NodeId, UiNode>` parallel gepflegt werden, ohne den Wire-
//!   Vertrag aufzubrechen (siehe Roadmap-Risiken zu Phase 1).
//!
//! Mutation via Leptos-`RwSignal<UiTree>` — siehe [`UiTreeContext`] in
//! [`super::mod_rs`-Doku](super) fuer den Provider/Subscriber-Pfad.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use super::node::{NodeId, UiNode};

/// Aktuell unterstuetzte Tree-Schema-Version.
///
/// Wird bei jedem Save in `entity_designs` mitgeschrieben (Phase 1.6) und
/// erlaubt dem Loader, alte Stände durch eine Migrationsfunktion zu fuehren.
/// Erhoehen, sobald sich die Form von [`UiNode`] aendert.
pub const TREE_SCHEMA_VERSION: u32 = 1;

/// Wurzel des Builder-States.
///
/// `next_id` ist persistenter Bestandteil des Trees, damit nach einem
/// Save/Load keine ID-Kollisionen entstehen.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UiTree {
    #[serde(default)]
    pub nodes: Vec<UiNode>,
    #[serde(default)]
    pub next_id: u64,
}

impl UiTree {
    /// Leerer Tree mit `next_id = 1`.
    pub fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            next_id: 1,
        }
    }

    /// Reserviert die naechste freie [`NodeId`] und erhoeht den Zaehler.
    pub fn allocate_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id.max(1));
        self.next_id = id.0 + 1;
        id
    }

    /// Iteriert ueber **alle** Knoten (DFS, Pre-Order) ueber alle Root-Knoten.
    pub fn walk(&self) -> impl Iterator<Item = &UiNode> {
        self.nodes.iter().flat_map(|n| n.walk())
    }

    /// Sucht einen Knoten anhand seiner ID im gesamten Tree.
    pub fn find(&self, id: NodeId) -> Option<&UiNode> {
        self.walk().find(|n| n.id == id)
    }

    /// Sucht einen mutablen Knoten anhand seiner ID.
    pub fn find_mut(&mut self, id: NodeId) -> Option<&mut UiNode> {
        for root in &mut self.nodes {
            if let Some(found) = root.find_mut(id) {
                return Some(found);
            }
        }
        None
    }

    /// Fuegt einen Knoten als neuen Root ein.
    pub fn push_root(&mut self, node: UiNode) {
        self.nodes.push(node);
    }

    /// Fuegt einen Knoten als Kind eines existierenden Knotens ein.
    /// Liefert `false`, wenn die Eltern-ID nicht existiert.
    pub fn push_child(&mut self, parent: NodeId, node: UiNode) -> bool {
        match self.find_mut(parent) {
            Some(p) => {
                p.children.push(node);
                true
            }
            None => false,
        }
    }

    /// Entfernt einen Knoten aus dem Tree (egal auf welcher Ebene).
    /// Liefert den entfernten Knoten oder `None`, wenn die ID unbekannt war.
    pub fn remove(&mut self, id: NodeId) -> Option<UiNode> {
        if let Some(pos) = self.nodes.iter().position(|n| n.id == id) {
            return Some(self.nodes.remove(pos));
        }
        for root in &mut self.nodes {
            if let Some(removed) = remove_from(root, id) {
                return Some(removed);
            }
        }
        None
    }

    /// Anzahl aller Knoten (rekursiv).
    pub fn len(&self) -> usize {
        self.walk().count()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

fn remove_from(parent: &mut UiNode, id: NodeId) -> Option<UiNode> {
    if let Some(pos) = parent.children.iter().position(|n| n.id == id) {
        return Some(parent.children.remove(pos));
    }
    for child in &mut parent.children {
        if let Some(removed) = remove_from(child, id) {
            return Some(removed);
        }
    }
    None
}

/// Reaktiver Wrapper um [`UiTree`]. Leptos-Signal, das von Builder-
/// Komponenten geteilt wird.
///
/// `Copy` (Leptos-Signals sind Cheap-To-Copy Handles), damit der Context-
/// Provider-/Subscriber-Pfad nicht in Lifetime-Verrenkungen kippt.
#[derive(Clone, Copy)]
pub struct UiTreeSignal {
    pub tree: RwSignal<UiTree>,
}

impl UiTreeSignal {
    /// Erstellt einen frischen Signal-Wrapper um einen leeren Tree.
    pub fn new() -> Self {
        Self {
            tree: RwSignal::new(UiTree::empty()),
        }
    }

    /// Erstellt einen Wrapper aus einem bestehenden Tree (z.B. von Server).
    pub fn from_tree(tree: UiTree) -> Self {
        Self {
            tree: RwSignal::new(tree),
        }
    }

    /// Bequemer Mutations-Pfad: Closure bekommt eine `&mut UiTree`,
    /// das Signal wird danach als geaendert markiert.
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut UiTree),
    {
        self.tree.update(f);
    }
}

impl Default for UiTreeSignal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::node::UiNode;

    #[test]
    fn empty_tree_allocates_monotonic_ids() {
        let mut t = UiTree::empty();
        let a = t.allocate_id();
        let b = t.allocate_id();
        let c = t.allocate_id();
        assert_eq!(a.0, 1);
        assert_eq!(b.0, 2);
        assert_eq!(c.0, 3);
    }

    #[test]
    fn push_and_find_work_across_levels() {
        let mut t = UiTree::empty();
        let root_id = t.allocate_id();
        let child_id = t.allocate_id();
        t.push_root(UiNode::new(root_id));
        assert!(t.push_child(root_id, UiNode::new(child_id)));
        assert_eq!(t.find(child_id).map(|n| n.id), Some(child_id));
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn remove_drops_subtree() {
        let mut t = UiTree::empty();
        let r = t.allocate_id();
        let a = t.allocate_id();
        let b = t.allocate_id();
        t.push_root(UiNode::new(r));
        t.push_child(r, UiNode::new(a));
        t.push_child(a, UiNode::new(b));
        let removed = t.remove(a).expect("a must exist");
        assert_eq!(removed.id, a);
        assert_eq!(removed.children.len(), 1);
        assert!(t.find(a).is_none());
        assert!(t.find(b).is_none(), "b war Kind von a und faellt mit weg");
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn push_child_returns_false_for_unknown_parent() {
        let mut t = UiTree::empty();
        assert!(!t.push_child(NodeId(99), UiNode::new(NodeId(1))));
    }
}
