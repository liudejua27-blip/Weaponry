//! Rust-owned, bounded assembly edits for an existing Agent asset.
//!
//! `AssemblyDeltaProgram@1` is the missing bridge between a generated arm and
//! the next user turn.  It is an intent/change contract, not an executable
//! ShapeProgram: only reviewed Recipe identities, existing Part/Connector
//! identities and bounded transforms/poses can cross this boundary.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::component_recipes::transform::{
    euler_xyz_from_rotation, inverse_rigid, multiply, rigid_rotation, rotation_matrix_from_euler,
    transform_matrix, transform_point, Matrix4,
};
use crate::{
    semantic_sha256, AgentAssetVersion, ComponentRecipeRef, CoreError, CoreResult, RecipeExpander,
    RecipeExpansionPolicy, RecipeInstantiationRequest, RecipeRegistry, RecipeTransform,
    RecipeValidator,
};

pub const ASSEMBLY_DELTA_PROGRAM_SCHEMA_VERSION: &str = "AssemblyDeltaProgram@1";
pub const ASSEMBLY_DELTA_LOWERING_SCHEMA_VERSION: &str = "AssemblyDeltaLowering@1";

const REVIEWED_ARM_RECIPES: [&str; 14] = [
    "recipe_c106_arm_turntable",
    "recipe_c106_arm_joint_housing",
    "recipe_c106_arm_link_armor",
    "recipe_c106_arm_cable_harness",
    "recipe_c106_arm_gripper",
    "recipe_c106_arm_surface_trim",
    "recipe_c110c_arm_sensor_pod",
    "recipe_c110d_arm_actuator_cover",
    "recipe_c110d_arm_cable_guide",
    "recipe_c110d_arm_wrist_tool_mount",
    "recipe_c110g_parallel_rail",
    "recipe_c110g_parallel_carriage",
    "recipe_c110g_parallel_link",
    "recipe_c110g_parallel_end_effector",
];

