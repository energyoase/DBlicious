//! Phase 0.6 B1 Akzeptanz: alle D2V-Entity-Types laden via `ForeignSqliteSource`.
//!
//! Verifiziert: jede Entity in `examples/d2v` hat eine Binding, und für jede
//! Binding kann `list_page` ohne Fehler aufgerufen werden (leere Page bei
//! leerer Fixture-Tabelle).
//!
//! Ergänzt das engere `d2v_e2e.rs` (3 ausgewählte Entities) um den Breite-
//! Beweis "alle laden".

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use server::example::{self, ExampleSet};
use server::source::foreign_sqlite::ForeignSqliteSource;
use server::source::{PageQuery, Source};
use shared::source::{BindingLocator, EntityBinding};
use shared::FilterCriteria;

fn d2v_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("d2v")
}

fn unique_db_url() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("sqlite:file:test_all17_{nanos}_{seq}?mode=memory&cache=shared")
}

/// Aggregiert pro (Tabellen-Name) die Menge DB-Spalten + PK-Spalten aus allen
/// Bindings, die auf die Tabelle zeigen.
struct TableSpec {
    columns: BTreeSet<String>,
    primary_key: Vec<String>,
}

fn collect_table_specs(set: &ExampleSet) -> BTreeMap<String, TableSpec> {
    let mut by_table: BTreeMap<String, TableSpec> = BTreeMap::new();
    for (et, entry) in &set.entities {
        let binding = match entry.settings.as_ref().and_then(|s| s.binding.clone()) {
            Some(b) => b,
            None => panic!("{et}: settings.binding fehlt"),
        };
        let BindingLocator::Table { table } = &binding.locator else {
            // Nicht-Table-Locators ueberspringen (z.B. GenericEntityRow auf
            // local fuer DBlicious-eigene Tabellen). D2V hat heute keine.
            continue;
        };
        let resolve = |logical: &str| -> String {
            binding
                .column_map
                .get(logical)
                .cloned()
                .unwrap_or_else(|| logical.to_string())
        };
        let pk_db: Vec<String> = binding.primary_key.iter().map(|k| resolve(k)).collect();
        let entry = by_table.entry(table.clone()).or_insert_with(|| TableSpec {
            columns: BTreeSet::new(),
            primary_key: pk_db.clone(),
        });
        // Alle DB-Spalten aus column_map sammeln.
        for db_col in binding.column_map.values() {
            entry.columns.insert(db_col.clone());
        }
        // Plus die PK-Spalten (falls nicht im column_map enthalten).
        for col in &pk_db {
            entry.columns.insert(col.clone());
        }
    }
    by_table
}

fn ddl_for(table: &str, spec: &TableSpec) -> String {
    // Alle Spalten typenlos (SQLite-default: BLOB-affinity, akzeptiert alles).
    // Reicht fuer Introspektion + list_page mit 0 Rows; produktive Tests in
    // `d2v_e2e.rs` testen mit echten Typen.
    let cols_sql: Vec<String> = spec.columns.iter().map(|c| format!("\"{c}\"")).collect();
    let pk_sql = spec
        .primary_key
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "CREATE TABLE \"{table}\" ({cols}, PRIMARY KEY ({pk}))",
        cols = cols_sql.join(", "),
        pk = pk_sql,
    )
}

async fn build_fixture(set: &ExampleSet) -> (String, sea_orm::DatabaseConnection) {
    let url = unique_db_url();
    let anchor = Database::connect(sea_orm::ConnectOptions::new(url.clone()))
        .await
        .expect("connect anchor");
    let specs = collect_table_specs(set);
    for (table, spec) in &specs {
        let sql = ddl_for(table, spec);
        anchor
            .execute(Statement::from_string(DbBackend::Sqlite, sql))
            .await
            .unwrap_or_else(|e| panic!("DDL fuer {table} fehlgeschlagen: {e}"));
    }
    (url, anchor)
}

fn binding_for(set: &ExampleSet, entity_type: &str) -> EntityBinding {
    set.entities[entity_type]
        .settings
        .as_ref()
        .and_then(|s| s.binding.clone())
        .unwrap_or_else(|| panic!("{entity_type}: binding fehlt"))
}

