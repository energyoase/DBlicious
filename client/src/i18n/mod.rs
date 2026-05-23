//! Project-Fluent-basierte Lokalisierung.
//!
//! Die `.ftl`-Dateien werden zur Compile-Zeit eingebettet, sodass keine
//! zusaetzlichen HTTP-Requests fuer die Sprache anfallen. Der aktuelle Locale
//! wird ueber einen Leptos-Signal-Context gepflegt, damit alle aufrufenden
//! Komponenten reaktiv auf Sprachwechsel reagieren.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use fluent::concurrent::FluentBundle;
use fluent::{FluentArgs, FluentResource};
use leptos::prelude::*;
use shared::TranslatableBundle;
use unic_langid::{langid, LanguageIdentifier};

const EN_FTL: &str = include_str!("../../locales/en/main.ftl");
const DE_FTL: &str = include_str!("../../locales/de/main.ftl");
const FR_FTL: &str = include_str!("../../locales/fr/main.ftl");

/// Unterstuetzte Locales.
///
/// Eine neue Sprache hinzufuegen (z.B. `it`):
/// 1. `client/locales/it/main.ftl` mit dem gleichen Schluessel-Set wie
///    `de/`/`en/` anlegen (fehlende Schluessel fallen auf die Default-Locale
///    zurueck — `t()` liefert dann den Roh-Schluessel).
/// 2. Diesen `Locale`-Enum um eine Variante erweitern.
/// 3. `code()`, `from_code()`, `is_known_code()`, `lang_id()`, `ftl_source()`
///    und den initialen `available`-Vector in [`I18nContext::provide`]
///    aktualisieren.
/// 4. `locale-<code>`-Eintrag in jedem bestehenden `main.ftl` ergaenzen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    De,
    En,
    Fr,
}

impl Locale {
    pub fn code(self) -> &'static str {
        match self {
            Locale::De => "de",
            Locale::En => "en",
            Locale::Fr => "fr",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "de" => Locale::De,
            "fr" => Locale::Fr,
            _ => Locale::En,
        }
    }

    /// `true`, wenn `code` einer der bekannten Locales entspricht. Wird vom
    /// DB-Bundle-Loader benutzt, um unbekannte Sprachen nur einmal zu
    /// loggen statt sie still auf `En` zu mappen.
    pub fn is_known_code(code: &str) -> bool {
        let primary = code
            .split(['-', '_'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        matches!(primary.as_str(), "de" | "en" | "fr")
    }

    fn lang_id(self) -> LanguageIdentifier {
        match self {
            Locale::De => langid!("de"),
            Locale::En => langid!("en"),
            Locale::Fr => langid!("fr"),
        }
    }

    fn ftl_source(self) -> &'static str {
        match self {
            Locale::De => DE_FTL,
            Locale::En => EN_FTL,
            Locale::Fr => FR_FTL,
        }
    }
}

type Bundle = FluentBundle<FluentResource>;

fn make_bundle(locale: Locale) -> Bundle {
    let resource = FluentResource::try_new(locale.ftl_source().to_string())
        .expect("Konnte FTL-Ressource nicht parsen");
    let mut bundle = FluentBundle::new_concurrent(vec![locale.lang_id()]);
    // Unicode-Bidi-Marker abschalten – sonst landen \u{2068} im Output.
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .expect("FTL-Resource konnte nicht hinzugefuegt werden");
    bundle
}

/// Bundles inkl. moeglicher Runtime-Erweiterungen aus der DB-Translatable.
///
/// Der `Mutex` ist hier sinnvoll, weil [`install_translatable_bundle`] das
/// HashMap im laufenden Betrieb tauscht (z.B. nach erfolgreichem Fetch von
/// `shared::TranslatableBundle`).
fn bundles_cell() -> &'static Mutex<HashMap<Locale, Bundle>> {
    static BUNDLES: OnceLock<Mutex<HashMap<Locale, Bundle>>> = OnceLock::new();
    BUNDLES.get_or_init(|| {
        let mut map = HashMap::new();
        map.insert(Locale::De, make_bundle(Locale::De));
        map.insert(Locale::En, make_bundle(Locale::En));
        map.insert(Locale::Fr, make_bundle(Locale::Fr));
        Mutex::new(map)
    })
}

