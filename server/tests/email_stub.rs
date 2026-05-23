//! Phase 1.7.10: Email-Trait + Stub-Backend.

use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serial_test::serial;

use server::email::{self, stub::StubSender, EmailMessage, EmailSender};

#[tokio::test]
#[serial]
async fn stub_send_records_message() {
    server::fresh_test_setup().await;
    StubSender::reset();
    let sender = StubSender;
    let msg = EmailMessage {
        from: "noreply@example.com",
        to: &["alice@example.com"],
        cc: &[],
        bcc: &[],
        subject: "Rechnung INV-2026-000007",
        body_text: "Anbei Ihre Rechnung.",
        body_html: None,
        attachments: &[],
    };
    sender.send(msg).await.unwrap();
    let sent = StubSender::sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].subject, "Rechnung INV-2026-000007");
    assert_eq!(sent[0].to, vec!["alice@example.com".to_string()]);
}

#[tokio::test]
#[serial]
async fn missing_from_or_to_returns_invalid_input() {
    server::fresh_test_setup().await;
    StubSender::reset();
    let sender = StubSender;
    let err = sender.send(EmailMessage::empty()).await.unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("invalid_input"), "got: {msg}");
}

#[tokio::test]
#[serial]
async fn send_with_audit_writes_email_sent_kind() {
    server::fresh_test_setup().await;
    StubSender::reset();
    let sender = StubSender;
    let msg = EmailMessage {
        from: "noreply@x",
        to: &["a@b"],
        cc: &[],
        bcc: &[],
        subject: "T",
        body_text: "T",
        body_html: None,
        attachments: &[],
    };
    email::send_with_audit(&sender, msg, Some("alice"))
        .await
        .unwrap();

    let conn = server::db::conn();
    use server::entity::audit_log;
    let rows = audit_log::Entity::find()
        .filter(audit_log::Column::Kind.eq("email_sent"))
        .all(&conn)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].actor_user_id.as_deref(), Some("alice"));
    assert_eq!(rows[0].resource_kind.as_deref(), Some("email"));
}

#[tokio::test]
#[serial]
async fn send_with_audit_writes_email_failed_on_error() {
    server::fresh_test_setup().await;
    StubSender::reset();
    let sender = StubSender;
    let _ = email::send_with_audit(&sender, EmailMessage::empty(), Some("bob")).await;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    let conn = server::db::conn();
    use server::entity::audit_log;
    let rows = audit_log::Entity::find()
        .filter(audit_log::Column::Kind.eq("email_failed"))
        .all(&conn)
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}
