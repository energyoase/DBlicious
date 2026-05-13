//! Generische Tabelle fuer Entitaeten.
//!
//! Module:
//!   - `column`      : Spalten-Definitionen (lokal hartkodiert, in spaeteren
//!                     Versionen vom Server geliefert)
//!   - `data_source` : Trait-basierter Zugriff auf Daten – heute `RemoteSource`
//!                     fuer Server-seitige Beladung, vorbereitet auch fuer
//!                     `LocalSource` und client-seitige Sortierung/Filterung.
//!   - `formatters`  : Formatierungslogik fuer Skalar-Typen mit i18n
//!   - `state`       : `TableState` mit Sortier-, Filter-, Pagination-Stubs
//!   - `view`        : eigentliche `<EntityTable>`-Komponente

pub mod column;
pub mod data_source;
pub mod formatters;
pub mod state;
pub mod view;

pub use column::{column_set_for, ColumnSet};
pub use data_source::{DataRequest, DataSource, LocalSource, RemoteSource};
pub use state::TableState;
pub use view::EntityTable;
