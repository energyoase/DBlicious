//! Translatable-Entitaeten – Fluent-konform.
//!
//! Anders als die C#-Vorlage (`JezitLibraryShared/EntityProvider/Translatable/`)
//! speichert dieses Modell *nicht* einen flachen String pro Sprache, sondern
//! den **FTL-Quelltext** einer einzelnen Fluent-Nachricht. Dadurch sind
//! Selektoren, Attribute, Plural-Regeln und Variablen-Substitutionen direkt
//! abgebildet – also exakt das, was im Client unter
//! `client/locales/<locale>/main.ftl` schon zur Compile-Zeit eingebettet wird.
//!
//! Konsequenz: ein Bundle pro Sprache wird zur Laufzeit gebildet, indem alle
//! [`TranslatableValue::ftl_source`] derselben Sprache mit Newlines verkettet
//! und an `FluentBundle::add_resource` uebergeben werden. Statisch
//! eingebettete `.ftl`-Dateien und dynamisch geladene DB-Eintraege
//! koennen so im selben Bundle koexistieren.
//!
//! Konventionen, die einzuhalten sind:
//!   - `TranslatableEntry.id` ist eine **gueltige Fluent-Message-ID**
//!     (z.B. `nav-dashboard`, keine Punkte – der Client mappt die im Code
//!     ueblichen `nav.dashboard`-Schluessel via [`message_id_for_key`]).
//!   - `TranslatableValue.ftl_source` ist die *Werte-Seite* der Nachricht,
//!     gegebenenfalls inklusive Attributen, ohne den `id =` -Prefix.
//!     Der Konstruktor [`TranslatableValue::compose_resource`] fuegt den
//!     Prefix beim Serialisieren ins Bundle hinzu.

use serde::{Deserialize, Serialize};

/// Eine in der Datenbank verwaltete Sprache.
///
/// `code` ist ein BCP-47-Sprach-Tag (`de`, `en-US`, …). Falls die UI auf
/// Granularitaet `de` arbeitet, traegt der Eintrag entsprechend `code = "de"`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslatableLanguage {
    pub id: String,
    pub code: String,
    /// Anzeigename als FTL-Schluessel (self-referentially uebersetzbar:
    /// `locale-de`, `locale-en` …).
    pub name_key: String,
    /// Fallback-Sprache, falls eine Nachricht in dieser Sprache fehlt.
    /// Bildet eine Kette (deutsch → englisch → englisch ⇒ Stop).
    #[serde(default)]
    pub fallback_id: Option<String>,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_active() -> bool {
    true
}

/// Ein logischer Uebersetzungs-Eintrag (Message). Pro Eintrag gibt es n
/// [`TranslatableValue`]s — einen pro Sprache.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslatableEntry {
    /// Fluent-Message-ID (siehe Modul-Doku).
    pub id: String,
    /// Optionale Kategorie fuer Editor-Gruppierung (z.B. "navigation", "table").
    #[serde(default)]
    pub category: Option<String>,
    /// Optionaler Kommentar/Hinweis fuer Uebersetzer.
    #[serde(default)]
    pub description: Option<String>,
}

