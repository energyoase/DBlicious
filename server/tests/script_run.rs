//! Q0009 Phase 3.4 — Run-Pipeline + Audit-Buffer-Flush.
//!
//! Nach dem B3-Umbau (Q0009-Review) fuehrt `run_and_persist` echte
//! Skript-Host-Calls durch die Sandbox-Gate — die Tests treiben das
//! daher mit echten Skripten (`db.entities(...)`) statt einer
//! simulierenden Body-Closure.

use std::sync::Arc;

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serial_test::serial;

use server::entity::script_audit_log;
use server::script::run::run_and_persist;
use server::script::save::{save_script, SaveInput};
use shared::script::engine::ScriptCtx;
use shared::script::manifest::ScriptManifest;
use shared::script::model::ScriptKind;
use shared::script::{capability::ScriptTier, CapabilityToken, ScriptError, ScriptValue};

fn manifest_with(caps: Vec<CapabilityToken>, tier: ScriptTier) -> ScriptManifest {
    ScriptManifest {
        manifest_version: 1,
        tier,
        capabilities: caps,
        ui_primitives: vec![],
        timeout_ms: Some(5_000),
        memory_kb: Some(2_048),
        lift_capable: false,
    }
}

async fn persist_simple_script(id: &str, source: &str) -> shared::script::Script {
    persist_script_with_caps(id, source, vec![CapabilityToken::ReadOwnEntities]).await
}

async fn persist_script_with_caps(
    id: &str,
    source: &str,
    caps: Vec<CapabilityToken>,
) -> shared::script::Script {
    let db = server::db::conn();
    let input = SaveInput {
        id: id.into(),
        source: source.into(),
        manifest: manifest_with(caps, ScriptTier::Author),
        kind: ScriptKind::Component { entry: "x".into() },
        user: ScriptTier::Author,
        user_id: "u-system".into(),
        prev_version: None,
    };
    save_script(&db, input).await.expect("save")
}

#[tokio::test]
#[serial]
async fn run_and_persist_records_ok_outcome_with_token_uses() {
    let _ = server::fresh_test_setup().await;
    let db = server::db::conn();
    // Echtes Skript: zwei db.entities-Calls (ReadOwnEntities-Gate) + 42.
    let script = persist_simple_script("r-1", r#"db.entities("a"); db.entities("b"); 42"#).await;

    let mock = Arc::new(shared::script::testing::MockHostApi::new());
    let ctx = ScriptCtx {
        user_id: Some("u-system".into()),
        tenant_id: None,
        locale: "de".into(),
    };

    let rec = run_and_persist(&db, &script, ctx, mock).await.expect("run");

    assert_eq!(rec.outcome, "ok");
    assert!(matches!(rec.value, Some(ScriptValue::Number(_))));
    assert!(rec.error.is_none());

    // Audit-Row pruefen.
    let rows = script_audit_log::Entity::find()
        .filter(script_audit_log::Column::ScriptId.eq("r-1"))
        .all(&db)
        .await
        .expect("query");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.outcome, "ok");
    assert_eq!(row.script_version, 1);
    assert_eq!(row.user_id.as_deref(), Some("u-system"));
    // Tokens_used JSON-Array mit 2 Eintraegen, beide "ok".
    let tokens: serde_json::Value = serde_json::from_str(&row.tokens_used).expect("tokens JSON");
    assert!(tokens.is_array());
    assert_eq!(tokens.as_array().unwrap().len(), 2);
    assert_eq!(tokens[0]["outcome"], serde_json::Value::String("ok".into()));
}

