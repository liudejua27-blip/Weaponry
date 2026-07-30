//! FGC-VP204 bounded patch and semantic incremental invalidation for VP203.
//!
//! A patch addresses stable typed IDs only. It cannot replace the full graph,
//! append executable operations, rename nodes, or carry JSON paths/code.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    lower_forge_visual_geometry_program_v2, semantic_sha256, CoreError, CoreResult,
    ForgeVisualGeometryLoweringV2, ForgeVisualGeometryProgramV2, HighLevelGeometryNodeV2,
};

pub const FORGE_VISUAL_GEOMETRY_PATCH_SCHEMA_VERSION: &str = "ForgeVisualGeometryPatch@1";
pub const GEOMETRY_INCREMENTAL_PLAN_SCHEMA_VERSION: &str = "GeometryIncrementalPlan@1";

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message.into())
}

fn require_id(value: &str, prefix: &str) -> CoreResult<()> {
    if value.starts_with(prefix)
        && value.len() > prefix.len()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
    {
        Ok(())
    } else {
        Err(invalid(
            "FORGE_VISUAL_VP204_ID_INVALID",
            format!("ID must match the bounded {prefix} set"),
        ))
    }
}

fn require_hash(value: &str) -> CoreResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(invalid(
            "FORGE_VISUAL_VP204_HASH_INVALID",
            "expected_source_sha256 must be a lowercase SHA-256",
        ))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ForgeVisualGeometryPatchOperationV2 {
    SetNodePosition {
        node_id: String,
        position: [f64; 3],
    },
    SetExtrudeHeight {
        node_id: String,
        height: f64,
    },
    SetRevolveAngle {
        node_id: String,
        angle: f64,
    },
    SetLoftAxisLength {
        node_id: String,
        axis_length: f64,
    },
    SetSweepProfileScale {
        node_id: String,
        profile_scale: [f64; 2],
    },
    SetArray {
        node_id: String,
        count: u16,
        spacing: f64,
    },
    SetMaterialBase {
        material_id: String,
        base_material_id: String,
    },
}

