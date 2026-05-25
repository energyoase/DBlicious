//! Phase 1.7.8 Folge-Item: S3-/MinIO-Storage-Backend.
//!
//! Zwei Test-Klassen:
//!   1. Ohne I/O — Konstruktor + Trait-Konformitaet. Laeuft immer.
//!   2. Echter Roundtrip gegen MinIO — `#[ignore]`, nur aktiv wenn die
//!      env-Variablen gesetzt sind. Manuell:
//!      $env:DBLICIOUS_S3_TEST_ENDPOINT="http://localhost:9000"
//!      $env:DBLICIOUS_S3_TEST_BUCKET="dblicious-test"
//!      $env:DBLICIOUS_S3_TEST_ACCESS_KEY="minioadmin"
//!      $env:DBLICIOUS_S3_TEST_SECRET_KEY="minioadmin"
//!      cargo test -p server --test storage_s3 -- --ignored

use server::storage::{
    s3::{S3Config, S3Storage},
    Storage,
};

fn dummy_config() -> S3Config {
    S3Config {
        endpoint: Some("http://localhost:9000".into()),
        region: "us-east-1".into(),
        bucket: "dblicious-test".into(),
        access_key: "minioadmin".into(),
        secret_key: "minioadmin".into(),
        force_path_style: true,
    }
}

#[test]
fn s3_storage_constructs_without_io() {
    let s = S3Storage::new(dummy_config());
    assert_eq!(s.kind(), "s3");
    assert_eq!(s.bucket_name(), "dblicious-test");
}

#[test]
fn s3_storage_is_object_safe_as_dyn_storage() {
    // Stellt sicher, dass S3Storage als `dyn Storage` benutzbar bleibt
    // (genauso wie LocalFsStorage) — der Service-Layer in storage::mod
    // nimmt `&dyn Storage`.
    let s = S3Storage::new(dummy_config());
    let dyn_ref: &dyn Storage = &s;
    assert_eq!(dyn_ref.kind(), "s3");
}

// ---------------------------------------------------------------------------
// Echter MinIO-Roundtrip — nur mit gesetzten env-Variablen + --ignored.
// ---------------------------------------------------------------------------

fn config_from_env() -> Option<S3Config> {
    let endpoint = std::env::var("DBLICIOUS_S3_TEST_ENDPOINT").ok()?;
    let bucket = std::env::var("DBLICIOUS_S3_TEST_BUCKET").ok()?;
    let access_key = std::env::var("DBLICIOUS_S3_TEST_ACCESS_KEY").ok()?;
    let secret_key = std::env::var("DBLICIOUS_S3_TEST_SECRET_KEY").ok()?;
    Some(S3Config {
        endpoint: Some(endpoint),
        region: std::env::var("DBLICIOUS_S3_TEST_REGION").unwrap_or_else(|_| "us-east-1".into()),
        bucket,
        access_key,
        secret_key,
        force_path_style: true,
    })
}

#[tokio::test]
#[ignore = "braucht laufenden MinIO/S3 + DBLICIOUS_S3_TEST_* env-vars"]
async fn s3_put_get_delete_roundtrip_against_minio() {
    let Some(cfg) = config_from_env() else {
        eprintln!("DBLICIOUS_S3_TEST_* nicht gesetzt — Test uebersprungen");
        return;
    };
    let storage = S3Storage::new(cfg);
    let key = format!("test/roundtrip/{}", ulid::Ulid::new());
    let payload = b"hallo s3 roundtrip";

    storage.put(&key, payload).await.expect("put");
    let got = storage.get(&key).await.expect("get");
    assert_eq!(got, payload);

    let deleted = storage.delete(&key).await.expect("delete");
    assert!(deleted);

    // Nach Delete muss get NotFound liefern.
    let err = storage.get(&key).await.expect_err("get nach delete");
    assert!(
        matches!(err, server::storage::StorageError::NotFound(_)),
        "erwartete NotFound, war {err:?}"
    );
}
