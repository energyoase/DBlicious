//! Datenquellen-Abstraktion fuer die Tabelle.
//!
//! Der `DataSource`-Trait erlaubt es, die Tabelle von der konkreten
//! Beschaffung der Daten zu entkoppeln:
//!
//!   - `RemoteSource`  – Server-seitige Pagination/Sortierung/Filterung
//!     ueber GraphQL (in dieser Phase ohne aktive Sortier-/Filter-Logik).
//!   - `LocalSource`   – Vorgemerkt fuer Faelle, in denen die komplette
//!     Datenmenge clientseitig vorliegt und lokal verarbeitet wird.
//!
//! ## Script-Filter Resource-Schranken (Q0014 + Q0018-H3)
//! `script:`-Filter-Praedikate laufen pro Zeile durch die Rhai-Engine. Drei
//! Schranken begrenzen den Ressourcenverbrauch:
//!   (a) `set_max_operations(50_000)` **pro Run** (rhai.rs::apply_limits) —
//!       deckelt CPU-Ops eines einzelnen Skript-Laufs.
//!   (b) optionaler `manifest.timeout_ms` **pro Run** (sandbox.rs) als
//!       Wall-Clock-Deadline.
//!   (c) `MAX_SCRIPT_FILTER_RUNS` **aggregiert pro `fetch`** (Q0018-H3) —
//!       begrenzt die *Anzahl* der Filter-Runs ueber die Datenmenge; bei
//!       Erschoepfung fail-open (verbleibende Zeilen durchgelassen) + ein
//!       einmaliges `console::warn`. Reine Resource-Hygiene, kein
//!       Trust-Boundary.
//!
//! Die Tabelle selbst arbeitet nur gegen den Trait und merkt nicht, ob die
//! Sortierung gerade serverseitig oder clientseitig erfolgt.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use serde_json::Value;
use shared::script::engine::{HostApi, ScriptCtx, ScriptInputs, ScriptValue};
use shared::script::model::ProviderSlot;
use shared::{
    ops_for_named, ColumnMeta, Entity, EntityChangeResult, FieldType, FilterCriteria,
    FilterPredicate, Sort, SortDirection,
};

use crate::graphql::queries::{
    create_entity, delete_entity, fetch_entities, update_entity, EntityPageResult,
};
use crate::graphql::GqlError;
use crate::script::provider_lookup::{lookup_provider, LookupResult, SCRIPT_PREFIX};
use crate::script::registry::ScriptRegistry;

/// H3 (Q0018): Aggregat-Budget fuer `script:`-Filter-Runs pro `fetch`.
/// Jeder einzelne Run ist bereits durch `set_max_operations(50_000)`
/// (rhai.rs::apply_limits) gedeckelt; dieses Budget begrenzt die *Anzahl*
/// der Runs ueber die Datenmenge, damit `n_rows x teures Skript` den eigenen
/// Browser-Tab nicht unbeobachtet blockiert. Reine Resource-Hygiene, **kein
/// Trust-Boundary** (self-inflicted, client-side). Bei Erschoepfung: fail-open
/// + einmaliges Warning.
#[allow(dead_code)] // unter debug_assertions ungenutzt; aktiv im Release-Budget
const MAX_SCRIPT_FILTER_RUNS_PROD: usize = 5_000;

/// Kleiner Test-Override, damit der H3-Integrationstest in < 1 s laeuft.
/// `pub`, damit der Integrationstest die Schwelle als Konstante lesen kann.
/// Wird nur als aktives Budget verwendet, wenn `debug_assertions` aktiv ist
/// (also unter `cargo test`/dev-builds); Release-WASM nutzt den Prod-Wert.
pub const TEST_MAX_SCRIPT_FILTER_RUNS: usize = 200;

/// Das in diesem Build aktive Aggregat-Budget.
#[cfg(debug_assertions)]
const MAX_SCRIPT_FILTER_RUNS: usize = TEST_MAX_SCRIPT_FILTER_RUNS;
#[cfg(not(debug_assertions))]
const MAX_SCRIPT_FILTER_RUNS: usize = MAX_SCRIPT_FILTER_RUNS_PROD;

