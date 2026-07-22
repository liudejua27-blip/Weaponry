//! Rust-owned geometry-family lowering for `ArmDesignIntent@1`.
//!
//! This is deliberately a bounded compiler pass, not a free-form mesh API.
//! The intent selects reviewed changes to an already reviewed serial-chain
//! Recipe expansion.  It changes the ShapeProgram and its AssemblyGraph
//! together, preserving operation/output/part identity and never accepting
//! dimensions, code, paths, or arbitrary operations from a Provider.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{lower_arm_design_intent, semantic_sha256, ArmDesignIntent, CoreError, CoreResult};

pub const ARM_GEOMETRY_FAMILY_SCHEMA_VERSION: &str = "ArmGeometryFamily@1";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArmGeometryFamilyBinding {
    pub schema_version: String,
    pub family_id: String,
    pub architecture: String,
    pub intent_sha256: String,
    pub changed_operation_count: u32,
    pub changed_part_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape_program_sha256: Option<String>,
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("ARM_GEOMETRY_FAMILY_INVALID", message)
}

fn positive_number(value: &mut Value, factor: f64) -> bool {
    let Some(current) = value.as_f64() else {
        return false;
    };
    let next = current * factor;
    if !next.is_finite() || next <= 0.0 {
        return false;
    }
    *value = json!(next);
    true
}

fn signed_number(value: &mut Value, factor: f64) -> bool {
    let Some(current) = value.as_f64() else {
        return false;
    };
    let next = current * factor;
    if !next.is_finite() {
        return false;
    }
    *value = json!(next);
    true
}

fn positive_array_factor(value: &mut Value, axis: usize, factor: f64) -> bool {
    let Some(values) = value.as_array_mut() else {
        return false;
    };
    let Some(item) = values.get_mut(axis) else {
        return false;
    };
    positive_number(item, factor)
}

fn existing_positive(args: &mut Map<String, Value>, key: &str, factor: f64) -> bool {
    args.get_mut(key)
        .is_some_and(|value| positive_number(value, factor))
}

fn existing_positive_array(
    args: &mut Map<String, Value>,
    key: &str,
    axis: usize,
    factor: f64,
) -> bool {
    args.get_mut(key)
        .is_some_and(|value| positive_array_factor(value, axis, factor))
}

fn bounded_factor(value: &str, table: &[(&str, f64)], default: f64) -> f64 {
    table
        .iter()
        .find_map(|(key, factor)| (*key == value).then_some(*factor))
        .unwrap_or(default)
}

fn pose_angle(pose: &str) -> f64 {
    match pose {
        "neutral" => 0.0,
        "grounded" => -0.08,
        "elevated" => 0.14,
        "extended" => -0.16,
        "folded" => 0.32,
        _ => 0.0,
    }
}

