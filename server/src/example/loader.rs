//! Konkreter Loader fuer das `examples/<name>/`-Layout.
//!
//! Die Funktion [`load`] erwartet ein Verzeichnis und liefert ein
//! [`ExampleSet`] zurueck. Fehlende optionale Dateien sind kein Fehler — der
//! Server faehrt dann ohne Navigation, ohne Seed-User etc. hoch (sinnvoll
//! z.B. fuer Tests, die nur Teile brauchen).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use super::format::{find_file, read_typed_opt, SUPPORTED_EXTS};
use super::{EntityTypeSet, ExampleConfig, ExampleSet, ScriptSeed, UserSeed};

/// Sektion `[server]` und `[meta]` aus `config.{toml,json}`.
#[derive(serde::Deserialize)]
struct ConfigFile {
    #[serde(default)]
    server: Option<ConfigServer>,
    #[serde(default)]
    meta: Option<ConfigMeta>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigServer {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    bind: Option<String>,
}

/// Sektion `[meta]` aus `config.{toml,json}` (Q0012 §2.2).
///
/// Vollstaendig optional. Fehlt sie, gilt `dataDirFormat = 0` (vor-Q0012).
#[derive(serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct ConfigMeta {
    /// SemVer-Major des data-dir-Vertrags, gegen den dieses Verzeichnis
    /// geschrieben wurde. Wird gegen `shared::DATA_DIR_FORMAT` verglichen.
    #[serde(default)]
    data_dir_format: Option<u32>,
    /// Optionale Mindest-Server-Version — reine Warn-Schwelle, kein Stopp.
    /// Format: `major.minor.patch` (lex-vergleichbar reicht uns hier nicht;
    /// wir tracen nur, wir parsen es nicht weiter — entkoppelt von SemVer).
    #[serde(default)]
    min_server_version: Option<String>,
}

