//! Q0009 Phase 3.3 — Save-Pipeline + Lift-Capability-Analyse.

use sea_orm::EntityTrait;
use serial_test::serial;

use server::entity::script_version;
use server::script::save::{prepare_save, save_script, SaveInput};
use shared::script::capability::ScriptTier;
use shared::script::manifest::{ScriptManifest, UiPrimitive};
use shared::script::model::{ScriptKind, ScriptState};
use shared::script::CapabilityToken;

fn manifest_reader_minimal() -> ScriptManifest {
    ScriptManifest {
        manifest_version: 1,
        tier: ScriptTier::Reader,
        capabilities: vec![CapabilityToken::ComputeOnly],
        ui_primitives: vec![],
        timeout_ms: Some(1_000),
        memory_kb: Some(1_024),
        lift_capable: false,
    }
}

#[test]
fn prepare_save_parse_error_yields_draft_with_last_error() {
    let input = SaveInput {
        id: "broken".into(),
        source: "let x = ;".into(), // Syntax-Fehler
        manifest: manifest_reader_minimal(),
        kind: ScriptKind::Component {
            entry: "render".into(),
        },
        user: ScriptTier::Author,
        user_id: "u-1".into(),
        prev_version: None,
    };
    let prepared = prepare_save(&input);
    assert!(matches!(prepared.state, ScriptState::Draft));
    assert!(matches!(
        prepared.last_error,
        Some(shared::script::ScriptError::ParseFailed { .. })
    ));
}

#[test]
fn prepare_save_tier_mismatch_yields_draft() {
    let mut m = manifest_reader_minimal();
    m.tier = ScriptTier::Admin; // ueber dem User-Tier
    let input = SaveInput {
        id: "tier".into(),
        source: "1+2".into(),
        manifest: m,
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Reader,
        user_id: "u-1".into(),
        prev_version: None,
    };
    let prepared = prepare_save(&input);
    assert!(matches!(prepared.state, ScriptState::Draft));
    assert!(matches!(
        prepared.last_error,
        Some(shared::script::ScriptError::TierExceeded { .. })
    ));
}

#[test]
fn prepare_save_unknown_capability_for_tier_yields_draft() {
    // Reader-Tier darf KEIN WriteEntity haben.
    let mut m = manifest_reader_minimal();
    m.capabilities = vec![CapabilityToken::WriteEntity { validated: true }];
    let input = SaveInput {
        id: "cap".into(),
        source: "1+2".into(),
        manifest: m,
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Admin,
        user_id: "u-1".into(),
        prev_version: None,
    };
    let prepared = prepare_save(&input);
    assert!(matches!(prepared.state, ScriptState::Draft));
    assert!(matches!(
        prepared.last_error,
        Some(shared::script::ScriptError::ManifestInvalid { .. })
    ));
}

#[test]
fn prepare_save_clean_yields_active() {
    let mut m = manifest_reader_minimal();
    m.ui_primitives = vec![UiPrimitive::Text];
    let input = SaveInput {
        id: "clean".into(),
        source: "let r = 1 + 2; r".into(),
        manifest: m,
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Reader,
        user_id: "u-1".into(),
        prev_version: None,
    };
    let prepared = prepare_save(&input);
    assert!(
        matches!(prepared.state, ScriptState::Active),
        "expected Active, got {:?} with err {:?}",
        prepared.state,
        prepared.last_error
    );
    assert!(prepared.last_error.is_none());
    assert_eq!(prepared.version, 1);
}

#[test]
fn lift_capability_true_for_literal_string_arg_to_db_entities() {
    let input = SaveInput {
        id: "lift-ok".into(),
        source: r#"let xs = db.entities("product"); xs"#.into(),
        manifest: manifest_reader_minimal(),
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Reader,
        user_id: "u-1".into(),
        prev_version: None,
    };
    let prepared = prepare_save(&input);
    assert!(matches!(prepared.state, ScriptState::Active));
    assert!(
        prepared.manifest.lift_capable,
        "literal-string arg muss lift_capable=true setzen"
    );
}

#[test]
fn lift_capability_false_for_dynamic_arg_to_db_entities() {
    let input = SaveInput {
        id: "lift-no".into(),
        // entity_name aus einer Variable -> dynamic
        source: r#"let t = "product"; let xs = db.entities(t); xs"#.into(),
        manifest: manifest_reader_minimal(),
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Reader,
        user_id: "u-1".into(),
        prev_version: None,
    };
    let prepared = prepare_save(&input);
    assert!(matches!(prepared.state, ScriptState::Active));
    assert!(
        !prepared.manifest.lift_capable,
        "dynamic arg muss lift_capable=false ziehen"
    );
}

#[test]
fn lift_capability_only_inspects_first_arg_for_db_entity() {
    let input = SaveInput {
        id: "lift-no-2".into(),
        source: r#"let id = "p-1"; let e = db.entity("product", id); e"#.into(),
        manifest: manifest_reader_minimal(),
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Reader,
        user_id: "u-1".into(),
        prev_version: None,
    };
    // Aktuelle Regel (Spec): nur das ERSTE Argument zaehlt fuer Lift —
    // `db.entity(<literal>, <dyn>)` ist erlaubt. Der dynamische ID-Param
    // soll Lift nicht kollabieren. Test pinned diese Konvention.
    let prepared = prepare_save(&input);
    assert!(prepared.manifest.lift_capable);
}

