//! `BuilderPreviewSource` — Datenquelle fuer die Live-Vorschau im Visual
//! Builder (Phase 1.5).
//!
//! Aufgabe: dieselbe [`crate::components::table::EntityTable`] wie im
//! Produktivpfad mit Daten fuettern, **ohne** dass ein Server-Roundtrip
//! noetig waere. Reihen werden anhand der projizierten Spalten-Metadaten
//! synthetisiert (siehe [`synthesize_preview_rows`]); Sort, Filter und
//! Pagination wandern unveraendert durch die bestehende
//! [`LocalSource`]-Auswertung.
//!
//! Integration ueber den `UiTree`:
//! ```ignore
//! let cols  = crate::builder::project_columns(&tree);
//! let src   = BuilderPreviewSource::from_columns(&cols, 8);
//! view! { <EntityTableShell columns=cols source=Rc::new(src) /> }
//! ```
//!
//! Reaktivitaet: die Quelle haelt eine **Snapshot** der Spalten — der
//! Designer-Host (Phase 1.4-Route) baut sie via Leptos-`Memo` aus
//! [`crate::builder::UiTreeSignal`] neu, sobald sich der Tree aendert.
//! So bleibt die Quelle selbst trivial cloneable und Thread-frei.

use std::rc::Rc;

use serde_json::{json, Value};
use shared::{ColumnMeta, Entity, FieldType};

use crate::builder::{project_columns, UiTree};

use super::data_source::{BoxFuture, DataRequest, DataResponse, DataSource, LocalSource};
use crate::graphql::GqlError;

/// Default-Zeilenanzahl fuer die Preview, wenn der Aufrufer nichts setzt.
pub const DEFAULT_PREVIEW_ROWS: usize = 8;

/// Live-Preview-Datenquelle fuer den Builder.
///
/// Wrappt eine [`LocalSource`], damit Sort/Filter/Pagination ohne
/// Sondercode greifen. Der Wrapper existiert primaer aus zwei Gruenden:
///   - Konstruktion aus [`UiTree`] bzw. [`ColumnMeta`]-Snapshot (statt
///     Caller, der selbst synthetisieren muesste),
///   - Marker-Typ im Aufruf-Code, damit `BuilderPreviewSource` und
///     `LocalSource` unterschiedliche Lese-Intentionen tragen.
#[derive(Clone)]
pub struct BuilderPreviewSource {
    inner: LocalSource,
}

impl BuilderPreviewSource {
    /// Konstruiert die Quelle aus bereits projizierten Spalten.
    pub fn from_columns(columns: &[ColumnMeta], row_count: usize) -> Self {
        let rows = synthesize_preview_rows(columns, row_count);
        Self {
            inner: LocalSource::new(rows, columns),
        }
    }

    /// Bequemer Wrapper: projiziert den Tree und baut die Quelle in einem.
    pub fn from_tree(tree: &UiTree, row_count: usize) -> Self {
        let cols = project_columns(tree);
        Self::from_columns(&cols, row_count)
    }

    /// `Rc`-Variante fuer den direkten Einbau in `EntityTableShell`.
    pub fn rc_from_tree(tree: &UiTree, row_count: usize) -> Rc<dyn DataSource> {
        Rc::new(Self::from_tree(tree, row_count))
    }
}

impl DataSource for BuilderPreviewSource {
    fn fetch(&self, req: DataRequest) -> BoxFuture<Result<DataResponse, GqlError>> {
        self.inner.fetch(req)
    }
}

// =============================================================================
// Synthese der Preview-Reihen
// =============================================================================

/// Erzeugt `row_count` synthetische Entitaeten, deren Felder Platzhalter-
/// Werte passend zum `FieldType` jeder Spalte enthalten.
///
/// Werte sind stabil (deterministisch nach `row_index` + `column_key`), damit
/// die Vorschau nicht bei jedem Render flackert.
pub fn synthesize_preview_rows(columns: &[ColumnMeta], row_count: usize) -> Vec<Entity> {
    (0..row_count)
        .map(|i| {
            let mut fields = serde_json::Map::new();
            for c in columns {
                fields.insert(c.key.clone(), placeholder_value(&c.field_type, i));
            }
            Entity {
                id: format!("preview-{i}"),
                fields,
            }
        })
        .collect()
}

