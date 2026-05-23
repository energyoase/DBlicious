//! Host-Module — implementiert pro Bereich (i18n, ctx, db, ui, audit) die
//! konkrete Server-Seite des `HostApi`-Vertrags.
//!
//! Jedes Modul ist ein duenner Wrapper: es delegiert an
//! `shared::script::engine::HostApi`-Methoden bzw. an die Server-internen
//! Services (SeaORM, Fluent), und stellt die explizit gebrandete Signatur
//! bereit, die der Rhai-Adapter (Task 2.X) als Native-Function registriert.
//!
//! Sandbox-Gating (Capability-Token-Check) passiert NICHT hier — der
//! `Sandbox::gate(...)`-Aufruf wickelt diese Funktionen ein. So sind die
//! Host-Module testbar, ohne den Sandbox-Pfad nachzubilden.

pub mod audit;
pub mod ctx;
pub mod db;
pub mod i18n;
pub mod ui;
