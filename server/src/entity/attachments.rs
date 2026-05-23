//! Attachment-Index (Phase 1.7.8).
//!
//! Eine `attachments`-Row verknuepft ein Entity-Instance mit einem Blob
//! im Storage-Backend. Der Blob-Inhalt liegt NICHT in der DB — `blob_ref`
//! ist der Backend-spezifische Pfad (Local-FS: relativer Pfad unter dem
//! Storage-Root; S3 spaeter: Bucket-Key).
//!
//! `hash` ist der SHA-256 des Blobs zum Insert-Zeitpunkt; beim `get`
//! prueft der Service ihn gegen den tatsaechlichen Inhalt
//! (Integrity-Check).

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "attachments")]
pub struct Model {
    /// ULID — vom Service vergeben.
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub entity_type: String,
    #[sea_orm(column_type = "Text")]
    pub entity_id: String,
    /// Schluessel im Storage-Backend (Local-FS: relativer Pfad).
    #[sea_orm(column_type = "Text")]
    pub blob_ref: String,
    /// MIME-Typ — Aufrufer-Verantwortung.
    #[sea_orm(column_type = "Text")]
    pub mime: String,
    /// Originaler Dateiname (UI-Hilfe), nullable wenn Hochlade-API
    /// keinen liefert.
    #[sea_orm(column_type = "Text", nullable)]
    pub filename: Option<String>,
    /// Groesse in Bytes.
    pub size_bytes: i64,
    /// SHA-256 als Hex-String (64 Zeichen).
    #[sea_orm(column_type = "Text")]
    pub hash: String,
    /// ISO-8601 (UTC).
    #[sea_orm(column_type = "Text")]
    pub created_at: String,
    /// User, der hochgeladen hat (Audit).
    #[sea_orm(column_type = "Text", nullable)]
    pub created_by: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub tenant_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
