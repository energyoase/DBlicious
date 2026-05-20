use server::source::{boot_registry, registry};

#[tokio::test]
#[serial_test::serial]
async fn boot_registry_creates_local_managed_sqlite() {
    server::fresh_test_setup().await;
    let reg = registry();
    let local = reg.get("local").expect("local source registered");
    assert_eq!(local.kind(), "managed-sqlite");
    assert_eq!(local.name(), "local");
}

#[tokio::test]
#[serial_test::serial]
async fn data_entities_page_routes_via_registry_for_default_binding() {
    server::fresh_test_setup().await;
    // Default-Binding fuer "product" geht ueber "local"-Source — Ergebnis
    // muss identisch zum Direkt-Aufruf der raw-Helper sein.
    let direct = server::data::entities_page_raw(
        "product", 1, 100, None, Default::default(),
    ).await;
    let routed = server::data::entities_page(
        "product", 1, 100, None, Default::default(),
    ).await;
    assert_eq!(direct.total_count as i64, routed.total_count);
    assert_eq!(direct.items.len(), routed.items.len());
}