/// These are visual attachment affordances.  They are deliberately separate
/// from the required C106 root slots so an edit cannot silently replace the
/// structural arm just because a user asked for another accessory.
const REVIEWED_ARM_ATTACHMENT_SLOTS: [&str; 8] = [
    "slot_arm_sensor_pod",
    "slot_arm_guard_rail",
    "slot_arm_tool_changer",
    "slot_arm_camera_boom",
    "slot_c110g_parallel_rail",
    "slot_c110g_parallel_carriage",
    "slot_c110g_parallel_link",
    "slot_c110g_parallel_tool",
];

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeltaTransform {
    pub position: [f64; 3],
    pub rotation: [f64; 3],
    pub scale: [f64; 3],
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeltaJointPose {
    pub rotation: [f64; 3],
    pub translation: [f64; 3],
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum AssemblyDeltaOperation {
    AddReviewedRecipe {
        operation_id: String,
        new_part_id: String,
        parent_part_id: String,
        parent_connector_id: String,
        child_connector_id: String,
        recipe_id: String,
        slot_id: String,
        transform: DeltaTransform,
    },
    ReplaceReviewedRecipe {
        operation_id: String,
        part_id: String,
        recipe_id: String,
    },
    SetPartTransform {
        operation_id: String,
        part_id: String,
        transform: DeltaTransform,
    },
    SetJointPose {
        operation_id: String,
        part_id: String,
        joint_id: String,
        pose: DeltaJointPose,
    },
    SnapPartToConnector {
        operation_id: String,
        part_id: String,
        target_part_id: String,
        target_connector_id: String,
        connector_id: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AssemblyDeltaProgram {
    pub schema_version: String,
    pub domain_pack_id: String,
    pub base_asset_version_id: String,
    pub summary: String,
    pub operations: Vec<AssemblyDeltaOperation>,
    pub visual_only: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AssemblyDeltaLowering {
    pub schema_version: String,
    pub status: String,
    pub base_asset_version_id: String,
    pub operations: Vec<Value>,
    pub intent_sha256: String,
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("ASSEMBLY_DELTA_INVALID", message)
}

fn bounded_id(field: &str, value: &str) -> CoreResult<()> {
    if value.is_empty()
        || value.len() > 120
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b':'))
    {
        return Err(invalid(format!(
            "{field} must be a bounded stable identifier"
        )));
    }
    Ok(())
}

fn validate_transform(transform: &DeltaTransform) -> CoreResult<()> {
    for (label, values) in [
        ("position", transform.position),
        ("rotation", transform.rotation),
        ("scale", transform.scale),
    ] {
        if values
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 100_000.0)
        {
            return Err(invalid(format!(
                "transform.{label} is outside the visual bound"
            )));
        }
    }
    if transform
        .scale
        .iter()
        .any(|value| *value <= 0.0 || *value > 100.0)
    {
        return Err(invalid(
            "transform.scale must contain positive bounded values",
        ));
    }
    Ok(())
}

fn validate_pose(pose: &DeltaJointPose) -> CoreResult<()> {
    for (label, values) in [
        ("rotation", pose.rotation),
        ("translation", pose.translation),
    ] {
        if values
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 100_000.0)
        {
            return Err(invalid(format!("pose.{label} is outside the visual bound")));
        }
    }
    Ok(())
}

fn validate_operation(
    operation: &AssemblyDeltaOperation,
    seen: &mut BTreeSet<String>,
) -> CoreResult<()> {
    let operation_id = match operation {
        AssemblyDeltaOperation::AddReviewedRecipe { operation_id, .. }
        | AssemblyDeltaOperation::ReplaceReviewedRecipe { operation_id, .. }
        | AssemblyDeltaOperation::SetPartTransform { operation_id, .. }
        | AssemblyDeltaOperation::SetJointPose { operation_id, .. }
        | AssemblyDeltaOperation::SnapPartToConnector { operation_id, .. } => operation_id,
    };
    bounded_id("operation_id", operation_id)?;
    if !seen.insert(operation_id.clone()) {
        return Err(invalid("operation_id values must be unique"));
    }
    match operation {
        AssemblyDeltaOperation::AddReviewedRecipe {
            new_part_id,
            parent_part_id,
            parent_connector_id,
            child_connector_id,
            recipe_id,
            slot_id,
            transform,
            ..
        } => {
            for (field, value) in [
                ("new_part_id", new_part_id),
                ("parent_part_id", parent_part_id),
                ("parent_connector_id", parent_connector_id),
                ("child_connector_id", child_connector_id),
                ("recipe_id", recipe_id),
                ("slot_id", slot_id),
            ] {
                bounded_id(field, value)?;
            }
            if !new_part_id.starts_with("part_") {
                return Err(invalid("new_part_id must use the stable part_ namespace"));
            }
            if !REVIEWED_ARM_RECIPES.contains(&recipe_id.as_str()) {
                return Err(invalid(
                    "add_reviewed_recipe must use a reviewed arm Recipe",
                ));
            }
            if !REVIEWED_ARM_ATTACHMENT_SLOTS.contains(&slot_id.as_str()) {
                return Err(invalid(
                    "slot_id is not a reviewed robotic-arm attachment slot",
                ));
            }
            validate_transform(transform)?;
        }
        AssemblyDeltaOperation::ReplaceReviewedRecipe {
            part_id, recipe_id, ..
        } => {
            bounded_id("part_id", part_id)?;
            if !REVIEWED_ARM_RECIPES.contains(&recipe_id.as_str()) {
                return Err(invalid(
                    "replace_reviewed_recipe must use a reviewed arm Recipe",
                ));
            }
        }
        AssemblyDeltaOperation::SetPartTransform {
            part_id, transform, ..
        } => {
            bounded_id("part_id", part_id)?;
            validate_transform(transform)?;
        }
        AssemblyDeltaOperation::SetJointPose {
            part_id,
            joint_id,
            pose,
            ..
        } => {
            bounded_id("part_id", part_id)?;
            bounded_id("joint_id", joint_id)?;
            validate_pose(pose)?;
        }
        AssemblyDeltaOperation::SnapPartToConnector {
            part_id,
            target_part_id,
            target_connector_id,
            connector_id,
            ..
        } => {
            for (field, value) in [
                ("part_id", part_id),
                ("target_part_id", target_part_id),
                ("target_connector_id", target_connector_id),
                ("connector_id", connector_id),
            ] {
                bounded_id(field, value)?;
            }
        }
    }
    Ok(())
}

impl AssemblyDeltaProgram {
    pub fn validate(&self) -> CoreResult<()> {
        if self.schema_version != ASSEMBLY_DELTA_PROGRAM_SCHEMA_VERSION {
            return Err(invalid("schema_version must be AssemblyDeltaProgram@1"));
        }
        if self.domain_pack_id != "pack_robotic_arm_concept" {
            return Err(invalid("C110C currently accepts robotic-arm deltas only"));
        }
        bounded_id("base_asset_version_id", &self.base_asset_version_id)?;
        if self.summary.trim().is_empty() || self.summary.chars().count() > 2_000 {
            return Err(invalid("summary must be a bounded non-empty description"));
        }
        if !self.visual_only {
            return Err(invalid("visual_only must be true"));
        }
        if self.operations.is_empty() || self.operations.len() > 8 {
            return Err(invalid("operations must contain 1 to 8 bounded edits"));
        }
        let mut seen = BTreeSet::new();
        for operation in &self.operations {
            validate_operation(operation, &mut seen)?;
        }
        Ok(())
    }
}

/// Lower to the existing ChangeSet vocabulary while retaining the delta's
/// exact identity.  `add_reviewed_recipe` and `replace_reviewed_recipe` are
/// intentionally explicit operations; the repository validates their
/// reviewed Recipe/slot references before a geometry worker is invoked.
pub fn lower_assembly_delta(value: &Value) -> CoreResult<AssemblyDeltaLowering> {
    let program: AssemblyDeltaProgram = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("AssemblyDeltaProgram@1 failed closed: {error}")))?;
    program.validate()?;
    let operations = program
        .operations
        .iter()
        .map(|operation| match operation {
            AssemblyDeltaOperation::AddReviewedRecipe {
                operation_id,
                new_part_id,
                parent_part_id,
                parent_connector_id,
                child_connector_id,
                recipe_id,
                slot_id,
                transform,
            } => json!({
                "operation_id": operation_id,
                "op": "add_reviewed_recipe",
                "part_id": parent_part_id,
                "new_part_id": new_part_id,
                "parent_connector_id": parent_connector_id,
                "child_connector_id": child_connector_id,
                "recipe_id": recipe_id,
                "slot_id": slot_id,
                "transform": transform,
            }),
            AssemblyDeltaOperation::ReplaceReviewedRecipe {
                operation_id,
                part_id,
                recipe_id,
            } => json!({
                "operation_id": operation_id,
                "op": "replace_reviewed_recipe",
                "part_id": part_id,
                "recipe_id": recipe_id,
            }),
            AssemblyDeltaOperation::SetPartTransform {
                operation_id,
                part_id,
                transform,
            } => json!({
                "operation_id": operation_id,
                "op": "set_part_transform",
                "part_id": part_id,
                "transform": transform,
            }),
            AssemblyDeltaOperation::SetJointPose {
                operation_id,
                part_id,
                joint_id,
                pose,
            } => json!({
                "operation_id": operation_id,
                "op": "set_joint_pose",
                "part_id": part_id,
                "joint_id": joint_id,
                "pose": pose,
            }),
            AssemblyDeltaOperation::SnapPartToConnector {
                operation_id,
                part_id,
                target_part_id,
                target_connector_id,
                connector_id,
            } => json!({
                "operation_id": operation_id,
                "op": "snap_part_to_connector",
                "part_id": part_id,
                "target_part_id": target_part_id,
                "target_connector_id": target_connector_id,
                "connector_id": connector_id,
            }),
        })
        .collect::<Vec<_>>();
    let intent_sha256 = semantic_sha256(&program)?;
    Ok(AssemblyDeltaLowering {
        schema_version: ASSEMBLY_DELTA_LOWERING_SCHEMA_VERSION.into(),
        status: "lowered".into(),
        base_asset_version_id: program.base_asset_version_id,
        operations,
        intent_sha256,
    })
}

