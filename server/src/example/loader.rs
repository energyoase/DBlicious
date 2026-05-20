//! Konkreter Loader fuer das `examples/<name>/`-Layout.
//!
//! Die Funktion [`load`] erwartet ein Verzeichnis und liefert ein
//! [`ExampleSet`] zurueck. Fehlende optionale Dateien sind kein Fehler — der
//! Server faehrt dann ohne Navigation, ohne Seed-User etc. hoch (sinnvoll
//! z.B. fuer Tests, die nur Teile brauchen).

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};

use super::format::{find_file, read_typed_opt};
use super::{EntityTypeSet, ExampleConfig, ExampleSet, UserSeed};

/// Sektion `[server]` aus `config.{toml,json}`.
#[derive(serde::Deserialize)]
struct ConfigFile {
    #[serde(default)]
    server: Option<ConfigServer>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConfigServer {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    bind: Option<String>,
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
        let read = std::fs::read_dir(&entities_root).with_context(|| {
            format!("kann '{}' nicht lesen", entities_root.display())
        })?;
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
    })
}

fn load_entity_type(dir: &Path) -> Result<EntityTypeSet> {
    let columns: Vec<shared::ColumnMeta> =
        read_typed_opt(find_file(dir, "columns"))?.unwrap_or_default();
    let editor: Option<shared::EditorMeta> = read_typed_opt(find_file(dir, "editor"))?;
    let mut settings: Option<shared::EntitySettings> =
        read_typed_opt(find_file(dir, "settings"))?;
    let binding: Option<shared::source::EntityBinding> =
        read_typed_opt(find_file(dir, "binding"))?;
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
