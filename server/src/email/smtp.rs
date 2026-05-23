//! SMTP-Email-Sender ueber `lettre` (Phase 1.7.10 Folge-Item).
//!
//! Produktiver Pfad — Verschluesselung via STARTTLS oder Implicit-TLS
//! ueber rustls; SMTP-AUTH wahlweise PLAIN/LOGIN/CRAM-MD5 (lettre wirft
//! das passende automatisch raus).
//!
//! Defence-in-depth:
//!   - Header werden ueber den lettre-Builder gesetzt; eingehende Werte
//!     muessen RFC-5322-konforme Adressen sein (sonst `InvalidInput`).
//!   - DKIM/SPF/DMARC sind Domain-Konfiguration, kein Code — siehe
//!     `docs/superpowers/` (TODO Folge-Item) fuer das Operator-Handbuch.
//!   - Bounce-Handling ist Subscriber-seitig (IMAP/SMTP-Reverse-Lookup);
//!     der heutige Sender liefert nur "konnte abgegeben werden", nicht
//!     "wurde zugestellt".

use async_trait::async_trait;
use lettre::{
    address::AddressError,
    message::{
        header::ContentType, Attachment, Mailbox, Mailboxes, Message, MultiPart, SinglePart,
    },
    transport::smtp::{
        authentication::Credentials, client::TlsParameters, AsyncSmtpTransport, AsyncSmtpTransportBuilder,
    },
    transport::stub::AsyncStubTransport,
    AsyncTransport, Tokio1Executor,
};

use super::{EmailError, EmailMessage, EmailSender};

/// Konfiguration fuer den SMTP-Sender. Aufrufer (i.d.R. `main.rs` oder
/// CCM-Daemon-Boot) bauen das aus dem Keyring / `config.toml` zusammen
/// und reichen es einmalig in [`SmtpSender::new`].
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    /// FQDN des Mailservers (z.B. `"smtp.example.com"`).
    pub host: String,
    /// Port (typ. 465 fuer Implicit-TLS, 587 fuer STARTTLS).
    pub port: u16,
    /// SMTP-AUTH-Username (oft die Mail-Adresse).
    pub username: Option<String>,
    /// SMTP-AUTH-Passwort.
    pub password: Option<String>,
    /// `true` ⇒ Implicit-TLS auf Port 465. `false` ⇒ STARTTLS (Pflicht
    /// nach EHLO; reiner Plaintext wird nicht angeboten).
    pub implicit_tls: bool,
}

/// Zwei Backing-Varianten unter derselben Trait-Surface:
/// - `Smtp`: echter `AsyncSmtpTransport` ueber rustls.
/// - `Stub`: lettre's `AsyncStubTransport` — Tests nutzen das, weil ein
///   echtes SMTP-Endpoint im CI nicht zumutbar ist. Verhaelt sich wie
///   `Smtp` aus Sicht des Callers; Inhalte landen im internen Puffer
///   und sind ueber `AsyncStubTransport::messages` abrufbar.
pub enum SmtpSender {
    Smtp {
        transport: AsyncSmtpTransport<Tokio1Executor>,
        default_from: Option<String>,
    },
    Stub {
        transport: AsyncStubTransport,
        default_from: Option<String>,
    },
}

impl SmtpSender {
    /// Baut den produktiven Sender. `default_from` ist optional — fehlt
    /// es, MUSS jede `EmailMessage` ein `from` setzen.
    pub fn new(cfg: SmtpConfig, default_from: Option<String>) -> Result<Self, EmailError> {
        let tls = TlsParameters::new(cfg.host.clone())
            .map_err(|e| EmailError::Backend(format!("tls_params: {e}")))?;
        let builder: AsyncSmtpTransportBuilder = if cfg.implicit_tls {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host)
                .tls(lettre::transport::smtp::client::Tls::Wrapper(tls))
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&cfg.host)
                .tls(lettre::transport::smtp::client::Tls::Required(tls))
        };
        let mut builder = builder.port(cfg.port);
        if let (Some(u), Some(p)) = (cfg.username.as_ref(), cfg.password.as_ref()) {
            builder = builder.credentials(Credentials::new(u.clone(), p.clone()));
        }
        Ok(Self::Smtp {
            transport: builder.build(),
            default_from,
        })
    }

    /// Test-Konstruktor — gibt einen Stub-Transport zurueck, dessen
    /// Inhalte ueber [`SmtpSender::stub_messages`] ausgelesen werden
    /// koennen.
    pub fn stub(default_from: Option<String>) -> Self {
        Self::Stub {
            transport: AsyncStubTransport::new_ok(),
            default_from,
        }
    }

    /// Snapshot aller bisherigen Stub-Nachrichten. Panic, wenn der
    /// Sender nicht im Stub-Mode laeuft.
    pub async fn stub_messages(&self) -> Vec<(lettre::address::Envelope, String)> {
        match self {
            SmtpSender::Stub { transport, .. } => transport.messages().await,
            SmtpSender::Smtp { .. } => panic!("stub_messages() nur im Stub-Mode"),
        }
    }

    fn default_from(&self) -> Option<&str> {
        match self {
            SmtpSender::Smtp { default_from, .. } => default_from.as_deref(),
            SmtpSender::Stub { default_from, .. } => default_from.as_deref(),
        }
    }
}

