//! `dblicious` CLI — Verwaltungs-Frontend fuer das dblicious-Backend.
//!
//! Erste Version: Nutzer- und Gruppenverwaltung. Spaeter koennen weitere
//! Subkommandos (z.B. Schema-Operationen, Seed-Tools) ohne Bruch zur
//! bestehenden Top-Level-Struktur ergaenzt werden.
//!
//! DB-Wahl:
//!   * `--db-url <url>` (Vorrang)
//!   * Env `DBLICIOUS_DATABASE_URL`
//!   * Default `sqlite://./dblicious.db?mode=rwc`
//!
//! Das CLI ruft den Server-Library-Stack (`server::db` + `server::data`) auf,
//! damit Schema-Erzeugung, Seeds und Passwort-Hashing identisch zum Server
//! laufen.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};

use server::{data, db};

const DEFAULT_DB_URL: &str = "sqlite://./dblicious.db?mode=rwc";

#[derive(Parser)]
#[command(
    name = "dblicious",
    about = "Verwaltungs-CLI fuer dblicious",
    version
)]
struct Cli {
    /// SQLite-Verbindung. Default: sqlite://./dblicious.db?mode=rwc
    /// (alternativ via Env DBLICIOUS_DATABASE_URL).
    #[arg(long, global = true)]
    db_url: Option<String>,

    #[command(subcommand)]
    cmd: Top,
}

#[derive(Subcommand)]
enum Top {
    /// Nutzer verwalten
    User {
        #[command(subcommand)]
        cmd: UserCmd,
    },
    /// Gruppen verwalten
    Group {
        #[command(subcommand)]
        cmd: GroupCmd,
    },
    /// Security-Format migrieren: konvertiert das alte
    /// `security/{users,groups}.{json,toml}`-Format in das Phase-0.7-Format
    /// (`permissions.json`, `roles.json`, `role_assignments.json`).
    MigrateSecurity(MigrateSecurityArgs),
}

#[derive(clap::Args)]
struct MigrateSecurityArgs {
    /// Pfad zum Example-Verzeichnis (enthaelt `security/users.*` und
    /// `security/groups.*` sowie `entities/<type>/...`). Default: aktuelles
    /// Arbeitsverzeichnis.
    #[arg(long, default_value = ".")]
    data_dir: String,
    /// Nicht schreiben — nur ausgeben, was geschrieben werden wuerde.
    #[arg(long)]
    dry_run: bool,
    /// Bestehende Ziel-Dateien ueberschreiben statt ablehnen.
    #[arg(long)]
    force: bool,
}

#[derive(Subcommand)]
enum UserCmd {
    /// Neuen Nutzer anlegen
    Create {
        username: String,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        locale: Option<String>,
        /// Passwort direkt mitgeben (sonst interaktiver Prompt).
        #[arg(long)]
        password: Option<String>,
        /// Kein Passwort setzen (Login bleibt blockiert).
        #[arg(long, conflicts_with = "password")]
        no_password: bool,
        /// Eine oder mehrere Gruppen direkt zuweisen (Gruppen-ID).
        #[arg(long = "group", value_name = "GROUP_ID")]
        groups: Vec<String>,
    },
    /// Nutzer loeschen (per Nutzername)
    Delete { username: String },
    /// Alle Nutzer auflisten
    List,
    /// Passwort eines Nutzers (neu) setzen
    SetPassword {
        username: String,
        /// Passwort direkt mitgeben (sonst interaktiver Prompt).
        #[arg(long)]
        password: Option<String>,
    },
    /// Nutzer in eine Gruppe aufnehmen
    Join {
        username: String,
        group_id: String,
    },
    /// Nutzer aus einer Gruppe entfernen
    Leave {
        username: String,
        group_id: String,
    },
}

#[derive(Subcommand)]
enum GroupCmd {
    /// Neue Gruppe anlegen
    Create {
        /// Stabile ID (z.B. g-marketing)
        id: String,
        /// Anzeigename oder Fluent-Schluessel
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
    },
    /// Gruppe loeschen
    Delete { id: String },
    /// Alle Gruppen auflisten
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // DB-URL festlegen, bevor `db::init()` sie liest. CLI-Flag > Env > Default.
    if let Some(url) = cli.db_url.as_deref() {
        std::env::set_var("DBLICIOUS_DATABASE_URL", url);
    } else if std::env::var_os("DBLICIOUS_DATABASE_URL").is_none() {
        std::env::set_var("DBLICIOUS_DATABASE_URL", DEFAULT_DB_URL);
    }