impl ForgeVisualGeometryPatchOperationV2 {
    fn target_id(&self) -> &str {
        match self {
            Self::SetNodePosition { node_id, .. }
            | Self::SetExtrudeHeight { node_id, .. }
            | Self::SetRevolveAngle { node_id, .. }
            | Self::SetLoftAxisLength { node_id, .. }
            | Self::SetSweepProfileScale { node_id, .. }
            | Self::SetArray { node_id, .. } => node_id,
            Self::SetMaterialBase { material_id, .. } => material_id,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualGeometryPatchV2 {
    pub schema_version: String,
    pub patch_id: String,
    pub expected_source_sha256: String,
    pub operations: Vec<ForgeVisualGeometryPatchOperationV2>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GeometryIncrementalPlanV2 {
    pub schema_version: String,
    pub base_source_sha256: String,
    pub patched_source_sha256: String,
    pub patch_sha256: String,
    pub reused_profile_input_ids: Vec<String>,
    pub invalidated_profile_input_ids: Vec<String>,
    pub reused_source_node_ids: Vec<String>,
    pub invalidated_source_node_ids: Vec<String>,
    pub reused_shape_operation_ids: Vec<String>,
    pub invalidated_shape_operation_ids: Vec<String>,
    pub reused_output_ids: Vec<String>,
    pub invalidated_output_ids: Vec<String>,
    pub full_compile_cache_hit: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PatchedVisualGeometryProgramV2 {
    pub patched_program: Value,
    pub lowering: ForgeVisualGeometryLoweringV2,
    pub incremental_plan: GeometryIncrementalPlanV2,
}

fn node_inputs(node: &HighLevelGeometryNodeV2) -> Vec<&str> {
    match node {
        HighLevelGeometryNodeV2::Mirror { input_node_id, .. }
        | HighLevelGeometryNodeV2::Array { input_node_id, .. }
        | HighLevelGeometryNodeV2::RadialArray { input_node_id, .. }
        | HighLevelGeometryNodeV2::BevelApprox { input_node_id, .. }
        | HighLevelGeometryNodeV2::SurfacePanel { input_node_id, .. }
        | HighLevelGeometryNodeV2::LatticeDeform { input_node_id, .. }
        | HighLevelGeometryNodeV2::Part { input_node_id, .. }
        | HighLevelGeometryNodeV2::MaterialZone { input_node_id, .. } => vec![input_node_id],
        HighLevelGeometryNodeV2::Union { input_node_ids, .. }
        | HighLevelGeometryNodeV2::Subtract { input_node_ids, .. } => {
            input_node_ids.iter().map(String::as_str).collect()
        }
        _ => Vec::new(),
    }
}

fn node_id(node: &HighLevelGeometryNodeV2) -> &str {
    match node {
        HighLevelGeometryNodeV2::Box { node_id, .. }
        | HighLevelGeometryNodeV2::Cylinder { node_id, .. }
        | HighLevelGeometryNodeV2::Capsule { node_id, .. }
        | HighLevelGeometryNodeV2::Wedge { node_id, .. }
        | HighLevelGeometryNodeV2::Extrude { node_id, .. }
        | HighLevelGeometryNodeV2::Revolve { node_id, .. }
        | HighLevelGeometryNodeV2::Loft { node_id, .. }
        | HighLevelGeometryNodeV2::Sweep { node_id, .. }
        | HighLevelGeometryNodeV2::Mirror { node_id, .. }
        | HighLevelGeometryNodeV2::Array { node_id, .. }
        | HighLevelGeometryNodeV2::RadialArray { node_id, .. }
        | HighLevelGeometryNodeV2::BevelApprox { node_id, .. }
        | HighLevelGeometryNodeV2::SurfacePanel { node_id, .. }
        | HighLevelGeometryNodeV2::LatticeDeform { node_id, .. }
        | HighLevelGeometryNodeV2::Union { node_id, .. }
        | HighLevelGeometryNodeV2::Subtract { node_id, .. }
        | HighLevelGeometryNodeV2::Part { node_id, .. }
        | HighLevelGeometryNodeV2::MaterialZone { node_id, .. } => node_id,
    }
}

fn node_fingerprints(
    program: &ForgeVisualGeometryProgramV2,
) -> CoreResult<BTreeMap<String, String>> {
    let profile_hashes = program
        .profiles
        .iter()
        .map(|profile| Ok((profile.profile_id.clone(), semantic_sha256(profile)?)))
        .collect::<CoreResult<BTreeMap<_, _>>>()?;
    let section_hashes = program
        .section_sets
        .iter()
        .map(|set| {
            let dependencies = set
                .sections
                .iter()
                .map(|section| profile_hashes[&section.profile_id].clone())
                .collect::<Vec<_>>();
            Ok((
                set.section_set_id.clone(),
                semantic_sha256(&json!({"section_set": set, "profiles": dependencies}))?,
            ))
        })
        .collect::<CoreResult<BTreeMap<_, _>>>()?;
    let material_hashes = program
        .materials
        .iter()
        .map(|material| Ok((material.material_id.clone(), semantic_sha256(material)?)))
        .collect::<CoreResult<BTreeMap<_, _>>>()?;
    let mut result = BTreeMap::<String, String>::new();
    for node in &program.nodes {
        let dependency_hashes = node_inputs(node)
            .iter()
            .map(|input| result[*input].clone())
            .collect::<Vec<_>>();
        let resource_hashes = match node {
            HighLevelGeometryNodeV2::Extrude { profile_id, .. }
            | HighLevelGeometryNodeV2::Revolve { profile_id, .. }
            | HighLevelGeometryNodeV2::Sweep { profile_id, .. } => {
                vec![profile_hashes[profile_id].clone()]
            }
            HighLevelGeometryNodeV2::Loft { section_set_id, .. } => {
                vec![section_hashes[section_set_id].clone()]
            }
            HighLevelGeometryNodeV2::MaterialZone { material_id, .. } => {
                vec![material_hashes[material_id].clone()]
            }
            _ => Vec::new(),
        };
        result.insert(
            node_id(node).to_string(),
            semantic_sha256(&json!({
                "node": node,
                "dependencies": dependency_hashes,
                "resources": resource_hashes,
            }))?,
        );
    }
    Ok(result)
}

fn item_hashes(items: &[Value], id_field: &str) -> CoreResult<BTreeMap<String, String>> {
    items
        .iter()
        .map(|item| {
            let id = item.get(id_field).and_then(Value::as_str).ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP204_LOWERING_INVALID",
                    "lowered item ID is missing",
                )
            })?;
            Ok((id.to_string(), semantic_sha256(item)?))
        })
        .collect()
}

fn output_fingerprints(
    outputs: &[Value],
    operation_hashes: &BTreeMap<String, String>,
) -> CoreResult<BTreeMap<String, String>> {
    outputs
        .iter()
        .map(|output| {
            let output_id = output
                .get("output_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_VP204_LOWERING_INVALID",
                        "lowered output ID is missing",
                    )
                })?;
            let operation_id = output
                .get("operation_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_VP204_LOWERING_INVALID",
                        "lowered output operation binding is missing",
                    )
                })?;
            let operation_sha256 = operation_hashes.get(operation_id).ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP204_LOWERING_INVALID",
                    "lowered output operation binding is unresolved",
                )
            })?;
            Ok((
                output_id.to_string(),
                semantic_sha256(&json!({
                    "output": output,
                    "operation_sha256": operation_sha256,
                }))?,
            ))
        })
        .collect()
}

