//! Trait-Konformitaet von `ManagedSqliteSource`.

use server::source::managed_sqlite::ManagedSqliteSource;
use server::source::{Capabilities, Source};

#[test]
fn managed_sqlite_has_expected_capabilities() {
    let src = ManagedSqliteSource::new("local".into());
    assert_eq!(src.name(), "local");
    assert_eq!(src.kind(), "managed-sqlite");
    let cap = src.capabilities();
    assert_eq!(
        cap,
        Capabilities {
            supports_write: true,
            supports_transactions: true,
            supports_sql_pushdown: true,
            supports_introspection: false,
            supports_composite_pk: false,
        }
    );
}

#[tokio::test]
#[serial_test::serial]
async fn managed_sqlite_init_is_idempotent() {
    // Sauberen Pool sicherstellen, dann zweimal init(). #[serial] weil
    // die Methode auf den globalen db::CONN-Slot zugreift.
    server::db::reset();
    std::env::set_var("DBLICIOUS_DATABASE_URL", "sqlite::memory:");
    let mut src = ManagedSqliteSource::new("local".into());
    src.init().await.expect("init #1");
    src.init().await.expect("init #2");
}

use shared::source::{BindingLocator, EntityBinding};
use shared::FilterCriteria;
use server::source::PageQuery;
use std::collections::BTreeMap;

fn shop_product_binding() -> EntityBinding {
    EntityBinding {
        source: "local".into(),
        locator: BindingLocator::GenericEntityRow { entity_type: "product".into() },
        primary_key: vec!["id".into()],
        read_only: false,
        column_map: BTreeMap::new(),
    }
}

#[tokio::test]
#[serial_test::serial]
async fn managed_sqlite_list_page_returns_seeded_shop_products() {
    server::fresh_test_setup().await;

    let src = ManagedSqliteSource::new("local".into());
    let binding = shop_product_binding();
    let q = PageQuery { page: 1, page_size: 10, sort: None, filter: FilterCriteria::default() };

    let page = src.list_page(&binding, &q).await.expect("list_page");
    assert!(page.total_count > 0, "expected seeded products, got 0");
    assert_eq!(page.page, 1);
    assert_eq!(page.page_size, 10);
}
