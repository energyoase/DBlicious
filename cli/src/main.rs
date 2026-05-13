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

    db::init()
        .await
        .map_err(|e| anyhow!("DB-Init fehlgeschlagen: {e}"))?;
    data::ensure_default_groups()
        .await
        .map_err(|e| anyhow!("Default-Gruppen konnten nicht angelegt werden: {e}"))?;

    match cli.cmd {
        Top::User { cmd } => handle_user(cmd).await,
        Top::Group { cmd } => handle_group(cmd).await,
    }
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
