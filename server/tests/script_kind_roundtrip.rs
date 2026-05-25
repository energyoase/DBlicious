//! Roundtrip-Test: ProviderSlot ueberlebt server → GQL → client (PRW-C1).
//!
//! Testet zwei Wege:
//!
//! 1. **Client-Unit**: `RawScript::into_typed` mit
//!    `{"kind":"provider","slot":"formatter"}` liefert
//!    `ScriptKind::Provider { slot: ProviderSlot::Formatter }`.
//!    Schlaegt fehl, wenn der Client auf den `unwrap_or(Component)`-Pfad
//!    zurueckfaellt (alter Bug: slot fehlte im JSON).
//!
//! 2. **Server-GQL-Roundtrip**: Script via `saveScript` anlegen, dann
//!    `script(id)` abfragen und pruefen, dass das `kind`-JSON den Slot
//!    enthaelt UND `into_typed` daraus wieder `Provider{Formatter}`
//!    rekonstruiert.

use async_graphql::{Request, Variables};
use serde_json::{json, Value};
use serial_test::serial;

use client::graphql::queries::RawScript;
use shared::script::model::{ProviderSlot, ScriptKind};

use server::{auth, build_schema, fresh_test_setup, setup_for_tests, AuthContext};

// ---------------------------------------------------------------------------
// Helpers (analog zu script_graphql.rs)
// ---------------------------------------------------------------------------

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

fn reader_manifest() -> Value {
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

// ---------------------------------------------------------------------------
// Test 1: Client-Unit — into_typed mit vollem Provider/Formatter-JSON
// ---------------------------------------------------------------------------

#[test]
fn raw_script_into_typed_reconstructs_provider_formatter() {
    // Dieses JSON ist exakt das, was `script_model_to_gql` ab PRW-C1 emittiert.
    let raw_kind = json!({ "kind": "provider", "slot": "formatter" });

    let raw = RawScript {
        id: "test-provider".into(),
        kind: raw_kind,
        source: "".into(),
        version: 1,
        state: "ACTIVE".into(),
        manifest: json!({}),
        last_error: None,
        created_by: "system".into(),
        created_at: "2024-01-01T00:00:00Z".into(),
        updated_at: "2024-01-01T00:00:00Z".into(),
    };

    let script = raw.into_typed();
    assert_eq!(
        script.kind,
        ScriptKind::Provider {
            slot: ProviderSlot::Formatter
        },
        "into_typed muss Provider{{Formatter}} liefern — nicht Component (alter Bug)"
    );
}

#[test]
fn raw_script_into_typed_falls_back_to_component_without_slot() {
    // Verifikation des alten Verhaltens als Kontrollfall:
    // {"kind":"provider"} ohne slot schlaegt die Deserialisierung fehl
    // und faellt auf Component zurueck.
    let raw_kind = json!({ "kind": "provider" });

    let raw = RawScript {
        id: "test-fallback".into(),
        kind: raw_kind,
        source: "".into(),
        version: 1,
        state: "ACTIVE".into(),
        manifest: json!({}),
        last_error: None,
        created_by: "system".into(),
        created_at: "2024-01-01T00:00:00Z".into(),
        updated_at: "2024-01-01T00:00:00Z".into(),
    };

    let script = raw.into_typed();
    // Ohne slot ist Provider nicht deserialisierbar -> Component-Fallback.
    assert!(
        matches!(script.kind, ScriptKind::Component { .. }),
        "Ohne slot muss into_typed auf Component zurueckfallen (dokumentiertes Fallback-Verhalten)"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Server-GQL-Roundtrip — slot ueberlebt DB-Schreib/Lese-Zyklus
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn save_provider_formatter_then_query_returns_slot_in_kind() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    // saveScript mit provider/formatter
    let save_res = exec(
        r#"mutation($i:SaveScriptInput!){
            saveScript(input:$i){ id state version }
        }"#,
        json!({
            "i": {
                "id": "rt-provider-formatter",
                "source": "1 + 1",
                "manifest": reader_manifest(),
                "kind": { "kind": "provider", "slot": "formatter" }
            }
        }),
        ctx.clone(),
    )
    .await;
    assert!(
        save_res.errors.is_empty(),
        "saveScript fehlgeschlagen: {:?}",
        save_res.errors
    );
    let sv = save_res.data.into_json().unwrap();
    assert_eq!(
        sv["saveScript"]["state"],
        json!("ACTIVE"),
        "Script muss ACTIVE sein"
    );

    // script(id) lesen und kind-JSON pruefen
    let query_res = exec(
        r#"query($id:String!){ script(id:$id){ id kind } }"#,
        json!({ "id": "rt-provider-formatter" }),
        ctx,
    )
    .await;
    assert!(
        query_res.errors.is_empty(),
        "script-Query fehlgeschlagen: {:?}",
        query_res.errors
    );
    let qv = query_res.data.into_json().unwrap();

    // kind ist ein inline JSON-Objekt im GQL-Response (async_graphql::Json<V>
    // serialisiert den Wert als inline JSON, nicht als quoted String).
    let kind_raw: Value = qv["script"]["kind"].clone();

    assert_eq!(
        kind_raw["kind"],
        json!("provider"),
        "kind.kind muss 'provider' sein"
    );
    assert_eq!(
        kind_raw["slot"],
        json!("formatter"),
        "kind.slot muss 'formatter' sein — PRW-C1-Kern-Assert: slot ueberlebt DB-Roundtrip"
    );

    // Und nochmal via into_typed: der Slot kommt als Provider{Formatter} an.
    let raw = RawScript {
        id: "rt-provider-formatter".into(),
        kind: kind_raw,
        source: "1 + 1".into(),
        version: 1,
        state: "ACTIVE".into(),
        manifest: json!({}),
        last_error: None,
        created_by: "admin".into(),
        created_at: "".into(),
        updated_at: "".into(),
    };
    let script = raw.into_typed();
    assert_eq!(
        script.kind,
        ScriptKind::Provider {
            slot: ProviderSlot::Formatter
        },
        "into_typed muss nach GQL-Roundtrip Provider{{Formatter}} liefern"
    );
}
