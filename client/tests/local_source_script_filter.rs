//! End-to-End (Q0014, Lücke B): `LocalSource` filtert Zeilen per `stackId`
//! durch das echte `d2v_stack_filter`-Skript (reale Rhai-Engine, realer
//! lookup_provider — kein Mock-Praedikat).

use client::components::table::data_source::{DataRequest, DataSource, LocalSource};
use client::script::registry::ScriptRegistry;
use shared::script::model::{ProviderSlot, Script, ScriptKind, ScriptState};
use shared::script::{CapabilityToken, ScriptId, ScriptManifest, ScriptTier};
use shared::{ColumnFilter, ColumnMeta, Entity, FieldType, FilterCriteria, FilterPredicate};

const STACK_SRC: &str = include_str!("../../examples/d2v/scripts/d2v_stack_filter.rhai");

fn stack_filter_script() -> Script {
    Script {
        id: ScriptId("d2v_stack_filter".into()),
        kind: ScriptKind::Provider {
            slot: ProviderSlot::Filter,
        },
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Reader,
            capabilities: vec![CapabilityToken::ComputeOnly],
            ..Default::default()
        },
        source: STACK_SRC.into(),
        version: 1,
        state: ScriptState::Active,
        last_error: None,
        created_by: "u-1".into(),
        created_at: "2026-05-23T00:00:00Z".into(),
        updated_at: "2026-05-23T00:00:00Z".into(),
    }
}

fn row(id: &str, stack: i64) -> Entity {
    let mut m = serde_json::Map::new();
    m.insert("stackId".into(), serde_json::json!(stack));
    Entity {
        id: id.into(),
        fields: m,
    }
}

fn columns() -> Vec<ColumnMeta> {
    vec![ColumnMeta {
        key: "stackId".into(),
        label_key: "k".into(),
        field_type: FieldType::Integer,
        sortable: true,
        filterable: true,
        comparator_id: None,
        filter_id: Some("script:d2v_stack_filter".into()),
        editor_id: None,
        formatter_id: None,
        validator_id: None,
        action_ids: vec![],
    }]
}

fn filter_for_stack(sel: f64) -> FilterCriteria {
    FilterCriteria {
        global_search: None,
        predicates: vec![ColumnFilter {
            key: "stackId".into(),
            predicate: FilterPredicate::NumberEquals { value: sel },
        }],
    }
}

fn run(src: &LocalSource, filter: FilterCriteria) -> Vec<String> {
    let req = DataRequest {
        page: 1,
        page_size: 100,
        sort: None,
        filter,
    };
    let resp = futures::executor::block_on(src.fetch(req)).unwrap();
    resp.items.into_iter().map(|e| e.id).collect()
}

fn source() -> LocalSource {
    let reg = ScriptRegistry::new();
    reg.insert(stack_filter_script());
    // Host wird injiziert: im Test der testing-MockHostApi (das d2v_stack_filter-
    // Skript ist computeOnly und ruft keine Host-Methode). Produktion injiziert
    // RenderHost. So bleibt `data_source.rs` frei von testing-gegateten Typen.
    let host: std::sync::Arc<dyn shared::script::engine::HostApi> =
        std::sync::Arc::new(shared::script::testing::MockHostApi::new());
    LocalSource::with_script_registry(
        vec![row("a", 1), row("b", 2), row("c", 3)],
        &columns(),
        std::sync::Arc::new(reg),
        host,
    )
}

#[test]
fn selected_stack_includes_only_matching_rows() {
    let src = source();
    let ids = run(&src, filter_for_stack(2.0));
    assert_eq!(ids, vec!["b".to_string()]);
}

#[test]
fn unselected_stack_passes_all_rows() {
    let src = source();
    let ids = run(&src, filter_for_stack(-1.0));
    assert_eq!(ids.len(), 3, "selectedStackId == -1 => alle Stacks");
}

// --- H1 (Q0018): typstrenger INT-vs-FLOAT-Vergleich ---------------------

