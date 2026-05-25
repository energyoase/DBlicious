//! Reference-Label-Resolution (U1). Pluggbare Strategie; heute nur
//! `ServerEmbed`: beim Servieren einer Entity-Seite das Anzeige-Label der
//! referenzierten Zielzeile (deren `EntitySettings.display_field`) je
//! Reference-Spalte einbetten.
use shared::FieldType;
use std::collections::BTreeMap;

/// Wahl der Resolution-Strategie. Default `ServerEmbed`. `ClientBatch` +
/// `PerCell` sind dokumentierte Folge-Strategien (Seam) — heute nicht gebaut.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReferenceResolutionStrategy {
    #[default]
    ServerEmbed,
    ClientBatch,
    PerCell,
}

/// Liefert {row_id → {col_key → label}} für die gegebenen Zeilen.
/// `fetch_target` liefert (vorab geladen) für eine Ziel-Entity die Map
/// {target_id → fields}; `display_field_of` das display_field einer
/// Ziel-Entity. Beides synchron, weil der Aufrufer das DB-IO vorher erledigt.
pub fn resolve_server_embed(
    rows: &[shared::Entity],
    columns: &[shared::ColumnMeta],
    fetch_target: &mut dyn FnMut(
        &str,
    )
        -> BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
    display_field_of: &dyn Fn(&str) -> Option<String>,
) -> BTreeMap<String, BTreeMap<String, String>> {
    let refs: Vec<(&str, &str)> = columns
        .iter()
        .filter_map(|c| match &c.field_type {
            FieldType::Reference { entity } => Some((c.key.as_str(), entity.as_str())),
            _ => None,
        })
        .collect();
    if refs.is_empty() {
        return BTreeMap::new();
    }
    let mut target_cache: BTreeMap<
        String,
        BTreeMap<String, serde_json::Map<String, serde_json::Value>>,
    > = BTreeMap::new();
    let mut df_cache: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for row in rows {
        for (col, target) in &refs {
            let Some(fk) = row.fields.get(*col).and_then(|v| v.as_str()) else {
                continue;
            };
            let target_rows = target_cache
                .entry((*target).to_string())
                .or_insert_with(|| fetch_target(target));
            let df = df_cache
                .entry((*target).to_string())
                .or_insert_with(|| display_field_of(target));
            let Some(df) = df.as_deref() else { continue };
            let Some(trow) = target_rows.get(fk) else {
                continue;
            };
            let Some(label) = trow.get(df).and_then(|v| v.as_str()) else {
                continue;
            };
            out.entry(row.id.clone())
                .or_default()
                .insert((*col).to_string(), label.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{ColumnMeta, Entity, FieldType};

    fn col(key: &str, ft: FieldType) -> ColumnMeta {
        ColumnMeta {
            key: key.into(),
            label_key: format!("f.{key}"),
            field_type: ft,
            sortable: false,
            filterable: false,
            comparator_id: None,
            filter_id: None,
            editor_id: None,
            formatter_id: None,
            action_ids: vec![],
        }
    }

    fn row(id: &str, k: &str, v: &str) -> Entity {
        let mut m = serde_json::Map::new();
        m.insert(k.into(), serde_json::json!(v));
        Entity {
            id: id.into(),
            fields: m,
        }
    }

    #[test]
    fn embeds_label_via_display_field() {
        let rows = vec![row("o-1", "customer", "c-7")];
        let cols = vec![col(
            "customer",
            FieldType::Reference {
                entity: "customer".into(),
            },
        )];
        let mut targets: BTreeMap<String, serde_json::Map<String, serde_json::Value>> =
            BTreeMap::new();
        let mut cm = serde_json::Map::new();
        cm.insert("displayName".into(), serde_json::json!("Max M."));
        targets.insert("c-7".into(), cm);
        let mut fetch = |_t: &str| targets.clone();
        let df = |_t: &str| Some("displayName".to_string());
        let out = resolve_server_embed(&rows, &cols, &mut fetch, &df);
        assert_eq!(out["o-1"]["customer"], "Max M.");
    }

    #[test]
    fn dangling_target_yields_no_label() {
        let rows = vec![row("o-1", "customer", "c-7")];
        let cols = vec![col(
            "customer",
            FieldType::Reference {
                entity: "customer".into(),
            },
        )];
        let mut fetch = |_t: &str| BTreeMap::new(); // Ziel leer
        let df = |_t: &str| Some("displayName".to_string());
        assert!(resolve_server_embed(&rows, &cols, &mut fetch, &df).is_empty());
    }

    #[test]
    fn no_display_field_yields_no_label() {
        let rows = vec![row("o-1", "customer", "c-7")];
        let cols = vec![col(
            "customer",
            FieldType::Reference {
                entity: "customer".into(),
            },
        )];
        let mut t: BTreeMap<String, serde_json::Map<String, serde_json::Value>> = BTreeMap::new();
        t.insert("c-7".into(), serde_json::Map::new());
        let mut fetch = |_x: &str| t.clone();
        let df = |_t: &str| None;
        assert!(resolve_server_embed(&rows, &cols, &mut fetch, &df).is_empty());
    }
}
