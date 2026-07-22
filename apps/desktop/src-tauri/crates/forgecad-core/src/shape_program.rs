use serde_json::Value;

use crate::{CoreError, CoreResult};

/// Normalizes a ShapeProgram at the JSON persistence boundary.
///
/// ShapeProgram identity is deliberately not changed here: the existing
/// semantic hash remains the authority. This helper only makes the value that
/// is compiled and persisted use the same serde JSON representation, then
/// proves that the normalized value is stable across one more persistence
/// round-trip.
pub fn normalize_persisted_shape_program(value: &Value) -> CoreResult<Value> {
    let serialized = serde_json::to_string(value).map_err(|_| {
        CoreError::invalid_data(
            "SHAPE_PROGRAM_NORMALIZATION_SERIALIZE_FAILED",
            "ShapeProgram could not be serialized for its persistence boundary.",
        )
    })?;
    let normalized = serde_json::from_str::<Value>(&serialized).map_err(|_| {
        CoreError::invalid_data(
            "SHAPE_PROGRAM_NORMALIZATION_PARSE_FAILED",
            "ShapeProgram could not be parsed after persistence serialization.",
        )
    })?;
    let normalized_roundtrip =
        serde_json::from_str::<Value>(&serde_json::to_string(&normalized).map_err(|_| {
            CoreError::invalid_data(
                "SHAPE_PROGRAM_NORMALIZATION_SERIALIZE_FAILED",
                "Normalized ShapeProgram could not be serialized for idempotence validation.",
            )
        })?)
        .map_err(|_| {
            CoreError::invalid_data(
                "SHAPE_PROGRAM_NORMALIZATION_PARSE_FAILED",
                "Normalized ShapeProgram could not be parsed for idempotence validation.",
            )
        })?;
    if normalized != normalized_roundtrip {
        return Err(CoreError::invalid_data(
            "SHAPE_PROGRAM_NORMALIZATION_NOT_IDEMPOTENT",
            "Normalized ShapeProgram changed across a second persistence round-trip.",
        ));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    fn json_type(value: &Value) -> &'static str {
        match value {
            Value::Null => "null",
            Value::Bool(_) => "bool",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        }
    }

    fn number_class(value: &Value) -> &'static str {
        value
            .as_number()
            .map(|number| {
                if number.is_i64() {
                    "signed_integer"
                } else if number.is_u64() {
                    "unsigned_integer"
                } else {
                    "float"
                }
            })
            .unwrap_or("not_number")
    }

    fn first_difference(before: &Value, after: &Value, path: &str) -> Option<String> {
        match (before, after) {
            (Value::Object(before), Value::Object(after)) => {
                let mut keys = before.keys().chain(after.keys()).collect::<Vec<_>>();
                keys.sort();
                keys.dedup();
                for key in keys {
                    let child_path = format!("{path}/{key}");
                    match (before.get(key), after.get(key)) {
                        (Some(left), Some(right)) => {
                            if let Some(difference) = first_difference(left, right, &child_path) {
                                return Some(difference);
                            }
                        }
                        (Some(left), None) | (None, Some(left)) => {
                            return Some(format!(
                                "path={child_path} before_type={} after_type={} number_class={}",
                                json_type(left),
                                if before.get(key).is_some() {
                                    "missing"
                                } else {
                                    json_type(after.get(key).expect("checked"))
                                },
                                number_class(left),
                            ));
                        }
                        (None, None) => unreachable!("key came from one of the objects"),
                    }
                }
                None
            }
            (Value::Array(before), Value::Array(after)) => {
                if before.len() != after.len() {
                    return Some(format!(
                        "path={path} before_type=array after_type=array number_class=length"
                    ));
                }
                before
                    .iter()
                    .zip(after)
                    .enumerate()
                    .find_map(|(index, (left, right))| {
                        first_difference(left, right, &format!("{path}/{index}"))
                    })
            }
            (Value::Number(before), Value::Number(after)) if before != after => Some(format!(
                "path={path} before_type=number after_type=number number_class={}/{}",
                number_class(&Value::Number(before.clone())),
                number_class(&Value::Number(after.clone())),
            )),
            (before, after) if before != after => Some(format!(
                "path={path} before_type={} after_type={} number_class={}/{}",
                json_type(before),
                json_type(after),
                number_class(before),
                number_class(after),
            )),
            _ => None,
        }
    }

    #[test]
    fn k001_set_part_parameter_shape_program_persistence_normalization_is_idempotent() {
        // This is the packaged K001 set_part_parameter shape fixture: the
        // parameter edit replaces the x-axis size before preview compilation.
        let input = serde_json::json!({
            "schema_version": "ShapeProgram@1",
            "program_id": "shape_k001_packaged_parameter",
            "units": "millimeter",
            "seed": 17,
            "triangle_budget": 1000,
            "parameters": [{
                "parameter_id": "editparam_part_primary_scale_x",
                "path": "transform.scale.x",
                "default": 1.0,
                "min": 0.6,
                "max": 1.4,
                "step": 0.1,
                "unit": "ratio"
            }],
            "operations": [{
                "operation_id": "op_primary_shell",
                "op": "box",
                "inputs": [],
                "args": {
                    "size": [120.0, 40.0, 20.0],
                    "position": [0.0, 0.0, 0.0],
                    "rotation": [0.0, 0.0, 0.0],
                    "part_role": "primary_form",
                    "zone_id": "zone_primary",
                    "material_id": "mat_graphite"
                }
            }],
            "outputs": [{
                "output_id": "output_primary_shell",
                "operation_id": "op_primary_shell",
                "kind": "mesh",
                "part_role": "primary_form"
            }],
            "non_functional_only": true
        });
        let normalized = normalize_persisted_shape_program(&input).unwrap();
        let reparsed = normalize_persisted_shape_program(&normalized).unwrap();
        assert_eq!(normalized, reparsed);
        assert_eq!(
            first_difference(&normalized, &reparsed, "$/shape_program"),
            None
        );
    }

    #[test]
    fn normalized_shape_program_preserves_number_contract_and_is_idempotent() {
        let input = serde_json::json!({
            "integer": 1,
            "float": 1.0,
            "negative_zero": -0.0,
            "exponent": 1e-3,
        });
        let normalized = normalize_persisted_shape_program(&input).unwrap();
        let reparsed = normalize_persisted_shape_program(&normalized).unwrap();
        assert_eq!(normalized, reparsed);
        assert_eq!(normalized["integer"].as_i64(), Some(1));
        assert_eq!(normalized["float"].as_f64(), Some(1.0));
        assert_eq!(normalized["negative_zero"].as_f64(), Some(-0.0));
        assert_eq!(normalized["exponent"].as_f64(), Some(1e-3));
    }
}
