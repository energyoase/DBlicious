//! Phase 1.7.9: Typst-PDF-Backend.
//!
//! Verifiziert den `EmailMessage`-losen Render-Pfad: Typst-Source + Vars
//! → PDF-Bytes. Drei Achsen: gueltiges Template liefert PDF-Magic,
//! ungueltiges Template liefert `TemplateInvalid`, injizierte Vars
//! tauchen im extrahierten PDF-Text auf.
//!
//! Kein externer Mailserver/Binary noetig — typst-as-lib rendert
//! in-process mit embedded Fonts.

use serde_json::json;
use server::pdf::{PdfError, PdfRenderer, TypstRenderer};

fn vars(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

fn invoice_template() -> String {
    std::fs::read_to_string("../examples/shop/pdf-templates/invoice.typ")
        .expect("invoice.typ fixture lesbar")
}

#[tokio::test]
async fn renders_valid_template_to_pdf_magic_bytes() {
    let r = TypstRenderer::new();
    let v = vars(&[
        ("invoice_no", json!("INV-2026-000007")),
        ("customer_name", json!("ACME GmbH")),
        ("total", json!("119.00")),
    ]);
    let pdf = r.render(&invoice_template(), &v).await.expect("render ok");
    assert!(pdf.len() > 1000, "PDF sollte > 1KB sein, war {}", pdf.len());
    assert_eq!(&pdf[..5], b"%PDF-", "PDF-Magic-Bytes erwartet");
}

#[tokio::test]
async fn invalid_template_yields_template_invalid_error() {
    // Unbalancierte Klammer ⇒ Typst-Compile-Fehler.
    let r = TypstRenderer::new();
    let err = r.render("#let x = (", &vars(&[])).await.unwrap_err();
    assert!(
        matches!(err, PdfError::TemplateInvalid(_)),
        "erwartete TemplateInvalid, war {err:?}"
    );
}

#[tokio::test]
async fn rendered_pdf_contains_injected_vars() {
    let r = TypstRenderer::new();
    let v = vars(&[
        ("invoice_no", json!("INV-2026-000042")),
        ("customer_name", json!("Mueller KG")),
        ("total", json!("250.00")),
    ]);
    let pdf = r.render(&invoice_template(), &v).await.unwrap();
    let text = pdf_extract::extract_text_from_mem(&pdf).expect("text-extract");
    assert!(
        text.contains("INV-2026-000042"),
        "invoice_no fehlt im PDF-Text: {text:?}"
    );
    assert!(
        text.contains("Mueller KG"),
        "customer_name fehlt im PDF-Text: {text:?}"
    );
}

#[tokio::test]
async fn empty_vars_still_render_with_template_defaults() {
    // Ohne Vars greifen die `default:`-Werte im Template — kein Fehler.
    let r = TypstRenderer::new();
    let pdf = r
        .render(&invoice_template(), &vars(&[]))
        .await
        .expect("render ok");
    assert_eq!(&pdf[..5], b"%PDF-");
}
