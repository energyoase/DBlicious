//! Typabhaengige Operationen pro [`FieldType`].
//!
//! Diese Schicht ersetzt im Vergleich zur C#-Vorlage
//! (`JezitLibrary.EntityProvider.Implementation` mit `IComparator<T>` und
//! `IFilter<T>`) den reflektionsgesteuerten Apparat aus `ImplComparator`,
//! `ImplFilter`, `ImplSupportedType` und `ImplementationExtension`. Statt
//! Implementierungen ueber Attribute zu entdecken, eine n:n-Tabelle zu
//! befragen und per `IsAssignableFrom` zur Laufzeit aufzuloesen, gibt es
//! hier genau eine [`ops_for`]-Funktion, die jeden [`FieldType`] exhaustiv
//! auf die zustaendige Implementierung abbildet.
//!
//! Die konkreten Operationen sind absichtlich klein:
//!   - [`FieldOps::compare`]        — Sortiervergleich (Pendant zu `IComparator<T>`).
//!   - [`FieldOps::matches`]        — Praedikat-Auswertung (Pendant zu `IFilter<T>`).
//!   - [`FieldOps::matches_search`] — Freitext-Treffer fuer die globale Suche.

use std::cmp::Ordering;

use serde_json::Value;

use crate::{FieldType, FilterPredicate};

/// Vertrag fuer typgebundene Vergleichs- und Filteroperationen.
///
/// Implementierungen sind zustandsbehaftet, wenn der zugehoerige
/// [`FieldType`] Konfiguration mitbringt (z.B. die Praezision eines
/// `Decimal` oder die Werteliste eines `Enum`).
pub trait FieldOps {
    /// Sortiervergleich zweier roher JSON-Werte.
    ///
    /// `Value::Null` wird konsequent vor allen anderen Werten einsortiert,
    /// unabhaengig von der Sortierrichtung – die richtungsabhaengige
    /// Umkehrung passiert in der konsumierenden Schicht (siehe
    /// `client/src/components/table/data_source.rs::LocalSource`).
    fn compare(&self, a: &Value, b: &Value) -> Ordering;

    /// Wertet ein [`FilterPredicate`] gegen einen Wert aus.
    ///
    /// Wenn der Operator nicht zum Feldtyp passt (z.B. `NumberRange` auf
    /// einem `Text`-Feld), liefert die Default-Implementierung `false`.
    fn matches(&self, value: &Value, predicate: &FilterPredicate) -> bool;

    /// Default fuer die globale Freitextsuche: nur textartige Felder
    /// matchen ueberhaupt.
    fn matches_search(&self, _value: &Value, _needle: &str) -> bool {
        false
    }
}

// =============================================================================
// Hilfsfunktionen
// =============================================================================

