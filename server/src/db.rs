//! DB-Layer (SeaORM + SQLite).
//!
//! Verhalten:
//!   1. `init()` liest `DBLICIOUS_DATABASE_URL`. Ohne Variable wird
//!      `sqlite::memory:` benutzt — alle Daten sind also pro Prozess-Lauf
//!      frisch. Mit `sqlite://<datei>` (oder `sqlite:<datei>`) wird
//!      persistiert.
//!   2. Schema-Erzeugung: `Schema::create_table_from_entity` fuer jede
//!      Entity in `entity::*`. Idempotent: bei existierender Tabelle wird
//!      `CREATE TABLE` mit `IF NOT EXISTS` geflagt und einfach uebersprungen.
//!   3. Seed: leere Tabellen werden aus den JSON-Fixtures bzw. den
//!      hartkodierten Mock-Daten gefuellt.
//!
//! Die `Schema`-Erzeugung produziert nicht alle Defaults — SeaORM emittiert
//! reines CREATE TABLE ohne `IF NOT EXISTS`. Wir wickeln das selbst um, um
//! den Restart-Pfad idempotent zu halten.

use std::sync::OnceLock;

use parking_lot::Mutex;
use sea_orm::{
    sea_query::TableCreateStatement, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, DbBackend, EntityTrait, Schema, Statement,
};

use crate::entity;

/// Globaler Pool-Slot. `parking_lot::Mutex` statt Std-Mutex, damit ein
/// Test-Panic den Mutex nicht poisoned (Std-Mutex sperrt sich danach).
static CONN: OnceLock<Mutex<Option<DatabaseConnection>>> = OnceLock::new();

const DEFAULT_URL: &str = "sqlite::memory:";

fn slot() -> &'static Mutex<Option<DatabaseConnection>> {
    CONN.get_or_init(|| Mutex::new(None))
}