fn operation_text(operation: &Map<String, Value>) -> String {
    operation
        .get("operation_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn role(operation: &Map<String, Value>) -> &str {
    operation
        .get("args")
        .and_then(Value::as_object)
        .and_then(|args| args.get("part_role"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn set_rotation(args: &mut Map<String, Value>, angle: f64) -> bool {
    let rotation = args
        .entry("rotation")
        .or_insert_with(|| json!([0.0, 0.0, 0.0]));
    let Some(values) = rotation.as_array_mut() else {
        return false;
    };
    let Some(value) = values.get_mut(2) else {
        return false;
    };
    let Some(current) = value.as_f64() else {
        return false;
    };
    let next = current + angle;
    if !next.is_finite() {
        return false;
    }
    *value = json!(next);
    true
}

fn material_for_palette(current: &str, palette: &str) -> String {
    match palette {
        // Keep the reviewed material vocabulary.  A palette changes the
        // existing zone assignment only; it cannot invent a Provider ID.
        "white_aluminum" if current == "mat_graphite" => "mat_aluminum".into(),
        "monochrome_technical" if current == "mat_emissive_blue" => "mat_graphite".into(),
        "industrial_yellow" if current == "mat_graphite" => "mat_aluminum".into(),
        "warm_copper" if current == "mat_graphite" => "mat_aluminum".into(),
        _ => current.to_owned(),
    }
}

/// Apply the reviewed serial-chain geometry family to an expanded Recipe.
pub fn apply_serial_chain_geometry_family(
    intent_value: &Value,
    shape_program: &mut Value,
    assembly_graph: &mut Value,
) -> CoreResult<ArmGeometryFamilyBinding> {
    apply_arm_geometry_family(intent_value, shape_program, assembly_graph)
}

/// Apply a reviewed serial-chain or parallel-link geometry family to an
/// expanded Recipe. Parallel-link is a bounded visual layout family with its
/// own C110G Recipe catalog; it is not an engineering kinematics claim.
pub fn apply_arm_geometry_family(
    intent_value: &Value,
    shape_program: &mut Value,
    assembly_graph: &mut Value,
) -> CoreResult<ArmGeometryFamilyBinding> {
    let intent: ArmDesignIntent = serde_json::from_value(intent_value.clone())
        .map_err(|error| invalid(format!("ArmDesignIntent@1 failed closed: {error}")))?;
    let lowering = lower_arm_design_intent(intent_value)?;
    if lowering.status != "lowered"
        || !matches!(
            intent.architecture.as_str(),
            "serial_chain" | "parallel_link"
        )
    {
        return Err(invalid(
            "Only reviewed serial_chain and parallel_link geometry families are currently available",
        ));
    }
    if shape_program.get("schema_version").and_then(Value::as_str) != Some("ShapeProgram@1") {
        return Err(invalid("ShapeProgram@1 is required"));
    }

    let operations = shape_program
        .get_mut("operations")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ShapeProgram operations are missing"))?;
    let link_cross_section = match intent.link_language.as_str() {
        "closed_shell" => (1.0, 1.0),
        "twin_rail" => (0.82, 1.14),
        "open_truss" => (0.68, 1.24),
        "tapered_loft" => (1.12, 0.86),
        "tube_frame" => (0.74, 1.08),
        _ => unreachable!("validated ArmDesignIntent link language"),
    };
    let joint_factor = bounded_factor(
        &intent.joint_language,
        &[
            ("armored_bearing", 1.05),
            ("exposed_ring", 1.16),
            ("gimbal_shell", 0.94),
            ("capsule_joint", 1.0),
            ("bellows_joint", 1.10),
        ],
        1.0,
    );
    let base_factor = bounded_factor(
        &intent.base_language,
        &[
            ("round_turntable", 1.0),
            ("hex_platform", 1.10),
            ("floating_pedestal", 0.90),
            ("industrial_deck", 1.20),
            ("compact_puck", 0.84),
        ],
        1.0,
    );
    let wrist_factor = bounded_factor(
        &intent.wrist_language,
        &[
            ("layered_wrist", 1.05),
            ("gimbal_wrist", 0.96),
            ("cylindrical_wrist", 0.90),
            ("fork_wrist", 1.14),
        ],
        1.0,
    );
    let end_factor = bounded_factor(
        &intent.end_effector_language,
        &[
            ("parallel_gripper", 1.0),
            ("adaptive_claw", 1.12),
            ("precision_tool", 0.86),
            ("sensor_probe", 0.76),
            ("soft_pad_gripper", 1.16),
        ],
        1.0,
    );
    let proportion_factor = bounded_factor(
        &intent.proportion_profile,
        &[
            ("compact", 0.86),
            ("balanced", 1.0),
            ("long_reach", 1.16),
            ("heavy_base", 0.96),
            ("slender", 1.08),
        ],
        1.0,
    );
    let cable_factor = bounded_factor(
        &intent.cable_language,
        &[
            ("internal_routing", 0.72),
            ("braided_external", 1.16),
            ("armored_harness", 1.06),
            ("minimal_cable", 0.54),
        ],
        1.0,
    );
    let angle = pose_angle(&intent.pose)
        + if intent.architecture == "parallel_link" {
            0.18
        } else {
            0.0
        };
    let mut changed_operations = 0_u32;

    for value in operations {
        let Some(operation) = value.as_object_mut() else {
            return Err(invalid("ShapeProgram operation must be an object"));
        };
        let operation_id = operation_text(operation);
        let part_role = role(operation).to_owned();
        // Rotation belongs on the source primitive.  The restricted worker
        // deliberately rejects a second transform on derived mesh nodes
        // (bevel/surface/array), so an intent must not leak a rotation onto
        // those downstream operations.
        let is_source_operation = operation
            .get("inputs")
            .and_then(Value::as_array)
            .map_or(true, |inputs| inputs.is_empty());
        let Some(args) = operation.get_mut("args").and_then(Value::as_object_mut) else {
            continue;
        };
        let mut changed = false;
        if part_role == "upper_link_form"
            || part_role == "lower_link_form"
            || operation_id.contains("link")
        {
            changed |= existing_positive_array(args, "size", 0, proportion_factor);
            changed |= existing_positive_array(args, "size", 1, link_cross_section.0);
            changed |= existing_positive_array(args, "size", 2, link_cross_section.1);
            changed |=
                existing_positive_array(args, "cross_section_scale", 0, link_cross_section.0);
            changed |=
                existing_positive_array(args, "cross_section_scale", 1, link_cross_section.1);
            changed |= existing_positive(args, "axis_length", proportion_factor);
            if is_source_operation {
                changed |= set_rotation(args, angle);
            }
            if intent.architecture == "parallel_link" {
                // Mirror the two reviewed link components around the carrier
                // plane. This is deliberately bounded and role-aware.
                changed |= existing_positive_array(args, "size", 2, 1.18);
                if let Some(position) = args.get_mut("position").and_then(Value::as_array_mut) {
                    if let Some(z) = position.get_mut(2) {
                        changed |= signed_number(
                            z,
                            if operation_id.contains("upper") {
                                -1.0
                            } else {
                                1.0
                            },
                        );
                    }
                }
            }
        }
        if operation_id.contains("collar")
            || operation_id.contains("joint")
            || part_role == "joint_housing"
        {
            changed |= existing_positive_array(args, "size", 0, joint_factor);
            changed |= existing_positive_array(args, "size", 1, joint_factor);
            changed |= existing_positive_array(args, "size", 2, joint_factor);
            changed |= existing_positive(args, "radius", joint_factor);
            changed |= existing_positive(args, "height", joint_factor);
        }
        if part_role == "base_form"
            || operation_id.contains("plinth")
            || operation_id.contains("turntable")
        {
            changed |= existing_positive_array(args, "size", 0, base_factor);
            changed |= existing_positive_array(args, "size", 1, base_factor);
            changed |= existing_positive_array(args, "size", 2, base_factor);
            changed |= existing_positive(args, "radius", base_factor);
            changed |= existing_positive(args, "height", base_factor);
            if let Some(points) = args.get_mut("points").and_then(Value::as_array_mut) {
                for point in points {
                    if let Some(coords) = point.as_array_mut() {
                        if let Some(y) = coords.get_mut(1) {
                            changed |= signed_number(y, base_factor);
                        }
                    }
                }
            }
        }
        if operation_id.contains("wrist") || part_role == "wrist_form" {
            changed |= existing_positive_array(args, "size", 0, wrist_factor);
            changed |= existing_positive_array(args, "size", 2, wrist_factor);
            changed |= existing_positive(args, "radius", wrist_factor);
            changed |= existing_positive(args, "height", wrist_factor);
        }
        if operation_id.contains("head_tool")
            || operation_id.contains("gripper")
            || part_role == "end_effector_form"
        {
            changed |= existing_positive_array(args, "size", 0, end_factor);
            changed |= existing_positive_array(args, "size", 2, end_factor);
            changed |= existing_positive(args, "radius", end_factor);
            changed |= existing_positive(args, "height", end_factor);
        }
        if operation_id.contains("cable") || part_role == "cable_harness" {
            changed |= existing_positive_array(args, "profile_scale", 0, cable_factor);
            changed |= existing_positive_array(args, "profile_scale", 1, cable_factor);
            if let Some(points) = args.get_mut("path_points").and_then(Value::as_array_mut) {
                for point in points {
                    if let Some(coords) = point.as_array_mut() {
                        if let Some(z) = coords.get_mut(2) {
                            changed |= signed_number(z, cable_factor);
                        }
                    }
                }
            }
        }
        if let Some(material) = args.get("material_id").and_then(Value::as_str) {
            let mapped = material_for_palette(material, &intent.material_palette);
            if mapped != material {
                args.insert("material_id".into(), Value::String(mapped));
                changed = true;
            }
        }
        if changed {
            changed_operations = changed_operations.saturating_add(1);
        }
    }

    let parts = assembly_graph
        .get_mut("parts")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("AssemblyGraph parts are missing"))?;
    let mut changed_part_count = 0_u32;
    for part in parts {
        let Some(part) = part.as_object_mut() else {
            return Err(invalid("AssemblyGraph part must be an object"));
        };
        let operation_id = part
            .get("operation_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        let mut changed = false;
        let part_role = part
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let transform = part.entry("transform").or_insert_with(
            || json!({"position":[0.0,0.0,0.0],"rotation":[0.0,0.0,0.0],"scale":[1.0,1.0,1.0]}),
        );
        let Some(transform) = transform.as_object_mut() else {
            return Err(invalid("AssemblyGraph transform must be an object"));
        };
        if part_role == "upper_link_form"
            || part_role == "lower_link_form"
            || operation_id.contains("link")
        {
            changed |= existing_positive_array(transform, "scale", 0, proportion_factor);
            changed |= existing_positive_array(transform, "scale", 1, link_cross_section.0);
            changed |= existing_positive_array(transform, "scale", 2, link_cross_section.1);
            changed |= set_rotation(transform, angle);
            if intent.architecture == "parallel_link" {
                changed |= existing_positive_array(transform, "scale", 2, 1.18);
                if let Some(position) = transform.get_mut("position").and_then(Value::as_array_mut)
                {
                    if let Some(z) = position.get_mut(2) {
                        changed |= signed_number(
                            z,
                            if operation_id.contains("upper") {
                                -1.0
                            } else {
                                1.0
                            },
                        );
                    }
                }
            }
        }
        if part_role == "base_form" || operation_id.contains("base_") {
            changed |= existing_positive_array(transform, "scale", 0, base_factor);
            changed |= existing_positive_array(transform, "scale", 2, base_factor);
        }
        if part_role == "end_effector_form" || operation_id.contains("gripper") {
            changed |= existing_positive_array(transform, "scale", 0, end_factor);
            changed |= existing_positive_array(transform, "scale", 2, end_factor);
        }
        if changed {
            changed_part_count = changed_part_count.saturating_add(1);
        }
    }

    if changed_operations == 0 || changed_part_count == 0 {
        return Err(invalid(
            "ArmDesignIntent did not bind to any reviewed geometry operation or part",
        ));
    }
    let intent_sha256 = semantic_sha256(&intent)?;
    Ok(ArmGeometryFamilyBinding {
        schema_version: ARM_GEOMETRY_FAMILY_SCHEMA_VERSION.into(),
        family_id: if intent.architecture == "parallel_link" {
            "robotic_arm.parallel_link.c110g_v1".into()
        } else {
            "robotic_arm.serial_chain.reviewed_v1".into()
        },
        architecture: intent.architecture,
        intent_sha256,
        changed_operation_count: changed_operations,
        changed_part_count,
        shape_program_sha256: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(link_language: &str) -> Value {
        json!({
            "schema_version": "ArmDesignIntent@1",
            "domain_pack_id": "pack_robotic_arm_concept",
            "architecture": "serial_chain",
            "joint_language": "exposed_ring",
            "link_language": link_language,
            "base_language": "hex_platform",
            "wrist_language": "fork_wrist",
            "end_effector_language": "adaptive_claw",
            "cable_language": "braided_external",
            "surface_language": ["panel_seams"],
            "material_palette": "white_aluminum",
            "detail_density": "dense",
            "pose": "extended",
            "proportion_profile": "long_reach",
            "style_keywords": ["mechanical"],
            "source": "user_brief",
            "visual_only": true
        })
    }

    fn geometry() -> (Value, Value) {
        (
            json!({
                "schema_version":"ShapeProgram@1",
                "operations":[
                    {"operation_id":"op_lower_link","op":"loft","args":{"part_role":"upper_link_form","axis_length":100.0,"cross_section_scale":[10.0,20.0],"material_id":"mat_graphite"}},
                    {"operation_id":"op_base_turntable","op":"cylinder","args":{"part_role":"base_form","radius":20.0,"height":4.0,"material_id":"mat_graphite"}},
                    {"operation_id":"op_wrist_base","op":"box","args":{"part_role":"visual_detail","size":[10.0,10.0,10.0],"material_id":"mat_graphite"}},
                    {"operation_id":"op_trim_cable","op":"sweep","args":{"part_role":"visual_detail","profile_scale":[2.0,1.0],"path_points":[[0.0,0.0,1.0],[1.0,1.0,2.0]],"material_id":"mat_graphite"}}
                ]
            }),
            json!({"schema_version":"AssemblyGraph@1","parts":[
                {"operation_id":"op_lower_link","role":"upper_link_form","transform":{"scale":[1.0,1.0,1.0],"rotation":[0.0,0.0,0.0]}},
                {"operation_id":"op_base_turntable","role":"base_form","transform":{"scale":[1.0,1.0,1.0],"rotation":[0.0,0.0,0.0]}}
            ]}),
        )
    }

    #[test]
    fn intent_changes_shape_and_assembly_together() {
        let (mut shape, mut graph) = geometry();
        let binding =
            apply_serial_chain_geometry_family(&intent("twin_rail"), &mut shape, &mut graph)
                .unwrap();
        assert_eq!(binding.family_id, "robotic_arm.serial_chain.reviewed_v1");
        assert!(binding.changed_operation_count >= 3);
        assert!(binding.changed_part_count >= 2);
        assert_ne!(shape["operations"][0]["args"]["axis_length"], json!(100.0));
        assert_ne!(
            graph["parts"][0]["transform"]["rotation"],
            json!([0.0, 0.0, 0.0])
        );
        assert_eq!(
            shape["operations"][1]["args"]["material_id"],
            "mat_aluminum"
        );
    }

    #[test]
    fn different_link_language_changes_the_shape_fingerprint() {
        let (mut closed_shape, mut closed_graph) = geometry();
        let (mut truss_shape, mut truss_graph) = geometry();
        apply_serial_chain_geometry_family(
            &intent("closed_shell"),
            &mut closed_shape,
            &mut closed_graph,
        )
        .unwrap();
        apply_serial_chain_geometry_family(
            &intent("open_truss"),
            &mut truss_shape,
            &mut truss_graph,
        )
        .unwrap();
        assert_ne!(closed_shape, truss_shape);
        assert_ne!(closed_graph, truss_graph);
    }

    #[test]
    fn non_serial_architecture_fails_closed() {
        let mut value = intent("closed_shell");
        value["architecture"] = json!("scara");
        let (mut shape, mut graph) = geometry();
        let error = apply_serial_chain_geometry_family(&value, &mut shape, &mut graph).unwrap_err();
        assert_eq!(error.code(), "ARM_GEOMETRY_FAMILY_INVALID");
    }

    #[test]
    fn parallel_link_changes_reviewed_shape_and_assembly() {
        let mut value = intent("closed_shell");
        value["architecture"] = json!("parallel_link");
        let (mut shape, mut graph) = geometry();
        let binding = apply_arm_geometry_family(&value, &mut shape, &mut graph).unwrap();
        assert_eq!(binding.family_id, "robotic_arm.parallel_link.c110g_v1");
        assert!(binding.changed_operation_count >= 3);
        assert!(binding.changed_part_count >= 2);
        assert_ne!(shape["operations"][0]["args"]["rotation"], json!(null));
        assert_ne!(
            graph["parts"][0]["transform"]["rotation"],
            json!([0.0, 0.0, 0.0])
        );
    }
}
