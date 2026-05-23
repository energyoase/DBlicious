//! Pin-Test fuer das Wire-Format der Skript-Sprache (Q0009).
//!
//! Bricht in CI, wenn jemand camelCase/Tag/skip_serializing_if veraendert.
//! Lehnt sich an `field_type_wire_format.rs` an.

use serde_json::json;
use shared::script::{default_tokens_for_tier, CapabilityToken, ScriptTier};

#[test]
fn script_tier_serializes_lowercase() {
    assert_eq!(serde_json::to_value(ScriptTier::Reader).unwrap(), json!("reader"));
    assert_eq!(serde_json::to_value(ScriptTier::Author).unwrap(), json!("author"));
    assert_eq!(serde_json::to_value(ScriptTier::Developer).unwrap(), json!("developer"));
    assert_eq!(serde_json::to_value(ScriptTier::Admin).unwrap(), json!("admin"));
}

#[test]
fn default_tokens_for_reader_is_minimal_set() {
    let toks = default_tokens_for_tier(ScriptTier::Reader);
    assert!(toks.contains(&CapabilityToken::ReadOwnEntities));
    assert!(toks.contains(&CapabilityToken::ReadI18n));
    assert!(toks.contains(&CapabilityToken::ComputeOnly));
    // Reader darf KEIN WriteEntity haben:
    assert!(!toks.iter().any(|t| matches!(t, CapabilityToken::WriteEntity { .. })));
}

#[test]
fn capability_token_simple_variants_use_kind_field() {
    assert_eq!(
        serde_json::to_value(CapabilityToken::ReadOwnEntities).unwrap(),
        json!({"kind": "readOwnEntities"})
    );
    assert_eq!(
        serde_json::to_value(CapabilityToken::ReadI18n).unwrap(),
        json!({"kind": "readI18n"})
    );
    assert_eq!(
        serde_json::to_value(CapabilityToken::ComputeOnly).unwrap(),
        json!({"kind": "computeOnly"})
    );
}

#[test]
fn capability_token_write_entity_keeps_snake_case_inner_field() {
    // wie bei FieldType::Money: inner field bleibt snake_case
    let v = serde_json::to_value(CapabilityToken::WriteEntity { validated: true }).unwrap();
    assert_eq!(v, json!({"kind": "writeEntity", "validated": true}));
}

#[test]
fn capability_token_emit_ui_node_carries_scope() {
    let v = serde_json::to_value(CapabilityToken::EmitUiNode {
        scope: shared::script::UiScope::Composite,
    })
    .unwrap();
    assert_eq!(v, json!({"kind": "emitUiNode", "scope": "composite"}));
}

#[test]
fn capability_token_roundtrips_all_variants() {
    use shared::script::UiScope;
    use CapabilityToken::*;
    let originals = vec![
        ReadOwnEntities,
        ReadAllEntitiesWhereAllowed,
        WriteEntity { validated: true },
        WriteEntity { validated: false },
        ComputeOnly,
        ReadI18n,
        EmitUiNode { scope: UiScope::Leaf },
        EmitUiNode { scope: UiScope::Composite },
        EmitWorkflowAction,
        LoadOtherScript,
        ReadAuditLog { own_only: true },
        ReadAuditLog { own_only: false },
        WriteAuditLog,
        RegisterHostFunction,
        ScheduleJob,
    ];
    for t in originals {
        let s = serde_json::to_string(&t).unwrap();
        let back: CapabilityToken = serde_json::from_str(&s).unwrap();
        assert_eq!(t, back, "CapabilityToken-Roundtrip fehlgeschlagen: {s}");
    }
}

#[test]
fn unknown_capability_kind_fails_to_deserialize() {
    let r: Result<CapabilityToken, _> =
        serde_json::from_value(json!({"kind": "frobnicated"}));
    assert!(r.is_err(), "unbekannter kind muss Fehler werfen");
}

#[test]
fn manifest_serializes_camelcase_with_pinned_fields() {
    use shared::script::{ScriptManifest, UiPrimitive};
    let m = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ReadOwnEntities, CapabilityToken::ReadI18n],
        ui_primitives: vec![UiPrimitive::Text],
        timeout_ms: Some(100),
        memory_kb: None,
        lift_capable: true,
    };
    let v = serde_json::to_value(&m).unwrap();
    assert_eq!(v["manifestVersion"], json!(1));
    assert_eq!(v["tier"], json!("reader"));
    assert_eq!(v["capabilities"][0], json!({"kind": "readOwnEntities"}));
    assert_eq!(v["uiPrimitives"], json!(["text"]));
    assert_eq!(v["timeoutMs"], json!(100));
    assert_eq!(v["liftCapable"], json!(true));
    assert!(
        v.get("memoryKb").is_none(),
        "memoryKb soll weggelassen werden: {v}"
    );
}

