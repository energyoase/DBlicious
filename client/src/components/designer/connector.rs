//! Geometrie der Beziehungs-Linien.
//!
//! Es wird ein einfacher horizontaler Bezier zwischen zwei Ankerpunkten
//! gezeichnet. Die Endpunkte werden anhand der Tabellen-Position und der
//! Zeilennummer der Spalte rechnerisch ermittelt; das spart eine echte
//! DOM-Messung (die in CSR-Leptos teuer und ueber Frames hinweg flackrig waere)
//! und ist „gut genug" fuer einen Schema-Designer.

use shared::{DbTable, Position};

/// Fester Wert: Karten-Header in Pixeln (siehe `DesignSystem::designer_table_header`).
const HEADER_HEIGHT: f64 = 32.0;
/// Fester Wert: Zeilenhoehe pro Spalte (siehe `DesignSystem::designer_column_row`).
const ROW_HEIGHT: f64 = 30.0;
/// Konservative Breite der Tabellenkarten (siehe `DesignSystem::designer_table`).
const CARD_WIDTH: f64 = 240.0;

/// Mittelpunkt der Zeile relativ zur Tabellen-Origin (top-left).
fn row_center_y(column_index: usize) -> f64 {
    HEADER_HEIGHT + (column_index as f64) * ROW_HEIGHT + ROW_HEIGHT / 2.0
}

#[derive(Debug, Clone, Copy)]
pub struct Anchor {
    pub x: f64,
    pub y: f64,
}

/// Berechnet die beiden Ankerpunkte zwischen zwei Spalten, automatisch auf
/// der jeweils zueinander zugewandten Seite (links/rechts der Karte).
pub fn anchors_for(
    source: &DbTable,
    source_col: &str,
    target: &DbTable,
    target_col: &str,
) -> Option<(Anchor, Anchor)> {
    let source_idx = source.columns.iter().position(|c| c.id == source_col)?;
    let target_idx = target.columns.iter().position(|c| c.id == target_col)?;

    let src_center = source.position.x + CARD_WIDTH / 2.0;
    let tgt_center = target.position.x + CARD_WIDTH / 2.0;
    let source_on_right = src_center <= tgt_center;

    let s = if source_on_right {
        Anchor { x: source.position.x + CARD_WIDTH, y: source.position.y + row_center_y(source_idx) }
    } else {
        Anchor { x: source.position.x, y: source.position.y + row_center_y(source_idx) }
    };
    let t = if source_on_right {
        Anchor { x: target.position.x, y: target.position.y + row_center_y(target_idx) }
    } else {
        Anchor { x: target.position.x + CARD_WIDTH, y: target.position.y + row_center_y(target_idx) }
    };
    Some((s, t))
}

/// Erzeugt eine SVG-Pfadbeschreibung (Attribut `d`) fuer einen horizontalen
/// Bezier zwischen zwei Punkten.
pub fn bezier_path(s: Anchor, t: Anchor) -> String {
    let dx = (t.x - s.x).abs().max(40.0);
    let control = dx * 0.45;
    let c1x = s.x + control * if t.x >= s.x { 1.0 } else { -1.0 };
    let c2x = t.x - control * if t.x >= s.x { 1.0 } else { -1.0 };
    format!(
        "M {sx:.1} {sy:.1} C {c1x:.1} {sy:.1}, {c2x:.1} {ty:.1}, {tx:.1} {ty:.1}",
        sx = s.x,
        sy = s.y,
        tx = t.x,
        ty = t.y,
        c1x = c1x,
        c2x = c2x,
    )
}

/// Hoehe einer Tabellenkarte – wird vom Canvas verwendet, um die SVG-Flaeche
/// (und damit das Scroll-Bereich) gross genug zu dimensionieren.
pub fn card_height(table: &DbTable) -> f64 {
    HEADER_HEIGHT + (table.columns.len() as f64) * ROW_HEIGHT
}

/// Bounding-Box aller Tabellen – fuer die Mindestgroesse der Leinwand.
pub fn bounds(tables: &[DbTable]) -> (f64, f64) {
    tables.iter().fold((600.0_f64, 400.0_f64), |(w, h), t| {
        let right = t.position.x + CARD_WIDTH + 80.0;
        let bottom = t.position.y + card_height(t) + 80.0;
        (w.max(right), h.max(bottom))
    })
}

/// Reine Hilfsfunktion: Position aus optionalem `DbTable`.
pub fn pos_or_zero(t: Option<&DbTable>) -> Position {
    t.map(|t| t.position).unwrap_or_default()
}
