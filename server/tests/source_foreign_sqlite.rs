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
