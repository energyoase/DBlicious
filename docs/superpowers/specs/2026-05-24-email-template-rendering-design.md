# Email-Template-Rendering — Design (Roadmap 1.7.10-Folge)

Date: 2026-05-24
Status: approved (brainstorming)
Roadmap-Bezug: Phase 1.7.10 — „Template-Rendering" war als Folge-Item zu
`server/src/email/` (Stub + SMTP) vermerkt.

## Kontext

`server/src/email/` liefert heute eine reine Versand-Service-Layer:

- `EmailSender`-Trait + `StubSender` (In-Memory-Buffer) + `SmtpSender` (lettre).
- `send_with_audit(...)` schreibt pro Versand einen `audit_log`-Eintrag
  (`email_sent` / `email_failed`).
- `EmailMessage<'a>` trägt bereits **fertig gerenderte** Strings
  (`subject`, `body_text`, `body_html: Option`).
- **Kein Aufrufer** im Codebase — das Modul ist eine isolierte Service-Layer.

Was fehlt: ein Schritt **vor** dem Versand, der ein Template + Variablen zu
`subject`/`body_text`/`body_html` rendert. Das ist diese Arbeit.

Vorbild ist `server/src/pdf/` (1.7.9): `PdfRenderer::render(template: &str,
vars: &PdfVars)` ist bewusst **quellen-agnostisch** — die Template-Quelle
(Loader-Datei, DB-Tabelle, Designer) ist ein orthogonales Folge-Item, und
Variablen kommen strukturiert als JSON herein, nie per String-Konkatenation
(kein Interpolations-Injection-Risiko).

## Ziel

Eine **reine Render-Layer** in `server/src/email/template.rs`:
`(EmailTemplate, EmailVars) -> RenderedEmail`. Quelle der Templates und
Locale-Auswahl bleiben Folge-Items, aber die Typen sind so geschnitten, dass
beide später ohne Umbau andocken.

## Scope

**Drin (v1):**
- `EmailTemplateRenderer` + Typen (`EmailTemplate`, `EmailVars`,
  `RenderedEmail`) in `server/src/email/template.rs`.
- minijinja-basiertes Rendering mit Per-Part-Autoescape und Strict-Undefined.
- Helper `RenderedEmail::as_message(...)` zur Einspeisung in `EmailMessage<'a>`.
- Tests in `server/tests/email_template.rs`.

**Folge-Items (explizit raus):**
- **Template-Quelle**: Loader-Sidecar `email-templates/<id>.toml` bzw.
  `email_templates`-Tabelle + Designer (Variante 2 des Scopes).
- **Locale/i18n**: Locale-abhängige Template-Auswahl und `t()`-Zugriff auf den
  `translatable_bundle` (Variante 3).
- **Versand-Aufrufer**: GraphQL-Mutation / Service-Aufruf, der ein Template
  rendert und versendet (würde `server/src/schema.rs` berühren — bewusst
  ausgelassen, hält die Arbeit isoliert).
- **Bounce-Handling, DKIM/SPF/DMARC** — bereits in `email/mod.rs` als
  Operator-/Folge-Themen markiert.

## Architektur

Neue Datei `server/src/email/template.rs`, eingebunden via `pub mod template;`
in `server/src/email/mod.rs`. **Keine** Änderung an `loader.rs`, `schema.rs`,
`data.rs` oder dem `ExampleSet`.

Eine neue Server-Dependency: `minijinja = "2"` (pure Rust, sandboxed,
eingebautes HTML-Autoescaping). Server-only — kein WASM-Größenbudget betroffen.

**Kein Trait.** PDF nutzt einen Trait, weil es zwei echte Backends gibt
(deps-freier Stub für CI + Typst). minijinja rendert in CI deterministisch und
ohne externe Ressourcen, also gibt es kein zweites Backend → ein konkreter
`EmailTemplateRenderer`-Struct genügt (YAGNI).

## Kerntypen

Alle in `server/src/email/template.rs`:

```rust
/// Variablen-Map, analog `pdf::PdfVars`.
pub type EmailVars = serde_json::Map<String, serde_json::Value>;

/// Ein Email-Template als Bündel dreier Template-Strings. Quellen-agnostisch:
/// Loader/DB/Designer produzieren das später (Folge-Item). `Deserialize`,
/// damit ein künftiger `email-templates/<id>.toml`-Sidecar direkt hineinlädt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmailTemplate {
    pub subject: String,            // Template-String (plain)
    pub body_text: String,          // Template-String (plain)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_html: Option<String>,  // Template-String (HTML, autoescaped)
}

/// Gerendertes Ergebnis. Besitzt die Strings, damit es eine `EmailMessage<'a>`
/// per Borrow speisen kann (das Rendered-Ergebnis lebt länger als die Message).
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedEmail {
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
}