/// Wie `d2v_stack_filter`, aber typstreng: erzwingt, dass `selectedStackId`
/// als INT ankommt (`type_of(row) == type_of(sel)`). Mit dem alten FLOAT-Pfad
/// waere `type_of(sel) == "f64"` != `type_of(row) == "i64"` => leeres Ergebnis.
const TYPED_STACK_SRC: &str = "\
let sel = fields.selectedStackId;\n\
let row = fields.stackId;\n\
if sel == () || sel == -1 { true } else { type_of(row) == type_of(sel) && row == sel }\n";

fn typed_stack_filter_script() -> Script {
    Script {
        id: ScriptId("typed_stack_filter".into()),
        kind: ScriptKind::Provider {
            slot: ProviderSlot::Filter,
        },
        manifest: ScriptManifest {
            manifest_version: 1,
            tier: ScriptTier::Reader,
            capabilities: vec![CapabilityToken::ComputeOnly],
            ..Default::default()
        },
        source: TYPED_STACK_SRC.into(),
        version: 1,
        state: ScriptState::Active,
        last_error: None,
        created_by: "u-1".into(),
        created_at: "2026-05-23T00:00:00Z".into(),
        updated_at: "2026-05-23T00:00:00Z".into(),
    }
}

fn typed_columns() -> Vec<ColumnMeta> {
    vec![ColumnMeta {
        key: "stackId".into(),
        label_key: "k".into(),
        field_type: FieldType::Integer,
        sortable: true,
        filterable: true,
        comparator_id: None,
        filter_id: Some("script:typed_stack_filter".into()),
        editor_id: None,
        formatter_id: None,
        validator_id: None,
        action_ids: vec![],
    }]
}

fn typed_source() -> LocalSource {
    let reg = ScriptRegistry::new();
    reg.insert(typed_stack_filter_script());
    let host: std::sync::Arc<dyn shared::script::engine::HostApi> =
        std::sync::Arc::new(shared::script::testing::MockHostApi::new());
    LocalSource::with_script_registry(
        vec![row("a", 1), row("b", 2), row("c", 3)],
        &typed_columns(),
        std::sync::Arc::new(reg),
        host,
    )
}

#[test]
fn h1_selected_stack_reaches_script_as_int() {
    let src = typed_source();
    // filter_for_stack(2.0) baut NumberEquals { value: 2.0 } (f64). Mit dem Fix
    // wird der ganze Wert int-typisiert injiziert => Rhai-INT => type_of match.
    let ids = run(&src, filter_for_stack(2.0));
    assert_eq!(
        ids,
        vec!["b".to_string()],
        "selectedStackId muss als INT ankommen (type_of == row); vor dem Fix leer"
    );
}

#[test]
fn h1_sentinel_minus_one_passes_all_as_int() {
    let src = typed_source();
    // -1-Sentinel: jetzt INT -1; das `sel == -1`-Branch greift => alle 3 Zeilen.
    let ids = run(&src, filter_for_stack(-1.0));
    assert_eq!(ids.len(), 3, "INT -1 Sentinel => alle Stacks");
}

// --- H2 (Q0018): global_search klammert `script:`-Spalten aus -----------

fn named_row(id: &str, name: &str, stack: i64) -> Entity {
    let mut m = serde_json::Map::new();
    m.insert("name".into(), serde_json::json!(name));
    m.insert("stackId".into(), serde_json::json!(stack));
    Entity {
        id: id.into(),
        fields: m,
    }
}

fn mixed_columns() -> Vec<ColumnMeta> {
    vec![
        ColumnMeta {
            key: "name".into(),
            label_key: "k".into(),
            field_type: FieldType::Text,
            sortable: true,
            filterable: true,
            comparator_id: None,
            filter_id: None,
            editor_id: None,
            formatter_id: None,
            validator_id: None,
            action_ids: vec![],
        },
        ColumnMeta {
            key: "stackId".into(),
            label_key: "k".into(),
            field_type: FieldType::Integer,
            sortable: true,
            filterable: true,
            comparator_id: None,
            filter_id: Some("script:d2v_stack_filter".into()),
            editor_id: None,
            formatter_id: None,
            validator_id: None,
            action_ids: vec![],
        },
    ]
}

