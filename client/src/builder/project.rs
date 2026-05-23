//! Projektion `UiTree` → `Vec<ColumnMeta>` (Phase 1.3).
//!
//! Die Bruecke zwischen Builder-State und der generischen Tabelle. Aus
//! jedem `UiNode` mit gesetztem [`BoundField`] entsteht ein
//! [`shared::ColumnMeta`]; nicht-gebundene Knoten (Layout-Container, freie
//! Beschriftungen, …) sind in dieser Projektion irrelevant.
//!
//! Konventionen:
//!   - Reihenfolge folgt der DFS-Pre-Order-Traversierung des Trees.
//!   - Duplikate (`key` mehrfach gebunden) werden silently dedupliziert,
//!     der erste Treffer gewinnt. Mehrfach-Bindings sind im Designer
//!     ausdruecklich erlaubt (z.B. ein Feld in zwei Layout-Karten), die
//!     Tabelle hat aber nur Platz fuer eine Spalte pro Key.
//!   - Defaults fuer `label_key`/`sortable`/`filterable` orientieren sich
//!     am [`FieldType`]: Skalare sind sortier-/filterbar, Reference und
//!     Collection nicht.

use std::collections::HashSet;

use shared::{ColumnMeta, FieldType};

use super::node::{BoundField, UiNode};
use super::tree::UiTree;

/// Projiziert den Tree in die Spalten-Metadaten, die [`crate::components::table::EntityTable`]
/// erwartet.
pub fn project_columns(tree: &UiTree) -> Vec<ColumnMeta> {
    let mut out: Vec<ColumnMeta> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for node in tree.walk() {
        let Some(bound) = &node.bound_field else {
            continue;
        };
        if !seen.insert(bound.key.clone()) {
            continue;
        }
        out.push(column_from(bound));
    }
    out
}

/// Projiziert genau einen `UiNode` (inkl. Kinder) — wird in Phase 1.5 vom
/// Live-Preview verwendet, wenn nur ein Sub-Tree betrachtet werden soll.
pub fn project_columns_from_node(node: &UiNode) -> Vec<ColumnMeta> {
    let mut out: Vec<ColumnMeta> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for n in node.walk() {
        let Some(bound) = &n.bound_field else {
            continue;
        };
        if !seen.insert(bound.key.clone()) {
            continue;
        }
        out.push(column_from(bound));
    }
    out
}

fn column_from(bound: &BoundField) -> ColumnMeta {
    let field_type = bound.field_type.clone().unwrap_or(FieldType::Text);
    let label_key = bound
        .label_key
        .clone()
        .unwrap_or_else(|| format!("column.{}", bound.key));
    let sortable = bound
        .sortable
        .unwrap_or_else(|| default_sortable(&field_type));
    let filterable = bound
        .filterable
        .unwrap_or_else(|| default_filterable(&field_type));
    ColumnMeta {
        key: bound.key.clone(),
        label_key,
        field_type,
        sortable,
        filterable,
        comparator_id: None,
        filter_id: None,
        // Phase 1.5: Implementations-IDs sind in der Projektion leer —
        // BoundField bekommt entsprechende Override-Felder, sobald der
        // Client UI fuer die Implementation-Wahl liefert (1.5.4/1.5.5).
        editor_id: None,
        formatter_id: None,
        action_ids: Vec::new(),
    }
}

/// Default-Sortierbarkeit pro [`FieldType`].
///
/// Skalare lassen sich ordnen; `Reference`/`Collection` nicht — die
/// Tabelle kennt keine semantische Ordnung auf gerichteten Referenzen
/// und benoetigt fuer eine sinnvolle Sortierung die referenzierten
/// Entities, die in der Spalte nicht vorliegen.
pub fn default_sortable(ft: &FieldType) -> bool {
    ft.is_scalar()
}

/// Default-Filterbarkeit pro [`FieldType`].
pub fn default_filterable(ft: &FieldType) -> bool {
    ft.is_scalar()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::node::{NodeId, UiNode};

    fn bound(node_id: u64, key: &str, ft: Option<FieldType>) -> UiNode {
        UiNode {
            id: NodeId(node_id),
            bound_field: Some(BoundField {
                key: key.into(),
                field_type: ft,
                label_key: None,
                sortable: None,
                filterable: None,
            }),
            ..UiNode::new(NodeId(node_id))
        }
    }

    #[test]
    fn empty_tree_projects_to_empty_vec() {
        let cols = project_columns(&UiTree::empty());
        assert!(cols.is_empty());
    }

    #[test]
    fn bound_node_projects_to_single_column() {
        let mut t = UiTree::empty();
        t.push_root(bound(1, "name", None));
        let cols = project_columns(&t);
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].key, "name");
        assert_eq!(cols[0].label_key, "column.name");
        assert!(matches!(cols[0].field_type, FieldType::Text));
        assert!(cols[0].sortable);
        assert!(cols[0].filterable);
    }

    #[test]
    fn nodes_without_binding_are_ignored() {
        let mut t = UiTree::empty();
        t.push_root(UiNode::new(NodeId(1))); // kein bound_field
        t.push_root(bound(2, "price", Some(FieldType::Decimal { precision: 2 })));
        let cols = project_columns(&t);
        assert_eq!(
            cols.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
            vec!["price"]
        );
    }

    #[test]
    fn duplicate_keys_first_wins() {
        let mut t = UiTree::empty();
        t.push_root(bound(1, "price", Some(FieldType::Integer)));
        t.push_root(bound(2, "price", Some(FieldType::Decimal { precision: 2 })));
        let cols = project_columns(&t);
        assert_eq!(cols.len(), 1);
        assert!(matches!(cols[0].field_type, FieldType::Integer));
    }

    #[test]
    fn preorder_dfs_determines_column_order() {
        let mut t = UiTree::empty();
        let root_id = NodeId(1);
        let mut root = UiNode::new(root_id);
        root.children.push(bound(2, "a", None));
        let mut nested = UiNode::new(NodeId(3));
        nested.children.push(bound(4, "b", None));
        root.children.push(nested);
        root.children.push(bound(5, "c", None));
        t.push_root(root);
        let cols: Vec<_> = project_columns(&t).into_iter().map(|c| c.key).collect();
        assert_eq!(cols, vec!["a", "b", "c"]);
    }

    #[test]
    fn reference_and_collection_default_to_non_sortable() {
        let mut t = UiTree::empty();
        t.push_root(bound(
            1,
            "owner",
            Some(FieldType::Reference {
                entity: "user".into(),
            }),
        ));
        t.push_root(bound(
            2,
            "tags",
            Some(FieldType::Collection {
                entity: "tag".into(),
            }),
        ));
        let cols = project_columns(&t);
        assert!(!cols[0].sortable && !cols[0].filterable);
        assert!(!cols[1].sortable && !cols[1].filterable);
    }

    #[test]
    fn explicit_overrides_win_over_defaults() {
        let mut t = UiTree::empty();
        let bf = BoundField {
            key: "price".into(),
            field_type: Some(FieldType::Decimal { precision: 2 }),
            label_key: Some("custom.label".into()),
            sortable: Some(false),
            filterable: Some(false),
        };
        t.push_root(UiNode {
            id: NodeId(1),
            bound_field: Some(bf),
            ..UiNode::new(NodeId(1))
        });
        let cols = project_columns(&t);
        assert_eq!(cols[0].label_key, "custom.label");
        assert!(!cols[0].sortable);
        assert!(!cols[0].filterable);
    }
}
