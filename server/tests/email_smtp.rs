//! Phase 1.7.10 Folge-Item: SMTP-Sender ueber `lettre`.
//!
//! Tests laufen ohne echten Mailserver — wir nutzen entweder die pure
//! `build_message`-Funktion (kein Transport noetig) oder lettre's
//! `AsyncStubTransport` ueber [`SmtpSender::stub`]. Damit ist der
//! Konversions-Pfad `EmailMessage → lettre::Message → MIME-Bytes` voll
//! abgedeckt.

use server::email::{
    smtp::{build_message, SmtpSender},
    EmailAttachment, EmailMessage, EmailSender,
};

#[test]
fn build_message_minimal_text_only() {
    let msg = EmailMessage {
        from:        "alice@example.com",
        to:          &["bob@example.com"],
        cc:          &[],
        bcc:         &[],
        subject:     "Hallo",
        body_text:   "Erste Zeile",
        body_html:   None,
        attachments: &[],
    };
    let m = build_message(&msg, None).expect("build");
    let bytes = m.formatted();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("From: alice@example.com"));
    assert!(text.contains("To: bob@example.com"));
    assert!(text.contains("Subject: Hallo"));
    assert!(text.contains("Erste Zeile"));
}

#[test]
fn build_message_uses_default_from_when_empty() {
    let msg = EmailMessage {
        from:        "",
        to:          &["bob@example.com"],
        cc:          &[],
        bcc:         &[],
        subject:     "X",
        body_text:   "Y",
        body_html:   None,
        attachments: &[],
    };
    let m = build_message(&msg, Some("noreply@example.com")).expect("build with default_from");
    let text = String::from_utf8_lossy(&m.formatted()).to_string();
    assert!(text.contains("From: noreply@example.com"), "got: {text}");
}

#[test]
fn build_message_requires_from_or_default() {
    let msg = EmailMessage {
        from:        "",
        to:          &["bob@example.com"],
        cc:          &[],
        bcc:         &[],
        subject:     "X",
        body_text:   "Y",
        body_html:   None,
        attachments: &[],
    };
    let err = build_message(&msg, None).expect_err("must reject without from");
    assert!(format!("{err}").contains("from missing"), "{err}");
}

#[test]
fn build_message_rejects_invalid_to_address() {
    let msg = EmailMessage {
        from:        "alice@example.com",
        to:          &["nicht-eine-email"],
        cc:          &[],
        bcc:         &[],
        subject:     "X",
        body_text:   "Y",
        body_html:   None,
        attachments: &[],
    };
    let err = build_message(&msg, None).expect_err("invalid to");
    assert!(format!("{err}").contains("to 'nicht-eine-email'"), "{err}");
}

#[test]
fn build_message_supports_html_alternative() {
    let msg = EmailMessage {
        from:        "alice@example.com",
        to:          &["bob@example.com"],
        cc:          &[],
        bcc:         &[],
        subject:     "HTML",
        body_text:   "Plain",
        body_html:   Some("<p>Reich</p>"),
        attachments: &[],
    };
    let m = build_message(&msg, None).expect("build");
    let text = String::from_utf8_lossy(&m.formatted()).to_string();
    // MIME-multipart/alternative wird im Header und Body sichtbar.
    assert!(text.contains("multipart/alternative"), "alternative-Part: {text}");
    assert!(text.contains("Plain"), "Plain-Body: {text}");
    assert!(text.contains("<p>Reich</p>"), "HTML-Body: {text}");
}

#[test]
fn build_message_supports_attachment() {
    let att = EmailAttachment {
        filename: "rechnung.pdf",
        mime:     "application/pdf",
        bytes:    b"%PDF-1.4 stub",
    };
    let msg = EmailMessage {
        from:        "alice@example.com",
        to:          &["bob@example.com"],
        cc:          &[],
        bcc:         &[],
        subject:     "Mit Anhang",
        body_text:   "Anbei die Rechnung.",
        body_html:   None,
        attachments: std::slice::from_ref(&att),
    };
    let m = build_message(&msg, None).expect("build");
    let text = String::from_utf8_lossy(&m.formatted()).to_string();
    assert!(text.contains("multipart/mixed"), "mixed-Part fehlt: {text}");
    assert!(text.contains("rechnung.pdf"), "Attachment-Name fehlt: {text}");
    assert!(text.contains("application/pdf"), "Attachment-MIME fehlt: {text}");
}

#[tokio::test]
async fn smtp_stub_sender_records_message_via_lettre_transport() {
    let sender = SmtpSender::stub(Some("noreply@example.com".into()));
    let msg = EmailMessage {
        from:        "alice@example.com",
        to:          &["bob@example.com"],
        cc:          &[],
        bcc:         &[],
        subject:     "Stub-Roundtrip",
        body_text:   "Body",
        body_html:   None,
        attachments: &[],
    };
    sender.send(msg).await.expect("stub send ok");
    let recorded = sender.stub_messages().await;
    assert_eq!(recorded.len(), 1, "Stub muss genau eine Nachricht halten");
    let (_envelope, raw) = &recorded[0];
    assert!(raw.contains("Subject: Stub-Roundtrip"), "{raw}");
    assert!(raw.contains("To: bob@example.com"), "{raw}");
}

#[tokio::test]
async fn smtp_stub_sender_kind_is_smtp_stub() {
    let sender = SmtpSender::stub(None);
    assert_eq!(sender.kind(), "smtp-stub");
}

#[tokio::test]
async fn smtp_stub_sender_default_from_used_when_message_from_empty() {
    let sender = SmtpSender::stub(Some("noreply@example.com".into()));
    let msg = EmailMessage {
        from:        "",
        to:          &["bob@example.com"],
        cc:          &[],
        bcc:         &[],
        subject:     "Default-From",
        body_text:   "Body",
        body_html:   None,
        attachments: &[],
    };
    sender.send(msg).await.unwrap();
    let recorded = sender.stub_messages().await;
    let (_env, raw) = &recorded[0];
    assert!(raw.contains("From: noreply@example.com"), "{raw}");
}
