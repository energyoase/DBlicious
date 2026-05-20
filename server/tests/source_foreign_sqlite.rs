use server::source::foreign_sqlite::ForeignSqliteSource;
use server::source::{Capabilities, Source};

#[test]
fn foreign_sqlite_capabilities() {
    let src = ForeignSqliteSource::new("d2v_legacy".into(), "sqlite::memory:".into());
    assert_eq!(src.name(), "d2v_legacy");
    assert_eq!(src.kind(), "foreign-sqlite");
    assert_eq!(
        src.capabilities(),
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: true,
            supports_composite_pk: true,
        }
    );
}

use sea_orm::{ConnectOptions, ConnectionTrait, Database, Statement, DbBackend};

async fn build_fixture_db() -> sea_orm::DatabaseConnection {
    let opts = ConnectOptions::new("sqlite::memory:");
    let db = Database::connect(opts).await.unwrap();
    let stmts = vec![
        r#"CREATE TABLE Single (Id INTEGER PRIMARY KEY, Name TEXT NOT NULL)"#,
        r#"CREATE TABLE Composite (
            BankCode TEXT NOT NULL,
            Code     TEXT NOT NULL,
            Balance  REAL,
            PRIMARY KEY (BankCode, Code)
        )"#,
    ];
    for s in stmts {
        db.execute(Statement::from_string(DbBackend::Sqlite, s)).await.unwrap();
    }
    db
}

#[tokio::test]
async fn introspect_reads_single_pk() {
    let db = build_fixture_db().await;
    let schema = server::source::introspect::read_schema(&db).await.unwrap();
    let single = schema.tables.get("Single").expect("table Single");
    assert_eq!(single.primary_key, vec!["Id".to_string()]);
    let col_names: Vec<&str> = single.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(col_names, vec!["Id", "Name"]);
}

#[tokio::test]
async fn introspect_reads_composite_pk_in_correct_order() {
    let db = build_fixture_db().await;
    let schema = server::source::introspect::read_schema(&db).await.unwrap();
    let comp = schema.tables.get("Composite").expect("table Composite");
    assert_eq!(comp.primary_key, vec!["BankCode".to_string(), "Code".to_string()]);
    let col_names: Vec<&str> = comp.columns.iter().map(|c| c.name.as_str()).collect();
    assert!(col_names.contains(&"BankCode"));
    assert!(col_names.contains(&"Balance"));
}

use shared::source::{BindingLocator, EntityBinding};
use std::collections::BTreeMap;

fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos().to_string()
}

async fn fixture_source_with_data() -> (ForeignSqliteSource, sea_orm::DatabaseConnection) {
    // Named shared-memory DB so the source's own connection sees the same data.
    let url = format!("sqlite:file:test_{}?mode=memory&cache=shared", uuid_like());
    let anchor = sea_orm::Database::connect(sea_orm::ConnectOptions::new(url.clone())).await.unwrap();
    let stmts = [
        "CREATE TABLE DatevAccounts (Number INTEGER PRIMARY KEY, Name TEXT NOT NULL)",
        "INSERT INTO DatevAccounts (Number, Name) VALUES (1000, 'Kasse')",
        "INSERT INTO DatevAccounts (Number, Name) VALUES (1200, 'Bank')",
        "INSERT INTO DatevAccounts (Number, Name) VALUES (1800, 'Privat')",
    ];
    for s in stmts {
        anchor.execute(sea_orm::Statement::from_string(sea_orm::DbBackend::Sqlite, s)).await.unwrap();
    }
    let mut src = ForeignSqliteSource::new("d2v_test".into(), url);
    src.init().await.unwrap();
    (src, anchor)
}

fn account_binding() -> EntityBinding {
    let mut map = BTreeMap::new();
    map.insert("number".into(), "Number".into());
    map.insert("name".into(), "Name".into());
    EntityBinding {
        source: "d2v_test".into(),
        locator: BindingLocator::Table { table: "DatevAccounts".into() },
        primary_key: vec!["number".into()],
        read_only: false,
        column_map: map,
    }
}

#[tokio::test]
async fn foreign_list_page_returns_all_rows() {
    let (src, _anchor) = fixture_source_with_data().await;
    let page = src.list_page(&account_binding(), &server::source::PageQuery {
        page: 1, page_size: 10, sort: None, filter: shared::FilterCriteria::default(),
    }).await.unwrap();
    assert_eq!(page.total_count, 3);
    assert_eq!(page.items.len(), 3);
}

#[tokio::test]
async fn foreign_list_page_paginates() {
    let (src, _anchor) = fixture_source_with_data().await;
    let page = src.list_page(&account_binding(), &server::source::PageQuery {
        page: 1, page_size: 2, sort: None, filter: shared::FilterCriteria::default(),
    }).await.unwrap();
    assert_eq!(page.total_count, 3);
    assert_eq!(page.items.len(), 2);
}

#[tokio::test]
async fn foreign_get_single_pk() {
    let (src, _a) = fixture_source_with_data().await;
    let id = shared::source::EntityId::Single("1200".into());
    let e = src.get(&account_binding(), &id).await.unwrap();
    assert!(e.is_some());
    assert_eq!(e.unwrap().fields.get("name").and_then(|v| v.as_str()), Some("Bank"));
}

async fn fixture_composite_pk() -> (ForeignSqliteSource, sea_orm::DatabaseConnection) {
    let url = format!("sqlite:file:test_{}?mode=memory&cache=shared", uuid_like());
    let anchor = sea_orm::Database::connect(sea_orm::ConnectOptions::new(url.clone())).await.unwrap();
    let stmts = [
        "CREATE TABLE StarMoneyAccounts (
            BankCode TEXT NOT NULL,
            Code     TEXT NOT NULL,
            Balance  REAL,
            PRIMARY KEY (BankCode, Code)
        )",
        "INSERT INTO StarMoneyAccounts VALUES ('60050101', '1234', 100.0)",
        "INSERT INTO StarMoneyAccounts VALUES ('60050101', '5678', 200.0)",
    ];
    for s in stmts {
        anchor.execute(sea_orm::Statement::from_string(sea_orm::DbBackend::Sqlite, s)).await.unwrap();
    }
    let mut src = ForeignSqliteSource::new("sm".into(), url);
    src.init().await.unwrap();
    (src, anchor)
}

fn sm_binding() -> EntityBinding {
    let mut map = BTreeMap::new();
    map.insert("bankCode".into(), "BankCode".into());
    map.insert("code".into(), "Code".into());
    map.insert("balance".into(), "Balance".into());
    EntityBinding {
        source: "sm".into(),
        locator: BindingLocator::Table { table: "StarMoneyAccounts".into() },
        primary_key: vec!["bankCode".into(), "code".into()],
        read_only: false,
        column_map: map,
    }
}

#[tokio::test]
async fn foreign_get_composite_pk() {
    let (src, _a) = fixture_composite_pk().await;
    let id = shared::source::EntityId::Composite(vec!["60050101".into(), "5678".into()]);
    let e = src.get(&sm_binding(), &id).await.unwrap();
    assert!(e.is_some());
    let row = e.unwrap();
    assert_eq!(row.fields.get("code").and_then(|v| v.as_str()), Some("5678"));
}
