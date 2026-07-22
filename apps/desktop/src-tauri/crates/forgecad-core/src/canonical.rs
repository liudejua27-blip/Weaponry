use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::{CoreError, CoreResult};

/// Serialize JSON with recursively sorted object keys and no insignificant
/// whitespace. This matches the semantic-hash role used by the Python
/// compatibility fixtures without treating SQLite row order as meaning.
pub fn canonical_json<T: Serialize + ?Sized>(value: &T) -> CoreResult<String> {
    let value = serde_json::to_value(value)
        .map_err(|error| CoreError::invalid_data("JSON_SERIALIZATION_FAILED", error.to_string()))?;
    serde_json::to_string(&canonicalize(value))
        .map_err(|error| CoreError::invalid_data("JSON_SERIALIZATION_FAILED", error.to_string()))
}

pub fn semantic_sha256<T: Serialize + ?Sized>(value: &T) -> CoreResult<String> {
    let canonical = canonical_json(value)?;
    Ok(sha256_bytes(canonical.as_bytes()))
}

pub(crate) fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn canonicalize(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries = object.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, canonicalize(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize).collect()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn semantic_hash_ignores_object_key_order_but_not_array_order() {
        let left = json!({"b": [2, 1], "a": {"d": 4, "c": 3}});
        let right = json!({"a": {"c": 3, "d": 4}, "b": [2, 1]});
        let changed = json!({"a": {"c": 3, "d": 4}, "b": [1, 2]});
        assert_eq!(
            semantic_sha256(&left).unwrap(),
            semantic_sha256(&right).unwrap()
        );
        assert_ne!(
            semantic_sha256(&left).unwrap(),
            semantic_sha256(&changed).unwrap()
        );
    }

    #[test]
    fn canonical_json_round_trips_adversarial_shape_program_floats() {
        let source = r#"{"rotation":[0.0,-0.0,0.9272952180016123,-0.6435011087932844,1e-7,1e20]}"#;
        let parsed: Value = serde_json::from_str(source).unwrap();
        let canonical = canonical_json(&parsed).unwrap();
        let reparsed: Value = serde_json::from_str(&canonical).unwrap();
        assert_eq!(canonical, canonical_json(&reparsed).unwrap());
        assert_eq!(
            semantic_sha256(&parsed).unwrap(),
            semantic_sha256(&reparsed).unwrap()
        );
        assert!(canonical.contains("0.9272952180016123"));
        assert!(canonical.contains("-0.6435011087932844"));
    }
}
