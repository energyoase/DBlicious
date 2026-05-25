//! Phase 0.6 B5: Designer-DDL geht durch die Source-Trait-Capability.
//!
//! - `ManagedSqliteSource::apply_schema` legt Tabellen wirklich an (CREATE
//!   TABLE IF NOT EXISTS via bestehenden `ddl::try_apply_schema`-Pfad).
//! - `ForeignSqliteSource::apply_schema` lehnt mit `ReadOnly` ab (Foreign-DBs
//!   verwalten ihr Schema selbst).
//! - Default-Trait-Impl: jede neue Source, die `supports_ddl=false` markiert
//!   und keinen Override liefert, wird automatisch ablehnen.

use sea_orm::{ConnectionTrait, Database, DbBackend, FromQueryResult, JsonValue, Statement};
use server::source::foreign_sqlite::ForeignSqliteSource;
use server::source::managed_sqlite::ManagedSqliteSource;
use server::source::{Source, SourceError};
use shared::{AuditRole, ColumnGenerated, DbColumn, DbColumnType, DbSchema, DbTable, Position};

fn t_col(name: &str, ty: DbColumnType, pk: bool) -> DbColumn {
    DbColumn {
        id: name.into(),
        name: name.into(),
        data_type: ty,
        nullable: !pk,
        primary_key: pk,
        unique: false,
        generated: ColumnGenerated::Never,
        concurrency_token: false,
        default_value: None,
        audit_role: AuditRole::None,
    }
}

fn minimal_schema() -> DbSchema {
    DbSchema {
        id: "phase06_b5".into(),
        name: "phase06_b5_test".into(),
        tables: vec![DbTable {
            id: "t1".into(),
            name: "phase06_b5_widget".into(),
            position: Position::default(),
            columns: vec![
                t_col("id", DbColumnType::Text, true),
                t_col("name", DbColumnType::Text, false),
            ],
        }],
        relations: vec![],
        keys: vec![],
        indices: vec![],
    }
}

#[tokio::test]
async fn managed_sqlite_applies_schema_and_creates_table() {
    // ManagedSqliteSource teilt den Prozess-DB-Slot mit dem Test-Bootstrap;
    // wir reichen den fresh_test_setup() durch und schauen danach im DB
    // nach der Tabelle.
    server::fresh_test_setup().await;
    let mut src = ManagedSqliteSource::new("local".into());
    src.init().await.expect("init managed source");

    assert!(
        src.capabilities().supports_ddl,
        "managed-sqlite muss supports_ddl=true haben"
    );

    let schema = minimal_schema();
    let applied = src.apply_schema(&schema).await.expect("apply schema ok");
    assert_eq!(applied, 1, "genau 1 Tabelle erzeugt");

    // Verifikation: die Tabelle existiert im gemeinsamen Pool.
    let conn = server::db::conn();
    let stmt = Statement::from_string(
        DbBackend::Sqlite,
        "SELECT name FROM sqlite_master WHERE type='table' AND name='phase06_b5_widget'",
    );
    let rows: Vec<serde_json::Value> = JsonValue::find_by_statement(stmt).all(&conn).await.unwrap();
    assert_eq!(rows.len(), 1, "Tabelle phase06_b5_widget muss existieren");
}

#[tokio::test]
async fn foreign_sqlite_rejects_schema_with_read_only() {
    let url = "sqlite::memory:".to_string();
    let conn = Database::connect(sea_orm::ConnectOptions::new(url.clone()))
        .await
        .unwrap();
    // Eine Dummy-Tabelle reicht, damit init() introspect-OK ist.
    conn.execute(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE dummy (id INTEGER PRIMARY KEY)",
    ))
    .await
    .unwrap();

    let mut src = ForeignSqliteSource::new("foreign".into(), url);
    src.init().await.expect("init foreign");

    assert!(
        !src.capabilities().supports_ddl,
        "foreign-sqlite darf supports_ddl=true NICHT haben"
    );

    let err = src
        .apply_schema(&minimal_schema())
        .await
        .expect_err("apply_schema muss bei foreign-sqlite fehlschlagen");
    assert!(
        matches!(err, SourceError::ReadOnly),
        "Erwarte SourceError::ReadOnly, bekam: {err:?}"
    );
}
