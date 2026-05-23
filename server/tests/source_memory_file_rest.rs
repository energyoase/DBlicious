//! Phase 0.6 B3: MemorySource + FileSource + RestSource Smoke-Tests.
//!
//! - Memory: vollstaendiger CRUD-Roundtrip (kein I/O)
//! - File: vollstaendiger CRUD inkl. Persistierung (tmp-Datei)
//! - Rest: Capabilities + Locator-Recognition; echte HTTP-Roundtrips
//!   wuerden einen Mock-Server brauchen — `boot_registry` erkennt `rest`-
//!   Kind als Marker
use std::collections::BTreeMap;

use server::source::config::SourceConfig;
use server::source::file::FileSource;
use server::source::memory::MemorySource;
use server::source::rest::RestSource;
use server::source::{Capabilities, PageQuery, Source, SourceError};
use shared::source::{BindingLocator, EntityBinding, EntityId};

fn widgets_binding(source_name: &str, table: &str) -> EntityBinding {
    EntityBinding {
        source:      source_name.into(),
        locator:     BindingLocator::Table { table: table.into() },
        primary_key: vec!["id".into()],
        read_only:   false,
        column_map:  BTreeMap::new(),
    }
}

// =============================================================================
// MemorySource
// =============================================================================

#[tokio::test]
async fn memory_source_full_crud_roundtrip() {
    let mut src = MemorySource::new("mem".into());
    src.init().await.unwrap();
    let binding = widgets_binding("mem", "widgets");

    // create
    let mut fields = serde_json::Map::new();
    fields.insert("id".into(),   serde_json::json!("w-1"));
    fields.insert("name".into(), serde_json::json!("Hammer"));
    let created = src.create(&binding, None, fields, None).await.unwrap();
    assert_eq!(created.id, "w-1");

    // list
    let page = src.list_page(&binding, &PageQuery {
        page: 1, page_size: 10, ..PageQuery::default()
    }).await.unwrap();
    assert_eq!(page.total_count, 1);

    // get
    let got = src.get(&binding, &EntityId::Single("w-1".into())).await.unwrap();
    assert_eq!(got.as_ref().unwrap().fields["name"], serde_json::json!("Hammer"));

    // update
    let mut patch = serde_json::Map::new();
    patch.insert("name".into(), serde_json::json!("Big Hammer"));
    let updated = src.update(&binding, &EntityId::Single("w-1".into()), patch, None)
        .await.unwrap().unwrap();
    assert_eq!(updated.fields["name"], serde_json::json!("Big Hammer"));

    // delete
    assert!(src.delete(&binding, &EntityId::Single("w-1".into())).await.unwrap());
    let page = src.list_page(&binding, &PageQuery::default()).await.unwrap();
    assert_eq!(page.total_count, 0);
}

#[tokio::test]
async fn memory_source_seed_helper_works() {
    let src = MemorySource::new("mem".into());
    let mut e = shared::Entity { id: "x".into(), fields: serde_json::Map::new() };
    e.fields.insert("v".into(), serde_json::json!(42));
    src.seed_table("tbl", vec![e]);
    let binding = widgets_binding("mem", "tbl");
    let page = src.list_page(&binding, &PageQuery::default()).await.unwrap();
    assert_eq!(page.total_count, 1);
}

#[tokio::test]
async fn memory_source_read_only_rejects_mutates() {
    let mut src = MemorySource::new("mem".into());
    src.init().await.unwrap();
    let mut binding = widgets_binding("mem", "widgets");
    binding.read_only = true;
    let err = src.create(&binding, None, serde_json::Map::new(), None).await.unwrap_err();
    assert!(matches!(err, SourceError::ReadOnly));
}

#[test]
fn memory_capabilities_match() {
    let src = MemorySource::new("m".into());
    assert_eq!(src.kind(), "memory");
    let c = src.capabilities();
    assert_eq!(c, Capabilities {
        supports_write: true,
        supports_transactions: false,
        supports_sql_pushdown: false,
        supports_introspection: false,
        supports_composite_pk: true,
        supports_ddl: false,
    });
}