#[tokio::test]
#[serial]
async fn run_and_persist_records_capability_denied_outcome() {
    let _ = server::fresh_test_setup().await;
    let db = server::db::conn();
    // Manifest hat NUR ComputeOnly — `db.entities(...)` braucht aber
    // ReadOwnEntities. Die Gate feuert im echten eval-Pfad (B3) und der
    // Fehler ist unmaskable (B1), also bricht der Run mit CapabilityDenied.
    let script = persist_script_with_caps(
        "r-deny",
        r#"db.entities("x")"#,
        vec![CapabilityToken::ComputeOnly],
    )
    .await;

    let mock = Arc::new(shared::script::testing::MockHostApi::new());
    let ctx = ScriptCtx::default();

    let rec = run_and_persist(&db, &script, ctx, mock).await.expect("run");

    assert_eq!(rec.outcome, "capabilityDenied");
    assert!(matches!(
        rec.error,
        Some(ScriptError::CapabilityDenied { .. })
    ));
    let rows = script_audit_log::Entity::find()
        .filter(script_audit_log::Column::ScriptId.eq("r-deny"))
        .all(&db)
        .await
        .expect("query");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].outcome, "capabilityDenied");

    // Token-Use-Buffer enthaelt 1 Eintrag mit outcome=denied.
    let tokens: serde_json::Value =
        serde_json::from_str(&rows[0].tokens_used).expect("tokens JSON");
    assert_eq!(tokens.as_array().unwrap().len(), 1);
    assert_eq!(
        tokens[0]["outcome"],
        serde_json::Value::String("denied".into())
    );
}

#[tokio::test]
#[serial]
async fn capability_denied_cannot_be_caught_by_try_catch() {
    // B1 (Q0009-Review) end-to-end: ein unmaskable CapabilityDenied darf
    // nicht per `try`/`catch` im Skript abgefangen werden. Das Skript
    // versucht genau das — der Run MUSS trotzdem mit capabilityDenied enden
    // (nicht mit dem catch-Wert 42).
    let _ = server::fresh_test_setup().await;
    let db = server::db::conn();
    let script = persist_script_with_caps(
        "r-catch",
        r#"let x = 0; try { db.entities("x"); x = 1 } catch(e) { x = 42 } x"#,
        vec![CapabilityToken::ComputeOnly], // KEIN ReadOwnEntities
    )
    .await;

    let mock = Arc::new(shared::script::testing::MockHostApi::new());
    let rec = run_and_persist(&db, &script, ScriptCtx::default(), mock)
        .await
        .expect("run");

    assert_eq!(
        rec.outcome, "capabilityDenied",
        "try/catch darf den unmaskable CapabilityDenied NICHT schlucken"
    );
    assert!(matches!(
        rec.error,
        Some(ScriptError::CapabilityDenied { .. })
    ));
    assert!(
        !matches!(rec.value, Some(ScriptValue::Number(n)) if n == 42.0),
        "der catch-Zweig (x=42) darf NICHT erreicht worden sein"
    );
}

#[tokio::test]
#[serial]
async fn run_and_persist_handles_compile_failure_without_panic() {
    // Skript mit Syntax-Fehler -> auch dann muss ein Audit-Eintrag entstehen.
    let _ = server::fresh_test_setup().await;
    let db = server::db::conn();

    // Wir bauen den Script-Eintrag direkt: save_script wuerde den Source als
    // Draft markieren. Hier interessiert uns nur run_and_persist's Robustheit
    // bei einer kaputten Quelle.
    let script = shared::script::Script {
        id: "r-bad".into(),
        kind: ScriptKind::Component { entry: "x".into() },
        manifest: manifest_with(vec![CapabilityToken::ComputeOnly], ScriptTier::Reader),
        source: "let x = ;".into(),
        version: 1,
        state: shared::script::ScriptState::Draft,
        last_error: None,
        created_by: "u-system".into(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };

    // FK-Constraint: parent-Row in scripts braucht es noch. Wir setzen sie
    // separat ein (das spiegelt einen pathologischen Fall, kein Save-Pfad).
    use sea_orm::ActiveModelTrait;
    let now = chrono::Utc::now().to_rfc3339();
    server::entity::script::ActiveModel {
        id: sea_orm::Set("r-bad".into()),
        kind: sea_orm::Set("component".into()),
        manifest_json: sea_orm::Set("{}".into()),
        source: sea_orm::Set("let x = ;".into()),
        version: sea_orm::Set(1),
        state: sea_orm::Set("draft".into()),
        last_error: sea_orm::Set(None),
        created_by: sea_orm::Set("u-system".into()),
        created_at: sea_orm::Set(now.clone()),
        updated_at: sea_orm::Set(now),
    }
    .insert(&db)
    .await
    .expect("seed bad script");

    let mock = Arc::new(shared::script::testing::MockHostApi::new());
    let rec = run_and_persist(&db, &script, ScriptCtx::default(), mock)
        .await
        .expect("run");

    assert_eq!(rec.outcome, "parseFailed");
    assert!(matches!(rec.error, Some(ScriptError::ParseFailed { .. })));
}
