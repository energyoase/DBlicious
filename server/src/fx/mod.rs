//! FX-Rate-Conversion-Service (Phase 1.7.2).
//!
//! Aufgabe: gegebenen Betrag in `from_currency` zum Stichtag in
//! `to_currency` umrechnen.
//!
//! Lookup-Strategie:
//!   1. exakter Match `(date, from, to)` → benutze diese rate
//!   2. exakter Match `(date, to, from)` → invertiere (1.0 / rate)
//!   3. juengster Match `<= date` fuer das Paar (forward oder invertiert)
//!   4. nichts gefunden → `FxError::NoRate`
//!
//! Rundung: standardmaessig Banker-Rounding (`f64::round_ties_even`) auf
//! gewuenschte Dezimalstellen. Aufrufer entscheidet Skalierung pro
//! Waehrung (EUR/USD typisch 2, JPY 0, BHD 3, …).
//!
//! Provider: heute manuell — Sets fuettern via `upsert_rate` (z.B. aus
//! CLI/Loader/Plugin). ECB-Daily-Fetch ist out-of-scope, kommt als
//! Plugin oder als spaeterer Background-Job (1.7.7).

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use thiserror::Error;

use crate::entity::fx_rates;

#[derive(Debug, Error)]
pub enum FxError {
    #[error("database error: {0}")]
    Db(#[from] sea_orm::DbErr),
    #[error("no rate available for {from}→{to} on or before {date}")]
    NoRate {
        from: String,
        to: String,
        date: String,
    },
}

/// Bequemes Pair-Tupel.
#[derive(Debug, Clone, Copy)]
struct Pair<'a> {
    from: &'a str,
    to: &'a str,
}

/// Legt einen Rate-Eintrag an oder ueberschreibt einen vorhandenen
/// `(date, from, to)`. Provider-Tag (`source`) frei.
pub async fn upsert_rate(
    conn: &DatabaseConnection,
    date: &str,
    from: &str,
    to: &str,
    rate: f64,
    source: &str,
) -> Result<(), FxError> {
    let existing =
        fx_rates::Entity::find_by_id((date.to_string(), from.to_string(), to.to_string()))
            .one(conn)
            .await?;
    match existing {
        Some(m) => {
            let mut am: fx_rates::ActiveModel = m.into();
            am.rate = ActiveValue::Set(rate);
            am.source = ActiveValue::Set(source.to_string());
            am.update(conn).await?;
        }
        None => {
            fx_rates::ActiveModel {
                date: ActiveValue::Set(date.to_string()),
                from_currency: ActiveValue::Set(from.to_string()),
                to_currency: ActiveValue::Set(to.to_string()),
                rate: ActiveValue::Set(rate),
                source: ActiveValue::Set(source.to_string()),
                tenant_id: ActiveValue::Set(None),
            }
            .insert(conn)
            .await?;
        }
    }
    Ok(())
}

/// Findet die anwendbare rate zum Stichtag (≤ date). Liefert `(rate,
/// inverted)` — `inverted=true` bedeutet, der Aufrufer muss `1.0/rate`
/// anwenden.
async fn lookup_rate(
    conn: &DatabaseConnection,
    date: &str,
    pair: Pair<'_>,
) -> Result<Option<(f64, bool)>, FxError> {
    // 1. forward
    let forward = fx_rates::Entity::find()
        .filter(fx_rates::Column::FromCurrency.eq(pair.from))
        .filter(fx_rates::Column::ToCurrency.eq(pair.to))
        .filter(fx_rates::Column::Date.lte(date))
        .order_by_desc(fx_rates::Column::Date)
        .one(conn)
        .await?;
    if let Some(m) = forward {
        return Ok(Some((m.rate, false)));
    }
    // 2. inverted
    let inverted = fx_rates::Entity::find()
        .filter(fx_rates::Column::FromCurrency.eq(pair.to))
        .filter(fx_rates::Column::ToCurrency.eq(pair.from))
        .filter(fx_rates::Column::Date.lte(date))
        .order_by_desc(fx_rates::Column::Date)
        .one(conn)
        .await?;
    Ok(inverted.map(|m| (m.rate, true)))
}

/// Konvertiert `amount` von `from` nach `to` zum `date`.
///
/// `scale` steuert die Banker-Rounding-Dezimalstellen (z.B. `2` fuer
/// EUR/USD, `0` fuer JPY).
pub async fn convert(
    conn: &DatabaseConnection,
    amount: f64,
    from: &str,
    to: &str,
    date: &str,
    scale: u32,
) -> Result<f64, FxError> {
    // Identität: gleiche Waehrung ⇒ runden + zurueck.
    if from.eq_ignore_ascii_case(to) {
        return Ok(banker_round(amount, scale));
    }
    let pair = Pair { from, to };
    let (rate, inverted) = match lookup_rate(conn, date, pair).await? {
        Some(r) => r,
        None => {
            return Err(FxError::NoRate {
                from: from.to_string(),
                to: to.to_string(),
                date: date.to_string(),
            });
        }
    };
    let raw = if inverted {
        amount / rate
    } else {
        amount * rate
    };
    Ok(banker_round(raw, scale))
}

/// Banker-Rounding (round-half-to-even) auf `scale` Dezimalstellen.
/// Nutzt `f64::round_ties_even` (Rust 1.77+).
pub fn banker_round(value: f64, scale: u32) -> f64 {
    let factor = 10f64.powi(scale as i32);
    (value * factor).round_ties_even() / factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banker_round_half_to_even() {
        // 0.125 → 0.12 (next even), 0.135 → 0.14 (next even)
        assert_eq!(banker_round(0.125, 2), 0.12);
        assert_eq!(banker_round(0.135, 2), 0.14);
        // 0.5 → 0 (even); 1.5 → 2 (even); 2.5 → 2 (even); 3.5 → 4 (even)
        assert_eq!(banker_round(0.5, 0), 0.0);
        assert_eq!(banker_round(1.5, 0), 2.0);
        assert_eq!(banker_round(2.5, 0), 2.0);
        assert_eq!(banker_round(3.5, 0), 4.0);
    }
}
