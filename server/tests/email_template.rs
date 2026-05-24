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

#[test]
fn html_part_escapes_but_text_part_is_raw() {
    let r = EmailTemplateRenderer::new();
    let tpl = EmailTemplate {
        subject: "x".into(),
        body_text: "{{ x }}".into(),
        body_html: Some("{{ x }}".into()),
    };
    let out = r
        .render(&tpl, &vars(json!({ "x": "<b>hi</b>" })))
        .expect("render");
    // Text-Part: roh durchgereicht.
    assert_eq!(out.body_text, "<b>hi</b>");
    // HTML-Part: escaped. minijinja escaped < > & " ' und auch / (als &#x2f;).
    assert_eq!(out.body_html.as_deref(), Some("&lt;b&gt;hi&lt;&#x2f;b&gt;"));
}

#[test]
fn missing_variable_fails_loudly() {
    use server::email::EmailError;
    let r = EmailTemplateRenderer::new();
    let tpl = EmailTemplate {
        subject: "Hallo {{ missing }}".into(),
        body_text: "egal".into(),
        body_html: None,
    };
    let err = r
        .render(&tpl, &vars(json!({})))
        .expect_err("fehlende Variable muss fehlschlagen");
    assert!(
        matches!(err, EmailError::RenderFailed(_)),
        "erwartet RenderFailed, war: {err:?}"
    );
}

#[test]
fn syntax_error_maps_to_template_invalid() {
    use server::email::EmailError;
    let r = EmailTemplateRenderer::new();
    let tpl = EmailTemplate {
        // ungeschlossener Ausdruck => Parse-/Syntaxfehler
        subject: "Hallo {{ name".into(),
        body_text: "egal".into(),
        body_html: None,
    };
    let err = r
        .render(&tpl, &vars(json!({ "name": "Alice" })))
        .expect_err("Syntaxfehler muss fehlschlagen");
    assert!(
        matches!(err, EmailError::TemplateInvalid(_)),
        "erwartet TemplateInvalid, war: {err:?}"
    );
}
