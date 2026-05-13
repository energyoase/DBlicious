//! Leptos-WebAssembly-Client.
//!
//! Modulgliederung:
//!   - `app`        : Top-Level-App-Komponente, Router und Layout
//!   - `components` : Wiederverwendbare UI-Bausteine (Navigation, Tabelle, ...)
//!   - `graphql`    : GraphQL-Client und Query-Definitionen
//!   - `i18n`       : Project-Fluent-basierte Lokalisierung
//!   - `routes`     : Routen-Konfiguration
//!   - `styling`    : Abstraktionsschicht ueber das Design-System
//!
//! Die Trennung zwischen Struktur (Komponenten) und Darstellung (Styling) ist
//! ein zentrales Architekturziel: Komponenten dependieren ausschliesslich auf
//! das `DesignSystem`-Trait, nie auf konkrete CSS-Klassen oder Style-Strings.

pub mod app;
pub mod auth;
pub mod builder;
pub mod commands;
pub mod components;
pub mod graphql;
pub mod header;
pub mod i18n;
pub mod routes;
pub mod styling;
pub mod tabs;
pub mod validation;

pub use app::App;