/// Laed das Beispiel im angegebenen Verzeichnis. Schlaegt fehl, wenn das
/// Verzeichnis selbst nicht existiert; einzelne Sub-Dateien sind aber
/// optional.
pub fn load(dir: &Path) -> Result<ExampleSet> {
    if !dir.is_dir() {
        return Err(anyhow!(
            "Datenverzeichnis '{}' existiert nicht oder ist kein Ordner",
            dir.display()
        ));
    }

    // ---- Config ----
    let config_path = find_file(dir, "config");
    let mut config = ExampleConfig::default();
    if let Some(cfg_file) = read_typed_opt::<ConfigFile>(config_path)? {
        if let Some(server) = cfg_file.server {
            if let Some(name) = server.name {
                config.name = name;
            }
            if let Some(bind) = server.bind {
                config.bind = bind;
            }
        }
    }
    // ---- Loader-Format-Version (Q0012 §2.2) ----
    // Re-parse minimal, um `[meta]` getrennt vom `[server]`-Pfad zu lesen.
    // (Wir koennten das im ersten Read mitnehmen; das hier ist robust gegen
    // spaetere Refactorings der ConfigFile-Struct und entkoppelt den Check
    // explizit als eigene Phase.)
    let meta = {
        let config_path = find_file(dir, "config");
        read_typed_opt::<ConfigFile>(config_path)?
            .and_then(|cf| cf.meta)
            .unwrap_or_default()
    };
    let declared = meta.data_dir_format.unwrap_or(0);
    let supported = shared::DATA_DIR_FORMAT;
    if declared > supported {
        return Err(anyhow!(
            "data-dir '{}' verlangt dataDirFormat = {declared}, dieses Binary unterstuetzt bis {supported}. \
             Aktualisiere das dblicious-Binary (Spec Q0012 §2.2).",
            dir.display()
        ));
    }
    if declared > 0 && declared < supported {
        tracing::warn!(
            "data-dir '{}' verwendet dataDirFormat = {declared}; dieses Binary unterstuetzt bis {supported} (Forward-Compat — laeuft, sollte aber aktualisiert werden).",
            dir.display()
        );
    }
    if let Some(min_ver) = meta.min_server_version.as_deref() {
        let our_ver = env!("CARGO_PKG_VERSION");
        if min_ver != our_ver {
            tracing::warn!(
                "data-dir '{}' deklariert minServerVersion = '{min_ver}', dieses Binary ist {our_ver}. \
                 Es findet keine harte Pruefung statt — bitte selbst verifizieren.",
                dir.display()
            );
        }
    }

    if config.name == "unnamed" {
        if let Some(name) = dir.file_name().and_then(|s| s.to_str()) {
            config.name = name.into();
        }
    }

    // ---- Navigation ----
    let navigation: Vec<shared::NavigationNode> =
        read_typed_opt(find_file(dir, "navigation"))?.unwrap_or_default();

    // ---- Security ----
    let security_dir = dir.join("security");
    let users: Vec<UserSeed> = if security_dir.is_dir() {
        read_typed_opt(find_file(&security_dir, "users"))?.unwrap_or_default()
    } else {
        Vec::new()
    };
    let groups: Vec<shared::SecurityGroup> = if security_dir.is_dir() {
        read_typed_opt(find_file(&security_dir, "groups"))?.unwrap_or_default()
    } else {
        Vec::new()
    };
    // Phase 0.7: neues Permission-Modell. Drei optionale Dateien neben
    // users/groups. Fehlende Dateien sind kein Fehler — bestehende
    // Beispiele ohne diese Dateien fahren unveraendert hoch.
    let permissions: Vec<shared::auth::Permission> = if security_dir.is_dir() {
        read_typed_opt(find_file(&security_dir, "permissions"))?.unwrap_or_default()
    } else {
        Vec::new()
    };
    let roles: Vec<shared::auth::Role> = if security_dir.is_dir() {
        read_typed_opt(find_file(&security_dir, "roles"))?.unwrap_or_default()
    } else {
        Vec::new()
    };
    let role_assignments: Vec<shared::auth::RoleAssignment> = if security_dir.is_dir() {
        read_typed_opt(find_file(&security_dir, "role_assignments"))?.unwrap_or_default()
    } else {
        Vec::new()
    };
    // Validierung: RoleAssignment.subject darf nicht Subject::Role sein —
    // wir modellieren keine Role-Hierarchie.
    for ra in &role_assignments {
        if matches!(ra.subject, shared::auth::Subject::Role { .. }) {
            return Err(anyhow!(
                "RoleAssignment.subject darf nicht 'role' sein (keine Role-Hierarchien): {ra:?}"
            ));
        }
    }

    // ---- Translatables ----
    let translatables_dir = dir.join("translatables");
    let mut translatables = shared::TranslatableBundle {
        languages: Vec::new(),
        entries: Vec::new(),
        values: Vec::new(),
    };
    if translatables_dir.is_dir() {
        translatables.languages =
            read_typed_opt(find_file(&translatables_dir, "languages"))?.unwrap_or_default();
        translatables.entries =
            read_typed_opt(find_file(&translatables_dir, "entries"))?.unwrap_or_default();
        translatables.values =
            read_typed_opt(find_file(&translatables_dir, "values"))?.unwrap_or_default();
    }

    // ---- Entities ----
    let entities_root = dir.join("entities");
    let mut entities: BTreeMap<String, EntityTypeSet> = BTreeMap::new();
    if entities_root.is_dir() {
        let read = std::fs::read_dir(&entities_root)
            .with_context(|| format!("kann '{}' nicht lesen", entities_root.display()))?;
        for entry in read {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let entity_type = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow!("Entity-Ordnername ist kein gueltiges UTF-8"))?;
            let set = load_entity_type(&path)?;
            entities.insert(entity_type, set);
        }
    }

    // ---- Scripts (Q0009 Phase 3.2) ----
    let scripts = load_scripts(dir)?;

    // ---- Sources ----
    let mut sources = crate::source::config::load_from_dir(dir)
        .map_err(|e| anyhow!("sources.toml: {e}"))?
        .sources;
    if !sources.contains_key("local") {
        sources.insert(
            "local".into(),
            crate::source::config::SourceConfig {
                kind: "managed-sqlite".into(),
                url: std::env::var("DBLICIOUS_DATABASE_URL").ok(),
            },
        );
    }

    Ok(ExampleSet {
        root: dir.to_path_buf(),
        config,
        navigation,
        users,
        groups,
        translatables,
        entities,
        permissions,
        roles,
        role_assignments,
        sources,
        scripts,
    })
}

