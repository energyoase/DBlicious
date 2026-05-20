//! Validiert `examples/d2v/`-Bindings gegen das echte `d2v.db`-Schema.
//!
//! Skipped wenn `D2V_LEGACY_URL` nicht gesetzt ist — der Test ist kein
//! CI-Teil, sondern ein lokaler Schema-Drift-Detektor. Setzt
//! `D2V_LEGACY_URL=sqlite:///pfad/zu/d2v-kopie.db` und ruft den Test auf.
//!
//! Findings:
//! - **Missing in DB**: Spalten, die in `binding.json` stehen, in der echten
//!   Tabelle aber fehlen. SQLite-Falle: `SELECT "Foo"` mit nicht-existenter
//!   `Foo` liefert den Literal-String `"Foo"` als Wert — schwer zu sehen,
//!   triggert aber den `"\"CompanyId\"": "CompanyId"`-Bug.
//! - **Unmapped in DB**: Spalten, die in der DB existieren, im Binding aber
//!   nicht referenziert sind. Optional — kein Bug, nur Vollstaendigkeits-Hinweis.

use std::path::{Path, PathBuf};

use sea_orm::Database;

fn d2v_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("d2v")
}

#[tokio::test]
async fn validate_all_bindings_against_real_schema() {
    let Ok(url) = std::env::var("D2V_LEGACY_URL") else {
        eprintln!("D2V_LEGACY_URL nicht gesetzt — Schema-Check skipped.");
        return;
    };

    let db = Database::connect(&url).await.expect("connect d2v.db");
    let schema = server::source::introspect::read_schema(&db)
        .await
        .expect("introspect d2v.db");

    let set = server::example::load(&d2v_dir()).expect("load examples/d2v");

    let mut total_missing = 0usize;
    let mut entities_with_issues = Vec::new();

    for (entity_type, entity_set) in &set.entities {
        let settings = entity_set
            .settings
            .as_ref()
            .unwrap_or_else(|| panic!("{entity_type}: settings fehlt"));
        let binding = settings
            .binding
            .as_ref()
            .unwrap_or_else(|| panic!("{entity_type}: binding fehlt"));

        let shared::source::BindingLocator::Table { table } = &binding.locator else {
            panic!("{entity_type}: erwarte Table-Locator");
        };

        let table_meta = match schema.tables.get(table) {
            Some(m) => m,
            None => {
                eprintln!("\n[{entity_type}] Tabelle '{table}' fehlt in d2v.db!");
                entities_with_issues.push(entity_type.clone());
                continue;
            }
        };

        let db_cols: std::collections::BTreeSet<String> =
            table_meta.columns.iter().map(|c| c.name.clone()).collect();

        let mapped_cols: std::collections::BTreeSet<String> =
            binding.column_map.values().cloned().collect();

        let missing_in_db: Vec<String> = mapped_cols.difference(&db_cols).cloned().collect();
        let unmapped_in_db: Vec<String> =
            db_cols.difference(&mapped_cols).cloned().collect();

        if !missing_in_db.is_empty() || !unmapped_in_db.is_empty() {
            entities_with_issues.push(entity_type.clone());
            eprintln!("\n[{entity_type}] table='{table}'");
            if !missing_in_db.is_empty() {
                eprintln!("  ❌ Im binding.json, NICHT in DB ({} Spalten):", missing_in_db.len());
                for c in &missing_in_db {
                    let wire_key = binding
                        .column_map
                        .iter()
                        .find(|(_, v)| v == &c)
                        .map(|(k, _)| k.clone())
                        .unwrap_or_default();
                    eprintln!("     - {c}  (wire: {wire_key})");
                }
                total_missing += missing_in_db.len();
            }
            if !unmapped_in_db.is_empty() {
                eprintln!("  ℹ  In DB, NICHT im binding.json ({} Spalten):", unmapped_in_db.len());
                for c in &unmapped_in_db {
                    eprintln!("     - {c}");
                }
            }
        }
    }

    eprintln!(
        "\n=== Zusammenfassung: {} Entitaeten mit Diffs, {} Spalten fehlen in DB ===",
        entities_with_issues.len(),
        total_missing
    );
    if !entities_with_issues.is_empty() {
        eprintln!("Entitaeten mit Issues: {}", entities_with_issues.join(", "));
    }

    // Der Test SCHEITERT, wenn binding-Spalten in der DB fehlen — das ist
    // der Bug, den wir suchen. Unmapped-in-DB (extra DB-Spalten) ist nur
    // ein Hinweis, kein Fehler.
    assert_eq!(
        total_missing, 0,
        "{total_missing} Spalten aus binding.json existieren nicht in d2v.db — siehe stderr fuer Details"
    );
}
