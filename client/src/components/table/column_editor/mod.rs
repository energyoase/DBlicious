//! Q0005 — Pure-Function-Layer fuer den In-Place Column-Editor.

use std::collections::HashMap;
use shared::view::ViewPropertyOverride;
use shared::{ColumnMeta, EntitySettings};
#[cfg(test)]
use shared::Visibility;

/// Wendet pending Edits auf `cols` + `settings` an. Beide werden in-place
/// mutiert; das Rendering liest hinterher die geaenderten Werte.
pub fn apply_pending_overrides(
    cols:      &mut Vec<ColumnMeta>,
    settings:  &mut EntitySettings,
    overrides: &HashMap<String, ViewPropertyOverride>,
) {
    for (key, ov) in overrides {
        // ColumnMeta-Felder (sortable/filter_id/formatter_id) direkt
        if let Some(col) = cols.iter_mut().find(|c| &c.key == key) {
            if let Some(s)  = ov.sortable                                { col.sortable    = s; }
            if let Some(id) = ov.filter_id_override.as_deref()           { col.filter_id    = Some(id.into()); }
            if let Some(id) = ov.formatter_id_override.as_deref()        { col.formatter_id = Some(id.into()); }
        }
        // PropertySettings-Felder
        let p = settings.ensure_property(key);
        if let Some(v) = ov.visibility                  { p.visibility         = v; }
        if let Some(o) = ov.order                       { p.order              = o; }
        if let Some(w) = ov.min_width                   { p.min_width          = Some(w); }
        if let Some(l) = ov.label_override_key.clone()  { p.label_override_key = Some(l); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(key: &str) -> ColumnMeta {
        ColumnMeta {
            key: key.into(),
            label_key: format!("col.{key}"),
            field_type: shared::FieldType::Text,
            sortable: false, filterable: false,
            comparator_id: None, filter_id: None,
            editor_id: None, formatter_id: None,
            action_ids: Vec::new(),
        }
    }

    #[test]
    fn applies_sortable_override_to_column_meta() {
        let mut cols = vec![col("amount")];
        let mut settings = EntitySettings::default();
        let mut ovs = HashMap::new();
        ovs.insert("amount".into(), ViewPropertyOverride {
            key: "amount".into(),
            sortable: Some(true),
            ..Default::default()
        });
        apply_pending_overrides(&mut cols, &mut settings, &ovs);
        assert!(cols[0].sortable);
    }

    #[test]
    fn applies_visibility_to_settings() {
        let mut cols = vec![col("amount")];
        let mut settings = EntitySettings::default();
        let mut ovs = HashMap::new();
        ovs.insert("amount".into(), ViewPropertyOverride {
            key: "amount".into(),
            visibility: Some(Visibility::Hidden),
            ..Default::default()
        });
        apply_pending_overrides(&mut cols, &mut settings, &ovs);
        assert_eq!(settings.property("amount").unwrap().visibility, Visibility::Hidden);
    }

    #[test]
    fn unknown_keys_are_silently_ignored() {
        let mut cols = vec![col("amount")];
        let mut settings = EntitySettings::default();
        let mut ovs = HashMap::new();
        ovs.insert("foreign".into(), ViewPropertyOverride {
            key: "foreign".into(),
            sortable: Some(true),
            ..Default::default()
        });
        apply_pending_overrides(&mut cols, &mut settings, &ovs);
        assert!(!cols[0].sortable, "amount unangetastet");
        // PropertySettings wird angelegt — das ist okay, der Renderer filtert
        // sowieso auf cols.
        assert!(settings.property("foreign").is_some());
    }
}

/// Header-Bounding-Box im Viewport (px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeaderRect {
    pub left:   f64,
    pub right:  f64,
}

#[derive(Debug, Clone, Copy)]
pub struct DragState {
    pub from_index: usize,
    pub pointer_x:  f64,
}

/// Berechnet die neue Spalten-Reihenfolge als Indexliste. Stable.
///
/// Inputs:
///   - `headers`: aktuelle Header-Rects in Render-Reihenfolge
///   - `drag`:    welche Spalte gerade gezogen wird + aktuelle Maus-X
///
/// Output: Vec<usize> der gleichen Laenge wie `headers`, jeweils Index in
/// der urspruenglichen Liste. Beispiel: [0,1,2] -> dragged 0 nach Mitte
/// von 2 -> [1,2,0].
pub fn compute_reorder(headers: &[HeaderRect], drag: &DragState) -> Vec<usize> {
    let n = headers.len();
    if n == 0 || drag.from_index >= n { return (0..n).collect(); }
    // Drop-Zone: ueber welchem Header ist die Maus?
    let to_index = headers.iter().enumerate()
        .find(|(_, r)| drag.pointer_x >= r.left && drag.pointer_x < r.right)
        .map(|(i, _)| i)
        .unwrap_or(if drag.pointer_x < headers[0].left { 0 } else { n - 1 });
    let mut idx: Vec<usize> = (0..n).collect();
    let item = idx.remove(drag.from_index);
    let insert = to_index.min(idx.len());
    idx.insert(insert, item);
    idx
}

#[cfg(test)]
mod reorder_tests {
    use super::*;
    fn r(l: f64) -> HeaderRect { HeaderRect { left: l, right: l + 100.0 } }

    #[test]
    fn drag_first_to_third_swaps_to_back() {
        let h = vec![r(0.0), r(100.0), r(200.0)];
        let ds = DragState { from_index: 0, pointer_x: 250.0 };
        assert_eq!(compute_reorder(&h, &ds), vec![1, 2, 0]);
    }

    #[test]
    fn drag_last_to_first_swaps_to_front() {
        let h = vec![r(0.0), r(100.0), r(200.0)];
        let ds = DragState { from_index: 2, pointer_x: 50.0 };
        assert_eq!(compute_reorder(&h, &ds), vec![2, 0, 1]);
    }

    #[test]
    fn drag_onto_self_is_noop() {
        let h = vec![r(0.0), r(100.0), r(200.0)];
        let ds = DragState { from_index: 1, pointer_x: 150.0 };
        assert_eq!(compute_reorder(&h, &ds), vec![0, 1, 2]);
    }

    #[test]
    fn pointer_left_of_all_drops_at_index_0() {
        let h = vec![r(0.0), r(100.0)];
        let ds = DragState { from_index: 1, pointer_x: -50.0 };
        assert_eq!(compute_reorder(&h, &ds), vec![1, 0]);
    }
}
