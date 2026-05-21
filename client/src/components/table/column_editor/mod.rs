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
