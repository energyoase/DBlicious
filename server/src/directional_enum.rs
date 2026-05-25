//! Grenz-Konvertierung fuer [`shared::FieldType::DirectionalEnum`].
//! Teilt die int<->wire-Logik mit `int_enum` ueber `boundary_enum`-Paare;
//! `sign` ist fuer die Konvertierung irrelevant (erst Aggregation/Welle 2).

use serde_json::Value;
use shared::DirectionalEnumValue;

pub fn decode(stored: &Value, values: &[DirectionalEnumValue]) -> Value {
    let pairs: Vec<(i32, &str)> = values
        .iter()
        .map(|v| (v.value, v.wire_name.as_str()))
        .collect();
    crate::int_enum::decode_pairs(stored, &pairs)
}

pub fn encode(
    incoming: &Value,
    values: &[DirectionalEnumValue],
) -> Result<Value, crate::int_enum::IntEnumError> {
    let pairs: Vec<(i32, &str)> = values
        .iter()
        .map(|v| (v.value, v.wire_name.as_str()))
        .collect();
    crate::int_enum::encode_pairs(incoming, &pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn values() -> Vec<DirectionalEnumValue> {
        vec![
            DirectionalEnumValue {
                value: 0,
                label_key: "soll".into(),
                wire_name: "SOLL".into(),
                sign: 1,
            },
            DirectionalEnumValue {
                value: 1,
                label_key: "haben".into(),
                wire_name: "HABEN".into(),
                sign: -1,
            },
        ]
    }
    #[test]
    fn decode_known_int_to_wire() {
        assert_eq!(
            decode(&serde_json::json!(1), &values()),
            Value::String("HABEN".into())
        );
    }
    #[test]
    fn decode_unknown_to_null() {
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
        assert!(encode(&serde_json::json!("X"), &values()).is_err());
    }
}
