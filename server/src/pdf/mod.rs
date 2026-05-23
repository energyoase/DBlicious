//! PDF-Renderer (Phase 1.7.9).
//!
//! Trait-basiert, damit Backends austauschbar sind. Heute mitgeliefert:
//! [`stub::StubRenderer`] (deterministischer Test-PDF). Produktives
//! Backend (Typst) als Folge-Item — der Crate ist gross (~30 MB), wir
//! ziehen ihn rein, sobald Templates definiert sind.
//!
//! Templates leben perspektivisch in einer eigenen `pdf_templates`-
//! Tabelle und sind per Designer pflegbar; fuer den Trait-Vertrag reicht
//! der Template-String als Argument.

pub mod stub;

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
