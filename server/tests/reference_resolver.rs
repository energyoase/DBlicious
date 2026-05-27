//! Integrations-Tests fuer die Reference-Label-Resolution (U1 / A2).
//!
//! Testet:
//!   1. `data::entities_page_raw` füllt `reference_labels` korrekt, wenn
//!      `display_field` gesetzt und ein passender Ziel-Datensatz in der DB liegt.
//!   2. Kein Label, wenn `display_field` fehlt (Default-Zustand des Shop-
//!      Beispiels in A2; A4 setzt es).
//!   3. Die GraphQL-`entities`-Query liefert das `referenceLabels`-Feld
//!      (GQL-Durchreichung verbugged nicht still).
//!
//! Die Shop-Seed-Daten eignen sich nicht direkt fuer den Resolver-Test, weil
//! das Shop-Beispiel den `customer`-FK als Objekt speichert (Stand vor A4).
//! Die Tests verwenden eigene Minimal-Entitaeten via GQL `createEntity`.

use async_graphql::{Request, Variables};
use serde_json::{json, Value};
use serial_test::serial;

use server::{build_schema, data, fresh_test_setup, AuthContext};

/// Test-Setup mit Admin-Login
async fn boot() -> AuthContext {
    let _ = fresh_test_setup().await;
    login_as("admin", "admin").await
}

async fn login_as(username: &str, password: &str) -> AuthContext {
    let _ = server::setup_for_tests().await;
    let session = server::auth::login(username, password)
        .await
        .expect("login");
    AuthContext {
        user: Some(server::auth::strip_secret(session.user)),
        token: Some(session.token),
    }
}

async fn exec(query: &str, vars: Value, ctx: AuthContext) -> async_graphql::Response {
    let _ = server::setup_for_tests().await;
    let schema = build_schema();
    let req = Request::new(query)
        .variables(Variables::from_json(vars))
        .data(ctx);
    schema.execute(req).await
}

async fn create_entity(entity_type: &str, id: &str, fields: Value, ctx: AuthContext) {
    let res = exec(
        r#"mutation($t:String!,$id:String,$f:JSON!){
            createEntity(entityType:$t,id:$id,fields:$f){ ok }
        }"#,
        json!({"t": entity_type, "id": id, "f": fields}),
        ctx,
    )
    .await;
    assert!(
        res.errors.is_empty(),
        "createEntity fehlgeschlagen: {:?}",
        res.errors
    );
}

// ---------------------------------------------------------------------------
// 1. entities_page_raw — kein display_field → keine Labels
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn raw_no_display_field_yields_empty_labels() {
    let _ = fresh_test_setup().await;
    // Keine columns fuer einen unbekannten Entity-Typ → resolver findet keine
    // Reference-Spalten → reference_labels bleibt leer.
    let page = data::entities_page_raw("customer", 1, 5, None, Default::default()).await;
    // customer hat kein display_field im Shop-Beispiel (Task A2-Stand)
    // → resolver liefert leere Map (keine Reference-Spalten in customer columns)
    assert!(
        page.reference_labels.is_empty(),
        "Ohne display_field darf reference_labels nicht befuellt sein, ist: {:?}",
        page.reference_labels
    );
}

// ---------------------------------------------------------------------------
// 2. entities_page_raw — mit display_field → Labels aufgeloest
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn raw_with_display_field_resolves_label() {
    let ctx = boot().await;

    // Ziel-Entity (Referenz-Ziel) via GQL anlegen — plain String-Felder
    create_entity(
        "dftest_customer",
        "dcu-001",
        json!({"displayName": "Erika Mustermann"}),
        ctx.clone(),
    )
    .await;

    // Quell-Entity mit plain-String-FK
    create_entity(
        "dftest_order",
        "do-001",
        json!({"customer": "dcu-001"}),
        ctx.clone(),
    )
    .await;

    // Dem installierten Beispiel-Set die ColumnMeta + Settings hinzufuegen,
    // damit der Resolver weiss: `customer` ist ein Reference-Feld auf
    // `dftest_customer`, und `dftest_customer.display_field` = "displayName".
    let customer_col = shared::ColumnMeta {
        key: "customer".into(),
        label_key: "f.customer".into(),
        field_type: shared::FieldType::Reference {
            entity: "dftest_customer".into(),
        },
        sortable: false,
        filterable: false,
        comparator_id: None,
        filter_id: None,
        editor_id: None,
        formatter_id: None,
        action_ids: vec![],
    };

    server::example::mutate(|set| {
        set.entities.insert(
            "dftest_order".into(),
            server::example::EntityTypeSet {
                columns: vec![customer_col],
                editor: None,
                settings: Some(shared::EntitySettings {
                    entity_type: "dftest_order".into(),
                    ..Default::default()
                }),
                seeds: vec![],
            },
        );
        set.entities.insert(
            "dftest_customer".into(),
            server::example::EntityTypeSet {
                columns: vec![],
                editor: None,
                settings: Some(shared::EntitySettings {
                    entity_type: "dftest_customer".into(),
                    display_field: Some("displayName".into()),
                    ..Default::default()
                }),
                seeds: vec![],
            },
        );
    });

    let page = data::entities_page_raw("dftest_order", 1, 20, None, Default::default()).await;
    assert_eq!(page.items.len(), 1, "Genau eine Bestellung erwartet");

    let row_labels = page
        .reference_labels
        .get("do-001")
        .expect("do-001 muss in reference_labels vorhanden sein");
    assert_eq!(
        row_labels.get("customer").map(String::as_str),
        Some("Erika Mustermann"),
        "Label 'Erika Mustermann' fuer customer-FK erwartet"
    );
}

