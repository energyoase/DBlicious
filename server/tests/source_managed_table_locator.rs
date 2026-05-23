//! Phase 0.6 B4: `ManagedSqliteSource` mit `BindingLocator::Table` —
//! echte SQL-Spalten in der eigenen managed-sqlite-DB statt JSON-Blob.

use std::collections::BTreeMap;

use sea_orm::{ConnectionTrait, DbBackend, Statement};
use server::source::managed_sqlite::ManagedSqliteSource;
use server::source::{PageQuery, Source};
use shared::source::{BindingLocator, EntityBinding, EntityId};
use shared::{FilterCriteria, SortDirection};

async fn fresh_managed_with_table(table_ddl: &str) -> ManagedSqliteSource {
    // Frischen Prozess-DB-Slot fuer diesen Test.
    server::fresh_test_setup().await;
    let mut src = ManagedSqliteSource::new("local".into());
    src.init().await.unwrap();
    // CREATE TABLE im gemeinsamen Pool.
    let conn = server::db::conn();
    conn.execute(Statement::from_string(DbBackend::Sqlite, table_ddl))
        .await
        .expect("CREATE TABLE muss klappen");
    src
}

fn widget_binding() -> EntityBinding {
    let mut column_map = BTreeMap::new();
    column_map.insert("id".into(), "id".into());
    column_map.insert("name".into(), "name".into());
    column_map.insert("price".into(), "price".into());
    EntityBinding {
        source: "local".into(),
        locator: BindingLocator::Table {
            table: "phase06_b4_widget".into(),
        },
        primary_key: vec!["id".into()],
        read_only: false,
        column_map,
    }
}

#[tokio::test]
#[serial_test::serial]
async fn managed_sqlite_table_locator_full_crud_roundtrip() {
    let src = fresh_managed_with_table(
        r#"CREATE TABLE phase06_b4_widget (
            id    TEXT NOT NULL PRIMARY KEY,
            name  TEXT NOT NULL,
            price REAL
        )"#,
    )
    .await;
    let binding = widget_binding();

    // create
    let mut fields = serde_json::Map::new();
    fields.insert("id".into(), serde_json::json!("w-1"));
    fields.insert("name".into(), serde_json::json!("Hammer"));
    fields.insert("price".into(), serde_json::json!(9.99));
    let created = src
        .create(&binding, None, fields, None)
        .await
        .expect("create");
    assert_eq!(created.id, "w-1");

    // list_page sieht ihn
    let page = src
        .list_page(
            &binding,
            &PageQuery {
                page: 1,
                page_size: 10,
                sort: None,
                filter: FilterCriteria::default(),
            },
        )
        .await
        .expect("list");
    assert_eq!(page.total_count, 1);
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].fields.get("name").unwrap(),
        &serde_json::json!("Hammer")
    );

    // get — price geht durch SQLite REAL (f32-affinity), wir vergleichen
    // mit Toleranz statt mit JSON-Equality.
    let got = src
        .get(&binding, &EntityId::Single("w-1".into()))
        .await
        .unwrap()
        .expect("get returns Some");
    let got_price = got.fields["price"].as_f64().expect("price ist Number");
    assert!(
        (got_price - 9.99).abs() < 0.01,
        "price ~ 9.99, got {got_price}"
    );

    // update
    let mut patch = serde_json::Map::new();
    patch.insert("price".into(), serde_json::json!(12.50));
    let updated = src
        .update(&binding, &EntityId::Single("w-1".into()), patch, None)
        .await
        .unwrap()
        .unwrap();
    let upd_price = updated.fields["price"].as_f64().unwrap();
    assert!((upd_price - 12.50).abs() < 0.01);

    // delete
    let deleted = src
        .delete(&binding, &EntityId::Single("w-1".into()))
        .await
        .unwrap();
    assert!(deleted);
    let page_after = src
        .list_page(&binding, &PageQuery::default())
        .await
        .unwrap();
    assert_eq!(page_after.total_count, 0);
}

#[tokio::test]
#[serial_test::serial]
async fn managed_sqlite_table_locator_supports_composite_pk_and_sort() {
    // Composite-PK (bank, code) + Sort + Filter testen.
    let src = fresh_managed_with_table(
        r#"CREATE TABLE phase06_b4_account (
            bank TEXT NOT NULL,
            code TEXT NOT NULL,
            name TEXT,
            PRIMARY KEY (bank, code)
        )"#,
    )
    .await;
    let mut cm = BTreeMap::new();
    cm.insert("bank".into(), "bank".into());
    cm.insert("code".into(), "code".into());
    cm.insert("name".into(), "name".into());
    let binding = EntityBinding {
        source: "local".into(),
        locator: BindingLocator::Table {
            table: "phase06_b4_account".into(),
        },
        primary_key: vec!["bank".into(), "code".into()],
        read_only: false,
        column_map: cm,
    };

    for (b, c, n) in &[
        ("DE-A", "001", "Alpha"),
        ("DE-A", "002", "Bravo"),
        ("DE-B", "001", "Charlie"),
    ] {
        let mut fields = serde_json::Map::new();
        fields.insert("bank".into(), serde_json::json!(b));
        fields.insert("code".into(), serde_json::json!(c));
        fields.insert("name".into(), serde_json::json!(n));
        src.create(&binding, None, fields, None).await.unwrap();
    }

    // Sort by name DESC
    let page = src
        .list_page(
            &binding,
            &PageQuery {
                page: 1,
                page_size: 10,
                sort: Some(shared::Sort {
                    field: "name".into(),
                    direction: SortDirection::Desc,
                }),
                filter: FilterCriteria::default(),
            },
        )
        .await
        .unwrap();
    assert_eq!(page.total_count, 3);
    let names: Vec<String> = page
        .items
        .iter()
        .map(|e| e.fields["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["Charlie", "Bravo", "Alpha"]);

    // Composite-PK get
    let id = EntityId::Composite(vec!["DE-A".into(), "002".into()]);
    let got = src.get(&binding, &id).await.unwrap().unwrap();
    assert_eq!(got.fields["name"], serde_json::json!("Bravo"));

    // Composite-PK delete + verify count
    src.delete(&binding, &id).await.unwrap();
    let page = src
        .list_page(&binding, &PageQuery::default())
        .await
        .unwrap();
    assert_eq!(page.total_count, 2);
}
