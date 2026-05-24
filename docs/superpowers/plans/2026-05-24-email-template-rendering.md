# Email-Template-Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eine reine Email-Template-Render-Layer in `server/src/email/template.rs`, die `(EmailTemplate, EmailVars) -> RenderedEmail` rendert — quellen-agnostisch, mit Per-Part-HTML-Autoescape und lautem Fehlschlagen bei fehlenden Variablen.

**Architecture:** Spiegelt das `server/src/pdf/`-Muster (Renderer ist quellen-agnostisch, Variablen kommen strukturiert als JSON, keine String-Konkatenation). Ein konkreter `EmailTemplateRenderer`-Struct hält eine einmal konfigurierte `minijinja::Environment` (Strict-Undefined + Autoescape-Callback nach Template-Namens-Suffix). Pro Render werden die drei Template-Parts (subject/body_text/body_html) via `render_named_str` mit `.txt`- bzw. `.html`-Namen gerendert, sodass nur der HTML-Part escaped wird. Alles bleibt in `server/src/email/` — keine Berührung von `schema.rs`, `data.rs`, `loader.rs`.

**Tech Stack:** Rust, `minijinja = "2"` (pure Rust, sandboxed, Jinja2-Syntax), `serde`/`serde_json`, `thiserror`, `cargo test`.

**Spec:** [`docs/superpowers/specs/2026-05-24-email-template-rendering-design.md`](../specs/2026-05-24-email-template-rendering-design.md)

**Verifizierte minijinja-2.x-API (context7, 2026-05-24):**
- `Environment::render_named_str(&self, name: &str, source: &str, ctx: S) -> Result<String, Error>` — parst + rendert einen String-Template-Part in einem Schritt; der `name` triggert den Autoescape-Callback. (`render_str` ohne Namen nutzt intern `<string>` → kein Autoescape, daher ungeeignet.)
- `Environment::set_undefined_behavior(UndefinedBehavior::Strict)` — **Default ist `Lenient`**, Strict muss explizit gesetzt werden.
- `Environment::set_auto_escape_callback(|name| AutoEscape::Html | AutoEscape::None)` — Signatur verifiziert.
- `minijinja::Error::kind() -> ErrorKind`; Syntaxfehler = `ErrorKind::SyntaxError`. Der `Display` mit `{e:#}` liefert Detail inkl. Position.
- Kontext: `serde_json::Map<String, Value>` ist `Serialize` und wird als Variablen-Namespace behandelt (`{{ name }}` → `vars["name"]`).

**Hinweis zu parallelen Sessions:** Eine andere Session arbeitet zeitgleich an Q0009 (Working-Tree-Änderungen in `server/src/script/*`, `server/src/schema.rs`, `server/src/data.rs`, `client/src/script/*`). Dieser Plan fasst **keine** dieser Dateien an. Für Cargo-Befehle `--target-dir target-test` verwenden (gitignored, von der Q0009-Session — die `target-q0009` nutzt — frei), um Windows-`server.exe`-Lock- und Build-Lock-Konkurrenz zu vermeiden. Beim Stagen **immer gezielt** die genannten Pfade adden, nie `git add -A`.

---

## File Structure

**Created:**
- `server/src/email/template.rs` — `EmailVars`, `EmailTemplate`, `RenderedEmail`, `EmailTemplateRenderer` + `RenderedEmail::as_message`. Eine klar abgegrenzte Verantwortung: Template-String + Variablen → gerenderte Strings.
- `server/tests/email_template.rs` — Integrationstests (pure, synchron, kein DB/Audit, kein `#[serial]`).

**Modified:**
- `server/Cargo.toml` — `minijinja = "2"` unter `[dependencies]`.
- `server/src/email/mod.rs` — `pub mod template;` + zwei `EmailError`-Varianten (`TemplateInvalid`, `RenderFailed`).

**Deliberately untouched** (hält die Arbeit Q0009-fern): `server/src/schema.rs`, `server/src/data.rs`, `server/src/example/loader.rs`, `server/src/example/mod.rs`, alles unter `server/src/script/` und `client/`.

---

## Task 1: Dependency + Modul-Verdrahtung + EmailError-Varianten

Ziel: `minijinja` ist verfügbar, das (noch leere) `template`-Modul ist eingebunden, und `EmailError` kennt die zwei Render-Fehlerklassen. Kompiliert, noch keine Tests.

**Files:**
- Modify: `server/Cargo.toml`
- Modify: `server/src/email/mod.rs`
- Create: `server/src/email/template.rs`

- [ ] **Step 1: `minijinja` zu `server/Cargo.toml` hinzufügen**

