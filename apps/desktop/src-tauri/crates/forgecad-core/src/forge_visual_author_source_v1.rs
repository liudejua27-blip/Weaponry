//! E005-R1 unified compact formal author source.
//!
//! This envelope is deliberately not a fourth geometry IR. Geometry templates
//! are validated by the existing `ForgeVisualGeometryProgram@2` compiler; this
//! module only adds typed parameters, bounded macro instances/repeat, rigid
//! hierarchy, surface intent and detail-motif lineage before deterministically
//! expanding back into that same VP203 geometry source.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::component_recipes::{
    transform::{
        euler_xyz_from_rotation, multiply, rigid_rotation, transform_matrix, transform_point,
        Matrix4,
    },
    RecipeTransform,
};
use crate::shape_program::normalize_persisted_shape_program;
use crate::{
    lower_forge_visual_geometry_program_v2, semantic_sha256, CoreError, CoreResult,
    ForgeVisualParameterKindV2, ForgeVisualParameterV2, ForgeVisualUnitSystemV2,
};

pub const FORGE_VISUAL_AUTHOR_SOURCE_SCHEMA_VERSION: &str = "ForgeVisualAuthorSource@1";
pub const FORGE_VISUAL_AUTHOR_LOWERING_SCHEMA_VERSION: &str = "ForgeVisualAuthorLowering@1";
const FORGE_VISUAL_AUTHOR_COMPILER_VERSION: &str = "forgecad-core-e005-r1.1";
const FORGE_VISUAL_AUTHOR_ID_ALGORITHM_VERSION: &str = "author-instance-hash-v1";

fn invalid(code: &'static str, message: impl Into<String>) -> CoreError {
    CoreError::invalid_data(code, message.into())
}

