//! Wire-Format-Tests fuer den Builder-Vertrag (Phase 1.1/1.2).
//!
//! Vertrag ist die Roadmap-Spezifikation in `ROADMAP.md` (Abschnitt
//! "Architektur-Vertraege: Builder ↔ Plugin"). Diese Tests pinnen die
//! JSON-Form der relevanten Typen, damit Aenderungen am Tag-/Feldnamen
//! nicht unbemerkt durch beide Seiten (Server, Client) gehen koennen.
//!
//! Konvention wie bei `FieldType`: tagged Enums tragen `kind`, innere
//! Felder einer Struct-Variante bleiben snake_case (siehe
//! `field_type_wire_format.rs`).

use serde_json::json;
use shared::{EventKind, EventTrigger, GuardExpr, TriggerTarget};

#[test]
fn event_trigger_click_navigate_serializes_camelcase() {
    let t = EventTrigger {
        event: EventKind::Click,
        target: TriggerTarget::Navigate {
            route: "/entities/product".into(),
        },
        guard: None,
        debounce_ms: None,
    };
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(
        v,
        json!({
            "event": {"kind": "click"},
            "target": {"kind": "navigate", "route": "/entities/product"},
        })
    );
}

#[test]
fn event_trigger_change_carries_field_name() {
    let t = EventTrigger {
        event: EventKind::Change {
            field: "price".into(),
        },
        target: TriggerTarget::BuiltinAction {
            name: "save".into(),
            args: json!({}),
        },
        guard: None,
        debounce_ms: Some(250),
    };
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(v["event"], json!({"kind": "change", "field": "price"}));
    assert_eq!(
        v["target"],
        json!({"kind": "builtinAction", "name": "save", "args": {}})
    );
    assert_eq!(v["debounceMs"], json!(250));
}

#[test]
fn event_trigger_plugin_target_passes_args_through() {
    let t = EventTrigger {
        event: EventKind::Submit,
        target: TriggerTarget::Plugin {
            id: "com.example.slug".into(),
            function: "slugify".into(),
            args: json!({"from": "name"}),
        },
        guard: None,
        debounce_ms: None,
    };
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(
        v["target"],
        json!({
            "kind": "plugin",
            "id": "com.example.slug",
            "function": "slugify",
            "args": {"from": "name"},
        })
    );
}

#[test]
fn event_trigger_server_kinds_serialize_camelcase() {
    for (kind, expected) in [
        (EventKind::BeforeSave, "beforeSave"),
        (EventKind::AfterSave, "afterSave"),
        (EventKind::BeforeDelete, "beforeDelete"),
    ] {
        let t = EventTrigger {
            event: kind,
            target: TriggerTarget::BuiltinAction {
                name: "noop".into(),
                args: json!({}),
            },
            guard: None,
            debounce_ms: None,
        };
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["event"], json!({"kind": expected}));
    }
}

#[test]
fn event_trigger_custom_carries_name() {
    let t = EventTrigger {
        event: EventKind::Custom {
            name: "shipOrder".into(),
        },
        target: TriggerTarget::BuiltinAction {
            name: "noop".into(),
            args: json!({}),
        },
        guard: None,
        debounce_ms: None,
    };
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(v["event"], json!({"kind": "custom", "name": "shipOrder"}));
}

#[test]
fn event_trigger_optionals_are_omitted_when_none() {
    let t = EventTrigger {
        event: EventKind::Click,
        target: TriggerTarget::Navigate { route: "/".into() },
        guard: None,
        debounce_ms: None,
    };
    let v = serde_json::to_value(&t).unwrap();
    assert!(
        v.get("guard").is_none(),
        "guard sollte weggelassen werden: {v}"
    );
    assert!(
        v.get("debounceMs").is_none(),
        "debounceMs sollte weggelassen werden: {v}"
    );
}

#[test]
fn guard_expr_is_transparent_string() {
    let g = GuardExpr::new("fields.status == \"draft\"");
    let v = serde_json::to_value(&g).unwrap();
    assert_eq!(v, json!("fields.status == \"draft\""));
    let back: GuardExpr = serde_json::from_value(v).unwrap();
    assert_eq!(back, g);
}

#[test]
fn event_trigger_with_guard_roundtrips() {
    let t = EventTrigger {
        event: EventKind::BeforeSave,
        target: TriggerTarget::Plugin {
            id: "p".into(),
            function: "validate".into(),
            args: json!({}),
        },
        guard: Some(GuardExpr::new("fields.price > 0")),
        debounce_ms: None,
    };
    let s = serde_json::to_string(&t).unwrap();
    let back: EventTrigger = serde_json::from_str(&s).unwrap();
    assert_eq!(t, back);
}

#[test]
fn unknown_event_kind_fails_to_deserialize() {
    let r: Result<EventTrigger, _> = serde_json::from_value(json!({
        "event": {"kind": "frobnicated"},
        "target": {"kind": "navigate", "route": "/"}
    }));
    assert!(r.is_err(), "unbekannter event-kind muss Fehler werfen");
}

#[test]
fn guard_expr_parses_and_evaluates_simple_predicate() {
    let g = GuardExpr::new("fields.status == \"draft\"");
    let ast = g.parse().expect("parse ok");
    let fields: serde_json::Map<String, serde_json::Value> = match json!({"status": "draft"}) {
        serde_json::Value::Object(m) => m,
        _ => unreachable!(),
    };
    assert!(ast.evaluate(&fields));
}