/// Materialize bounded C110C operations against an immutable base
/// asset.  This is a pure function: it does not write a Version, Snapshot or
/// CAS object.  The caller must send its returned ShapeProgram to the normal
/// restricted compiler and then use preview -> confirm for persistence.
///
pub fn materialize_assembly_delta(
    base: &AgentAssetVersion,
    value: &Value,
) -> CoreResult<AgentAssetVersion> {
    let program: AssemblyDeltaProgram = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("AssemblyDeltaProgram@1 failed closed: {error}")))?;
    program.validate()?;
    if base.asset_version_id != program.base_asset_version_id {
        return Err(CoreError::conflict(
            "ASSEMBLY_DELTA_BASE_STALE",
            "Assembly delta base AssetVersion does not match the active immutable asset.",
        ));
    }
    let mut result = base.clone();
    let mut applied = 0usize;
    for operation in &program.operations {
        match operation {
            AssemblyDeltaOperation::AddReviewedRecipe {
                operation_id,
                new_part_id,
                parent_part_id,
                parent_connector_id,
                child_connector_id,
                recipe_id,
                slot_id,
                transform,
            } => {
                materialize_add_reviewed_recipe(
                    &mut result,
                    &program,
                    operation_id,
                    new_part_id,
                    parent_part_id,
                    parent_connector_id,
                    child_connector_id,
                    recipe_id,
                    slot_id,
                    transform,
                )?;
                applied += 1;
            }
            AssemblyDeltaOperation::ReplaceReviewedRecipe {
                operation_id,
                part_id,
                recipe_id,
            } => {
                materialize_replace_reviewed_recipe(
                    &mut result,
                    &program,
                    operation_id,
                    part_id,
                    recipe_id,
                )?;
                applied += 1;
            }
            AssemblyDeltaOperation::SetPartTransform {
                part_id, transform, ..
            } => {
                materialize_set_part_transform(&mut result, part_id, transform)?;
                applied += 1;
            }
            AssemblyDeltaOperation::SetJointPose {
                part_id,
                joint_id,
                pose,
                ..
            } => {
                materialize_set_joint_pose(&mut result, part_id, joint_id, pose)?;
                applied += 1;
            }
            AssemblyDeltaOperation::SnapPartToConnector {
                part_id,
                target_part_id,
                target_connector_id,
                connector_id,
                ..
            } => {
                materialize_snap_part_to_connector(
                    &mut result,
                    part_id,
                    target_part_id,
                    target_connector_id,
                    connector_id,
                )?;
                applied += 1;
            }
        }
    }
    if applied == 0 {
        return Err(invalid(
            "Assembly delta did not contain a materialized operation",
        ));
    }
    Ok(result)
}

fn graph_parts(value: &mut AgentAssetVersion) -> CoreResult<&mut Vec<Value>> {
    value
        .assembly_graph
        .get_mut("parts")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("AssemblyGraph has no mutable parts"))
}

