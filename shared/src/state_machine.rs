//! State-Machine-Konfiguration (Phase 1.7.5).
//!
//! Pro Entity-Typ optional in [`crate::EntitySettings::state_machine`].
//! Eine `StateMachine` definiert die erlaubten States, den Initial-State
//! und die Transitionen (jede mit optionalem Guard + Permission).
//!
//! Die State-Spalte des Entities heisst per Konvention `"state"` (kann
//! per `state_field` ueberschrieben werden, fuer Bestands-Schemata mit
//! anderem Namen). Server-seitige Logik:
//!   1. liest `entity.fields[state_field]`
//!   2. sucht passende Transition `(from == current, event)`
//!   3. wertet Guard aus (optional)
//!   4. prueft Permission (optional)
//!   5. setzt `entity.fields[state_field] = to`
//!   6. schreibt Audit-Eintrag `kind = "state_transition"`
//!
//! Wire-Format: `camelCase`, Defaults werden mit `skip_serializing_if`
//! weggelassen.

use serde::{Deserialize, Serialize};

use crate::builder::guard::GuardExpr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StateMachine {
    /// Erlaubte States. Der Resolver akzeptiert nur Werte aus dieser Liste
    /// in `entity.fields[state_field]`. Leer = keine Validierung.
    #[serde(default)]
    pub states: Vec<String>,
    /// Initial-State fuer neue Datensaetze. Wenn None, bleibt das Feld
    /// beim Create leer und kann nicht transitionieren.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<String>,
    /// Name der Entity-Feld-Spalte, die den State haelt. Default `"state"`.
    #[serde(default = "default_state_field")]
    pub state_field: String,
    /// Alle erlaubten Transitionen.
    #[serde(default)]
    pub transitions: Vec<Transition>,
}

fn default_state_field() -> String { "state".to_string() }

impl Default for StateMachine {
    fn default() -> Self {
        Self {
            states: Vec::new(),
            initial: None,
            state_field: default_state_field(),
            transitions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    /// Quell-State. `"*"` matcht jeden State (universelle Transition).
    pub from: String,
    /// Ziel-State. Muss in `StateMachine.states` enthalten sein, wenn
    /// die Liste nicht leer ist.
    pub to: String,
    /// Event-Name (Aufrufer ruft `transition(entity, "post", ...)` etc.).
    pub event: String,
    /// Optionaler Guard (Builder-DSL `fields.amount > 0` etc.). False
    /// blockiert die Transition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard: Option<GuardExpr>,
    /// Optional: required Action-Permission-Name. Wenn gesetzt, prueft
    /// der Resolver `Resource::Action{name:permission}` + `Op::Execute`.
    /// Default-Konvention: `"<entity_type>.<event>"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission: Option<String>,
}

impl StateMachine {
    /// Findet die erste Transition, die auf `(current_state, event)`
    /// passt. `"*"`-from matcht jeden State.
    pub fn find_transition(&self, current_state: &str, event: &str) -> Option<&Transition> {
        self.transitions.iter().find(|t| {
            t.event == event && (t.from == "*" || t.from == current_state)
        })
    }

    pub fn state_field_name(&self) -> &str {
        &self.state_field
    }

    /// `true` wenn `to_state` in der erlaubten State-Liste ist oder die
    /// Liste leer ist (keine Validierung).
    pub fn is_known_state(&self, state: &str) -> bool {
        self.states.is_empty() || self.states.iter().any(|s| s == state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> StateMachine {
        StateMachine {
            states:      vec!["draft".into(), "posted".into(), "cancelled".into()],
            initial:     Some("draft".into()),
            state_field: "state".into(),
            transitions: vec![
                Transition {
                    from:       "draft".into(),
                    to:         "posted".into(),
                    event:      "post".into(),
                    guard:      None,
                    permission: Some("invoice.post".into()),
                },
                Transition {
                    from:       "posted".into(),
                    to:         "cancelled".into(),
                    event:      "cancel".into(),
                    guard:      None,
                    permission: None,
                },
                Transition {
                    from:       "*".into(),
                    to:         "draft".into(),
                    event:      "reset".into(),
                    guard:      None,
                    permission: None,
                },
            ],
        }
    }

    #[test]
    fn finds_transition_by_from_event() {
        let sm = sample();
        let t = sm.find_transition("draft", "post").unwrap();
        assert_eq!(t.to, "posted");
    }

    #[test]
    fn wildcard_from_matches_any_state() {
        let sm = sample();
        let t = sm.find_transition("posted", "reset").unwrap();
        assert_eq!(t.to, "draft");
    }

    #[test]
    fn no_match_returns_none() {
        let sm = sample();
        assert!(sm.find_transition("draft", "cancel").is_none());
    }

    #[test]
    fn default_state_field_is_state() {
        let sm = StateMachine::default();
        assert_eq!(sm.state_field_name(), "state");
    }

    #[test]
    fn is_known_state_empty_list_accepts_anything() {
        let sm = StateMachine::default();
        assert!(sm.is_known_state("anything"));
    }

    #[test]
    fn is_known_state_filters_against_list() {
        let sm = sample();
        assert!(sm.is_known_state("draft"));
        assert!(!sm.is_known_state("nope"));
    }

    #[test]
    fn json_roundtrip_with_only_required_fields() {
        let json = r#"{"transitions":[{"from":"a","to":"b","event":"e"}]}"#;
        let sm: StateMachine = serde_json::from_str(json).unwrap();
        assert_eq!(sm.transitions.len(), 1);
        assert_eq!(sm.state_field, "state"); // default
        // Re-serialize: defaults werden weggelassen
        let out = serde_json::to_string(&sm).unwrap();
        assert!(out.contains(r#""transitions""#));
    }
}
