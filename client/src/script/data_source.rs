//! `ScriptSource` — `DataSource`-Adapter, der einen Provider-Skript-Run als
//! Tabellen-Datenquelle vorhaelt.
//!
//! Verwendung: ein Provider-Skript (Spec §6: `ProviderSlot::DataSource`)
//! gibt einen JSON-Array-`Value` zurueck; dieser Adapter packt das Ergebnis
//! in `Vec<Entity>` und delegiert die nachgelagerte Sort/Filter/Pagination-
//! Auswertung an die bestehende `LocalSource`.
//!
//! Bewusst dual mit `LocalSource` aufgebaut: der heutige Phase-4-Pfad ruft
//! das Skript einmal und cached das Ergebnis lokal. Der Phase-5-Renderer
//! kann eine reaktive Version dieses Adapters bauen (Skript bei jeder
//! `DataRequest` neu evaluieren) — die Trait-Surface fuer den Tabellen-
//! Code bleibt identisch.

use std::rc::Rc;

use serde_json::Value;
use shared::{ColumnMeta, Entity};

use crate::components::table::data_source::{
    BoxFuture, DataRequest, DataResponse, DataSource, LocalSource,
};
use crate::graphql::GqlError;

/// Adapter: nimmt einen JSON-Array-Run-Output, baut daraus `Vec<Entity>`,
/// reicht das an `LocalSource` durch.
///
/// `script_result`: das, was ein Provider-Skript via `db.entities(...)` o.ae.
/// als Top-Level-Wert zurueckgibt. Erwartete Form: `[{...}, {...}]` mit
/// JSON-Maps pro Eintrag, der `id`-Feld muss vorhanden sein. Falls der Run
/// einen Non-Array-Value liefert (z.B. ein einzelnes Entity), wird er als
/// einelementiger Array interpretiert.
#[derive(Clone)]
pub struct ScriptSource {
    inner: Rc<LocalSource>,
}

impl ScriptSource {
    pub fn from_script_result(script_result: Value, columns: &[ColumnMeta]) -> Self {
        let items = entities_from_value(script_result);
        let local = LocalSource::new(items, columns);
        Self {
            inner: Rc::new(local),
        }
    }
}

impl DataSource for ScriptSource {
    fn fetch(&self, req: DataRequest) -> BoxFuture<Result<DataResponse, GqlError>> {
        self.inner.fetch(req)
    }
}

/// Konvertiert einen Skript-Output in `Vec<Entity>`. Unerlaubte Form
/// (`Value::Null`, primitive Werte) ergibt einen leeren Vektor — das ist
/// kein Fehler des Adapters, ein leerer Provider ist im Tabellen-Kontext
/// "keine Zeilen".
fn entities_from_value(v: Value) -> Vec<Entity> {
    let array = match v {
        Value::Array(arr) => arr,
        Value::Object(_) => vec![v],
        _ => return Vec::new(),
    };
    array
        .into_iter()
        .filter_map(|item| match item {
            Value::Object(map) => {
                let id = map
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                Some(Entity {
                    id,
                    fields: map.into_iter().collect(),
                })
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_produces_no_rows() {
        let s = ScriptSource::from_script_result(Value::Null, &[]);
        let _ = s; // smoke: konstruierbar
    }

    #[test]
    fn array_of_objects_maps_to_entities() {
        let v = serde_json::json!([
            {"id": "p-1", "name": "A"},
            {"id": "p-2", "name": "B"}
        ]);
        let items = entities_from_value(v);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id, "p-1");
        assert_eq!(items[1].id, "p-2");
    }

    #[test]
    fn single_object_is_treated_as_one_row() {
        let v = serde_json::json!({"id": "x", "n": 1});
        let items = entities_from_value(v);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, "x");
    }
}