fn graph_part<'a>(value: &'a AgentAssetVersion, part_id: &str) -> CoreResult<&'a Value> {
    value
        .assembly_graph
        .get("parts")
        .and_then(Value::as_array)
        .and_then(|parts| {
            parts
                .iter()
                .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        })
        .ok_or_else(|| CoreError::not_found("Assembly delta target Part"))
}

fn graph_part_mut<'a>(
    value: &'a mut AgentAssetVersion,
    part_id: &str,
) -> CoreResult<&'a mut Value> {
    graph_parts(value)?
        .iter_mut()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        .ok_or_else(|| CoreError::not_found("Assembly delta target Part"))
}

fn vec3(value: Option<&Value>, field: &str) -> CoreResult<[f64; 3]> {
    let values = value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("{field} must be a three-number vector")))?;
    if values.len() != 3 {
        return Err(invalid(format!("{field} must be a three-number vector")));
    }
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        result[index] = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid(format!("{field} must contain finite numbers")))?;
    }
    Ok(result)
}

fn graph_transform(part: &Value) -> CoreResult<RecipeTransform> {
    let transform = part
        .get("transform")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("AssemblyGraph Part has no transform"))?;
    Ok(RecipeTransform {
        position: vec3(transform.get("position"), "Part transform.position")?,
        rotation: vec3(transform.get("rotation"), "Part transform.rotation")?,
        scale: vec3(transform.get("scale"), "Part transform.scale")?,
    })
}

fn require_rigid_transform(transform: &DeltaTransform) -> CoreResult<Matrix4> {
    if transform
        .scale
        .iter()
        .any(|value| (*value - 1.0).abs() > 1e-9)
    {
        return Err(CoreError::invalid_data(
            "ASSEMBLY_DELTA_SCALE_UNSUPPORTED",
            "C110C geometry propagation currently accepts rigid visual transforms only; scale must be [1,1,1].",
        ));
    }
    let matrix = transform_matrix(&RecipeTransform {
        position: transform.position,
        rotation: transform.rotation,
        scale: transform.scale,
    })?;
    let _ = rigid_rotation(matrix)?;
    Ok(matrix)
}

fn part_operation_prefix(part: &Value) -> CoreResult<String> {
    let instance_id = part
        .get("recipe_instance_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("AssemblyGraph Part has no recipe_instance_id"))?;
    Ok(format!(
        "op_{}_*",
        instance_id.trim_start_matches("recipeinst_")
    ))
}

fn operation_belongs_to_part(operation_id: &str, prefix: &str) -> bool {
    prefix.ends_with("_*") && operation_id.starts_with(prefix.trim_end_matches('*'))
}

fn apply_rigid_delta_to_part(
    value: &mut AgentAssetVersion,
    part_id: &str,
    delta: Matrix4,
    desired_transform: RecipeTransform,
) -> CoreResult<()> {
    let part_snapshot = graph_part(value, part_id)?.clone();
    let prefix = part_operation_prefix(&part_snapshot)?;
    let shape = value
        .shape_program
        .as_object_mut()
        .ok_or_else(|| invalid("ShapeProgram is not an object"))?;
    let operations = shape
        .get_mut("operations")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ShapeProgram has no operations"))?;
    let mut changed = 0usize;
    for operation in operations.iter_mut() {
        let operation_id = operation
            .get("operation_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !operation_belongs_to_part(operation_id, &prefix) {
            continue;
        }
        let inputs = operation
            .get("inputs")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("ShapeProgram operation has no inputs"))?;
        // Only source operations own a baked frame. Derived operations follow
        // their inputs and must not receive a second transform.
        if !inputs.is_empty() {
            continue;
        }
        let op = operation
            .get("op")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if op == "profile" {
            continue;
        }
        let args = operation
            .get_mut("args")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| invalid("ShapeProgram source operation has no args"))?;
        let old_position = vec3(args.get("position"), "source operation position")?;
        args.insert(
            "position".into(),
            json!(transform_point(delta, old_position)?),
        );
        let old_rotation = match args.get("rotation") {
            Some(value) => vec3(Some(value), "source operation rotation")?,
            None => [0.0; 3],
        };
        let combined = multiply(delta, rotation_matrix_from_euler(old_rotation));
        let rotation = euler_xyz_from_rotation(rigid_rotation(combined)?);
        if rotation.iter().any(|value| value.abs() > 1e-12) {
            args.insert("rotation".into(), json!(rotation));
        } else {
            args.remove("rotation");
        }
        changed += 1;
    }
    if changed == 0 {
        return Err(CoreError::conflict(
            "ASSEMBLY_DELTA_GEOMETRY_BINDING_MISSING",
            "Target Part has no transformable reviewed source geometry.",
        ));
    }
    let part = graph_part_mut(value, part_id)?;
    part["transform"] = json!({
        "position": desired_transform.position,
        "rotation": desired_transform.rotation,
        "scale": desired_transform.scale,
    });
    Ok(())
}

