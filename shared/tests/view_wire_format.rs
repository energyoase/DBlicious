//! Pin-Test fuer das Wire-Format der Named-Views (Q0005).
//! Bricht in CI, wenn jemand camelCase/Tag/skip_serializing_if veraendert.

use shared::view::{EntityView, ViewLayer, ViewPropertyOverride};
use shared::Visibility;

#[test]
fn entity_view_global_serializes_to_pinned_camelcase_json() {
    let v = EntityView {
        id: "v-1".into(),
        entity_type: "order".into(),
        view_name: "default".into(),
        layer: ViewLayer::Global,
        owner_id: None,
        properties: vec![ViewPropertyOverride {
            key: "amount".into(),
            visibility: Some(Visibility::Visible),
            order: Some(2),
            min_width: None,
            label_override_key: None,
            sortable: None,
            filter_id_override: None,
            formatter_id_override: None,
        }],
        default_filter: None,
        default_sort: None,
        default_page_size: None,
        version: 1,
        updated_at: "2026-05-21T00:00:00Z".into(),
        updated_by: Some("u-1".into()),
    };
    let s = serde_json::to_string(&v).unwrap();
    let expected = r#"{"id":"v-1","entityType":"order","viewName":"default","layer":"global","ownerId":null,"properties":[{"key":"amount","visibility":"visible","order":2}],"defaultFilter":null,"defaultSort":null,"defaultPageSize":null,"version":1,"updatedAt":"2026-05-21T00:00:00Z","updatedBy":"u-1"}"#;
    assert_eq!(s, expected);
}

#[test]
fn view_layer_serializes_lowercase() {
    assert_eq!(serde_json::to_string(&ViewLayer::Global).unwrap(), "\"global\"");
    assert_eq!(serde_json::to_string(&ViewLayer::Group).unwrap(),  "\"group\"");
    assert_eq!(serde_json::to_string(&ViewLayer::User).unwrap(),   "\"user\"");
}

#[test]
fn empty_overrides_drop_via_skip_serializing_if() {
    let o = ViewPropertyOverride { key: "x".into(), ..Default::default() };
    let s = serde_json::to_string(&o).unwrap();
    assert_eq!(s, r#"{"key":"x"}"#);
}
