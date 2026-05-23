//! Phase 1.7.8: File-Storage + Attachments.
//!
//! Akzeptanz: Rechnung mit Beleg-PDF speicherbar; Backend per Config
//! waehlbar (hier Local-FS via tempdir); Hash-Check beim Lesen.

use serial_test::serial;
use server::storage::{self, local_fs::LocalFsStorage, Storage, StorageError, UploadInput};

#[tokio::test]
#[serial]
async fn put_get_roundtrip_verifies_hash() {
    server::fresh_test_setup().await;
    let dir = tempfile::tempdir().unwrap();
    let store = LocalFsStorage::new(dir.path());
    let conn = server::db::conn();

    let bytes: &[u8] = b"%PDF-1.4 ... fake PDF ...";
    let res = storage::put(
        &conn,
        &store,
        UploadInput {
            entity_type: "invoice",
            entity_id:   "inv-1",
            filename:    Some("rechnung.pdf"),
            mime:        "application/pdf",
            bytes,
        },
        Some("alice"),
    )
    .await
    .unwrap();
    assert_eq!(res.size_bytes, bytes.len() as i64);
    assert_eq!(res.hash, storage::sha256_hex(bytes));

    let (back, model) = storage::get(&conn, &store, &res.attachment_id).await.unwrap();
    assert_eq!(back, bytes);
    assert_eq!(model.mime,     "application/pdf");
    assert_eq!(model.filename.as_deref(), Some("rechnung.pdf"));
    assert_eq!(model.created_by.as_deref(), Some("alice"));
}

#[tokio::test]
#[serial]
async fn hash_mismatch_when_blob_tampered() {
    server::fresh_test_setup().await;
    let dir = tempfile::tempdir().unwrap();
    let store = LocalFsStorage::new(dir.path());
    let conn = server::db::conn();

    let bytes: &[u8] = b"original-content";
    let res = storage::put(
        &conn,
        &store,
        UploadInput {
            entity_type: "x",
            entity_id:   "y",
            filename:    None,
            mime:        "text/plain",
            bytes,
        },
        None,
    )
    .await
    .unwrap();

    // Tamper: schreibe direkt anderen Inhalt am gleichen Pfad.
    let full = dir.path().join(&res.blob_ref);
    std::fs::write(&full, b"tampered").unwrap();

    let err = storage::get(&conn, &store, &res.attachment_id).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("hash_mismatch"), "got: {msg}");
}

#[tokio::test]
#[serial]
async fn delete_removes_blob_and_row() {
    server::fresh_test_setup().await;
    let dir = tempfile::tempdir().unwrap();
    let store = LocalFsStorage::new(dir.path());
    let conn = server::db::conn();

    let bytes: &[u8] = b"to-delete";
    let res = storage::put(
        &conn,
        &store,
        UploadInput {
            entity_type: "x",
            entity_id:   "y",
            filename:    None,
            mime:        "text/plain",
            bytes,
        },
        None,
    )
    .await
    .unwrap();
    let removed = storage::delete(&conn, &store, &res.attachment_id).await.unwrap();
    assert!(removed);
    // Row weg
    let list = storage::list_for_entity(&conn, "x", "y").await.unwrap();
    assert_eq!(list.len(), 0);
    // Blob weg
    let err = store.get(&res.blob_ref).await.unwrap_err();
    assert!(matches!(err, StorageError::NotFound(_)));
}

#[tokio::test]
#[serial]
async fn delete_unknown_attachment_is_no_op() {
    server::fresh_test_setup().await;
    let dir = tempfile::tempdir().unwrap();
    let store = LocalFsStorage::new(dir.path());
    let conn = server::db::conn();
    let removed = storage::delete(&conn, &store, "no-such-id").await.unwrap();
    assert!(!removed);
}

#[tokio::test]
#[serial]
async fn list_for_entity_isolates_correctly() {
    server::fresh_test_setup().await;
    let dir = tempfile::tempdir().unwrap();
    let store = LocalFsStorage::new(dir.path());
    let conn = server::db::conn();
    let bytes: &[u8] = b"x";
    for et in &["a", "b"] {
        let _ = storage::put(
            &conn,
            &store,
            UploadInput {
                entity_type: et,
                entity_id:   "1",
                filename:    None,
                mime:        "x",
                bytes,
            },
            None,
        )
        .await
        .unwrap();
    }
    assert_eq!(storage::list_for_entity(&conn, "a", "1").await.unwrap().len(), 1);
    assert_eq!(storage::list_for_entity(&conn, "b", "1").await.unwrap().len(), 1);
    assert_eq!(storage::list_for_entity(&conn, "a", "2").await.unwrap().len(), 0);
}

#[test]
fn sha256_hex_known_value() {
    // Bekannter Test-Vektor.
    let h = storage::sha256_hex(b"");
    assert_eq!(h, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[tokio::test]
async fn local_fs_rejects_dotdot_paths() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalFsStorage::new(dir.path());
    let err = store.put("../bad", b"x").await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains(".."), "Path-traversal-Schutz fehlt: {msg}");
}