fn materialize_set_part_transform(
    value: &mut AgentAssetVersion,
    part_id: &str,
    transform: &DeltaTransform,
) -> CoreResult<()> {
    let desired = require_rigid_transform(transform)?;
    let current = graph_transform(graph_part(value, part_id)?)?;
    let current_matrix = transform_matrix(&current)?;
    let delta = multiply(desired, inverse_rigid(current_matrix));
    apply_rigid_delta_to_part(
        value,
        part_id,
        delta,
        RecipeTransform {
            position: transform.position,
            rotation: transform.rotation,
            scale: transform.scale,
        },
    )
}

fn materialize_set_joint_pose(
    value: &mut AgentAssetVersion,
    part_id: &str,
    joint_id: &str,
    pose: &DeltaJointPose,
) -> CoreResult<()> {
    let part = graph_part(value, part_id)?.clone();
    let has_joint = part
        .get("joints")
        .and_then(Value::as_array)
        .is_some_and(|joints| {
            joints
                .iter()
                .any(|joint| joint.get("joint_id").and_then(Value::as_str) == Some(joint_id))
        });
    let role = part.get("role").and_then(Value::as_str).unwrap_or_default();
    if !has_joint && !(role == "joint_housing" && joint_id.starts_with("joint_")) {
        return Err(CoreError::not_found(
            "Assembly delta target Joint is not a reviewed visual joint",
        ));
    }
    if !has_joint {
        let target = graph_part_mut(value, part_id)?;
        target
            .as_object_mut()
            .expect("Part is an object")
            .entry("joints")
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .expect("joints is an array")
            .push(json!({
                "joint_id": joint_id,
                "kind": "revolute_visual",
                "target_part_id": part_id,
                "axis": [0, 0, 1],
                "min_value": -3.141592653589793,
                "max_value": 3.141592653589793,
            }));
    }
    let current = graph_transform(graph_part(value, part_id)?)?;
    let current_matrix = transform_matrix(&current)?;
    let pose_matrix = transform_matrix(&RecipeTransform {
        position: pose.translation,
        rotation: pose.rotation,
        scale: [1.0; 3],
    })?;
    let desired_matrix = multiply(pose_matrix, current_matrix);
    let desired_rotation = euler_xyz_from_rotation(rigid_rotation(desired_matrix)?);
    materialize_set_part_transform(
        value,
        part_id,
        &DeltaTransform {
            position: [
                desired_matrix[0][3],
                desired_matrix[1][3],
                desired_matrix[2][3],
            ],
            rotation: desired_rotation,
            scale: [1.0; 3],
        },
    )
}

fn connector(value: &Value, connector_id: &str) -> CoreResult<Value> {
    value
        .get("connectors")
        .and_then(Value::as_array)
        .and_then(|connectors| {
            connectors.iter().find(|connector| {
                connector.get("connector_id").and_then(Value::as_str) == Some(connector_id)
            })
        })
        .cloned()
        .ok_or_else(|| CoreError::not_found("Assembly delta connector"))
}

fn connector_world_position(part: &Value, connector_id: &str) -> CoreResult<[f64; 3]> {
    let frame = connector(part, connector_id)?;
    let part_transform = graph_transform(part)?;
    let matrix = transform_matrix(&part_transform)?;
    transform_point(matrix, vec3(frame.get("position"), "connector position")?)
}

fn materialize_snap_part_to_connector(
    value: &mut AgentAssetVersion,
    part_id: &str,
    target_part_id: &str,
    target_connector_id: &str,
    connector_id: &str,
) -> CoreResult<()> {
    let source = graph_part(value, part_id)?.clone();
    let target = graph_part(value, target_part_id)?.clone();
    let source_connector = connector(&source, connector_id)?;
    let target_connector = connector(&target, target_connector_id)?;
    if source_connector.get("kind") != target_connector.get("kind") {
        return Err(CoreError::conflict(
            "ASSEMBLY_DELTA_CONNECTOR_INCOMPATIBLE",
            "Source and target connector kinds must match.",
        ));
    }
    let source_position = connector_world_position(&source, connector_id)?;
    let target_position = connector_world_position(&target, target_connector_id)?;
    let current = graph_transform(&source)?;
    let desired_position = [
        current.position[0] + target_position[0] - source_position[0],
        current.position[1] + target_position[1] - source_position[1],
        current.position[2] + target_position[2] - source_position[2],
    ];
    materialize_set_part_transform(
        value,
        part_id,
        &DeltaTransform {
            position: desired_position,
            rotation: current.rotation,
            scale: [1.0; 3],
        },
    )
}

fn attachment_registry_for_recipe(recipe_id: &str) -> CoreResult<RecipeRegistry> {
    if matches!(
        recipe_id,
        "recipe_c110c_arm_sensor_pod"
            | "recipe_c110d_arm_actuator_cover"
            | "recipe_c110d_arm_cable_guide"
            | "recipe_c110d_arm_wrist_tool_mount"
    ) {
        RecipeRegistry::from_embedded_c110c_robotic_arm_attachments()
    } else if matches!(
        recipe_id,
        "recipe_c110g_parallel_rail"
            | "recipe_c110g_parallel_carriage"
            | "recipe_c110g_parallel_link"
            | "recipe_c110g_parallel_end_effector"
    ) {
        RecipeRegistry::from_embedded_c110g_parallel_link()
    } else {
        RecipeRegistry::from_embedded_c106_robotic_arm()
    }
}