fn require_prefixed_id(value: &str, prefix: &str, field: &str) -> CoreResult<()> {
    if (prefix.len() + 2..=96).contains(&value.len())
        && value.starts_with(prefix)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
    {
        Ok(())
    } else {
        Err(invalid(
            "FORGE_VISUAL_R1_ID_INVALID",
            format!("{field} must be a bounded lowercase {prefix} ID"),
        ))
    }
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum AuthorScalarV1 {
    Literal(f64),
    Parameter { parameter_id: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum AuthorCountV1 {
    Literal(u16),
    Parameter { parameter_id: String },
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AuthorRepeatV1 {
    pub count: AuthorCountV1,
    pub step: [AuthorScalarV1; 3],
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AuthorRigidTransformV1 {
    pub position: [AuthorScalarV1; 3],
    pub rotation: [AuthorScalarV1; 3],
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorMacroSemanticKindV1 {
    PrimaryForm,
    StructuralForm,
    DetailMotif,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorMacroV1 {
    pub macro_id: String,
    pub semantic_kind: AuthorMacroSemanticKindV1,
    pub output_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorPartRefV1 {
    pub instance_id: String,
    pub repeat_index: u16,
    pub output_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AuthorInstanceV1 {
    pub instance_id: String,
    pub macro_id: String,
    pub transform: AuthorRigidTransformV1,
    pub repeat: AuthorRepeatV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<AuthorPartRefV1>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorSurfaceProfileV1 {
    PaintedMetal,
    BrushedMetal,
    DarkInset,
    Rubberized,
    EmissiveTrim,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AuthorSurfaceBindingV1 {
    pub binding_id: String,
    pub macro_id: String,
    pub output_id: String,
    pub material_id: String,
    pub surface_profile: AuthorSurfaceProfileV1,
    pub edge_wear: f64,
    pub micro_detail: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AuthorBudgetV1 {
    pub schema_version: String,
    pub max_parameters: u16,
    pub max_macros: u16,
    pub max_instances: u16,
    pub max_repeat_count: u16,
    pub max_expanded_nodes: u16,
    pub max_expanded_parts: u16,
    pub max_expanded_outputs: u16,
    pub max_operations: u16,
    pub triangle_budget: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualAuthorSourceV1 {
    pub schema_version: String,
    pub program_id: String,
    pub domain: String,
    pub units: ForgeVisualUnitSystemV2,
    pub seed: u32,
    #[serde(default)]
    pub parameters: Vec<ForgeVisualParameterV2>,
    pub geometry_templates: Value,
    pub macros: Vec<AuthorMacroV1>,
    pub instances: Vec<AuthorInstanceV1>,
    pub root_part: AuthorPartRefV1,
    pub surface_bindings: Vec<AuthorSurfaceBindingV1>,
    pub budgets: AuthorBudgetV1,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualAuthorSourceLineageV1 {
    pub macro_id: String,
    pub semantic_kind: AuthorMacroSemanticKindV1,
    pub instance_id: String,
    pub repeat_index: u16,
    pub source_output_id: String,
    pub expanded_output_id: String,
    pub source_node_ids: Vec<String>,
    pub expanded_node_ids: Vec<String>,
    pub parameter_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualAuthorSurfaceBindingExpandedV1 {
    pub binding_id: String,
    pub macro_id: String,
    pub instance_id: String,
    pub repeat_index: u16,
    pub source_output_id: String,
    pub expanded_output_id: String,
    pub part_id: String,
    pub material_zone_id: String,
    pub material_id: String,
    pub surface_profile: AuthorSurfaceProfileV1,
    pub edge_wear: f64,
    pub micro_detail: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualAuthorSurfacePlanV1 {
    pub schema_version: String,
    pub source_program_sha256: String,
    pub bindings: Vec<ForgeVisualAuthorSurfaceBindingExpandedV1>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualSemanticDensityEvidenceV1 {
    pub source_json_bytes: u32,
    pub template_node_count: u16,
    pub macro_count: u16,
    pub instance_count: u16,
    pub expanded_node_count: u16,
    pub expanded_output_count: u16,
    pub detail_motif_instance_count: u16,
    pub node_expansion_ratio_bps: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualAuthorLoweringV1 {
    pub schema_version: String,
    pub compiler_version: String,
    pub id_algorithm_version: String,
    pub source_program_sha256: String,
    pub expanded_geometry_source_sha256: String,
    pub expanded_geometry_dag_sha256: String,
    pub lineage_sha256: String,
    pub lineage: Vec<ForgeVisualAuthorSourceLineageV1>,
    pub shape_program_sha256: String,
    pub shape_program: Value,
    pub assembly_graph_sha256: String,
    pub assembly_graph: Value,
    pub surface_plan_sha256: String,
    pub surface_plan: ForgeVisualAuthorSurfacePlanV1,
    pub semantic_density: ForgeVisualSemanticDensityEvidenceV1,
}

/// Stable compiler seam consumed by VP204 and its recovery state machine.
/// Formal E005 uses the unified contract; the legacy branch is explicit and
/// retained only for existing local regressions and stored sessions.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct VisualRuntimeSourceLoweringV1 {
    pub source_contract_id: String,
    pub compiler_profile_id: String,
    pub source_program_sha256: String,
    pub expanded_program_sha256: String,
    pub shape_program_sha256: String,
    pub shape_program: Value,
}

pub fn lower_visual_runtime_source_v1(value: &Value) -> CoreResult<VisualRuntimeSourceLoweringV1> {
    match value.get("schema_version").and_then(Value::as_str) {
        Some(FORGE_VISUAL_AUTHOR_SOURCE_SCHEMA_VERSION) => {
            let lowering = lower_forge_visual_author_source_v1(value)?;
            Ok(VisualRuntimeSourceLoweringV1 {
                source_contract_id: FORGE_VISUAL_AUTHOR_SOURCE_SCHEMA_VERSION.into(),
                compiler_profile_id: FORGE_VISUAL_AUTHOR_COMPILER_VERSION.into(),
                source_program_sha256: lowering.source_program_sha256,
                expanded_program_sha256: lowering.expanded_geometry_source_sha256,
                shape_program_sha256: lowering.shape_program_sha256,
                shape_program: lowering.shape_program,
            })
        }
        Some("ForgeVisualGeometryProgram@2") => {
            let lowering = lower_forge_visual_geometry_program_v2(value)?;
            Ok(VisualRuntimeSourceLoweringV1 {
                source_contract_id: "ForgeVisualGeometryProgram@2".into(),
                compiler_profile_id: "forgecad-core-vp203.1".into(),
                source_program_sha256: lowering.source_program_sha256,
                expanded_program_sha256: lowering.expanded_dag.expanded_program_sha256,
                shape_program_sha256: lowering.shape_program_sha256,
                shape_program: lowering.shape_program,
            })
        }
        _ => Err(invalid(
            "FORGE_VISUAL_RUNTIME_SOURCE_CONTRACT_INVALID",
            "runtime source must declare an explicitly supported visual contract",
        )),
    }
}

#[derive(Clone)]
struct ResolvedInstance {
    count: u16,
    position: [f64; 3],
    rotation: [f64; 3],
    step: [f64; 3],
    parameter_ids: Vec<String>,
}

#[derive(Clone)]
struct ExpandedPartRecord {
    instance_id: String,
    repeat_index: u16,
    source_output_id: String,
    expanded_output_id: String,
    /// World frame already baked into the lowered ShapeProgram geometry.
    /// AssemblyGraph records this frame as durable metadata; consumers must
    /// not compose the authored local transform a second time.
    world_transform: RecipeTransform,
    parent: Option<AuthorPartRefV1>,
}

fn resolve_scalar(
    scalar: &AuthorScalarV1,
    parameters: &BTreeMap<&str, &ForgeVisualParameterV2>,
    expected: ForgeVisualParameterKindV2,
) -> CoreResult<(f64, Option<String>)> {
    match scalar {
        AuthorScalarV1::Literal(value) => {
            if value.is_finite() {
                Ok((*value, None))
            } else {
                Err(invalid(
                    "FORGE_VISUAL_R1_NUMBER_INVALID",
                    "author scalar must be finite",
                ))
            }
        }
        AuthorScalarV1::Parameter { parameter_id } => {
            let parameter = parameters.get(parameter_id.as_str()).ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_R1_PARAMETER_MISSING",
                    "author scalar references an unknown parameter",
                )
            })?;
            if parameter.kind != expected {
                return Err(invalid(
                    "FORGE_VISUAL_R1_PARAMETER_KIND_INVALID",
                    "author scalar parameter kind does not match the field",
                ));
            }
            let value = parameter.default.as_f64().ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_R1_PARAMETER_VALUE_INVALID",
                    "author scalar parameter default is not numeric",
                )
            })?;
            Ok((value, Some(parameter_id.clone())))
        }
    }
}

fn resolve_count(
    count: &AuthorCountV1,
    parameters: &BTreeMap<&str, &ForgeVisualParameterV2>,
) -> CoreResult<(u16, Option<String>)> {
    match count {
        AuthorCountV1::Literal(value) => Ok((*value, None)),
        AuthorCountV1::Parameter { parameter_id } => {
            let parameter = parameters.get(parameter_id.as_str()).ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_R1_PARAMETER_MISSING",
                    "repeat count references an unknown parameter",
                )
            })?;
            if parameter.kind != ForgeVisualParameterKindV2::Integer {
                return Err(invalid(
                    "FORGE_VISUAL_R1_PARAMETER_KIND_INVALID",
                    "repeat count requires an integer parameter",
                ));
            }
            let value = parameter.default.as_u64().ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_R1_PARAMETER_VALUE_INVALID",
                    "repeat count parameter default is not an unsigned integer",
                )
            })?;
            let value = u16::try_from(value).map_err(|_| {
                invalid(
                    "FORGE_VISUAL_R1_REPEAT_INVALID",
                    "repeat count exceeds the bounded u16 range",
                )
            })?;
            Ok((value, Some(parameter_id.clone())))
        }
    }
}

fn resolve_instance(
    instance: &AuthorInstanceV1,
    parameters: &BTreeMap<&str, &ForgeVisualParameterV2>,
    max_repeat_count: u16,
) -> CoreResult<ResolvedInstance> {
    let (count, count_parameter) = resolve_count(&instance.repeat.count, parameters)?;
    if count == 0 || count > max_repeat_count {
        return Err(invalid(
            "FORGE_VISUAL_R1_REPEAT_INVALID",
            "repeat count must be non-zero and within the declared budget",
        ));
    }
    let mut position = [0.0; 3];
    let mut rotation = [0.0; 3];
    let mut step = [0.0; 3];
    let mut parameter_ids = BTreeSet::new();
    if let Some(parameter_id) = count_parameter {
        parameter_ids.insert(parameter_id);
    }
    for axis in 0..3 {
        let (value, parameter) = resolve_scalar(
            &instance.transform.position[axis],
            parameters,
            ForgeVisualParameterKindV2::Length,
        )?;
        if value.abs() > 100_000.0 {
            return Err(invalid(
                "FORGE_VISUAL_R1_TRANSFORM_INVALID",
                "instance position exceeds the lightweight concept range",
            ));
        }
        position[axis] = value;
        if let Some(parameter) = parameter {
            parameter_ids.insert(parameter);
        }
        let (value, parameter) = resolve_scalar(
            &instance.transform.rotation[axis],
            parameters,
            ForgeVisualParameterKindV2::Angle,
        )?;
        if value.abs() > std::f64::consts::PI {
            return Err(invalid(
                "FORGE_VISUAL_R1_TRANSFORM_INVALID",
                "instance rotation must remain within one bounded Euler turn",
            ));
        }
        rotation[axis] = value;
        if let Some(parameter) = parameter {
            parameter_ids.insert(parameter);
        }
        let (value, parameter) = resolve_scalar(
            &instance.repeat.step[axis],
            parameters,
            ForgeVisualParameterKindV2::Length,
        )?;
        if value.abs() > 100_000.0 {
            return Err(invalid(
                "FORGE_VISUAL_R1_REPEAT_INVALID",
                "repeat step exceeds the lightweight concept range",
            ));
        }
        step[axis] = value;
        if let Some(parameter) = parameter {
            parameter_ids.insert(parameter);
        }
    }
    Ok(ResolvedInstance {
        count,
        position,
        rotation,
        step,
        parameter_ids: parameter_ids.into_iter().collect(),
    })
}

fn node_inputs(node: &Value) -> CoreResult<Vec<&str>> {
    let kind = node.get("kind").and_then(Value::as_str).ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_R1_TEMPLATE_INVALID",
            "geometry template node kind is missing",
        )
    })?;
    match kind {
        "mirror" | "array" | "part" | "material_zone" => Ok(vec![node
            .get("input_node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_R1_TEMPLATE_INVALID",
                    "geometry template input_node_id is missing",
                )
            })?]),
        "union" | "subtract" => Ok(node
            .get("input_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                invalid(
                    "FORGE_VISUAL_R1_TEMPLATE_INVALID",
                    "geometry template input_node_ids are missing",
                )
            })?
            .iter()
            .map(|value| {
                value.as_str().ok_or_else(|| {
                    invalid(
                        "FORGE_VISUAL_R1_TEMPLATE_INVALID",
                        "geometry template input node ID is invalid",
                    )
                })
            })
            .collect::<CoreResult<Vec<_>>>()?),
        _ => Ok(Vec::new()),
    }
}

