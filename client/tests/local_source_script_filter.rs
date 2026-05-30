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
    Entity { id: id.into(), fields: m }
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