/// Wrapper-Schema fuer `scripts/<id>.manifest.{json,toml}` — buendelt
/// `ScriptKind` und `ScriptManifest`. Bewusst eigenstaendig (statt direkt
/// gegen einen `shared::script::ScriptManifest` zu deserialisieren), damit
/// das `kind`-Feld auf Top-Level liegt; das Manifest darunter ist die
/// gleiche `ScriptManifest`-Wire-Form.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptDescriptorFile {
    /// Tagged enum wie auf der Wire-Form: `{"kind":"provider","slot":...}`
    /// oder `{"kind":"component","entry":"..."}`.
    kind: shared::script::ScriptKind,
    manifest: shared::script::ScriptManifest,
}

fn load_scripts(dir: &Path) -> Result<BTreeMap<String, ScriptSeed>> {
    let scripts_root = dir.join("scripts");
    let mut out: BTreeMap<String, ScriptSeed> = BTreeMap::new();
    if !scripts_root.is_dir() {
        return Ok(out);
    }
    let read = std::fs::read_dir(&scripts_root)
        .with_context(|| format!("kann '{}' nicht lesen", scripts_root.display()))?;
    // Wir scannen einmal nach `.rhai`-Dateien und suchen dann pro Source die
    // dazugehoerige Manifest-Datei.
    for entry in read {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if ext != "rhai" {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Skript-Dateiname kein UTF-8: {}", path.display()))?
            .to_string();
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("Skript-Source nicht lesbar: {}", path.display()))?;

        // Manifest suchen: <stem>.manifest.{json,toml}
        let mut manifest_path = None;
        for cand_ext in SUPPORTED_EXTS {
            let p = scripts_root.join(format!("{stem}.manifest.{cand_ext}"));
            if p.is_file() {
                manifest_path = Some(p);
                break;
            }
        }

        let (manifest, manifest_error, kind) = match manifest_path {
            None => {
                // Kein Manifest: Loader liefert "Draft mit Fehler" weiter —
                // der Seed-Schritt setzt state=Draft + last_error.
                (
                    None,
                    Some(format!("manifest file missing for script '{stem}'")),
                    shared::script::ScriptKind::Component {
                        entry: String::new(),
                    },
                )
            }
            Some(p) => match super::format::read_typed::<ScriptDescriptorFile>(&p) {
                Ok(desc) => (Some(desc.manifest), None, desc.kind),
                Err(e) => (
                    None,
                    Some(format!("{e:#}")),
                    shared::script::ScriptKind::Component {
                        entry: String::new(),
                    },
                ),
            },
        };

        out.insert(
            stem.clone(),
            ScriptSeed {
                id: stem,
                source,
                manifest,
                manifest_error,
                kind,
            },
        );
    }
    Ok(out)
}

fn load_entity_type(dir: &Path) -> Result<EntityTypeSet> {
    let columns: Vec<shared::ColumnMeta> =
        read_typed_opt(find_file(dir, "columns"))?.unwrap_or_default();
    let editor: Option<shared::EditorMeta> = read_typed_opt(find_file(dir, "editor"))?;
    let mut settings: Option<shared::EntitySettings> = read_typed_opt(find_file(dir, "settings"))?;
    let binding: Option<shared::source::EntityBinding> = read_typed_opt(find_file(dir, "binding"))?;
    if let Some(b) = binding {
        let entry = settings.get_or_insert_with(shared::EntitySettings::default);
        entry.binding = Some(b);
    }
    let seeds: Vec<shared::Entity> = read_typed_opt(find_file(dir, "seed"))?.unwrap_or_default();
    Ok(EntityTypeSet {
        columns,
        editor,
        settings,
        seeds,
    })
}