fn split_reuse(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> (Vec<String>, Vec<String>) {
    let mut reused = Vec::new();
    let mut invalidated = Vec::new();
    for (id, hash) in after {
        if before.get(id) == Some(hash) {
            reused.push(id.clone());
        } else {
            invalidated.push(id.clone());
        }
    }
    (reused, invalidated)
}

fn apply_operation(
    program: &mut ForgeVisualGeometryProgramV2,
    operation: &ForgeVisualGeometryPatchOperationV2,
) -> CoreResult<()> {
    match operation {
        ForgeVisualGeometryPatchOperationV2::SetMaterialBase {
            material_id,
            base_material_id,
        } => {
            require_id(material_id, "mat_")?;
            require_id(base_material_id, "mat_")?;
            let material = program
                .materials
                .iter_mut()
                .find(|material| material.material_id == *material_id)
                .ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_VP204_PATCH_TARGET_MISSING",
                        "material target is missing",
                    )
                })?;
            material.base_material_id = base_material_id.clone();
        }
        _ => {
            let target = operation.target_id();
            require_id(target, "node_")?;
            let node = program
                .nodes
                .iter_mut()
                .find(|node| node_id(node) == target)
                .ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_VP204_PATCH_TARGET_MISSING",
                        "node target is missing",
                    )
                })?;
            match (operation, node) {
                (
                    ForgeVisualGeometryPatchOperationV2::SetNodePosition { position, .. },
                    HighLevelGeometryNodeV2::Box {
                        position: current, ..
                    },
                )
                | (
                    ForgeVisualGeometryPatchOperationV2::SetNodePosition { position, .. },
                    HighLevelGeometryNodeV2::Extrude {
                        position: current, ..
                    },
                )
                | (
                    ForgeVisualGeometryPatchOperationV2::SetNodePosition { position, .. },
                    HighLevelGeometryNodeV2::Revolve {
                        position: current, ..
                    },
                )
                | (
                    ForgeVisualGeometryPatchOperationV2::SetNodePosition { position, .. },
                    HighLevelGeometryNodeV2::Loft {
                        position: current, ..
                    },
                )
                | (
                    ForgeVisualGeometryPatchOperationV2::SetNodePosition { position, .. },
                    HighLevelGeometryNodeV2::Sweep {
                        position: current, ..
                    },
                ) => *current = *position,
                (
                    ForgeVisualGeometryPatchOperationV2::SetExtrudeHeight { height, .. },
                    HighLevelGeometryNodeV2::Extrude {
                        height: current, ..
                    },
                ) => *current = *height,
                (
                    ForgeVisualGeometryPatchOperationV2::SetRevolveAngle { angle, .. },
                    HighLevelGeometryNodeV2::Revolve { angle: current, .. },
                ) => *current = *angle,
                (
                    ForgeVisualGeometryPatchOperationV2::SetLoftAxisLength { axis_length, .. },
                    HighLevelGeometryNodeV2::Loft {
                        axis_length: current,
                        ..
                    },
                ) => *current = *axis_length,
                (
                    ForgeVisualGeometryPatchOperationV2::SetSweepProfileScale {
                        profile_scale, ..
                    },
                    HighLevelGeometryNodeV2::Sweep {
                        profile_scale: current,
                        ..
                    },
                ) => *current = *profile_scale,
                (
                    ForgeVisualGeometryPatchOperationV2::SetArray { count, spacing, .. },
                    HighLevelGeometryNodeV2::Array {
                        count: current_count,
                        spacing: current_spacing,
                        ..
                    },
                ) => {
                    *current_count = *count;
                    *current_spacing = *spacing;
                }
                _ => {
                    return Err(invalid(
                        "FORGE_VISUAL_VP204_PATCH_TYPE_MISMATCH",
                        "typed patch operation does not match its target node",
                    ))
                }
            }
        }
    }
    Ok(())
}