fn collect_source_graph<'a>(
    node_id: &'a str,
    nodes: &BTreeMap<&'a str, &'a Value>,
    graph: &mut BTreeSet<&'a str>,
) -> CoreResult<()> {
    if graph.insert(node_id) {
        let node = nodes.get(node_id).ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_R1_TEMPLATE_INVALID",
                "macro output graph references an unknown node",
            )
        })?;
        for input in node_inputs(node)? {
            collect_source_graph(input, nodes, graph)?;
        }
    }
    Ok(())
}

fn expanded_id(prefix: &str, identity: &Value) -> CoreResult<String> {
    let digest = semantic_sha256(identity)?;
    Ok(format!("{prefix}r1_{}", &digest[..20]))
}

fn vec3(value: &Value, field: &str) -> CoreResult<[f64; 3]> {
    let values = value.as_array().ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_R1_TEMPLATE_INVALID",
            format!("{field} must be a three-number vector"),
        )
    })?;
    if values.len() != 3 {
        return Err(invalid(
            "FORGE_VISUAL_R1_TEMPLATE_INVALID",
            format!("{field} must be a three-number vector"),
        ));
    }
    let mut result = [0.0; 3];
    for axis in 0..3 {
        result[axis] = values[axis].as_f64().ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_R1_TEMPLATE_INVALID",
                format!("{field} must contain finite numbers"),
            )
        })?;
    }
    Ok(result)
}

fn rotate_vector(matrix: Matrix4, vector: [f64; 3]) -> CoreResult<[f64; 3]> {
    let rotation = rigid_rotation(matrix)?;
    Ok(std::array::from_fn(|row| {
        (0..3)
            .map(|column| rotation[row][column] * vector[column])
            .sum()
    }))
}

fn rewrite_node(
    source: &Value,
    id_map: &BTreeMap<String, String>,
    part_map: &BTreeMap<String, String>,
    zone_map: &BTreeMap<String, String>,
    world: Matrix4,
) -> CoreResult<Value> {
    let mut node = source.clone();
    let object = node.as_object_mut().ok_or_else(|| {
        invalid(
            "FORGE_VISUAL_R1_TEMPLATE_INVALID",
            "geometry template node must be an object",
        )
    })?;
    let source_node_id = object
        .get("node_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("FORGE_VISUAL_R1_TEMPLATE_INVALID", "node_id is missing"))?
        .to_string();
    object.insert("node_id".into(), json!(id_map[&source_node_id]));
    if let Some(input) = object.get_mut("input_node_id") {
        let source_id = input.as_str().ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_R1_TEMPLATE_INVALID",
                "input_node_id must be a string",
            )
        })?;
        *input = json!(id_map[source_id]);
    }
    if let Some(inputs) = object.get_mut("input_node_ids") {
        let source_ids = inputs.as_array().ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_R1_TEMPLATE_INVALID",
                "input_node_ids must be an array",
            )
        })?;
        *inputs = json!(source_ids
            .iter()
            .map(|value| id_map[value.as_str().expect("VP203 validated")].clone())
            .collect::<Vec<_>>());
    }
    if let Some(part_id) = object.get_mut("part_id") {
        let source_id = part_id.as_str().expect("VP203 validated");
        *part_id = json!(part_map[source_id]);
    }
    if let Some(zone_id) = object.get_mut("zone_id") {
        let source_id = zone_id.as_str().expect("VP203 validated");
        *zone_id = json!(zone_map[source_id]);
    }
    let kind = object
        .get("kind")
        .and_then(Value::as_str)
        .expect("VP203 validated")
        .to_string();
    if matches!(
        kind.as_str(),
        "box" | "extrude" | "revolve" | "loft" | "sweep"
    ) {
        let position = vec3(object.get("position").expect("VP203 validated"), "position")?;
        object.insert("position".into(), json!(transform_point(world, position)?));
        if kind == "sweep" {
            let points = object
                .get("path_points")
                .and_then(Value::as_array)
                .expect("VP203 validated")
                .iter()
                .map(|point| rotate_vector(world, vec3(point, "path_points")?))
                .collect::<CoreResult<Vec<_>>>()?;
            object.insert("path_points".into(), json!(points));
        }
    }
    Ok(node)
}

fn part_and_material_for_output(
    output_id: &str,
    outputs: &BTreeMap<&str, &Value>,
    nodes: &BTreeMap<&str, &Value>,
) -> CoreResult<(String, String, String, String)> {
    let output = outputs[output_id];
    let zone_node = nodes[output["node_id"].as_str().expect("VP203 validated")];
    let part_node = nodes[zone_node["input_node_id"]
        .as_str()
        .expect("VP203 validated")];
    Ok((
        part_node["part_id"]
            .as_str()
            .expect("VP203 validated")
            .to_string(),
        part_node["role"]
            .as_str()
            .expect("VP203 validated")
            .to_string(),
        zone_node["zone_id"]
            .as_str()
            .expect("VP203 validated")
            .to_string(),
        zone_node["material_id"]
            .as_str()
            .expect("VP203 validated")
            .to_string(),
    ))
}