    // Migrate-security braucht keine DB — es ist ein reiner Datei-Konverter.
    // Wir behandeln es darum *vor* dem DB-Init.
    if let Top::MigrateSecurity(args) = &cli.cmd {
        return run_migrate_security(args);
    }

    db::init()
        .await
        .map_err(|e| anyhow!("DB-Init fehlgeschlagen: {e}"))?;
    data::ensure_default_groups()
        .await
        .map_err(|e| anyhow!("Default-Gruppen konnten nicht angelegt werden: {e}"))?;

    match cli.cmd {
        Top::User { cmd } => handle_user(cmd).await,
        Top::Group { cmd } => handle_group(cmd).await,
        Top::MigrateSecurity(_) => unreachable!("migrate-security wurde oben behandelt"),
    }
}

// =============================================================================
// migrate-security
// =============================================================================
//
// Konvertiert das alte security/-Format in das Phase-0.7-Format:
//   - 1 SecurityGroup -> 1 Role (gleiche ID, name_key, description_key)
//   - 1 (group, can_*, entity_type)-Tupel -> n Permissions
//     (subject = Role(group.id), resource = EntityType(name), op = …, Allow)
//   - 1 (user, group_id)-Mitgliedschaft -> 1 RoleAssignment
//     (subject = User(user.id), role_id = group.id)
//
// `entity_type = "*"` aus dem alten Format wird zu einer expliziten
// Permission pro tatsaechlichem Entity-Typ aus `entities/` expandiert,
// damit die neue Resource-Form (kein Wildcard) korrekt funktioniert.

fn run_migrate_security(args: &MigrateSecurityArgs) -> Result<()> {
    let data_dir = std::path::Path::new(&args.data_dir).to_path_buf();
    let security_dir = data_dir.join("security");
    if !security_dir.is_dir() {
        return Err(anyhow!(
            "security/-Verzeichnis nicht gefunden unter '{}'",
            data_dir.display()
        ));
    }

    let groups = read_existing_groups(&security_dir)?;
    let users = read_existing_users(&security_dir)?;
    let entity_types = list_entity_types(&data_dir);
    if entity_types.is_empty() {
        eprintln!(
            "WARN: keine entity-Typen unter '{}' gefunden — Wildcard-Permissions ('*') koennen nicht expandiert werden.",
            data_dir.join("entities").display()
        );
    }

    let mut roles: Vec<shared::auth::Role> = Vec::new();
    let mut permissions: Vec<shared::auth::Permission> = Vec::new();
    let mut role_assignments: Vec<shared::auth::RoleAssignment> = Vec::new();

    for g in &groups {
        roles.push(shared::auth::Role {
            id: g.id.clone(),
            name_key: g.name_key.clone(),
            description_key: g.description_key.clone(),
        });
        for perm in &g.permissions {
            // Welche Entity-Typen sind betroffen?
            let targets: Vec<String> = if perm.entity_type == "*" {
                entity_types.clone()
            } else {
                vec![perm.entity_type.clone()]
            };
            // Welche Ops sind erlaubt?
            let mut ops: Vec<shared::auth::Op> = Vec::new();
            if perm.can_read {
                ops.push(shared::auth::Op::Read);
            }
            if perm.can_create {
                ops.push(shared::auth::Op::Create);
            }
            if perm.can_update {
                ops.push(shared::auth::Op::Update);
            }
            if perm.can_delete {
                ops.push(shared::auth::Op::Delete);
            }
            for target in &targets {
                for &op in &ops {
                    permissions.push(shared::auth::Permission {
                        subject: shared::auth::Subject::Role { id: g.id.clone() },
                        resource: shared::auth::Resource::EntityType { name: target.clone() },
                        op,
                        effect: shared::auth::Effect::Allow,
                        priority: 0,
                        tenant_id: None,
                    });
                }
            }
        }
    }

    for u in &users {
        for gid in &u.group_ids {
            role_assignments.push(shared::auth::RoleAssignment {
                subject: shared::auth::Subject::User { id: u.id.clone() },
                role_id: gid.clone(),
            });
        }
    }

    let perm_json = serde_json::to_string_pretty(&permissions)?;
    let roles_json = serde_json::to_string_pretty(&roles)?;
    let ra_json = serde_json::to_string_pretty(&role_assignments)?;

    let perm_target = security_dir.join("permissions.json");
    let roles_target = security_dir.join("roles.json");
    let ra_target = security_dir.join("role_assignments.json");

    if args.dry_run {
        println!("== dry-run — keine Dateien werden geaendert ==");
        println!("\n# {} ({} Eintraege)", perm_target.display(), permissions.len());
        println!("{perm_json}");
        println!("\n# {} ({} Eintraege)", roles_target.display(), roles.len());
        println!("{roles_json}");
        println!(
            "\n# {} ({} Eintraege)",
            ra_target.display(),
            role_assignments.len()
        );
        println!("{ra_json}");
        return Ok(());
    }

    for path in [&perm_target, &roles_target, &ra_target] {
        if path.exists() && !args.force {
            return Err(anyhow!(
                "'{}' existiert bereits — --force benutzen, um zu ueberschreiben",
                path.display()
            ));
        }
    }

    std::fs::write(&perm_target, &perm_json)
        .with_context(|| format!("schreiben {}", perm_target.display()))?;
    std::fs::write(&roles_target, &roles_json)
        .with_context(|| format!("schreiben {}", roles_target.display()))?;
    std::fs::write(&ra_target, &ra_json)
        .with_context(|| format!("schreiben {}", ra_target.display()))?;

    println!(
        "OK: {} Permissions, {} Rollen, {} Zuweisungen geschrieben.",
        permissions.len(),
        roles.len(),
        role_assignments.len()
    );
    Ok(())
}