/// Spielt Eintraege aus einem [`TranslatableBundle`] in die bereits geladenen
/// statischen Bundles ein. Bestehende Message-IDs werden ueberschrieben —
/// das ist der gewollte Override-Pfad ("DB schlaegt Default").
///
/// Sprachen aus dem Bundle, die nicht zu einem bekannten [`Locale`] passen,
/// werden ignoriert (heute nur `de`/`en`).
pub fn install_translatable_bundle(tr: &TranslatableBundle) {
    let mut bundles = bundles_cell().lock().unwrap();
    for lang in &tr.languages {
        let Some(locale) = match_locale(&lang.code) else {
            continue;
        };
        let ftl = tr.ftl_for_language(&lang.id);
        if ftl.is_empty() {
            continue;
        }
        let resource = match FluentResource::try_new(ftl) {
            Ok(r) => r,
            Err((r, errs)) => {
                log::warn!(
                    "TranslatableBundle (lang={}) hat {} Parse-Fehler – installiere trotzdem die brauchbaren Eintraege.",
                    lang.id,
                    errs.len()
                );
                r
            }
        };
        if let Some(bundle) = bundles.get_mut(&locale) {
            // `add_resource_overriding` liefert `()` (auch in der konkurrenten
            // Variante) und ueberschreibt vorhandene Message-IDs ohne Fehler —
            // genau das, was wir hier brauchen.
            bundle.add_resource_overriding(resource);
        }
    }
}

fn match_locale(code: &str) -> Option<Locale> {
    Some(Locale::from_code(code))
}

/// Trigger fuer eine Re-Render-Welle nach DB-Bundle-Install. Muss aus einem
/// Reactive-Owner-Kontext aufgerufen werden, weil `I18nContext::use_context`
/// auf einen vorhandenen Owner zugreift.
pub fn bump_revision_if_available() {
    if let Some(ctx) = leptos::prelude::use_context::<I18nContext>() {
        ctx.bump();
    }
}

/// Aktualisiert die in der UI angebotene Sprach-Liste aus einem
/// [`TranslatableBundle`]. Sprachen, die kein bekanntes [`Locale`] sind,
/// werden uebersprungen und einmal pro Code geloggt — sonst tauchen sie
/// nicht in der Topbar auf, wuerden aber durch `Locale::from_code` als
/// `En`-Duplikat erscheinen.
pub fn set_available_locales_from_bundle(bundle: &TranslatableBundle) {
    let Some(ctx) = leptos::prelude::use_context::<I18nContext>() else {
        return;
    };
    let mut seen: Vec<Locale> = Vec::new();
    for lang in &bundle.languages {
        if !lang.active {
            continue;
        }
        if !Locale::is_known_code(&lang.code) {
            log::warn!(
                "TranslatableBundle: unbekannter Locale-Code '{}' — uebersprungen.",
                lang.code
            );
            continue;
        }
        let l = Locale::from_code(&lang.code);
        if !seen.contains(&l) {
            seen.push(l);
        }
    }
    if !seen.is_empty() {
        ctx.available.set(seen);
    }
}

/// Reaktiver i18n-Kontext.
///
/// `revision` ist ein Bump-Counter, der bei jeder Aenderung der Bundles
/// (Locale-Wechsel ist ueber `locale` bereits abgedeckt; DB-Bundle-Install
/// schickt einen Bump) erhoeht wird. Konsumenten der `t()`-Funktion
/// subskribieren auf `revision`, sodass auch nach einem Bundle-Reload neu
/// gerendert wird.
#[derive(Clone, Copy)]
pub struct I18nContext {
    pub locale: RwSignal<Locale>,
    pub revision: RwSignal<u32>,
    /// Sprachen, die zur Auswahl angeboten werden. Wird zur Laufzeit aus
    /// dem TranslatableBundle gespeist (`set_available_locales`).
    pub available: RwSignal<Vec<Locale>>,
}

impl I18nContext {
    pub fn provide(initial: Locale) -> Self {
        let ctx = Self {
            locale: RwSignal::new(initial),
            revision: RwSignal::new(0),
            available: RwSignal::new(vec![Locale::De, Locale::En, Locale::Fr]),
        };
        provide_context(ctx);
        ctx
    }

    pub fn use_context() -> Self {
        leptos::prelude::use_context::<Self>()
            .expect("I18nContext nicht vorhanden – `I18nContext::provide` aufrufen")
    }

    pub fn bump(&self) {
        self.revision.update(|r| *r += 1);
    }
}

/// Uebersetzt einen Schluessel ohne Argumente.
/// Subskribiert reaktiv auf Locale-Wechsel UND Bundle-Reloads.
pub fn t(key: &str) -> String {
    let ctx = I18nContext::use_context();
    // `revision` subskribieren, damit Cache-Bust funktioniert.
    let _ = ctx.revision.get();
    translate(ctx.locale.get(), key, None)
}

