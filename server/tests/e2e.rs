//! End-to-End-Tests gegen das GraphQL-Schema.
//!
//! Diese Tests fahren *kein* Axum hoch — sie reden direkt mit
//! `async_graphql::Schema::execute()`. Das ist hinreichend, um den
//! Resolver-Pfad inkl. Auth-Context und Permission-Gating zu pruefen.
//! Echte HTTP-Roundtrips (Header, Status-Codes) waeren zusaetzlich
//! sinnvoll, sind aber fuer den GraphQL-Kontrakt nicht zwingend.

use async_graphql::{Request, Variables};
use serde_json::{json, Value};

use server::{auth, build_schema, fresh_test_setup, setup_for_tests, AuthContext};

fn anon() -> AuthContext {
    AuthContext::default()
}

/// Test-Setup. Jeder Test ruft das einmal am Anfang auf — `fresh_test_setup`
/// reisst den prozessweiten DB-Slot ab, sodass keine Restdaten vom vorigen
/// Test stehen bleiben. Folgende `exec`-Calls reusen die so erzeugte
/// Verbindung (init ist idempotent).
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

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn anonymous_login_succeeds_and_returns_token() {
    boot().await;
    let res = exec(
        r#"mutation($u:String!,$p:String!){ login(username:$u,password:$p){ ok error session { token user { username } } } }"#,
        json!({"u":"admin","p":"admin"}),
        anon(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["login"]["ok"], json!(true));
    assert!(v["login"]["session"]["token"].as_str().unwrap().len() >= 16);
    assert_eq!(v["login"]["session"]["user"]["username"], json!("admin"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn anonymous_login_with_wrong_password_returns_invalid_credentials() {
    boot().await;
    let res = exec(
        r#"mutation($u:String!,$p:String!){ login(username:$u,password:$p){ ok error } }"#,
        json!({"u":"admin","p":"nope"}),
        anon(),
    )
    .await;
    assert!(res.errors.is_empty());
    let v = res.data.into_json().unwrap();
    assert_eq!(v["login"]["ok"], json!(false));
    assert_eq!(v["login"]["error"], json!("invalidCredentials"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn navigation_requires_authentication() {
    boot().await;
    let res = exec(
        r#"query{ navigation { id } }"#,
        json!({}),
        anon(),
    )
    .await;
    assert!(!res.errors.is_empty(), "anonyme navigation muss fehlschlagen");
    let msg = res.errors[0].message.clone();
    assert!(msg.contains("unauthenticated"), "got: {msg}");
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn current_user_returns_authenticated_identity() {
    boot().await;
    let ctx = login_as("editor", "editor").await;
    let res = exec(
        r#"query{ currentUser { username displayName } currentPermissions { entityType canRead } }"#,
        json!({}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["currentUser"]["username"], json!("editor"));
    assert!(!v["currentPermissions"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn viewer_cannot_create_product() {
    boot().await;
    let ctx = login_as("viewer", "viewer").await;
    let res = exec(
        r#"mutation($t:String!,$f:JSON!){ createEntity(entityType:$t,fields:$f){ ok } }"#,
        json!({"t":"product","f":{"name":"Foo","price":1}}),
        ctx,
    )
    .await;
    assert!(!res.errors.is_empty(), "viewer darf nicht erstellen");
    assert!(res.errors[0].message.contains("forbidden"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn admin_create_then_update_with_stale_hash_is_rejected() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    // 1) Erstellen.
    let res = exec(
        r#"mutation($t:String!,$id:String,$f:JSON!){
            createEntity(entityType:$t,id:$id,fields:$f){
                ok entity { id fields } validation
            }
        }"#,
        json!({"t":"product","id":"p-e2e-1","f":{"name":"E2E Probe","price":1.0,"currency":"EUR"}}),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["createEntity"]["ok"], json!(true));

    // 2) Update mit falschem expected_hash.
    let res = exec(
        r#"mutation($t:String!,$id:String!,$f:JSON!,$h:String){
            updateEntity(entityType:$t,id:$id,fields:$f,expectedHash:$h){
                ok validation
            }
        }"#,
        json!({"t":"product","id":"p-e2e-1","f":{"name":"neuer Name"},"h":"42"}),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["updateEntity"]["ok"], json!(false));
    let msgs = &v["updateEntity"]["validation"]["messages"];
    assert!(
        msgs.as_array()
            .unwrap()
            .iter()
            .any(|m| m["messageKey"] == "error.concurrent_modification"),
        "expected concurrent_modification, got: {v:?}"
    );

    // Cleanup.
    let _ = exec(
        r#"mutation($t:String!,$id:String!){ deleteEntity(entityType:$t,id:$id){ ok } }"#,
        json!({"t":"product","id":"p-e2e-1"}),
        ctx,
    )
    .await;
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn editor_validation_blocks_required_field() {
    boot().await;
    let ctx = login_as("editor", "editor").await;
    let res = exec(
        r#"mutation($t:String!,$f:JSON!){
            createEntity(entityType:$t,fields:$f){ ok validation }
        }"#,
        json!({"t":"customer","f":{}}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["createEntity"]["ok"], json!(false));
    let msgs = &v["createEntity"]["validation"]["messages"];
    assert!(msgs
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["messageKey"] == "validation.required"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn editor_validation_blocks_bad_email_pattern() {
    boot().await;
    let ctx = login_as("editor", "editor").await;
    let res = exec(
        r#"mutation($t:String!,$f:JSON!){
            createEntity(entityType:$t,fields:$f){ ok validation }
        }"#,
        json!({"t":"customer","f":{"id":"c-1","displayName":"Alice Tester","email":"not-an-email"}}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["createEntity"]["ok"], json!(false));
    let msgs = &v["createEntity"]["validation"]["messages"];
    assert!(msgs
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["messageKey"] == "validation.pattern"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn bulk_create_handles_mixed_validation_results() {
    boot().await;
    let ctx = login_as("editor", "editor").await;
    let res = exec(
        r#"mutation($t:String!,$items:[JSON!]!){
            createEntities(entityType:$t, items:$items){ ok }
        }"#,
        json!({
            "t":"customer",
            "items":[
                {"displayName":"OK Person","email":"ok@example.com"},
                {"displayName":""}, // required + length verletzt
            ]
        }),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    let arr = v["createEntities"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["ok"], json!(true));
    assert_eq!(arr[1]["ok"], json!(false));
}

// =============================================================================
// Sort / Filter (0.5.1)
// =============================================================================
//
// Diese Tests verifizieren, dass `entities`-Query die `sort_by`/`sort_dir`/
// `filter`-Argumente tatsaechlich auswertet. Sie laufen gegen den seed-state
// aus `examples/shop/entities/customer/seed.*`.

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn entities_sort_by_display_name_asc_then_desc() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    let q = r#"query($t:String!,$by:String!,$dir:String!){
        entities(entityType:$t, page:1, pageSize:100, sortBy:$by, sortDir:$dir) {
            items { id fields }
            totalCount
        }
    }"#;

    let asc = exec(
        q,
        json!({"t":"customer","by":"displayName","dir":"asc"}),
        ctx.clone(),
    )
    .await;
    assert!(asc.errors.is_empty(), "{:?}", asc.errors);
    let asc_items = asc.data.into_json().unwrap()["entities"]["items"]
        .as_array()
        .cloned()
        .unwrap();
    assert!(asc_items.len() >= 2, "Seed sollte mehrere Customer haben");
    let asc_names: Vec<String> = asc_items
        .iter()
        .map(|e| e["fields"]["displayName"].as_str().unwrap_or("").to_string())
        .collect();
    let mut sorted = asc_names.clone();
    sorted.sort();
    assert_eq!(asc_names, sorted, "asc-Sort liefert nicht aufsteigend");

    let desc = exec(
        q,
        json!({"t":"customer","by":"displayName","dir":"desc"}),
        ctx,
    )
    .await;
    assert!(desc.errors.is_empty(), "{:?}", desc.errors);
    let desc_items = desc.data.into_json().unwrap()["entities"]["items"]
        .as_array()
        .cloned()
        .unwrap();
    let desc_names: Vec<String> = desc_items
        .iter()
        .map(|e| e["fields"]["displayName"].as_str().unwrap_or("").to_string())
        .collect();
    let mut expected_desc = asc_names.clone();
    expected_desc.reverse();
    assert_eq!(desc_names, expected_desc, "desc-Sort ist nicht die Umkehrung");
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn entities_filter_text_contains_narrows_result_set() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    // Erst alle Customers holen, um eine echte Substring-Probe zu finden.
    let q_all = r#"query($t:String!){
        entities(entityType:$t, page:1, pageSize:100) {
            items { fields }
            totalCount
        }
    }"#;
    let all = exec(q_all, json!({"t":"customer"}), ctx.clone()).await;
    let total_unfiltered = all.data.into_json().unwrap()["entities"]["totalCount"]
        .as_i64()
        .unwrap();
    assert!(total_unfiltered >= 1, "Seed muss Customers haben");

    // Filter mit globalSearch auf ein konstantes Sub-String aus dem Beispiel:
    // examples/shop/entities/customer/seed.* enthaelt mind. einen Customer
    // mit "@". Wir filtern auf "@" und erwarten >0 Treffer und <= Gesamtzahl.
    let q_filtered = r#"query($t:String!, $f:JSON!){
        entities(entityType:$t, page:1, pageSize:100, filter:$f) {
            totalCount
        }
    }"#;
    let res = exec(
        q_filtered,
        json!({
            "t":"customer",
            "f": {"globalSearch": "@", "predicates": []}
        }),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let total_filtered = res.data.into_json().unwrap()["entities"]["totalCount"]
        .as_i64()
        .unwrap();
    assert!(total_filtered > 0, "globalSearch '@' muss mindestens einen Treffer haben");
    assert!(
        total_filtered <= total_unfiltered,
        "gefilterte Anzahl darf die Gesamtzahl nicht uebersteigen"
    );

    // Filter, der niemals matchen sollte → 0 Treffer.
    let res = exec(
        q_filtered,
        json!({
            "t":"customer",
            "f": {"globalSearch": "zzz_unmoeglicher_suchbegriff_xyz", "predicates": []}
        }),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let total = res.data.into_json().unwrap()["entities"]["totalCount"]
        .as_i64()
        .unwrap();
    assert_eq!(total, 0, "unbekannter Suchbegriff darf nichts liefern");
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn entities_filter_predicate_text_equals_matches_exact_record() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    // Erst irgendeinen displayName aus dem Seed holen.
    let q_all = r#"query($t:String!){
        entities(entityType:$t, page:1, pageSize:100) { items { fields } }
    }"#;
    let all = exec(q_all, json!({"t":"customer"}), ctx.clone()).await;
    let items = all.data.into_json().unwrap()["entities"]["items"]
        .as_array()
        .cloned()
        .unwrap();
    let probe = items[0]["fields"]["displayName"].as_str().unwrap().to_string();

    // Praedikats-Filter `textEquals` auf displayName == probe.
    let q = r#"query($t:String!, $f:JSON!){
        entities(entityType:$t, page:1, pageSize:100, filter:$f) {
            items { fields }
            totalCount
        }
    }"#;
    let res = exec(
        q,
        json!({
            "t":"customer",
            "f": {
                "globalSearch": null,
                "predicates": [{
                    "key": "displayName",
                    "predicate": { "op": "textEquals", "value": probe, "case_insensitive": false }
                }]
            }
        }),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    let total = v["entities"]["totalCount"].as_i64().unwrap();
    assert!(total >= 1, "exakter Match muss mindestens 1 liefern");
    let found = v["entities"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|i| i["fields"]["displayName"].as_str() == Some(probe.as_str()));
    assert!(found, "alle Treffer muessen displayName='{probe}' haben");
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn entities_unknown_sort_field_does_not_crash() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let q = r#"query($t:String!){
        entities(entityType:$t, page:1, pageSize:100, sortBy:"frobnicated", sortDir:"asc") {
            totalCount
        }
    }"#;
    let res = exec(q, json!({"t":"customer"}), ctx).await;
    assert!(res.errors.is_empty(), "unbekanntes Sort-Feld darf nicht failen");
    assert!(res.data.into_json().unwrap()["entities"]["totalCount"].as_i64().unwrap() >= 0);
}

// =============================================================================
// Phase 0.7.4 — neuer Enforcement-Pfad
// =============================================================================
//
// Sobald die `permissions`-Tabelle nicht leer ist, wird der Resolver aus
// `auth::resolver::effective` authoritative. Diese Tests fuegen gezielt
// einzelne Permission-Rows ein und verifizieren das Verhalten.

async fn insert_permission_row(
    subject_kind: &str,
    subject_id: &str,
    resource_kind: &str,
    resource_id: &str,
    op: &str,
    effect: &str,
) {
    use sea_orm::{ActiveModelTrait, ActiveValue};
    let _ = setup_for_tests().await;
    let db = server::db::conn();
    server::entity::permissions::ActiveModel {
        id: ActiveValue::NotSet,
        subject_kind: ActiveValue::Set(subject_kind.to_string()),
        subject_id: ActiveValue::Set(subject_id.to_string()),
        resource_kind: ActiveValue::Set(resource_kind.to_string()),
        resource_id: ActiveValue::Set(resource_id.to_string()),
        op: ActiveValue::Set(op.to_string()),
        effect: ActiveValue::Set(effect.to_string()),
        priority: ActiveValue::Set(0),
        tenant_id: ActiveValue::Set(None),
    }
    .insert(&db)
    .await
    .expect("insert permission");
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn new_permissions_resolver_blocks_when_table_populated() {
    boot().await;
    // Eine Permission einfuegen, die *einen anderen* User zustimmt — der
    // eingeloggte admin hat damit keine Allow-Regel und wird abgelehnt,
    // sobald die neue Schicht aktiv ist.
    insert_permission_row(
        "user",
        "u-someone-else",
        "entityType",
        "product",
        "read",
        "allow",
    )
    .await;

    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"query($t:String!){ entities(entityType:$t, page:1, pageSize:1) { totalCount } }"#,
        json!({"t":"product"}),
        ctx,
    )
    .await;
    assert!(!res.errors.is_empty(), "admin darf nach Phase-0.7.4-Switch nicht mehr lesen");
    assert!(res.errors[0].message.contains("forbidden"), "got: {:?}", res.errors[0]);
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn new_permissions_resolver_allows_when_user_has_grant() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let admin_id = ctx.user.as_ref().unwrap().id.clone();
    insert_permission_row(
        "user",
        &admin_id,
        "entityType",
        "product",
        "read",
        "allow",
    )
    .await;

    let res = exec(
        r#"query($t:String!){ entities(entityType:$t, page:1, pageSize:1) { totalCount } }"#,
        json!({"t":"product"}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "admin mit expliziter Permission darf lesen: {:?}", res.errors);
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn my_permissions_lists_rules_for_authenticated_user() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let admin_id = ctx.user.as_ref().unwrap().id.clone();
    insert_permission_row(
        "user",
        &admin_id,
        "entityType",
        "product",
        "read",
        "allow",
    )
    .await;
    insert_permission_row(
        "user",
        "u-someone-else",
        "entityType",
        "product",
        "delete",
        "deny",
    )
    .await;

    let res = exec(
        r#"query { myPermissions { subjectKind subjectId resourceKind resourceId op effect } }"#,
        json!({}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    let perms = v["myPermissions"].as_array().unwrap();
    // Nur die admin-eigene Permission sollte enthalten sein.
    assert_eq!(perms.len(), 1);
    assert_eq!(perms[0]["resourceId"], json!("product"));
    assert_eq!(perms[0]["op"], json!("read"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn why_allowed_returns_trace_for_self() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let admin_id = ctx.user.as_ref().unwrap().id.clone();
    insert_permission_row(
        "user",
        &admin_id,
        "entityType",
        "product",
        "read",
        "allow",
    )
    .await;
    insert_permission_row(
        "user",
        &admin_id,
        "entityProperty",
        "product.price",
        "read",
        "deny",
    )
    .await;

    let q = r#"query($u:String!,$rk:String!,$ri:String!,$op:String!){
        whyAllowed(userId:$u, resourceKind:$rk, resourceId:$ri, op:$op) {
            finalEffect
            rules { subjectId resourceKind resourceId op effect specificity priority }
            note
        }
    }"#;

    // EntityType-Read → Allow gewinnt (nur eine passende Regel auf Score 1).
    let res = exec(
        q,
        json!({"u":admin_id,"rk":"entityType","ri":"product","op":"read"}),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["whyAllowed"]["finalEffect"], json!("allow"));
    let rules = v["whyAllowed"]["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["effect"], json!("allow"));
    assert_eq!(rules[0]["specificity"], json!(1));

    // EntityProperty-Read → Property-Deny (Spez 2) gewinnt vor EntityType-Allow (Spez 1).
    let res = exec(
        q,
        json!({"u":admin_id,"rk":"entityProperty","ri":"product.price","op":"read"}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["whyAllowed"]["finalEffect"], json!("deny"));
    let rules = v["whyAllowed"]["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 2, "beide Regeln sollten matchen");
    // Gewinner ist die spezifischere Property-Deny — also rules[0].
    assert_eq!(rules[0]["effect"], json!("deny"));
    assert_eq!(rules[0]["specificity"], json!(2));
    assert_eq!(rules[1]["specificity"], json!(1));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn audit_log_records_deny_from_legacy_path() {
    boot().await;
    let ctx = login_as("viewer", "viewer").await;
    let viewer_id = ctx.user.as_ref().unwrap().id.clone();

    // viewer hat kein create-Recht — Legacy-Pfad lehnt ab.
    let res = exec(
        r#"mutation($t:String!,$f:JSON!){ createEntity(entityType:$t,fields:$f){ ok } }"#,
        json!({"t":"product","f":{"name":"x","price":1}}),
        ctx,
    )
    .await;
    assert!(!res.errors.is_empty());
    assert!(res.errors[0].message.contains("forbidden"));

    let entries = server::schema::recent_audit_entries(5).await;
    let denied = entries
        .iter()
        .find(|e| {
            e.kind == "deny"
                && e.actor_user_id.as_deref() == Some(viewer_id.as_str())
                && e.op.as_deref() == Some("create")
                && e.resource_id.as_deref() == Some("product")
        })
        .expect("deny-Eintrag fuer viewer/product/create muss existieren");
    assert_eq!(denied.resource_kind.as_deref(), Some("entityType"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn audit_log_records_deny_from_new_resolver_path() {
    boot().await;
    // Permission-Schicht aktivieren, aber nicht fuer admin auf product.
    insert_permission_row(
        "user",
        "u-someone-else",
        "entityType",
        "product",
        "read",
        "allow",
    )
    .await;

    let ctx = login_as("admin", "admin").await;
    let admin_id = ctx.user.as_ref().unwrap().id.clone();
    let res = exec(
        r#"query($t:String!){ entities(entityType:$t, page:1, pageSize:1) { totalCount } }"#,
        json!({"t":"product"}),
        ctx,
    )
    .await;
    assert!(!res.errors.is_empty(), "admin sollte abgelehnt werden");

    let entries = server::schema::recent_audit_entries(5).await;
    assert!(
        entries.iter().any(|e| {
            e.kind == "deny"
                && e.actor_user_id.as_deref() == Some(admin_id.as_str())
                && e.op.as_deref() == Some("read")
        }),
        "deny-Eintrag fuer admin/read aus neuem Resolver-Pfad fehlt: {:?}",
        entries
    );
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn why_allowed_cross_user_requires_admin() {
    boot().await;
    // viewer fragt nach admin → muss verboten sein, weil viewer kein
    // Wildcard-Update-Recht hat.
    let ctx = login_as("viewer", "viewer").await;
    let q = r#"query($u:String!){
        whyAllowed(userId:$u, resourceKind:"entityType", resourceId:"product", op:"read") {
            finalEffect
        }
    }"#;
    let res = exec(q, json!({"u":"u-someone-else"}), ctx).await;
    assert!(!res.errors.is_empty());
    assert!(res.errors[0].message.contains("forbidden"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn my_permissions_is_empty_for_anonymous() {
    boot().await;
    let res = exec(
        r#"query { myPermissions { subjectId } }"#,
        json!({}),
        anon(),
    )
    .await;
    assert!(res.errors.is_empty());
    let v = res.data.into_json().unwrap();
    assert_eq!(v["myPermissions"].as_array().unwrap().len(), 0);
}

// =============================================================================
// Phase 1.6 — Builder-Design-Persistenz
// =============================================================================

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn entity_design_boot_snapshot_exists_for_each_loader_type() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    for et in ["product", "order", "customer"] {
        let res = exec(
            r#"query($t:String!){ entityDesign(entityType:$t){
                entityType version schemaVersion createdBy locked state
            } }"#,
            json!({"t": et}),
            ctx.clone(),
        )
        .await;
        assert!(res.errors.is_empty(), "{} -> {:?}", et, res.errors);
        let v = res.data.into_json().unwrap();
        let d = &v["entityDesign"];
        assert!(!d.is_null(), "Boot-Snapshot fuer '{et}' muss existieren");
        assert_eq!(d["version"], json!(0));
        assert_eq!(d["createdBy"], json!("system"));
        assert_eq!(d["locked"], json!(false));
        // projection.columns muss nicht-leer sein (Loader-Daten projiziert).
        let cols = &d["state"]["projection"]["columns"];
        assert!(
            cols.as_array().map(|a| !a.is_empty()).unwrap_or(false),
            "projection.columns fuer {et} darf nicht leer sein, war: {cols}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn save_entity_design_bumps_version() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    // Boot-Snapshot ist version=0; naechster save erwartet expected_version=0.
    let q = r#"mutation($t:String!,$sv:Int!,$s:JSON!,$e:Int){
        saveEntityDesign(entityType:$t, schemaVersion:$sv, state:$s, expectedVersion:$e){
            ok error
            design { entityType version createdBy }
        }
    }"#;
    let state = json!({
        "schemaVersion": 1,
        "tree": { "nodes": [] },
        "projection": { "columns": [], "settings": null, "editor": null }
    });
    let res = exec(
        q,
        json!({"t":"product","sv":1,"s":state,"e":0}),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["saveEntityDesign"]["ok"], json!(true));
    assert_eq!(v["saveEntityDesign"]["design"]["version"], json!(1));
    assert_ne!(
        v["saveEntityDesign"]["design"]["createdBy"], json!("system"),
        "User-Save darf nicht als 'system' zaehlen"
    );

    // Zweite Save: expectedVersion=1 → wird 2.
    let res = exec(
        q,
        json!({"t":"product","sv":1,"s":state,"e":1}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty());
    assert_eq!(res.data.into_json().unwrap()["saveEntityDesign"]["design"]["version"], json!(2));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn save_entity_design_returns_conflict_on_stale_expected_version() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    let state = json!({"schemaVersion":1,"tree":{"nodes":[]},"projection":{"columns":[]}});
    let q = r#"mutation($t:String!,$sv:Int!,$s:JSON!,$e:Int){
        saveEntityDesign(entityType:$t, schemaVersion:$sv, state:$s, expectedVersion:$e){
            ok error
            conflictCurrent { version }
        }
    }"#;

    // Boot-Snapshot ist version=0; expected=42 ist falsch.
    let res = exec(
        q,
        json!({"t":"product","sv":1,"s":state,"e":42}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["saveEntityDesign"]["ok"], json!(false));
    assert_eq!(
        v["saveEntityDesign"]["error"],
        json!("concurrent_design_modification")
    );
    assert_eq!(v["saveEntityDesign"]["conflictCurrent"]["version"], json!(0));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn revert_entity_design_creates_new_version_with_old_state() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    // Erst eine neue Version anlegen, sodass wir auf version=0 zuruecksetzen koennen.
    let save_q = r#"mutation($t:String!,$sv:Int!,$s:JSON!,$e:Int){
        saveEntityDesign(entityType:$t, schemaVersion:$sv, state:$s, expectedVersion:$e){
            ok design { version state }
        }
    }"#;
    let new_state = json!({
        "schemaVersion": 1,
        "tree": { "nodes": [{"id": 1, "boundField": {"key": "marker"}}] },
        "projection": { "columns": [], "settings": null, "editor": null }
    });
    let save_res = exec(
        save_q,
        json!({"t":"product","sv":1,"s":new_state,"e":0}),
        ctx.clone(),
    )
    .await;
    assert!(save_res.errors.is_empty());
    assert_eq!(
        save_res.data.into_json().unwrap()["saveEntityDesign"]["design"]["version"],
        json!(1)
    );

    // Revert auf version=0 → soll Version 2 mit State von Version 0 anlegen.
    let revert_q = r#"mutation($t:String!,$v:Int!){
        revertEntityDesign(entityType:$t, targetVersion:$v){
            ok design { version state createdBy }
        }
    }"#;
    let res = exec(
        revert_q,
        json!({"t":"product","v":0}),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["revertEntityDesign"]["ok"], json!(true));
    let reverted_version = v["revertEntityDesign"]["design"]["version"].as_i64().unwrap();
    assert_eq!(reverted_version, 2, "Revert muss neue Version anlegen, keine Loeschung");
    // Der createdBy des Revert ist der admin (nicht "system"), auch wenn der
    // Original-State von system stammt.
    assert_ne!(v["revertEntityDesign"]["design"]["createdBy"], json!("system"));
    // State.tree muss leer sein (wie in Version 0).
    let nodes = &v["revertEntityDesign"]["design"]["state"]["tree"]["nodes"];
    assert_eq!(nodes.as_array().map(|a| a.len()).unwrap_or(99), 0);
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn save_entity_design_requires_update_permission() {
    boot().await;
    let ctx = login_as("viewer", "viewer").await;
    let state = json!({"schemaVersion":1,"tree":{"nodes":[]},"projection":{"columns":[]}});
    let res = exec(
        r#"mutation($t:String!,$sv:Int!,$s:JSON!){
            saveEntityDesign(entityType:$t, schemaVersion:$sv, state:$s){
                ok error
            }
        }"#,
        json!({"t":"product","sv":1,"s":state}),
        ctx,
    )
    .await;
    assert!(
        !res.errors.is_empty(),
        "viewer ohne Update-Recht muss abgelehnt werden"
    );
    assert!(res.errors[0].message.contains("forbidden"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn entity_design_at_returns_specific_version() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    // Boot-Snapshot ist 0. Wir wollen explizit version=0 zurueckkriegen.
    let res = exec(
        r#"query($t:String!,$v:Int!){ entityDesignAt(entityType:$t, version:$v){
            version createdBy
        } }"#,
        json!({"t":"product","v":0}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["entityDesignAt"]["version"], json!(0));
    assert_eq!(v["entityDesignAt"]["createdBy"], json!("system"));
}

// =============================================================================
// Phase 2 — Plugin-Manager (Schema + CRUD)
// =============================================================================

/// Minimal valides WASM-Modul (nur Magic + Version). Lädt unter Extism als
/// "leeres" Plugin — keine Exports, also nicht call_function-bar, aber
/// zum Testen von install/list/enable/delete reichts.
const EMPTY_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // \0asm magic
    0x01, 0x00, 0x00, 0x00, // version 1
];

fn empty_wasm_b64() -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(EMPTY_WASM)
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn install_plugin_persists_and_lists() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    let manifest = json!({
        "id": "com.example.smoke",
        "version": "0.1.0",
        "apiVersion": 1,
        "capabilities": { "triggers": ["validate"] },
        "functions": {}
    });

    let res = exec(
        r#"mutation($m:JSON!,$w:String!){
            installPlugin(manifest:$m, wasmBase64:$w) {
                id version enabled
            }
        }"#,
        json!({"m": manifest, "w": empty_wasm_b64()}),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["installPlugin"]["id"], json!("com.example.smoke"));
    assert_eq!(v["installPlugin"]["enabled"], json!(true));

    let list = exec(
        r#"query { plugins { id version enabled } }"#,
        json!({}),
        ctx,
    )
    .await;
    assert!(list.errors.is_empty(), "{:?}", list.errors);
    let arr = list.data.into_json().unwrap()["plugins"]
        .as_array()
        .cloned()
        .unwrap();
    assert!(arr.iter().any(|p| p["id"] == "com.example.smoke"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn install_plugin_rejects_invalid_api_version() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let manifest = json!({
        "id": "com.example.bad",
        "version": "0.1.0",
        "apiVersion": 99,
        "capabilities": {},
        "functions": {}
    });
    let res = exec(
        r#"mutation($m:JSON!,$w:String!){
            installPlugin(manifest:$m, wasmBase64:$w) { id }
        }"#,
        json!({"m": manifest, "w": empty_wasm_b64()}),
        ctx,
    )
    .await;
    assert!(!res.errors.is_empty());
    assert!(res.errors[0].message.contains("invalid_manifest"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn install_plugin_rejects_invalid_wasm() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let manifest = json!({
        "id": "com.example.bad-wasm",
        "version": "0.1.0",
        "apiVersion": 1,
        "capabilities": {},
        "functions": {}
    });
    // Bewusst kaputt: 4 Bytes ohne WASM-Magic.
    use base64::Engine;
    let broken = base64::engine::general_purpose::STANDARD.encode(b"junk");
    let res = exec(
        r#"mutation($m:JSON!,$w:String!){
            installPlugin(manifest:$m, wasmBase64:$w) { id }
        }"#,
        json!({"m": manifest, "w": broken}),
        ctx,
    )
    .await;
    assert!(!res.errors.is_empty());
    assert!(res.errors[0].message.contains("invalid_wasm"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn set_plugin_enabled_toggles() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    // Erst installieren.
    let manifest = json!({
        "id": "com.example.toggle",
        "version": "0.1.0",
        "apiVersion": 1,
        "capabilities": {},
        "functions": {}
    });
    let _ = exec(
        r#"mutation($m:JSON!,$w:String!){ installPlugin(manifest:$m, wasmBase64:$w) { id } }"#,
        json!({"m": manifest, "w": empty_wasm_b64()}),
        ctx.clone(),
    )
    .await;

    let res = exec(
        r#"mutation { setPluginEnabled(id:"com.example.toggle", enabled:false) }"#,
        json!({}),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(res.data.into_json().unwrap()["setPluginEnabled"], json!(true));

    let q = exec(
        r#"query { plugin(id:"com.example.toggle") { enabled } }"#,
        json!({}),
        ctx,
    )
    .await;
    assert!(q.errors.is_empty());
    assert_eq!(q.data.into_json().unwrap()["plugin"]["enabled"], json!(false));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn delete_plugin_removes_it() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let manifest = json!({
        "id": "com.example.gone",
        "version": "0.1.0",
        "apiVersion": 1,
        "capabilities": {},
        "functions": {}
    });
    let _ = exec(
        r#"mutation($m:JSON!,$w:String!){ installPlugin(manifest:$m, wasmBase64:$w) { id } }"#,
        json!({"m": manifest, "w": empty_wasm_b64()}),
        ctx.clone(),
    )
    .await;

    let res = exec(
        r#"mutation { deletePlugin(id:"com.example.gone") }"#,
        json!({}),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(res.data.into_json().unwrap()["deletePlugin"], json!(true));

    let q = exec(
        r#"query { plugin(id:"com.example.gone") { id } }"#,
        json!({}),
        ctx,
    )
    .await;
    assert!(q.data.into_json().unwrap()["plugin"].is_null());
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn plugin_endpoints_require_admin_wildcard() {
    boot().await;
    let ctx = login_as("viewer", "viewer").await;
    let res = exec(
        r#"query { plugins { id } }"#,
        json!({}),
        ctx,
    )
    .await;
    assert!(!res.errors.is_empty(), "viewer darf keine Plugins listen");
    assert!(res.errors[0].message.contains("forbidden"));
}

// =============================================================================
// Phase 1.5 — Implementations-Resolution
// =============================================================================

async fn install_field_type_default(
    entity_type: &str,
    field_type_kind: &str,
    field: &str,
    value: serde_json::Value,
    allowed: serde_json::Value,
) {
    // Settings ueber den Server-internen Helper installieren, damit die
    // Map persistent fuer die Test-Session ist. Da heute kein
    // saveEntitySettings-Endpoint existiert, mutieren wir die installierte
    // EntitySettings direkt im example::*-Slot ueber den Designer-Pfad.
    let _ = setup_for_tests().await;
    server::data::with_settings_mut(entity_type, |s: &mut shared::EntitySettings| {
        let entry = s
            .field_type_defaults
            .entry(field_type_kind.to_string())
            .or_insert_with(shared::FieldTypeDefaults::default);
        // Ein einzelnes Feld setzen — generisch ueber serde_json::Value.
        match field {
            "filter_id" => entry.filter_id = value.as_str().map(String::from),
            "editor_id" => entry.editor_id = value.as_str().map(String::from),
            "formatter_id" => entry.formatter_id = value.as_str().map(String::from),
            "allowed_filter_ids" => {
                entry.allowed_filter_ids = allowed
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default()
            }
            _ => panic!("unbekanntes field: {field}"),
        }
    });
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn resolve_implementation_uses_column_override() {
    boot().await;
    server::data::with_columns_mut("product", |cols: &mut Vec<shared::ColumnMeta>| {
        if let Some(price) = cols.iter_mut().find(|c| c.key == "price") {
            price.filter_id = Some("custom-money-filter".into());
        }
    });

    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"query($t:String!,$p:String!,$r:String!){
            resolveImplementation(entityType:$t, property:$p, registry:$r)
        }"#,
        json!({"t":"product","p":"price","r":"filter"}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(
        res.data.into_json().unwrap()["resolveImplementation"],
        json!("custom-money-filter")
    );
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn resolve_implementation_falls_back_to_field_type_default() {
    boot().await;
    // Boot-Snapshot enthaelt keine Column-Overrides. FieldType-Default fuer
    // "money" setzen.
    install_field_type_default(
        "product",
        "money",
        "filter_id",
        json!("money-range-filter"),
        json!([]),
    )
    .await;

    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"query($t:String!,$p:String!,$r:String!){
            resolveImplementation(entityType:$t, property:$p, registry:$r)
        }"#,
        json!({"t":"product","p":"price","r":"filter"}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(
        res.data.into_json().unwrap()["resolveImplementation"],
        json!("money-range-filter")
    );
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn resolve_implementation_returns_null_without_overrides() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"query($t:String!,$p:String!,$r:String!){
            resolveImplementation(entityType:$t, property:$p, registry:$r)
        }"#,
        json!({"t":"product","p":"price","r":"editor"}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert!(res.data.into_json().unwrap()["resolveImplementation"].is_null());
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn allowed_implementations_includes_default_plus_allowed() {
    boot().await;
    install_field_type_default(
        "product",
        "money",
        "filter_id",
        json!("money-range-filter"),
        json!([]),
    )
    .await;
    install_field_type_default(
        "product",
        "money",
        "allowed_filter_ids",
        json!(""),
        json!(["money-range-filter", "money-exact-filter"]),
    )
    .await;

    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"query($t:String!,$p:String!,$r:String!){
            allowedImplementations(entityType:$t, property:$p, registry:$r)
        }"#,
        json!({"t":"product","p":"price","r":"filter"}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let arr = res.data.into_json().unwrap()["allowedImplementations"]
        .as_array()
        .unwrap()
        .clone();
    assert!(arr.contains(&json!("money-range-filter")));
    assert!(arr.contains(&json!("money-exact-filter")));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn set_implementation_choice_persists_and_resolves() {
    boot().await;
    install_field_type_default(
        "product",
        "money",
        "filter_id",
        json!("money-range-filter"),
        json!([]),
    )
    .await;
    install_field_type_default(
        "product",
        "money",
        "allowed_filter_ids",
        json!(""),
        json!(["money-range-filter", "money-exact-filter"]),
    )
    .await;

    let ctx = login_as("admin", "admin").await;
    let set_res = exec(
        r#"mutation($t:String!,$p:String!,$r:String!,$c:String!){
            setImplementationChoice(entityType:$t, property:$p, registry:$r, chosenId:$c)
        }"#,
        json!({"t":"product","p":"price","r":"filter","c":"money-exact-filter"}),
        ctx.clone(),
    )
    .await;
    assert!(set_res.errors.is_empty(), "{:?}", set_res.errors);
    assert_eq!(set_res.data.into_json().unwrap()["setImplementationChoice"], json!(true));

    // Resolve fragt nach dem User → liefert die persistierte Wahl,
    // nicht den FieldType-Default.
    let resolve = exec(
        r#"query($t:String!,$p:String!,$r:String!){
            resolveImplementation(entityType:$t, property:$p, registry:$r)
        }"#,
        json!({"t":"product","p":"price","r":"filter"}),
        ctx,
    )
    .await;
    assert!(resolve.errors.is_empty(), "{:?}", resolve.errors);
    assert_eq!(
        resolve.data.into_json().unwrap()["resolveImplementation"],
        json!("money-exact-filter")
    );
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn set_implementation_choice_rejects_unknown_id() {
    boot().await;
    install_field_type_default(
        "product",
        "money",
        "filter_id",
        json!("money-range-filter"),
        json!([]),
    )
    .await;

    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"mutation($t:String!,$p:String!,$r:String!,$c:String!){
            setImplementationChoice(entityType:$t, property:$p, registry:$r, chosenId:$c)
        }"#,
        json!({"t":"product","p":"price","r":"filter","c":"some-bogus-id"}),
        ctx,
    )
    .await;
    assert!(!res.errors.is_empty());
    assert!(
        res.errors[0].message.contains("not_allowed_for_property"),
        "got: {:?}", res.errors[0]
    );
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn set_implementation_choice_enforces_choose_permission() {
    boot().await;
    install_field_type_default(
        "product",
        "money",
        "filter_id",
        json!("money-range-filter"),
        json!([]),
    )
    .await;
    install_field_type_default(
        "product",
        "money",
        "allowed_filter_ids",
        json!(""),
        json!(["money-range-filter", "money-exact-filter"]),
    )
    .await;

    // Permissions-Schicht aktivieren mit einem Allow-Eintrag, der NICHT
    // Op::Choose auf filter/money-exact-filter enthaelt — d.h. der admin
    // hat im neuen Modell *keine* Choose-Permission.
    insert_permission_row(
        "user",
        "u-someone-else",
        "entityType",
        "product",
        "read",
        "allow",
    )
    .await;

    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"mutation($t:String!,$p:String!,$r:String!,$c:String!){
            setImplementationChoice(entityType:$t, property:$p, registry:$r, chosenId:$c)
        }"#,
        json!({"t":"product","p":"price","r":"filter","c":"money-exact-filter"}),
        ctx,
    )
    .await;
    assert!(!res.errors.is_empty());
    assert!(res.errors[0].message.contains("forbidden"));
}

// =============================================================================
// Phase 0.7.4-Lueckenschluss — AuthSession.effective + currentEffective
// =============================================================================

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn login_returns_no_effective_when_permissions_table_empty() {
    boot().await;
    let res = exec(
        r#"mutation($u:String!,$p:String!){
            login(username:$u, password:$p) {
                ok session { token effective { resourceKind } }
            }
        }"#,
        json!({"u":"admin","p":"admin"}),
        anon(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["login"]["ok"], json!(true));
    // Legacy-Mode: effective ist null (Client faellt auf permissions zurueck).
    assert!(
        v["login"]["session"]["effective"].is_null(),
        "effective sollte null sein, war: {}",
        v["login"]["session"]["effective"]
    );
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn login_returns_projected_allows_when_permissions_table_populated() {
    boot().await;
    // Admin bekommt einen Allow auf product.read.
    insert_permission_row(
        "user",
        "u-1", // u-1 ist der admin im examples/shop
        "entityType",
        "product",
        "read",
        "allow",
    )
    .await;
    // Plus ein Deny, der NICHT projiziert werden soll.
    insert_permission_row(
        "user",
        "u-1",
        "entityType",
        "product",
        "delete",
        "deny",
    )
    .await;

    let res = exec(
        r#"mutation($u:String!,$p:String!){
            login(username:$u, password:$p) {
                ok session { effective { resourceKind resourceId op } }
            }
        }"#,
        json!({"u":"admin","p":"admin"}),
        anon(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    let effective = v["login"]["session"]["effective"]
        .as_array()
        .expect("effective sollte Liste sein, kein null");
    // Mind. der read-Eintrag muss da sein.
    let has_read = effective.iter().any(|e| {
        e["resourceKind"] == "entityType"
            && e["resourceId"] == "product"
            && e["op"] == "read"
    });
    assert!(has_read, "read-allow muss in effective stehen: {effective:?}");
    // Kein Deny.
    let has_delete = effective
        .iter()
        .any(|e| e["op"] == "delete");
    assert!(!has_delete, "deny-Regel darf nicht in der projizierten Liste sein");
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn current_effective_query_returns_same_shape() {
    boot().await;
    insert_permission_row(
        "user",
        "u-1",
        "entityType",
        "product",
        "read",
        "allow",
    )
    .await;

    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"query { currentEffective { resourceKind resourceId op } }"#,
        json!({}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let arr = res
        .data
        .into_json()
        .unwrap()["currentEffective"]
        .as_array()
        .cloned()
        .unwrap();
    assert!(arr.iter().any(|e| e["op"] == "read"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn current_effective_returns_null_in_legacy_mode() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"query { currentEffective { resourceKind } }"#,
        json!({}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert!(v["currentEffective"].is_null());
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn logout_invalidates_only_this_token() {
    boot().await;
    let ctx_a = login_as("admin", "admin").await;
    let ctx_b = login_as("admin", "admin").await;
    let token_a = ctx_a.token.clone().unwrap();
    let token_b = ctx_b.token.clone().unwrap();
    assert_ne!(token_a, token_b);

    let res = exec(
        r#"mutation{ logout }"#,
        json!({}),
        ctx_a.clone(),
    )
    .await;
    assert!(res.errors.is_empty());
    assert_eq!(res.data.into_json().unwrap()["logout"], json!(true));

    assert!(auth::user_for_bearer(Some(&format!("Bearer {token_a}")))
        .await
        .is_none());
    assert!(auth::user_for_bearer(Some(&format!("Bearer {token_b}")))
        .await
        .is_some());
}

// =============================================================================
// Q0005: Named Views E2E
// =============================================================================

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn entity_view_returns_resolved_default_for_authenticated_user() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    let res = exec(
        r#"query($t:String!){ entityView(entityType:$t) { entityType viewName version } }"#,
        json!({"t": "product"}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    let ev = &v["entityView"];
    assert_eq!(ev["entityType"], json!("product"));
    assert_eq!(ev["viewName"], json!("default"));
    // Version ist die Summe aller Layer-Versionen.
    // F1 seeded version=0 → Summe ist 0.
    assert!(ev["version"].as_i64().is_some());
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn save_entity_view_creates_then_conflicts_on_stale_version() {
    boot().await;
    let ctx = login_as("admin", "admin").await;

    // Erst speichern — kein expectedVersion → wird version=1.
    let res = exec(
        r#"mutation($i:SaveEntityViewInput!){ saveEntityView(input:$i) { kind view { version } message } }"#,
        json!({
            "i": {
                "entityType": "product",
                "viewName": "default",
                "layer": "GLOBAL",
                "payload": { "properties": [], "defaultPageSize": 50 }
            }
        }),
        ctx.clone(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    assert_eq!(v["saveEntityView"]["kind"], json!("OK"));

    // Jetzt mit stale expectedVersion=0 erneut speichern.
    // Die aktuelle Version ist jetzt 1, daher Conflict.
    let res2 = exec(
        r#"mutation($i:SaveEntityViewInput!){ saveEntityView(input:$i) { kind } }"#,
        json!({
            "i": {
                "entityType": "product",
                "viewName": "default",
                "layer": "GLOBAL",
                "expectedVersion": 0,
                "payload": { "properties": [] }
            }
        }),
        ctx,
    )
    .await;
    assert!(res2.errors.is_empty(), "{:?}", res2.errors);
    assert_eq!(res2.data.into_json().unwrap()["saveEntityView"]["kind"], json!("CONFLICT"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn save_entity_view_forbidden_without_auth() {
    boot().await;
    let res = exec(
        r#"mutation($i:SaveEntityViewInput!){ saveEntityView(input:$i) { kind message } }"#,
        json!({
            "i": {
                "entityType": "product",
                "viewName": "default",
                "layer": "GLOBAL",
                "payload": { "properties": [] }
            }
        }),
        anon(),
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    assert_eq!(res.data.into_json().unwrap()["saveEntityView"]["kind"], json!("FORBIDDEN"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn revert_entity_view_removes_user_layer_only() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    // Admin-ID im Shop-Beispiel ist "u-1".
    let admin_id = ctx.user.as_ref().unwrap().id.clone();

    // User-Layer anlegen.
    let save = exec(
        r#"mutation($i:SaveEntityViewInput!){ saveEntityView(input:$i) { kind } }"#,
        json!({
            "i": {
                "entityType": "product",
                "viewName": "default",
                "layer": "USER",
                "ownerId": admin_id,
                "payload": { "properties": [] }
            }
        }),
        ctx.clone(),
    )
    .await;
    assert!(save.errors.is_empty(), "{:?}", save.errors);
    assert_eq!(save.data.into_json().unwrap()["saveEntityView"]["kind"], json!("OK"));

    // User-Layer wieder loeschen (revert).
    let revert = exec(
        r#"mutation($et:String!,$vn:String!,$l:ViewLayer!,$oid:String){
            revertEntityView(entityType:$et, viewName:$vn, layer:$l, ownerId:$oid) { ok }
        }"#,
        json!({
            "et": "product",
            "vn": "default",
            "l": "USER",
            "oid": admin_id
        }),
        ctx.clone(),
    )
    .await;
    assert!(revert.errors.is_empty(), "{:?}", revert.errors);
    assert_eq!(revert.data.into_json().unwrap()["revertEntityView"]["ok"], json!(true));

    // Global-Layer aus F1-Seed bleibt erhalten.
    let q = exec(
        r#"query($t:String!){ entityView(entityType:$t) { viewName entityType } }"#,
        json!({"t": "product"}),
        ctx,
    )
    .await;
    assert!(q.errors.is_empty(), "{:?}", q.errors);
    assert_eq!(q.data.into_json().unwrap()["entityView"]["viewName"], json!("default"));
}

#[tokio::test(flavor = "current_thread")]
#[serial_test::serial]
async fn entity_settings_resolves_via_view_overlay() {
    boot().await;
    let ctx = login_as("admin", "admin").await;
    // F1 seeded entity views aus Loader-Settings; entitySettings laedt
    // ueber resolve_view und liefert ein typisiertes EntitySettings-Objekt.
    let res = exec(
        r#"query($t:String!){ entitySettings(entityType:$t) { entityType access } }"#,
        json!({"t": "product"}),
        ctx,
    )
    .await;
    assert!(res.errors.is_empty(), "{:?}", res.errors);
    let v = res.data.into_json().unwrap();
    let es = &v["entitySettings"];
    // entitySettings ist Some(...) weil F1 einen Global-Layer geseedet hat.
    assert!(!es.is_null(), "entitySettings muss Some sein nach F1-Seed");
    assert_eq!(es["entityType"], json!("product"));
}