fn mixed_source() -> LocalSource {
    let reg = ScriptRegistry::new();
    reg.insert(stack_filter_script());
    let host: std::sync::Arc<dyn shared::script::engine::HostApi> =
        std::sync::Arc::new(shared::script::testing::MockHostApi::new());
    LocalSource::with_script_registry(
        vec![
            named_row("apple", "apple", 2),
            named_row("banana", "banana", 9),
        ],
        &mixed_columns(),
        std::sync::Arc::new(reg),
        host,
    )
}

#[test]
fn h2_global_search_skips_script_columns() {
    let src = mixed_source();
    let filter = FilterCriteria {
        global_search: Some("2".into()),
        predicates: vec![],
    };
    let ids = run(&src, filter);
    // "2" passt auf KEINEN `name`; die `script:`-Spalte `stackId` (Rohwert 2 bei
    // "apple") ist aus der global_search ausgeklammert => kein Treffer.
    // Vor dem Fix wuerde "apple" ueber den stackId-Rohwert matchen.
    assert!(
        ids.is_empty(),
        "script:-Spalte darf nicht zur global_search beitragen, got {ids:?}"
    );
}

#[test]
fn h2_global_search_still_hits_text_column() {
    let src = mixed_source();
    let filter = FilterCriteria {
        global_search: Some("appl".into()),
        predicates: vec![],
    };
    let ids = run(&src, filter);
    // Treffer ueber die Text-Spalte `name` bleibt unveraendert.
    assert_eq!(ids, vec!["apple".to_string()]);
}

// --- H3 (Q0018): Aggregat-Run-Budget greift (fail-open) -----------------

/// `client::components::table::data_source::TEST_MAX_SCRIPT_FILTER_RUNS` ist
/// der `#[cfg(test)]`-Override des Produktionsbudgets (klein, damit der Test
/// schnell bleibt). Re-exportiert fuer die Assertion-Schwelle.
use client::components::table::data_source::TEST_MAX_SCRIPT_FILTER_RUNS;

fn big_excluding_source(n: usize) -> LocalSource {
    let reg = ScriptRegistry::new();
    reg.insert(stack_filter_script());
    let host: std::sync::Arc<dyn shared::script::engine::HostApi> =
        std::sync::Arc::new(shared::script::testing::MockHostApi::new());
    // Alle Zeilen haben stack != 999 => das Filter-Skript schliesst jede Zeile
    // aus, solange das Budget reicht.
    let rows: Vec<Entity> = (0..n).map(|i| row(&format!("r{i}"), 1)).collect();
    LocalSource::with_script_registry(rows, &columns(), std::sync::Arc::new(reg), host)
}

#[test]
fn h3_budget_lets_excess_rows_pass_open() {
    let budget = TEST_MAX_SCRIPT_FILTER_RUNS;
    let n = budget + 50; // mehr Zeilen als Budget-Runs
    let src = big_excluding_source(n);
    // selectedStackId = 999 schliesst jede Zeile aus, solange ein Skript laeuft.
    let req = DataRequest {
        page: 1,
        page_size: (n as u32) + 10, // alle Zeilen auf einer Seite
        sort: None,
        filter: filter_for_stack(999.0),
    };
    let resp = futures::executor::block_on(src.fetch(req)).unwrap();
    // Ohne Budget waere total_count == 0 (alle ausgeschlossen). Mit Budget
    // werden die Zeilen jenseits des Budgets durchgelassen (fail-open).
    assert!(
        resp.total_count > 0,
        "Zeilen jenseits des Budgets muessen fail-open durchgelassen werden"
    );
    // Genauer: hoechstens `budget` Zeilen wurden tatsaechlich ausgewertet (und
    // ausgeschlossen); der Rest passiert. Also >= n - budget Durchlaeufer.
    assert!(
        resp.total_count >= (n - budget) as u64,
        "mindestens die {} Ueberschuss-Zeilen muessen durchgelassen werden, got {}",
        n - budget,
        resp.total_count
    );
}

#[test]
fn h3_within_budget_no_regression() {
    // Datenmenge <= Budget: Verhalten unveraendert (alle ausgeschlossen).
    let src = big_excluding_source(TEST_MAX_SCRIPT_FILTER_RUNS / 2);
    let ids = run(&src, filter_for_stack(999.0));
    assert!(
        ids.is_empty(),
        "innerhalb des Budgets bleibt der Filter strikt"
    );
}