fn compare_nulls(a: &Value, b: &Value) -> Option<Ordering> {
    match (a.is_null(), b.is_null()) {
        (true, true) => Some(Ordering::Equal),
        (true, false) => Some(Ordering::Less),
        (false, true) => Some(Ordering::Greater),
        (false, false) => None,
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn as_str(v: &Value) -> Option<&str> {
    v.as_str()
}

fn ci_contains(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

// =============================================================================
// Implementierungen pro FieldType
// =============================================================================

#[derive(Clone, Copy)]
pub struct TextOps {
    pub case_insensitive: bool,
}

impl FieldOps for TextOps {
    fn compare(&self, a: &Value, b: &Value) -> Ordering {
        if let Some(o) = compare_nulls(a, b) {
            return o;
        }
        match (as_str(a), as_str(b)) {
            (Some(sa), Some(sb)) => {
                if self.case_insensitive {
                    sa.to_lowercase().cmp(&sb.to_lowercase())
                } else {
                    sa.cmp(sb)
                }
            }
            _ => Ordering::Equal,
        }
    }

    fn matches(&self, value: &Value, predicate: &FilterPredicate) -> bool {
        let Some(text) = as_str(value) else {
            return matches!(predicate, FilterPredicate::IsNull);
        };
        match predicate {
            FilterPredicate::TextContains {
                value: needle,
                case_insensitive,
            } => {
                if *case_insensitive {
                    ci_contains(text, needle)
                } else {
                    text.contains(needle.as_str())
                }
            }
            FilterPredicate::TextEquals {
                value: needle,
                case_insensitive,
            } => {
                if *case_insensitive {
                    text.eq_ignore_ascii_case(needle)
                } else {
                    text == needle
                }
            }
            FilterPredicate::IsNotNull => true,
            FilterPredicate::IsNull => false,
            _ => false,
        }
    }

    fn matches_search(&self, value: &Value, needle: &str) -> bool {
        as_str(value)
            .map(|s| ci_contains(s, needle))
            .unwrap_or(false)
    }
}

#[derive(Clone, Copy)]
pub struct NumberOps;

impl FieldOps for NumberOps {
    fn compare(&self, a: &Value, b: &Value) -> Ordering {
        if let Some(o) = compare_nulls(a, b) {
            return o;
        }
        match (as_f64(a), as_f64(b)) {
            (Some(na), Some(nb)) => na.partial_cmp(&nb).unwrap_or(Ordering::Equal),
            _ => Ordering::Equal,
        }
    }

    fn matches(&self, value: &Value, predicate: &FilterPredicate) -> bool {
        let n = as_f64(value);
        match predicate {
            FilterPredicate::NumberEquals { value: v } => {
                n.is_some_and(|x| (x - *v).abs() < f64::EPSILON)
            }
            FilterPredicate::NumberRange { min, max } => match n {
                Some(x) => min.is_none_or(|lo| x >= lo) && max.is_none_or(|hi| x <= hi),
                None => false,
            },
            FilterPredicate::IsNull => n.is_none(),
            FilterPredicate::IsNotNull => n.is_some(),
            _ => false,
        }
    }

    fn matches_search(&self, value: &Value, needle: &str) -> bool {
        // Zahlen werden ueber ihre Stringdarstellung mitgesucht, damit der
        // Nutzer z.B. "9.99" eintippen und den Treffer sehen kann.
        match value {
            Value::Number(n) => n.to_string().contains(needle),
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct BooleanOps;

impl FieldOps for BooleanOps {
    fn compare(&self, a: &Value, b: &Value) -> Ordering {
        if let Some(o) = compare_nulls(a, b) {
            return o;
        }
        match (a.as_bool(), b.as_bool()) {
            (Some(x), Some(y)) => x.cmp(&y),
            _ => Ordering::Equal,
        }
    }

    fn matches(&self, value: &Value, predicate: &FilterPredicate) -> bool {
        match predicate {
            FilterPredicate::BoolEquals { value: v } => value.as_bool() == Some(*v),
            FilterPredicate::IsNull => value.is_null(),
            FilterPredicate::IsNotNull => !value.is_null(),
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
pub struct DateOps;

impl FieldOps for DateOps {
    fn compare(&self, a: &Value, b: &Value) -> Ordering {
        if let Some(o) = compare_nulls(a, b) {
            return o;
        }
        // ISO-8601 ist lexikographisch sortierbar.
        match (as_str(a), as_str(b)) {
            (Some(sa), Some(sb)) => sa.cmp(sb),
            _ => Ordering::Equal,
        }
    }

    fn matches(&self, value: &Value, predicate: &FilterPredicate) -> bool {
        let s = as_str(value);
        match predicate {
            FilterPredicate::DateRange { from, to } => match s {
                Some(d) => {
                    from.as_deref().is_none_or(|lo| d >= lo)
                        && to.as_deref().is_none_or(|hi| d <= hi)
                }
                None => false,
            },
            FilterPredicate::TextEquals { value: v, .. } => s == Some(v.as_str()),
            FilterPredicate::IsNull => s.is_none(),
            FilterPredicate::IsNotNull => s.is_some(),
            _ => false,
        }
    }

    fn matches_search(&self, value: &Value, needle: &str) -> bool {
        as_str(value).map(|s| s.contains(needle)).unwrap_or(false)
    }
}

#[derive(Clone)]
pub struct EnumOps {
    pub values: Vec<String>,
}

impl FieldOps for EnumOps {
    fn compare(&self, a: &Value, b: &Value) -> Ordering {
        if let Some(o) = compare_nulls(a, b) {
            return o;
        }
        // Sortiert nach der Position in der definierten Wertereihenfolge.
        let pos = |v: &Value| -> Option<usize> {
            v.as_str()
                .and_then(|s| self.values.iter().position(|x| x == s))
        };
        match (pos(a), pos(b)) {
            (Some(pa), Some(pb)) => pa.cmp(&pb),
            _ => {
                // Fallback: lexikographisch.
                match (as_str(a), as_str(b)) {
                    (Some(sa), Some(sb)) => sa.cmp(sb),
                    _ => Ordering::Equal,
                }
            }
        }
    }

    fn matches(&self, value: &Value, predicate: &FilterPredicate) -> bool {
        let s = as_str(value);
        match predicate {
            FilterPredicate::EnumIn { values } => match s {
                Some(v) => values.iter().any(|x| x == v),
                None => false,
            },
            FilterPredicate::TextEquals {
                value: v,
                case_insensitive,
            } => match s {
                Some(actual) => {
                    if *case_insensitive {
                        actual.eq_ignore_ascii_case(v)
                    } else {
                        actual == v
                    }
                }
                None => false,
            },
            FilterPredicate::IsNull => s.is_none(),
            FilterPredicate::IsNotNull => s.is_some(),
            _ => false,
        }
    }

    fn matches_search(&self, value: &Value, needle: &str) -> bool {
        as_str(value)
            .map(|s| ci_contains(s, needle))
            .unwrap_or(false)
    }
}

/// Fallback fuer komplexe Typen (`Reference`, `Collection`), die heute weder
/// vergleichbar noch filterbar sind. Der Trait wird nur fuer Vollstaendigkeit
/// implementiert, damit [`ops_for`] erschoepfend bleibt.
#[derive(Clone, Copy)]
pub struct OpaqueOps;

impl FieldOps for OpaqueOps {
    fn compare(&self, _a: &Value, _b: &Value) -> Ordering {
        Ordering::Equal
    }
    fn matches(&self, value: &Value, predicate: &FilterPredicate) -> bool {
        match predicate {
            FilterPredicate::IsNull => value.is_null(),
            FilterPredicate::IsNotNull => !value.is_null(),
            _ => false,
        }
    }
}

// =============================================================================
// Dispatch
// =============================================================================

/// Liefert die zustaendige Operationen-Implementierung fuer einen
/// [`FieldType`]. Das Pattern-Match ist erschoepfend — neue Varianten in
/// [`FieldType`] erzeugen hier sofort einen Compile-Fehler, der den
/// Implementierenden zur Pflege zwingt.
pub fn ops_for(field: &FieldType) -> Box<dyn FieldOps> {
    match field {
        FieldType::Text => Box::new(TextOps {
            case_insensitive: true,
        }),
        FieldType::Integer => Box::new(NumberOps),
        FieldType::Decimal { .. } => Box::new(NumberOps),
        FieldType::Boolean => Box::new(BooleanOps),
        FieldType::Date => Box::new(DateOps),
        FieldType::DateTime => Box::new(DateOps),
        FieldType::Money { .. } => Box::new(NumberOps),
        FieldType::Enum { values } => Box::new(EnumOps {
            values: values.clone(),
        }),
        // Auf der Leitung ein String (wire_name) — Ops verhalten sich wie ein
        // Enum dieser Namen. Sortierung nach Integer-Reihenfolge ist eine
        // bewusste v1-Luecke (siehe G7-Plan).
        FieldType::IntEnum { values } => Box::new(EnumOps {
            values: values.iter().map(|v| v.wire_name.clone()).collect(),
        }),
        // DirectionalEnum: wie IntEnum, wire_name-basiert. Das sign-Feld
        // wird bei Aggregationen (Welle 2) benutzt; hier nur Enum-Ops.
        FieldType::DirectionalEnum { values, .. } => Box::new(EnumOps {
            values: values.iter().map(|v| v.wire_name.clone()).collect(),
        }),
        FieldType::Reference { .. } => Box::new(OpaqueOps),
        FieldType::Collection { .. } => Box::new(OpaqueOps),
    }
}

// =============================================================================
// Implementation-Selektor
// =============================================================================
//
// Pendant zum reflektionsgesteuerten `ImplementationExtension` aus dem
// C#-Original. Eine Spalte kann per [`crate::ColumnMeta::comparator_id`] /
// [`crate::ColumnMeta::filter_id`] eine alternative Implementierung
// anfordern. Die Aufloesung passiert hier — exhaustiv, kein Reflection.

/// Bezeichner fuer eingebaute Comparator-Varianten.
pub mod comparators {
    /// Default-Comparator gemaess [`super::ops_for`].
    pub const DEFAULT: &str = "default";
    /// Text-Sortierung case-sensitive (Default ist case-insensitive).
    pub const TEXT_CASE_SENSITIVE: &str = "text.case_sensitive";
}

/// Bezeichner fuer eingebaute Filter-Varianten.
pub mod filters {
    /// Default-Filter gemaess [`super::ops_for`].
    pub const DEFAULT: &str = "default";
    /// Text-Filter, der `TextContains`-Praedikate als "starts with"
    /// auswertet (statt Substring). Wird auf [`crate::FieldType::Text`]
    /// angewendet. Beispiel fuer einen alternativen Filter-Pfad — eine
    /// echte Regex-Variante wuerde eine `regex`-Dependency erfordern.
    pub const TEXT_STARTS_WITH: &str = "text.starts_with";
}

/// Filter-Variante: TextContains-Praedikat wirkt als Praefix-Match.
#[derive(Clone, Copy)]
pub struct TextStartsWithOps {
    pub case_insensitive: bool,
}

impl FieldOps for TextStartsWithOps {
    fn compare(&self, a: &Value, b: &Value) -> Ordering {
        TextOps {
            case_insensitive: self.case_insensitive,
        }
        .compare(a, b)
    }

    fn matches(&self, value: &Value, predicate: &FilterPredicate) -> bool {
        let Some(text) = as_str(value) else {
            return matches!(predicate, FilterPredicate::IsNull);
        };
        match predicate {
            FilterPredicate::TextContains {
                value: needle,
                case_insensitive,
            } => {
                if *case_insensitive {
                    text.to_lowercase().starts_with(&needle.to_lowercase())
                } else {
                    text.starts_with(needle.as_str())
                }
            }
            other => TextOps {
                case_insensitive: self.case_insensitive,
            }
            .matches(value, other),
        }
    }

    fn matches_search(&self, value: &Value, needle: &str) -> bool {
        TextOps {
            case_insensitive: self.case_insensitive,
        }
        .matches_search(value, needle)
    }
}

/// Wie [`ops_for`], jedoch mit optionalem Implementations-Override
/// fuer Comparator UND Filter. Beide unbekannten Ids fallen still auf den
/// Default zurueck: das vermeidet UI-Crashes bei veralteten Settings.
///
/// **Praezedenz**: `filter_id` schlaegt `comparator_id`, falls beide
/// gesetzt sind und nicht zueinander passen — Filter hat den hoeheren
/// Einfluss auf die Match-Logik, Comparator nur auf Sortierung.
pub fn ops_for_named(
    field: &FieldType,
    comparator_id: Option<&str>,
    filter_id: Option<&str>,
) -> Box<dyn FieldOps> {
    match (field, comparator_id, filter_id) {
        (FieldType::Text, _, Some(filters::TEXT_STARTS_WITH)) => Box::new(TextStartsWithOps {
            case_insensitive: true,
        }),
        (FieldType::Text, Some(comparators::TEXT_CASE_SENSITIVE), _) => Box::new(TextOps {
            case_insensitive: false,
        }),
        _ => ops_for(field),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn text_compare_is_case_insensitive_by_default() {
        let ops = ops_for(&FieldType::Text);
        assert_eq!(
            ops.compare(&json!("Apfel"), &json!("birne")),
            Ordering::Less
        );
    }

    #[test]
    fn null_sorts_before_value() {
        let ops = ops_for(&FieldType::Integer);
        assert_eq!(ops.compare(&Value::Null, &json!(1)), Ordering::Less);
    }

    #[test]
    fn number_range_matches() {
        let ops = ops_for(&FieldType::Decimal { precision: 2 });
        let pred = FilterPredicate::NumberRange {
            min: Some(10.0),
            max: Some(20.0),
        };
        assert!(ops.matches(&json!(15.5), &pred));
        assert!(!ops.matches(&json!(9.99), &pred));
        assert!(!ops.matches(&Value::Null, &pred));
    }

    #[test]
    fn enum_sorts_by_declared_order() {
        let ops = ops_for(&FieldType::Enum {
            values: vec!["new".into(), "paid".into(), "shipped".into()],
        });
        assert_eq!(
            ops.compare(&json!("shipped"), &json!("new")),
            Ordering::Greater
        );
    }

    #[test]
    fn search_matches_substring_case_insensitively() {
        let ops = ops_for(&FieldType::Text);
        assert!(ops.matches_search(&json!("Produkt Nr. 12"), "PRODUKT"));
    }

    #[test]
    fn opaque_ops_only_react_to_null_predicates() {
        let ops = ops_for(&FieldType::Reference {
            entity: "category".into(),
        });
        assert!(ops.matches(&Value::Null, &FilterPredicate::IsNull));
        assert!(!ops.matches(&json!({"id":"c-1"}), &FilterPredicate::IsNull));
    }

    #[test]
    fn named_comparator_text_case_sensitive_distinguishes_case() {
        let ops = ops_for_named(
            &FieldType::Text,
            Some(comparators::TEXT_CASE_SENSITIVE),
            None,
        );
        assert_eq!(
            ops.compare(&json!("Apfel"), &json!("apfel")),
            Ordering::Less
        );
    }

    #[test]
    fn named_comparator_unknown_id_falls_back_to_default() {
        let ops = ops_for_named(&FieldType::Text, Some("does.not.exist"), None);
        assert_eq!(
            ops.compare(&json!("Apfel"), &json!("birne")),
            Ordering::Less
        );
    }

    #[test]
    fn named_filter_text_starts_with_matches_prefix_only() {
        let ops = ops_for_named(&FieldType::Text, None, Some(filters::TEXT_STARTS_WITH));
        let pred = FilterPredicate::TextContains {
            value: "prod".into(),
            case_insensitive: true,
        };
        assert!(ops.matches(&json!("Produkt"), &pred));
        assert!(!ops.matches(&json!("Mein Produkt"), &pred));
    }
}