/// Baut einen lettre-`Message` aus dem internen `EmailMessage`. Pure
/// Funktion — die Test-Suite verifiziert das hier ohne Transport.
pub fn build_message(
    msg: &EmailMessage<'_>,
    default_from: Option<&str>,
) -> Result<Message, EmailError> {
    let from_str = if msg.from.is_empty() {
        default_from.ok_or_else(|| EmailError::InvalidInput("from missing".into()))?
    } else {
        msg.from
    };
    if msg.to.is_empty() {
        return Err(EmailError::InvalidInput("to required".into()));
    }

    let from_mb: Mailbox = from_str
        .parse()
        .map_err(|e: AddressError| EmailError::InvalidInput(format!("from: {e}")))?;

    let to_mbs: Mailboxes = msg
        .to
        .iter()
        .map(|t| {
            t.parse::<Mailbox>()
                .map_err(|e: AddressError| EmailError::InvalidInput(format!("to '{t}': {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect();

    let cc_mbs: Mailboxes = msg
        .cc
        .iter()
        .map(|t| {
            t.parse::<Mailbox>()
                .map_err(|e: AddressError| EmailError::InvalidInput(format!("cc '{t}': {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect();

    let bcc_mbs: Mailboxes = msg
        .bcc
        .iter()
        .map(|t| {
            t.parse::<Mailbox>()
                .map_err(|e: AddressError| EmailError::InvalidInput(format!("bcc '{t}': {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect();

    let mut builder = Message::builder()
        .from(from_mb)
        .subject(msg.subject);
    for mb in to_mbs {
        builder = builder.to(mb);
    }
    for mb in cc_mbs {
        builder = builder.cc(mb);
    }
    for mb in bcc_mbs {
        builder = builder.bcc(mb);
    }

    // Body / Attachments / HTML — Entscheidungsbaum:
    //   - keine Attachments + kein HTML  ⇒ einfacher Text-Body
    //   - keine Attachments + HTML       ⇒ MultiPart::alternative
    //   - Attachments                    ⇒ MultiPart::mixed (mit
    //     alternative-Subpart, falls HTML gesetzt)
    let body = build_body(msg)?;

    builder
        .multipart(body)
        .map_err(|e| EmailError::InvalidInput(format!("build: {e}")))
}

fn build_body(msg: &EmailMessage<'_>) -> Result<MultiPart, EmailError> {
    let text_part = SinglePart::builder()
        .header(ContentType::TEXT_PLAIN)
        .body(msg.body_text.to_string());

    let body = if let Some(html) = msg.body_html {
        let html_part = SinglePart::builder()
            .header(ContentType::TEXT_HTML)
            .body(html.to_string());
        MultiPart::alternative()
            .singlepart(text_part)
            .singlepart(html_part)
    } else {
        // alternative mit nur Text-Part ist auch erlaubt; haelt den
        // Multipart-Pfad einheitlich.
        MultiPart::alternative().singlepart(text_part)
    };

    if msg.attachments.is_empty() {
        return Ok(body);
    }

    let mut mixed = MultiPart::mixed().multipart(body);
    for att in msg.attachments {
        let ct: ContentType = att
            .mime
            .parse()
            .map_err(|e| EmailError::InvalidInput(format!("attachment mime '{}': {e}", att.mime)))?;
        let att_part = Attachment::new(att.filename.to_string())
            .body(att.bytes.to_vec(), ct);
        mixed = mixed.singlepart(att_part);
    }
    Ok(mixed)
}

#[async_trait]
impl EmailSender for SmtpSender {
    fn kind(&self) -> &'static str {
        match self {
            SmtpSender::Smtp { .. } => "smtp",
            SmtpSender::Stub { .. } => "smtp-stub",
        }
    }

    async fn send(&self, msg: EmailMessage<'_>) -> Result<(), EmailError> {
        let lettre_msg = build_message(&msg, self.default_from())?;
        match self {
            SmtpSender::Smtp { transport, .. } => transport
                .send(lettre_msg)
                .await
                .map(|_| ())
                .map_err(|e| EmailError::Backend(format!("smtp_send: {e}"))),
            SmtpSender::Stub { transport, .. } => transport
                .send(lettre_msg)
                .await
                .map(|_| ())
                .map_err(|e| EmailError::Backend(format!("stub_send: {e}"))),
        }
    }
}