fn world_matrix_for(
    instance_id: &str,
    repeat_index: u16,
    instances: &BTreeMap<&str, &AuthorInstanceV1>,
    resolved: &BTreeMap<&str, ResolvedInstance>,
    visiting: &mut BTreeSet<String>,
    memo: &mut BTreeMap<(String, u16), Matrix4>,
) -> CoreResult<Matrix4> {
    let key = (instance_id.to_string(), repeat_index);
    if let Some(matrix) = memo.get(&key) {
        return Ok(*matrix);
    }
    if !visiting.insert(instance_id.to_string()) {
        return Err(invalid(
            "FORGE_VISUAL_R1_ASSEMBLY_CYCLE",
            "author instance hierarchy contains a cycle",
        ));
    }
    let instance = instances[instance_id];
    let values = &resolved[instance_id];
    if repeat_index >= values.count {
        return Err(invalid(
            "FORGE_VISUAL_R1_PART_REF_INVALID",
            "part reference repeat_index is outside the expanded instance",
        ));
    }
    let local = RecipeTransform {
        position: std::array::from_fn(|axis| {
            values.position[axis] + values.step[axis] * f64::from(repeat_index)
        }),
        rotation: values.rotation,
        scale: [1.0; 3],
    };
    let local_matrix = transform_matrix(&local)?;
    let world = if let Some(parent) = &instance.parent {
        let parent_world = world_matrix_for(
            &parent.instance_id,
            parent.repeat_index,
            instances,
            resolved,
            visiting,
            memo,
        )?;
        multiply(parent_world, local_matrix)
    } else {
        local_matrix
    };
    visiting.remove(instance_id);
    memo.insert(key, world);
    Ok(world)
}

fn validate_budget(budget: &AuthorBudgetV1) -> CoreResult<()> {
    if budget.schema_version != "ForgeVisualAuthorBudget@1"
        || budget.max_parameters == 0
        || budget.max_parameters > 64
        || budget.max_macros == 0
        || budget.max_macros > 64
        || budget.max_instances == 0
        || budget.max_instances > 64
        || budget.max_repeat_count == 0
        || budget.max_repeat_count > 64
        || budget.max_expanded_nodes == 0
        || budget.max_expanded_nodes > 256
        || budget.max_expanded_parts == 0
        || budget.max_expanded_parts > 128
        || budget.max_expanded_outputs == 0
        || budget.max_expanded_outputs > 128
        || budget.max_operations == 0
        || budget.max_operations > 256
        || !(100..=100_000).contains(&budget.triangle_budget)
    {
        return Err(invalid(
            "FORGE_VISUAL_R1_BUDGET_INVALID",
            "author budget is outside the bounded R1 profile",
        ));
    }
    Ok(())
}

