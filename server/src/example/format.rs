//! Format-Dispatch fuer Beispiel-Dateien.
//!
//! Heute unterstuetzt: `.json`, `.toml`. Weitere Formate (YAML, Skripte) lassen
//! sich durch je einen zusaetzlichen Match-Arm in [`read_typed`] plus einen
//! Eintrag in [`SUPPORTED_EXTS`] ergaenzen — keine Aenderung am Loader noetig.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use serde::de::DeserializeOwned;

/// Reihenfolge der gepruefen Endungen, wenn ein Datei*stamm* (ohne Endung)
/// gesucht wird. Erstes Match gewinnt — daher zuerst die "Standard"-Formate.
pub const SUPPORTED_EXTS: &[&str] = &["json", "toml"];

/// Sucht im Verzeichnis nach `<stem>.<ext>` fuer alle [`SUPPORTED_EXTS`] und
/// liefert den ersten existierenden Pfad. Gibt es mehrere parallel, gewinnt
/// die fruehste Endung in der Liste — der Loader warnt diesbezueglich nicht,
/// das ist nur ein Konfigurations-Hygiene-Hinweis.
pub fn find_file(dir: &Path, stem: &str) -> Option<PathBuf> {
    for ext in SUPPORTED_EXTS {
        let p = dir.join(format!("{stem}.{ext}"));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Liest eine Datei und deserialisiert sie in `T`. Format wird ueber die
/// Datei-Endung gewaehlt.
pub fn read_typed<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let bytes = std::fs::read(path)
        .with_context(|| format!("kann '{}' nicht lesen", path.display()))?;
    match ext.as_str() {
        "json" => serde_json::from_slice(&bytes)
            .with_context(|| format!("JSON-Parse-Fehler in '{}'", path.display())),
        "toml" => {
            let s = std::str::from_utf8(&bytes)
                .with_context(|| format!("'{}' ist kein gueltiges UTF-8", path.display()))?;
            toml::from_str(s)
                .with_context(|| format!("TOML-Parse-Fehler in '{}'", path.display()))
        }
        // Hier waeren spaeter `yaml`/`yml` / `rhai` / ... anzufuegen.
        other => Err(anyhow!(
            "Unbekanntes Format '{other}' fuer '{}'. Unterstuetzt: {:?}",
            path.display(),
            SUPPORTED_EXTS
        )),
    }
}

/// Optional-Variante: keine Datei -> `Ok(None)`.
pub fn read_typed_opt<T: DeserializeOwned>(path: Option<PathBuf>) -> Result<Option<T>> {
    match path {
        Some(p) => read_typed::<T>(&p).map(Some),
        None => Ok(None),
    }
}