/// H3 (Q0018): pro `fetch` gefuehrtes Run-Budget fuer `script:`-Filter.
struct ScriptBudget {
    /// Verbleibende erlaubte `script:`-Filter-Runs in diesem `fetch`.
    runs_left: usize,
    /// Wurde das Budget in diesem `fetch` bereits ueberschritten? Dient dem
    /// einmaligen Warning (kein Per-Zeile-Spam).
    warned: bool,
}

impl ScriptBudget {
    fn new() -> Self {
        Self {
            runs_left: MAX_SCRIPT_FILTER_RUNS,
            warned: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DataRequest {
    pub page: u32,
    pub page_size: u32,
    pub sort: Option<Sort>,
    pub filter: FilterCriteria,
}

#[derive(Clone)]
pub struct DataResponse {
    pub items: Vec<Entity>,
    pub total_count: u64,
    /// Aufgeloeste Display-Labels fuer Reference-Felder:
    /// `{ col_key → { row_id → label } }`. Leer bei LocalSource oder wenn
    /// der Server keine Reference-Spalten mit `display_field` konfiguriert hat.
    pub reference_labels: BTreeMap<String, BTreeMap<String, String>>,
}

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T>>>;

pub trait DataSource: 'static {
    fn fetch(&self, req: DataRequest) -> BoxFuture<Result<DataResponse, GqlError>>;

    /// Liefert eine [`SavableSource`]-Sicht, falls die Datenquelle
    /// Erstellung/Aktualisierung unterstuetzt. Default: `None`
    /// (reine Read-only-Quellen wie aggregierte Reports).
    fn savable(&self) -> Option<&dyn SavableSource> {
        None
    }

    /// Liefert eine [`DeletableSource`]-Sicht, falls die Datenquelle
    /// Loeschungen unterstuetzt.
    fn deletable(&self) -> Option<&dyn DeletableSource> {
        None
    }
}

/// Capability fuer schreibende Operationen (Create/Update).
///
/// Pendant zu `ISavableAccess` aus der C#-Vorlage. Bewusst getrennt von
/// [`DeletableSource`], damit Quellen mit append-only-Semantik (z.B. Audit-
/// Logs) nur das implementieren, was sie wirklich koennen.
///
/// `expected_hash` (siehe [`shared::EntityHeader::hash`]) ermoeglicht
/// optimistic concurrency: stimmt der serverseitige Hash nicht ueberein,
/// liefert der Server einen `error.concurrent_modification`-Eintrag im
/// [`EntityChangeResult::validation`].
pub trait SavableSource {
    fn create(
        &self,
        fields: serde_json::Map<String, serde_json::Value>,
    ) -> BoxFuture<Result<EntityChangeResult, GqlError>>;

    fn update(
        &self,
        id: String,
        fields: serde_json::Map<String, serde_json::Value>,
        expected_hash: Option<u64>,
    ) -> BoxFuture<Result<EntityChangeResult, GqlError>>;
}

/// Capability fuer Loeschungen.
pub trait DeletableSource {
    fn delete(
        &self,
        id: String,
        expected_hash: Option<u64>,
    ) -> BoxFuture<Result<EntityChangeResult, GqlError>>;
}

/// Datenquelle, die die Anfragen 1:1 an den GraphQL-Server weiterreicht.
#[derive(Clone)]
pub struct RemoteSource {
    pub entity_type: Rc<str>,
}

impl RemoteSource {
    pub fn new(entity_type: impl Into<Rc<str>>) -> Self {
        Self {
            entity_type: entity_type.into(),
        }
    }
}

impl DataSource for RemoteSource {
    fn fetch(&self, req: DataRequest) -> BoxFuture<Result<DataResponse, GqlError>> {
        let entity_type = self.entity_type.clone();
        Box::pin(async move {
            let res: EntityPageResult =
                fetch_entities(&entity_type, req.page as i32, req.page_size as i32).await?;
            Ok(DataResponse {
                items: res.items,
                total_count: res.total_count,
                reference_labels: res.reference_labels,
            })
        })
    }

    fn savable(&self) -> Option<&dyn SavableSource> {
        Some(self)
    }

    fn deletable(&self) -> Option<&dyn DeletableSource> {
        Some(self)
    }
}

impl SavableSource for RemoteSource {
    fn create(
        &self,
        fields: serde_json::Map<String, serde_json::Value>,
    ) -> BoxFuture<Result<EntityChangeResult, GqlError>> {
        let entity_type = self.entity_type.clone();
        Box::pin(async move { create_entity(&entity_type, None, fields).await })
    }

    fn update(
        &self,
        id: String,
        fields: serde_json::Map<String, serde_json::Value>,
        expected_hash: Option<u64>,
    ) -> BoxFuture<Result<EntityChangeResult, GqlError>> {
        let entity_type = self.entity_type.clone();
        Box::pin(async move { update_entity(&entity_type, &id, fields, expected_hash).await })
    }
}

impl DeletableSource for RemoteSource {
    fn delete(
        &self,
        id: String,
        expected_hash: Option<u64>,
    ) -> BoxFuture<Result<EntityChangeResult, GqlError>> {
        let entity_type = self.entity_type.clone();
        Box::pin(async move { delete_entity(&entity_type, &id, expected_hash).await })
    }
}

/// Datenquelle, die eine vollstaendige `Vec<Entity>` clientseitig haelt und
/// Sort, Filter und Pagination ueber die [`FieldOps`](shared::FieldOps)-Schicht
/// auswertet.
///
/// Vorteil: dieselben Tabellen-Komponenten funktionieren unveraendert,
/// egal ob der Server filtert oder der Client. Die ganze Auswertung lebt
/// in einer einzigen Funktion und ist damit ohne Aufwand testbar.
///
/// Im C#-Vorbild waere das eine Sammlung von `IComparator<T>`- und
/// `IFilter<T>`-Instanzen, die ueber `ImplementationExtension` pro Spalte
/// ausgewaehlt wuerden. Hier genuegt `ops_for(&FieldType)`.
/// Spalten-Metadaten, die `LocalSource` zur Auswertung braucht.
/// Wir halten neben dem [`FieldType`] auch optionale Comparator-/Filter-IDs,
/// damit [`ops_for_named`] die richtige Variante waehlen kann.
#[derive(Clone)]
struct ColumnLookup {
    field_type: FieldType,
    comparator_id: Option<String>,
    filter_id: Option<String>,
}

#[derive(Clone)]
pub struct LocalSource {
    items: Rc<Vec<Entity>>,
    columns: Rc<HashMap<String, ColumnLookup>>,
    /// Q0014: optionale Skript-Registry + Host fuer `script:`-Filter-
    /// Praedikate. `None` => reiner Built-in-Ops-Pfad (Bestandsverhalten).
    scripts: Option<std::sync::Arc<ScriptRegistry>>,
    host: Option<std::sync::Arc<dyn HostApi>>,
}

impl LocalSource {
    pub fn new(items: Vec<Entity>, columns: &[ColumnMeta]) -> Self {
        Self::build(items, columns, None, None)
    }

    /// Q0014: wie `new`, aber mit Skript-Registry + Host fuer `script:`-Filter.
    /// `host` wird injiziert (Produktion: `RenderHost`; Test: `MockHostApi`),
    /// damit diese Datei frei von `testing`-gegateten Typen bleibt.
    pub fn with_script_registry(
        items: Vec<Entity>,
        columns: &[ColumnMeta],
        scripts: std::sync::Arc<ScriptRegistry>,
        host: std::sync::Arc<dyn HostApi>,
    ) -> Self {
        Self::build(items, columns, Some(scripts), Some(host))
    }

    fn build(
        items: Vec<Entity>,
        columns: &[ColumnMeta],
        scripts: Option<std::sync::Arc<ScriptRegistry>>,
        host: Option<std::sync::Arc<dyn HostApi>>,
    ) -> Self {
        let columns = columns
            .iter()
            .map(|c| {
                (
                    c.key.clone(),
                    ColumnLookup {
                        field_type: c.field_type.clone(),
                        comparator_id: c.comparator_id.clone(),
                        filter_id: c.filter_id.clone(),
                    },
                )
            })
            .collect();
        Self {
            items: Rc::new(items),
            columns: Rc::new(columns),
            scripts,
            host,
        }
    }

    fn passes(
        entity: &Entity,
        filter: &FilterCriteria,
        columns: &HashMap<String, ColumnLookup>,
        scripts: Option<&std::sync::Arc<ScriptRegistry>>,
        host: Option<&std::sync::Arc<dyn HostApi>>,
        budget: &mut ScriptBudget,
    ) -> bool {
        for cf in &filter.predicates {
            let Some(col) = columns.get(&cf.key) else {
                return false;
            };
            // Q0014: `script:`-Filter => Per-Row-Boolean-Praedikat statt Ops.
            if let Some(fid) = col.filter_id.as_deref() {
                if fid.starts_with(SCRIPT_PREFIX) {
                    if let (Some(reg), Some(h)) = (scripts, host) {
                        // H3 (Q0018): Aggregat-Run-Budget pro `fetch`. Ist es
                        // erschoepft, laeuft kein weiteres Filter-Skript mehr;
                        // die Zeile wird durchgelassen (fail-open, konsistent
                        // mit der "kein Skript => Zeile durchlassen"-Semantik
                        // unten). Einmaliges Warning beim ersten Ueberschreiten.
                        if budget.runs_left == 0 {
                            if !budget.warned {
                                budget.warned = true;
                                log::warn!(
                                    "dblicious: script-Filter Run-Budget pro fetch \
                                     erschoepft (MAX_SCRIPT_FILTER_RUNS); restliche \
                                     Zeilen werden ungefiltert durchgelassen."
                                );
                            }
                            continue; // fail-open
                        }
                        budget.runs_left -= 1;
                        if !script_predicate(entity, &cf.predicate, fid, reg, h.clone()) {
                            return false;
                        }
                        continue; // Skript hat entschieden; Ops ueberspringen.
                    }
                    // Keine Registry/Host => Skript inaktiv => Zeile durchlassen.
                    continue;
                }
            }
            let value = entity.fields.get(&cf.key).cloned().unwrap_or(Value::Null);
            let ops = ops_for_named(
                &col.field_type,
                col.comparator_id.as_deref(),
                col.filter_id.as_deref(),
            );
            if !ops.matches(&value, &cf.predicate) {
                return false;
            }
        }
        if let Some(needle) = filter.global_search.as_deref().filter(|s| !s.is_empty()) {
            let hit = columns.iter().any(|(key, col)| {
                // H2 (Q0018): `script:`-Filter-Spalten sind boolesche Per-Row-
                // Praedikate ohne sinnvollen durchsuchbaren Textwert. Aus der
                // global_search explizit ausklammern (statt implizit ueber den
                // Roh-`field_type` zu suchen). Kein Skript-Run hier.
                if col
                    .filter_id
                    .as_deref()
                    .is_some_and(|fid| fid.starts_with(SCRIPT_PREFIX))
                {
                    return false;
                }
                let value = entity.fields.get(key).cloned().unwrap_or(Value::Null);
                let ops = ops_for_named(
                    &col.field_type,
                    col.comparator_id.as_deref(),
                    col.filter_id.as_deref(),
                );
                ops.matches_search(&value, needle)
            });
            if !hit {
                return false;
            }
        }
        true
    }

    fn sort_in_place(items: &mut [Entity], sort: &Sort, columns: &HashMap<String, ColumnLookup>) {
        let Some(col) = columns.get(&sort.field) else {
            return;
        };
        let ops = ops_for_named(
            &col.field_type,
            col.comparator_id.as_deref(),
            col.filter_id.as_deref(),
        );
        let direction = sort.direction;
        let key = sort.field.clone();
        items.sort_by(|a, b| {
            let va = a.fields.get(&key).cloned().unwrap_or(Value::Null);
            let vb = b.fields.get(&key).cloned().unwrap_or(Value::Null);
            let ord = ops.compare(&va, &vb);
            match direction {
                SortDirection::Asc => ord,
                SortDirection::Desc => match ord {
                    Ordering::Less => Ordering::Greater,
                    Ordering::Greater => Ordering::Less,
                    Ordering::Equal => Ordering::Equal,
                },
            }
        });
    }
}

impl DataSource for LocalSource {
    fn fetch(&self, req: DataRequest) -> BoxFuture<Result<DataResponse, GqlError>> {
        let items = self.items.clone();
        let columns = self.columns.clone();
        let scripts = self.scripts.clone();
        let host = self.host.clone();
        Box::pin(async move {
            // 1) Filter
            // H3 (Q0018): ein Run-Budget pro `fetch` ueber alle Zeilen.
            let mut budget = ScriptBudget::new();
            let mut filtered: Vec<Entity> = items
                .iter()
                .filter(|e| {
                    Self::passes(
                        e,
                        &req.filter,
                        &columns,
                        scripts.as_ref(),
                        host.as_ref(),
                        &mut budget,
                    )
                })
                .cloned()
                .collect();

            // 2) Sort
            if let Some(s) = &req.sort {
                Self::sort_in_place(&mut filtered, s, &columns);
            }

            // 3) Pagination
            let total_count = filtered.len() as u64;
            let page = req.page.max(1) as usize;
            let page_size = req.page_size.max(1) as usize;
            let start = (page - 1).saturating_mul(page_size);
            let end = start.saturating_add(page_size).min(filtered.len());
            let slice = if start < filtered.len() {
                filtered[start..end].to_vec()
            } else {
                Vec::new()
            };

            Ok(DataResponse {
                items: slice,
                total_count,
                reference_labels: BTreeMap::new(),
            })
        })
    }
}

/// Q0014: wertet ein `script:`-Filter-Praedikat fuer **eine** Zeile aus.
/// `selected` ist der Vergleichswert aus dem Filter-State (fuer eine
/// integer-Spalte ein `NumberEquals`-Value); er wird als `selectedStackId`
/// in eine angereicherte `fields`-Kopie injiziert. `true` = Zeile behalten.
/// `host` wird von aussen injiziert (kein `testing`-gegateter Typ hier).
/// Fallback/NotAScriptId/Nicht-Bool => `true` (nicht ausschliessen).
fn script_predicate(
    entity: &Entity,
    predicate: &FilterPredicate,
    filter_id: &str,
    registry: &ScriptRegistry,
    host: std::sync::Arc<dyn HostApi>,
) -> bool {
    let selected = match predicate {
        FilterPredicate::NumberEquals { value } => Some(*value),
        _ => None,
    };
    let mut fields = entity.fields.clone();
    // H1 (Q0018): Ganzzahlige Filterwerte int-typisiert injizieren, damit
    // `selectedStackId` in `json_to_dynamic` als Rhai-INT marshalt — symmetrisch
    // zur Row-`stackId` (kommt aus `serde_json::json!(<i64>)` ebenfalls als INT).
    // So haengt der Vergleich `row == sel` nicht an Rhais INT/FLOAT-Coercion.
    // Echte Nachkommastellen / out-of-range / NaN/inf bleiben FLOAT bzw. Null.
    let sel_value = match selected {
        Some(v)
            if v.is_finite()
                && v.fract() == 0.0
                && v >= i64::MIN as f64
                && v <= i64::MAX as f64 =>
        {
            Value::Number((v as i64).into())
        }
        Some(v) => serde_json::Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        None => Value::Null,
    };
    fields.insert("selectedStackId".into(), sel_value);

    let inputs = ScriptInputs {
        value: Value::Null,
        fields,
    };
    match lookup_provider(
        filter_id,
        ProviderSlot::Filter,
        registry,
        host,
        ScriptCtx::default(),
        inputs,
    ) {
        LookupResult::Ok {
            value: ScriptValue::Bool(b),
        } => b,
        // Fallback / NotAScriptId / Nicht-Bool => Zeile nicht ausschliessen.
        _ => true,
    }
}
