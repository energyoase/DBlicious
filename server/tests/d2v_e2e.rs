//! E2E-Integrationstest: echte `examples/d2v/`-Bindings gegen Fixture-SQLite.
//!
//! Beweist, dass die Bindings aus den binding.json-Dateien tatsaechlich gegen
//! ein Foreign-Schema funktionieren. Ersetzt einen Teil der Plan-Tasks 25-26
//! in kompakter Form.
//!
//! Getestete Entities:
//! - `datev_account`          — ACTIVE, Single-PK (`number`), table `DatevAccounts`
//! - `star_money_account`     — ACTIVE, Composite-PK (`bankCode`, `code`), table `StarMoneyAccounts`
//! - `star_money_booking_text`— LEGACY-IMPORT, read-only, Single-PK (`key`), table `StarMoneyBookingTexts`

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use server::example::{self, ExampleSet};
use server::source::foreign_sqlite::ForeignSqliteSource;
use server::source::{PageQuery, Source, SourceError};
use shared::source::EntityBinding;
use shared::FilterCriteria;

// =============================================================================
// Helper-Funktionen
// =============================================================================

fn d2v_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("examples")
        .join("d2v")
}

fn load_examples_d2v() -> ExampleSet {
    example::load(&d2v_dir()).expect("examples/d2v muss ladbar sein")
}

fn binding_for(set: &ExampleSet, entity_type: &str) -> EntityBinding {
    set.entities[entity_type]
        .settings
        .as_ref()
        .unwrap_or_else(|| panic!("{entity_type}: settings fehlt"))
        .binding
        .as_ref()
        .unwrap_or_else(|| panic!("{entity_type}: binding fehlt"))
        .clone()
}

fn uuid_like() -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{nanos}_{seq}")
}

/// Erstellt eine benannte Shared-Memory-SQLite-DB, fuehrt die DDL/DML-Statements
/// aus und gibt Anchor-Connection zurueck (haelt DB am Leben).
async fn make_fixture_db(ddl: &[&str]) -> (String, sea_orm::DatabaseConnection) {
    let url = format!("sqlite:file:test_{}?mode=memory&cache=shared", uuid_like());
    let anchor = Database::connect(sea_orm::ConnectOptions::new(url.clone()))
        .await
        .unwrap();
    for stmt in ddl {
        anchor
            .execute(Statement::from_string(DbBackend::Sqlite, *stmt))
            .await
            .unwrap();
    }
    (url, anchor)
}

async fn make_fixture_source(url: &str, source_name: &str) -> ForeignSqliteSource {
    let mut src = ForeignSqliteSource::new(source_name.into(), url.into());
    src.init().await.unwrap();
    src
}

// =============================================================================
// Test 1: datev_account — Single-PK, list_page, fields haben camelCase-Keys
// =============================================================================

#[tokio::test]
async fn test_datev_account_list_page() {
    let set = load_examples_d2v();
    let binding = binding_for(&set, "datev_account");

    // Spalten aus columnMap (PascalCase): Number, Name, EnglishName, TaxRate,
    // AccountType, AutomaticTaxAccountNr, AutomaticReverseTaxAccountNr, CompanyId
    // FieldType-Mapping: number->INTEGER, name->TEXT, englishName->TEXT,
    //   taxRate->REAL (decimal), accountType->TEXT, automaticTaxAccountNr->INTEGER,
    //   automaticReverseTaxAccountNr->INTEGER, companyId->INTEGER
    let ddl = &[
        r#"CREATE TABLE DatevAccounts (
            Number                       INTEGER PRIMARY KEY,
            Name                         TEXT,
            EnglishName                  TEXT,
            TaxRate                      REAL,
            AccountType                  TEXT,
            AutomaticTaxAccountNr        INTEGER,
            AutomaticReverseTaxAccountNr INTEGER,
            CompanyId                    INTEGER
        )"#,
        "INSERT INTO DatevAccounts (Number, Name, EnglishName, TaxRate, AccountType, AutomaticTaxAccountNr, AutomaticReverseTaxAccountNr, CompanyId) VALUES (1200, 'Bank', 'Bank', NULL, 1, NULL, NULL, NULL)",
        "INSERT INTO DatevAccounts (Number, Name, EnglishName, TaxRate, AccountType, AutomaticTaxAccountNr, AutomaticReverseTaxAccountNr, CompanyId) VALUES (1000, 'Kasse', 'Cash', NULL, 1, NULL, NULL, NULL)",
    ];

    let (url, _anchor) = make_fixture_db(ddl).await;
    let src = make_fixture_source(&url, "d2v_legacy").await;

    let page = src
        .list_page(
            &binding,
            &PageQuery {
                page: 1,
                page_size: 10,
                sort: None,
                filter: FilterCriteria::default(),
            },
        )
        .await
        .unwrap();

    assert_eq!(page.total_count, 2, "Erwarte 2 eingefuegte Zeilen");
    assert_eq!(page.items.len(), 2);

    // Felder muessen camelCase-Keys aus columnMap haben (nicht PascalCase)
    let first = &page.items[0];
    assert!(
        first.fields.contains_key("number"),
        "Feld 'number' (camelCase) fehlt, vorhanden: {:?}",
        first.fields.keys().collect::<Vec<_>>()
    );
    assert!(first.fields.contains_key("name"), "Feld 'name' fehlt");
    // Kein PascalCase soll durchrutschen
    assert!(
        !first.fields.contains_key("Number"),
        "PascalCase-Key 'Number' darf nicht im Entity.fields sein"
    );
}

