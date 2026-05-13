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
