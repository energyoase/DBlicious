//! Reaktiver Zustand des Datenbank-Designers.
//!
//! Es gibt bewusst genau eine zentrale Struktur (`DesignerModel`), die
//! alle Signale buendelt und ueber Methoden gemutiert wird. Komponenten
//! greifen ausschliesslich ueber diese Methoden zu, damit Invarianten
//! (z.B. „beim Loeschen einer Tabelle muessen abhaengige Beziehungen
//! mitverschwinden") an einer Stelle gepflegt werden.

use leptos::prelude::*;
use shared::{
    ColumnGenerated, DbColumn, DbColumnType, DbRelation, DbSchema, DbTable, DeleteBehavior,
    Position, RelationColumnPair, RelationKind,
};

/// Welche „Seite" einer Spalte ein Port repraesentiert. Wird ausschliesslich
/// fuer die Geometrie der Verbindungslinien benoetigt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortSide {
    Left,
    Right,
}

/// Stabile Adresse eines Ports (Tabelle + Spalte + Seite).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortAddress {
    pub table_id: String,
    pub column_id: String,
    pub side: PortSide,
}

/// Laufender Drag-Vorgang einer Tabelle.
#[derive(Debug, Clone)]
pub struct DragState {
    pub table_id: String,
    /// Versatz zwischen Cursor und Tabellen-Origin im Moment des `pointerdown`.
    pub offset_x: f64,
    pub offset_y: f64,
}

/// Status der letzten `saveDbSchema`-Mutation.
#[derive(Debug, Clone)]
pub enum SaveStatus {
    Idle,
    Saving,
    Ok(String),
    Err(String),
}

/// Hauptmodell. Wird ueber Context bereitgestellt und von allen
/// Designer-Komponenten geteilt.
#[derive(Clone, Copy)]
pub struct DesignerModel {
    pub schema_name: RwSignal<String>,
    pub tables: RwSignal<Vec<DbTable>>,
    pub relations: RwSignal<Vec<DbRelation>>,

    /// Naechste freie ID. Wir bauen IDs lokal („t-1", „c-7", „r-3"); der
    /// Server vergibt in einer spaeteren Iteration eigene IDs.
    pub next_id: RwSignal<u64>,

    pub selected_table: RwSignal<Option<String>>,
    pub drag: RwSignal<Option<DragState>>,

    /// Verknuepfungsmodus: wenn aktiv, klickt der Nutzer zwei Spalten an,
    /// um eine Beziehung zu erzeugen. `pending_source` haelt den ersten Klick.
    pub link_mode: RwSignal<bool>,
    pub pending_source: RwSignal<Option<PortAddress>>,

    pub save_status: RwSignal<SaveStatus>,
}

impl DesignerModel {
    /// Initialisiert das Modell mit einem kleinen Beispiel-Schema, damit
    /// die Leinwand beim ersten Aufruf nicht leer ist.
    pub fn with_demo() -> Self {
        let model = Self {
            schema_name: RwSignal::new(String::from("Neues Schema")),
            tables: RwSignal::new(Vec::new()),
            relations: RwSignal::new(Vec::new()),
            next_id: RwSignal::new(1),
            selected_table: RwSignal::new(None),
            drag: RwSignal::new(None),
            link_mode: RwSignal::new(false),
            pending_source: RwSignal::new(None),
            save_status: RwSignal::new(SaveStatus::Idle),
        };
        model.seed_demo();
        model
    }

    fn seed_demo(&self) {
        let customer_id = self.add_table_at("Kunde", 80.0, 80.0);
        let cust_id_col = self
            .last_column_id(&customer_id)
            .expect("Demo: Kunde muss eine Spalte haben");
        self.add_column(
            &customer_id,
            "name",
            DbColumnType::Text,
            false,
            false,
            false,
        );
        self.add_column(
            &customer_id,
            "email",
            DbColumnType::Text,
            false,
            false,
            true,
        );

        let order_id = self.add_table_at("Bestellung", 420.0, 60.0);
        let _ = self.last_column_id(&order_id);
        self.add_column(
            &order_id,
            "kunde_id",
            DbColumnType::ForeignKey,
            false,
            false,
            false,
        );
        self.add_column(
            &order_id,
            "betrag",
            DbColumnType::Decimal {
                precision: 12,
                scale: 2,
            },
            false,
            false,
            false,
        );
        self.add_column(
            &order_id,
            "bestellt_am",
            DbColumnType::DateTime,
            false,
            false,
            false,
        );

        let order_kunde_fk = self.tables.with_untracked(|tables| {
            tables
                .iter()
                .find(|t| t.id == order_id)
                .and_then(|t| t.columns.iter().find(|c| c.name == "kunde_id"))
                .map(|c| c.id.clone())
        });

        if let Some(fk) = order_kunde_fk {
            self.add_relation(
                &order_id,
                &fk,
                &customer_id,
                &cust_id_col,
                RelationKind::ManyToOne,
            );
        }
    }