// ---------------------------------------------------------------------------
// 3. GraphQL-Passthrough: referenceLabels-Feld ist vorhanden
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
#[serial]
async fn gql_entities_carries_reference_labels_field() {
    // Wir brauchen einen autentifizierten Aufrufer, weil `entities` immer
    // `require_permission` aufruft. Admin-Login liefert vollen Zugriff.
    let ctx = boot().await;

    // Die `entities`-Query muss `referenceLabels` ohne GraphQL-Error liefern.
    // Shop-customer hat kein display_field in A2 → referenceLabels = {} oder null,
    // aber das Feld selbst muss im Response vorhanden sein.
    let res = exec(
        r#"query($t:String!,$p:Int!,$ps:Int!){
            entities(entityType:$t,page:$p,pageSize:$ps){
                items { id }
                referenceLabels
            }
        }"#,
        json!({"t": "customer", "p": 1, "ps": 5}),
        ctx,
    )
    .await;

    assert!(
        res.errors.is_empty(),
        "Keine GQL-Fehler erwartet: {:?}",
        res.errors
    );
    let v = res.data.into_json().unwrap();
    let rl = &v["entities"]["referenceLabels"];
    assert!(
        rl.is_object() || rl.is_null(),
        "referenceLabels muss Objekt oder null sein, ist: {rl}"
    );
}

// ---------------------------------------------------------------------------
// 4. Shop-Seed-Resolver: order→customer-Label aus echter Shop-Seed (I1)
// ---------------------------------------------------------------------------

/// Verifiziert, dass nach dem Shop-Boot der Resolver fuer die order-Seite
/// korrekte reference_labels liefert. Voraussetzung: customer-FK im Seed ist
/// ein plain-String (nicht ein Objekt) und customer hat display_field="displayName".
#[tokio::test(flavor = "current_thread")]
#[serial]
async fn shop_seed_order_customer_label_resolved() {
    let _ = fresh_test_setup().await;

    // Shop-Seed ist nach fresh_test_setup + init geladen.
    // Order o-0001 referenziert customer cu-2 → displayName "Kunde 2".
    let page = data::entities_page_raw("order", 1, 30, None, Default::default()).await;

    assert!(
        !page.items.is_empty(),
        "Mindestens eine Order muss geladen sein"
    );

    let row_labels = page
        .reference_labels
        .get("o-0001")
        .expect("o-0001 muss in reference_labels vorhanden sein");
    assert_eq!(
        row_labels.get("customer").map(String::as_str),
        Some("Kunde 2"),
        "Label 'Kunde 2' fuer order o-0001 customer-FK (cu-2) erwartet"
    );
}

// ---------------------------------------------------------------------------
// 6. GQL entitySettings liefert displayField (U1-fix Roundtrip-Guard)
// ---------------------------------------------------------------------------

/// Verifiziert, dass `entitySettings.displayField` fuer `customer` den
/// konfigurierten Wert "displayName" liefert — end-to-end ueber GQL.
/// Regress-Guard: stellt sicher, dass `map_settings` das Feld nicht
/// still auf None laesst.
#[tokio::test(flavor = "current_thread")]
#[serial]
async fn gql_settings_carries_display_field_for_customer() {
    let ctx = boot().await;

    let res = exec(
        r#"query($t: String!) {
            entitySettings(entityType: $t) {
                entityType
                displayField
            }
        }"#,
        json!({"t": "customer"}),
        ctx,
    )
    .await;

    assert!(
        res.errors.is_empty(),
        "Keine GQL-Fehler erwartet: {:?}",
        res.errors
    );
    let v = res.data.into_json().unwrap();
    let df = &v["entitySettings"]["displayField"];
    assert_eq!(
        df,
        &json!("displayName"),
        "entitySettings.displayField fuer customer muss 'displayName' sein, ist: {df}"
    );
}