#[test]
fn lift_capability_true_when_no_db_calls_at_all() {
    let input = SaveInput {
        id: "lift-pure".into(),
        source: r#"1 + 2"#.into(),
        manifest: manifest_reader_minimal(),
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Reader,
        user_id: "u-1".into(),
        prev_version: None,
    };
    let prepared = prepare_save(&input);
    // Ohne db.entities/entity-Calls bleibt das Skript trivial lift-fest.
    assert!(prepared.manifest.lift_capable);
}

#[tokio::test]
#[serial]
async fn save_script_persists_active_and_writes_version_row() {
    let _ = server::fresh_test_setup().await;
    let db = server::db::conn();

    let input = SaveInput {
        id: "persisted".into(),
        source: r#"let r = db.entities("product"); r"#.into(),
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Reader,
            capabilities: vec![CapabilityToken::ReadOwnEntities],
            ui_primitives: vec![],
            timeout_ms: Some(1_000),
            memory_kb: Some(1_024),
            lift_capable: false,
        },
        kind: ScriptKind::Provider {
            slot: shared::script::ProviderSlot::Formatter,
        },
        user: ScriptTier::Author,
        user_id: "u-system".into(),
        prev_version: None,
    };

    let result = save_script(&db, input).await.expect("save");
    assert_eq!(result.version, 1);
    assert!(matches!(result.state, ScriptState::Active));
    assert!(result.manifest.lift_capable);

    // Versionshistorie hat genau eine Row.
    let versions = script_version::Entity::find()
        .all(&db)
        .await
        .expect("query")
        .into_iter()
        .filter(|r| r.script_id == "persisted")
        .collect::<Vec<_>>();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version, 1);
    assert_eq!(versions[0].state_at_save, "active");
}

#[tokio::test]
#[serial]
async fn save_script_version_monotone_increases() {
    let _ = server::fresh_test_setup().await;
    let db = server::db::conn();

    let mk_input = |prev: Option<u32>| SaveInput {
        id: "evolve".into(),
        source: "1 + 2".into(),
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Reader,
            capabilities: vec![CapabilityToken::ComputeOnly],
            ui_primitives: vec![],
            timeout_ms: Some(1_000),
            memory_kb: Some(1_024),
            lift_capable: false,
        },
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Reader,
        user_id: "u-1".into(),
        prev_version: prev,
    };

    let v1 = save_script(&db, mk_input(None)).await.expect("v1");
    assert_eq!(v1.version, 1);

    let v2 = save_script(&db, mk_input(Some(1))).await.expect("v2");
    assert_eq!(v2.version, 2);

    // Falsche prev_version -> Fehler.
    let bad = save_script(&db, mk_input(Some(99))).await;
    assert!(bad.is_err(), "version conflict muss erkannt werden");
}

// S4-Regression: die Tier-Validierung muss struct-Variant-Felder beachten.
// Vor dem Fix verglich `token_eq` nur die Variante (matches! mit `{..}`),
// sodass ein Reader eine Composite-UI-Node deklarieren konnte, obwohl das
// Reader-Set nur `EmitUiNode{Leaf}` erlaubt — ein Tier-Bypass.
#[test]
fn reader_cannot_declare_composite_ui_node_scope() {
    use shared::script::capability::UiScope;
    let mut m = manifest_reader_minimal();
    m.capabilities = vec![CapabilityToken::EmitUiNode {
        scope: UiScope::Composite,
    }];
    let input = SaveInput {
        id: "scope-bypass".into(),
        source: "1+2".into(),
        manifest: m,
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Admin, // hoher User-Tier — Deckel ist nicht das Thema
        user_id: "u-1".into(),
        prev_version: None,
    };
    let prepared = prepare_save(&input);
    assert!(
        matches!(prepared.state, ScriptState::Draft),
        "Composite-Scope fuer Reader muss als Draft (ungueltig) enden"
    );
    assert!(matches!(
        prepared.last_error,
        Some(shared::script::ScriptError::ManifestInvalid { .. })
    ));
}

// S4: der legitime Reader-Wert `EmitUiNode{Leaf}` muss weiterhin akzeptiert
// werden — der Fix darf nicht zu streng sein.
#[test]
fn reader_may_declare_leaf_ui_node_scope() {
    use shared::script::capability::UiScope;
    let mut m = manifest_reader_minimal();
    m.capabilities = vec![CapabilityToken::EmitUiNode {
        scope: UiScope::Leaf,
    }];
    let input = SaveInput {
        id: "scope-ok".into(),
        source: "1+2".into(),
        manifest: m,
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Reader,
        user_id: "u-1".into(),
        prev_version: None,
    };
    let prepared = prepare_save(&input);
    assert!(
        matches!(prepared.state, ScriptState::Active),
        "Leaf-Scope fuer Reader muss gueltig (Active) sein, war {:?}",
        prepared.state
    );
}
