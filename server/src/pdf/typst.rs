//! Typst-PDF-Backend (Phase 1.7.9).
//!
//! VERIFIED API (typst-as-lib 0.15.4 + typst 0.14.2, Spike 2026-05-23):
//!   - Engine:  `TypstEngine::builder().main_file(src)
//!                   .search_fonts_with(TypstKitFontOptions::default()).build()`
//!   - Compile: `engine.compile_with_input(inputs: Dict)
//!                   -> Warned<Result<PagedDocument, TypstAsLibError>>`
//!     Das `Warned { output, warnings }`-Feld `output` traegt das Result.
//!   - Inputs:  als `typst::foundations::Dict`; im Template ueber
//!              `#import sys: inputs` erreichbar.
//!   - Export:  `typst_pdf::pdf(&doc, &PdfOptions::default())
//!                   -> SourceResult<Vec<u8>>`
//!   - Fonts:   typst-kit mit `include_embedded_fonts=true` (default) laedt
//!              die typst-assets-Default-Fonts; kein eigenes TTF noetig und
//!              kein manueller Font-Cache.
//!
//! Vars-Injection: die `PdfVars` werden als **ein** JSON-String unter
//! `sys.inputs.data` uebergeben — nicht per String-Interpolation ins
//! Quell-Template (Injection-Schutz, strukturierte Daten moeglich). Das
//! Template liest sie via `#let data = json(bytes(inputs.data))`.
//!
//! Crate vs. Modul: dieses Modul heisst `typst`, der gleichnamige Crate
//! wird ueber den absoluten Pfad `::typst::...` angesprochen.

use async_trait::async_trait;

use ::typst::foundations::{Dict, IntoValue};
use ::typst::layout::PagedDocument;
use typst_as_lib::typst_kit_options::TypstKitFontOptions;
use typst_as_lib::TypstEngine;
use typst_pdf::{pdf, PdfOptions};

use super::{PdfError, PdfRenderer, PdfVars};

pub struct TypstRenderer;

impl TypstRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TypstRenderer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PdfRenderer for TypstRenderer {
    fn kind(&self) -> &'static str {
        "typst"
    }

    async fn render(&self, template: &str, vars: &PdfVars) -> Result<Vec<u8>, PdfError> {
        let data_json = serde_json::to_string(vars)
            .map_err(|e| PdfError::Render(format!("vars-encode: {e}")))?;

        // Vars als ein JSON-String unter sys.inputs.data.
        let mut inputs = Dict::new();
        inputs.insert("data".into(), data_json.into_value());

        // Engine pro Aufruf: typst-kit laedt embedded Fonts. Das Kompilieren
        // dominiert die Kosten; eine Engine-Wiederverwendung waere eine
        // spaetere Optimierung (Dev/Prod-Asymmetrie — hier nicht noetig).
        let engine = TypstEngine::builder()
            .main_file(template.to_string())
            .search_fonts_with(TypstKitFontOptions::default())
            .build();

        let compiled = engine.compile_with_input::<_, PagedDocument>(inputs);
        let document = compiled
            .output
            .map_err(|e| PdfError::TemplateInvalid(format!("{e:?}")))?;

        pdf(&document, &PdfOptions::default())
            .map_err(|diags| PdfError::Render(format!("pdf-export: {diags:?}")))
    }
}
