//! Generische Tabelle fuer Entitaeten.
//!
//! Module:
//!   - `data_source` : Trait-basierter Zugriff auf Daten – heute `RemoteSource`
//!                     fuer Server-seitige Beladung, vorbereitet auch fuer
//!                     `LocalSource` und client-seitige Sortierung/Filterung.
//!   - `formatters`  : Formatierungslogik fuer Skalar-Typen mit i18n
//!   - `state`       : `TableState` mit Sortier-, Filter-, Pagination-Stubs
//!   - `view`        : eigentliche `<EntityTable>`-Komponente
//!
//! Spalten-Metadaten werden ausschliesslich vom Server geliefert
//! (`graphql::queries::fetch_columns`).

pub mod data_source;
pub mod formatters;
pub mod state;
pub mod view;

pub use data_source::{DataRequest, DataSource, LocalSource, RemoteSource};
pub use state::TableState;
pub use view::EntityTable;