fn placeholder_value(ft: &FieldType, row_index: usize) -> Value {
    let i = row_index as i64;
    match ft {
        FieldType::Text => Value::String(format!("Beispiel {}", i + 1)),
        FieldType::Integer => json!(i * 10 + 1),
        FieldType::Decimal { precision } => {
            // Skaliert mit `precision`, damit der Wert sichtbar Nachkommastellen
            // erzeugt, ohne formatabhaengig zu werden.
            let base = (i as f64) * 10.0 + 0.99;
            let scale = 10f64.powi(*precision as i32);
            json!((base * scale).round() / scale)
        }
        FieldType::Boolean => Value::Bool(i % 2 == 0),
        FieldType::Date => {
            // Statische Basis 2026-01-01 + row_index Tage. Bewusst ohne
            // chrono-Dependency: simple Tag-Arithmetik im Januar-Bereich
            // genuegt fuer eine Vorschau.
            let day = (i as i32 % 28) + 1;
            Value::String(format!("2026-01-{:02}", day))
        }
        FieldType::DateTime => {
            let day = (i as i32 % 28) + 1;
            Value::String(format!("2026-01-{:02}T09:00:00Z", day))
        }
        FieldType::Money { .. } => json!((i as f64) * 100.0 + 19.99),
        FieldType::Reference { entity } => Value::String(format!("{entity}-preview-{i}")),
        FieldType::Collection { .. } => Value::Array(Vec::new()),
        FieldType::Enum { values } => {
            if values.is_empty() {
                Value::Null
            } else {
                Value::String(values[(i as usize) % values.len()].clone())
            }
        }
        FieldType::IntEnum { values } => {
            if values.is_empty() {
                Value::Null
            } else {
                Value::String(values[(i as usize) % values.len()].wire_name.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::{node::NodeId, BoundField, UiNode, UiTree};

    fn col(key: &str, ft: FieldType) -> ColumnMeta {
        ColumnMeta {
            key: key.into(),
            label_key: format!("column.{key}"),
            field_type: ft,
            sortable: true,
            filterable: true,
            comparator_id: None,
            filter_id: None,
            editor_id: None,
            formatter_id: None,
            action_ids: Vec::new(),
        }
    }

    #[test]
    fn synthesize_preview_rows_fills_every_column() {
        let cols = vec![
            col("name", FieldType::Text),
            col("price", FieldType::Decimal { precision: 2 }),
            col("active", FieldType::Boolean),
        ];
        let rows = synthesize_preview_rows(&cols, 3);
        assert_eq!(rows.len(), 3);
        for row in &rows {
            assert_eq!(row.fields.len(), cols.len());
            for c in &cols {
                assert!(row.fields.contains_key(&c.key));
            }
        }
    }

    #[test]
    fn synthesize_zero_rows_produces_empty_vec() {
        let cols = vec![col("name", FieldType::Text)];
        assert!(synthesize_preview_rows(&cols, 0).is_empty());
    }

    #[test]
    fn placeholder_decimal_respects_precision() {
        let v = placeholder_value(&FieldType::Decimal { precision: 2 }, 0);
        let n = v.as_f64().unwrap();
        // Erwartung: 0.99 mit max. 2 Nachkommastellen
        assert!((n - 0.99).abs() < 1e-9, "got {n}");
    }

    #[test]
    fn placeholder_boolean_alternates() {
        assert_eq!(placeholder_value(&FieldType::Boolean, 0), Value::Bool(true));
        assert_eq!(
            placeholder_value(&FieldType::Boolean, 1),
            Value::Bool(false)
        );
        assert_eq!(placeholder_value(&FieldType::Boolean, 2), Value::Bool(true));
    }

    #[test]
    fn placeholder_enum_cycles_values() {
        let ft = FieldType::Enum {
            values: vec!["new".into(), "paid".into()],
        };
        assert_eq!(placeholder_value(&ft, 0), Value::String("new".into()));
        assert_eq!(placeholder_value(&ft, 1), Value::String("paid".into()));
        assert_eq!(placeholder_value(&ft, 2), Value::String("new".into()));
    }

    #[test]
    fn placeholder_enum_with_no_values_is_null() {
        assert_eq!(
            placeholder_value(&FieldType::Enum { values: vec![] }, 0),
            Value::Null
        );
    }

    #[test]
    fn from_tree_projects_and_synthesizes_columns() {
        let mut t = UiTree::empty();
        t.push_root(UiNode {
            id: NodeId(1),
            bound_field: Some(BoundField {
                key: "name".into(),
                field_type: Some(FieldType::Text),
                label_key: None,
                sortable: None,
                filterable: None,
            }),
            ..UiNode::new(NodeId(1))
        });
        let src = BuilderPreviewSource::from_tree(&t, 2);
        // `inner` ist privat — wir validieren ueber DataSource::fetch in einem
        // sync-blocking-style: LocalSource resolved sofort.
        let fut = src.fetch(DataRequest {
            page: 1,
            page_size: 10,
            sort: None,
            filter: shared::FilterCriteria::default(),
        });
        let res = futures_executor_block_on(fut).expect("fetch ok");
        assert_eq!(res.total_count, 2);
        assert_eq!(res.items.len(), 2);
        assert!(res.items[0].fields.contains_key("name"));
    }

    /// Mini-Executor fuer den Test (LocalSource::fetch ist tatsaechlich
    /// synchron unter der Pin<Box<...>>-Huelle).
    fn futures_executor_block_on<T>(mut fut: super::BoxFuture<T>) -> T {
        use std::task::{Context, Poll, Wake, Waker};
        struct Noop;
        impl Wake for Noop {
            fn wake(self: std::sync::Arc<Self>) {}
        }
        let waker = Waker::from(std::sync::Arc::new(Noop));
        let mut cx = Context::from_waker(&waker);
        loop {
            match fut.as_mut().poll(&mut cx) {
                Poll::Ready(v) => return v,
                Poll::Pending => {}
            }
        }
    }
}
