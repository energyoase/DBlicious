//! Undo/Redo fuer den Visual-Builder (Phase 1.7).
//!
//! Klassischer Two-Stack-Ansatz:
//!   - `past`  haelt vergangene Zustaende (Top = juengster Snapshot vor der
//!     aktuellen Mutation),
//!   - `future` haelt zurueckgenommene Zustaende (gefuellt durch Undo,
//!     geleert durch jede neue Mutation).
//!
//! Mutations-Workflow:
//!   1. Vor jeder Aenderung am Tree: [`BuilderHistory::record`] mit dem
//!      aktuellen Stand aufrufen → `future` wird geleert.
//!   2. Tree mutieren.
//!   3. Undo: [`BuilderHistory::undo`] gibt den Vorzustand zurueck und
//!      schiebt den aktuellen Stand in `future`.
//!   4. Redo: [`BuilderHistory::redo`] kehrt 3. um.
//!
//! Convenience-Wrapper [`mutate_with_history`] kapselt 1.+2. fuer
//! [`crate::builder::UiTreeSignal`].
//!
//! Kapazitaet: ein hartes Limit ist Pflicht, weil der Tree fuer jedes
//! Snapshot komplett kopiert wird (Clone). Default `DEFAULT_CAPACITY`
//! reicht fuer typische Designer-Sessions; bei zu vielen Snapshots wird
//! der **aelteste** Eintrag verworfen (FIFO am Boden des Stacks).

use leptos::prelude::*;

use super::tree::UiTree;
use super::tree::UiTreeSignal;

/// Default-Kapazitaet beider Stacks pro [`BuilderHistory`].
pub const DEFAULT_CAPACITY: usize = 100;

/// Undo/Redo-Container.
///
/// `Copy`, weil alle Felder Leptos-Signal-Handles sind (cheap-to-clone).
/// `capacity` ist nicht reaktiv (immutable nach Konstruktion).
#[derive(Clone, Copy)]
pub struct BuilderHistory {
    pub past: RwSignal<Vec<UiTree>>,
    pub future: RwSignal<Vec<UiTree>>,
    pub capacity: usize,
}

impl BuilderHistory {
    /// Erstellt eine leere History mit dem angegebenen Capacity-Limit.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            past: RwSignal::new(Vec::new()),
            future: RwSignal::new(Vec::new()),
            capacity,
        }
    }

    /// Erstellt eine History mit Default-Kapazitaet.
    pub fn default_capacity() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }

    /// Reaktiver Lese-Zugriff: gibt es mindestens einen Undo-Schritt?
    pub fn can_undo(&self) -> bool {
        self.past.with(|v| !v.is_empty())
    }

    /// Reaktiver Lese-Zugriff: gibt es mindestens einen Redo-Schritt?
    pub fn can_redo(&self) -> bool {
        self.future.with(|v| !v.is_empty())
    }

    /// Aktuelle Stack-Groessen (vor allem fuer Tests/UI-Counter).
    pub fn lengths(&self) -> (usize, usize) {
        (
            self.past.with(Vec::len),
            self.future.with(Vec::len),
        )
    }

    /// Pusht `snapshot` auf den `past`-Stack und leert `future`.
    ///
    /// Diese Methode wird **vor** der eigentlichen Mutation aufgerufen, mit
    /// dem aktuellen Stand des Trees als `snapshot`.
    pub fn record(&self, snapshot: UiTree) {
        let cap = self.capacity;
        self.past.update(|stack| {
            stack.push(snapshot);
            // FIFO-Drop am Boden, wenn ueber Kapazitaet.
            while stack.len() > cap {
                stack.remove(0);
            }
        });
        self.future.update(|f| f.clear());
    }

    /// Schiebt den aktuellen Tree-Stand auf `future` und gibt den vorherigen
    /// Stand vom `past`-Stack zurueck. Liefert `None`, wenn nichts auf
    /// `past` liegt.
    pub fn pop_undo(&self, current: UiTree) -> Option<UiTree> {
        let mut prior: Option<UiTree> = None;
        self.past.update(|stack| {
            prior = stack.pop();
        });
        if let Some(prev) = prior {
            let cap = self.capacity;
            self.future.update(|stack| {
                stack.push(current);
                while stack.len() > cap {
                    stack.remove(0);
                }
            });
            Some(prev)
        } else {
            None
        }
    }

    /// Gegenstueck zu [`pop_undo`](Self::pop_undo): schiebt den aktuellen
    /// Stand auf `past` und gibt den naechsten `future`-Stand zurueck.
    pub fn pop_redo(&self, current: UiTree) -> Option<UiTree> {
        let mut next: Option<UiTree> = None;
        self.future.update(|stack| {
            next = stack.pop();
        });
        if let Some(n) = next {
            let cap = self.capacity;
            self.past.update(|stack| {
                stack.push(current);
                while stack.len() > cap {
                    stack.remove(0);
                }
            });
            Some(n)
        } else {
            None
        }
    }

    /// Leert beide Stacks (z.B. nach erfolgreichem Save in Phase 1.6).
    pub fn clear(&self) {
        self.past.update(|v| v.clear());
        self.future.update(|v| v.clear());
    }
}

impl Default for BuilderHistory {
    fn default() -> Self {
        Self::default_capacity()
    }
}

// =============================================================================
// Convenience-Bindungen an UiTreeSignal
// =============================================================================

