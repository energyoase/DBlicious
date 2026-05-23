//! Phase 1.7.9: PDF-Renderer-Trait + Stub-Impl.
//!
//! Typst-Backend ist Folge-Item; hier verifizieren wir den Trait-Vertrag
//! mit dem Stub.

use server::pdf::{stub::StubRenderer, PdfRenderer, PdfVars};

#[tokio::test]
async fn stub_renderer_returns_pdf_magic_bytes() {
    let r = StubRenderer;
    let mut vars = PdfVars::new();
    vars.insert("invoice_no".into(), serde_json::json!("INV-2026-000007"));
    let bytes = r.render("hello {invoice_no}", &vars).await.unwrap();
    assert!(bytes.starts_with(b"%PDF-"), "Magic-Bytes fehlen");
    let s = String::from_utf8_lossy(&bytes);
    assert!(s.contains("hello {invoice_no}"));
    assert!(s.contains("INV-2026-000007"));
}

#[tokio::test]
async fn stub_renderer_is_deterministic_for_same_inputs() {
    let r = StubRenderer;
    let mut vars = PdfVars::new();
    vars.insert("x".into(), serde_json::json!(42));
    let a = r.render("t", &vars).await.unwrap();
    let b = r.render("t", &vars).await.unwrap();
    assert_eq!(a, b);
}

#[test]
fn renderer_kind_is_stub() {
    assert_eq!(StubRenderer.kind(), "stub");
}
