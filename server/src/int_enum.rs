//! Grenz-Konvertierung fuer [`shared::FieldType::IntEnum`].
//!
//! Read-Pfad (DB -> Wire): `decode(i32, &values)` -> wire_name `String`.
//! Write-Pfad (Wire -> DB): `encode(&str, &values)` -> `i32`.
//! Unbekannte Werte: Lese-Pfad liefert `Null` (defensiv), Schreib-Pfad
//! lehnt mit [`IntEnumError`] ab — einen Tippfehler still auf eine falsche
//! Zahl zu mappen waere das schlechtestmoegliche Ergebnis.

use serde_json::Value;
use shared::IntEnumValue;

#[derive(Debug, thiserror::Error)]
pub enum IntEnumError {
    #[error("unknown intEnum wire name: {0}")]
    UnknownWireName(String),
    #[error("expected string for intEnum value, got {0}")]
    WrongType(&'static str),
}

pub fn decode(stored: &Value, values: &[IntEnumValue]) -> Value {
    let Some(n) = stored.as_i64() else {
        return Value::Null;
    };
    match values.iter().find(|v| v.value as i64 == n) {
        Some(v) => Value::String(v.wire_name.clone()),
        None => Value::Null,
    }
}

pub fn encode(incoming: &Value, values: &[IntEnumValue]) -> Result<Value, IntEnumError> {
    if incoming.is_null() {
        return Ok(Value::Null);
    }
    let Some(name) = incoming.as_str() else {
        return Err(IntEnumError::WrongType(type_name(incoming)));
    };
    let v = values
        .iter()
        .find(|v| v.wire_name == name)
        .ok_or_else(|| IntEnumError::UnknownWireName(name.into()))?;
    Ok(Value::Number(serde_json::Number::from(v.value)))
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> Vec<IntEnumValue> {
        vec![
            IntEnumValue {
                value: 0,
                label_key: "soll".into(),
                wire_name: "SOLL".into(),
            },
            IntEnumValue {
                value: 1,
                label_key: "haben".into(),
                wire_name: "HABEN".into(),
            },
        ]
    }

    #[test]
    fn decode_known_int_to_wire_name() {
        assert_eq!(
            decode(&serde_json::json!(0), &values()),
            Value::String("SOLL".into())
        );
        assert_eq!(
            decode(&serde_json::json!(1), &values()),
            Value::String("HABEN".into())
        );
    }

    #[test]
    fn decode_unknown_int_to_null() {
        assert_eq!(decode(&serde_json::json!(99), &values()), Value::Null);
    }

    #[test]
    fn encode_known_name_to_int() {
        assert_eq!(
            encode(&serde_json::json!("SOLL"), &values()).unwrap(),
            serde_json::json!(0)
        );
    }

    #[test]
    fn encode_unknown_name_errors() {
        let err = encode(&serde_json::json!("MAYBE"), &values()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("MAYBE"));
    }

    #[test]
    fn encode_wrong_type_errors() {
        let err = encode(&serde_json::json!(7), &values()).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("number"));
    }

    #[test]
    fn encode_null_passes_through() {
        assert_eq!(encode(&Value::Null, &values()).unwrap(), Value::Null);
    }
}