/// Fuehrt `f` als Mutation am [`UiTreeSignal`] aus und legt vorher einen
/// Snapshot in der History ab.
pub fn mutate_with_history<F>(tree_sig: UiTreeSignal, history: BuilderHistory, f: F)
where
    F: FnOnce(&mut UiTree),
{
    let snapshot = tree_sig.tree.get_untracked();
    history.record(snapshot);
    tree_sig.update(f);
}

/// Fuehrt einen Undo-Schritt am [`UiTreeSignal`] aus. Liefert `true`,
/// wenn der Tree-Stand veraendert wurde.
pub fn undo(tree_sig: UiTreeSignal, history: BuilderHistory) -> bool {
    let current = tree_sig.tree.get_untracked();
    if let Some(prev) = history.pop_undo(current) {
        tree_sig.tree.set(prev);
        true
    } else {
        false
    }
}

/// Fuehrt einen Redo-Schritt am [`UiTreeSignal`] aus. Liefert `true`,
/// wenn der Tree-Stand veraendert wurde.
pub fn redo(tree_sig: UiTreeSignal, history: BuilderHistory) -> bool {
    let current = tree_sig.tree.get_untracked();
    if let Some(next) = history.pop_redo(current) {
        tree_sig.tree.set(next);
        true
    } else {
        false
    }
}

/// Installiert eine frische [`BuilderHistory`] im Leptos-Context.
pub fn provide_history() -> BuilderHistory {
    let h = BuilderHistory::default_capacity();
    provide_context(h);
    h
}

/// Holt die [`BuilderHistory`] aus dem Leptos-Context.
pub fn use_history() -> BuilderHistory {
    expect_context::<BuilderHistory>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::node::{NodeId, UiNode};

    // Leptos-Signals brauchen einen Owner-Scope. Fuer Unit-Tests liefert
    // `Owner::new()` einen synchronen, eigenstaendigen Scope.
    fn with_owner<R>(f: impl FnOnce() -> R) -> R {
        let owner = Owner::new();
        owner.with(f)
    }

    fn sample_tree(label: u64) -> UiTree {
        let mut t = UiTree::empty();
        t.push_root(UiNode::new(NodeId(label)));
        t
    }

    #[test]
    fn empty_history_cannot_undo_or_redo() {
        with_owner(|| {
            let h = BuilderHistory::new(10);
            assert!(!h.can_undo());
            assert!(!h.can_redo());
        });
    }

    #[test]
    fn record_then_undo_returns_prior_state() {
        with_owner(|| {
            let h = BuilderHistory::new(10);
            let t0 = sample_tree(1);
            let t1 = sample_tree(2);
            h.record(t0.clone());
            assert!(h.can_undo());
            let prev = h.pop_undo(t1.clone()).expect("undo ok");
            assert_eq!(prev, t0);
            assert!(!h.can_undo());
            assert!(h.can_redo());
            let next = h.pop_redo(prev).expect("redo ok");
            assert_eq!(next, t1);
        });
    }

    #[test]
    fn record_clears_future_stack() {
        with_owner(|| {
            let h = BuilderHistory::new(10);
            h.record(sample_tree(1));
            // Simulierter Undo: future enthaelt jetzt einen Eintrag.
            h.pop_undo(sample_tree(2));
            assert!(h.can_redo());
            // Neue Mutation: future muss geleert sein.
            h.record(sample_tree(3));
            assert!(!h.can_redo());
        });
    }

    #[test]
    fn capacity_drops_oldest_entry() {
        with_owner(|| {
            let h = BuilderHistory::new(3);
            for i in 1..=5 {
                h.record(sample_tree(i));
            }
            // Erwartung: 3 juengste Eintraege blieben.
            assert_eq!(h.lengths().0, 3);
            // Aelteste vorhandene ID muss 3 sein (1 und 2 sind gedroppt).
            let oldest = h.past.with(|v| v[0].clone());
            assert_eq!(oldest.nodes[0].id.0, 3);
        });
    }

    #[test]
    fn pop_undo_on_empty_returns_none() {
        with_owner(|| {
            let h = BuilderHistory::new(2);
            assert!(h.pop_undo(sample_tree(1)).is_none());
            assert!(h.pop_redo(sample_tree(1)).is_none());
        });
    }

    #[test]
    fn mutate_with_history_records_and_applies() {
        with_owner(|| {
            let sig = UiTreeSignal::from_tree(sample_tree(1));
            let h = BuilderHistory::new(5);
            mutate_with_history(sig, h, |t| {
                t.push_root(UiNode::new(NodeId(99)));
            });
            assert!(h.can_undo());
            assert_eq!(sig.tree.get_untracked().len(), 2);
        });
    }

    #[test]
    fn undo_redo_round_trip_restores_tree() {
        with_owner(|| {
            let sig = UiTreeSignal::from_tree(sample_tree(1));
            let h = BuilderHistory::new(5);
            mutate_with_history(sig, h, |t| {
                t.push_root(UiNode::new(NodeId(2)));
            });
            assert_eq!(sig.tree.get_untracked().len(), 2);
            assert!(undo(sig, h));
            assert_eq!(sig.tree.get_untracked().len(), 1);
            assert!(redo(sig, h));
            assert_eq!(sig.tree.get_untracked().len(), 2);
        });
    }

    #[test]
    fn clear_drops_both_stacks() {
        with_owner(|| {
            let h = BuilderHistory::new(5);
            h.record(sample_tree(1));
            h.pop_undo(sample_tree(2));
            h.clear();
            assert!(!h.can_undo());
            assert!(!h.can_redo());
        });
    }
}
