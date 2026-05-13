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
//! Die Tabelle selbst arbeitet nur gegen den Trait und merkt nicht, ob die
//! Sortierung gerade serverseitig oder clientseitig erfolgt.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use serde_json::Value;
use shared::{
    ops_for_named, ColumnMeta, Entity, EntityChangeResult, FieldType, FilterCriteria, Sort,
    SortDirection,
};

use crate::graphql::queries::{
    create_entity, delete_entity, fetch_entities, update_entity, EntityPageResult,
};
use crate::graphql::GqlError;

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
        Self { entity_type: entity_type.into() }
    }
}

impl DataSource for RemoteSource {
    fn fetch(&self, req: DataRequest) -> BoxFuture<Result<DataResponse, GqlError>> {
        let entity_type = self.entity_type.clone();
        Box::pin(async move {
            let res: EntityPageResult =
                fetch_entities(&entity_type, req.page as i32, req.page_size as i32).await?;
            Ok(DataResponse { items: res.items, total_count: res.total_count })
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
}

impl LocalSource {
    pub fn new(items: Vec<Entity>, columns: &[ColumnMeta]) -> Self {
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
        }
    }

    fn passes(
        entity: &Entity,
        filter: &FilterCriteria,
        columns: &HashMap<String, ColumnLookup>,
    ) -> bool {
        for cf in &filter.predicates {
            let Some(col) = columns.get(&cf.key) else { return false };
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

    fn sort_in_place(
        items: &mut [Entity],
        sort: &Sort,
        columns: &HashMap<String, ColumnLookup>,
    ) {
        let Some(col) = columns.get(&sort.field) else { return };
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
        Box::pin(async move {
            // 1) Filter
            let mut filtered: Vec<Entity> = items
                .iter()
                .filter(|e| Self::passes(e, &req.filter, &columns))
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

            Ok(DataResponse { items: slice, total_count })
        })
    }
}
