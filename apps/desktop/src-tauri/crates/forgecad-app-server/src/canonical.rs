use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) fn canonical_json(value: &Value) -> String {
    let mut output = String::new();
    write_value(value, &mut output);
    output
}

fn write_value(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => {
            output
                .push_str(&serde_json::to_string(value).expect("serializing a string cannot fail"));
        }
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_value(value, output);
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output
                    .push_str(&serde_json::to_string(key).expect("serializing a key cannot fail"));
                output.push(':');
                write_value(&values[key], output);
            }
            output.push('}');
        }
    }
}

pub(crate) fn sha256_hex(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn object_key_order_does_not_change_hash_input() {
        assert_eq!(
            canonical_json(&json!({"b": 2, "a": {"d": 4, "c": 3}})),
            canonical_json(&json!({"a": {"c": 3, "d": 4}, "b": 2}))
        );
    }

    #[test]
    fn a004_complete_item_fixture_has_cross_language_canonical_hashes() {
        let fixture: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/concept-spec/fixtures/k001-a004-turn-compatibility.json"
        )))
        .unwrap();
        let golden = &fixture["canonical_golden"];
        let items = golden["items"].as_array().unwrap();
        let expected = golden["item_sha256"].as_array().unwrap();
        let hashes = items
            .iter()
            .map(|item| {
                serde_json::from_value::<forgecad_app_server_protocol::AgentItem>(item.clone())
                    .unwrap();
                sha256_hex(canonical_json(item).as_bytes())
            })
            .collect::<Vec<_>>();
        assert_eq!(
            hashes,
            expected
                .iter()
                .map(|value| value.as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        );
        let manifest = items
            .iter()
            .zip(&hashes)
            .map(|(item, hash)| {
                json!({
                    "sequence": item["sequence"].as_u64().unwrap(),
                    "item_sha256": hash,
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(
            sha256_hex(canonical_json(&json!(manifest)).as_bytes()),
            golden["turn_items_sha256"].as_str().unwrap()
        );
    }
}
