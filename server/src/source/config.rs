//! Loader fuer `sources.toml`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceConfig {
    pub kind: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcesFile {
    #[serde(default)]
    pub sources: BTreeMap<String, SourceConfig>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
}

/// Liest `<dir>/sources.toml`. Existiert die Datei nicht, liefert sie ein
/// leeres `SourcesFile` (kein Fehler — der Boot synthetisiert dann den
/// Default-Eintrag).
pub fn load_from_dir(dir: &Path) -> Result<SourcesFile, ConfigError> {
    let path = dir.join("sources.toml");
    if !path.exists() {
        return Ok(SourcesFile::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    let expanded = expand_env(&raw);
    Ok(toml::from_str(&expanded)?)
}

/// `${VAR:-default}`-Expansion (subset von shell-style).
/// `${VAR}` ohne Default expandiert zu leerem String, wenn unset.
pub(crate) fn expand_env(input: &str) -> String {
    let re = regex::Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)(?::-([^}]*))?\}").expect("regex");
    re.replace_all(input, |caps: &regex::Captures<'_>| {
        let var = &caps[1];
        let default = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        std::env::var(var).unwrap_or_else(|_| default.to_string())
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_returns_empty() {
        let tmp = tempdir_for_test();
        let f = load_from_dir(tmp.path()).unwrap();
        assert!(f.sources.is_empty());
    }

    #[test]
    fn parses_two_sources() {
        let tmp = tempdir_for_test();
        std::fs::write(
            tmp.path().join("sources.toml"),
            r#"
[sources.local]
kind = "managed-sqlite"
url  = "sqlite::memory:"

[sources.d2v_legacy]
kind = "foreign-sqlite"
url  = "sqlite:///pfad/zur/d2v.db"
            "#,
        ).unwrap();
        let f = load_from_dir(tmp.path()).unwrap();
        assert_eq!(f.sources.len(), 2);
        assert_eq!(f.sources["local"].kind, "managed-sqlite");
        assert_eq!(f.sources["d2v_legacy"].kind, "foreign-sqlite");
    }

    #[test]
    fn env_var_expansion_with_default() {
        std::env::remove_var("DBLICIOUS_TEST_VAR");
        let expanded = expand_env("url = \"${DBLICIOUS_TEST_VAR:-fallback}\"");
        assert!(expanded.contains("fallback"));
    }

    #[test]
    fn env_var_expansion_from_env() {
        std::env::set_var("DBLICIOUS_TEST_VAR2", "from_env");
        let expanded = expand_env("url = \"${DBLICIOUS_TEST_VAR2:-fallback}\"");
        assert!(expanded.contains("from_env"));
    }

    fn tempdir_for_test() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }
}
