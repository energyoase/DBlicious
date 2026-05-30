//! Client-seitige Resolution-Kette (Phase 1.5.4).
//!
//! Bestimmt die wirksame Implementations-ID fuer eine Spalte+Registry
//! analog zur Server-Logik in `data::resolve_implementation`. Der Server
//! hat die hoehere Autoritaet (Per-User-Choice + Permission-Gates); die
//! Client-Resolution dient als **Fallback**, wenn der Server keine ID
//! liefert.
//!
//! Reihenfolge:
//!   1. `ColumnMeta.{filter,editor,formatter}_id` (Server-Pflicht).
//!   2. `EntitySettings.field_type_defaults[<kind_str>].{...}_id`.
//!   3. Client-Hardcoded-Fallback pro `FieldType`.

use shared::{ColumnMeta, EntitySettings, FieldType};

/// Wirksame ID fuer `(column, registry)`. `registry` ist `"filter"`,
/// `"editor"` oder `"formatter"`. Liefert `None`, wenn kein Default
/// existiert — der Aufrufer kann dann den eingebauten `FieldCell`/
/// `FieldEditor`-Pfad ohne Registry benutzen.
pub fn resolve_implementation_id(
    column: &ColumnMeta,
    settings: Option<&EntitySettings>,
    registry: &str,
) -> Option<String> {
    // 1) Server-Pflicht pro Spalte.
    let column_override = match registry {
        "filter" => column.filter_id.clone(),
        "editor" => column.editor_id.clone(),
        "formatter" => column.formatter_id.clone(),
        "validator" => column.validator_id.clone(),
        _ => None,
    };
    if column_override.is_some() {
        return column_override;
    }

    // 2) FieldType-Defaults aus EntitySettings.
    if let Some(s) = settings {
        let kind = column.field_type.kind_str();
        if let Some(defaults) = s.field_type_defaults.get(kind) {
            let from_settings = match registry {
                "filter" => defaults.filter_id.clone(),
                "editor" => defaults.editor_id.clone(),
                "formatter" => defaults.formatter_id.clone(),
                "validator" => defaults.validator_id.clone(),
                _ => None,
            };
            if from_settings.is_some() {
                return from_settings;
            }
        }
    }

    // 3) Client-Fallback pro FieldType.
    client_fallback(&column.field_type, registry)
}

fn client_fallback(ft: &FieldType, registry: &str) -> Option<String> {
    let id = match (registry, ft) {
        ("editor", FieldType::Text) => "text-input",
        ("editor", FieldType::Integer) => "number-input",
        ("editor", FieldType::Decimal { .. }) => "number-input",
        ("editor", FieldType::Money { .. }) => "number-input",
        ("editor", FieldType::Boolean) => "checkbox",
        ("editor", FieldType::Date) => "date-picker",
        ("editor", FieldType::DateTime) => "date-picker",
        ("editor", FieldType::Enum { .. }) => "text-input", // kein dedicated select-default
        ("editor", _) => return None,                       // Reference/Collection: kein Default
        ("formatter", FieldType::Text) => "text",
        ("formatter", FieldType::Integer) => "integer",
        ("formatter", FieldType::Decimal { .. }) => "decimal-2",
        ("formatter", FieldType::Money { .. }) => "money",
        ("formatter", FieldType::Boolean) => "boolean",
        ("formatter", FieldType::Date) => "date",
        ("formatter", FieldType::DateTime) => "datetime",
        ("formatter", FieldType::Enum { .. }) => "text",
        ("formatter", _) => return None,
        ("filter", FieldType::Text) => "text-contains",
        ("filter", FieldType::Integer) => "number-range",
        ("filter", FieldType::Decimal { .. }) => "number-range",
        ("filter", FieldType::Money { .. }) => "number-range",
        ("filter", FieldType::Boolean) => "bool-equals",
        ("filter", FieldType::Date) => "date-range",
        ("filter", FieldType::DateTime) => "date-range",
        ("filter", FieldType::Enum { .. }) => "enum-in",
        _ => return None,
    };
    Some(id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::FieldType;

    fn col(field_type: FieldType) -> ColumnMeta {
        ColumnMeta {
            key: "k".into(),
            label_key: "k".into(),
            field_type,
            sortable: true,
            filterable: true,
            comparator_id: None,
            filter_id: None,
            editor_id: None,
            formatter_id: None,
            validator_id: None,
            action_ids: Vec::new(),
        }
    }

    #[test]
    fn column_override_wins() {
        let mut c = col(FieldType::Text);
        c.editor_id = Some("custom-editor".into());
        assert_eq!(
            resolve_implementation_id(&c, None, "editor"),
            Some("custom-editor".to_string())
        );
    }

    #[test]
    fn field_type_default_used_when_no_column_override() {
        let c = col(FieldType::Money {
            currency_code_field: None,
        });
        let mut settings = EntitySettings::default();
        let defaults = shared::FieldTypeDefaults {
            editor_id: Some("money-editor".into()),
            ..Default::default()
        };
        settings
            .field_type_defaults
            .insert("money".into(), defaults);
        assert_eq!(
            resolve_implementation_id(&c, Some(&settings), "editor"),
            Some("money-editor".to_string())
        );
    }

    #[test]
    fn client_fallback_text_field() {
        let c = col(FieldType::Text);
        assert_eq!(
            resolve_implementation_id(&c, None, "editor"),
            Some("text-input".to_string())
        );
        assert_eq!(
            resolve_implementation_id(&c, None, "formatter"),
            Some("text".to_string())
        );
        assert_eq!(
            resolve_implementation_id(&c, None, "filter"),
            Some("text-contains".to_string())
        );
    }

    #[test]
    fn validator_column_override_wins() {
        let mut c = col(FieldType::Money {
            currency_code_field: None,
        });
        c.validator_id = Some("script:d2v_balance_validator".into());
        assert_eq!(
            resolve_implementation_id(&c, None, "validator"),
            Some("script:d2v_balance_validator".to_string())
        );
    }

    #[test]
    fn validator_field_type_default_used() {
        let c = col(FieldType::Money {
            currency_code_field: None,
        });
        let mut settings = EntitySettings::default();
        let defaults = shared::FieldTypeDefaults {
            validator_id: Some("script:d2v_balance_validator".into()),
            ..Default::default()
        };
        settings
            .field_type_defaults
            .insert("money".into(), defaults);
        assert_eq!(
            resolve_implementation_id(&c, Some(&settings), "validator"),
            Some("script:d2v_balance_validator".to_string())
        );
    }

    #[test]
    fn validator_has_no_client_fallback() {
        let c = col(FieldType::Text);
        assert!(resolve_implementation_id(&c, None, "validator").is_none());
    }

    #[test]
    fn no_fallback_for_reference_editor() {
        let c = col(FieldType::Reference {
            entity: "tag".into(),
        });
        assert!(resolve_implementation_id(&c, None, "editor").is_none());
        assert!(resolve_implementation_id(&c, None, "formatter").is_none());
    }
}