/// Konkrete Uebersetzung eines [`TranslatableEntry`] in einer Sprache.
///
/// `ftl_source` ist die Werte-Seite einer Fluent-Nachricht. Beispiele:
///
/// ```text
/// // einfach:
/// "Übersicht"
///
/// // mit Argument:
/// "Fehler: { $message }"
///
/// // mit Selektor (Plural):
/// "{ $count ->
///    [one] 1 Eintrag
///   *[other] { $count } Einträge
/// }"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranslatableValue {
    pub entry_id: String,
    pub language_id: String,
    /// Fluent-Quelle ohne den `id = `-Prefix.
    pub ftl_source: String,
    /// ISO-8601-Zeitstempel, rein informativ.
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl TranslatableValue {
    /// Erzeugt einen vollstaendigen FTL-Quelltext (`id = value …`) fuer den
    /// uebergebenen Eintrag. Der Aufrufer kann mehrere solche Strings mit
    /// `\n` verbinden und an `FluentBundle::add_resource` uebergeben.
    pub fn compose_resource(&self, entry: &TranslatableEntry) -> String {
        debug_assert_eq!(self.entry_id, entry.id);
        // Fluent erwartet bei mehrzeiligen Werten Einrueckung – hier setzen
        // wir auf "value steht in einer Zeile oder bringt seine Einrueckung
        // selbst mit". Aufrufer mit komplexen Selektoren liefern den `value`
        // bereits korrekt eingerueckt.
        //
        // Punkt-Schluessel (`nav.datev`, `field.foo.bar`) muessen in
        // Fluent-konforme IDs (`nav-datev`, `field-foo-bar`) gewandelt
        // werden — sonst weist `FluentResource::try_new` die Zeile als
        // invalide Message-ID zurueck und die Uebersetzung erscheint nie
        // im Bundle. Lookup-seitig macht `i18n::translate` exakt diese
        // Konvertierung; hier ist das Spiegelbild.
        format!("{} = {}", message_id_for_key(&entry.id), self.ftl_source)
    }
}

/// Konvertiert einen "Punkt-Schluessel" wie `nav.dashboard` in die
/// Fluent-konforme Message-ID `nav-dashboard`.
///
/// Spiegelbild zur Mapping-Logik im Client (`i18n::translate`). Hier
/// extrahiert, damit Server- und Client-Code beide darauf bauen koennen.
pub fn message_id_for_key(key: &str) -> String {
    key.replace('.', "-")
}

/// Bequemer Container fuer den Initial-Load: Sprachen, Eintraege und alle
/// Werte in einem Aufruf.
///
/// In dieser Form kommt das Bundle ueber GraphQL und wird im Client zu
/// `n` Fluent-Resources zerlegt — einer pro [`TranslatableLanguage`].
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranslatableBundle {
    pub languages: Vec<TranslatableLanguage>,
    pub entries: Vec<TranslatableEntry>,
    pub values: Vec<TranslatableValue>,
}

impl TranslatableBundle {
    /// Liefert den vollstaendigen FTL-Quelltext fuer eine Sprache: alle
    /// Werte, die zu `language_id` gehoeren, mit Newlines verkettet.
    ///
    /// Werte zu Eintraegen, die nicht in `self.entries` stehen, werden
    /// uebersprungen.
    pub fn ftl_for_language(&self, language_id: &str) -> String {
        let mut lines: Vec<String> = Vec::new();
        for value in self.values.iter().filter(|v| v.language_id == language_id) {
            if let Some(entry) = self.entries.iter().find(|e| e.id == value.entry_id) {
                lines.push(value.compose_resource(entry));
            }
        }
        lines.join("\n")
    }

    pub fn language_by_code(&self, code: &str) -> Option<&TranslatableLanguage> {
        self.languages.iter().find(|l| l.code == code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bundle() -> TranslatableBundle {
        TranslatableBundle {
            languages: vec![TranslatableLanguage {
                id: "de".into(),
                code: "de".into(),
                name_key: "locale-de".into(),
                fallback_id: Some("en".into()),
                active: true,
            }],
            entries: vec![TranslatableEntry {
                id: "nav-dashboard".into(),
                category: Some("navigation".into()),
                description: None,
            }],
            values: vec![TranslatableValue {
                entry_id: "nav-dashboard".into(),
                language_id: "de".into(),
                ftl_source: "Übersicht".into(),
                updated_at: None,
            }],
        }
    }

    #[test]
    fn message_id_replaces_dots_with_dashes() {
        assert_eq!(message_id_for_key("nav.dashboard"), "nav-dashboard");
    }

    #[test]
    fn ftl_for_language_emits_id_equals_value() {
        let b = sample_bundle();
        assert_eq!(b.ftl_for_language("de"), "nav-dashboard = Übersicht");
    }

    #[test]
    fn unknown_language_yields_empty_resource() {
        let b = sample_bundle();
        assert!(b.ftl_for_language("fr").is_empty());
    }
}
