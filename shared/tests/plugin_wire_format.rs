//! Wire-Format-Tests fuer das Plugin-Manifest (Phase 2.2).

use serde_json::json;
use shared::plugin::{
    BeforeSaveInput, BeforeSaveOutput, Capabilities, Compatibility, FunctionDef, PluginError,
    PluginManifest, PluginResponse, TriggerKind, ValidateInput, ValidationErrorFromPlugin,
    ValidationFromPlugin,
};

#[test]
fn manifest_minimal_roundtrip() {
    let m = PluginManifest {
        id: "com.example.slug".into(),
        version: "1.0.0".into(),
        api_version: 1,
        compatibility: Compatibility::default(),
        capabilities: Capabilities::default(),
        functions: Default::default(),
        signing: None,
    };
    let s = serde_json::to_string(&m).unwrap();
    let back: PluginManifest = serde_json::from_str(&s).unwrap();
    assert_eq!(m, back);
}

#[test]
fn trigger_kind_serializes_as_camelcase() {
    for (variant, expected) in [
        (TriggerKind::BeforeSave, "beforeSave"),
        (TriggerKind::AfterSave, "afterSave"),
        (TriggerKind::BeforeDelete, "beforeDelete"),
        (TriggerKind::DeriveField, "deriveField"),
        (TriggerKind::Validate, "validate"),
        (TriggerKind::CustomAction, "customAction"),
    ] {
        let v = serde_json::to_value(variant).unwrap();
        assert_eq!(v, json!(expected));
    }
}

#[test]
fn capabilities_defaults_max_pages_and_runtime() {
    let raw = json!({});
    let caps: Capabilities = serde_json::from_value(raw).unwrap();
    assert_eq!(caps.max_pages, 16);
    assert_eq!(caps.max_runtime_ms, 200);
    assert!(caps.triggers.is_empty());
}

#[test]
fn function_def_omits_optional_fields() {
    let f = FunctionDef {
        trigger: TriggerKind::Validate,
        target_field: None,
        from_field: None,
        entity_types: Vec::new(),
    };
    let v = serde_json::to_value(&f).unwrap();
    assert_eq!(v["trigger"], json!("validate"));
    assert!(v.get("targetField").is_none());
    assert!(v.get("fromField").is_none());
    assert!(v.get("entityTypes").is_none());
}

#[test]
fn before_save_input_carries_user_and_fields() {
    let mut fields = serde_json::Map::new();
    fields.insert("name".into(), json!("Test"));
    let input = BeforeSaveInput {
        entity_type: "product".into(),
        fields_before: None,
        fields_after: fields,
        user: "u-1".into(),
    };
    let v = serde_json::to_value(&input).unwrap();
    assert_eq!(v["entityType"], json!("product"));
    assert_eq!(v["fieldsAfter"]["name"], json!("Test"));
    assert_eq!(v["user"], json!("u-1"));
    assert!(v.get("fieldsBefore").is_none(), "None soll skip_serializing");
}

#[test]
fn before_save_output_can_carry_validation_errors() {
    let out = BeforeSaveOutput {
        fields_after: None,
        validation: Some(ValidationFromPlugin {
            errors: vec![ValidationErrorFromPlugin {
                field: Some("slug".into()),
                code: "slug_collision".into(),
                message: Some("error.slug.collision".into()),
            }],
        }),
    };
    let s = serde_json::to_string(&out).unwrap();
    let back: BeforeSaveOutput = serde_json::from_str(&s).unwrap();
    assert_eq!(out, back);
    let errs = &back.validation.unwrap().errors;
    assert_eq!(errs[0].code, "slug_collision");
}

#[test]
fn validate_input_minimal() {
    let v = ValidateInput {
        entity_type: "customer".into(),
        fields: serde_json::Map::new(),
        user: "u-1".into(),
    };
    let s = serde_json::to_string(&v).unwrap();
    let back: ValidateInput = serde_json::from_str(&s).unwrap();
    assert_eq!(v, back);
}

#[test]
fn plugin_response_data_only() {
    let resp = PluginResponse {
        data: Some(json!({"ok": true})),
        error: None,
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["data"]["ok"], json!(true));
    assert!(v.get("error").is_none());
}

#[test]
fn plugin_response_error_only() {
    let resp: PluginResponse<serde_json::Value> = PluginResponse {
        data: None,
        error: Some(PluginError {
            code: "rate_limited".into(),
            message: "too many calls".into(),
            details: None,
        }),
    };
    let v = serde_json::to_value(&resp).unwrap();
    assert!(v.get("data").is_none());
    assert_eq!(v["error"]["code"], json!("rate_limited"));
}