    fn alloc_id(&self, prefix: &str) -> String {
        let n = self.next_id.get_untracked();
        self.next_id.set(n + 1);
        format!("{prefix}-{n}")
    }

    fn last_column_id(&self, table_id: &str) -> Option<String> {
        self.tables.with_untracked(|tables| {
            tables
                .iter()
                .find(|t| t.id == table_id)
                .and_then(|t| t.columns.last().map(|c| c.id.clone()))
        })
    }

    // ------------------------------------------------------------------
    // Tabellen
    // ------------------------------------------------------------------

    /// Fuegt eine neue Tabelle an der Position (x, y) auf der Leinwand ein.
    /// Gibt die generierte Tabellen-ID zurueck.
    pub fn add_table_at(&self, name: &str, x: f64, y: f64) -> String {
        let table_id = self.alloc_id("t");
        let id_col = DbColumn {
            id: self.alloc_id("c"),
            name: "id".into(),
            data_type: DbColumnType::Uuid,
            nullable: false,
            primary_key: true,
            unique: true,
            generated: ColumnGenerated::OnAdd,
            concurrency_token: false,
            default_value: None,
            audit_role: shared::AuditRole::None,
        };
        let table = DbTable {
            id: table_id.clone(),
            name: name.into(),
            position: Position { x, y },
            columns: vec![id_col],
        };
        self.tables.update(|t| t.push(table));
        self.selected_table.set(Some(table_id.clone()));
        table_id
    }

    pub fn remove_table(&self, table_id: &str) {
        self.tables
            .update(|tables| tables.retain(|t| t.id != table_id));
        // Verwaiste Beziehungen mit entfernen.
        self.relations.update(|rs| {
            rs.retain(|r| r.source_table_id != table_id && r.target_table_id != table_id)
        });
        if self.selected_table.get_untracked().as_deref() == Some(table_id) {
            self.selected_table.set(None);
        }
        // Eventuell stehengebliebene Auswahl im Linkmodus mitloeschen.
        if let Some(p) = self.pending_source.get_untracked() {
            if p.table_id == table_id {
                self.pending_source.set(None);
            }
        }
    }

    pub fn rename_table(&self, table_id: &str, new_name: String) {
        self.tables.update(|tables| {
            if let Some(t) = tables.iter_mut().find(|t| t.id == table_id) {
                t.name = new_name;
            }
        });
    }

    pub fn set_position(&self, table_id: &str, x: f64, y: f64) {
        self.tables.update(|tables| {
            if let Some(t) = tables.iter_mut().find(|t| t.id == table_id) {
                t.position = Position { x, y };
            }
        });
    }

    /// Liefert die aktuelle Position oder (0, 0) falls Tabelle nicht (mehr) existiert.
    pub fn position_of(&self, table_id: &str) -> Position {
        self.tables.with_untracked(|tables| {
            tables
                .iter()
                .find(|t| t.id == table_id)
                .map(|t| t.position)
                .unwrap_or_default()
        })
    }

    // ------------------------------------------------------------------
    // Spalten
    // ------------------------------------------------------------------

    pub fn add_column(
        &self,
        table_id: &str,
        name: &str,
        data_type: DbColumnType,
        primary_key: bool,
        nullable: bool,
        unique: bool,
    ) -> Option<String> {
        let column_id = self.alloc_id("c");
        let mut inserted = false;
        let col = DbColumn {
            id: column_id.clone(),
            name: name.into(),
            data_type,
            nullable,
            primary_key,
            unique,
            generated: ColumnGenerated::Never,
            concurrency_token: false,
            default_value: None,
            audit_role: shared::AuditRole::None,
        };
        self.tables.update(|tables| {
            if let Some(t) = tables.iter_mut().find(|t| t.id == table_id) {
                t.columns.push(col);
                inserted = true;
            }
        });
        inserted.then_some(column_id)
    }

    pub fn remove_column(&self, table_id: &str, column_id: &str) {
        self.tables.update(|tables| {
            if let Some(t) = tables.iter_mut().find(|t| t.id == table_id) {
                t.columns.retain(|c| c.id != column_id);
            }
        });
        self.relations.update(|rs| {
            rs.retain(|r| {
                let touches_source = r.source_table_id == table_id
                    && r.column_pairs
                        .iter()
                        .any(|p| p.source_column_id == column_id);
                let touches_target = r.target_table_id == table_id
                    && r.column_pairs
                        .iter()
                        .any(|p| p.target_column_id == column_id);
                !(touches_source || touches_target)
            })
        });
        if let Some(p) = self.pending_source.get_untracked() {
            if p.table_id == table_id && p.column_id == column_id {
                self.pending_source.set(None);
            }
        }
    }

