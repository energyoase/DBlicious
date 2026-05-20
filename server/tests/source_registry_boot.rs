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