In `server/Cargo.toml` unter `[dependencies]` (alphabetisch in der Nähe der anderen Krates) einfügen:

```toml
minijinja = "2"
```

- [ ] **Step 2: Leere Modul-Datei anlegen**

`server/src/email/template.rs` mit nur dem Modul-Doc-Kommentar:

```rust
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
```

- [ ] **Step 3: Modul + Fehler-Varianten in `server/src/email/mod.rs`**

Bei den Modul-Deklarationen (`pub mod smtp;` / `pub mod stub;`) ergänzen:

```rust
pub mod template;
```

In der `EmailError`-Enum die zwei Varianten hinzufügen (nach `InvalidInput`):

```rust
    #[error("template_invalid: {0}")]
    TemplateInvalid(String),
    #[error("render: {0}")]
    RenderFailed(String),
```

- [ ] **Step 4: Build prüfen**

Run: `cargo build -p server --target-dir target-test`
Expected: PASS. (Falls ein bestehendes `match` auf `EmailError` in `smtp.rs`/`stub.rs` nicht-exhaustiv wird, schlägt der Build hier fehl — dann das `match` um die zwei neuen Varianten ergänzen bzw. einen `_`-Arm prüfen. Erwartung: kein solches Match existiert, Build ist grün.)

- [ ] **Step 5: Commit**

```bash
git add server/Cargo.toml server/src/email/mod.rs server/src/email/template.rs
git commit -m "feat(email): minijinja dep + template module skeleton + error variants (1.7.10)"
```

---

## Task 2: Kerntypen + Renderer mit Variablen-Substitution (TDD)

Ziel: `EmailTemplate`, `EmailVars`, `RenderedEmail`, `EmailTemplateRenderer` existieren; einfache `{{ var }}`-Substitution in allen drei Parts funktioniert. Dies implementiert den vollständigen Renderer (Strict-Undefined + Autoescape-Callback + Fehler-Mapping); die Folge-Tasks pinnen die einzelnen Garantien.

**Files:**
- Modify: `server/src/email/template.rs`
- Create: `server/tests/email_template.rs`

- [ ] **Step 1: Failing Test schreiben**

`server/tests/email_template.rs`:

```rust
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
```

- [ ] **Step 2: Test laufen lassen — muss fehlschlagen**

Run: `cargo test -p server --test email_template --target-dir target-test`
Expected: FAIL (Compile-Error: `EmailTemplate`/`EmailTemplateRenderer`/`EmailVars` existieren noch nicht).

- [ ] **Step 3: Renderer + Typen implementieren**

`server/src/email/template.rs` (nach dem Doc-Kommentar) vollständig:

```rust
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
```

- [ ] **Step 4: Test laufen lassen — muss bestehen**

Run: `cargo test -p server --test email_template --target-dir target-test`
Expected: PASS (`renders_simple_substitution_in_all_parts`).

- [ ] **Step 5: Commit**

```bash
git add server/src/email/template.rs server/tests/email_template.rs
git commit -m "feat(email): EmailTemplateRenderer + variable substitution (1.7.10)"
```

---

## Task 3: Per-Part-HTML-Autoescape pinnen

Ziel: Beweisen, dass dieselbe Variable im HTML-Part escaped, im Text-Part roh erscheint. Behavior ist durch den Autoescape-Callback aus Task 2 bereits gegeben — dieser Test pinnt die Injection-Grenze.

**Files:**
- Modify: `server/tests/email_template.rs`

- [ ] **Step 1: Test ergänzen**

In `server/tests/email_template.rs`:

```rust
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
    // HTML-Part: escaped. minijinja escaped < > & " ' (Slash NICHT).
    assert_eq!(out.body_html.as_deref(), Some("&lt;b&gt;hi&lt;/b&gt;"));
}
```

Hinweis: Sollte minijinja eine andere Entity-Kodierung verwenden (z.B. `&#x27;` für Anführungszeichen), nur den erwarteten String an die tatsächliche Ausgabe anpassen — die getestete Invariante ist „escaped im HTML, roh im Text", nicht die exakte Entity-Wahl. Der gewählte Wert (`<b>hi</b>`) enthält keine Anführungszeichen, daher ist die Erwartung eindeutig.

- [ ] **Step 2: Test laufen lassen**

Run: `cargo test -p server --test email_template --target-dir target-test -- html_part_escapes_but_text_part_is_raw`
Expected: PASS (pinnt die Autoescape-Garantie aus Task 2).

- [ ] **Step 3: Commit**

```bash
git add server/tests/email_template.rs
git commit -m "test(email): pin per-part HTML autoescape boundary (1.7.10)"
```

---

