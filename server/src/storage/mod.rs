//! File-Storage-Abstraktion (Phase 1.7.8).
//!
//! Trait-basiert, damit Backends (Local-FS heute; S3/MinIO als Folge-
//! Items) ohne Code-Aenderung an den Konsumenten austauschbar sind.
//!
//! Verbindung zur DB: pro Upload eine `attachments`-Row, die das Entity-
//! Instance referenziert und den `blob_ref` (Backend-Schluessel) +
//! SHA-256-Hash speichert. Beim `get` prueft der Service den Hash gegen
//! den tatsaechlichen Inhalt.

pub mod local_fs;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("not_found: {0}")]
    NotFound(String),
    #[error("hash_mismatch: blob_ref='{blob_ref}' expected='{expected}' actual='{actual}'")]
    HashMismatch {
        blob_ref: String,
        expected: String,
        actual: String,
    },
    #[error("io: {0}")]
    Io(String),
}

/// Datenklasse fuer einen Upload — Aufrufer gibt Inhalt + Metadaten,
/// der Service vergibt blob_ref + Hash.
#[derive(Debug, Clone)]
pub struct UploadInput<'a> {
    pub entity_type: &'a str,
    pub entity_id:   &'a str,
    pub filename:    Option<&'a str>,
    pub mime:        &'a str,
    pub bytes:       &'a [u8],
}

/// Ergebnis eines `put`-Aufrufs.
#[derive(Debug, Clone)]
pub struct UploadResult {
    pub attachment_id: String,
    pub blob_ref:      String,
    pub hash:          String,
    pub size_bytes:    i64,
}

/// Storage-Backend.
#[async_trait]
pub trait Storage: Send + Sync {
    fn kind(&self) -> &'static str;
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError>;
    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError>;
    async fn delete(&self, key: &str) -> Result<bool, StorageError>;
}

/// SHA-256 als 64-Zeichen-Hex-String.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let result = h.finalize();
    let mut out = String::with_capacity(64);
    for b in result {
        use std::fmt::Write;
        let _ = write!(out, "{b:02x}");
    }
    out
}

// =============================================================================
// High-Level Service (verbindet Storage + attachments-Tabelle)
// =============================================================================

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};

use crate::entity::attachments;

#[derive(Debug, Error)]
pub enum AttachmentError {
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("database: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("attachment_not_found: {0}")]
    NotFound(String),
}

pub async fn put(
    conn:    &DatabaseConnection,
    storage: &dyn Storage,
    input:   UploadInput<'_>,
    actor:   Option<&str>,
) -> Result<UploadResult, AttachmentError> {
    let id   = ulid::Ulid::new().to_string();
    let hash = sha256_hex(input.bytes);
    let blob_ref = format!("{}/{}/{}", input.entity_type, input.entity_id, id);
    storage.put(&blob_ref, input.bytes).await?;

    let size = input.bytes.len() as i64;
    let now  = chrono::Utc::now().to_rfc3339();
    attachments::ActiveModel {
        id:          ActiveValue::Set(id.clone()),
        entity_type: ActiveValue::Set(input.entity_type.to_string()),
        entity_id:   ActiveValue::Set(input.entity_id.to_string()),
        blob_ref:    ActiveValue::Set(blob_ref.clone()),
        mime:        ActiveValue::Set(input.mime.to_string()),
        filename:    ActiveValue::Set(input.filename.map(String::from)),
        size_bytes:  ActiveValue::Set(size),
        hash:        ActiveValue::Set(hash.clone()),
        created_at:  ActiveValue::Set(now),
        created_by:  ActiveValue::Set(actor.map(String::from)),
        tenant_id:   ActiveValue::Set(None),
    }
    .insert(conn)
    .await?;

    Ok(UploadResult { attachment_id: id, blob_ref, hash, size_bytes: size })
}

/// Liefert (Bytes, attachments::Model). Prueft den SHA-256-Hash gegen
/// die DB-Spalte — `Err(StorageError::HashMismatch)` wenn der Blob
/// veraendert wurde.
pub async fn get(
    conn:          &DatabaseConnection,
    storage:       &dyn Storage,
    attachment_id: &str,
) -> Result<(Vec<u8>, attachments::Model), AttachmentError> {
    let m = attachments::Entity::find_by_id(attachment_id.to_string())
        .one(conn)
        .await?
        .ok_or_else(|| AttachmentError::NotFound(attachment_id.to_string()))?;
    let bytes = storage.get(&m.blob_ref).await?;
    let actual_hash = sha256_hex(&bytes);
    if actual_hash != m.hash {
        return Err(AttachmentError::Storage(StorageError::HashMismatch {
            blob_ref: m.blob_ref.clone(),
            expected: m.hash.clone(),
            actual:   actual_hash,
        }));
    }
    Ok((bytes, m))
}

pub async fn delete(
    conn:          &DatabaseConnection,
    storage:       &dyn Storage,
    attachment_id: &str,
) -> Result<bool, AttachmentError> {
    let m = attachments::Entity::find_by_id(attachment_id.to_string())
        .one(conn)
        .await?;
    let Some(m) = m else { return Ok(false) };
    storage.delete(&m.blob_ref).await?;
    attachments::Entity::delete_by_id(attachment_id.to_string()).exec(conn).await?;
    Ok(true)
}

pub async fn list_for_entity(
    conn:        &DatabaseConnection,
    entity_type: &str,
    entity_id:   &str,
) -> Result<Vec<attachments::Model>, AttachmentError> {
    Ok(attachments::Entity::find()
        .filter(attachments::Column::EntityType.eq(entity_type))
        .filter(attachments::Column::EntityId.eq(entity_id))
        .all(conn)
        .await?)
}
