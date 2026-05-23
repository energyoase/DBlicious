//! Phase 0.6 B2: PostgresSource + MySqlSource Smoke-Tests.
//!
//! Echte DB-Roundtrips brauchen Container und laufen nicht in CI. Hier
//! pinnen wir die Trait-Wiring + Boot-Erkennung — die SQL-Generator-
//! Logik selbst ist in `source_managed_table_locator.rs` gegen SQLite
//! voll getestet und wird per Backend-Tag wiederverwendet.

use std::collections::BTreeMap;

use server::source::config::SourceConfig;
use server::source::mysql::MySqlSource;
use server::source::postgres::PostgresSource;
use server::source::{Capabilities, Source, SourceError};
use shared::source::{BindingLocator, EntityBinding};

#[test]
fn postgres_capabilities_match_relational_profile() {
    let src = PostgresSource::new("pg".into(), "postgres://nobody@localhost/x".into());
    assert_eq!(src.name(), "pg");
    assert_eq!(src.kind(), "postgres");
    assert_eq!(
        src.capabilities(),
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: false,
            supports_composite_pk: true,
            supports_ddl: false,
        }
    );
}

#[test]
fn mysql_capabilities_match_relational_profile() {
    let src = MySqlSource::new("my".into(), "mysql://nobody@localhost/x".into());
    assert_eq!(src.name(), "my");
    assert_eq!(src.kind(), "mysql");
    assert_eq!(
        src.capabilities(),
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: false,
            supports_composite_pk: true,
            supports_ddl: false,
        }
    );
}

#[tokio::test]
async fn postgres_rejects_non_table_locator() {
    let src = PostgresSource::new("pg".into(), "postgres://nobody@localhost/x".into());
    let binding = EntityBinding {
        source: "pg".into(),
        locator: BindingLocator::GenericEntityRow {
            entity_type: "x".into(),
        },
        primary_key: vec!["id".into()],
        read_only: false,
        column_map: BTreeMap::new(),
    };
    // Wir koennen list_page aufrufen, ohne init() — der Locator-Check
    // greift vor dem Connection-Check.
    let err = src
        .list_page(&binding, &server::source::PageQuery::default())
        .await
        .err()
        .expect("non-table locator muss abgelehnt werden");
    assert!(
        matches!(err, SourceError::UnsupportedLocator(_)),
        "Erwarte UnsupportedLocator, bekam: {err:?}"
    );
}

#[tokio::test]
async fn boot_registry_recognises_postgres_and_mysql_kinds_but_fails_connect() {
    // `boot_registry` ruft `init()` auf jeder Source, was bei nicht-existenten
    // DBs fehlschlaegt. Wir pruefen, dass beide Kinds in der match-Liste sind
    // und dass der Fehler eine DB-Fehlermeldung ist (kein "unknown source").
    let mut cfg = std::collections::BTreeMap::new();
    cfg.insert(
        "pg".into(),
        SourceConfig {
            kind: "postgres".into(),
            url: Some("postgres://does-not-exist:1/nope".into()),
        },
    );
    let res = server::source::boot_registry(&cfg).await;
    let err = res.expect_err("connect to nonexistent postgres muss fehlschlagen");
    assert!(
        !matches!(err, SourceError::Other(ref s) if s.contains("unknown source kind")),
        "PostgresSource muss erkannt werden, nicht als unknown abgewiesen — {err:?}"
    );

    let mut cfg = std::collections::BTreeMap::new();
    cfg.insert(
        "my".into(),
        SourceConfig {
            kind: "mysql".into(),
            url: Some("mysql://does-not-exist:1/nope".into()),
        },
    );
    let res = server::source::boot_registry(&cfg).await;
    let err = res.expect_err("connect to nonexistent mysql muss fehlschlagen");
    assert!(
        !matches!(err, SourceError::Other(ref s) if s.contains("unknown source kind")),
        "MySqlSource muss erkannt werden, nicht als unknown abgewiesen — {err:?}"
    );
}
