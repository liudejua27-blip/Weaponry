use std::collections::BTreeMap;

use serde_json::Value;

use crate::{valid_stable_id, RpcError};

pub(crate) const MAX_NOTE_CHARS: usize = 1_000;
pub(crate) const MAX_JSON_BYTES: usize = 1_048_576;

pub(crate) fn require_schema(field: &str, actual: &str, expected: &str) -> Result<(), RpcError> {
    if actual != expected {
        return Err(RpcError::invalid_params(format!(
            "{field} must be {expected}."
        )));
    }
    Ok(())
}

pub(crate) fn require_stable_id(field: &str, value: &str) -> Result<(), RpcError> {
    if !valid_stable_id(value) {
        return Err(RpcError::invalid_params(format!(
            "{field} must be a stable ID containing 1 to 160 ASCII letters, digits, '_', '-' or '.'."
        )));
    }
    Ok(())
}

pub(crate) fn require_optional_stable_id(field: &str, value: Option<&str>) -> Result<(), RpcError> {
    if let Some(value) = value {
        require_stable_id(field, value)?;
    }
    Ok(())
}

pub(crate) fn require_text(
    field: &str,
    value: &str,
    min_chars: usize,
    max_chars: usize,
) -> Result<(), RpcError> {
    let count = value.chars().count();
    if !(min_chars..=max_chars).contains(&count) {
        return Err(RpcError::invalid_params(format!(
            "{field} must contain between {min_chars} and {max_chars} characters."
        )));
    }
    Ok(())
}

pub(crate) fn require_sha256(field: &str, value: &str) -> Result<(), RpcError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RpcError::invalid_params(format!(
            "{field} must be a lowercase SHA-256 hex digest."
        )));
    }
    Ok(())
}

pub(crate) fn require_bounded_json(
    field: &str,
    value: &BTreeMap<String, Value>,
    forbidden_keys: &[&str],
) -> Result<(), RpcError> {
    let encoded = serde_json::to_vec(value)
        .map_err(|error| RpcError::invalid_params(format!("{field} is not JSON: {error}")))?;
    if encoded.len() > MAX_JSON_BYTES {
        return Err(RpcError::invalid_params(format!(
            "{field} must contain at most {MAX_JSON_BYTES} encoded bytes."
        )));
    }
    if let Some(key) = first_forbidden_key(
        &Value::Object(
            value
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
        ),
        forbidden_keys,
    ) {
        return Err(RpcError::invalid_params(format!(
            "{field} contains forbidden key {key}."
        )));
    }
    Ok(())
}

pub(crate) fn first_forbidden_key<'a>(
    value: &'a Value,
    forbidden_keys: &[&str],
) -> Option<&'a str> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if forbidden_keys.iter().any(|forbidden| key == forbidden) {
                    return Some(key.as_str());
                }
                if let Some(found) = first_forbidden_key(nested, forbidden_keys) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(values) => values
            .iter()
            .find_map(|nested| first_forbidden_key(nested, forbidden_keys)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn nested_forbidden_keys_are_rejected() {
        let value = BTreeMap::from([(
            "outer".to_string(),
            json!({"nested": {"provider_key": "secret"}}),
        )]);
        assert!(require_bounded_json("arguments", &value, &["provider_key"]).is_err());
    }

    #[test]
    fn sha256_validation_is_lowercase_and_exact() {
        require_sha256("hash", &"a".repeat(64)).unwrap();
        assert!(require_sha256("hash", &"A".repeat(64)).is_err());
        assert!(require_sha256("hash", &"a".repeat(63)).is_err());
    }
}
