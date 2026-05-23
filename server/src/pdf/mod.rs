//! PDF-Renderer (Phase 1.7.9).
//!
//! Trait-basiert, damit Backends austauschbar sind. Backend-Auswahl:
//!   - [`stub::StubRenderer`] (`kind="stub"`): deterministischer ASCII-
//!     PDF mit `%PDF-`-Magic, deps-frei — CI-Default und Debug-Dump von
//!     Template+Vars.
//!   - [`typst::TypstRenderer`] (`kind="typst"`): echtes Typesetting ueber
//!     typst-as-lib + embedded Fonts; Vars kommen als JSON unter
//!     `sys.inputs.data` ins Template (kein String-Interpolations-
//!     Injection-Risiko).
//!
//! Produktive Aufrufer waehlen ueber `kind()`. Die Quelle des Templates
//! (Loader-Datei `pdf-templates/<id>.typ`, kuenftig `pdf_templates`-
//! Tabelle + Designer) ist orthogonal zum Renderer — Roadmap 1.7.9
//! Folge-Item.

pub mod stub;
pub mod typst;

pub use typst::TypstRenderer;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("template_invalid: {0}")]
    TemplateInvalid(String),
    #[error("render: {0}")]
    Render(String),
}

/// Variablen-Map, die einem Template uebergeben werden.
pub type PdfVars = serde_json::Map<String, serde_json::Value>;

#[async_trait]
pub trait PdfRenderer: Send + Sync {
    fn kind(&self) -> &'static str;
    /// Rendert das Template mit Variablen zu PDF-Bytes.
    async fn render(&self, template: &str, vars: &PdfVars) -> Result<Vec<u8>, PdfError>;
}
