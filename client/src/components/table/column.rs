//! Spalten-Definitionen fuer die generische Tabelle.
//!
//! In dieser Iteration werden die Spalten clientseitig hartkodiert. Die
//! Funktion `column_set_for(entity_type)` ist absichtlich identisch in der
//! Signatur zur kuenftigen Server-Variante (`fetch_columns(entity_type)`).
//! Der Wechsel erfordert nur den Austausch der Datenquelle in `view.rs`.

use shared::{ColumnMeta, FieldType};

pub type ColumnSet = Vec<ColumnMeta>;

pub fn column_set_for(entity_type: &str) -> ColumnSet {
    match entity_type {
        "product" => vec![
            col("id", "field.id", FieldType::Text, true, false),
            col("name", "field.name", FieldType::Text, true, true),
            col(
                "price",
                "field.price",
                FieldType::Money { currency_code_field: Some("currency".into()) },
                true,
                true,
            ),
            col("inStock", "field.in_stock", FieldType::Boolean, true, true),
            col("createdAt", "field.created_at", FieldType::DateTime, true, false),
            col(
                "category",
                "field.category",
                FieldType::Reference { entity: "category".into() },
                false,
                true,
            ),
            col(
                "tags",
                "field.tags",
                FieldType::Collection { entity: "tag".into() },
                false,
                false,
            ),
        ],
        "order" => vec![
            col("id", "field.id", FieldType::Text, true, false),
            col("orderNumber", "field.order_number", FieldType::Text, true, true),
            col(
                "total",
                "field.total",
                FieldType::Money { currency_code_field: Some("currency".into()) },
                true,
                true,
            ),
            col("placedAt", "field.placed_at", FieldType::DateTime, true, false),
            col(
                "status",
                "field.status",
                FieldType::Enum {
                    values: vec!["new".into(), "paid".into(), "shipped".into(), "cancelled".into()],
                },
                true,
                true,
            ),
            col(
                "customer",
                "field.customer",
                FieldType::Reference { entity: "customer".into() },
                false,
                true,
            ),
        ],
        "customer" => vec![
            col("id", "field.id", FieldType::Text, true, false),
            col("displayName", "field.display_name", FieldType::Text, true, true),
            col("email", "field.email", FieldType::Text, true, true),
            col("memberSince", "field.member_since", FieldType::Date, true, false),
            col("orderCount", "field.order_count", FieldType::Integer, true, true),
        ],
        _ => vec![],
    }
}

fn col(
    key: &str,
    label: &str,
    field_type: FieldType,
    sortable: bool,
    filterable: bool,
) -> ColumnMeta {
    ColumnMeta {
        key: key.into(),
        label_key: label.into(),
        field_type,
        sortable,
        filterable,
        comparator_id: None,
        filter_id: None,
    }
}