    pub fn rename_column(&self, table_id: &str, column_id: &str, new_name: String) {
        self.tables.update(|tables| {
            if let Some(t) = tables.iter_mut().find(|t| t.id == table_id) {
                if let Some(c) = t.columns.iter_mut().find(|c| c.id == column_id) {
                    c.name = new_name;
                }
            }
        });
    }

    pub fn set_column_type(&self, table_id: &str, column_id: &str, data_type: DbColumnType) {
        self.tables.update(|tables| {
            if let Some(t) = tables.iter_mut().find(|t| t.id == table_id) {
                if let Some(c) = t.columns.iter_mut().find(|c| c.id == column_id) {
                    c.data_type = data_type;
                }
            }
        });
    }

    pub fn toggle_pk(&self, table_id: &str, column_id: &str) {
        self.tables.update(|tables| {
            if let Some(t) = tables.iter_mut().find(|t| t.id == table_id) {
                if let Some(c) = t.columns.iter_mut().find(|c| c.id == column_id) {
                    c.primary_key = !c.primary_key;
                }
            }
        });
    }

    pub fn toggle_nullable(&self, table_id: &str, column_id: &str) {
        self.tables.update(|tables| {
            if let Some(t) = tables.iter_mut().find(|t| t.id == table_id) {
                if let Some(c) = t.columns.iter_mut().find(|c| c.id == column_id) {
                    c.nullable = !c.nullable;
                }
            }
        });
    }

    // ------------------------------------------------------------------
    // Beziehungen
    // ------------------------------------------------------------------

    pub fn add_relation(
        &self,
        source_table_id: &str,
        source_column_id: &str,
        target_table_id: &str,
        target_column_id: &str,
        kind: RelationKind,
    ) -> Option<String> {
        // Doppelte (gleiche Endpunkte) verhindern.
        let already = self.relations.with_untracked(|rs| {
            rs.iter().any(|r| {
                r.source_table_id == source_table_id
                    && r.target_table_id == target_table_id
                    && r.column_pairs.len() == 1
                    && r.column_pairs[0].source_column_id == source_column_id
                    && r.column_pairs[0].target_column_id == target_column_id
            })
        });
        if already {
            return None;
        }
        let id = self.alloc_id("r");
        let rel = DbRelation {
            id: id.clone(),
            name: String::new(),
            kind,
            on_delete: DeleteBehavior::default(),
            required: false,
            source_table_id: source_table_id.into(),
            target_table_id: target_table_id.into(),
            column_pairs: vec![RelationColumnPair {
                source_column_id: source_column_id.into(),
                target_column_id: target_column_id.into(),
            }],
        };
        self.relations.update(|rs| rs.push(rel));
        Some(id)
    }

    pub fn remove_relation(&self, relation_id: &str) {
        self.relations
            .update(|rs| rs.retain(|r| r.id != relation_id));
    }

    // ------------------------------------------------------------------
    // Verknuepfungsmodus
    // ------------------------------------------------------------------

    /// Wird beim Klick auf einen Port aufgerufen. Erst Klick merkt den
    /// Ausgangspunkt, zweiter Klick schliesst die Beziehung ab.
    pub fn handle_port_click(&self, addr: PortAddress) {
        if !self.link_mode.get_untracked() {
            return;
        }
        let current = self.pending_source.get_untracked();
        match current {
            None => self.pending_source.set(Some(addr)),
            Some(source) => {
                // Klick auf denselben Port = abbrechen.
                if source == addr {
                    self.pending_source.set(None);
                    return;
                }
                self.add_relation(
                    &source.table_id,
                    &source.column_id,
                    &addr.table_id,
                    &addr.column_id,
                    RelationKind::ManyToOne,
                );
                self.pending_source.set(None);
            }
        }
    }

    pub fn cancel_pending_link(&self) {
        self.pending_source.set(None);
    }

    // ------------------------------------------------------------------
    // Serialisierung
    // ------------------------------------------------------------------

    /// Baut ein `DbSchema` aus dem aktuellen Stand. Wird unmittelbar vor
    /// dem Versand an den Server aufgerufen.
    pub fn snapshot(&self) -> DbSchema {
        DbSchema {
            id: "local".into(),
            name: self.schema_name.get_untracked(),
            tables: self.tables.get_untracked(),
            relations: self.relations.get_untracked(),
            keys: Vec::new(),
            indices: Vec::new(),
        }
    }
}

/// Context-Provider. Stellt das Modell allen Designer-Komponenten zur Verfuegung.
pub fn provide_designer_model() -> DesignerModel {
    let model = DesignerModel::with_demo();
    provide_context(model);
    model
}

pub fn use_designer_model() -> DesignerModel {
    use_context::<DesignerModel>().expect("DesignerModel nicht im Context")
}
