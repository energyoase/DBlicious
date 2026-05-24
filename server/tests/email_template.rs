//! Email-Template-Rendering (Roadmap 1.7.10-Folge). Pure Render-Tests:
//! kein DB/Audit, kein #[serial], synchron.

use serde_json::json;
use server::email::template::{EmailTemplate, EmailTemplateRenderer, EmailVars};

/// Baut eine `EmailVars`-Map aus einem JSON-Objekt-Literal.
fn vars(v: serde_json::Value) -> EmailVars {
    v.as_object().cloned().expect("vars muss ein JSON-Objekt sein")
}

#[test]
fn renders_simple_substitution_in_all_parts() {
    let r = EmailTemplateRenderer::new();
    let tpl = EmailTemplate {
        subject: "Rechnung {{ number }}".into(),
        body_text: "Hallo {{ name }}, anbei Ihre Rechnung.".into(),
        body_html: Some("<p>Hallo {{ name }}</p>".into()),
    };
    let out = r
        .render(&tpl, &vars(json!({ "number": "INV-7", "name": "Alice" })))
        .expect("render");
    assert_eq!(out.subject, "Rechnung INV-7");
    assert_eq!(out.body_text, "Hallo Alice, anbei Ihre Rechnung.");
    assert_eq!(out.body_html.as_deref(), Some("<p>Hallo Alice</p>"));
}
