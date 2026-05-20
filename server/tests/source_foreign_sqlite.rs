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