// =============================================================================
// Test 2: star_money_account — Composite-PK, get per Composite-ID
// =============================================================================

#[tokio::test]
async fn test_star_money_account_composite_get() {
    let set = load_examples_d2v();
    let binding = binding_for(&set, "star_money_account");

    // columnMap: bankCode->BankCode (TEXT), code->Code (TEXT),
    //   defaultName->DefaultName (TEXT), companyId->CompanyId (INTEGER),
    //   datevAccountNr->DatevAccountNr (INTEGER)
    // Composite-PK: (BankCode, Code)
    let ddl = &[
        r#"CREATE TABLE StarMoneyAccounts (
            BankCode       TEXT NOT NULL,
            Code           TEXT NOT NULL,
            DefaultName    TEXT,
            CompanyId      INTEGER,
            DatevAccountNr INTEGER,
            PRIMARY KEY (BankCode, Code)
        )"#,
        "INSERT INTO StarMoneyAccounts (BankCode, Code, DefaultName, CompanyId, DatevAccountNr) VALUES ('BANK1', 'ACC1', 'Hauptkonto', 42, 1200)",
        "INSERT INTO StarMoneyAccounts (BankCode, Code, DefaultName, CompanyId, DatevAccountNr) VALUES ('BANK1', 'ACC2', 'Sparkonto', 42, 1201)",
    ];

    let (url, _anchor) = make_fixture_db(ddl).await;
    let src = make_fixture_source(&url, "d2v_legacy").await;

    let id = shared::source::EntityId::Composite(vec!["BANK1".into(), "ACC1".into()]);
    let result = src.get(&binding, &id).await.unwrap();

    assert!(
        result.is_some(),
        "Entity muss gefunden werden (Composite-PK)"
    );
    let entity = result.unwrap();
    assert_eq!(
        entity.fields.get("code").and_then(|v| v.as_str()),
        Some("ACC1"),
        "Feld 'code' muss 'ACC1' sein"
    );
    assert_eq!(
        entity.fields.get("bankCode").and_then(|v| v.as_str()),
        Some("BANK1"),
        "Feld 'bankCode' muss 'BANK1' sein"
    );
    assert_eq!(
        entity.fields.get("defaultName").and_then(|v| v.as_str()),
        Some("Hauptkonto"),
        "Feld 'defaultName' muss 'Hauptkonto' sein"
    );
}

// =============================================================================
// Test 3: star_money_booking_text — read-only, update muss ReadOnly-Fehler liefern
// =============================================================================

#[tokio::test]
async fn test_star_money_booking_text_read_only_rejects_update() {
    let set = load_examples_d2v();
    let binding = binding_for(&set, "star_money_booking_text");

    // Sanity-Check: Binding muss read_only=true sein (aus binding.json)
    assert!(
        binding.read_only,
        "star_money_booking_text muss readOnly=true haben laut binding.json"
    );

    // columnMap: key->Key (INTEGER PK), text->Text (TEXT), text2..text8 (TEXT)
    let ddl = &[
        r#"CREATE TABLE StarMoneyBookingTexts (
            Key   INTEGER PRIMARY KEY,
            Text  TEXT,
            Text2 TEXT,
            Text3 TEXT,
            Text4 TEXT,
            Text5 TEXT,
            Text6 TEXT,
            Text7 TEXT,
            Text8 TEXT
        )"#,
        "INSERT INTO StarMoneyBookingTexts (Key, Text, Text2, Text3, Text4, Text5, Text6, Text7, Text8) VALUES (1, 'Sample', NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
        "INSERT INTO StarMoneyBookingTexts (Key, Text, Text2, Text3, Text4, Text5, Text6, Text7, Text8) VALUES (2, 'Zweiter', NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
    ];

    let (url, _anchor) = make_fixture_db(ddl).await;
    let src = make_fixture_source(&url, "d2v_legacy").await;

    // list_page muss trotzdem funktionieren (read-only blockiert nur Writes)
    let page = src
        .list_page(
            &binding,
            &PageQuery {
                page: 1,
                page_size: 10,
                sort: None,
                filter: FilterCriteria::default(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        page.total_count, 2,
        "Lesen muss bei read-only funktionieren"
    );

    // update muss ReadOnly-Fehler liefern
    let id = shared::source::EntityId::Single("1".into());
    let mut patch = serde_json::Map::new();
    patch.insert("text".into(), serde_json::json!("Geaendert"));

    let err = src
        .update(&binding, &id, patch, None)
        .await
        .err()
        .expect("update auf read-only Binding muss Err liefern");

    assert!(
        matches!(err, SourceError::ReadOnly),
        "Erwarte SourceError::ReadOnly, bekam: {err:?}"
    );
}
