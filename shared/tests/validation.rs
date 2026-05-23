//! Tests fuer [`shared::validation`].

use shared::{Severity, ValidationMessage, ValidationResult};

#[test]
fn default_result_is_empty_and_non_blocking() {
    let r = ValidationResult::default();
    assert!(r.is_empty());
    assert!(!r.has_blocking());
}

#[test]
fn warnings_dont_block_but_show_in_messages() {
    let mut r = ValidationResult::default();
    r.push(ValidationMessage::warning("name", "validation.length"));
    assert!(!r.has_blocking());
    assert!(!r.is_empty());
}

#[test]
fn errors_block() {
    let mut r = ValidationResult::default();
    r.push(ValidationMessage::error("name", "validation.required"));
    assert!(r.has_blocking());
}

#[test]
fn for_target_filters_correctly() {
    let mut r = ValidationResult::default();
    r.push(ValidationMessage::error("name", "validation.required"));
    r.push(ValidationMessage::error("email", "validation.email"));
    let nm: Vec<_> = r.for_target("name").collect();
    assert_eq!(nm.len(), 1);
    assert_eq!(nm[0].message_key, "validation.required");
}

#[test]
fn args_attach_to_message() {
    let msg = ValidationMessage::error("name", "validation.min_length").with_arg("min", 5_i64);
    assert_eq!(msg.severity, Severity::Error);
    assert_eq!(msg.args.get("min").and_then(|v| v.as_i64()), Some(5));
}

#[test]
fn entity_level_messages_are_target_none() {
    let mut r = ValidationResult::default();
    r.push(ValidationMessage {
        severity: Severity::Error,
        message_key: "validation.global".into(),
        target: None,
        args: serde_json::Map::new(),
    });
    r.push(ValidationMessage::error("name", "validation.required"));
    let entity: Vec<_> = r.entity_level().collect();
    assert_eq!(entity.len(), 1);
}
