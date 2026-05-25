use shared::{EntityPage, EntitySettings};

#[test]
fn entity_settings_parses_display_field() {
    let s: EntitySettings =
        serde_json::from_value(serde_json::json!({ "displayField": "displayName" })).unwrap();
    assert_eq!(s.display_field.as_deref(), Some("displayName"));
}

#[test]
fn entity_page_reference_labels_roundtrip() {
    let mut labels = std::collections::BTreeMap::new();
    let mut row = std::collections::BTreeMap::new();
    row.insert("customer".to_string(), "Max M.".to_string());
    labels.insert("order-1".to_string(), row);
    let page = EntityPage {
        items: vec![],
        total_count: 0,
        page: 1,
        page_size: 50,
        reference_labels: labels,
    };
    let json = serde_json::to_string(&page).unwrap();
    assert!(json.contains("referenceLabels"));
    let back: EntityPage = serde_json::from_str(&json).unwrap();
    assert_eq!(back.reference_labels["order-1"]["customer"], "Max M.");
}

#[test]
fn entity_page_without_reference_labels_defaults_empty() {
    let page: EntityPage = serde_json::from_value(serde_json::json!({
        "items": [], "totalCount": 0, "page": 1, "pageSize": 50
    }))
    .unwrap();
    assert!(page.reference_labels.is_empty());
}
