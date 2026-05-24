//! Email-Template-Rendering (Roadmap 1.7.10-Folge).
//!
//! Reine Render-Layer, quellen-agnostisch (Vorbild `crate::pdf`). Ein Template
//! ist ein Buendel dreier Template-Strings (subject/body_text/body_html); der
//! Renderer fuellt sie mit Variablen. Die Template-QUELLE (Loader/DB/Designer)
//! und Locale-Auswahl sind Folge-Items.
//!
//! Sicherheit: Variablen gehen strukturiert (serde) in den Render-Kontext,
//! keine String-Konkatenation. Der HTML-Part wird autoescaped, subject/text
//! nicht (Per-Part-Autoescape ueber Template-Namens-Suffix).

use minijinja::{AutoEscape, Environment, ErrorKind, UndefinedBehavior};
use serde::{Deserialize, Serialize};

use crate::email::{EmailAttachment, EmailError, EmailMessage};

/// Variablen-Map, analog `crate::pdf::PdfVars`.
pub type EmailVars = serde_json::Map<String, serde_json::Value>;

/// Ein Email-Template als Buendel dreier Template-Strings. Quellen-agnostisch:
/// Loader/DB/Designer produzieren das spaeter (Folge-Item). `Deserialize`,
/// damit ein kuenftiger `email-templates/<id>.toml`-Sidecar direkt hineinlaedt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmailTemplate {
    pub subject: String,
    pub body_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_html: Option<String>,
}

/// Gerendertes Ergebnis. Besitzt die Strings, damit es eine `EmailMessage<'a>`
/// per Borrow speisen kann (siehe `as_message`).
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedEmail {
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
}

/// Reiner Template-Renderer. Haelt eine einmal konfigurierte minijinja-Engine.
pub struct EmailTemplateRenderer {
    env: Environment<'static>,
}

impl Default for EmailTemplateRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl EmailTemplateRenderer {
    pub fn new() -> Self {
        let mut env = Environment::new();
        // Fehlende Variable => Render bricht ab (statt still leer zu rendern).
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        // Per-Part-Autoescape: nur *.html escapen, *.txt (subject/text) nicht.
        env.set_auto_escape_callback(|name| {
            if name.ends_with(".html") {
                AutoEscape::Html
            } else {
                AutoEscape::None
            }
        });
        Self { env }
    }

    /// Rendert alle drei Parts. `body_html` nur, wenn im Template gesetzt.
    pub fn render(
        &self,
        tpl: &EmailTemplate,
        vars: &EmailVars,
    ) -> Result<RenderedEmail, EmailError> {
        let subject = self.render_part("subject.txt", &tpl.subject, vars)?;
        let body_text = self.render_part("body.txt", &tpl.body_text, vars)?;
        let body_html = match &tpl.body_html {
            Some(src) => Some(self.render_part("body.html", src, vars)?),
            None => None,
        };
        Ok(RenderedEmail {
            subject,
            body_text,
            body_html,
        })
    }

    fn render_part(
        &self,
        name: &str,
        source: &str,
        vars: &EmailVars,
    ) -> Result<String, EmailError> {
        self.env
            .render_named_str(name, source, vars)
            .map_err(|e| map_minijinja_err(&e))
    }
}

/// Syntaxfehler (Parse) => `TemplateInvalid`; alles andere (inkl.
/// strict-undefined) => `RenderFailed`.
fn map_minijinja_err(e: &minijinja::Error) -> EmailError {
    if e.kind() == ErrorKind::SyntaxError {
        EmailError::TemplateInvalid(format!("{e:#}"))
    } else {
        EmailError::RenderFailed(format!("{e:#}"))
    }
}

impl RenderedEmail {
    /// Baut eine ausleihende `EmailMessage`. `self` (subject/body) wird
    /// geborgt, Empfaenger + Attachments kommen vom Aufrufer.
    pub fn as_message<'a>(
        &'a self,
        from: &'a str,
        to: &'a [&'a str],
        cc: &'a [&'a str],
        bcc: &'a [&'a str],
        attachments: &'a [EmailAttachment<'a>],
    ) -> EmailMessage<'a> {
        EmailMessage {
            from,
            to,
            cc,
            bcc,
            subject: &self.subject,
            body_text: &self.body_text,
            body_html: self.body_html.as_deref(),
            attachments,
        }
    }
}
