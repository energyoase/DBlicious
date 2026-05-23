//! Stub-Renderer (Phase 1.7.9 MVP).
//!
//! Liefert ein minimal-gueltiges PDF mit einer Textseite, in der das
//! Template + die Variablen als JSON eingebettet sind. Deterministisch
//! (Hash-stabil), funktioniert ohne externe Deps — gut fuer Pipeline-
//! Tests und CI.
//!
//! Produktiver Backend-Wechsel (Typst): einfach eine zweite Impl von
//! `PdfRenderer` registrieren und die Aufrufer-Selection auf
//! `kind() == "typst"` umstellen.

use async_trait::async_trait;

use super::{PdfError, PdfRenderer, PdfVars};

pub struct StubRenderer;

#[async_trait]
impl PdfRenderer for StubRenderer {
    fn kind(&self) -> &'static str { "stub" }

    async fn render(&self, template: &str, vars: &PdfVars) -> Result<Vec<u8>, PdfError> {
        // Minimaler PDF-Body als ASCII-Wrapper. Genug, dass `application/pdf`
        // korrekt detektiert wird (Magic-Bytes "%PDF-").
        let payload = serde_json::to_string(vars)
            .map_err(|e| PdfError::Render(format!("vars-encode: {e}")))?;
        let body = format!(
            "%PDF-1.4\n\
             %% stub-renderer\n\
             %% template:\n{template}\n\
             %% vars:\n{payload}\n\
             %%EOF\n"
        );
        Ok(body.into_bytes())
    }
}