/// Uebersetzt einen Schluessel mit Argumenten.
pub fn t_with(key: &str, args: &FluentArgs) -> String {
    let ctx = I18nContext::use_context();
    let _ = ctx.revision.get();
    translate(ctx.locale.get(), key, Some(args))
}

/// Bequemes Macro: `t!("key")` oder `t!("key", "name" => "World")`.
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::t($key)
    };
    ($key:expr, $( $arg:expr => $val:expr ),+ $(,)?) => {{
        let mut __args = ::fluent::FluentArgs::new();
        $( __args.set($arg, ::fluent::FluentValue::from($val)); )+
        $crate::i18n::t_with($key, &__args)
    }};
}

fn translate(locale: Locale, key: &str, args: Option<&FluentArgs>) -> String {
    // Fluent erlaubt keine Punkte in Message-IDs, im Rust-Code wird aber bewusst
    // mit Punkten aufgerufen ("nav.dashboard"). Mapping erfolgt hier zentral.
    let normalized = key.replace('.', "-");
    let bundles = bundles_cell().lock().unwrap();
    let bundle = bundles
        .get(&locale)
        .or_else(|| bundles.get(&Locale::En))
        .expect("Bundle fuer Default-Locale fehlt");
    let Some(message) = bundle.get_message(&normalized) else {
        return key.to_string();
    };
    let Some(pattern) = message.value() else {
        return key.to_string();
    };
    let mut errors = vec![];
    bundle
        .format_pattern(pattern, args, &mut errors)
        .into_owned()
}

/// Liest die bevorzugte Sprache aus dem Browser.
pub fn detect_browser_locale() -> Locale {
    web_sys::window()
        .and_then(|w| w.navigator().language())
        .map(|c| Locale::from_code(&c))
        .unwrap_or(Locale::En)
}

/// Hilfen fuer das Formatieren von Werten in der aktuellen Sprache.
/// Verwenden die Browser-`Intl`-API, damit die Formate dem System des Nutzers entsprechen.
pub mod format {
    use super::Locale;
    use js_sys::{Array, Date, Intl, Object, Reflect};
    use wasm_bindgen::JsValue;

    fn locale_array(locale: Locale) -> Array {
        let arr = Array::new();
        arr.push(&JsValue::from_str(locale.code()));
        arr
    }

    pub fn integer(value: i64, locale: Locale) -> String {
        let formatter = Intl::NumberFormat::new(&locale_array(locale), &Object::new());
        formatter
            .format()
            .call1(&formatter, &JsValue::from_f64(value as f64))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| value.to_string())
    }

    pub fn decimal(value: f64, precision: u8, locale: Locale) -> String {
        let opts = Object::new();
        let _ = Reflect::set(
            &opts,
            &"minimumFractionDigits".into(),
            &JsValue::from_f64(precision as f64),
        );
        let _ = Reflect::set(
            &opts,
            &"maximumFractionDigits".into(),
            &JsValue::from_f64(precision as f64),
        );
        let formatter = Intl::NumberFormat::new(&locale_array(locale), &opts);
        formatter
            .format()
            .call1(&formatter, &JsValue::from_f64(value))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| format!("{value:.*}", precision as usize))
    }

    pub fn money(value: f64, currency: &str, locale: Locale) -> String {
        let opts = Object::new();
        let _ = Reflect::set(&opts, &"style".into(), &"currency".into());
        let _ = Reflect::set(&opts, &"currency".into(), &JsValue::from_str(currency));
        let formatter = Intl::NumberFormat::new(&locale_array(locale), &opts);
        formatter
            .format()
            .call1(&formatter, &JsValue::from_f64(value))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| format!("{value:.2} {currency}"))
    }

    pub fn date(iso: &str, locale: Locale) -> String {
        let date = Date::new(&JsValue::from_str(iso));
        let opts = Object::new();
        let _ = Reflect::set(&opts, &"dateStyle".into(), &"medium".into());
        let formatter = Intl::DateTimeFormat::new(&locale_array(locale), &opts);
        formatter
            .format()
            .call1(&formatter, &date)
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| iso.to_string())
    }

    pub fn datetime(iso: &str, locale: Locale) -> String {
        let date = Date::new(&JsValue::from_str(iso));
        let opts = Object::new();
        let _ = Reflect::set(&opts, &"dateStyle".into(), &"medium".into());
        let _ = Reflect::set(&opts, &"timeStyle".into(), &"short".into());
        let formatter = Intl::DateTimeFormat::new(&locale_array(locale), &opts);
        formatter
            .format()
            .call1(&formatter, &date)
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_else(|| iso.to_string())
    }
}