pub fn lower_forge_visual_author_source_v1(
    value: &Value,
) -> CoreResult<ForgeVisualAuthorLoweringV1> {
    let source: ForgeVisualAuthorSourceV1 =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(
                "FORGE_VISUAL_R1_SCHEMA_INVALID",
                format!("unified author source is invalid: {error}"),
            )
        })?;
    if source.schema_version != FORGE_VISUAL_AUTHOR_SOURCE_SCHEMA_VERSION {
        return Err(invalid(
            "FORGE_VISUAL_R1_SCHEMA_INVALID",
            "schema_version must be ForgeVisualAuthorSource@1",
        ));
    }
    require_prefixed_id(&source.program_id, "visual_", "program_id")?;
    if source.domain.is_empty() || source.domain.len() > 96 || source.seed > i32::MAX as u32 {
        return Err(invalid(
            "FORGE_VISUAL_R1_ENVELOPE_INVALID",
            "author domain or seed is outside the bounded envelope",
        ));
    }
    validate_budget(&source.budgets)?;
    if source.parameters.len() > source.budgets.max_parameters as usize
        || source.macros.is_empty()
        || source.macros.len() > source.budgets.max_macros as usize
        || source.instances.is_empty()
        || source.instances.len() > source.budgets.max_instances as usize
    {
        return Err(invalid(
            "FORGE_VISUAL_R1_BUDGET_EXCEEDED",
            "author source exceeds declared parameter/macro/instance budgets",
        ));
    }
    let mut parameter_ids = BTreeSet::new();
    for parameter in &source.parameters {
        parameter.validate()?;
        if !parameter_ids.insert(parameter.parameter_id.as_str()) {
            return Err(invalid(
                "FORGE_VISUAL_R1_PARAMETER_DUPLICATE",
                "author parameter IDs must be unique",
            ));
        }
    }
    let parameter_map = source
        .parameters
        .iter()
        .map(|parameter| (parameter.parameter_id.as_str(), parameter))
        .collect::<BTreeMap<_, _>>();

    let template_lowering = lower_forge_visual_geometry_program_v2(&source.geometry_templates)?;
    if source.geometry_templates["domain"].as_str() != Some(source.domain.as_str())
        || source.geometry_templates["units"] != json!("millimeter")
    {
        return Err(invalid(
            "FORGE_VISUAL_R1_TEMPLATE_ENVELOPE_MISMATCH",
            "geometry templates must share the author domain and units",
        ));
    }
    let template_nodes = source.geometry_templates["nodes"]
        .as_array()
        .expect("VP203 validated");
    let template_outputs = source.geometry_templates["outputs"]
        .as_array()
        .expect("VP203 validated");
    let node_map = template_nodes
        .iter()
        .map(|node| (node["node_id"].as_str().expect("VP203 validated"), node))
        .collect::<BTreeMap<_, _>>();
    let output_map = template_outputs
        .iter()
        .map(|output| {
            (
                output["output_id"].as_str().expect("VP203 validated"),
                output,
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut macro_ids = BTreeSet::new();
    let mut claimed_outputs = BTreeSet::new();
    let mut macro_map = BTreeMap::new();
    for author_macro in &source.macros {
        require_prefixed_id(&author_macro.macro_id, "macro_", "macro_id")?;
        if !macro_ids.insert(author_macro.macro_id.as_str())
            || author_macro.output_ids.is_empty()
            || author_macro.output_ids.len() > 32
        {
            return Err(invalid(
                "FORGE_VISUAL_R1_MACRO_INVALID",
                "author macro IDs and output lists must be unique and bounded",
            ));
        }
        for output_id in &author_macro.output_ids {
            require_prefixed_id(output_id, "output_", "macro.output_id")?;
            if !output_map.contains_key(output_id.as_str()) || !claimed_outputs.insert(output_id) {
                return Err(invalid(
                    "FORGE_VISUAL_R1_MACRO_OUTPUT_INVALID",
                    "each template output must exist and belong to only one macro",
                ));
            }
        }
        macro_map.insert(author_macro.macro_id.as_str(), author_macro);
    }
    if claimed_outputs.len() != template_outputs.len() {
        return Err(invalid(
            "FORGE_VISUAL_R1_TEMPLATE_ORPHAN",
            "every geometry template output must belong to one author macro",
        ));
    }

    let mut instance_ids = BTreeSet::new();
    let mut instance_map = BTreeMap::new();
    let mut resolved_instances = BTreeMap::new();
    for instance in &source.instances {
        require_prefixed_id(&instance.instance_id, "instance_", "instance_id")?;
        if !instance_ids.insert(instance.instance_id.as_str())
            || !macro_map.contains_key(instance.macro_id.as_str())
        {
            return Err(invalid(
                "FORGE_VISUAL_R1_INSTANCE_INVALID",
                "author instances must have unique IDs and a declared macro",
            ));
        }
        instance_map.insert(instance.instance_id.as_str(), instance);
        resolved_instances.insert(
            instance.instance_id.as_str(),
            resolve_instance(instance, &parameter_map, source.budgets.max_repeat_count)?,
        );
    }
    for instance in &source.instances {
        if let Some(parent) = &instance.parent {
            let parent_instance =
                instance_map
                    .get(parent.instance_id.as_str())
                    .ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_R1_PART_REF_INVALID",
                            "instance parent references an unknown instance",
                        )
                    })?;
            let parent_macro = macro_map[parent_instance.macro_id.as_str()];
            if !parent_macro.output_ids.contains(&parent.output_id)
                || parent.repeat_index >= resolved_instances[parent.instance_id.as_str()].count
            {
                return Err(invalid(
                    "FORGE_VISUAL_R1_PART_REF_INVALID",
                    "instance parent references an unavailable expanded part",
                ));
            }
        }
    }
    let root_instance = instance_map
        .get(source.root_part.instance_id.as_str())
        .ok_or_else(|| invalid("FORGE_VISUAL_R1_ROOT_INVALID", "root instance is missing"))?;
    if root_instance.parent.is_some()
        || source.root_part.repeat_index
            >= resolved_instances[root_instance.instance_id.as_str()].count
        || !macro_map[root_instance.macro_id.as_str()]
            .output_ids
            .contains(&source.root_part.output_id)
        || source
            .instances
            .iter()
            .filter(|instance| instance.parent.is_none())
            .count()
            != 1
    {
        return Err(invalid(
            "FORGE_VISUAL_R1_ROOT_INVALID",
            "author hierarchy must have exactly one root instance and valid root output",
        ));
    }
    let mut world_memo = BTreeMap::new();
    for instance in &source.instances {
        for repeat_index in 0..resolved_instances[instance.instance_id.as_str()].count {
            world_matrix_for(
                &instance.instance_id,
                repeat_index,
                &instance_map,
                &resolved_instances,
                &mut BTreeSet::new(),
                &mut world_memo,
            )?;
        }
    }

    let mut surface_keys = BTreeSet::new();
    let mut surface_map = BTreeMap::new();
    for binding in &source.surface_bindings {
        require_prefixed_id(&binding.binding_id, "surface_", "binding_id")?;
        require_prefixed_id(&binding.material_id, "mat_", "surface material_id")?;
        if !binding.edge_wear.is_finite()
            || !binding.micro_detail.is_finite()
            || !(0.0..=1.0).contains(&binding.edge_wear)
            || !(0.0..=1.0).contains(&binding.micro_detail)
        {
            return Err(invalid(
                "FORGE_VISUAL_R1_SURFACE_INVALID",
                "surface wear and micro detail must remain within 0..=1",
            ));
        }
        let author_macro = macro_map.get(binding.macro_id.as_str()).ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_R1_SURFACE_INVALID",
                "surface binding references an unknown macro",
            )
        })?;
        if !author_macro.output_ids.contains(&binding.output_id)
            || !surface_keys.insert((binding.macro_id.as_str(), binding.output_id.as_str()))
        {
            return Err(invalid(
                "FORGE_VISUAL_R1_SURFACE_INVALID",
                "surface binding target must be unique and belong to its macro",
            ));
        }
        let (_, _, _, template_material_id) =
            part_and_material_for_output(&binding.output_id, &output_map, &node_map)?;
        if template_material_id != binding.material_id {
            return Err(invalid(
                "FORGE_VISUAL_R1_SURFACE_MATERIAL_MISMATCH",
                "surface material must match the geometry template Material Zone",
            ));
        }
        surface_map.insert(
            (binding.macro_id.as_str(), binding.output_id.as_str()),
            binding,
        );
    }
    if surface_keys.len() != claimed_outputs.len() {
        return Err(invalid(
            "FORGE_VISUAL_R1_SURFACE_MISSING",
            "every macro output requires one typed surface binding",
        ));
    }

    let source_program_sha256 = semantic_sha256(&source)?;
    let mut expanded_nodes = Vec::new();
    let mut expanded_outputs = Vec::new();
    let mut lineage = Vec::new();
    let mut part_records = Vec::new();
    let mut rotation_by_expanded_node = BTreeMap::<String, [f64; 3]>::new();
    let mut axis_matrix_by_expanded_node = BTreeMap::<String, Matrix4>::new();
    let mut part_lookup = BTreeMap::<(String, u16, String), String>::new();
    let mut zone_lookup = BTreeMap::<String, String>::new();
    for instance in &source.instances {
        let author_macro = macro_map[instance.macro_id.as_str()];
        let resolved = &resolved_instances[instance.instance_id.as_str()];
        for repeat_index in 0..resolved.count {
            let world = world_memo[&(instance.instance_id.clone(), repeat_index)];
            let world_rotation = euler_xyz_from_rotation(rigid_rotation(world)?);
            let world_transform = RecipeTransform {
                position: [world[0][3], world[1][3], world[2][3]],
                rotation: world_rotation,
                scale: [1.0; 3],
            };
            let mut graph = BTreeSet::new();
            for output_id in &author_macro.output_ids {
                let terminal = output_map[output_id.as_str()]["node_id"]
                    .as_str()
                    .expect("VP203 validated");
                collect_source_graph(terminal, &node_map, &mut graph)?;
            }
            let id_map = graph
                .iter()
                .map(|source_node_id| {
                    Ok((
                        (*source_node_id).to_string(),
                        expanded_id(
                            "node_",
                            &json!({"instance":instance.instance_id,"repeat":repeat_index,"node":source_node_id}),
                        )?,
                    ))
                })
                .collect::<CoreResult<BTreeMap<_, _>>>()?;
            let mut part_map = BTreeMap::new();
            let mut zone_map = BTreeMap::new();
            for source_node_id in &graph {
                let node = node_map[source_node_id];
                if let Some(part_id) = node.get("part_id").and_then(Value::as_str) {
                    part_map.insert(
                        part_id.to_string(),
                        expanded_id(
                            "part_",
                            &json!({"instance":instance.instance_id,"repeat":repeat_index,"part":part_id}),
                        )?,
                    );
                }
                if let Some(zone_id) = node.get("zone_id").and_then(Value::as_str) {
                    zone_map.insert(
                        zone_id.to_string(),
                        expanded_id(
                            "zone_",
                            &json!({"instance":instance.instance_id,"repeat":repeat_index,"zone":zone_id}),
                        )?,
                    );
                }
            }
            for node in template_nodes {
                let source_node_id = node["node_id"].as_str().expect("VP203 validated");
                if !graph.contains(source_node_id) {
                    continue;
                }
                let rewritten = rewrite_node(node, &id_map, &part_map, &zone_map, world)?;
                let expanded_node_id = id_map[source_node_id].clone();
                let kind = node["kind"].as_str().expect("VP203 validated");
                if matches!(kind, "box" | "extrude" | "revolve" | "loft" | "sweep") {
                    rotation_by_expanded_node.insert(expanded_node_id.clone(), world_rotation);
                }
                if matches!(kind, "mirror" | "array") {
                    axis_matrix_by_expanded_node.insert(expanded_node_id.clone(), world);
                }
                expanded_nodes.push(rewritten);
            }
            for source_output_id in &author_macro.output_ids {
                let source_output = output_map[source_output_id.as_str()];
                let expanded_output_id = expanded_id(
                    "output_",
                    &json!({"instance":instance.instance_id,"repeat":repeat_index,"output":source_output_id}),
                )?;
                expanded_outputs.push(json!({
                    "output_id": expanded_output_id,
                    "node_id": id_map[source_output["node_id"].as_str().expect("VP203 validated")],
                }));
                let (source_part_id, _, source_zone_id, _) =
                    part_and_material_for_output(source_output_id, &output_map, &node_map)?;
                part_lookup.insert(
                    (
                        instance.instance_id.clone(),
                        repeat_index,
                        source_output_id.clone(),
                    ),
                    part_map[&source_part_id].clone(),
                );
                zone_lookup.insert(
                    expanded_output_id.clone(),
                    zone_map[&source_zone_id].clone(),
                );
                lineage.push(ForgeVisualAuthorSourceLineageV1 {
                    macro_id: author_macro.macro_id.clone(),
                    semantic_kind: author_macro.semantic_kind,
                    instance_id: instance.instance_id.clone(),
                    repeat_index,
                    source_output_id: source_output_id.clone(),
                    expanded_output_id: expanded_output_id.clone(),
                    source_node_ids: graph.iter().map(|id| (*id).to_string()).collect(),
                    expanded_node_ids: graph.iter().map(|id| id_map[*id].clone()).collect(),
                    parameter_ids: resolved.parameter_ids.clone(),
                });
                part_records.push(ExpandedPartRecord {
                    instance_id: instance.instance_id.clone(),
                    repeat_index,
                    source_output_id: source_output_id.clone(),
                    expanded_output_id,
                    world_transform: world_transform.clone(),
                    parent: instance.parent.clone(),
                });
            }
        }
    }
    if expanded_nodes.len() > source.budgets.max_expanded_nodes as usize
        || expanded_outputs.len() > source.budgets.max_expanded_outputs as usize
        || expanded_outputs.len() > source.budgets.max_expanded_parts as usize
    {
        return Err(invalid(
            "FORGE_VISUAL_R1_BUDGET_EXCEEDED",
            "expanded author source exceeds node/Part/output budgets",
        ));
    }
    lineage.sort_by(|left, right| left.expanded_output_id.cmp(&right.expanded_output_id));
    expanded_outputs
        .sort_by(|left, right| left["output_id"].as_str().cmp(&right["output_id"].as_str()));
    let expanded_geometry_source = json!({
        "schema_version":"ForgeVisualGeometryProgram@2",
        "program_id":source.program_id,
        "domain":source.domain,
        "units":"millimeter",
        "seed":source.seed,
        "materials":source.geometry_templates["materials"],
        "profiles":source.geometry_templates["profiles"],
        "section_sets":source.geometry_templates["section_sets"],
        "nodes":expanded_nodes,
        "outputs":expanded_outputs,
        "budgets":{
            "schema_version":"GeometryProgramBudget@1",
            "max_profiles":source.geometry_templates["profiles"].as_array().expect("VP203 validated").len(),
            "max_section_sets":source.geometry_templates["section_sets"].as_array().expect("VP203 validated").len(),
            "max_nodes":source.budgets.max_expanded_nodes,
            "max_parts":source.budgets.max_expanded_parts,
            "max_materials":source.geometry_templates["materials"].as_array().expect("VP203 validated").len(),
            "max_outputs":source.budgets.max_expanded_outputs,
            "max_operations":source.budgets.max_operations,
            "triangle_budget":source.budgets.triangle_budget,
        }
    });
    let expanded_geometry_source_sha256 = semantic_sha256(&expanded_geometry_source)?;
    let mut geometry_lowering = lower_forge_visual_geometry_program_v2(&expanded_geometry_source)?;
    let operations = geometry_lowering.shape_program["operations"]
        .as_array_mut()
        .expect("VP203 lowering");
    for operation in operations {
        let operation_id = operation["operation_id"].as_str().expect("VP203 lowering");
        let expanded_node_id = operation_id
            .strip_prefix("op_")
            .map(|suffix| format!("node_{suffix}"));
        let Some(expanded_node_id) = expanded_node_id else {
            continue;
        };
        let args = operation["args"].as_object_mut().expect("VP203 lowering");
        if let Some(rotation) = rotation_by_expanded_node.get(&expanded_node_id) {
            args.insert("rotation".into(), json!(rotation));
        }
        if let Some(matrix) = axis_matrix_by_expanded_node.get(&expanded_node_id) {
            if let Some(axis) = args.get("axis").cloned() {
                args.insert(
                    "axis".into(),
                    json!(rotate_vector(*matrix, vec3(&axis, "axis")?)?),
                );
            }
        }
    }
    geometry_lowering.shape_program =
        normalize_persisted_shape_program(&geometry_lowering.shape_program)?;
    geometry_lowering.shape_program_sha256 = semantic_sha256(&geometry_lowering.shape_program)?;

    let source_map_by_output = geometry_lowering
        .source_map
        .iter()
        .map(|entry| (entry.output_id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let root_part_id = part_lookup
        .get(&(
            source.root_part.instance_id.clone(),
            source.root_part.repeat_index,
            source.root_part.output_id.clone(),
        ))
        .ok_or_else(|| {
            invalid(
                "FORGE_VISUAL_R1_ROOT_INVALID",
                "expanded root part is missing",
            )
        })?
        .clone();
    let mut assembly_parts = Vec::new();
    for part in &part_records {
        let source_map = source_map_by_output[part.expanded_output_id.as_str()];
        let (_, role, _, _) =
            part_and_material_for_output(&part.source_output_id, &output_map, &node_map)?;
        let own_key = (
            part.instance_id.clone(),
            part.repeat_index,
            part.source_output_id.clone(),
        );
        let own_part_id = part_lookup[&own_key].clone();
        let parent_part_id = if own_part_id == root_part_id {
            None
        } else if let Some(parent) = &part.parent {
            Some(
                part_lookup
                    .get(&(
                        parent.instance_id.clone(),
                        parent.repeat_index,
                        parent.output_id.clone(),
                    ))
                    .ok_or_else(|| {
                        invalid(
                            "FORGE_VISUAL_R1_PART_REF_INVALID",
                            "expanded parent Part is missing",
                        )
                    })?
                    .clone(),
            )
        } else {
            Some(root_part_id.clone())
        };
        assembly_parts.push(json!({
            "part_id":own_part_id,
            "role":role,
            "parent_part_id":parent_part_id,
            "operation_id":source_map.terminal_operation_id,
            "output_id":part.expanded_output_id,
            "geometry_source":"shape_program",
            "material_zone_ids":[source_map.material_zone_id],
            "material_zones":[source_map.material_zone_id],
            // The ShapeProgram positions are already world-baked. Keep the
            // exact same world frame here for pivots, connectors and editing
            // lineage, matching the established Component Recipe contract.
            "transform":part.world_transform,
            "pivot":{"position":[0.0,0.0,0.0],"normal":[1.0,0.0,0.0],"up":[0.0,0.0,1.0]},
            "connectors":[],
            "joints":[],
            "locked":false,
            "editable_parameters":[],
            "editable_parameter_bindings":[],
            "provenance":"agent_generated",
        }));
    }
    assembly_parts.sort_by(|left, right| left["part_id"].as_str().cmp(&right["part_id"].as_str()));
    let assembly_graph = json!({
        "schema_version":"AssemblyGraph@1",
        "graph_id":expanded_id("graph_", &json!({"source":source_program_sha256}))?,
        "concept_id":expanded_id("asset_", &json!({"source":source_program_sha256}))?,
        "root_part_id":root_part_id,
        "parts":assembly_parts,
        "connections":[],
    });
    let assembly_graph_sha256 = semantic_sha256(&assembly_graph)?;

    let mut expanded_surface_bindings = Vec::new();
    for line in &lineage {
        let binding = surface_map[&(line.macro_id.as_str(), line.source_output_id.as_str())];
        let map = source_map_by_output[line.expanded_output_id.as_str()];
        expanded_surface_bindings.push(ForgeVisualAuthorSurfaceBindingExpandedV1 {
            binding_id: binding.binding_id.clone(),
            macro_id: line.macro_id.clone(),
            instance_id: line.instance_id.clone(),
            repeat_index: line.repeat_index,
            source_output_id: line.source_output_id.clone(),
            expanded_output_id: line.expanded_output_id.clone(),
            part_id: map.part_id.clone(),
            material_zone_id: zone_lookup[&line.expanded_output_id].clone(),
            material_id: binding.material_id.clone(),
            surface_profile: binding.surface_profile,
            edge_wear: binding.edge_wear,
            micro_detail: binding.micro_detail,
        });
    }
    expanded_surface_bindings
        .sort_by(|left, right| left.expanded_output_id.cmp(&right.expanded_output_id));
    let surface_plan = ForgeVisualAuthorSurfacePlanV1 {
        schema_version: "ForgeVisualAuthorSurfacePlan@1".into(),
        source_program_sha256: source_program_sha256.clone(),
        bindings: expanded_surface_bindings,
    };
    let surface_plan_sha256 = semantic_sha256(&surface_plan)?;
    let lineage_sha256 = semantic_sha256(&lineage)?;
    let source_json_bytes = crate::canonical_json(&source)?.len();
    let expanded_node_count = u16::try_from(
        expanded_geometry_source["nodes"]
            .as_array()
            .expect("constructed")
            .len(),
    )
    .map_err(|_| {
        invalid(
            "FORGE_VISUAL_R1_BUDGET_EXCEEDED",
            "expanded node count overflow",
        )
    })?;
    let template_node_count = u16::try_from(template_nodes.len()).map_err(|_| {
        invalid(
            "FORGE_VISUAL_R1_BUDGET_EXCEEDED",
            "template node count overflow",
        )
    })?;
    let expanded_output_count = u16::try_from(lineage.len()).map_err(|_| {
        invalid(
            "FORGE_VISUAL_R1_BUDGET_EXCEEDED",
            "expanded output count overflow",
        )
    })?;
    let detail_motif_instance_count = u16::try_from(
        lineage
            .iter()
            .filter(|entry| entry.semantic_kind == AuthorMacroSemanticKindV1::DetailMotif)
            .count(),
    )
    .map_err(|_| {
        invalid(
            "FORGE_VISUAL_R1_BUDGET_EXCEEDED",
            "detail motif count overflow",
        )
    })?;
    let semantic_density = ForgeVisualSemanticDensityEvidenceV1 {
        source_json_bytes: u32::try_from(source_json_bytes).map_err(|_| {
            invalid(
                "FORGE_VISUAL_R1_SOURCE_TOO_LARGE",
                "canonical author source exceeds the lightweight byte budget",
            )
        })?,
        template_node_count,
        macro_count: source.macros.len() as u16,
        instance_count: source.instances.len() as u16,
        expanded_node_count,
        expanded_output_count,
        detail_motif_instance_count,
        node_expansion_ratio_bps: u32::from(expanded_node_count)
            .saturating_mul(10_000)
            .checked_div(u32::from(template_node_count.max(1)))
            .unwrap_or(0),
    };
    if !valid_hash(&template_lowering.shape_program_sha256)
        || !valid_hash(&geometry_lowering.expanded_dag.expanded_dag_sha256)
    {
        return Err(invalid(
            "FORGE_VISUAL_R1_LOWERING_INVALID",
            "existing VP203 compiler returned invalid lineage hashes",
        ));
    }
    Ok(ForgeVisualAuthorLoweringV1 {
        schema_version: FORGE_VISUAL_AUTHOR_LOWERING_SCHEMA_VERSION.into(),
        compiler_version: FORGE_VISUAL_AUTHOR_COMPILER_VERSION.into(),
        id_algorithm_version: FORGE_VISUAL_AUTHOR_ID_ALGORITHM_VERSION.into(),
        source_program_sha256,
        expanded_geometry_source_sha256,
        expanded_geometry_dag_sha256: geometry_lowering.expanded_dag.expanded_dag_sha256,
        lineage_sha256,
        lineage,
        shape_program_sha256: geometry_lowering.shape_program_sha256,
        shape_program: geometry_lowering.shape_program,
        assembly_graph_sha256,
        assembly_graph,
        surface_plan_sha256,
        surface_plan,
        semantic_density,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../../../../packages/concept-spec/fixtures/e005-r1-unified-service-console.json"
        ))
        .unwrap()
    }

    #[test]
    fn e005_r1_unifies_parameter_macro_geometry_assembly_surface_and_motifs() {
        let lowering = lower_forge_visual_author_source_v1(&fixture()).unwrap();
        assert_eq!(
            lowering.schema_version,
            FORGE_VISUAL_AUTHOR_LOWERING_SCHEMA_VERSION
        );
        assert_eq!(lowering.semantic_density.macro_count, 3);
        assert_eq!(lowering.semantic_density.instance_count, 3);
        assert_eq!(lowering.semantic_density.expanded_output_count, 11);
        assert_eq!(lowering.semantic_density.detail_motif_instance_count, 10);
        assert!(lowering.semantic_density.node_expansion_ratio_bps > 10_000);
        assert_eq!(lowering.assembly_graph["schema_version"], "AssemblyGraph@1");
        assert_eq!(
            lowering.assembly_graph["parts"].as_array().unwrap().len(),
            11
        );
        assert_eq!(lowering.surface_plan.bindings.len(), 11);
        assert!(lowering.lineage.iter().any(|entry| entry
            .parameter_ids
            .contains(&"param_fastener_count".to_string())));
        assert!(lowering.shape_program["operations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|operation| operation["op"] == "loft"));
        assert!(valid_hash(&lowering.shape_program_sha256));
    }

    #[test]
    fn e005_r1_repeat_density_grows_without_linear_source_growth() {
        let source = fixture();
        let small = lower_forge_visual_author_source_v1(&source).unwrap();
        let mut large_source = source;
        large_source["parameters"][0]["default"] = json!(24);
        let large = lower_forge_visual_author_source_v1(&large_source).unwrap();
        assert!(
            large.semantic_density.expanded_output_count
                > small.semantic_density.expanded_output_count * 2
        );
        assert!(
            large.semantic_density.source_json_bytes
                <= small.semantic_density.source_json_bytes + 2
        );
        assert_eq!(
            large.semantic_density.template_node_count,
            small.semantic_density.template_node_count
        );
    }

    #[test]
    fn e005_r1_assembly_transform_matches_world_baked_hierarchy() {
        let mut source = fixture();
        source["instances"][0]["transform"]["position"] = json!([10.0, 20.0, 30.0]);
        let lowering = lower_forge_visual_author_source_v1(&source).unwrap();
        let parts = lowering.assembly_graph["parts"].as_array().unwrap();
        let shell = parts
            .iter()
            .find(|part| part["role"] == "primary_form")
            .unwrap();
        assert_eq!(shell["transform"]["position"], json!([10.0, 20.0, 30.0]));

        let first_fastener_output = lowering
            .lineage
            .iter()
            .find(|entry| entry.macro_id == "macro_fastener" && entry.repeat_index == 0)
            .unwrap()
            .expanded_output_id
            .as_str();
        let fastener = parts
            .iter()
            .find(|part| part["output_id"] == first_fastener_output)
            .unwrap();
        assert_eq!(
            fastener["transform"]["position"],
            json!([-65.0, -138.0, 122.0])
        );
    }

    #[test]
    fn e005_r1_hash_is_key_order_stable_and_semantic_changes_propagate() {
        let source = fixture();
        let mut reordered = source.as_object().unwrap().clone();
        let schema = reordered.remove("schema_version").unwrap();
        reordered.insert("schema_version".into(), schema);
        let left = lower_forge_visual_author_source_v1(&source).unwrap();
        let right = lower_forge_visual_author_source_v1(&Value::Object(reordered)).unwrap();
        assert_eq!(left.source_program_sha256, right.source_program_sha256);
        assert_eq!(left.shape_program_sha256, right.shape_program_sha256);
        let mut changed = source;
        changed["instances"][1]["repeat"]["step"][0] = json!(42.0);
        let changed = lower_forge_visual_author_source_v1(&changed).unwrap();
        assert_ne!(left.source_program_sha256, changed.source_program_sha256);
        assert_ne!(left.shape_program_sha256, changed.shape_program_sha256);
    }

    #[test]
    fn e005_r1_rejects_cycles_or_orphan_root_before_geometry_worker() {
        let mut cycle = fixture();
        cycle["instances"][0]["parent"] = json!({
            "instance_id":"instance_fasteners","repeat_index":0,"output_id":"output_fastener"
        });
        assert_eq!(
            lower_forge_visual_author_source_v1(&cycle)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_R1_ROOT_INVALID"
        );
        let mut second_root = fixture();
        second_root["instances"][1]
            .as_object_mut()
            .unwrap()
            .remove("parent");
        assert_eq!(
            lower_forge_visual_author_source_v1(&second_root)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_R1_ROOT_INVALID"
        );
    }

    #[test]
    fn e005_r1_rejects_surface_mismatch_and_expansion_budget() {
        let mut surface = fixture();
        surface["surface_bindings"][1]["material_id"] = json!("mat_graphite");
        assert_eq!(
            lower_forge_visual_author_source_v1(&surface)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_R1_SURFACE_MATERIAL_MISMATCH"
        );
        let mut budget = fixture();
        budget["budgets"]["max_expanded_outputs"] = json!(5);
        assert_eq!(
            lower_forge_visual_author_source_v1(&budget)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_R1_BUDGET_EXCEEDED"
        );
    }

    #[test]
    fn e005_r1_rejects_parameter_kind_and_unknown_template_capability() {
        let mut parameter = fixture();
        parameter["instances"][1]["repeat"]["count"] =
            json!({"parameter_id":"param_fastener_spacing"});
        assert_eq!(
            lower_forge_visual_author_source_v1(&parameter)
                .unwrap_err()
                .code(),
            "FORGE_VISUAL_R1_PARAMETER_KIND_INVALID"
        );
        let mut capability = fixture();
        capability["geometry_templates"]["nodes"][0]["kind"] = json!("arbitrary_script");
        assert!(lower_forge_visual_author_source_v1(&capability).is_err());
    }
}
