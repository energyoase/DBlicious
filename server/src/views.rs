//! Q0005 Resolver — stapelt Global → Group(*) → User-Layer.
//!
//! Per Property-Key wird Feld-fuer-Feld gemerged: `Some(_)` schlaegt
//! `Some(_)` der darunter liegenden Schicht; `None` = inherit. Group-Stack
//! wird vom Aufrufer in `group.id`-sortierter Reihenfolge geliefert.

use shared::settings::PropertySettings;
use shared::view::{EntityView, ResolvedLayerRef, ViewLayer, ViewPropertyOverride};
use shared::{FilterCriteria, SecurityUser, Sort, Visibility};

pub struct ResolvedView {
    pub entity_type: String,
    pub view_name: String,
    pub properties: Vec<PropertySettings>,
    pub default_filter: Option<FilterCriteria>,
    pub default_sort: Option<Sort>,
    pub default_page_size: Option<u32>,
    pub provenance: Vec<ResolvedLayerRef>,
}

/// Pure Funktion. Group-Stack erwartet stable-sorted nach `group.id`.
pub fn merge_layers(
    base: Option<EntityView>,
    groups: Vec<EntityView>,
    user: Option<EntityView>,
) -> ResolvedView {
    let entity_type = base
        .as_ref()
        .or(groups.first())
        .or(user.as_ref())
        .map(|v| v.entity_type.clone())
        .unwrap_or_default();
    let view_name = base
        .as_ref()
        .or(groups.first())
        .or(user.as_ref())
        .map(|v| v.view_name.clone())
        .unwrap_or_default();

    let mut accum: std::collections::BTreeMap<String, ViewPropertyOverride> =
        std::collections::BTreeMap::new();
    let mut default_filter: Option<FilterCriteria> = None;
    let mut default_sort: Option<Sort> = None;
    let mut default_page_size: Option<u32> = None;
    let mut provenance: Vec<ResolvedLayerRef> = Vec::new();

    let push_layer = |accum: &mut std::collections::BTreeMap<String, ViewPropertyOverride>,
                      v: &EntityView| {
        for ov in &v.properties {
            let entry = accum
                .entry(ov.key.clone())
                .or_insert_with(|| ViewPropertyOverride {
                    key: ov.key.clone(),
                    ..Default::default()
                });
            if ov.visibility.is_some() {
                entry.visibility = ov.visibility;
            }
            if ov.order.is_some() {
                entry.order = ov.order;
            }
            if ov.min_width.is_some() {
                entry.min_width = ov.min_width;
            }
            if ov.label_override_key.is_some() {
                entry.label_override_key = ov.label_override_key.clone();
            }
            if ov.sortable.is_some() {
                entry.sortable = ov.sortable;
            }
            if ov.filter_id_override.is_some() {
                entry.filter_id_override = ov.filter_id_override.clone();
            }
            if ov.formatter_id_override.is_some() {
                entry.formatter_id_override = ov.formatter_id_override.clone();
            }
        }
    };

    if let Some(b) = &base {
        push_layer(&mut accum, b);
        default_filter = b.default_filter.clone();
        default_sort = b.default_sort.clone();
        default_page_size = b.default_page_size;
        provenance.push(ResolvedLayerRef {
            layer: ViewLayer::Global,
            view_id: b.id.clone(),
            owner_id: b.owner_id.clone(),
            version: b.version,
        });
    }
    for g in &groups {
        push_layer(&mut accum, g);
        if g.default_filter.is_some() {
            default_filter = g.default_filter.clone();
        }
        if g.default_sort.is_some() {
            default_sort = g.default_sort.clone();
        }
        if g.default_page_size.is_some() {
            default_page_size = g.default_page_size;
        }
        provenance.push(ResolvedLayerRef {
            layer: ViewLayer::Group,
            view_id: g.id.clone(),
            owner_id: g.owner_id.clone(),
            version: g.version,
        });
    }
    if let Some(u) = &user {
        push_layer(&mut accum, u);
        if u.default_filter.is_some() {
            default_filter = u.default_filter.clone();
        }
        if u.default_sort.is_some() {
            default_sort = u.default_sort.clone();
        }
        if u.default_page_size.is_some() {
            default_page_size = u.default_page_size;
        }
        provenance.push(ResolvedLayerRef {
            layer: ViewLayer::User,
            view_id: u.id.clone(),
            owner_id: u.owner_id.clone(),
            version: u.version,
        });
    }

    // Konvertiere die akkumulierten Overrides in vollstaendige `PropertySettings`.
    // Feldwerte ohne Some(_) bekommen Default-Werte aus PropertySettings.
    let properties: Vec<PropertySettings> = accum
        .into_values()
        .map(|ov| PropertySettings {
            key: ov.key,
            visibility: ov.visibility.unwrap_or(Visibility::Visible),
            access: shared::settings::PropertyAccess::default(),
            load_method: shared::settings::LoadMethod::default(),
            order: ov.order.unwrap_or(i32::MAX),
            label_override_key: ov.label_override_key,
            min_width: ov.min_width,
        })
        .collect();

    ResolvedView {
        entity_type,
        view_name,
        properties,
        default_filter,
        default_sort,
        default_page_size,
        provenance,
    }
}