fn read_existing_groups(security_dir: &std::path::Path) -> Result<Vec<shared::SecurityGroup>> {
    let path = pick_existing(security_dir, "groups")
        .ok_or_else(|| anyhow!("groups.{{json,toml}} nicht in {} gefunden", security_dir.display()))?;
    parse_file(&path)
}

fn read_existing_users(security_dir: &std::path::Path) -> Result<Vec<shared::SecurityUser>> {
    // SecurityUser fehlt einige Felder, die im Loader-UserSeed sind —
    // wir akzeptieren das einfache Format. Falls keine users-Datei existiert,
    // ist die Mitgliedschafts-Konvertierung leer.
    let Some(path) = pick_existing(security_dir, "users") else {
        return Ok(Vec::new());
    };
    // Datei kann das `UserSeed`-Format haben (password_plain etc.). Wir
    // mappen nur die Felder, die fuer RoleAssignment relevant sind.
    let value: serde_json::Value = parse_file(&path)?;
    let Some(arr) = value.as_array() else {
        return Err(anyhow!("erwartete eine Liste in {}", path.display()));
    };
    let mut out = Vec::new();
    for raw in arr {
        let id = raw
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("user-Eintrag ohne 'id' in {}", path.display()))?
            .to_string();
        let group_ids = raw
            .get("groupIds")
            .or_else(|| raw.get("group_ids"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        out.push(shared::SecurityUser {
            id,
            username: String::new(),
            display_name: String::new(),
            locale: None,
            group_ids,
            active: true,
            password_hash: None,
        });
    }
    Ok(out)
}

fn pick_existing(dir: &std::path::Path, stem: &str) -> Option<std::path::PathBuf> {
    for ext in ["json", "toml"] {
        let p = dir.join(format!("{stem}.{ext}"));
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn parse_file<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("lesen {}", path.display()))?;
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    match ext {
        "json" => serde_json::from_str(&raw)
            .with_context(|| format!("parsen JSON {}", path.display())),
        "toml" => toml_to_t::<T>(&raw)
            .with_context(|| format!("parsen TOML {}", path.display())),
        _ => Err(anyhow!("unbekannte Erweiterung: {}", path.display())),
    }
}

fn toml_to_t<T: serde::de::DeserializeOwned>(s: &str) -> Result<T> {
    // toml::from_str gibt es; wir nutzen aber serde_json als Brueckenformat,
    // um Vec-of-Top-Level zu erlauben (TOML kann das nicht direkt — wir
    // erwarten dort einen Wrapper-Object-Style; fuers Migrations-Skript ist
    // JSON der primaere Pfad).
    Err(anyhow!("TOML-Input fuer migrate-security wird heute nicht unterstuetzt; bitte als JSON liefern. (Input: {} bytes)", s.len()))
}

fn list_entity_types(data_dir: &std::path::Path) -> Vec<String> {
    let entities = data_dir.join("entities");
    let Ok(read) = std::fs::read_dir(&entities) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in read.flatten() {
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out
}

async fn handle_user(cmd: UserCmd) -> Result<()> {
    match cmd {
        UserCmd::Create {
            username,
            display_name,
            locale,
            password,
            no_password,
            groups,
        } => {
            let pw = if no_password {
                None
            } else if let Some(p) = password {
                Some(p)
            } else {
                Some(prompt_new_password()?)
            };
            let user = data::create_user(
                &username,
                display_name.as_deref(),
                locale.as_deref(),
                pw.as_deref(),
            )
            .await
            .map_err(anyhow::Error::msg)?;
            for g in &groups {
                let added = data::add_user_to_group(&username, g)
                    .await
                    .map_err(anyhow::Error::msg)?;
                if added {
                    println!("  + Gruppe '{g}' zugewiesen");
                } else {
                    println!("  . Gruppe '{g}' war bereits zugewiesen");
                }
            }
            let pw_note = if pw.is_none() {
                " (ohne Passwort, Login blockiert)"
            } else {
                ""
            };
            println!(
                "Nutzer '{}' angelegt (id={}, display='{}'){}",
                user.username, user.id, user.display_name, pw_note
            );
            Ok(())
        }
        UserCmd::Delete { username } => {
            let ok = data::delete_user_by_username(&username)
                .await
                .map_err(anyhow::Error::msg)?;
            if ok {
                println!("Nutzer '{username}' geloescht");
            } else {
                println!("Nutzer '{username}' nicht gefunden");
            }
            Ok(())
        }
        UserCmd::List => {
            let users = data::users().await;
            if users.is_empty() {
                println!("(keine Nutzer)");
                return Ok(());
            }
            for u in users {
                let active = if u.active { "*" } else { "-" };
                let has_pw = if u.password_hash.is_some() { "pw" } else { "--" };
                let groups = if u.group_ids.is_empty() {
                    "-".to_string()
                } else {
                    u.group_ids.join(",")
                };
                println!(
                    "[{active}] {has_pw}  {:20} {:24} groups: {}",
                    u.username, u.display_name, groups
                );
            }
            Ok(())
        }
        UserCmd::SetPassword { username, password } => {
            let pw = match password {
                Some(p) => p,
                None => prompt_new_password()?,
            };
            data::set_user_password(&username, &pw)
                .await
                .map_err(anyhow::Error::msg)?;
            println!(
                "Passwort fuer '{username}' aktualisiert. Bestehende Sessions invalidiert."
            );
            Ok(())
        }
        UserCmd::Join { username, group_id } => {
            let added = data::add_user_to_group(&username, &group_id)
                .await
                .map_err(anyhow::Error::msg)?;
            if added {
                println!("'{username}' ist jetzt Mitglied von '{group_id}'");
            } else {
                println!("'{username}' war bereits Mitglied von '{group_id}'");
            }
            Ok(())
        }
        UserCmd::Leave { username, group_id } => {
            let removed = data::remove_user_from_group(&username, &group_id)
                .await
                .map_err(anyhow::Error::msg)?;
            if removed {
                println!("'{username}' aus '{group_id}' entfernt");
            } else {
                println!("'{username}' war nicht Mitglied von '{group_id}'");
            }
            Ok(())
        }
    }
}

async fn handle_group(cmd: GroupCmd) -> Result<()> {
    match cmd {
        GroupCmd::Create {
            id,
            name,
            description,
        } => {
            let group = data::create_group(&id, &name, description.as_deref())
                .await
                .map_err(anyhow::Error::msg)?;
            println!(
                "Gruppe '{}' angelegt (name='{}')",
                group.id, group.name_key
            );
            Ok(())
        }
        GroupCmd::Delete { id } => {
            let ok = data::delete_group(&id)
                .await
                .map_err(anyhow::Error::msg)?;
            if ok {
                println!("Gruppe '{id}' geloescht (Mitgliedschaften entfernt)");
            } else {
                println!("Gruppe '{id}' nicht gefunden");
            }
            Ok(())
        }
        GroupCmd::List => {
            let groups = data::groups().await;
            if groups.is_empty() {
                println!("(keine Gruppen)");
                return Ok(());
            }
            for g in groups {
                let desc = g.description_key.as_deref().unwrap_or("-");
                println!(
                    "  {:14}  name='{}'  desc='{}'  perms={}",
                    g.id,
                    g.name_key,
                    desc,
                    g.permissions.len()
                );
            }
            Ok(())
        }
    }
}

fn prompt_new_password() -> Result<String> {
    let p1 = rpassword::prompt_password("Neues Passwort: ")
        .context("Passwort-Prompt fehlgeschlagen")?;
    if p1.is_empty() {
        return Err(anyhow!("Passwort darf nicht leer sein"));
    }
    let p2 = rpassword::prompt_password("Wiederholen: ")
        .context("Passwort-Prompt fehlgeschlagen")?;
    if p1 != p2 {
        return Err(anyhow!("Passwoerter stimmen nicht ueberein"));
    }
    Ok(p1)
}
