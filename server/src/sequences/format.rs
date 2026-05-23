//! Number-Sequence-Template-Renderer (Phase 1.7.1).
//!
//! Minimaler Subset von Mustache/Tera mit drei Variablen:
//!
//! | Marker          | Wert                                                |
//! |-----------------|-----------------------------------------------------|
//! | `{scope}`       | der `scope`-String                                  |
//! | `{year}`        | das `year` als Dezimalzahl                          |
//! | `{seq}`         | die naechste Nummer ohne Padding                    |
//! | `{seq:NN}`      | die Nummer als Dezimalzahl, links mit `0` auf Breite `NN` aufgefuellt (NN: 1-9) |
//!
//! Beispiele:
//! - `"{seq:06}"`            → `"000042"`
//! - `"INV-{year}-{seq:06}"` → `"INV-2026-000042"`
//! - `"{scope}/{seq}"`       → `"invoice/42"`
//!
//! Unbekannte Marker liefern [`SequenceError::InvalidTemplate`]. Echte
//! geschweifte Klammern im Template kann es nicht geben (Marker-only ist
//! pragmatisch — alle realen ERP-Format-Templates kommen ohne `{`/`}`-
//! Literale aus).

use super::SequenceError;

pub fn render(template: &str, scope: &str, year: i32, seq: i64) -> Result<String, SequenceError> {
    let mut out = String::with_capacity(template.len() + 16);
    let mut iter = template.char_indices();
    while let Some((i, c)) = iter.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        // Token suchen: alles bis zum naechsten '}'.
        let rest = &template[i + 1..];
        let Some(end) = rest.find('}') else {
            return Err(SequenceError::InvalidTemplate(format!(
                "unschliessende Klammer ab Position {i} in '{template}'"
            )));
        };
        let token = &rest[..end];
        // Iter weiterspulen — `end` ist relativ zu `rest`, also `i + 1 + end + 1` absolut.
        // Wir nutzen Bytes, weil unsere Tokens ASCII sind.
        let consume = end + 1;
        for _ in 0..consume {
            iter.next();
        }
        match token {
            "scope" => out.push_str(scope),
            "year" => {
                use std::fmt::Write;
                let _ = write!(out, "{year}");
            }
            "seq" => {
                use std::fmt::Write;
                let _ = write!(out, "{seq}");
            }
            other if other.starts_with("seq:") => {
                let width_str = &other[4..];
                let Some(width) = width_str
                    .parse::<usize>()
                    .ok()
                    .filter(|w| (1..=9).contains(w))
                else {
                    return Err(SequenceError::InvalidTemplate(format!(
                        "ungueltige seq-Breite '{width_str}' (erlaubt: 1..=9)"
                    )));
                };
                use std::fmt::Write;
                let _ = write!(out, "{seq:0>width$}", seq = seq, width = width);
            }
            other => {
                return Err(SequenceError::InvalidTemplate(format!(
                    "unbekannter Marker '{other}'"
                )));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_seq_unpadded() {
        assert_eq!(render("{seq}", "invoice", 2026, 42).unwrap(), "42");
    }

    #[test]
    fn renders_seq_padded() {
        assert_eq!(render("{seq:06}", "invoice", 2026, 42).unwrap(), "000042");
    }

    #[test]
    fn renders_full_template() {
        assert_eq!(
            render("INV-{year}-{seq:06}", "invoice", 2026, 7).unwrap(),
            "INV-2026-000007"
        );
    }

    #[test]
    fn renders_scope_and_year() {
        assert_eq!(
            render("{scope}/{year}/{seq:04}", "order", 2024, 99).unwrap(),
            "order/2024/0099"
        );
    }

    #[test]
    fn rejects_unknown_marker() {
        let err = render("{nope}", "x", 0, 1).unwrap_err();
        assert!(matches!(err, SequenceError::InvalidTemplate(ref m) if m.contains("nope")));
    }

    #[test]
    fn rejects_unterminated_marker() {
        let err = render("{seq", "x", 0, 1).unwrap_err();
        assert!(matches!(err, SequenceError::InvalidTemplate(_)));
    }

    #[test]
    fn rejects_zero_width() {
        let err = render("{seq:0}", "x", 0, 1).unwrap_err();
        assert!(matches!(err, SequenceError::InvalidTemplate(_)));
    }

    #[test]
    fn no_marker_passes_through() {
        assert_eq!(render("static-text", "x", 0, 1).unwrap(), "static-text");
    }
}
