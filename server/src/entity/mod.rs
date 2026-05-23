//! SeaORM-Entity-Modelle.
//!
//! Jedes Sub-Modul ist eine `#[sea_orm(table_name = "...")]`-Tabelle. Die
//! Reexporte unten sind nur Bequemlichkeit — typische Aufrufe holen sich
//! `entity::entities::Entity` als Query-Wurzel.
//!
//! Konzeptionelle Anmerkungen:
//!
//!   - `entities`         haelt *alle* fachlichen Datensaetze (Product,
//!                        Customer, Order, …) generisch — `fields` ist eine
//!                        JSON-Spalte, der `entity_type`-Discriminator
//!                        bestimmt das logische Schema. Das spiegelt das
//!                        Schema der GraphQL-API (`shared::Entity`).
//!   - `users`/`groups`/`user_groups`  modellieren die Auth-Schicht.
//!   - `sessions`         ersetzt den frueheren In-Memory-Mutex-Store.
//!   - `translatable_*`   die DB-backed Pendants zu den Fluent-Bundles.
//!   - `metadata_*`       Editor-/Settings-Persistenz, heute optional.
//!   - `db_schemas`       der vom Designer gespeicherte `DbSchema`-Snapshot
//!                        (mehr als einer pro Schema-Name moeglich, der
//!                        neueste gewinnt).

pub mod audit_log;
pub mod db_schemas;
pub mod entities;
pub mod entity_designs;
pub mod entity_views;
pub mod groups;
pub mod metadata_editor;
pub mod metadata_settings;
pub mod number_sequences;
pub mod permissions;
pub mod plugin_invocations;
pub mod plugins;
pub mod role_assignments;
pub mod roles;
pub mod script;
pub mod script_audit_log;
pub mod script_version;
pub mod sessions;
pub mod translatable_entries;
pub mod translatable_languages;
pub mod translatable_values;
pub mod user_groups;
pub mod user_implementation_choices;
pub mod users;
