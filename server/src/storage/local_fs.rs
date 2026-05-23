//! Local-FS-Backend fuer den Storage-Trait (Phase 1.7.8).
//!
//! `key` ist ein relativer Pfad unter `root`. Sub-Verzeichnisse werden
//! beim `put` automatisch angelegt. Sicherheit: `key` darf keine `..`-
//! Komponenten enthalten — sonst Path-Traversal-Risiko.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::fs;

use super::{Storage, StorageError};

pub struct LocalFsStorage {
    root: PathBuf,
}

impl LocalFsStorage {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, key: &str) -> Result<PathBuf, StorageError> {
        // Schutz vor Path-Traversal.
        if key.split(['/', '\\']).any(|p| p == "..") {
            return Err(StorageError::Io(format!("key '{key}' enthaelt '..'")));
        }
        let path: PathBuf = self.root.join(key);
        Ok(path)
    }
}

#[async_trait]
impl Storage for LocalFsStorage {
    fn kind(&self) -> &'static str {
        "local-fs"
    }

    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        let path = self.resolve(key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| StorageError::Io(format!("mkdir: {e}")))?;
        }
        fs::write(&path, bytes)
            .await
            .map_err(|e| StorageError::Io(format!("write: {e}")))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, StorageError> {
        let path = self.resolve(key)?;
        if !Path::new(&path).exists() {
            return Err(StorageError::NotFound(key.to_string()));
        }
        fs::read(&path)
            .await
            .map_err(|e| StorageError::Io(format!("read: {e}")))
    }

    async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        let path = self.resolve(key)?;
        if !Path::new(&path).exists() {
            return Ok(false);
        }
        fs::remove_file(&path)
            .await
            .map_err(|e| StorageError::Io(format!("rm: {e}")))?;
        Ok(true)
    }
}