// =============================================================================
// FileSource
// =============================================================================

#[tokio::test]
async fn file_source_persists_to_disk_and_reloads() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("widgets.json");
    let binding = widgets_binding("disk", "widgets");

    // 1. Schreibphase: Source legt Datei + Inhalt an
    {
        let mut src = FileSource::new("disk".into(), path.clone());
        src.init().await.unwrap();
        let mut fields = serde_json::Map::new();
        fields.insert("id".into(),    serde_json::json!("w-1"));
        fields.insert("price".into(), serde_json::json!(7.50));
        src.create(&binding, Some("w-1".into()), fields, None).await.unwrap();
    }
    assert!(path.exists(), "FileSource muss die Datei persistiert haben");

    // 2. Reload-Phase: neue Instanz liest die Datei und sieht den Eintrag
    {
        let mut src = FileSource::new("disk".into(), path.clone());
        src.init().await.unwrap();
        let got = src.get(&binding, &EntityId::Single("w-1".into())).await.unwrap();
        let entity = got.expect("Datei muss w-1 enthalten");
        assert_eq!(entity.fields["price"], serde_json::json!(7.50));
    }
}

#[tokio::test]
async fn file_source_init_on_missing_file_is_ok() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("does-not-exist.json");
    let mut src = FileSource::new("disk".into(), path.clone());
    src.init().await.expect("missing file ⇒ empty layout, kein Error");
    let binding = widgets_binding("disk", "widgets");
    let page = src.list_page(&binding, &PageQuery::default()).await.unwrap();
    assert_eq!(page.total_count, 0);
}

// =============================================================================
// RestSource
// =============================================================================

#[test]
fn rest_capabilities_match() {
    let src = RestSource::new("api".into(), "http://localhost:9999".into());
    assert_eq!(src.kind(), "rest");
    let c = src.capabilities();
    assert_eq!(c, Capabilities {
        supports_write: true,
        supports_transactions: false,
        supports_sql_pushdown: false,
        supports_introspection: false,
        supports_composite_pk: true,
        supports_ddl: false,
    });
}

#[tokio::test]
async fn rest_source_uses_rest_endpoint_locator() {
    // Wir testen nur die Locator-Auflösung (URL-Build) ohne echten HTTP-
    // Roundtrip: ein nicht-erreichbarer Port liefert garantiert einen
    // Connect-Error, aber er muss "POST .../my/path" sein.
    let src = RestSource::new("api".into(), "http://127.0.0.1:1/".into());
    let binding = EntityBinding {
        source:      "api".into(),
        locator:     BindingLocator::RestEndpoint { path: "my/path".into() },
        primary_key: vec!["id".into()],
        read_only:   false,
        column_map:  BTreeMap::new(),
    };
    let err = src.create(&binding, None, serde_json::Map::new(), None).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("POST"), "Fehler-Praefix muss POST sein: {msg}");
}

#[tokio::test]
async fn boot_registry_recognises_memory_file_rest() {
    // Memory + file + rest sollten alle erkannt werden; file braucht eine
    // existente Datei oder erlaubt ein leeres Layout (siehe init-Test oben).
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("seed.json").to_string_lossy().to_string();
    let mut cfg = std::collections::BTreeMap::new();
    cfg.insert("m".into(),    SourceConfig { kind: "memory".into(), url: None });
    cfg.insert("f".into(),    SourceConfig { kind: "file".into(),   url: Some(file_path) });
    cfg.insert("r".into(),    SourceConfig { kind: "rest".into(),   url: Some("http://localhost:9".into()) });
    let res = server::source::boot_registry(&cfg).await;
    assert!(res.is_ok(), "Boot mit memory/file/rest muss klappen: {res:?}");
    let reg = server::source::registry();
    assert!(reg.get("m").is_some());
    assert!(reg.get("f").is_some());
    assert!(reg.get("r").is_some());
}
