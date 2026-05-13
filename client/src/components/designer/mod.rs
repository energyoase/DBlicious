//! Visueller Datenbank-Designer.
//!
//! Skipper18-inspirierte Oberflaeche zum Modellieren von Tabellen, Spalten
//! und Beziehungen. Aufbau:
//!
//! - `model`     – reaktiver Zustand (Tabellen, Spalten, Beziehungen, Auswahl,
//!                 ID-Generator, Drag- und Verknuepfungs-Zustand). Alle Mutationen
//!                 laufen ueber Methoden auf `DesignerModel`.
//! - `canvas`    – die eigentliche Leinwand mit einer SVG-Ebene fuer
//!                 Verbindungslinien und absolut positionierten Tabellenkarten.
//! - `table_box` – Karte einer einzelnen Tabelle: Drag-Handle, Spaltenliste,
//!                 Inline-Editoren, Lol-Ports zum Verknuepfen.
//! - `connector` – reine Berechnung von Bezier-Pfaden zwischen zwei Ports.
//! - `toolbar`   – Aktionsleiste: Tabelle hinzufuegen, Verknuepfungsmodus,
//!                 Speichern + Status.
//!
//! Der Designer ist bewusst von der GraphQL-Schicht entkoppelt: die einzige
//! Server-Beruehrung ist die `save_db_schema`-Mutation, die in
//! `toolbar.rs` getriggert wird.

pub mod canvas;
pub mod connector;
pub mod model;
pub mod table_box;
pub mod toolbar;

pub use canvas::Designer;
