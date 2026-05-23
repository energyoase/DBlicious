//! Email-Versand (Phase 1.7.10).
//!
//! Trait-basiert. Heute mitgeliefert: [`stub::StubSender`] — sammelt
//! alle "gesendeten" Mails in einem prozessweiten Buffer, gut fuer Tests
//! und CI ohne echten SMTP-Server.
//!
//! Produktives Backend (SMTP via `lettre`) als Folge-Item. Bounce-
//! Handling + DKIM/SPF-Doku gehoeren zum Produktiv-Pfad.
//!
//! Audit: jedes erfolgreiche `send` schreibt einen Eintrag in
//! `audit_log` kind=`email_sent`.

pub mod stub;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmailError {
    #[error("backend: {0}")]
    Backend(String),
    #[error("invalid_input: {0}")]
    InvalidInput(String),
}

/// Eine Email-Nachricht. `body_html` ist optional; ohne wird `body_text`
/// als plain gesendet.
#[derive(Debug, Clone)]
pub struct EmailMessage<'a> {
    pub from:      &'a str,
    pub to:        &'a [&'a str],
    pub cc:        &'a [&'a str],
    pub bcc:       &'a [&'a str],
    pub subject:   &'a str,
    pub body_text: &'a str,
    pub body_html: Option<&'a str>,
    pub attachments: &'a [EmailAttachment<'a>],
}

#[derive(Debug, Clone)]
pub struct EmailAttachment<'a> {
    pub filename: &'a str,
    pub mime:     &'a str,
    pub bytes:    &'a [u8],
}

#[async_trait]
pub trait EmailSender: Send + Sync {
    fn kind(&self) -> &'static str;
    async fn send(&self, msg: EmailMessage<'_>) -> Result<(), EmailError>;
}

/// Schreibt einen Audit-Eintrag pro Send-Aufruf. Aufrufer mit
/// Compliance-Anforderung sollten das nutzen.
pub async fn send_with_audit(
    sender: &dyn EmailSender,
    msg:    EmailMessage<'_>,
    actor:  Option<&str>,
) -> Result<(), EmailError> {
    let result = sender.send(msg.clone()).await;
    let to_join = msg.to.join(",");
    let kind = if result.is_ok() { "email_sent" } else { "email_failed" };
    crate::audit::record_email_event(
        actor,
        msg.from,
        &to_join,
        msg.subject,
        kind,
        result.as_ref().err().map(|e| format!("{e}")),
    )
    .await;
    result
}

// Bequeme Clone-Variante damit send_with_audit das msg duplizieren kann.
impl<'a> EmailMessage<'a> {
    pub fn empty() -> Self {
        Self {
            from:        "",
            to:          &[],
            cc:          &[],
            bcc:         &[],
            subject:     "",
            body_text:   "",
            body_html:   None,
            attachments: &[],
        }
    }
}