pub fn apply_forge_visual_geometry_patch_v2(
    source: &Value,
    patch_value: &Value,
) -> CoreResult<PatchedVisualGeometryProgramV2> {
    let (base, _) = ForgeVisualGeometryProgramV2::parse_and_validate(source)?;
    let base_lowering = lower_forge_visual_geometry_program_v2(source)?;
    let patch: ForgeVisualGeometryPatchV2 =
        serde_json::from_value(patch_value.clone()).map_err(|error| {
            invalid(
                "FORGE_VISUAL_VP204_PATCH_PARSE_FAILED",
                format!("typed patch failed closed: {error}"),
            )
        })?;
    if patch.schema_version != FORGE_VISUAL_GEOMETRY_PATCH_SCHEMA_VERSION {
        return Err(invalid(
            "FORGE_VISUAL_VP204_PATCH_SCHEMA_INVALID",
            "patch schema version is unsupported",
        ));
    }
    require_id(&patch.patch_id, "patch_")?;
    require_hash(&patch.expected_source_sha256)?;
    if patch.expected_source_sha256 != base_lowering.source_program_sha256 {
        return Err(CoreError::conflict(
            "FORGE_VISUAL_VP204_PATCH_STALE",
            "typed patch expected source hash does not match the active source",
        ));
    }
    if patch.operations.is_empty() || patch.operations.len() > 8 {
        return Err(invalid(
            "FORGE_VISUAL_VP204_PATCH_BOUNDS",
            "patch must contain 1..=8 typed operations",
        ));
    }
    let mut targets = BTreeSet::new();
    for operation in &patch.operations {
        if !targets.insert(operation.target_id()) {
            return Err(invalid(
                "FORGE_VISUAL_VP204_PATCH_DUPLICATE_TARGET",
                "one patch may modify each stable target once",
            ));
        }
    }
    let mut patched = base.clone();
    for operation in &patch.operations {
        apply_operation(&mut patched, operation)?;
    }
    let patched_program = serde_json::to_value(&patched)
        .map_err(|error| invalid("JSON_SERIALIZATION_FAILED", error.to_string()))?;
    let lowering = lower_forge_visual_geometry_program_v2(&patched_program)?;
    if lowering.source_program_sha256 == base_lowering.source_program_sha256 {
        return Err(invalid(
            "FORGE_VISUAL_VP204_PATCH_NO_CHANGE",
            "typed patch made no semantic change",
        ));
    }

    let before_nodes = node_fingerprints(&base)?;
    let after_nodes = node_fingerprints(&patched)?;
    let (reused_source_node_ids, invalidated_source_node_ids) =
        split_reuse(&before_nodes, &after_nodes);
    let empty_before_profiles = Vec::new();
    let empty_after_profiles = Vec::new();
    let before_profiles = item_hashes(
        base_lowering.shape_program["profile_inputs"]
            .as_array()
            .unwrap_or(&empty_before_profiles),
        "input_id",
    )?;
    let after_profiles = item_hashes(
        lowering.shape_program["profile_inputs"]
            .as_array()
            .unwrap_or(&empty_after_profiles),
        "input_id",
    )?;
    let (reused_profile_input_ids, invalidated_profile_input_ids) =
        split_reuse(&before_profiles, &after_profiles);
    let before_operations = item_hashes(
        base_lowering.shape_program["operations"]
            .as_array()
            .ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP204_LOWERING_INVALID",
                    "base operations missing",
                )
            })?,
        "operation_id",
    )?;
    let after_operations = item_hashes(
        lowering.shape_program["operations"]
            .as_array()
            .ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP204_LOWERING_INVALID",
                    "patched operations missing",
                )
            })?,
        "operation_id",
    )?;
    let (reused_shape_operation_ids, invalidated_shape_operation_ids) =
        split_reuse(&before_operations, &after_operations);
    let before_outputs = output_fingerprints(
        base_lowering.shape_program["outputs"]
            .as_array()
            .ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP204_LOWERING_INVALID",
                    "base outputs missing",
                )
            })?,
        &before_operations,
    )?;
    let after_outputs = output_fingerprints(
        lowering.shape_program["outputs"]
            .as_array()
            .ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_VP204_LOWERING_INVALID",
                    "patched outputs missing",
                )
            })?,
        &after_operations,
    )?;
    let (reused_output_ids, invalidated_output_ids) = split_reuse(&before_outputs, &after_outputs);
    let patch_sha256 = semantic_sha256(&patch)?;
    Ok(PatchedVisualGeometryProgramV2 {
        patched_program,
        incremental_plan: GeometryIncrementalPlanV2 {
            schema_version: GEOMETRY_INCREMENTAL_PLAN_SCHEMA_VERSION.into(),
            base_source_sha256: base_lowering.source_program_sha256,
            patched_source_sha256: lowering.source_program_sha256.clone(),
            patch_sha256,
            reused_profile_input_ids,
            invalidated_profile_input_ids,
            reused_source_node_ids,
            invalidated_source_node_ids,
            reused_shape_operation_ids,
            invalidated_shape_operation_ids,
            reused_output_ids,
            invalidated_output_ids,
            full_compile_cache_hit: false,
        },
        lowering,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rotor() -> Value {
        serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/forge-visual-geometry-v2-rotor.json"
        ))
        .unwrap()
    }

    #[test]
    fn vp204_typed_patch_invalidates_only_changed_dependency_chain() {
        let source = rotor();
        let base = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let patch = json!({
            "schema_version": "ForgeVisualGeometryPatch@1",
            "patch_id": "patch_rotor_spacing",
            "expected_source_sha256": base.source_program_sha256,
            "operations": [{"op":"set_array","node_id":"node_rotor_bank","count":4,"spacing":760.0}]
        });
        let result = apply_forge_visual_geometry_patch_v2(&source, &patch).unwrap();
        assert!(result
            .incremental_plan
            .reused_source_node_ids
            .contains(&"node_rotor".into()));
        assert!(result
            .incremental_plan
            .invalidated_source_node_ids
            .contains(&"node_rotor_bank".into()));
        assert!(result
            .incremental_plan
            .invalidated_source_node_ids
            .contains(&"node_rotor_zone".into()));
        assert!(result
            .incremental_plan
            .reused_shape_operation_ids
            .contains(&"op_rotor".into()));
        assert!(result
            .incremental_plan
            .invalidated_shape_operation_ids
            .contains(&"op_rotor_bank".into()));
        assert!(result
            .incremental_plan
            .invalidated_output_ids
            .contains(&"output_rotor_bank".into()));
        assert!(!result.incremental_plan.full_compile_cache_hit);
    }

    #[test]
    fn vp204_patch_rejects_stale_duplicate_unknown_and_type_mismatch() {
        let source = rotor();
        let base = lower_forge_visual_geometry_program_v2(&source).unwrap();
        let mut stale = json!({"schema_version":"ForgeVisualGeometryPatch@1","patch_id":"patch_stale","expected_source_sha256":"0".repeat(64),"operations":[{"op":"set_array","node_id":"node_rotor_bank","count":4,"spacing":700.0}]});
        assert_eq!(
            apply_forge_visual_geometry_patch_v2(&source, &stale)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP204_PATCH_STALE"
        );
        stale["expected_source_sha256"] = json!(base.source_program_sha256);
        stale["operations"] = json!([
            {"op":"set_array","node_id":"node_rotor_bank","count":4,"spacing":700.0},
            {"op":"set_array","node_id":"node_rotor_bank","count":5,"spacing":700.0}
        ]);
        assert_eq!(
            apply_forge_visual_geometry_patch_v2(&source, &stale)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP204_PATCH_DUPLICATE_TARGET"
        );
        stale["operations"] =
            json!([{"op":"set_extrude_height","node_id":"node_rotor","height":400.0}]);
        assert_eq!(
            apply_forge_visual_geometry_patch_v2(&source, &stale)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP204_PATCH_TYPE_MISMATCH"
        );
        stale["operations"] = json!([{"op":"replace_program","program":{}}]);
        assert_eq!(
            apply_forge_visual_geometry_patch_v2(&source, &stale)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_VP204_PATCH_PARSE_FAILED"
        );
    }
}
