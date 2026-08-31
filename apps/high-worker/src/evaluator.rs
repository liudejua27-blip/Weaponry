//! Non-destructive Native High evaluator orchestration.
//!
//! This is a Worker-local seam, not a second Runtime state writer.  It keeps
//! source parts immutable and materializes only evaluated sibling parts.  The
//! Boolean route is an explicit Manifold C-ABI module (opt-in at build time),
//! while the default subdivision route is a ForgeCAD-owned CPU Catmull-Clark
//! evaluator with a deliberately small OpenSubdiv-compatible typed policy.
//! AuthoringMesh V2 uses the explicit stitched variant so shared authored
//! edges/vertices are evaluated once across the whole bounded manifold polygon
//! mesh.  The polygon source is evaluated into quads; it is never treated as
//! a replacement for the Runtime-owned authored topology.
//! OpenSubdiv itself is never loaded implicitly: selecting that backend returns
//! a typed unavailable error before any partial result is accepted.

use crate::module::{
    module_descriptors, ModuleAvailability, CPU_SUBDIVISION_MODULE_ID, MANIFOLD_MODULE_ID,
    OPENSUBDIV_MODULE_ID,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const REQUEST_SCHEMA_VERSION: &str = "HighEvaluatorRequest@1";
pub const RESULT_SCHEMA_VERSION: &str = "HighEvaluatorResult@1";
pub const OPERATION: &str = "forgecad.production.high-evaluator@1";
pub const SUBDIVISION_POLICY: &str = "opensubdiv-compatible-regular-quad-cpu@1";
pub const STITCHED_SUBDIVISION_POLICY: &str = "forgecad-owned-cpu-catmull-clark-stitched-polygon@2";
pub const EVALUATOR_CONTRACT_SCHEMA_VERSION: &str = "NativeHighEvaluatorContract@1";

const MAX_STEPS: usize = 16;
const MAX_PARTS: usize = 128;
const MAX_CONTROL_POINTS: usize = 256;
const MAX_STITCHED_CONTROL_POINTS: usize = 32_768;
const MAX_STITCHED_FACES: usize = 32_768;
const MAX_STITCHED_EDGES: usize = 65_536;
const MAX_STITCHED_FACE_DEGREE: usize = 32;
const MAX_OUTPUT_VERTICES: usize = 300_000;
const MAX_OUTPUT_TRIANGLES: usize = 600_000;
const MAX_COORDINATE_ABS_M: f32 = 100.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighEvaluatorError(pub String);

impl fmt::Display for HighEvaluatorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for HighEvaluatorError {}

impl From<serde_json::Error> for HighEvaluatorError {
    fn from(error: serde_json::Error) -> Self {
        Self(format!("HIGH_EVALUATOR_JSON_INVALID:{error}"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighEvaluatorRequest {
    pub schema_version: String,
    pub operation: String,
    pub source_mesh: HighEvaluatorSourceMesh,
    pub source_mesh_sha256: String,
    pub steps: Vec<HighEvaluatorStep>,
    pub budgets: HighEvaluatorBudgets,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighEvaluatorSourceMesh {
    pub schema_version: String,
    pub parts: Vec<HighEvaluatorPart>,
}

/// Versioned evaluator semantics emitted with every High result.  This is a
/// worker contract, not a Runtime state claim: the source binding and
/// provenance fields describe what the transient evaluated sibling was
/// derived from, while `non_destructive` makes the write boundary explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighEvaluatorContract {
    pub schema_version: String,
    pub policy: String,
    pub topology: String,
    pub continuity: String,
    pub boundary_policy: String,
    pub crease_policy: String,
    pub adaptive_policy: String,
    pub source_binding: String,
    pub provenance: String,
    pub deterministic_replay: String,
    pub non_destructive: bool,
    pub max_subdivision_levels: usize,
}

/// `operand_id` is an evaluator-local stable input identity.  Multiple
/// operands may belong to one semantic `part_id`; the Boolean policy below
/// requires that semantic identity to match and never silently merges Parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighEvaluatorPart {
    pub operand_id: String,
    pub part_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_node_ids: Vec<String>,
    pub source_node_id: String,
    pub material_zone_id: String,
    pub source_element_lineage: Vec<String>,
    pub positions_m: Vec<[f32; 3]>,
    pub indices: Vec<[u32; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HighEvaluatorStep {
    Boolean(HighBooleanStep),
    Subdivision(HighSubdivisionStep),
    StitchedSubdivision(HighStitchedSubdivisionStep),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighBooleanStep {
    pub step_id: String,
    pub operation: BooleanOperation,
    pub output_part_id: String,
    pub left_operand_id: String,
    pub right_operand_id: String,
    pub max_runtime_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BooleanOperation {
    Union,
    Difference,
    Intersection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighSubdivisionStep {
    pub step_id: String,
    pub backend: SubdivisionBackend,
    pub part_id: String,
    pub material_zone_id: String,
    pub u_points: usize,
    pub v_points: usize,
    pub control_points: Vec<[f32; 3]>,
    pub subdivision_levels: usize,
    pub max_triangles: usize,
}

/// A bounded manifold polygon mesh input for the CPU stitched subdivision
/// route. `source_vertex_ids` is position-aligned, `faces` is face-id-aligned,
/// and each source edge binds an opaque stable edge ID to its endpoint
/// indices. The worker derives adjacency from these bindings, so adjacent
/// authored faces share one evaluated edge point and one evaluated vertex
/// point. Every authored face must have degree 3..=32; the Catmull-Clark
/// result is a quad mesh after one or more levels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighStitchedSubdivisionStep {
    pub step_id: String,
    pub backend: SubdivisionBackend,
    pub part_id: String,
    pub material_zone_id: String,
    /// Complete source-node lineage for one semantic Part.  The scalar
    /// compatibility fields remain authoritative for legacy single-node
    /// inputs; composite Parts preserve this ordered set end to end.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_node_ids: Vec<String>,
    pub source_revision_id: String,
    pub source_revision_sha256: String,
    pub source_vertex_ids: Vec<String>,
    pub source_edges: Vec<HighStitchedEdgeBinding>,
    pub source_face_ids: Vec<String>,
    pub control_points: Vec<[f32; 3]>,
    pub faces: Vec<Vec<u32>>,
    pub subdivision_levels: usize,
    pub max_triangles: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighStitchedEdgeBinding {
    pub edge_id: String,
    pub vertex_indices: [u32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubdivisionBackend {
    CpuRegularQuad,
    Opensubdiv,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighEvaluatorBudgets {
    pub max_steps: usize,
    pub max_output_vertices: usize,
    pub max_output_triangles: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighEvaluatorStepResult {
    pub step_id: String,
    pub kind: String,
    pub module_id: String,
    pub availability: ModuleAvailability,
    pub status: String,
    pub output_part_id: String,
    pub output_vertex_count: usize,
    pub output_triangle_count: usize,
    pub output_sha256: String,
    pub error_code: Option<String>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighEvaluatedPart {
    pub output_part_id: String,
    pub part_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_node_ids: Vec<String>,
    pub source_node_id: String,
    pub material_zone_id: String,
    pub module_id: String,
    pub source_operand_ids: Vec<String>,
    pub source_element_lineage: Vec<String>,
    pub positions_m: Vec<[f32; 3]>,
    pub indices: Vec<[u32; 3]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighEvaluatorResult {
    pub schema_version: String,
    pub operation: String,
    pub source_mesh_sha256: String,
    pub evaluator_contract: HighEvaluatorContract,
    pub module_descriptors: Vec<crate::module::ForgeCadModuleDescriptor>,
    pub base_parts: Vec<HighEvaluatorPart>,
    pub evaluated_parts: Vec<HighEvaluatedPart>,
    pub step_results: Vec<HighEvaluatorStepResult>,
    pub base_triangle_count: usize,
    pub evaluated_triangle_count: usize,
    pub triangle_count: usize,
    pub replay_count: u32,
    pub replay_byte_exact: bool,
    pub non_destructive: bool,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub quality_status: String,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone)]
struct MeshOutput {
    positions: Vec<[f32; 3]>,
    indices: Vec<[u32; 3]>,
    source_operand_ids: Vec<String>,
    source_element_lineage: Vec<String>,
}

/// Parse, evaluate twice, and return canonical bytes for the dedicated High
/// evaluator transport.  This function performs no filesystem or process I/O.
pub fn run_json(input: &[u8]) -> Result<Vec<u8>, HighEvaluatorError> {
    let request: HighEvaluatorRequest = serde_json::from_slice(input)?;
    let result = evaluate(&request)?;
    Ok(crate::canonical_bytes(
        &serde_json::to_value(result).map_err(HighEvaluatorError::from)?,
    ))
}

pub fn evaluate(request: &HighEvaluatorRequest) -> Result<HighEvaluatorResult, HighEvaluatorError> {
    validate_request(request)?;
    let first = evaluate_once(request)?;
    let second = evaluate_once(request)?;
    let first_preimage = result_preimage(&first)?;
    let second_preimage = result_preimage(&second)?;
    if crate::canonical_bytes(&first_preimage) != crate::canonical_bytes(&second_preimage) {
        return Err(HighEvaluatorError(
            "HIGH_EVALUATOR_REPLAY_NON_DETERMINISTIC".to_owned(),
        ));
    }
    let digest = sha256_value(&first_preimage)?;
    let mut result = first;
    result.canonical_sha256 = digest;
    Ok(result)
}

fn validate_request(request: &HighEvaluatorRequest) -> Result<(), HighEvaluatorError> {
    if request.schema_version != REQUEST_SCHEMA_VERSION {
        return invalid("HIGH_EVALUATOR_REQUEST_SCHEMA_MISMATCH");
    }
    if request.operation != OPERATION {
        return invalid("HIGH_EVALUATOR_OPERATION_NOT_ALLOWED");
    }
    if request.source_mesh.schema_version != "HighEvaluatorSourceMesh@1" {
        return invalid("HIGH_EVALUATOR_SOURCE_SCHEMA_MISMATCH");
    }
    validate_sha(&request.source_mesh_sha256, "source_mesh_sha256")?;
    validate_sha(&request.canonical_sha256, "canonical_sha256")?;
    let expected_source_sha = sha256_value(&serde_json::to_value(&request.source_mesh)?)?;
    if request.source_mesh_sha256 != expected_source_sha {
        return invalid("HIGH_EVALUATOR_SOURCE_HASH_MISMATCH");
    }
    let mut preimage = serde_json::to_value(request)?;
    preimage["canonical_sha256"] = Value::String(String::new());
    if request.canonical_sha256 != sha256_value(&preimage)? {
        return invalid("HIGH_EVALUATOR_REQUEST_CANONICAL_MISMATCH");
    }
    if request.steps.is_empty() || request.steps.len() > MAX_STEPS {
        return invalid("HIGH_EVALUATOR_STEP_BUDGET_INVALID");
    }
    if request.budgets.max_steps == 0
        || request.budgets.max_steps > MAX_STEPS
        || request.budgets.max_output_vertices == 0
        || request.budgets.max_output_vertices > MAX_OUTPUT_VERTICES
        || request.budgets.max_output_triangles == 0
        || request.budgets.max_output_triangles > MAX_OUTPUT_TRIANGLES
    {
        return invalid("HIGH_EVALUATOR_BUDGET_INVALID");
    }
    if request.steps.len() > request.budgets.max_steps {
        return invalid("HIGH_EVALUATOR_STEP_BUDGET_EXCEEDED");
    }
    if request.source_mesh.parts.is_empty() || request.source_mesh.parts.len() > MAX_PARTS {
        return invalid("HIGH_EVALUATOR_SOURCE_PART_BUDGET_INVALID");
    }
    let mut operands = BTreeSet::new();
    for part in &request.source_mesh.parts {
        validate_part(part)?;
        if !operands.insert(part.operand_id.clone()) {
            return invalid("HIGH_EVALUATOR_DUPLICATE_OPERAND_ID");
        }
    }
    let mut steps = BTreeSet::new();
    for step in &request.steps {
        let step_id = match step {
            HighEvaluatorStep::Boolean(step) => {
                validate_id(&step.step_id, "boolean.step_id")?;
                if step.max_runtime_ms == 0 || step.max_runtime_ms > 10_000 {
                    return invalid("HIGH_EVALUATOR_BOOLEAN_RUNTIME_BUDGET_INVALID");
                }
                validate_id(&step.output_part_id, "boolean.output_part_id")?;
                validate_id(&step.left_operand_id, "boolean.left_operand_id")?;
                validate_id(&step.right_operand_id, "boolean.right_operand_id")?;
                if step.left_operand_id == step.right_operand_id {
                    return invalid("HIGH_EVALUATOR_BOOLEAN_OPERANDS_MUST_DIFFER");
                }
                &step.step_id
            }
            HighEvaluatorStep::Subdivision(step) => {
                validate_id(&step.step_id, "subdivision.step_id")?;
                validate_id(&step.part_id, "subdivision.part_id")?;
                validate_id(&step.material_zone_id, "subdivision.material_zone_id")?;
                validate_grid(step)?;
                &step.step_id
            }
            HighEvaluatorStep::StitchedSubdivision(step) => {
                validate_id(&step.step_id, "stitched_subdivision.step_id")?;
                validate_id(&step.part_id, "stitched_subdivision.part_id")?;
                validate_id(
                    &step.material_zone_id,
                    "stitched_subdivision.material_zone_id",
                )?;
                validate_id(
                    &step.source_revision_id,
                    "stitched_subdivision.source_revision_id",
                )?;
                validate_sha(
                    &step.source_revision_sha256,
                    "stitched_subdivision.source_revision_sha256",
                )?;
                if !step.source_node_ids.is_empty() {
                    validate_unique_ids(
                        &step.source_node_ids,
                        "stitched_subdivision.source_node_id",
                    )?;
                }
                validate_stitched_grid(step)?;
                &step.step_id
            }
        };
        if !steps.insert(step_id.clone()) {
            return invalid("HIGH_EVALUATOR_DUPLICATE_STEP_ID");
        }
    }
    Ok(())
}

fn validate_part(part: &HighEvaluatorPart) -> Result<(), HighEvaluatorError> {
    for (value, label) in [
        (&part.operand_id, "operand_id"),
        (&part.part_id, "part_id"),
        (&part.source_node_id, "source_node_id"),
        (&part.material_zone_id, "material_zone_id"),
    ] {
        validate_id(value, label)?;
    }
    if !part.source_node_ids.is_empty()
        && (part.source_node_ids.first() != Some(&part.source_node_id)
            || part.source_node_ids.len() > 16)
    {
        return invalid("HIGH_EVALUATOR_SOURCE_NODE_SET_INVALID");
    }
    if !part.source_node_ids.is_empty() {
        validate_unique_ids(&part.source_node_ids, "source_node_id")?;
    }
    if part.positions_m.len() < 3 || part.positions_m.len() > MAX_OUTPUT_VERTICES {
        return invalid("HIGH_EVALUATOR_SOURCE_VERTEX_BUDGET_INVALID");
    }
    if part.indices.is_empty() || part.indices.len() > MAX_OUTPUT_TRIANGLES {
        return invalid("HIGH_EVALUATOR_SOURCE_TRIANGLE_BUDGET_INVALID");
    }
    if part.source_element_lineage.is_empty() {
        return invalid("HIGH_EVALUATOR_SOURCE_LINEAGE_MISSING");
    }
    for position in &part.positions_m {
        if position
            .iter()
            .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE_ABS_M)
        {
            return invalid("HIGH_EVALUATOR_SOURCE_NON_FINITE_POSITION");
        }
    }
    for triangle in &part.indices {
        if triangle
            .iter()
            .any(|index| *index as usize >= part.positions_m.len())
        {
            return invalid("HIGH_EVALUATOR_SOURCE_INDEX_OUT_OF_RANGE");
        }
        if triangle[0] == triangle[1] || triangle[1] == triangle[2] || triangle[2] == triangle[0] {
            return invalid("HIGH_EVALUATOR_SOURCE_DEGENERATE_TRIANGLE");
        }
    }
    Ok(())
}

fn validate_grid(step: &HighSubdivisionStep) -> Result<(), HighEvaluatorError> {
    if step.u_points < 2 || step.u_points > 16 || step.v_points < 2 || step.v_points > 16 {
        return invalid("HIGH_EVALUATOR_SUBDIVISION_GRID_BUDGET_INVALID");
    }
    if step.control_points.len() != step.u_points.saturating_mul(step.v_points)
        || step.control_points.len() > MAX_CONTROL_POINTS
    {
        return invalid("HIGH_EVALUATOR_SUBDIVISION_CONTROL_POINT_COUNT_INVALID");
    }
    if step.subdivision_levels > 2 || step.max_triangles == 0 {
        return invalid("HIGH_EVALUATOR_SUBDIVISION_POLICY_INVALID");
    }
    if step.backend == SubdivisionBackend::Opensubdiv {
        // The compatibility contract is closed, but the actual upstream
        // library is intentionally not in this product build.
        return invalid("OPENSUBDIV_NOT_VENDORED_OR_LINKED");
    }
    for point in &step.control_points {
        if point
            .iter()
            .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE_ABS_M)
        {
            return invalid("HIGH_EVALUATOR_SUBDIVISION_NON_FINITE_CONTROL_POINT");
        }
    }
    let scale = 1usize << step.subdivision_levels;
    let evaluated_u = (step.u_points - 1)
        .checked_mul(scale)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            HighEvaluatorError("HIGH_EVALUATOR_SUBDIVISION_TOPOLOGY_OVERFLOW".to_owned())
        })?;
    let evaluated_v = (step.v_points - 1)
        .checked_mul(scale)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            HighEvaluatorError("HIGH_EVALUATOR_SUBDIVISION_TOPOLOGY_OVERFLOW".to_owned())
        })?;
    let triangles = (evaluated_u - 1)
        .checked_mul(evaluated_v - 1)
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| {
            HighEvaluatorError("HIGH_EVALUATOR_SUBDIVISION_TOPOLOGY_OVERFLOW".to_owned())
        })?;
    if triangles > step.max_triangles {
        return invalid("HIGH_EVALUATOR_SUBDIVISION_BUDGET_EXCEEDED");
    }
    Ok(())
}

fn validate_stitched_grid(step: &HighStitchedSubdivisionStep) -> Result<(), HighEvaluatorError> {
    if step.backend != SubdivisionBackend::CpuRegularQuad {
        return invalid("HIGH_EVALUATOR_STITCHED_BACKEND_UNAVAILABLE");
    }
    if !(1..=2).contains(&step.subdivision_levels) || step.max_triangles == 0 {
        return invalid("HIGH_EVALUATOR_STITCHED_POLICY_INVALID");
    }
    if step.control_points.len() < 3 || step.control_points.len() > MAX_STITCHED_CONTROL_POINTS {
        return invalid("HIGH_EVALUATOR_STITCHED_CONTROL_POINT_BUDGET_INVALID");
    }
    if step.faces.is_empty() || step.faces.len() > MAX_STITCHED_FACES {
        return invalid("HIGH_EVALUATOR_STITCHED_FACE_BUDGET_INVALID");
    }
    if step.source_vertex_ids.len() != step.control_points.len()
        || step.source_face_ids.len() != step.faces.len()
    {
        return invalid("HIGH_EVALUATOR_STITCHED_SOURCE_BINDING_COUNT_INVALID");
    }
    if step.source_edges.is_empty() || step.source_edges.len() > MAX_STITCHED_EDGES {
        return invalid("HIGH_EVALUATOR_STITCHED_EDGE_BUDGET_INVALID");
    }
    validate_unique_ids(
        &step.source_vertex_ids,
        "stitched_subdivision.source_vertex_ids",
    )?;
    validate_unique_ids(
        &step.source_face_ids,
        "stitched_subdivision.source_face_ids",
    )?;
    for point in &step.control_points {
        if point
            .iter()
            .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE_ABS_M)
        {
            return invalid("HIGH_EVALUATOR_STITCHED_NON_FINITE_CONTROL_POINT");
        }
    }

    let faces = step
        .faces
        .iter()
        .map(|face| face.iter().map(|index| *index as usize).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let topology = build_subdivision_topology(&step.control_points, &faces)?;
    let mut edge_ids = BTreeSet::new();
    let mut source_edge_keys = BTreeSet::new();
    for edge in &step.source_edges {
        validate_id(&edge.edge_id, "stitched_subdivision.edge_id")?;
        if !edge_ids.insert(edge.edge_id.clone()) {
            return invalid("HIGH_EVALUATOR_STITCHED_DUPLICATE_EDGE_ID");
        }
        if edge.vertex_indices[0] == edge.vertex_indices[1]
            || edge
                .vertex_indices
                .iter()
                .any(|index| *index as usize >= step.control_points.len())
        {
            return invalid("HIGH_EVALUATOR_STITCHED_EDGE_ENDPOINT_INVALID");
        }
        let key = (
            edge.vertex_indices[0].min(edge.vertex_indices[1]) as usize,
            edge.vertex_indices[0].max(edge.vertex_indices[1]) as usize,
        );
        if !source_edge_keys.insert(key) {
            return invalid("HIGH_EVALUATOR_STITCHED_DUPLICATE_EDGE_ENDPOINT");
        }
    }
    let derived_edge_keys = topology
        .edges
        .iter()
        .map(|edge| (edge.a.min(edge.b), edge.a.max(edge.b)))
        .collect::<BTreeSet<_>>();
    if source_edge_keys != derived_edge_keys {
        return invalid("HIGH_EVALUATOR_STITCHED_EDGE_BINDING_MISMATCH");
    }

    // One input polygon of degree n becomes n quads after the first level;
    // each later level multiplies the quad count by four.  The result is
    // always triangulated as two triangles per generated quad.
    let mut output_quads = step.faces.iter().try_fold(0usize, |sum, face| {
        sum.checked_add(face.len()).ok_or_else(|| {
            HighEvaluatorError("HIGH_EVALUATOR_STITCHED_TOPOLOGY_OVERFLOW".to_owned())
        })
    })?;
    for _ in 1..step.subdivision_levels {
        output_quads = output_quads.checked_mul(4).ok_or_else(|| {
            HighEvaluatorError("HIGH_EVALUATOR_STITCHED_TOPOLOGY_OVERFLOW".to_owned())
        })?;
    }
    let output_triangles = output_quads.checked_mul(2).ok_or_else(|| {
        HighEvaluatorError("HIGH_EVALUATOR_STITCHED_TOPOLOGY_OVERFLOW".to_owned())
    })?;
    if output_triangles > step.max_triangles {
        return invalid("HIGH_EVALUATOR_STITCHED_SUBDIVISION_BUDGET_EXCEEDED");
    }
    if output_triangles > MAX_OUTPUT_TRIANGLES {
        return invalid("HIGH_EVALUATOR_STITCHED_OUTPUT_BUDGET_EXCEEDED");
    }
    Ok(())
}

fn validate_unique_ids(values: &[String], label: &str) -> Result<(), HighEvaluatorError> {
    let mut ids = BTreeSet::new();
    for value in values {
        validate_id(value, label)?;
        if !ids.insert(value) {
            return invalid("HIGH_EVALUATOR_STITCHED_DUPLICATE_SOURCE_ID");
        }
    }
    Ok(())
}

fn evaluate_once(
    request: &HighEvaluatorRequest,
) -> Result<HighEvaluatorResult, HighEvaluatorError> {
    let mut operands = request
        .source_mesh
        .parts
        .iter()
        .map(|part| (part.operand_id.clone(), part.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut evaluated_parts = Vec::new();
    let mut step_results = Vec::new();
    let mut stitched_subdivision_used = false;
    for step in &request.steps {
        match step {
            HighEvaluatorStep::Boolean(step) => {
                let left = operands.get(&step.left_operand_id).ok_or_else(|| {
                    HighEvaluatorError("HIGH_EVALUATOR_BOOLEAN_LEFT_OPERAND_UNAVAILABLE".to_owned())
                })?;
                let right = operands.get(&step.right_operand_id).ok_or_else(|| {
                    HighEvaluatorError(
                        "HIGH_EVALUATOR_BOOLEAN_RIGHT_OPERAND_UNAVAILABLE".to_owned(),
                    )
                })?;
                if left.part_id != right.part_id || left.material_zone_id != right.material_zone_id
                {
                    return Err(HighEvaluatorError(
                        "HIGH_EVALUATOR_BOOLEAN_CROSS_PART_OR_MATERIAL_ZONE_UNAVAILABLE".to_owned(),
                    ));
                }
                let mesh = evaluate_manifold_boolean(left, right, step)?;
                let source_node_ids = merge_source_node_ids(left, right)?;
                let output = build_evaluated_part(
                    &step.output_part_id,
                    &left.part_id,
                    &format!("forgecad.high-evaluator.{}", step.step_id),
                    &left.material_zone_id,
                    MANIFOLD_MODULE_ID,
                    &source_node_ids,
                    mesh,
                )?;
                enforce_output_budget(
                    &output,
                    request.budgets.max_output_vertices,
                    request.budgets.max_output_triangles,
                )?;
                let output_sha256 = mesh_hash(&output.positions_m, &output.indices)?;
                step_results.push(HighEvaluatorStepResult {
                    step_id: step.step_id.clone(),
                    kind: "boolean".to_owned(),
                    module_id: MANIFOLD_MODULE_ID.to_owned(),
                    availability: ModuleAvailability::Active,
                    status: "passed".to_owned(),
                    output_part_id: output.output_part_id.clone(),
                    output_vertex_count: output.positions_m.len(),
                    output_triangle_count: output.indices.len(),
                    output_sha256,
                    error_code: None,
                    limitations: vec![
                        "same-semantic-part-only".to_owned(),
                        "evaluated-source-lineage-is-operand-run-only".to_owned(),
                        "runtime-candidate-not-created".to_owned(),
                    ],
                });
                operands.insert(
                    format!("output:{}", step.step_id),
                    HighEvaluatorPart {
                        operand_id: format!("output:{}", step.step_id),
                        part_id: output.part_id.clone(),
                        source_node_ids: output.source_node_ids.clone(),
                        source_node_id: output.source_node_id.clone(),
                        material_zone_id: output.material_zone_id.clone(),
                        source_element_lineage: output.source_element_lineage.clone(),
                        positions_m: output.positions_m.clone(),
                        indices: output.indices.clone(),
                    },
                );
                evaluated_parts.push(output);
            }
            HighEvaluatorStep::Subdivision(step) => {
                let module_id = match step.backend {
                    SubdivisionBackend::CpuRegularQuad => CPU_SUBDIVISION_MODULE_ID,
                    SubdivisionBackend::Opensubdiv => OPENSUBDIV_MODULE_ID,
                };
                let source = request
                    .source_mesh
                    .parts
                    .iter()
                    .find(|part| part.part_id == step.part_id)
                    .ok_or_else(|| {
                        HighEvaluatorError("HIGH_EVALUATOR_SUBDIVISION_PART_UNAVAILABLE".to_owned())
                    })?;
                let mesh = evaluate_cpu_subdivision(step, source)?;
                let source_node_ids = effective_source_node_ids(source)?;
                let output = build_evaluated_part(
                    &format!("subdivision:{}", step.step_id),
                    &step.part_id,
                    &source.source_node_id,
                    &step.material_zone_id,
                    module_id,
                    &source_node_ids,
                    mesh,
                )?;
                enforce_output_budget(
                    &output,
                    request.budgets.max_output_vertices,
                    request.budgets.max_output_triangles,
                )?;
                let output_sha256 = mesh_hash(&output.positions_m, &output.indices)?;
                step_results.push(HighEvaluatorStepResult {
                    step_id: step.step_id.clone(),
                    kind: "subdivision".to_owned(),
                    module_id: module_id.to_owned(),
                    availability: ModuleAvailability::Active,
                    status: "passed".to_owned(),
                    output_part_id: output.output_part_id.clone(),
                    output_vertex_count: output.positions_m.len(),
                    output_triangle_count: output.indices.len(),
                    output_sha256,
                    error_code: None,
                    limitations: vec![
                        SUBDIVISION_POLICY.to_owned(),
                        "regular-rectangular-open-quad-grid-only".to_owned(),
                        "limit-surface-not-evaluated".to_owned(),
                        "creases-and-adaptive-subdivision-unsupported".to_owned(),
                        "runtime-candidate-not-created".to_owned(),
                    ],
                });
                evaluated_parts.push(output);
            }
            HighEvaluatorStep::StitchedSubdivision(step) => {
                stitched_subdivision_used = true;
                let source = request
                    .source_mesh
                    .parts
                    .iter()
                    .find(|part| part.part_id == step.part_id)
                    .ok_or_else(|| {
                        HighEvaluatorError(
                            "HIGH_EVALUATOR_STITCHED_SUBDIVISION_PART_UNAVAILABLE".to_owned(),
                        )
                    })?;
                if source.material_zone_id != step.material_zone_id
                    || source.positions_m != step.control_points
                    || (!step.source_node_ids.is_empty()
                        && step.source_node_ids != effective_source_node_ids(source)?)
                {
                    return Err(HighEvaluatorError(
                        "HIGH_EVALUATOR_STITCHED_SOURCE_BINDING_MISMATCH".to_owned(),
                    ));
                }
                let mesh = evaluate_cpu_stitched_subdivision(step, source)?;
                let source_node_ids = effective_source_node_ids(source)?;
                let output = build_evaluated_part(
                    &format!("stitched-subdivision:{}", step.step_id),
                    &step.part_id,
                    &source.source_node_id,
                    &step.material_zone_id,
                    CPU_SUBDIVISION_MODULE_ID,
                    &source_node_ids,
                    mesh,
                )?;
                enforce_output_budget(
                    &output,
                    request.budgets.max_output_vertices,
                    request.budgets.max_output_triangles,
                )?;
                let output_sha256 = mesh_hash(&output.positions_m, &output.indices)?;
                step_results.push(HighEvaluatorStepResult {
                    step_id: step.step_id.clone(),
                    kind: "stitched_subdivision".to_owned(),
                    module_id: CPU_SUBDIVISION_MODULE_ID.to_owned(),
                    availability: ModuleAvailability::Active,
                    status: "passed".to_owned(),
                    output_part_id: output.output_part_id.clone(),
                    output_vertex_count: output.positions_m.len(),
                    output_triangle_count: output.indices.len(),
                    output_sha256,
                    error_code: None,
                    limitations: vec![
                        STITCHED_SUBDIVISION_POLICY.to_owned(),
                        "shared-edge-and-shared-vertex-indexing@2".to_owned(),
                        "manifold-polygon-3-to-32-with-boundary-only@2".to_owned(),
                        "catmull-clark-output-quads@2".to_owned(),
                        "limit-surface-not-evaluated@1".to_owned(),
                        "creases-and-adaptive-subdivision-unsupported@1".to_owned(),
                        "runtime-candidate-not-created@1".to_owned(),
                    ],
                });
                operands.insert(
                    format!("output:{}", step.step_id),
                    HighEvaluatorPart {
                        operand_id: format!("output:{}", step.step_id),
                        part_id: output.part_id.clone(),
                        source_node_ids: output.source_node_ids.clone(),
                        source_node_id: output.source_node_id.clone(),
                        material_zone_id: output.material_zone_id.clone(),
                        source_element_lineage: output.source_element_lineage.clone(),
                        positions_m: output.positions_m.clone(),
                        indices: output.indices.clone(),
                    },
                );
                evaluated_parts.push(output);
            }
        }
    }
    let base_triangle_count = request
        .source_mesh
        .parts
        .iter()
        .map(|part| part.indices.len())
        .sum::<usize>();
    let evaluated_triangle_count = evaluated_parts
        .iter()
        .map(|part| part.indices.len())
        .sum::<usize>();
    if base_triangle_count.saturating_add(evaluated_triangle_count)
        > request.budgets.max_output_triangles
    {
        return invalid("HIGH_EVALUATOR_OUTPUT_TRIANGLE_BUDGET_EXCEEDED");
    }
    Ok(HighEvaluatorResult {
        schema_version: RESULT_SCHEMA_VERSION.to_owned(),
        operation: OPERATION.to_owned(),
        source_mesh_sha256: request.source_mesh_sha256.clone(),
        evaluator_contract: if stitched_subdivision_used {
            stitched_evaluator_contract()
        } else {
            regular_evaluator_contract()
        },
        module_descriptors: module_descriptors(),
        base_parts: request.source_mesh.parts.clone(),
        evaluated_parts,
        step_results,
        base_triangle_count,
        evaluated_triangle_count,
        triangle_count: base_triangle_count + evaluated_triangle_count,
        replay_count: 2,
        replay_byte_exact: true,
        non_destructive: true,
        structural_status: "PASS_SOURCE_STRUCTURAL".to_owned(),
        visual_status: "NOT_RUN".to_owned(),
        human_status: "NOT_RUN".to_owned(),
        quality_status: "structural_only".to_owned(),
        runtime_write_performed: false,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        canonical_sha256: String::new(),
    })
}

fn evaluate_manifold_boolean(
    left: &HighEvaluatorPart,
    right: &HighEvaluatorPart,
    step: &HighBooleanStep,
) -> Result<MeshOutput, HighEvaluatorError> {
    #[cfg(feature = "manifold-backend")]
    {
        let operation = match step.operation {
            BooleanOperation::Union => "union",
            BooleanOperation::Difference => "difference",
            BooleanOperation::Intersection => "intersection",
        };
        let output = forgecad_geometry_worker::manifold_boolean_typed(
            &left.positions_m,
            &left.indices,
            &right.positions_m,
            &right.indices,
            operation,
            600_000,
            step.max_runtime_ms,
        )
        .map_err(|error| HighEvaluatorError(format!("HIGH_EVALUATOR_MANIFOLD_FAILED:{error}")))?;
        let mut lineage = left.source_element_lineage.clone();
        lineage.extend(right.source_element_lineage.clone());
        lineage.sort();
        lineage.dedup();
        lineage.push(format!("boolean-step:{}", step.step_id));
        return Ok(MeshOutput {
            positions: output.positions,
            indices: output.indices,
            source_operand_ids: vec![left.operand_id.clone(), right.operand_id.clone()],
            source_element_lineage: lineage,
        });
    }
    #[cfg(not(feature = "manifold-backend"))]
    {
        let _ = (left, right, step);
        Err(HighEvaluatorError(format!(
            "HIGH_EVALUATOR_MODULE_UNAVAILABLE:{MANIFOLD_MODULE_ID}:MANIFOLD_BACKEND_FEATURE_DISABLED"
        )))
    }
}

fn evaluate_cpu_subdivision(
    step: &HighSubdivisionStep,
    source: &HighEvaluatorPart,
) -> Result<MeshOutput, HighEvaluatorError> {
    let mut positions = step.control_points.clone();
    let mut faces = Vec::with_capacity((step.u_points - 1) * (step.v_points - 1));
    for row in 0..step.v_points - 1 {
        for column in 0..step.u_points - 1 {
            let a = row * step.u_points + column;
            let b = a + 1;
            let d = a + step.u_points;
            let c = d + 1;
            faces.push(vec![a, b, c, d]);
        }
    }
    for _ in 0..step.subdivision_levels {
        let next = catmull_clark_step(&positions, &faces)?;
        positions = next.0;
        faces = next.1;
    }
    let indices = triangulate_quad_faces(&positions, &faces)?;
    let mut lineage = source.source_element_lineage.clone();
    lineage.push(format!("subdivision-step:{}", step.step_id));
    lineage.sort();
    lineage.dedup();
    Ok(MeshOutput {
        positions,
        indices,
        source_operand_ids: vec![source.operand_id.clone()],
        source_element_lineage: lineage,
    })
}

fn evaluate_cpu_stitched_subdivision(
    step: &HighStitchedSubdivisionStep,
    source: &HighEvaluatorPart,
) -> Result<MeshOutput, HighEvaluatorError> {
    let mut faces = step
        .faces
        .iter()
        .map(|face| face.iter().map(|index| *index as usize).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    validate_stitched_positions_and_faces(&step.control_points, &faces, &step.source_edges)?;
    let mut positions = step.control_points.clone();
    for _ in 0..step.subdivision_levels {
        let next = catmull_clark_step(&positions, &faces)?;
        positions = next.0;
        faces = next.1;
    }
    let indices = triangulate_quad_faces(&positions, &faces)?;
    let mut lineage = source.source_element_lineage.clone();
    lineage.extend(
        step.source_vertex_ids
            .iter()
            .map(|id| format!("source-vertex:{id}")),
    );
    lineage.extend(
        step.source_edges
            .iter()
            .map(|edge| format!("source-edge:{}", edge.edge_id)),
    );
    lineage.extend(
        step.source_face_ids
            .iter()
            .map(|id| format!("source-face:{id}")),
    );
    lineage.push(format!("subdivision-step:{}", step.step_id));
    lineage.push(format!("source-revision:{}", step.source_revision_id));
    lineage.push(format!(
        "source-revision-sha256:{}",
        step.source_revision_sha256
    ));
    lineage.push(format!("subdivision-level:{}", step.subdivision_levels));
    lineage.push(STITCHED_SUBDIVISION_POLICY.to_owned());
    lineage.sort();
    lineage.dedup();
    Ok(MeshOutput {
        positions,
        indices,
        source_operand_ids: vec![source.operand_id.clone()],
        source_element_lineage: lineage,
    })
}

fn validate_stitched_positions_and_faces(
    positions: &[[f32; 3]],
    faces: &[Vec<usize>],
    source_edges: &[HighStitchedEdgeBinding],
) -> Result<(), HighEvaluatorError> {
    let topology = build_subdivision_topology(positions, faces)?;
    let mut source_edge_keys = BTreeSet::new();
    for edge in source_edges {
        let key = (
            edge.vertex_indices[0].min(edge.vertex_indices[1]) as usize,
            edge.vertex_indices[0].max(edge.vertex_indices[1]) as usize,
        );
        if !source_edge_keys.insert(key) {
            return invalid("HIGH_EVALUATOR_STITCHED_DUPLICATE_EDGE_ENDPOINT");
        }
    }
    let derived_edge_keys = topology
        .edges
        .iter()
        .map(|edge| (edge.a.min(edge.b), edge.a.max(edge.b)))
        .collect::<BTreeSet<_>>();
    if source_edge_keys != derived_edge_keys {
        return invalid("HIGH_EVALUATOR_STITCHED_EDGE_BINDING_MISMATCH");
    }
    Ok(())
}

/// Build a deterministic manifold polygon adjacency graph.  The graph is
/// rebuilt for every subdivision level so generated quads receive the same
/// strict checks as the authored mixed polygon source.
fn build_subdivision_topology(
    positions: &[[f32; 3]],
    faces: &[Vec<usize>],
) -> Result<SubdivisionTopology, HighEvaluatorError> {
    if positions.is_empty() || faces.is_empty() {
        return invalid("HIGH_EVALUATOR_STITCHED_EMPTY_MESH");
    }
    let mut edge_lookup = BTreeMap::<(usize, usize), usize>::new();
    let mut edges = Vec::<SubdivisionEdge>::new();
    let mut vertex_edges = vec![Vec::<usize>::new(); positions.len()];
    let mut vertex_faces = vec![Vec::<usize>::new(); positions.len()];
    let mut face_edges = Vec::<Vec<usize>>::with_capacity(faces.len());
    for (face_index, face) in faces.iter().enumerate() {
        if !(3..=MAX_STITCHED_FACE_DEGREE).contains(&face.len()) {
            return invalid("HIGH_EVALUATOR_STITCHED_FACE_DEGREE_INVALID");
        }
        if face.iter().any(|index| *index >= positions.len()) {
            return invalid("HIGH_EVALUATOR_STITCHED_FACE_INDEX_OUT_OF_RANGE");
        }
        let unique_vertices = face.iter().copied().collect::<BTreeSet<_>>();
        if unique_vertices.len() != face.len() {
            return invalid("HIGH_EVALUATOR_STITCHED_FACE_REPEATED_VERTEX");
        }
        if face.iter().enumerate().any(|(corner, left)| {
            let right = face[(corner + 1) % face.len()];
            length(subtract(positions[*left], positions[right])) <= 1.0e-8
        }) {
            return invalid("HIGH_EVALUATOR_STITCHED_DEGENERATE_EDGE");
        }
        let polygon_area = (1..face.len() - 1)
            .map(|corner| {
                length(cross(
                    subtract(positions[face[corner]], positions[face[0]]),
                    subtract(positions[face[corner + 1]], positions[face[0]]),
                )) * 0.5
            })
            .sum::<f32>();
        if !polygon_area.is_finite() || polygon_area <= 1.0e-8 {
            return invalid("HIGH_EVALUATOR_STITCHED_DEGENERATE_FACE");
        }
        for vertex in face {
            vertex_faces[*vertex].push(face_index);
        }
        let mut edge_ids = Vec::with_capacity(face.len());
        for corner in 0..face.len() {
            let left = face[corner];
            let right = face[(corner + 1) % face.len()];
            let key = (left.min(right), left.max(right));
            let edge_index = if let Some(existing) = edge_lookup.get(&key).copied() {
                let edge = edges.get_mut(existing).expect("edge lookup synchronized");
                if edge.faces.len() >= 2 {
                    return invalid("HIGH_EVALUATOR_STITCHED_NON_MANIFOLD");
                }
                if edge.directions[0] == (left, right) {
                    return invalid("HIGH_EVALUATOR_STITCHED_ORIENTATION_INVALID");
                }
                edge.faces.push(face_index);
                edge.directions.push((left, right));
                existing
            } else {
                if edges.len() == MAX_STITCHED_EDGES {
                    return invalid("HIGH_EVALUATOR_STITCHED_EDGE_BUDGET_EXCEEDED");
                }
                let edge_index = edges.len();
                edge_lookup.insert(key, edge_index);
                edges.push(SubdivisionEdge {
                    a: key.0,
                    b: key.1,
                    faces: vec![face_index],
                    directions: vec![(left, right)],
                });
                edge_index
            };
            edge_ids.push(edge_index);
            if !vertex_edges[left].contains(&edge_index) {
                vertex_edges[left].push(edge_index);
            }
            if !vertex_edges[right].contains(&edge_index) {
                vertex_edges[right].push(edge_index);
            }
        }
        face_edges.push(edge_ids);
    }
    if vertex_faces.iter().any(Vec::is_empty) {
        return invalid("HIGH_EVALUATOR_STITCHED_ORPHAN_VERTEX");
    }
    for vertex_index in 0..positions.len() {
        validate_vertex_fan(
            vertex_index,
            &vertex_edges[vertex_index],
            &vertex_faces[vertex_index],
            &edges,
        )?;
    }
    Ok(SubdivisionTopology {
        edges,
        vertex_edges,
        vertex_faces,
        face_edges,
    })
}

fn validate_vertex_fan(
    vertex_index: usize,
    incident_edges: &[usize],
    incident_faces: &[usize],
    edges: &[SubdivisionEdge],
) -> Result<(), HighEvaluatorError> {
    let boundary_edges = incident_edges
        .iter()
        .filter(|edge_index| edges[**edge_index].faces.len() == 1)
        .count();
    if boundary_edges > 2 {
        return invalid("HIGH_EVALUATOR_STITCHED_BOUNDARY_NON_MANIFOLD");
    }
    let expected_edges = incident_faces.len() + usize::from(boundary_edges > 0);
    if incident_edges.len() != expected_edges {
        return invalid("HIGH_EVALUATOR_STITCHED_VERTEX_FAN_INVALID");
    }
    let incident_face_set = incident_faces.iter().copied().collect::<BTreeSet<_>>();
    let mut adjacency = BTreeMap::<usize, BTreeSet<usize>>::new();
    for face in incident_faces {
        adjacency.entry(*face).or_default();
    }
    for edge_index in incident_edges {
        let edge = &edges[*edge_index];
        if edge.faces.len() == 2 {
            if !incident_face_set.contains(&edge.faces[0])
                || !incident_face_set.contains(&edge.faces[1])
            {
                return invalid("HIGH_EVALUATOR_STITCHED_VERTEX_FAN_EDGE_INVALID");
            }
            adjacency
                .entry(edge.faces[0])
                .or_default()
                .insert(edge.faces[1]);
            adjacency
                .entry(edge.faces[1])
                .or_default()
                .insert(edge.faces[0]);
        }
    }
    let connection_count = adjacency.values().map(BTreeSet::len).sum::<usize>() / 2;
    let expected_connections = if boundary_edges == 0 {
        incident_faces.len()
    } else {
        incident_faces.len().saturating_sub(1)
    };
    if connection_count != expected_connections {
        return invalid("HIGH_EVALUATOR_STITCHED_VERTEX_FAN_DISCONNECTED");
    }
    if boundary_edges == 0 {
        if adjacency.values().any(|neighbors| neighbors.len() != 2) {
            return invalid("HIGH_EVALUATOR_STITCHED_VERTEX_FAN_NOT_CLOSED");
        }
    } else if incident_faces.len() == 1 {
        if adjacency.values().any(|neighbors| !neighbors.is_empty()) {
            return invalid("HIGH_EVALUATOR_STITCHED_VERTEX_FAN_BOUNDARY_INVALID");
        }
    } else {
        let degree_one = adjacency
            .values()
            .filter(|neighbors| neighbors.len() == 1)
            .count();
        if degree_one != 2 || adjacency.values().any(|neighbors| neighbors.len() > 2) {
            return invalid("HIGH_EVALUATOR_STITCHED_VERTEX_FAN_BOUNDARY_INVALID");
        }
    }
    let start = incident_faces[0];
    let mut visited = BTreeSet::new();
    let mut stack = vec![start];
    while let Some(face) = stack.pop() {
        if !visited.insert(face) {
            continue;
        }
        if let Some(neighbors) = adjacency.get(&face) {
            stack.extend(neighbors.iter().copied());
        }
    }
    if visited.len() != incident_faces.len() {
        let _ = vertex_index;
        return invalid("HIGH_EVALUATOR_STITCHED_VERTEX_FAN_ORPHAN");
    }
    Ok(())
}

fn validate_no_orphan_vertices(
    positions: &[[f32; 3]],
    faces: &[Vec<usize>],
    context: &str,
) -> Result<(), HighEvaluatorError> {
    let mut used = vec![false; positions.len()];
    for face in faces {
        if face.len() != 4 {
            return invalid("HIGH_EVALUATOR_SUBDIVISION_OUTPUT_NOT_QUAD");
        }
        for index in face {
            if *index >= positions.len() {
                return invalid("HIGH_EVALUATOR_SUBDIVISION_OUTPUT_INDEX_OUT_OF_RANGE");
            }
            used[*index] = true;
        }
    }
    if used.iter().any(|is_used| !is_used) {
        return invalid(&format!("{context}_ORPHAN_OUTPUT_VERTEX"));
    }
    Ok(())
}

fn triangulate_quad_faces(
    positions: &[[f32; 3]],
    faces: &[Vec<usize>],
) -> Result<Vec<[u32; 3]>, HighEvaluatorError> {
    let mut indices = Vec::with_capacity(faces.len() * 2);
    for face in faces {
        if face.len() != 4 {
            return invalid("HIGH_EVALUATOR_SUBDIVISION_OUTPUT_NOT_QUAD");
        }
        let [a, b, c, d] = [face[0], face[1], face[2], face[3]];
        if [a, b, c, d].iter().any(|index| *index >= positions.len()) {
            return invalid("HIGH_EVALUATOR_SUBDIVISION_OUTPUT_INDEX_OUT_OF_RANGE");
        }
        let cross_a = cross(
            subtract(positions[b], positions[a]),
            subtract(positions[c], positions[a]),
        );
        let cross_b = cross(
            subtract(positions[c], positions[a]),
            subtract(positions[d], positions[a]),
        );
        if length(cross_a) <= 1.0e-8 || length(cross_b) <= 1.0e-8 {
            return invalid("HIGH_EVALUATOR_SUBDIVISION_DEGENERATE_OUTPUT");
        }
        indices.extend([
            [a as u32, b as u32, c as u32],
            [a as u32, c as u32, d as u32],
        ]);
    }
    Ok(indices)
}

fn regular_evaluator_contract() -> HighEvaluatorContract {
    HighEvaluatorContract {
        schema_version: EVALUATOR_CONTRACT_SCHEMA_VERSION.to_owned(),
        policy: SUBDIVISION_POLICY.to_owned(),
        topology: "regular-rectangular-open-quad-patch@1".to_owned(),
        continuity: "shared-within-patch-only@1".to_owned(),
        boundary_policy: "two-edge-open-patch-boundary@1".to_owned(),
        crease_policy: "creases-and-sharpness-not-input@1".to_owned(),
        adaptive_policy: "uniform-levels-only@1".to_owned(),
        source_binding: "source-part-and-source-lineage@1".to_owned(),
        provenance: "source-operand-lineage-plus-step-id@1".to_owned(),
        deterministic_replay: "canonical-json-double-evaluation@1".to_owned(),
        non_destructive: true,
        max_subdivision_levels: 2,
    }
}

fn stitched_evaluator_contract() -> HighEvaluatorContract {
    HighEvaluatorContract {
        schema_version: EVALUATOR_CONTRACT_SCHEMA_VERSION.to_owned(),
        policy: STITCHED_SUBDIVISION_POLICY.to_owned(),
        topology: "manifold-polygon-degree-3-to-32-with-boundary@2".to_owned(),
        continuity: "shared-edge-and-shared-vertex-indexing@2".to_owned(),
        boundary_policy: "smooth-manifold-boundary-two-edge-vertex-fan@2".to_owned(),
        crease_policy: "creases-rejected-no-sharpness-input@1".to_owned(),
        adaptive_policy: "uniform-levels-only@1".to_owned(),
        source_binding: "position-aligned-vertex-edge-face-stable-ids@2".to_owned(),
        provenance: "source-vertex-edge-face-lineage-plus-step-id@2".to_owned(),
        deterministic_replay: "canonical-json-double-evaluation@1".to_owned(),
        non_destructive: true,
        max_subdivision_levels: 2,
    }
}

#[derive(Debug, Clone)]
struct SubdivisionEdge {
    a: usize,
    b: usize,
    faces: Vec<usize>,
    directions: Vec<(usize, usize)>,
}

#[derive(Debug, Clone)]
struct SubdivisionTopology {
    edges: Vec<SubdivisionEdge>,
    vertex_edges: Vec<Vec<usize>>,
    vertex_faces: Vec<Vec<usize>>,
    face_edges: Vec<Vec<usize>>,
}

fn catmull_clark_step(
    positions: &[[f32; 3]],
    faces: &[Vec<usize>],
) -> Result<(Vec<[f32; 3]>, Vec<Vec<usize>>), HighEvaluatorError> {
    let topology = build_subdivision_topology(positions, faces)?;
    let edges = topology.edges;
    let vertex_edges = topology.vertex_edges;
    let vertex_faces = topology.vertex_faces;
    let face_edges = topology.face_edges;
    let face_points = faces
        .iter()
        .map(|face| {
            scale(
                face.iter()
                    .copied()
                    .map(|index| positions[index])
                    .fold([0.0; 3], add_many),
                1.0 / face.len() as f32,
            )
        })
        .collect::<Vec<_>>();
    let edge_points = edges
        .iter()
        .map(|edge| {
            let midpoint = scale(add(positions[edge.a], positions[edge.b]), 0.5);
            if edge.faces.len() == 1 {
                midpoint
            } else {
                scale(
                    add(
                        add(positions[edge.a], positions[edge.b]),
                        add(face_points[edge.faces[0]], face_points[edge.faces[1]]),
                    ),
                    0.25,
                )
            }
        })
        .collect::<Vec<_>>();
    let next_vertex_count = positions
        .len()
        .checked_add(edge_points.len())
        .and_then(|count| count.checked_add(face_points.len()))
        .ok_or_else(|| {
            HighEvaluatorError("HIGH_EVALUATOR_SUBDIVISION_TOPOLOGY_OVERFLOW".to_owned())
        })?;
    let next_face_count = faces.iter().try_fold(0usize, |sum, face| {
        sum.checked_add(face.len()).ok_or_else(|| {
            HighEvaluatorError("HIGH_EVALUATOR_SUBDIVISION_TOPOLOGY_OVERFLOW".to_owned())
        })
    })?;
    if next_vertex_count > MAX_OUTPUT_VERTICES
        || next_face_count
            .checked_mul(2)
            .is_none_or(|triangles| triangles > MAX_OUTPUT_TRIANGLES)
    {
        return invalid("HIGH_EVALUATOR_SUBDIVISION_OUTPUT_BUDGET_EXCEEDED");
    }
    let mut next_positions = Vec::with_capacity(next_vertex_count);
    for (vertex_index, position) in positions.iter().copied().enumerate() {
        let boundary_edges = vertex_edges[vertex_index]
            .iter()
            .copied()
            .filter(|edge_index| edges[*edge_index].faces.len() == 1)
            .collect::<Vec<_>>();
        let next = if !boundary_edges.is_empty() {
            if boundary_edges.len() != 2 {
                return invalid("HIGH_EVALUATOR_SUBDIVISION_BOUNDARY_VALENCE_UNSUPPORTED");
            }
            let neighbors = boundary_edges
                .iter()
                .map(|edge_index| {
                    let edge = &edges[*edge_index];
                    if edge.a == vertex_index {
                        edge.b
                    } else {
                        edge.a
                    }
                })
                .collect::<Vec<_>>();
            scale(
                add(
                    add(scale(position, 6.0), positions[neighbors[0]]),
                    positions[neighbors[1]],
                ),
                0.125,
            )
        } else {
            let valence = vertex_edges[vertex_index].len();
            if valence == 0 || vertex_faces[vertex_index].len() != valence {
                return invalid("HIGH_EVALUATOR_SUBDIVISION_IRREGULAR_VERTEX");
            }
            let face_average = scale(
                vertex_faces[vertex_index]
                    .iter()
                    .fold([0.0; 3], |sum, face_index| {
                        add(sum, face_points[*face_index])
                    }),
                1.0 / valence as f32,
            );
            let edge_average = scale(
                vertex_edges[vertex_index]
                    .iter()
                    .fold([0.0; 3], |sum, edge_index| {
                        let edge = &edges[*edge_index];
                        add(sum, scale(add(positions[edge.a], positions[edge.b]), 0.5))
                    }),
                1.0 / valence as f32,
            );
            scale(
                add(
                    add(face_average, scale(edge_average, 2.0)),
                    scale(position, valence as f32 - 3.0),
                ),
                1.0 / valence as f32,
            )
        };
        if next
            .iter()
            .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE_ABS_M)
        {
            return invalid("HIGH_EVALUATOR_SUBDIVISION_NON_FINITE_OUTPUT");
        }
        next_positions.push(next);
    }
    let edge_offset = next_positions.len();
    next_positions.extend(edge_points);
    let face_offset = next_positions.len();
    next_positions.extend(face_points);
    let mut next_faces = Vec::with_capacity(faces.iter().map(Vec::len).sum());
    for (face_index, face) in faces.iter().enumerate() {
        let face_point = face_offset + face_index;
        for corner_index in 0..face.len() {
            let edge_next = face_edges[face_index][corner_index];
            let edge_prev = face_edges[face_index][(corner_index + face.len() - 1) % face.len()];
            next_faces.push(vec![
                face[corner_index],
                edge_offset + edge_next,
                face_point,
                edge_offset + edge_prev,
            ]);
        }
    }
    validate_no_orphan_vertices(&next_positions, &next_faces, "HIGH_EVALUATOR_SUBDIVISION")?;
    Ok((next_positions, next_faces))
}

fn build_evaluated_part(
    output_part_id: &str,
    part_id: &str,
    source_node_id: &str,
    material_zone_id: &str,
    module_id: &str,
    source_node_ids: &[String],
    mesh: MeshOutput,
) -> Result<HighEvaluatedPart, HighEvaluatorError> {
    validate_id(output_part_id, "output_part_id")?;
    if mesh.positions.len() < 3 || mesh.indices.is_empty() {
        return invalid("HIGH_EVALUATOR_EMPTY_OUTPUT");
    }
    for triangle in &mesh.indices {
        if triangle
            .iter()
            .any(|index| *index as usize >= mesh.positions.len())
        {
            return invalid("HIGH_EVALUATOR_OUTPUT_INDEX_OUT_OF_RANGE");
        }
    }
    Ok(HighEvaluatedPart {
        output_part_id: output_part_id.to_owned(),
        part_id: part_id.to_owned(),
        source_node_ids: source_node_ids.to_vec(),
        source_node_id: source_node_id.to_owned(),
        material_zone_id: material_zone_id.to_owned(),
        module_id: module_id.to_owned(),
        source_operand_ids: mesh.source_operand_ids,
        source_element_lineage: mesh.source_element_lineage,
        positions_m: mesh.positions,
        indices: mesh.indices,
    })
}

fn effective_source_node_ids(part: &HighEvaluatorPart) -> Result<Vec<String>, HighEvaluatorError> {
    let values = if part.source_node_ids.is_empty() {
        vec![part.source_node_id.clone()]
    } else {
        part.source_node_ids.clone()
    };
    if values.is_empty() || values[0] != part.source_node_id {
        return invalid("HIGH_EVALUATOR_SOURCE_NODE_SET_INVALID");
    }
    validate_unique_ids(&values, "source_node_id")?;
    Ok(values)
}

fn merge_source_node_ids(
    left: &HighEvaluatorPart,
    right: &HighEvaluatorPart,
) -> Result<Vec<String>, HighEvaluatorError> {
    let mut values = effective_source_node_ids(left)?;
    for value in effective_source_node_ids(right)? {
        if !values.contains(&value) {
            values.push(value);
        }
    }
    Ok(values)
}

fn enforce_output_budget(
    output: &HighEvaluatedPart,
    max_vertices: usize,
    max_triangles: usize,
) -> Result<(), HighEvaluatorError> {
    if output.positions_m.len() > max_vertices || output.indices.len() > max_triangles {
        return invalid("HIGH_EVALUATOR_OUTPUT_BUDGET_EXCEEDED");
    }
    Ok(())
}

fn result_preimage(result: &HighEvaluatorResult) -> Result<Value, HighEvaluatorError> {
    let mut value = serde_json::to_value(result)?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(value)
}

fn mesh_hash(positions: &[[f32; 3]], indices: &[[u32; 3]]) -> Result<String, HighEvaluatorError> {
    sha256_value(&serde_json::json!({"positions_m":positions,"indices":indices}))
}

fn validate_id(value: &str, label: &str) -> Result<(), HighEvaluatorError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte))
    {
        return Err(HighEvaluatorError(format!(
            "HIGH_EVALUATOR_INVALID_ID:{label}"
        )));
    }
    Ok(())
}

fn validate_sha(value: &str, label: &str) -> Result<(), HighEvaluatorError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(HighEvaluatorError(format!(
            "HIGH_EVALUATOR_INVALID_SHA:{label}"
        )));
    }
    Ok(())
}

fn sha256_value(value: &Value) -> Result<String, HighEvaluatorError> {
    let digest = Sha256::digest(crate::canonical_bytes(value));
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn invalid<T>(message: &str) -> Result<T, HighEvaluatorError> {
    Err(HighEvaluatorError(message.to_owned()))
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn add_many(sum: [f32; 3], value: [f32; 3]) -> [f32; 3] {
    add(sum, value)
}

fn subtract(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(value: [f32; 3], factor: f32) -> [f32; 3] {
    [value[0] * factor, value[1] * factor, value[2] * factor]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(value: [f32; 3]) -> f32 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> HighEvaluatorSourceMesh {
        HighEvaluatorSourceMesh {
            schema_version: "HighEvaluatorSourceMesh@1".to_owned(),
            parts: vec![HighEvaluatorPart {
                operand_id: "receiver".to_owned(),
                part_id: "receiver".to_owned(),
                source_node_ids: vec!["receiver-source".to_owned()],
                source_node_id: "receiver-source".to_owned(),
                material_zone_id: "zone-metal".to_owned(),
                source_element_lineage: vec!["part:receiver".to_owned()],
                positions_m: vec![
                    [-1.0, -1.0, 0.0],
                    [1.0, -1.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [-1.0, 1.0, 0.0],
                ],
                indices: vec![[0, 1, 2], [0, 2, 3]],
            }],
        }
    }

    fn request(backend: SubdivisionBackend) -> HighEvaluatorRequest {
        let source_mesh = source();
        let mut request = HighEvaluatorRequest {
            schema_version: REQUEST_SCHEMA_VERSION.to_owned(),
            operation: OPERATION.to_owned(),
            source_mesh: source_mesh.clone(),
            source_mesh_sha256: sha256_value(&serde_json::to_value(&source_mesh).unwrap()).unwrap(),
            steps: vec![HighEvaluatorStep::Subdivision(HighSubdivisionStep {
                step_id: "subdivision-1".to_owned(),
                backend,
                part_id: "receiver".to_owned(),
                material_zone_id: "zone-metal".to_owned(),
                u_points: 2,
                v_points: 2,
                control_points: vec![
                    [-1.0, -1.0, 0.0],
                    [1.0, -1.0, 0.0],
                    [-1.0, 1.0, 0.0],
                    [1.0, 1.0, 0.0],
                ],
                subdivision_levels: 1,
                max_triangles: 32,
            })],
            budgets: HighEvaluatorBudgets {
                max_steps: 4,
                max_output_vertices: 1024,
                max_output_triangles: 2048,
            },
            canonical_sha256: String::new(),
        };
        let mut preimage = serde_json::to_value(&request).unwrap();
        preimage["canonical_sha256"] = Value::String(String::new());
        request.canonical_sha256 = sha256_value(&preimage).unwrap();
        request
    }

    fn mixed_polygon_request() -> HighEvaluatorRequest {
        let source_mesh = HighEvaluatorSourceMesh {
            schema_version: "HighEvaluatorSourceMesh@1".to_owned(),
            parts: vec![HighEvaluatorPart {
                operand_id: "mixed-source".to_owned(),
                part_id: "mixed-part".to_owned(),
                source_node_ids: vec!["mixed-node".to_owned()],
                source_node_id: "mixed-node".to_owned(),
                material_zone_id: "mixed-zone".to_owned(),
                source_element_lineage: vec!["mixed-source-lineage".to_owned()],
                positions_m: vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, 2.0, 0.0],
                ],
                indices: vec![[0, 1, 2], [0, 2, 3], [3, 2, 4]],
            }],
        };
        let mut request = HighEvaluatorRequest {
            schema_version: REQUEST_SCHEMA_VERSION.to_owned(),
            operation: OPERATION.to_owned(),
            source_mesh: source_mesh.clone(),
            source_mesh_sha256: sha256_value(&serde_json::to_value(&source_mesh).unwrap()).unwrap(),
            steps: vec![HighEvaluatorStep::StitchedSubdivision(
                HighStitchedSubdivisionStep {
                    step_id: "mixed-stitched-1".to_owned(),
                    backend: SubdivisionBackend::CpuRegularQuad,
                    part_id: "mixed-part".to_owned(),
                    material_zone_id: "mixed-zone".to_owned(),
                    source_node_ids: vec!["mixed-node".to_owned()],
                    source_revision_id: "mixed-node".to_owned(),
                    source_revision_sha256: "a".repeat(64),
                    source_vertex_ids: vec![
                        "v0".to_owned(),
                        "v1".to_owned(),
                        "v2".to_owned(),
                        "v3".to_owned(),
                        "v4".to_owned(),
                    ],
                    source_edges: vec![
                        HighStitchedEdgeBinding {
                            edge_id: "e01".to_owned(),
                            vertex_indices: [0, 1],
                        },
                        HighStitchedEdgeBinding {
                            edge_id: "e12".to_owned(),
                            vertex_indices: [1, 2],
                        },
                        HighStitchedEdgeBinding {
                            edge_id: "e23".to_owned(),
                            vertex_indices: [2, 3],
                        },
                        HighStitchedEdgeBinding {
                            edge_id: "e30".to_owned(),
                            vertex_indices: [3, 0],
                        },
                        HighStitchedEdgeBinding {
                            edge_id: "e24".to_owned(),
                            vertex_indices: [2, 4],
                        },
                        HighStitchedEdgeBinding {
                            edge_id: "e43".to_owned(),
                            vertex_indices: [4, 3],
                        },
                    ],
                    source_face_ids: vec!["f-quad".to_owned(), "f-tri".to_owned()],
                    control_points: source_mesh.parts[0].positions_m.clone(),
                    faces: vec![vec![0, 1, 2, 3], vec![3, 2, 4]],
                    subdivision_levels: 1,
                    max_triangles: 32,
                },
            )],
            budgets: HighEvaluatorBudgets {
                max_steps: 4,
                max_output_vertices: 1024,
                max_output_triangles: 2048,
            },
            canonical_sha256: String::new(),
        };
        let mut preimage = serde_json::to_value(&request).unwrap();
        preimage["canonical_sha256"] = Value::String(String::new());
        request.canonical_sha256 = sha256_value(&preimage).unwrap();
        request
    }

    #[test]
    fn cpu_subdivision_is_deterministic_and_non_destructive() {
        let request = request(SubdivisionBackend::CpuRegularQuad);
        let first = evaluate(&request).expect("CPU subdivision");
        let second = evaluate(&request).expect("CPU subdivision replay");
        assert_eq!(
            serde_json::to_value(&first).expect("first result JSON"),
            serde_json::to_value(&second).expect("second result JSON")
        );
        assert_eq!(first.base_triangle_count, 2);
        assert_eq!(first.evaluated_triangle_count, 8);
        assert!(first.non_destructive);
        assert!(!first.runtime_write_performed);
        assert_eq!(first.module_descriptors.len(), 3);
        assert_eq!(
            first
                .module_descriptors
                .iter()
                .find(|descriptor| descriptor.module_id == OPENSUBDIV_MODULE_ID)
                .expect("OpenSubdiv descriptor")
                .availability,
            ModuleAvailability::Unavailable
        );
    }

    #[test]
    fn opensubdiv_selection_fails_closed_before_evaluation() {
        let error = evaluate(&request(SubdivisionBackend::Opensubdiv))
            .expect_err("OpenSubdiv must be unavailable");
        assert_eq!(error.0, "OPENSUBDIV_NOT_VENDORED_OR_LINKED");
    }

    #[test]
    fn stitched_mixed_tri_quad_is_deterministic_and_has_no_orphans() {
        let request = mixed_polygon_request();
        let first = evaluate(&request).expect("mixed polygon evaluation");
        let second = evaluate(&request).expect("mixed polygon replay");
        assert_eq!(first, second);
        let output = &first.evaluated_parts[0];
        assert_eq!(output.source_node_id, "mixed-node");
        assert_eq!(output.positions_m.len(), 13);
        assert_eq!(output.indices.len(), 14);
        let used = output
            .indices
            .iter()
            .flat_map(|triangle| triangle.iter().copied())
            .collect::<BTreeSet<_>>();
        assert_eq!(used.len(), output.positions_m.len());
        assert_eq!(first.evaluator_contract.policy, STITCHED_SUBDIVISION_POLICY);
        assert_eq!(
            first.evaluator_contract.topology,
            "manifold-polygon-degree-3-to-32-with-boundary@2"
        );
        assert!(first.non_destructive);
        assert_eq!(first.visual_status, "NOT_RUN");
        assert_eq!(first.human_status, "NOT_RUN");
        assert_eq!(first.quality_status, "structural_only");
    }

    #[test]
    fn stitched_mixed_tri_quad_two_levels_respects_quad_expansion_budget() {
        let mut request = mixed_polygon_request();
        if let HighEvaluatorStep::StitchedSubdivision(step) = &mut request.steps[0] {
            step.subdivision_levels = 2;
            step.max_triangles = 56;
        }
        let mut preimage = serde_json::to_value(&request).unwrap();
        preimage["canonical_sha256"] = Value::String(String::new());
        request.canonical_sha256 = sha256_value(&preimage).unwrap();
        let result = evaluate(&request).expect("two-level mixed polygon evaluation");
        let output = &result.evaluated_parts[0];
        assert_eq!(output.indices.len(), 56);
        assert!(
            output
                .indices
                .iter()
                .flat_map(|triangle| triangle.iter().copied())
                .collect::<BTreeSet<_>>()
                .len()
                == output.positions_m.len()
        );
    }

    #[test]
    fn stitched_polygon_rejects_budget_and_degenerate_edges() {
        let mut budget = mixed_polygon_request();
        if let HighEvaluatorStep::StitchedSubdivision(step) = &mut budget.steps[0] {
            step.max_triangles = 13;
        }
        let mut preimage = serde_json::to_value(&budget).unwrap();
        preimage["canonical_sha256"] = Value::String(String::new());
        budget.canonical_sha256 = sha256_value(&preimage).unwrap();
        let error = evaluate(&budget).expect_err("triangle budget must fail closed");
        assert!(error.0.contains("BUDGET_EXCEEDED"), "{}", error.0);

        let mut degenerate = mixed_polygon_request();
        degenerate.source_mesh.parts[0].positions_m[1] = [0.0, 0.0, 0.0];
        if let HighEvaluatorStep::StitchedSubdivision(step) = &mut degenerate.steps[0] {
            step.control_points[1] = [0.0, 0.0, 0.0];
        }
        degenerate.source_mesh_sha256 =
            sha256_value(&serde_json::to_value(&degenerate.source_mesh).unwrap()).unwrap();
        let mut preimage = serde_json::to_value(&degenerate).unwrap();
        preimage["canonical_sha256"] = Value::String(String::new());
        degenerate.canonical_sha256 = sha256_value(&preimage).unwrap();
        let error = evaluate(&degenerate).expect_err("degenerate edge must fail closed");
        assert!(error.0.contains("DEGENERATE"), "{}", error.0);

        let mut boundary = mixed_polygon_request();
        if let HighEvaluatorStep::StitchedSubdivision(step) = &mut boundary.steps[0] {
            step.source_face_ids.push("f-boundary".to_owned());
            step.faces.push(vec![0, 4, 2]);
            step.source_edges.extend([
                HighStitchedEdgeBinding {
                    edge_id: "e04".to_owned(),
                    vertex_indices: [0, 4],
                },
                HighStitchedEdgeBinding {
                    edge_id: "e02".to_owned(),
                    vertex_indices: [0, 2],
                },
            ]);
        }
        let mut preimage = serde_json::to_value(&boundary).unwrap();
        preimage["canonical_sha256"] = Value::String(String::new());
        boundary.canonical_sha256 = sha256_value(&preimage).unwrap();
        let error = evaluate(&boundary).expect_err("boundary fan must fail closed");
        assert!(error.0.contains("BOUNDARY_NON_MANIFOLD"), "{}", error.0);
    }

    #[test]
    fn stitched_polygon_rejects_non_manifold_and_orphan_source() {
        let mut non_manifold = mixed_polygon_request();
        if let HighEvaluatorStep::StitchedSubdivision(step) = &mut non_manifold.steps[0] {
            step.source_face_ids.push("f-extra".to_owned());
            step.faces.push(vec![2, 3, 0]);
        }
        let mut preimage = serde_json::to_value(&non_manifold).unwrap();
        preimage["canonical_sha256"] = Value::String(String::new());
        non_manifold.canonical_sha256 = sha256_value(&preimage).unwrap();
        let error = evaluate(&non_manifold).expect_err("third edge incidence must fail closed");
        assert!(error.0.contains("NON_MANIFOLD"), "{}", error.0);

        let mut orphan = mixed_polygon_request();
        orphan.source_mesh.parts[0]
            .positions_m
            .push([2.0, 2.0, 0.0]);
        if let HighEvaluatorStep::StitchedSubdivision(step) = &mut orphan.steps[0] {
            step.source_vertex_ids.push("v5".to_owned());
            step.control_points.push([2.0, 2.0, 0.0]);
        }
        orphan.source_mesh_sha256 =
            sha256_value(&serde_json::to_value(&orphan.source_mesh).unwrap()).unwrap();
        let mut preimage = serde_json::to_value(&orphan).unwrap();
        preimage["canonical_sha256"] = Value::String(String::new());
        orphan.canonical_sha256 = sha256_value(&preimage).unwrap();
        let error = evaluate(&orphan).expect_err("orphan source vertex must fail closed");
        assert!(error.0.contains("ORPHAN_VERTEX"), "{}", error.0);
    }
}