pub struct EmailTemplateRenderer { /* minijinja::Environment */ }
```

### Renderer-API

```rust
impl EmailTemplateRenderer {
    pub fn new() -> Self;  // Strict-Undefined, Per-Part-Autoescape konfiguriert
    pub fn render(&self, tpl: &EmailTemplate, vars: &EmailVars)
        -> Result<RenderedEmail, EmailError>;
}
```

### Brücke zu `EmailMessage`

```rust
impl RenderedEmail {
    pub fn as_message<'a>(
        &'a self,
        from: &'a str,
        to: &'a [&'a str],
        cc: &'a [&'a str],
        bcc: &'a [&'a str],
        attachments: &'a [EmailAttachment<'a>],
    ) -> EmailMessage<'a>;
}
```

Damit bleibt die bestehende `EmailMessage<'a>`-Borrow-Form unangetastet; der
Renderer ist additiv.

## Render-Verhalten

1. **Per-Part-Autoescape (Injection-Grenze).** `body_html` wird HTML-escaped,
   `subject` und `body_text` nicht. Umsetzung über minijinja-Autoescape nach
   Template-Name: der HTML-Part wird unter einem Namen mit `.html`-Endung
   registriert (Default-Autoescape greift), Subject/Text unter `.txt`-Namen.
   Konsequenz: eine Variable mit `<b>x</b>` landet im HTML escaped
   (`&lt;b&gt;…`), im Text-Part roh.

2. **Strict-Undefined (lautes Fehlschlagen).** Eine im Template referenzierte,
   aber nicht gelieferte Variable lässt das Rendern fehlschlagen
   (`UndefinedBehavior::Strict`) statt still leer zu rendern. Begründung: eine
   kaputte Mail mit Lücken ist schlimmer als ein abgebrochener Render, der
   geloggt/behandelt werden kann.

3. **Strukturierte Vars.** Vars gehen via `minijinja::Value::from_serialize`
   in den Render-Kontext — keine String-Konkatenation, gleiche
   Injection-Aversion wie der PDF-Pfad.

## Fehler

`EmailError` (existiert: `Backend`, `InvalidInput`) wird um zwei Render-Fälle
erweitert, analog `PdfError::{TemplateInvalid, Render}`:

```rust
#[error("template_invalid: {0}")]
TemplateInvalid(String),   // Parse-/Syntaxfehler im Template
#[error("render: {0}")]
RenderFailed(String),      // Laufzeit, inkl. strict-undefined
```

minijinja-`Error` wird in diese beiden gemappt (Syntaxfehler →
`TemplateInvalid`, Laufzeitfehler → `RenderFailed`).

## Forward-Compat (für später angedachte Varianten 2 + 3)

- **Variante 2 — Loader-Sidecar:** `EmailTemplate` ist `Deserialize`. Ein
  künftiger `email-templates/<id>.toml`-Loader (analog `scripts/`,
  `pdf-templates/`) deserialisiert direkt in den Typ. Heute nicht gebaut, der
  Typ ist bereit.
- **Variante 3 — i18n:** `EmailTemplateRenderer::new()` registriert heute keine
  i18n-Funktion. Eine spätere `with_i18n(resolver)` kann eine
  `t(key, args)`-minijinja-Funktion in dasselbe `Environment` hängen. Die
  Struktur lässt das additiv zu, ohne v1 an den Translatable-Pfad zu koppeln.

## Tests (`server/tests/email_template.rs`)

Stil wie `server/tests/email_stub.rs` / `email_smtp.rs`:

1. Variablen-Substitution in `subject`, `body_text`, `body_html`.
2. Schleife über eine Positionsliste (`{% for item in items %}…`) — beweist,
   dass mehr als Flat-Substitution funktioniert.
3. **HTML-Autoescape pro Part**: dieselbe `<script>`-Variable wird im
   HTML-Part escaped, im Text-Part roh ausgegeben.
4. **Strict-Undefined**: fehlende Variable → `Err(EmailError::RenderFailed)`.
5. **Syntaxfehler** im Template → `Err(EmailError::TemplateInvalid)`.
6. **`as_message`-Roundtrip**: `RenderedEmail` speist eine `EmailMessage`
   korrekt (Felder stimmen, `body_html` durchgereicht).
7. `body_html: None` → `RenderedEmail.body_html == None` (kein HTML gerendert).

## Datei-Struktur

**Neu:**
- `server/src/email/template.rs` — Renderer + Typen.
- `server/tests/email_template.rs` — Tests.

**Modifiziert:**
- `server/src/email/mod.rs` — `pub mod template;` + zwei `EmailError`-Varianten.
- `server/Cargo.toml` — `minijinja = "2"` unter `[dependencies]`.

**Nicht angefasst** (bewusst, hält die Arbeit Q0009-fern): `schema.rs`,
`data.rs`, `example/loader.rs`, `example/mod.rs`.

## Risiken

- **minijinja-API-Version**: `Value::from_serialize`, `set_undefined_behavior`,
  Autoescape-by-name sind in 2.x stabil — bei Pin auf konkrete Minor-Version
  im Plan verifizieren.
- **Autoescape-by-name-Mechanik**: minijinjas Default-Autoescape greift nach
  Template-Suffix. Der Plan muss die genaue Registrierungs-/Render-Mechanik
  (z.B. `add_template` mit `*.html`-Name vs. `render_str` + expliziter
  AutoEscape-Callback) im ersten Task festnageln.