/// Laedt die drei Layer aus der DB und ruft `merge_layers` auf.
pub async fn resolve_view(
    entity_type: &str,
    view_name: &str,
    user: Option<&SecurityUser>,
) -> ResolvedView {
    use crate::data;
    let global = data::find_entity_view(entity_type, view_name, ViewLayer::Global, None)
        .await
        .unwrap_or(None);

    let mut groups: Vec<EntityView> = Vec::new();
    if let Some(u) = user {
        let mut gids: Vec<&str> = u.group_ids.iter().map(String::as_str).collect();
        gids.sort(); // deterministisch
        for g in gids {
            if let Ok(Some(v)) =
                data::find_entity_view(entity_type, view_name, ViewLayer::Group, Some(g)).await
            {
                groups.push(v);
            }
        }
    }
    let user_v = match user {
        Some(u) => data::find_entity_view(entity_type, view_name, ViewLayer::User, Some(&u.id))
            .await
            .unwrap_or(None),
        None => None,
    };
    merge_layers(global, groups, user_v)
}

/// Filtert Overrides, deren `key` in der gegebenen `ColumnMeta`-Liste nicht
/// vorkommt — E1 aus der Spec. Wird vom GraphQL-Resolver vor der Auslieferung
/// auf die `properties`-Liste angewendet.
pub fn strip_unknown_keys(
    properties: &mut Vec<PropertySettings>,
    known_keys: &[String],
    entity_type: &str,
    view_name: &str,
) {
    let before = properties.len();
    properties.retain(|p| known_keys.iter().any(|k| k == &p.key));
    let dropped = before - properties.len();
    if dropped > 0 {
        tracing::info!(
            target: "server::views",
            "view '{entity_type}/{view_name}' enthielt {dropped} Override(s) fuer unbekannte Spalten"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::view::{EntityView, ViewLayer, ViewPropertyOverride};
    use shared::Visibility;

    fn ov(key: &str) -> ViewPropertyOverride {
        ViewPropertyOverride {
            key: key.into(),
            ..Default::default()
        }
    }

    fn view(
        layer: ViewLayer,
        owner_id: Option<&str>,
        overrides: Vec<ViewPropertyOverride>,
    ) -> EntityView {
        EntityView {
            id: format!("v-{layer:?}-{owner_id:?}"),
            entity_type: "order".into(),
            view_name: "default".into(),
            layer,
            owner_id: owner_id.map(String::from),
            properties: overrides,
            default_filter: None,
            default_sort: None,
            default_page_size: None,
            version: 1,
            updated_at: "2026-05-21T00:00:00Z".into(),
            updated_by: None,
        }
    }

    #[test]
    fn only_global_returns_global_properties() {
        let global = view(
            ViewLayer::Global,
            None,
            vec![ViewPropertyOverride {
                key: "amount".into(),
                visibility: Some(Visibility::Visible),
                order: Some(1),
                ..Default::default()
            }],
        );
        let r = merge_layers(Some(global), Vec::new(), None);
        assert_eq!(r.properties.len(), 1);
        assert_eq!(r.properties[0].order, 1);
    }

    #[test]
    fn user_overrides_group_overrides_global_field_by_field() {
        let global = view(
            ViewLayer::Global,
            None,
            vec![ViewPropertyOverride {
                key: "amount".into(),
                visibility: Some(Visibility::Visible),
                order: Some(1),
                min_width: Some(80),
                ..Default::default()
            }],
        );
        let group = view(
            ViewLayer::Group,
            Some("g-1"),
            vec![ViewPropertyOverride {
                key: "amount".into(),
                order: Some(2),
                ..Default::default()
            }],
        );
        let user = view(
            ViewLayer::User,
            Some("u-1"),
            vec![ViewPropertyOverride {
                key: "amount".into(),
                min_width: Some(120),
                ..Default::default()
            }],
        );
        let r = merge_layers(Some(global), vec![group], Some(user));
        let p = &r.properties[0];
        assert_eq!(p.visibility, Visibility::Visible, "von Global");
        assert_eq!(p.order, 2, "von Group");
        assert_eq!(p.min_width, Some(120), "von User");
    }

    #[test]
    fn two_groups_are_merged_in_id_sort_order() {
        let global = view(
            ViewLayer::Global,
            None,
            vec![ViewPropertyOverride {
                key: "amount".into(),
                order: Some(0),
                ..Default::default()
            }],
        );
        let group_a = view(
            ViewLayer::Group,
            Some("g-a"),
            vec![ViewPropertyOverride {
                key: "amount".into(),
                order: Some(10),
                ..Default::default()
            }],
        );
        let group_b = view(
            ViewLayer::Group,
            Some("g-b"),
            vec![ViewPropertyOverride {
                key: "amount".into(),
                order: Some(20),
                ..Default::default()
            }],
        );
        let r = merge_layers(Some(global), vec![group_a, group_b], None);
        assert_eq!(r.properties[0].order, 20, "g-b > g-a > global");
    }

    #[test]
    fn sparse_overrides_inherit_unset_fields() {
        let global = view(
            ViewLayer::Global,
            None,
            vec![ViewPropertyOverride {
                key: "amount".into(),
                visibility: Some(Visibility::Visible),
                order: Some(1),
                min_width: Some(80),
                label_override_key: Some("col.amount".into()),
                sortable: Some(true),
                filter_id_override: Some("range".into()),
                formatter_id_override: Some("money-symbol".into()),
            }],
        );
        let user = view(ViewLayer::User, Some("u-1"), vec![ov("amount")]);
        let r = merge_layers(Some(global), Vec::new(), Some(user));
        let p = &r.properties[0];
        assert_eq!(p.order, 1);
        assert_eq!(p.min_width, Some(80));
        assert_eq!(p.label_override_key, Some("col.amount".into()));
    }

    #[test]
    fn no_layers_yields_empty_resolved() {
        let r = merge_layers(None, Vec::new(), None);
        assert_eq!(r.properties.len(), 0);
        assert!(r.default_filter.is_none());
        assert!(r.default_sort.is_none());
        assert!(r.default_page_size.is_none());
    }
}
