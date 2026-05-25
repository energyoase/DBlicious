//! Q0009 Phase 6 — GraphQL Surface (Schema-Execute-Roundtrips).
//!
//! Wie `e2e.rs` fahren wir kein Axum hoch — `Schema::execute` reicht fuer den
//! Resolver-Vertrag. Auth-Kontext ist `login_as("admin",..)` (g-admin
//! erhaelt Tier=Admin im saveScript-Pfad), sonst Author.

use async_graphql::{Request, Variables};
use sea_orm::EntityTrait;
use serde_json::{json, Value};
use serial_test::serial;

use server::entity::script_audit_log;
use server::{auth, build_schema, fresh_test_setup, setup_for_tests, AuthContext};

async fn boot() {
    let _ = fresh_test_setup().await;
}

async fn login_as(username: &str, password: &str) -> AuthContext {
    let _ = setup_for_tests().await;
    let session = auth::login(username, password).await.expect("login");
    AuthContext {
        user: Some(auth::strip_secret(session.user)),
        token: Some(session.token),
    }
}

async fn exec(query: &str, vars: Value, ctx: AuthContext) -> async_graphql::Response {
    let _ = setup_for_tests().await;
    let schema = build_schema();
    let req = Request::new(query)
        .variables(Variables::from_json(vars))
        .data(ctx);
    schema.execute(req).await
}

fn clean_manifest() -> Value {
    // Reader-Tier mit `ComputeOnly`-Cap reicht fuer `1 + 1`.
    json!({
        "manifestVersion": 1,
        "tier": "reader",
        "capabilities": [{ "kind": "computeOnly" }],
        "uiPrimitives": [],
        "timeoutMs": 5000,
        "memoryKb": 2048,
        "liftCapable": false
    })
}

