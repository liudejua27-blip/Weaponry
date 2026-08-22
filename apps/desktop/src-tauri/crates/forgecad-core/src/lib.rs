use sha2::{Digest, Sha256};

pub mod feature_graph;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("invalid JSON payload: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

/// Serialises JSON independently of map insertion order so every Runtime
/// process computes the same request/object digest.  The Runtime contracts
/// intentionally keep this implementation small and dependency-free; no
/// renderer or model is allowed to provide a competing hash.
pub fn canonical_json_bytes(value: &serde_json::Value) -> Result<Vec<u8>, CoreError> {
    fn write(value: &serde_json::Value, output: &mut Vec<u8>) -> Result<(), CoreError> {
        match value {
            serde_json::Value::Null => output.extend_from_slice(b"null"),
            serde_json::Value::Bool(value) => {
                output.extend_from_slice(if *value { b"true" } else { b"false" })
            }
            serde_json::Value::Number(value) => {
                output.extend_from_slice(value.to_string().as_bytes())
            }
            serde_json::Value::String(value) => {
                serde_json::to_writer(output, value).map_err(CoreError::InvalidJson)?
            }
            serde_json::Value::Array(values) => {
                output.push(b'[');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.push(b',');
                    }
                    write(value, output)?;
                }
                output.push(b']');
            }
            serde_json::Value::Object(values) => {
                let mut keys = values.keys().collect::<Vec<_>>();
                keys.sort_unstable();
                output.push(b'{');
                for (index, key) in keys.iter().enumerate() {
                    if index != 0 {
                        output.push(b',');
                    }
                    serde_json::to_writer(&mut *output, key).map_err(CoreError::InvalidJson)?;
                    output.push(b':');
                    write(&values[*key], output)?;
                }
                output.push(b'}');
            }
        }
        Ok(())
    }

    let mut output = Vec::new();
    write(value, &mut output)?;
    Ok(output)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn canonical_json_hash(value: &serde_json::Value) -> String {
    sha256_hex(&canonical_json_bytes(value).expect("JSON values are serializable"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_hash_is_stable_for_same_value() {
        let value = serde_json::json!({"a": 1, "b": [true, null]});
        assert_eq!(canonical_json_hash(&value), canonical_json_hash(&value));
        assert_eq!(canonical_json_hash(&value).len(), 64);
    }

    #[test]
    fn canonical_hash_does_not_depend_on_object_order() {
        let left = serde_json::json!({"z": 1, "a": {"y": 2, "b": 3}});
        let right = serde_json::json!({"a": {"b": 3, "y": 2}, "z": 1});
        assert_eq!(canonical_json_hash(&left), canonical_json_hash(&right));
    }

    #[test]
    fn byte_hash_is_known_and_lowercase() {
        assert_eq!(
            sha256_hex(b"ForgeCAD"),
            "63ac8d4d27776c78674d43dc5c63638afd8ebc8d843eb0426b4a1997dd14b38e"
        );
    }
}