#[test]
fn script_kind_provider_serializes_with_slot() {
    use shared::script::{ProviderSlot, ScriptKind};
    let v = serde_json::to_value(ScriptKind::Provider {
        slot: ProviderSlot::Formatter,
    })
    .unwrap();
    assert_eq!(v, json!({"kind": "provider", "slot": "formatter"}));
}

#[test]
fn script_kind_component_serializes_with_entry() {
    use shared::script::ScriptKind;
    let v = serde_json::to_value(ScriptKind::Component {
        entry: "render".into(),
    })
    .unwrap();
    assert_eq!(v, json!({"kind": "component", "entry": "render"}));
}

/// `Wasm`-Variante ist Phase-2-reserviert. Wir pinnen die Diskriminante
/// (Wire-Tag `"wasm"`), damit Phase 2 sie nicht versehentlich aendert.
#[test]
fn script_kind_wasm_variant_is_reserved_and_pinned() {
    use shared::script::ScriptKind;
    let v = serde_json::to_value(ScriptKind::Wasm {
        wasm_bytes: vec![1, 2, 3],
        entry: "main".into(),
    })
    .unwrap();
    assert_eq!(v["kind"], json!("wasm"));
    assert_eq!(v["entry"], json!("main"));
    // wasm_bytes als Array von u8 (snake_case wie bei FieldType::Money):
    assert_eq!(v["wasm_bytes"], json!([1, 2, 3]));
}

#[test]
fn script_state_serializes_lowercase() {
    use shared::script::ScriptState;
    assert_eq!(serde_json::to_value(ScriptState::Draft).unwrap(), json!("draft"));
    assert_eq!(serde_json::to_value(ScriptState::Active).unwrap(), json!("active"));
    assert_eq!(serde_json::to_value(ScriptState::Locked).unwrap(), json!("locked"));
}

#[test]
fn script_id_is_transparent_string() {
    use shared::script::ScriptId;
    let id = ScriptId("abc123".into());
    assert_eq!(serde_json::to_value(&id).unwrap(), json!("abc123"));
    let back: ScriptId = serde_json::from_str("\"xyz\"").unwrap();
    assert_eq!(back, ScriptId("xyz".into()));
}

#[test]
fn manifest_roundtrips_through_json() {
    use shared::script::{ScriptManifest, UiPrimitive};
    let m = ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Author,
        capabilities: vec![CapabilityToken::ReadAllEntitiesWhereAllowed],
        ui_primitives: vec![UiPrimitive::Vstack, UiPrimitive::Text, UiPrimitive::Table],
        timeout_ms: Some(500),
        memory_kb: Some(16_000),
        lift_capable: false,
    };
    let s = serde_json::to_string(&m).unwrap();
    let back: ScriptManifest = serde_json::from_str(&s).unwrap();
    assert_eq!(m, back);
}

/// Exhaustiveness-Anker: wenn jemand eine neue Variante hinzufuegt, muss
/// hier ein Eintrag dazu. Bricht den Build absichtlich.
#[test]
fn capability_token_exhaustiveness_anchor() {
    use shared::script::UiScope;
    use CapabilityToken::*;
    fn anchor(t: &CapabilityToken) -> &'static str {
        match t {
            ReadOwnEntities => "readOwnEntities",
            ReadAllEntitiesWhereAllowed => "readAllEntitiesWhereAllowed",
            WriteEntity { .. } => "writeEntity",
            ComputeOnly => "computeOnly",
            ReadI18n => "readI18n",
            EmitUiNode { .. } => "emitUiNode",
            EmitWorkflowAction => "emitWorkflowAction",
            LoadOtherScript => "loadOtherScript",
            ReadAuditLog { .. } => "readAuditLog",
            WriteAuditLog => "writeAuditLog",
            RegisterHostFunction => "registerHostFunction",
            ScheduleJob => "scheduleJob",
        }
    }
    let _ = anchor(&ReadOwnEntities);
    let _ = anchor(&WriteEntity { validated: true });
    let _ = anchor(&EmitUiNode { scope: UiScope::Leaf });
    let _ = anchor(&ReadAuditLog { own_only: false });
}
