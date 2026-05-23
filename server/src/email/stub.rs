//! Stub-Email-Sender (Phase 1.7.10 MVP).
//!
//! Hat einen prozessweiten `Buffer`, in den jede "gesendete" Mail
//! abgelegt wird. Tests koennen [`StubSender::sent`] lesen, um zu
//! verifizieren was rausgegangen waere.

use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;

use super::{EmailError, EmailMessage, EmailSender};

#[derive(Debug, Clone)]
pub struct SentRecord {
    pub from: String,
    pub to: Vec<String>,
    pub subject: String,
    pub body_text: String,
    pub attachment_count: usize,
}

fn buffer() -> &'static Mutex<Vec<SentRecord>> {
    static BUF: OnceLock<Mutex<Vec<SentRecord>>> = OnceLock::new();
    BUF.get_or_init(|| Mutex::new(Vec::new()))
}

pub struct StubSender;

impl StubSender {
    /// Snapshot der bisherigen Sendungen.
    pub fn sent() -> Vec<SentRecord> {
        buffer().lock().unwrap().clone()
    }

    /// Setzt den Buffer zurueck — Tests rufen das vor jedem Run.
    pub fn reset() {
        buffer().lock().unwrap().clear();
    }
}

#[async_trait]
impl EmailSender for StubSender {
    fn kind(&self) -> &'static str {
        "stub"
    }

    async fn send(&self, msg: EmailMessage<'_>) -> Result<(), EmailError> {
        if msg.from.is_empty() || msg.to.is_empty() {
            return Err(EmailError::InvalidInput("from/to required".into()));
        }
        let rec = SentRecord {
            from: msg.from.to_string(),
            to: msg.to.iter().map(|s| (*s).to_string()).collect(),
            subject: msg.subject.to_string(),
            body_text: msg.body_text.to_string(),
            attachment_count: msg.attachments.len(),
        };
        buffer().lock().unwrap().push(rec);
        Ok(())
    }
}

// Marker damit Tests den Stub auch dann finden, wenn er via Trait-
// Object verwendet wird.
pub fn boxed() -> Arc<dyn EmailSender> {
    Arc::new(StubSender)
}
