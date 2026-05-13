//! Library-Facade des Servers — wird von Integration-Tests konsumiert.
//!
//! Die Binary (`main.rs`) bleibt der "echte" Einstieg; diese `lib.rs` macht
//! die innere Modul-Struktur fuer `tests/`-Verzeichnisse erreichbar und
//! exponiert eine `build_schema()`-Funktion plus die DB-Init.

pub mod audit;
pub mod auth;
pub mod data;
pub mod db;
pub mod ddl;
pub mod entity;
pub mod example;
pub mod schema;

use async_graphql::Schema;
pub use schema::{MutationRoot, QueryRoot};

pub type AppSchema = Schema<QueryRoot, MutationRoot, async_graphql::EmptySubscription>;

/// Auth-Kontext, identisch zur Binary-Definition. Wird per
/// `Request::data()` an jeden Resolver gereicht.
#[derive(Clone, Default)]
pub struct AuthContext {
    pub user: Option<shared::SecurityUser>,
    pub token: Option<String>,
}

/// Baut ein frisches Schema. Setzt voraus, dass `db::init().await`
/// schon gelaufen ist — `data::*`/`auth::*` greifen unbedingt auf den
/// SeaORM-Pool zu.
pub fn build_schema() -> AppSchema {
    Schema::build(QueryRoot, MutationRoot, async_graphql::EmptySubscription).finish()
}

/// Installiert das im Repo mitgelieferte `examples/shop/`-Beispiel, falls
/// noch keines geladen ist. Idempotent — wenn der Slot bereits gefuellt ist,
/// passiert nichts. Wird ausschliesslich von Tests benutzt; der produktive
/// Pfad geht ueber `server`-Binary + `--data-dir`.
fn install_test_example() {
    if example::current().is_some() {
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("shop");
    let set = example::load(&dir)
        .unwrap_or_else(|e| panic!("examples/shop fuer Tests laden: {e:#}"));
    example::install(set);
}

/// Bequemer Test-Setup-Helper: installiert das mitgelieferte Beispiel,
/// initialisiert die DB (idempotent) und liefert ein Schema. Mehrfachaufrufe
/// innerhalb desselben Tests sind ein no-op. Zwischen Tests muss
/// `db::reset()` aufgerufen werden — siehe [`fresh_test_setup`].
pub async fn setup_for_tests() -> AppSchema {
    install_test_example();
    let _ = db::init().await;
    let _ = data::rehydrate_db_schema().await;
    build_schema()
}

/// Erzwingt einen frischen, leeren `sqlite::memory:`-Pool und initialisiert
/// Schema + Seed neu. Tests rufen das am Test-Anfang auf.
pub async fn fresh_test_setup() -> AppSchema {
    db::reset();
    setup_for_tests().await
}