fn materialize_replace_reviewed_recipe(
    value: &mut AgentAssetVersion,
    program: &AssemblyDeltaProgram,
    operation_id: &str,
    part_id: &str,
    recipe_id: &str,
) -> CoreResult<()> {
    let old_part = graph_part(value, part_id)?.clone();
    let old_instance_id = old_part
        .get("recipe_instance_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("replace target Part has no recipe_instance_id"))?
        .to_owned();
    let old_prefix = format!("op_{}_", old_instance_id.trim_start_matches("recipeinst_"));
    let registry = attachment_registry_for_recipe(recipe_id)?;
    let recipe = registry
        .recipe(recipe_id)
        .ok_or_else(|| CoreError::not_found("Assembly delta replacement Recipe"))?;
    let current = graph_transform(&old_part)?;
    let op_hash = semantic_sha256(&json!({
        "base": program.base_asset_version_id,
        "operation_id": operation_id,
        "part_id": part_id,
        "recipe_id": recipe_id,
    }))?;
    let request = RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "active_asset_edit".into(),
        request_id: format!("recipereq_{}", &op_hash[..48]),
        project_id: Some(value.project_id.clone()),
        base_asset_version_id: Some(program.base_asset_version_id.clone()),
        snapshot_revision: Some(1),
        domain_pack_id: program.domain_pack_id.clone(),
        recipe_registry_sha256: registry.registry_sha256().into(),
        recipe: ComponentRecipeRef {
            schema_version: "ComponentRecipeRef@1".into(),
            recipe_id: recipe.recipe_id.clone(),
            version: recipe.version,
            recipe_sha256: RecipeValidator::recipe_sha256(recipe)?,
        },
        target_part_id: Some(part_id.to_owned()),
        slot_bindings: Vec::new(),
        parameter_values: Vec::new(),
        material_zone_overrides: Vec::new(),
    };
    let candidate = RecipeExpander::expand_with_root_transform(
        &registry,
        &request,
        &RecipeExpansionPolicy::default(),
        Some(&RecipeTransform {
            position: current.position,
            rotation: current.rotation,
            scale: [1.0; 3],
        }),
    )?;
    let candidate_parts = candidate
        .expanded_assembly_graph
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("replacement Recipe has no parts"))?;
    if candidate_parts.len() != 1 {
        return Err(CoreError::conflict(
            "ASSEMBLY_DELTA_REPLACEMENT_NOT_ATOMIC",
            "C110C replacement Recipes must expand to exactly one Part.",
        ));
    }
    let mut replacement = candidate_parts[0].clone();
    let old_parent = old_part
        .get("parent_part_id")
        .cloned()
        .unwrap_or(Value::Null);
    replacement["part_id"] = Value::String(part_id.to_owned());
    replacement["parent_part_id"] = old_parent;
    let replacement_connector = replacement
        .get("connectors")
        .and_then(Value::as_array)
        .and_then(|connectors| connectors.first())
        .cloned()
        .ok_or_else(|| invalid("replacement Recipe has no connector"))?;
    let old_connectors = old_part
        .get("connectors")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("replace target Part has no connectors"))?;
    let old_connector = old_connectors
        .first()
        .ok_or_else(|| invalid("replace target Part has no connector"))?;
    if old_connector.get("kind") != replacement_connector.get("kind") {
        return Err(CoreError::conflict(
            "ASSEMBLY_DELTA_REPLACEMENT_CONNECTOR_INCOMPATIBLE",
            "Replacement Recipe connector kind must match the existing assembly connection.",
        ));
    }
    let candidate_shape = candidate.expanded_shape_program;
    let candidate_operations = candidate_shape
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("replacement Recipe has no operations"))?;
    let candidate_outputs = candidate_shape
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("replacement Recipe has no outputs"))?;
    let shape = value
        .shape_program
        .as_object_mut()
        .ok_or_else(|| invalid("ShapeProgram is not an object"))?;
    let operations = shape
        .get_mut("operations")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ShapeProgram has no operations"))?;
    operations.retain(|operation| {
        !operation
            .get("operation_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with(&old_prefix))
    });
    operations.extend(candidate_operations.iter().cloned());
    let outputs = shape
        .get_mut("outputs")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("ShapeProgram has no outputs"))?;
    let old_output_prefix = format!(
        "output_{}_",
        old_instance_id.trim_start_matches("recipeinst_")
    );
    outputs.retain(|output| {
        !output
            .get("output_id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with(&old_output_prefix))
    });
    outputs.extend(candidate_outputs.iter().cloned());
    shape["program_id"] = Value::String(format!("shape_delta_{}", &op_hash[..16]));
    shape["seed"] = Value::Number(serde_json::Number::from(
        u32::from_str_radix(&op_hash[..8], 16).unwrap_or(0) % 2_147_483_647,
    ));
    let part_index = value
        .parts
        .iter()
        .position(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        .ok_or_else(|| CoreError::not_found("replace target AssetVersion Part"))?;
    value.parts[part_index] = replacement.clone();
    let graph = value
        .assembly_graph
        .as_object_mut()
        .ok_or_else(|| invalid("AssemblyGraph is not an object"))?;
    let graph_parts = graph
        .get_mut("parts")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("AssemblyGraph has no parts"))?;
    let graph_part_index = graph_parts
        .iter()
        .position(|part| part.get("part_id").and_then(Value::as_str) == Some(part_id))
        .ok_or_else(|| CoreError::not_found("replace target graph Part"))?;
    graph_parts[graph_part_index] = replacement;
    let connections = graph
        .get_mut("connections")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("AssemblyGraph has no connections"))?;
    let old_connector_id = old_connector
        .get("connector_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("old Part connector has no id"))?;
    let replacement_connector_id = replacement_connector
        .get("connector_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("replacement connector has no id"))?;
    for connection in connections.iter_mut() {
        if connection.get("to_part_id").and_then(Value::as_str) == Some(part_id)
            && connection.get("to_connector_id").and_then(Value::as_str) == Some(old_connector_id)
        {
            connection["to_connector_id"] = Value::String(replacement_connector_id.to_owned());
        }
        if connection.get("from_part_id").and_then(Value::as_str) == Some(part_id)
            && connection.get("from_connector_id").and_then(Value::as_str) == Some(old_connector_id)
        {
            connection["from_connector_id"] = Value::String(replacement_connector_id.to_owned());
        }
    }
    if let Some(instances) = graph
        .get_mut("component_recipe_instances")
        .and_then(Value::as_array_mut)
    {
        instances.retain(|instance| {
            instance.get("instance_id").and_then(Value::as_str) != Some(old_instance_id.as_str())
        });
        if let Some(candidate_instances) = candidate
            .expanded_assembly_graph
            .get("component_recipe_instances")
            .and_then(Value::as_array)
        {
            instances.extend(candidate_instances.iter().cloned());
        }
    }
    graph["graph_id"] = Value::String(format!("asset_delta_graph_{}", &op_hash[..16]));
    graph["concept_id"] = Value::String(format!("asset_delta_{}", &op_hash[..16]));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn materialize_add_reviewed_recipe(
    result: &mut AgentAssetVersion,
    program: &AssemblyDeltaProgram,
    operation_id: &str,
    new_part_id: &str,
    parent_part_id: &str,
    parent_connector_id: &str,
    child_connector_id: &str,
    recipe_id: &str,
    slot_id: &str,
    transform: &DeltaTransform,
) -> CoreResult<()> {
    let parts = result
        .assembly_graph
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("base AssemblyGraph has no parts"))?;
    let parent_part = parts
        .iter()
        .find(|part| part.get("part_id").and_then(Value::as_str) == Some(parent_part_id))
        .ok_or_else(|| CoreError::not_found("Assembly delta parent Part"))?;
    let parent_has_connector = parent_part
        .get("connectors")
        .and_then(Value::as_array)
        .is_some_and(|connectors| {
            connectors.iter().any(|connector| {
                connector.get("connector_id").and_then(Value::as_str) == Some(parent_connector_id)
            })
        });
    if !parent_has_connector {
        return Err(invalid(
            "parent_connector_id is not declared by the target Part",
        ));
    }
    if parts
        .iter()
        .any(|part| part.get("part_id").and_then(Value::as_str) == Some(new_part_id))
    {
        return Err(CoreError::conflict(
            "ASSEMBLY_DELTA_PART_EXISTS",
            "Assembly delta cannot add a duplicate Part identity.",
        ));
    }

    let registry = attachment_registry_for_recipe(recipe_id)?;
    let recipe = registry
        .recipe(recipe_id)
        .ok_or_else(|| CoreError::not_found("reviewed attachment Recipe"))?;
    let recipe_ref = ComponentRecipeRef {
        schema_version: "ComponentRecipeRef@1".into(),
        recipe_id: recipe.recipe_id.clone(),
        version: recipe.version,
        recipe_sha256: RecipeValidator::recipe_sha256(recipe)?,
    };
    let op_hash = semantic_sha256(&json!({
        "base": program.base_asset_version_id,
        "operation_id": operation_id,
        "new_part_id": new_part_id,
        "parent_part_id": parent_part_id,
        "recipe_id": recipe_id,
        "slot_id": slot_id,
        "transform": transform,
    }))?;
    let request = RecipeInstantiationRequest {
        schema_version: "ComponentRecipeInstantiationRequest@1".into(),
        context_mode: "initial_candidate".into(),
        request_id: format!("recipereq_{}", &op_hash[..48]),
        project_id: None,
        base_asset_version_id: None,
        snapshot_revision: None,
        domain_pack_id: program.domain_pack_id.clone(),
        recipe_registry_sha256: registry.registry_sha256().into(),
        recipe: recipe_ref,
        target_part_id: None,
        slot_bindings: Vec::new(),
        parameter_values: Vec::new(),
        material_zone_overrides: Vec::new(),
    };
    let root_transform = RecipeTransform {
        position: transform.position,
        rotation: transform.rotation,
        scale: transform.scale,
    };
    let candidate = RecipeExpander::expand_with_root_transform(
        &registry,
        &request,
        &RecipeExpansionPolicy::default(),
        Some(&root_transform),
    )?;
    let candidate_parts = candidate
        .expanded_assembly_graph
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Reviewed attachment candidate has no parts"))?;
    if candidate_parts.len() != 1 {
        return Err(CoreError::conflict(
            "ASSEMBLY_DELTA_ATTACHMENT_NOT_ATOMIC",
            "Reviewed attachment Recipe must expand to exactly one Part before it can be added.",
        ));
    }
    let mut new_part = candidate_parts[0].clone();
    new_part["part_id"] = Value::String(new_part_id.into());
    new_part["parent_part_id"] = Value::String(parent_part_id.into());
    let candidate_root_instance = candidate
        .expanded_assembly_graph
        .get("root_part_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("reviewed attachment candidate has no root Part"))?;
    if new_part.get("part_id").and_then(Value::as_str).is_none() {
        return Err(invalid(
            "reviewed attachment candidate Part identity is invalid",
        ));
    }
    // The candidate's primary output/operation IDs are immutable and remain
    // unique because the request hash includes the delta operation identity.
    if candidate_root_instance.is_empty() {
        return Err(invalid(
            "reviewed attachment candidate root identity is empty",
        ));
    }
    let child_has_connector = new_part
        .get("connectors")
        .and_then(Value::as_array)
        .is_some_and(|connectors| {
            connectors.iter().any(|connector| {
                connector.get("connector_id").and_then(Value::as_str) == Some(child_connector_id)
            })
        });
    if !child_has_connector {
        return Err(invalid(
            "child_connector_id is not declared by the reviewed attachment Recipe",
        ));
    }
    let candidate_program = candidate.expanded_shape_program;
    let candidate_operations = candidate_program
        .get("operations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("reviewed attachment candidate has no operations"))?;
    let candidate_outputs = candidate_program
        .get("outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("reviewed attachment candidate has no outputs"))?;
    let shape = result
        .shape_program
        .as_object_mut()
        .ok_or_else(|| invalid("base ShapeProgram is not an object"))?;
    for key in ["operations", "outputs"] {
        if shape.get(key).and_then(Value::as_array).is_none() {
            return Err(invalid(format!("base ShapeProgram has no {key} array")));
        }
    }
    shape["operations"] = Value::Array(
        shape["operations"]
            .as_array()
            .expect("validated")
            .iter()
            .cloned()
            .chain(candidate_operations.iter().cloned())
            .collect(),
    );
    shape["outputs"] = Value::Array(
        shape["outputs"]
            .as_array()
            .expect("validated")
            .iter()
            .cloned()
            .chain(candidate_outputs.iter().cloned())
            .collect(),
    );
    if let Some(candidate_profiles) = candidate_program
        .get("profile_inputs")
        .and_then(Value::as_array)
    {
        let profiles = shape
            .entry("profile_inputs")
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| invalid("base ShapeProgram profile_inputs is not an array"))?;
        profiles.extend(candidate_profiles.iter().cloned());
    }
    shape["program_id"] = Value::String(format!("shape_delta_{}", &op_hash[..16]));
    shape["seed"] = Value::Number(serde_json::Number::from(
        u32::from_str_radix(&op_hash[..8], 16).unwrap_or(0) % 2_147_483_647,
    ));
    let graph = result
        .assembly_graph
        .as_object_mut()
        .ok_or_else(|| invalid("base AssemblyGraph is not an object"))?;
    let graph_parts = graph
        .get_mut("parts")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("base AssemblyGraph has no mutable parts"))?;
    graph_parts.push(new_part.clone());
    let version_parts = &mut result.parts;
    version_parts.push(new_part);
    let connections = graph
        .entry("connections")
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| invalid("base AssemblyGraph connections is not an array"))?;
    connections.push(json!({
        "connection_id": format!("conn_delta_{}", &op_hash[..16]),
        "from_part_id": parent_part_id,
        "from_connector_id": parent_connector_id,
        "to_part_id": new_part_id,
        "to_connector_id": child_connector_id,
        "slot_id": slot_id,
        "status": "connected",
    }));
    if let Some(candidate_instances) = candidate
        .expanded_assembly_graph
        .get("component_recipe_instances")
        .and_then(Value::as_array)
    {
        let instances = graph
            .entry("component_recipe_instances")
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| invalid("base AssemblyGraph recipe provenance is not an array"))?;
        instances.extend(candidate_instances.iter().cloned());
    }
    graph["graph_id"] = Value::String(format!("asset_delta_graph_{}", &op_hash[..16]));
    graph["concept_id"] = Value::String(format!("asset_delta_{}", &op_hash[..16]));
    Ok(())
}