fn provider_kind() -> Value {
    json!({ "kind": "provider", "slot": "formatter" })
}

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn save_script_then_query_returns_active() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    let res = exec(
        r#"mutation($i:SaveScriptInput!){ saveScript(input:$i){ id state version } }"#,
        json!({
            "i": {
                "id": "gql-clean",
                "source": "1 + 1",
                "manifest": clean_manifest(),
                "kind": provider_kind()
            }
        }),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["saveScript"]["id"], json!("gql-clean"));
    assert_eq!(v["saveScript"]["state"], json!("ACTIVE"));
    assert_eq!(v["saveScript"]["version"], json!(1));

    // Roundtrip: script(id) liefert dieselbe Row.
    let res2 = exec(
        r#"query($id:String!){ script(id:$id){ id state version source } }"#,
        json!({ "id": "gql-clean" }),
        ctx,
    )
    .await;
    assert!(res2.errors.is_empty(), "{:?}", res2.errors);
    let v2 = res2.data.into_json().unwrap();
    assert_eq!(v2["script"]["id"], json!("gql-clean"));
    assert_eq!(v2["script"]["state"], json!("ACTIVE"));
    assert_eq!(v2["script"]["source"], json!("1 + 1"));
}

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn save_script_with_invalid_manifest_yields_draft_with_last_error() {
    boot().await;
    let ctx = login_as("editor", "editor").await;

    // Editor ist g-editor — saveScript ordnet user_tier=Author zu.
    // Manifest deklariert WriteEntity, das fuer Author nicht erlaubt ist
    // -> validate_manifest schlaegt fehl mit ManifestInvalid.
    // (Wir nutzen `editor`, weil admin-Login Tier=Admin bekommt und damit
    // WriteEntity erlauben wuerde.)
    let bad_manifest = json!({
        "manifestVersion": 1,
        "tier": "admin",
        "capabilities": [{ "kind": "writeEntity", "validated": true }],
        "uiPrimitives": [],
        "timeoutMs": 5000,
        "memoryKb": 2048,
        "liftCapable": false
    });

    let res = exec(
        r#"mutation($i:SaveScriptInput!){ saveScript(input:$i){ id state lastError } }"#,
        json!({
            "i": {
                "id": "gql-bad",
                "source": "1+1",
                "manifest": bad_manifest,
                "kind": provider_kind()
            }
        }),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["saveScript"]["id"], json!("gql-bad"));
    assert_eq!(v["saveScript"]["state"], json!("DRAFT"));
    let last_err = &v["saveScript"]["lastError"];
    assert!(
        !last_err.is_null(),
        "lastError muss gesetzt sein: {last_err}"
    );
    // `kind` ist entweder "tierExceeded" oder "manifestInvalid" je nachdem
    // wo die Validierung kippt — beide signalisieren Draft.
    let kind = last_err["kind"].as_str().unwrap_or("");
    assert!(
        kind == "tierExceeded" || kind == "manifestInvalid",
        "unerwarteter ScriptError-kind: {kind}"
    );
}

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn script_query_returns_null_for_unknown_id() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"query($id:String!){ script(id:$id){ id } }"#,
        json!({ "id": "does-not-exist" }),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert!(v["script"].is_null(), "erwartete null, bekam: {v:?}");
}

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn preview_script_run_for_simple_addition_returns_value_without_audit_row() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    // Skript persistieren.
    let save = exec(
        r#"mutation($i:SaveScriptInput!){ saveScript(input:$i){ id state } }"#,
        json!({
            "i": {
                "id": "gql-prev",
                "source": "1 + 1",
                "manifest": clean_manifest(),
                "kind": provider_kind()
            }
        }),
        ctx.clone(),
    )
    .await;
    assert!(save.errors.is_empty(), "{:?}", save.errors);

    // Audit-Log-Stand vor dem Preview.
    let db = server::db::conn();
    let before = script_audit_log::Entity::find()
        .all(&db)
        .await
        .expect("audit pre-count");
    let before_count = before.len();

    // Preview.
    let res = exec(
        r#"mutation($i:PreviewScriptRunInput!){
            previewScriptRun(input:$i){
                output error runId durationMs tokensUsed
            }
        }"#,
        json!({ "i": { "scriptId": "gql-prev", "args": null } }),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert!(v["previewScriptRun"]["error"].is_null());
    // Engine liefert Numbers als f64 zurueck — wir vergleichen nicht
    // strikt gegen json!(2) (int), sondern via as_f64.
    let val = v["previewScriptRun"]["output"]["value"]
        .as_f64()
        .expect("output.value muss Zahl sein");
    assert!((val - 2.0).abs() < 1e-9, "erwartete 2, bekam {val}");
    assert!(!v["previewScriptRun"]["runId"].as_str().unwrap().is_empty());

    // Audit-Log-Stand nach Preview: unveraendert.
    let after = script_audit_log::Entity::find()
        .all(&db)
        .await
        .expect("audit post-count");
    assert_eq!(
        after.len(),
        before_count,
        "previewScriptRun darf KEINE Audit-Row schreiben"
    );
}

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn preview_script_run_rejects_write_entity_capability() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    // Admin-Tier-Skript mit WriteEntity-Capability. Save klappt (Admin darf
    // das), aber das Preview muss CapabilityDenied liefern.
    let admin_manifest = json!({
        "manifestVersion": 1,
        "tier": "admin",
        "capabilities": [
            { "kind": "computeOnly" },
            { "kind": "writeEntity", "validated": true }
        ],
        "uiPrimitives": [],
        "timeoutMs": 5000,
        "memoryKb": 2048,
        "liftCapable": false
    });

    let save = exec(
        r#"mutation($i:SaveScriptInput!){ saveScript(input:$i){ id state lastError } }"#,
        json!({
            "i": {
                "id": "gql-write",
                "source": "1 + 1",
                "manifest": admin_manifest,
                "kind": provider_kind()
            }
        }),
        ctx.clone(),
    )
    .await;
    assert!(save.errors.is_empty(), "{:?}", save.errors);
    let sv = save.data.into_json().unwrap();
    // Skript muss aktiv sein — sonst pruefen wir den Preview-Pfad nicht.
    assert_eq!(
        sv["saveScript"]["state"],
        json!("ACTIVE"),
        "save-state war nicht ACTIVE: {sv:?}"
    );

    let res = exec(
        r#"mutation($i:PreviewScriptRunInput!){
            previewScriptRun(input:$i){ output error tokensUsed }
        }"#,
        json!({ "i": { "scriptId": "gql-write", "args": null } }),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert!(
        v["previewScriptRun"]["output"].is_null(),
        "output muss null sein bei CapabilityDenied"
    );
    let err = &v["previewScriptRun"]["error"];
    assert!(!err.is_null(), "error muss gesetzt sein");
    assert_eq!(
        err["kind"],
        json!("capabilityDenied"),
        "erwartete capabilityDenied, bekam: {err:?}"
    );
}