## Task 4: Strict-Undefined pinnen (fehlende Variable → Fehler)

Ziel: Eine im Template referenzierte, aber nicht gelieferte Variable lässt das Rendern fehlschlagen statt still leer zu rendern.

**Files:**
- Modify: `server/tests/email_template.rs`

- [ ] **Step 1: Test ergänzen**

```rust
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
```

- [ ] **Step 2: Test laufen lassen**

Run: `cargo test -p server --test email_template --target-dir target-test -- missing_variable_fails_loudly`
Expected: PASS (pinnt `UndefinedBehavior::Strict` aus Task 2).

- [ ] **Step 3: Commit**

```bash
git add server/tests/email_template.rs
git commit -m "test(email): pin strict-undefined render failure (1.7.10)"
```

---

## Task 5: Syntaxfehler → TemplateInvalid pinnen

Ziel: Ein Template mit Syntaxfehler liefert `EmailError::TemplateInvalid`, nicht `RenderFailed`.

**Files:**
- Modify: `server/tests/email_template.rs`

- [ ] **Step 1: Test ergänzen**

```rust
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
```

- [ ] **Step 2: Test laufen lassen**

Run: `cargo test -p server --test email_template --target-dir target-test -- syntax_error_maps_to_template_invalid`
Expected: PASS (pinnt das Fehler-Mapping `ErrorKind::SyntaxError → TemplateInvalid` aus Task 2).

- [ ] **Step 3: Commit**

```bash
git add server/tests/email_template.rs
git commit -m "test(email): pin syntax-error to TemplateInvalid mapping (1.7.10)"
```

---

## Task 6: Schleife über Positionsliste

Ziel: Beweisen, dass mehr als flache Substitution funktioniert — eine `{% for %}`-Schleife über eine Liste von Objekten (z.B. Rechnungspositionen). Das ist der Grund für eine echte Engine statt `{{var}}`-Substitution.

**Files:**
- Modify: `server/tests/email_template.rs`

- [ ] **Step 1: Test ergänzen**

```rust
#[test]
fn renders_loop_over_line_items() {
    let r = EmailTemplateRenderer::new();
    let tpl = EmailTemplate {
        subject: "Bestellung".into(),
        body_text: "{% for it in items %}- {{ it.name }} x{{ it.qty }}\n{% endfor %}".into(),
        body_html: None,
    };
    let out = r
        .render(
            &tpl,
            &vars(json!({
                "items": [
                    { "name": "Widget", "qty": 2 },
                    { "name": "Gadget", "qty": 5 }
                ]
            })),
        )
        .expect("render");
    assert!(out.body_text.contains("- Widget x2"), "war: {}", out.body_text);
    assert!(out.body_text.contains("- Gadget x5"), "war: {}", out.body_text);
}
```

- [ ] **Step 2: Test laufen lassen**

Run: `cargo test -p server --test email_template --target-dir target-test -- renders_loop_over_line_items`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add server/tests/email_template.rs
git commit -m "test(email): pin loop over line items (1.7.10)"
```

---

## Task 7: `as_message`-Brücke + `body_html: None`

Ziel: `RenderedEmail::as_message` speist eine `EmailMessage` korrekt; ein Template ohne HTML-Part ergibt `body_html: None`.

**Files:**
- Modify: `server/tests/email_template.rs`

- [ ] **Step 1: Tests ergänzen**

```rust
#[test]
fn as_message_feeds_email_message_fields() {
    let r = EmailTemplateRenderer::new();
    let tpl = EmailTemplate {
        subject: "Rechnung {{ n }}".into(),
        body_text: "Text {{ n }}".into(),
        body_html: Some("<p>{{ n }}</p>".into()),
    };
    let out = r.render(&tpl, &vars(json!({ "n": "7" }))).expect("render");

    let to = ["alice@example.com"];
    let msg = out.as_message("noreply@example.com", &to, &[], &[], &[]);

    assert_eq!(msg.from, "noreply@example.com");
    assert_eq!(msg.to, &["alice@example.com"]);
    assert_eq!(msg.subject, "Rechnung 7");
    assert_eq!(msg.body_text, "Text 7");
    assert_eq!(msg.body_html, Some("<p>7</p>"));
    assert!(msg.attachments.is_empty());
}

#[test]
fn template_without_html_renders_none() {
    let r = EmailTemplateRenderer::new();
    let tpl = EmailTemplate {
        subject: "s".into(),
        body_text: "nur text".into(),
        body_html: None,
    };
    let out = r.render(&tpl, &vars(json!({}))).expect("render");
    assert_eq!(out.body_html, None);
}
```

- [ ] **Step 2: Tests laufen lassen**

Run: `cargo test -p server --test email_template --target-dir target-test -- as_message_feeds_email_message_fields template_without_html_renders_none`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add server/tests/email_template.rs
git commit -m "test(email): pin as_message bridge + html-none case (1.7.10)"
```