fn database_url() -> String {
    std::env::var("DBLICIOUS_DATABASE_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

pub fn conn() -> DatabaseConnection {
    slot()
        .lock()
        .clone()
        .expect("DB nicht initialisiert — `db::init().await` zuerst aufrufen.")
}

/// Idempotent: legt den Pool genau einmal pro Prozess (oder pro
/// `reset()`-Boundary) an. Mehrfachaufrufe sind no-op.
pub async fn init() -> Result<(), sea_orm::DbErr> {
    if slot().lock().is_some() {
        return Ok(());
    }
    let url = database_url();
    let mut opts = ConnectOptions::new(url.clone());
    opts.sqlx_logging(false);
    let db = Database::connect(opts).await?;
    create_schema(&db).await?;
    seed_if_empty(&db).await?;
    *slot().lock() = Some(db);
    tracing::info!(target: "server::db", "SeaORM-Pool aktiv auf {url}");
    Ok(())
}

/// Setzt den Pool-Slot zurueck. Wird ausschliesslich von Tests aufgerufen,
/// um zwischen Test-Boundaries einen frischen `sqlite::memory:`-Stand zu
/// erzwingen. Im Produktivpfad nie aufrufen.
pub fn reset() {
    *slot().lock() = None;
}

async fn create_schema(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let schema = Schema::new(DbBackend::Sqlite);

    // Reihenfolge ist relevant fuer FK-References (users vor user_groups etc).
    // Wir nehmen die generierten Statements und zwingen `IF NOT EXISTS`,
    // damit Restart kein Fehler ist.
    let mut tables: Vec<TableCreateStatement> = vec![
        schema.create_table_from_entity(entity::entities::Entity),
        schema.create_table_from_entity(entity::sessions::Entity),
        schema.create_table_from_entity(entity::users::Entity),
        schema.create_table_from_entity(entity::groups::Entity),
        schema.create_table_from_entity(entity::user_groups::Entity),
        schema.create_table_from_entity(entity::translatable_languages::Entity),
        schema.create_table_from_entity(entity::translatable_entries::Entity),
        schema.create_table_from_entity(entity::translatable_values::Entity),
        schema.create_table_from_entity(entity::metadata_editor::Entity),
        schema.create_table_from_entity(entity::metadata_settings::Entity),
        schema.create_table_from_entity(entity::db_schemas::Entity),
        // Phase 0.7: neues Permission-Modell — koexistiert mit groups.permissions_json
        // bis 0.7.4/0.7.5 die alte Schicht abloest.
        schema.create_table_from_entity(entity::permissions::Entity),
        schema.create_table_from_entity(entity::roles::Entity),
        schema.create_table_from_entity(entity::role_assignments::Entity),
        // Phase 0.7.6: Audit-Log.
        schema.create_table_from_entity(entity::audit_log::Entity),
        // Phase 1.6: Builder-Design-Persistenz.
        schema.create_table_from_entity(entity::entity_designs::Entity),
        // Phase 1.5.3: Per-User-Wahl der Implementations-IDs.
        schema.create_table_from_entity(entity::user_implementation_choices::Entity),
        // Phase 2.5 + 2.8: Plugin-Storage + Audit-Log.
        schema.create_table_from_entity(entity::plugins::Entity),
        schema.create_table_from_entity(entity::plugin_invocations::Entity),
        // Q0005: Named Views — 1 Row pro (entity_type, view_name, layer, owner_id).
        schema.create_table_from_entity(entity::entity_views::Entity),
    ];
    for t in &mut tables {
        t.if_not_exists();
        let stmt = db.get_database_backend().build(t);
        db.execute(stmt).await?;
    }
    // UNIQUE (entity_type, view_name, layer, owner_id)
    // SeaORM hat hier keine bequeme Builder-API mit NULL-Semantik; raw SQL
    // bleibt sauber.
    let _ = db
        .execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_entity_views \
             ON entity_views(entity_type, view_name, layer, IFNULL(owner_id, ''))",
        )
        .await?;
    Ok(())
}

async fn seed_if_empty(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    use sea_orm::PaginatorTrait;
    // Reihenfolge wichtig: `user_groups` haengt an `users` UND `groups`.
    if entity::groups::Entity::find().count(db).await? == 0 {
        crate::data::seed_groups(db).await?;
    }
    if entity::users::Entity::find().count(db).await? == 0 {
        crate::data::seed_users(db).await?;
    }
    // System-User (Phase 1.6) als Audit-Anker. Idempotent — der Aufruf
    // bleibt no-op, wenn der User bereits existiert.
    crate::data::ensure_system_user(db).await?;
    if entity::translatable_languages::Entity::find()
        .count(db)
        .await?
        == 0
    {
        crate::data::seed_translatables(db).await?;
    }
    if entity::entities::Entity::find().count(db).await? == 0 {
        crate::data::seed_entities_from_example(db).await?;
    }
    // Phase 0.7: neues Permission-Modell. role_assignments referenziert roles
    // (FK) — Reihenfolge: roles vor role_assignments. permissions hat keine FK.
    if entity::roles::Entity::find().count(db).await? == 0 {
        crate::data::seed_roles(db).await?;
    }
    if entity::role_assignments::Entity::find().count(db).await? == 0 {
        crate::data::seed_role_assignments(db).await?;
    }
    if entity::permissions::Entity::find().count(db).await? == 0 {
        crate::data::seed_permissions(db).await?;
    }
    // Phase 1.6: Boot-Snapshot der Builder-Designs aus dem Loader-Set.
    // Idempotent pro entity_type (existierende version=0 bleibt unangefasst).
    crate::data::seed_entity_designs_from_example(db).await?;
    // Phase F (Q0005): Loader-Settings als Default-Global-View materialisieren.
    crate::data::seed_entity_views_from_example(db).await?;
    Ok(())
}

/// Ad-hoc-SQL ausfuehren (DDL-Apply-Pfad). Aequivalent zu
/// `pool.execute(sqlx::query)`. Liefert die Anzahl betroffener Zeilen
/// nur informativ.
pub async fn execute_raw(sql: &str) -> Result<u64, sea_orm::DbErr> {
    let db = conn();
    let stmt = Statement::from_string(db.get_database_backend(), sql.to_string());
    let res = db.execute(stmt).await?;
    Ok(res.rows_affected())
}
