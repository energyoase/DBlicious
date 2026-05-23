//! S3-/MinIO-Backend fuer den [`Storage`]-Trait (Phase 1.7.8 Folge-Item).
//!
//! Nutzt das offizielle `aws-sdk-s3`. MinIO-/anderer S3-kompatibler
//! Server: `endpoint` setzen (`http://localhost:9000`) **und**
//! `force_path_style = true` — sonst versucht die SDK
//! Virtual-Hosted-Buckets (`<bucket>.<host>/...`), was viele Test-
//! Container nicht unterstuetzen.
//!
//! Read/Write/Delete laufen synchron (in Test-Sicht): das `put` haelt
//! den ganzen Buffer im RAM, weil unser `Storage`-Trait `&[u8]`
//! akzeptiert. Fuer multipart-Uploads grosser Dateien (> 100 MB)
//! braeuchte es spaeter einen Streaming-Pfad — out of scope hier.
//!
//! Test-Strategie:
//!   - Konstruktor + Trait-Konformitaet ohne I/O: pruefbar ohne
//!     laufenden S3-Server (siehe `server/tests/storage_s3.rs`).
//!   - Echter Roundtrip: `#[ignore]`-Test, der nur lauft wenn die
//!     env-Variablen `DBLICIOUS_S3_TEST_*` gesetzt sind. So bleibt
//!     `cargo test` ohne MinIO gruen, und der manuelle Smoke ist
//!     reproduzierbar.

use async_trait::async_trait;
use aws_config::Region;
use aws_sdk_s3::{
    config::{Builder as S3ConfigBuilder, Credentials as SdkCredentials},
    error::SdkError,
    operation::get_object::GetObjectError,
    primitives::ByteStream,
    Client,
};

use super::{Storage, StorageError};

/// Konfiguration fuer den S3-/MinIO-Storage.
///
/// `region` ist Pflicht, weil die SDK ohne sie panic'en kann. Fuer MinIO
/// ist der Wert irrelevant (uebliche Konvention: `"us-east-1"`).
#[derive(Debug, Clone)]
pub struct S3Config {
    /// Endpoint-URL. `None` ⇒ echter AWS-S3 mit Region-Routing; `Some`
    /// ⇒ MinIO / anderer S3-kompatibler Server.
    pub endpoint: Option<String>,
    /// Region — auch fuer MinIO Pflicht (siehe Doku oben).
    pub region: String,
    /// Bucket-Name. Muss bereits existieren.
    pub bucket: String,
    /// AWS-AccessKey-ID / MinIO-User.
    pub access_key: String,
    /// AWS-SecretKey / MinIO-Passwort.
    pub secret_key: String,
    /// `true` ⇒ Path-Style (`<endpoint>/<bucket>/<key>`). Notwendig
    /// fuer MinIO und alle SDK-Setups ohne Virtual-Hosted-DNS.
    pub force_path_style: bool,
}

pub struct S3Storage {
    client: Client,
    bucket: String,
}

impl S3Storage {
    /// Baut den Client synchron — `aws-sdk-s3` braucht keinen Async-
    /// Boot, weil die Credentials hier statisch reinkommen. Wer aus
    /// `~/.aws/credentials` lesen will, ruft stattdessen
    /// `aws_config::load_defaults(...)` und uebergibt das Result an
    /// `from_aws_config` (TODO Folge-Item).
    pub fn new(cfg: S3Config) -> Self {
        let creds = SdkCredentials::new(
            cfg.access_key,
            cfg.secret_key,
            None,
            None,
            "dblicious-s3-static",
        );
        let mut builder: S3ConfigBuilder = aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(Region::new(cfg.region))
            .credentials_provider(SdkCredentials::from(creds))
            .force_path_style(cfg.force_path_style);
        if let Some(ep) = cfg.endpoint {
            builder = builder.endpoint_url(ep);
        }
        let client = Client::from_conf(builder.build());
        Self {
            client,
            bucket: cfg.bucket,
        }
    }

    /// Test-Hilfe — liefert den eingestellten Bucket-Namen ohne den
    /// Client zu serialisieren.
    pub fn bucket_name(&self) -> &str {
        &self.bucket
    }
}

#[async_trait]
impl Storage for S3Storage {
    fn kind(&self) -> &'static str {
        "s3"
    }

    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(bytes.to_vec()))
            .send()
            .await
            .map(|_| ())
            .map_err(|e| StorageError::Io(format!("s3_put: {}", display_sdk_error(&e))))
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let out = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| match &e {
                SdkError::ServiceError(svc)
                    if matches!(svc.err(), GetObjectError::NoSuchKey(_)) =>
                {
                    StorageError::NotFound(key.to_string())
                }
                other => StorageError::Io(format!("s3_get: {}", display_sdk_error(other))),
            })?;
        let body = out
            .body
            .collect()
            .await
            .map_err(|e| StorageError::Io(format!("s3_get_body: {e}")))?;
        Ok(body.into_bytes().to_vec())
    }

    async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        // S3 unterscheidet "war da" vs. "war nie da" nicht im Delete —
        // beide Faelle liefern 204. Wir signalisieren `true` immer,
        // wenn der Call durchgeht; konsistenz-Anker ist das Audit-Log
        // in attachments-Pfad (`storage::delete` in `mod.rs` prueft die
        // DB-Row).
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map(|_| true)
            .map_err(|e| StorageError::Io(format!("s3_delete: {}", display_sdk_error(&e))))
    }
}

fn display_sdk_error<E: std::fmt::Display>(e: &E) -> String {
    format!("{e}")
}