---

## Task 8: Verifikations-Gate

Ziel: Das ganze Email-Test-Set ist grün, `server` baut, und der neue Code ist clippy-sauber (auf den `server`-Code begrenzt — die Q0010-Lint-Baseline in `shared/` ist separat rot und nicht Teil dieser Arbeit).

**Files:** keine Änderungen — nur Verifikation.

- [ ] **Step 1: Gesamtes Email-Test-Set**

Run: `cargo test -p server --test email_template --target-dir target-test`
Expected: PASS — alle 7 Tests (`renders_simple_substitution_in_all_parts`, `html_part_escapes_but_text_part_is_raw`, `missing_variable_fails_loudly`, `syntax_error_maps_to_template_invalid`, `renders_loop_over_line_items`, `as_message_feeds_email_message_fields`, `template_without_html_renders_none`).

- [ ] **Step 2: Bestehende Email-Tests dürfen nicht gebrochen sein**

Run: `cargo test -p server --test email_stub --test email_smtp --target-dir target-test`
Expected: PASS (die zwei neuen `EmailError`-Varianten sind additiv).

- [ ] **Step 3: Server baut + clippy auf server-Code**

Run: `cargo clippy -p server --target-dir target-test`
Expected: keine neuen Warnungen aus `server/src/email/template.rs`. (Workspace-weites `-D warnings` NICHT gaten — `shared/`-Baseline ist via Q0010 separat rot.)

- [ ] **Step 4: Exit-Code direkt prüfen (nie `| tail`)**

Bei flaky Fails durch Parallel-Sessions (cargo-Target-Locks) den betroffenen Test einzeln isoliert re-runnen und den Exit-Code direkt erfassen.

- [ ] **Step 5: Kein Commit nötig** (reine Verifikation). Falls clippy kleine Lints im neuen File meldet (z.B. fehlendes `#[derive(Default)]`-Hinweis), inline beheben und committen:

```bash
git add server/src/email/template.rs
git commit -m "style(email): clippy fixes for template renderer (1.7.10)"
```

---

## Self-Review (gegen Spec)

- **Render-Layer `(EmailTemplate, EmailVars) -> RenderedEmail`** → Task 2 ✓
- **minijinja, server-only, kein Trait** → Task 1 (dep) + Task 2 (konkreter Struct) ✓
- **Per-Part-Autoescape (HTML escaped, Subject/Text roh)** → Task 2 (Callback) + Task 3 (Pin) ✓
- **Strict-Undefined** → Task 2 (`set_undefined_behavior`) + Task 4 (Pin) ✓
- **Strukturierte Vars, keine String-Konkatenation** → Task 2 (`render_named_str` mit serde-Map) ✓
- **Fehler: `TemplateInvalid` + `RenderFailed`** → Task 1 (Varianten) + Task 2 (Mapping) + Task 5 (Pin) ✓
- **`RenderedEmail::as_message`-Brücke zu `EmailMessage<'a>`** → Task 2 (Impl) + Task 7 (Pin) ✓
- **Schleifen/Bedingungen (Rechnungspositionen)** → Task 6 ✓
- **Forward-Compat: `EmailTemplate: Deserialize` (Loader-Sidecar)** → Task 2 (`derive(Deserialize)`) ✓
- **Forward-Compat: i18n via späterer `with_i18n`** → kein Code in v1; die `Environment`-im-Struct-Form lässt eine spätere Funktions-Registrierung additiv zu (Spec-Notiz, kein Task) ✓
- **Tests in `server/tests/email_template.rs`, alle Spec-§6-Fälle** → Tasks 2–7 ✓
- **Nicht angefasst: `schema.rs`/`data.rs`/`loader.rs`** → keine Task berührt sie ✓

Keine Platzhalter; Typnamen (`EmailTemplate`, `EmailVars`, `RenderedEmail`, `EmailTemplateRenderer`, `render`, `as_message`, `EmailError::{TemplateInvalid, RenderFailed}`) sind über alle Tasks konsistent.

## Was NICHT in diesem Plan ist

- Template-Quelle (Loader-Sidecar `email-templates/<id>.toml`, `email_templates`-Tabelle, Designer).
- Locale-Auswahl / i18n-`t()`-Funktion im Template.
- Versand-Aufrufer (GraphQL-Mutation / Service, der rendert + sendet — würde `schema.rs` berühren).
- Bounce-Handling, DKIM/SPF/DMARC.