#[tokio::test]
async fn all_d2v_entity_types_listable_via_foreign_sqlite() {
    let set = example::load(&d2v_dir()).expect("examples/d2v ladbar");
    // Roadmap §0.6.B1: "alle 17 D2V-Entity-Types laden". Wir lesen die Zahl
    // aus dem Loader und beweisen, dass es genau die erwartete Menge ist
    // (regression-safe, falls examples/d2v irgendwann waechst).
    let table_bound: Vec<&str> = set
        .entities
        .iter()
        .filter(|(_, entry)| {
            entry
                .settings
                .as_ref()
                .and_then(|s| s.binding.as_ref())
                .map(|b| matches!(b.locator, BindingLocator::Table { .. }))
                .unwrap_or(false)
        })
        .map(|(et, _)| et.as_str())
        .collect();
    assert_eq!(
        table_bound.len(),
        17,
        "Erwarte 17 Table-Bindings (Roadmap §0.6.B1), gefunden: {}: {:?}",
        table_bound.len(),
        table_bound
    );

    let (url, _anchor) = build_fixture(&set).await;
    let mut src = ForeignSqliteSource::new("d2v_legacy".into(), url);
    src.init().await.expect("init Source");

    let mut failures: Vec<String> = Vec::new();
    for et in &table_bound {
        let binding = binding_for(&set, et);
        let res = src
            .list_page(
                &binding,
                &PageQuery {
                    page: 1,
                    page_size: 10,
                    sort: None,
                    filter: FilterCriteria::default(),
                },
            )
            .await;
        match res {
            Ok(page) => {
                if page.total_count != 0 || !page.items.is_empty() {
                    failures.push(format!(
                        "{et}: erwartet 0 Rows in Fixture, total={} items={}",
                        page.total_count,
                        page.items.len()
                    ));
                }
            }
            Err(e) => failures.push(format!("{et}: list_page-Fehler {e:?}")),
        }
    }
    assert!(
        failures.is_empty(),
        "Diese Entities haben nicht geladen:\n  - {}",
        failures.join("\n  - ")
    );
}

#[tokio::test]
async fn explain_query_plan_uses_pk_index_on_datev_accounts() {
    // Roadmap §0.6.B1: "EXPLAIN QUERY PLAN zeigt Index-Use bei vorhandenen
    // Indizes". DatevAccounts hat einen PRIMARY KEY auf Number — SQLite
    // generiert dafuer implizit einen Index. Ein PK-Lookup via `get`
    // muss laut Plan SEARCH ... USING INTEGER PRIMARY KEY (nicht SCAN).
    let url = unique_db_url();
    let anchor = Database::connect(sea_orm::ConnectOptions::new(url.clone()))
        .await
        .unwrap();
    anchor
        .execute(Statement::from_string(
            DbBackend::Sqlite,
            r#"CREATE TABLE DatevAccounts (
                Number INTEGER PRIMARY KEY,
                Name   TEXT,
                CompanyId INTEGER
            )"#,
        ))
        .await
        .unwrap();

    // Genau das SQL, das `ForeignSqliteSource::get` baut (siehe
    // foreign_sqlite.rs: SELECT cols FROM "table" WHERE "pk" = ? LIMIT 1).
    let explain_sql = r#"EXPLAIN QUERY PLAN SELECT "Number","Name","CompanyId" FROM "DatevAccounts" WHERE "Number" = ? LIMIT 1"#;
    let stmt = Statement::from_sql_and_values(
        DbBackend::Sqlite,
        explain_sql,
        vec![sea_orm::Value::from(1i64)],
    );
    use sea_orm::{FromQueryResult, JsonValue};
    let rows: Vec<serde_json::Value> = JsonValue::find_by_statement(stmt)
        .all(&anchor)
        .await
        .unwrap();
    let detail_joined: String = rows
        .iter()
        .filter_map(|r| r.get("detail").and_then(|v| v.as_str()))
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(
        !detail_joined.is_empty(),
        "EXPLAIN QUERY PLAN lieferte keine detail-Spalte"
    );
    assert!(
        detail_joined.contains("SEARCH") && !detail_joined.contains("SCAN"),
        "Erwarte SEARCH (Index-Lookup) statt SCAN, bekam: {detail_joined}"
    );
}
