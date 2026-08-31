//! Closed `AuthoringMesh@2` to Native High projection.
//!
//! The Runtime-owned AuthoringMesh V2 kernel deliberately is not a dependency
//! of this standalone worker.  This module mirrors its serialized revision
//! contract at the process boundary, validates the original half-edge graph,
//! and projects original positions/faces into the existing non-destructive
//! High evaluator.  A Runtime-provided complete Part set is evaluated one
//! stitched step per Part; the legacy selected-revision projection remains a
//! compatibility path. Evaluated sidecars and caller supplied element
//! identities never become High authoring truth.

use crate::canonical_bytes;
use crate::evaluator::{
    evaluate as evaluate_high, HighEvaluatorBudgets, HighEvaluatorError, HighEvaluatorPart,
    HighEvaluatorRequest, HighEvaluatorResult, HighEvaluatorSourceMesh, HighEvaluatorStep,
    HighStitchedEdgeBinding, HighStitchedSubdivisionStep, SubdivisionBackend,
    STITCHED_SUBDIVISION_POLICY, SUBDIVISION_POLICY,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const REQUEST_SCHEMA_VERSION: &str = "AuthoringMeshV2HighRequest@1";
pub const RESULT_SCHEMA_VERSION: &str = "AuthoringMeshV2HighResult@2";
pub const READBACK_SCHEMA_VERSION: &str = "AuthoringMeshV2HighReadback@2";
/// Closed Runtime-to-Worker execution envelope.  It carries only the
/// immutable revision identity, the typed source parts and the fixed bounded
/// policy; evaluator steps are still derived inside this Worker module and
/// cannot be supplied by MCP or a caller.
pub const EXECUTION_REQUEST_SCHEMA_VERSION: &str = "AuthoringMeshV2HighExecutionRequest@2";
pub const EXECUTION_OPERATION: &str = "forgecad.production.authoring-mesh-v2-high-execute@1";
pub const OPERATION: &str = "forgecad.production.authoring-mesh-v2-high-evaluate@1";
pub const REVISION_SCHEMA_VERSION: &str = "AuthoringMeshRevision@2";
pub const ORIGINAL_SCHEMA_VERSION: &str = "AuthoringMesh@2";
pub const OPERATION_SCHEMA_VERSION: &str = "AuthoringMeshTopologyOperation@2";
pub const ID_POLICY: &str = "runtime-derived-lineage-operation-parent-stable-no-reuse@2";
pub const ORIGINAL_NAMESPACE: &str = "original";
pub const EVALUATED_NAMESPACE: &str = "evaluated";
pub const SOURCE_BINDING_SCHEMA_VERSION: &str = "AuthoringMeshV2SourceBinding@1";
pub const FOUNDATION_SOURCE_BINDING_SCHEMA_VERSION: &str =
    "AuthoringMeshV2FoundationSourceBinding@1";
pub const STITCHED_EVALUATOR_CONTRACT: &str = "forgecad-owned-cpu-catmull-clark-stitched-polygon@2";
pub const ALGORITHM: &str = "forgecad-authoring-mesh-v2-high@2|runtime-owned-execution-envelope|typed-source-binding|mixed-polygon-catmull-clark-levels-1-2|deterministic-double-evaluation|no-rng-no-time-no-network";

const MAX_ID_LENGTH: usize = 128;
const MAX_PARENTS: usize = 8;
const MAX_VERTICES: usize = 32_768;
const MAX_EDGES: usize = 65_536;
const MAX_HALF_EDGES: usize = 131_072;
const MAX_CORNERS: usize = 131_072;
const MAX_FACES: usize = 32_768;
const MAX_FACE_DEGREE: usize = 32;
const MAX_COORDINATE_M: f64 = 10.0;
const MIN_EDGE_LENGTH_M: f64 = 1.0e-7;
const MIN_FACE_AREA_M2: f64 = 1.0e-12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoringMeshV2HighError(pub String);

impl fmt::Display for AuthoringMeshV2HighError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AuthoringMeshV2HighError {}

impl From<serde_json::Error> for AuthoringMeshV2HighError {
    fn from(error: serde_json::Error) -> Self {
        Self(format!("AUTHORING_MESH_V2_HIGH_JSON_INVALID:{error}"))
    }
}

impl From<HighEvaluatorError> for AuthoringMeshV2HighError {
    fn from(error: HighEvaluatorError) -> Self {
        Self(format!(
            "AUTHORING_MESH_V2_HIGH_EVALUATOR_FAILED:{}",
            error.0
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    Vertex,
    Edge,
    HalfEdge,
    Corner,
    Face,
    Loop,
    Ring,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyOperationKind {
    SplitEdge,
    FaceExtrude,
    MoveVertices,
    OpenFrameNotch,
    RearStockVoidRailBow,
    RearStockVoidBoundaryBridge,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementRef {
    pub kind: ElementKind,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tombstone {
    pub element: ElementRef,
    pub retired_revision_index: u64,
    pub operation_lineage_sha256: String,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopologyOperation {
    pub schema_version: String,
    pub operation_id: String,
    pub kind: TopologyOperationKind,
    pub parent_revision_id: String,
    pub operation_lineage_sha256: String,
    pub source_elements: Vec<ElementRef>,
    pub generated_elements: Vec<ElementRef>,
    pub retired_elements: Vec<ElementRef>,
    pub tombstones: Vec<Tombstone>,
    pub locality_policy: String,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vertex {
    pub vertex_id: String,
    pub position_m: [f64; 3],
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edge {
    pub edge_id: String,
    pub vertex_ids: [String; 2],
    pub half_edge_ids: Vec<String>,
    pub boundary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HalfEdge {
    pub half_edge_id: String,
    pub origin_vertex_id: String,
    pub edge_id: String,
    pub face_id: String,
    pub corner_id: String,
    pub next_id: String,
    pub prev_id: String,
    pub twin_id: Option<String>,
    pub boundary: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corner {
    pub corner_id: String,
    pub half_edge_id: String,
    pub vertex_id: String,
    pub face_id: String,
    pub ordinal: u32,
    pub uv0: Option<[f64; 2]>,
    pub normal: Option<[f64; 3]>,
    pub tangent: Option<[f64; 4]>,
    pub seam: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Face {
    pub face_id: String,
    pub half_edge_ids: Vec<String>,
    pub loop_id: String,
    pub boundary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopRecord {
    pub loop_id: String,
    pub face_id: String,
    pub half_edge_ids: Vec<String>,
    pub boundary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ring {
    pub ring_id: String,
    pub edge_ids: Vec<String>,
    pub closed: bool,
    pub boundary: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Original {
    pub namespace: String,
    pub lineage_id: String,
    pub vertices: Vec<Vertex>,
    pub edges: Vec<Edge>,
    pub half_edges: Vec<HalfEdge>,
    pub corners: Vec<Corner>,
    pub faces: Vec<Face>,
    pub loops: Vec<LoopRecord>,
    pub rings: Vec<Ring>,
    pub tombstones: Vec<Tombstone>,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluatedSidecar {
    pub namespace: String,
    pub source_revision_id: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub readback_sha256: String,
    pub correspondence_status: String,
    pub canonical_sha256: String,
}

/// Runtime-derived candidate provenance copied into the Worker request as a
/// typed value.  The Worker validates its shape/hash and uses it only for
/// lineage; it never treats candidate data as authored topology.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2SourceBinding {
    pub schema_version: String,
    pub project_id: String,
    pub candidate_id: String,
    pub candidate_state_sha256: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub artifact_readback_sha256: String,
    pub geometry_program_sha256: String,
    pub source_node_id: String,
    pub part_id: String,
    pub material_zone_id: String,
    pub solid: bool,
    pub source_operator_id: String,
    pub source_parameters_sha256: String,
    pub part_output_sha256: String,
    pub position_m: [f64; 3],
    pub rotation_rad: [f64; 3],
    pub canonical_sha256: String,
}

/// Runtime-derived foundation provenance.  It is deliberately separate from
/// candidate source binding: an imported foundation is not a GeometryProgram
/// candidate and must not gain candidate semantics at the Worker boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2FoundationSourceBinding {
    pub schema_version: String,
    pub project_id: String,
    pub materialization_id: String,
    pub record_id: String,
    pub foundation_request_id: String,
    pub foundation_request_sha256: String,
    pub foundation_result_object_sha256: String,
    pub topology_object_sha256: String,
    pub socket_map_object_sha256: String,
    pub rig_map_object_sha256: String,
    pub fps_presentation_package_object_sha256: String,
    pub source_asset_id: String,
    pub source_asset_sha256: String,
    pub source_asset_role: String,
    pub part_id: String,
    pub material_zone_id: String,
    pub source_part_topology_sha256: String,
    pub authoring_mesh_id: String,
    pub authoring_mesh_lineage_id: String,
    pub authoring_mesh_revision_id: String,
    pub binding_policy: String,
    pub materialization_profile: String,
    pub source_only: bool,
    pub quality_status: String,
    pub review_status: String,
    pub canonicalization_policy: String,
    pub canonical_sha256: String,
}

/// Exact JSON projection of the Runtime-owned `AuthoringMeshRevision@2`.
/// This is a worker input type only; it never writes or mutates the Runtime
/// kernel and deliberately keeps the evaluated sidecar separate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2Revision {
    pub schema_version: String,
    pub mesh_id: String,
    pub lineage_id: String,
    pub revision_id: String,
    pub parent_revision_ids: Vec<String>,
    pub revision_index: u64,
    pub operation: Option<TopologyOperation>,
    pub original: Original,
    pub evaluated: Option<EvaluatedSidecar>,
    #[serde(default)]
    pub source_binding: Option<AuthoringMeshV2SourceBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foundation_source_binding: Option<AuthoringMeshV2FoundationSourceBinding>,
    pub id_policy: String,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2HighPartInput {
    /// The explicit source order is part of the GLB/primitive contract.  It
    /// must be contiguous and is never sorted by the Worker.
    pub part_index: u32,
    pub operand_id: String,
    pub part_id: String,
    /// Complete source-node lineage for this semantic Part.  The first item
    /// is mirrored by `source_node_id` for compatibility with the original
    /// single-node wire shape; a Part may retain multiple independent source
    /// nodes without being split into fake semantic Parts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_node_ids: Vec<String>,
    pub source_node_id: String,
    pub material_zone_id: String,
    /// Runtime-preserved source element lineage.  The Worker only appends
    /// derived typed lineage; it does not replace these identities.
    pub source_element_lineage: Vec<String>,
    /// Exact GeometryProgram `part_outputs[*]` semantic hash.
    pub source_part_output_sha256: String,
    /// Vertex/face identity arrays and polygon topology are position aligned.
    pub source_vertex_ids: Vec<String>,
    pub source_edges: Vec<HighStitchedEdgeBinding>,
    pub source_face_ids: Vec<String>,
    pub control_points: Vec<[f32; 3]>,
    pub faces: Vec<Vec<u32>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2HighRequest {
    pub schema_version: String,
    pub operation: String,
    pub revision: AuthoringMeshV2Revision,
    pub revision_sha256: String,
    /// Empty is retained only for the legacy one-part source-only request;
    /// Runtime execution uses a complete ordered part set.
    #[serde(default)]
    pub part_inputs: Vec<AuthoringMeshV2HighPartInput>,
    pub steps: Vec<HighEvaluatorStep>,
    pub budgets: HighEvaluatorBudgets,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2HighExecutionRequest {
    pub schema_version: String,
    pub operation: String,
    pub revision: AuthoringMeshV2Revision,
    pub revision_sha256: String,
    /// Complete ordered source part set materialized by Runtime.  No paths,
    /// scripts, URLs or evaluator steps cross this boundary.
    pub part_inputs: Vec<AuthoringMeshV2HighPartInput>,
    pub subdivision_levels: usize,
    pub max_triangles_per_face: usize,
    pub budgets: HighEvaluatorBudgets,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2HighReadback {
    pub schema_version: String,
    pub mesh_id: String,
    pub lineage_id: String,
    pub revision_id: String,
    pub revision_sha256: String,
    pub projected_source_mesh_sha256: String,
    pub source_vertex_count: usize,
    pub source_triangle_count: usize,
    pub evaluated_part_count: usize,
    pub evaluated_triangle_count: usize,
    pub high_evaluation_sha256: String,
    pub high_worker_algorithm_sha256: String,
    pub replay_count: u32,
    pub replay_byte_exact: bool,
    pub non_destructive: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub limitations: Vec<String>,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2HighResult {
    pub schema_version: String,
    pub operation: String,
    pub mesh_id: String,
    pub lineage_id: String,
    pub revision_id: String,
    pub revision_index: u64,
    pub revision_sha256: String,
    pub high_worker_algorithm_sha256: String,
    pub source_mesh: HighEvaluatorSourceMesh,
    pub evaluation: HighEvaluatorResult,
    pub readback: AuthoringMeshV2HighReadback,
    pub replay_count: u32,
    pub replay_byte_exact: bool,
    pub non_destructive: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub quality_status: String,
    pub limitations: Vec<String>,
    pub canonical_sha256: String,
}

/// Validate the V2 revision and project its original topology to one
/// deterministic evaluator source part.  The input arrays are never mutated;
/// BTreeMaps are used only for lookup and stable projection order.
pub fn project_revision(
    revision: &AuthoringMeshV2Revision,
) -> Result<HighEvaluatorSourceMesh, AuthoringMeshV2HighError> {
    validate_revision(revision)?;

    let vertices = revision
        .original
        .vertices
        .iter()
        .map(|vertex| {
            to_position(vertex.position_m, "vertex")
                .map(|position| (vertex.vertex_id.clone(), position))
        })
        .collect::<Result<BTreeMap<_, _>, AuthoringMeshV2HighError>>()?;
    let vertex_index = vertices
        .keys()
        .enumerate()
        .map(|(index, id)| (id.clone(), index as u32))
        .collect::<BTreeMap<_, _>>();

    let half_edges = revision
        .original
        .half_edges
        .iter()
        .map(|half_edge| (half_edge.half_edge_id.as_str(), half_edge))
        .collect::<BTreeMap<_, _>>();
    let faces = revision
        .original
        .faces
        .iter()
        .map(|face| (face.face_id.as_str(), face))
        .collect::<BTreeMap<_, _>>();
    let mut indices = Vec::new();
    for face in faces.values() {
        let face_vertices = face
            .half_edge_ids
            .iter()
            .map(|half_edge_id| {
                let half_edge = half_edges
                    .get(half_edge_id.as_str())
                    .ok_or_else(|| error("PROJECTION_HALF_EDGE_MISSING"))?;
                vertex_index
                    .get(&half_edge.origin_vertex_id)
                    .copied()
                    .ok_or_else(|| error("PROJECTION_VERTEX_MISSING"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for index in 1..face_vertices.len() - 1 {
            let triangle = [
                face_vertices[0],
                face_vertices[index],
                face_vertices[index + 1],
            ];
            if triangle[0] == triangle[1]
                || triangle[1] == triangle[2]
                || triangle[0] == triangle[2]
            {
                return Err(error("PROJECTION_DEGENERATE_TRIANGLE"));
            }
            indices.push(triangle);
        }
    }
    if indices.is_empty() {
        return Err(error("PROJECTION_NO_TRIANGLES"));
    }

    let (source_node_id, part_id, material_zone_id) =
        if let Some(binding) = &revision.source_binding {
            (
                binding.source_node_id.clone(),
                binding.part_id.clone(),
                binding.material_zone_id.clone(),
            )
        } else if let Some(binding) = &revision.foundation_source_binding {
            // Foundation provenance has no candidate source-node field.  The
            // Runtime-derived authoring revision is its stable node identity;
            // keep the foundation Part and material zone exact.
            (
                binding.authoring_mesh_revision_id.clone(),
                binding.part_id.clone(),
                binding.material_zone_id.clone(),
            )
        } else {
            let material_zone_hash = hash_value(&json!({
                "mesh_id": revision.mesh_id.clone(),
                "lineage_id": revision.lineage_id.clone(),
            }))?;
            (
                revision.revision_id.clone(),
                revision.mesh_id.clone(),
                format!("material-zone-{}", &material_zone_hash[..24]),
            )
        };
    let mut source_element_lineage = BTreeSet::new();
    source_element_lineage.insert(revision.mesh_id.clone());
    source_element_lineage.insert(revision.lineage_id.clone());
    source_element_lineage.insert(revision.revision_id.clone());
    source_element_lineage.extend(
        revision
            .original
            .faces
            .iter()
            .map(|face| face.face_id.clone()),
    );
    source_element_lineage.extend(
        revision
            .original
            .edges
            .iter()
            .map(|edge| edge.edge_id.clone()),
    );
    source_element_lineage.extend(
        revision
            .original
            .vertices
            .iter()
            .map(|vertex| vertex.vertex_id.clone()),
    );
    source_element_lineage.extend(
        revision
            .original
            .half_edges
            .iter()
            .map(|half_edge| half_edge.half_edge_id.clone()),
    );
    source_element_lineage.extend(
        revision
            .original
            .corners
            .iter()
            .map(|corner| corner.corner_id.clone()),
    );
    source_element_lineage.extend(
        revision
            .original
            .loops
            .iter()
            .map(|loop_record| loop_record.loop_id.clone()),
    );
    source_element_lineage.extend(
        revision
            .original
            .rings
            .iter()
            .map(|ring| ring.ring_id.clone()),
    );
    if let Some(source_binding) = &revision.source_binding {
        source_element_lineage.insert(format!(
            "source-binding-{}",
            source_binding.canonical_sha256
        ));
    }
    if let Some(foundation_source_binding) = &revision.foundation_source_binding {
        source_element_lineage.insert(format!(
            "foundation-source-binding-{}",
            foundation_source_binding.canonical_sha256
        ));
    }
    Ok(HighEvaluatorSourceMesh {
        schema_version: "HighEvaluatorSourceMesh@1".to_owned(),
        parts: vec![HighEvaluatorPart {
            operand_id: revision.mesh_id.clone(),
            part_id,
            source_node_ids: vec![source_node_id.clone()],
            source_node_id,
            material_zone_id,
            source_element_lineage: source_element_lineage.into_iter().collect(),
            positions_m: vertices.values().copied().collect(),
            indices,
        }],
    })
}

/// Project the complete Runtime-owned source part set without collapsing it
/// into the selected revision's legacy single Part.  `part_inputs` is already
/// a hash-bound typed projection from the GeometryProgram; this Worker only
/// validates its topology and derives evaluator-local triangles.
pub fn project_revision_with_parts(
    revision: &AuthoringMeshV2Revision,
    part_inputs: &[AuthoringMeshV2HighPartInput],
) -> Result<HighEvaluatorSourceMesh, AuthoringMeshV2HighError> {
    if part_inputs.is_empty() {
        return project_revision(revision);
    }
    validate_revision(revision)?;
    validate_part_inputs(revision, part_inputs)?;
    let parts = part_inputs
        .iter()
        .map(source_part_from_input)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(HighEvaluatorSourceMesh {
        schema_version: "HighEvaluatorSourceMesh@1".to_owned(),
        parts,
    })
}

fn source_part_from_input(
    input: &AuthoringMeshV2HighPartInput,
) -> Result<HighEvaluatorPart, AuthoringMeshV2HighError> {
    let mut indices = Vec::new();
    for face in &input.faces {
        for index in 1..face.len() - 1 {
            let triangle = [face[0], face[index], face[index + 1]];
            if triangle[0] == triangle[1]
                || triangle[1] == triangle[2]
                || triangle[0] == triangle[2]
            {
                return Err(error("PART_INPUT_DEGENERATE_TRIANGLE"));
            }
            indices.push(triangle);
        }
    }
    let mut lineage = input.source_element_lineage.clone();
    lineage.push(format!(
        "source-part-output-sha256:{}",
        input.source_part_output_sha256
    ));
    lineage.push(format!("source-part-index:{}", input.part_index));
    lineage.push(format!("source-part:{}", input.part_id));
    for source_node_id in effective_source_node_ids(input)? {
        lineage.push(format!("source-node:{source_node_id}"));
    }
    lineage.push(format!("material-zone:{}", input.material_zone_id));
    lineage.sort();
    lineage.dedup();
    Ok(HighEvaluatorPart {
        operand_id: input.operand_id.clone(),
        part_id: input.part_id.clone(),
        source_node_ids: effective_source_node_ids(input)?,
        source_node_id: input.source_node_id.clone(),
        material_zone_id: input.material_zone_id.clone(),
        source_element_lineage: lineage,
        positions_m: input.control_points.clone(),
        indices,
    })
}

fn stitched_step_for_input(
    revision: &AuthoringMeshV2Revision,
    input: &AuthoringMeshV2HighPartInput,
    subdivision_levels: usize,
    max_triangles_per_face: usize,
) -> Result<HighStitchedSubdivisionStep, AuthoringMeshV2HighError> {
    let max_triangles = max_triangles_per_face
        .checked_mul(input.faces.len())
        .ok_or_else(|| error("CPU_STITCHED_TRIANGLE_BUDGET_OVERFLOW"))?;
    let step_hash = hash_value(&json!({
        "revision_id": revision.revision_id.clone(),
        "revision_sha256": revision.canonical_sha256.clone(),
        "part_index": input.part_index,
        "part_id": input.part_id.clone(),
        "source_node_id": input.source_node_id.clone(),
        "source_node_ids": effective_source_node_ids(input)?,
        "source_part_output_sha256": input.source_part_output_sha256.clone(),
        "source_vertex_ids": input.source_vertex_ids.clone(),
        "source_edges": input.source_edges.clone(),
        "source_face_ids": input.source_face_ids.clone(),
        "faces": input.faces.clone(),
        "policy": STITCHED_EVALUATOR_CONTRACT,
    }))?;
    Ok(HighStitchedSubdivisionStep {
        step_id: format!("stitched-subdivision-{}", &step_hash[..24]),
        backend: SubdivisionBackend::CpuRegularQuad,
        part_id: input.part_id.clone(),
        material_zone_id: input.material_zone_id.clone(),
        source_node_ids: effective_source_node_ids(input)?,
        source_revision_id: revision.revision_id.clone(),
        source_revision_sha256: revision.canonical_sha256.clone(),
        source_vertex_ids: input.source_vertex_ids.clone(),
        source_edges: input.source_edges.clone(),
        source_face_ids: input.source_face_ids.clone(),
        control_points: input.control_points.clone(),
        faces: input.faces.clone(),
        subdivision_levels,
        max_triangles,
    })
}

/// Evaluate one closed V2 request twice through the existing High evaluator.
/// Both the source projection and the evaluator result must replay exactly.
pub fn evaluate(
    request: &AuthoringMeshV2HighRequest,
) -> Result<AuthoringMeshV2HighResult, AuthoringMeshV2HighError> {
    validate_request(request)?;
    let first = evaluate_once(request)?;
    let second = evaluate_once(request)?;
    if canonical_bytes(&serde_json::to_value(&first)?)
        != canonical_bytes(&serde_json::to_value(&second)?)
    {
        return Err(error("REPLAY_NON_DETERMINISTIC"));
    }
    let mut result = first;
    result.canonical_sha256 = hash_without_field(&result, "canonical_sha256")?;
    verify_readback(&result)?;
    Ok(result)
}

/// JSON-facing source-only entry for callers that already own the closed
/// worker request.  Runtime/CAS/MCP ownership remains outside this function.
pub fn run_json(input: &[u8]) -> Result<Vec<u8>, AuthoringMeshV2HighError> {
    let request: AuthoringMeshV2HighRequest = serde_json::from_slice(input)?;
    let result = evaluate(&request)?;
    Ok(canonical_bytes(&finalize_result_for_wire(&result)?))
}

/// JSON-facing Runtime-owned execution seam.  Unlike [`run_json`], this
/// envelope deliberately has no caller-supplied evaluator steps.  Runtime
/// resolves the immutable revision and fixed policy, then the Worker derives
/// the stitched request and performs the deterministic double evaluation.
pub fn run_execution_json(input: &[u8]) -> Result<Vec<u8>, AuthoringMeshV2HighError> {
    let request: AuthoringMeshV2HighExecutionRequest = serde_json::from_slice(input)?;
    validate_execution_request(&request)?;
    let worker_request = cpu_stitched_request_with_parts(
        request.revision,
        request.part_inputs,
        request.subdivision_levels,
        request.max_triangles_per_face,
        request.budgets,
    )?;
    let result = evaluate(&worker_request)?;
    Ok(canonical_bytes(&finalize_result_for_wire(&result)?))
}

/// Rebind every result hash to the JSON representation that actually crosses
/// the process boundary. Typed floating-point numbers can acquire a different
/// `serde_json::Number` representation after the Worker response is parsed;
/// persisting the pre-transport digest would therefore create an object that
/// cannot verify itself in Runtime. Input/revision hashes deliberately keep
/// their existing contract; normalization is isolated to this output adapter.
fn finalize_result_for_wire(
    result: &AuthoringMeshV2HighResult,
) -> Result<Value, AuthoringMeshV2HighError> {
    let typed = serde_json::to_value(result)?;
    let mut wire = canonical_roundtrip_fixed_point(typed)?;

    let source_mesh = wire
        .get("source_mesh")
        .cloned()
        .ok_or_else(|| error("WIRE_SOURCE_MESH_MISSING"))?;
    let source_mesh_sha256 = sha256_digest(&canonical_bytes(&source_mesh));

    {
        let evaluation = wire
            .get_mut("evaluation")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| error("WIRE_EVALUATION_MISSING"))?;
        evaluation.insert(
            "source_mesh_sha256".to_owned(),
            Value::String(source_mesh_sha256.clone()),
        );
        let evaluated_parts = evaluation
            .get("evaluated_parts")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| error("WIRE_EVALUATED_PARTS_MISSING"))?;
        let mut output_part_ids = BTreeSet::new();
        for part in &evaluated_parts {
            let output_part_id = part
                .get("output_part_id")
                .and_then(Value::as_str)
                .ok_or_else(|| error("WIRE_OUTPUT_PART_ID_MISSING"))?;
            if !output_part_ids.insert(output_part_id.to_owned()) {
                return Err(error("WIRE_DUPLICATE_OUTPUT_PART_ID"));
            }
        }
        let step_results = evaluation
            .get_mut("step_results")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| error("WIRE_STEP_RESULTS_MISSING"))?;
        for step_result in step_results {
            let output_part_id = step_result
                .get("output_part_id")
                .and_then(Value::as_str)
                .ok_or_else(|| error("WIRE_STEP_OUTPUT_PART_ID_MISSING"))?;
            let output = evaluated_parts
                .iter()
                .find(|part| {
                    part.get("output_part_id").and_then(Value::as_str) == Some(output_part_id)
                })
                .ok_or_else(|| error("WIRE_STEP_OUTPUT_PART_MISSING"))?;
            let mesh_preimage = json!({
                "positions_m": output
                    .get("positions_m")
                    .ok_or_else(|| error("WIRE_OUTPUT_POSITIONS_MISSING"))?,
                "indices": output
                    .get("indices")
                    .ok_or_else(|| error("WIRE_OUTPUT_INDICES_MISSING"))?,
            });
            step_result["output_sha256"] =
                Value::String(sha256_digest(&canonical_bytes(&mesh_preimage)));
        }
        evaluation.insert("canonical_sha256".to_owned(), Value::String(String::new()));
        let evaluation_preimage = Value::Object(evaluation.clone());
        evaluation.insert(
            "canonical_sha256".to_owned(),
            Value::String(sha256_digest(&canonical_bytes(&evaluation_preimage))),
        );
    }

    let evaluation_sha256 = sha256_digest(&canonical_bytes(
        wire.get("evaluation")
            .ok_or_else(|| error("WIRE_EVALUATION_MISSING"))?,
    ));
    {
        let readback = wire
            .get_mut("readback")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| error("WIRE_READBACK_MISSING"))?;
        readback.insert(
            "projected_source_mesh_sha256".to_owned(),
            Value::String(source_mesh_sha256),
        );
        readback.insert(
            "high_evaluation_sha256".to_owned(),
            Value::String(evaluation_sha256),
        );
        readback.insert("canonical_sha256".to_owned(), Value::String(String::new()));
        let readback_preimage = Value::Object(readback.clone());
        readback.insert(
            "canonical_sha256".to_owned(),
            Value::String(sha256_digest(&canonical_bytes(&readback_preimage))),
        );
    }

    wire["canonical_sha256"] = Value::String(String::new());
    let canonical_sha256 = sha256_digest(&canonical_bytes(&wire));
    wire["canonical_sha256"] = Value::String(canonical_sha256);
    Ok(wire)
}

fn canonical_roundtrip_fixed_point(mut value: Value) -> Result<Value, AuthoringMeshV2HighError> {
    for _ in 0..8 {
        let bytes = canonical_bytes(&value);
        let reparsed: Value = serde_json::from_slice(&bytes)?;
        if canonical_bytes(&reparsed) == bytes {
            return Ok(reparsed);
        }
        value = reparsed;
    }
    Err(error("WIRE_NUMBER_CANONICALIZATION_DID_NOT_CONVERGE"))
}

/// Stable identity of the direct V2 High algorithm, independent of one build
/// cohort or one evaluated revision.
pub fn algorithm_sha256() -> String {
    format!("{:x}", Sha256::digest(ALGORITHM.as_bytes()))
}

/// Convenience constructor for the fixed CPU path. It accepts bounded
/// manifold authored polygons (degree 3..=32) and emits one stitched topology
/// step. The original V2 revision remains immutable; the evaluated sibling
/// owns shared edge/vertex indexing and retains stable source IDs in the
/// evaluator lineage.
pub fn cpu_request(
    revision: AuthoringMeshV2Revision,
    subdivision_levels: usize,
    max_triangles_per_face: usize,
    budgets: HighEvaluatorBudgets,
) -> Result<AuthoringMeshV2HighRequest, AuthoringMeshV2HighError> {
    cpu_stitched_request(
        revision,
        subdivision_levels,
        max_triangles_per_face,
        budgets,
    )
}

/// Build the explicit polygon `StitchedSubdivision` evaluator contract for an
/// AuthoringMesh V2 revision. `max_triangles_per_face` is retained at this
/// convenience boundary for compatibility and is multiplied by the authored
/// face count with checked arithmetic before entering the worker contract.
pub fn cpu_stitched_request(
    revision: AuthoringMeshV2Revision,
    subdivision_levels: usize,
    max_triangles_per_face: usize,
    budgets: HighEvaluatorBudgets,
) -> Result<AuthoringMeshV2HighRequest, AuthoringMeshV2HighError> {
    cpu_stitched_request_with_parts(
        revision,
        Vec::new(),
        subdivision_levels,
        max_triangles_per_face,
        budgets,
    )
}

/// Build one stitched evaluator step per Runtime-materialized source Part.
/// The input order is authoritative: it is retained in the source mesh,
/// evaluator result and GLB primitive order.  An empty set intentionally
/// preserves the established single-revision compatibility path.
pub fn cpu_stitched_request_with_parts(
    revision: AuthoringMeshV2Revision,
    part_inputs: Vec<AuthoringMeshV2HighPartInput>,
    subdivision_levels: usize,
    max_triangles_per_face: usize,
    budgets: HighEvaluatorBudgets,
) -> Result<AuthoringMeshV2HighRequest, AuthoringMeshV2HighError> {
    if !(1..=2).contains(&subdivision_levels) || max_triangles_per_face == 0 {
        return Err(error("CPU_POLICY_INVALID"));
    }
    if !part_inputs.is_empty() {
        validate_part_inputs(&revision, &part_inputs)?;
        if part_inputs.len() > budgets.max_steps {
            return Err(error("CPU_STITCHED_PART_BUDGET_EXCEEDED"));
        }
    }
    if part_inputs.is_empty() {
        return cpu_stitched_request_single_part(
            revision,
            subdivision_levels,
            max_triangles_per_face,
            budgets,
        );
    }

    let source = project_revision_with_parts(&revision, &part_inputs)?;
    let mut steps = Vec::with_capacity(part_inputs.len());
    let mut source_vertex_count = 0usize;
    let mut source_triangle_count = 0usize;
    let mut estimated_evaluation_triangle_count = 0usize;
    for input in &part_inputs {
        source_vertex_count = source_vertex_count
            .checked_add(input.control_points.len())
            .ok_or_else(|| error("CPU_STITCHED_VERTEX_BUDGET_OVERFLOW"))?;
        let source_triangles = input.faces.iter().try_fold(0usize, |count, face| {
            count
                .checked_add(face.len().saturating_sub(2))
                .ok_or_else(|| error("CPU_STITCHED_TRIANGLE_BUDGET_OVERFLOW"))
        })?;
        source_triangle_count = source_triangle_count
            .checked_add(source_triangles)
            .ok_or_else(|| error("CPU_STITCHED_TRIANGLE_BUDGET_OVERFLOW"))?;
        let mut output_quads = input.faces.iter().try_fold(0usize, |count, face| {
            count
                .checked_add(face.len())
                .ok_or_else(|| error("CPU_STITCHED_TRIANGLE_BUDGET_OVERFLOW"))
        })?;
        for _ in 1..subdivision_levels {
            output_quads = output_quads
                .checked_mul(4)
                .ok_or_else(|| error("CPU_STITCHED_TRIANGLE_BUDGET_OVERFLOW"))?;
        }
        estimated_evaluation_triangle_count = estimated_evaluation_triangle_count
            .checked_add(
                output_quads
                    .checked_mul(2)
                    .ok_or_else(|| error("CPU_STITCHED_TRIANGLE_BUDGET_OVERFLOW"))?,
            )
            .ok_or_else(|| error("CPU_STITCHED_TRIANGLE_BUDGET_OVERFLOW"))?;
        steps.push(HighEvaluatorStep::StitchedSubdivision(
            stitched_step_for_input(&revision, input, subdivision_levels, max_triangles_per_face)?,
        ));
    }
    if source_vertex_count > budgets.max_output_vertices {
        return Err(error("CPU_STITCHED_SOURCE_VERTEX_BUDGET_EXCEEDED"));
    }
    if source_triangle_count
        .checked_add(estimated_evaluation_triangle_count)
        .is_none_or(|count| count > budgets.max_output_triangles)
    {
        return Err(error("CPU_STITCHED_OUTPUT_TRIANGLE_BUDGET_EXCEEDED"));
    }
    let mut request = AuthoringMeshV2HighRequest {
        schema_version: REQUEST_SCHEMA_VERSION.to_owned(),
        operation: OPERATION.to_owned(),
        revision_sha256: revision.canonical_sha256.clone(),
        revision,
        part_inputs,
        steps,
        budgets,
        canonical_sha256: String::new(),
    };
    // Force source construction before hashing so malformed topology cannot
    // enter a request that merely has a syntactically valid canonical hash.
    if source.parts.len() != request.part_inputs.len() {
        return Err(error("CPU_STITCHED_PART_SET_MISMATCH"));
    }
    request.canonical_sha256 = hash_without_field(&request, "canonical_sha256")?;
    Ok(request)
}

fn cpu_stitched_request_single_part(
    revision: AuthoringMeshV2Revision,
    subdivision_levels: usize,
    max_triangles_per_face: usize,
    budgets: HighEvaluatorBudgets,
) -> Result<AuthoringMeshV2HighRequest, AuthoringMeshV2HighError> {
    let source = project_revision(&revision)?;
    let part = source
        .parts
        .first()
        .ok_or_else(|| error("PROJECTION_NO_PART"))?;
    let vertex_map = revision
        .original
        .vertices
        .iter()
        .map(|vertex| (vertex.vertex_id.as_str(), vertex.position_m))
        .collect::<BTreeMap<_, _>>();
    let vertex_ids = vertex_map
        .keys()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    let vertex_index = vertex_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index as u32))
        .collect::<BTreeMap<_, _>>();
    let control_points = vertex_ids
        .iter()
        .map(|id| {
            vertex_map
                .get(id.as_str())
                .copied()
                .ok_or_else(|| error("CPU_STITCHED_VERTEX_MISSING"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|position| to_f32_position(position, "CPU stitched control point"))
        .collect::<Result<Vec<_>, _>>()?;
    let half_edges = revision
        .original
        .half_edges
        .iter()
        .map(|half_edge| (half_edge.half_edge_id.as_str(), half_edge))
        .collect::<BTreeMap<_, _>>();
    let face_map = revision
        .original
        .faces
        .iter()
        .map(|face| (face.face_id.as_str(), face))
        .collect::<BTreeMap<_, _>>();
    let mut face_ids = Vec::with_capacity(face_map.len());
    let mut faces = Vec::with_capacity(face_map.len());
    for (face_id, face) in face_map {
        if !(3..=MAX_FACE_DEGREE).contains(&face.half_edge_ids.len()) {
            return Err(error("CPU_STITCHED_FACE_DEGREE_INVALID"));
        }
        let ids = face
            .half_edge_ids
            .iter()
            .map(|half_edge_id| {
                half_edges
                    .get(half_edge_id.as_str())
                    .map(|half_edge| half_edge.origin_vertex_id.as_str())
                    .ok_or_else(|| error("CPU_FACE_HALF_EDGE_MISSING"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let indices = ids
            .iter()
            .map(|id| {
                vertex_index
                    .get(id)
                    .copied()
                    .ok_or_else(|| error("CPU_STITCHED_VERTEX_MISSING"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        face_ids.push(face_id.to_owned());
        faces.push(indices);
    }
    let mut edge_bindings_by_key = BTreeMap::<(u32, u32), String>::new();
    for edge in &revision.original.edges {
        let a = vertex_index
            .get(edge.vertex_ids[0].as_str())
            .copied()
            .ok_or_else(|| error("CPU_STITCHED_EDGE_VERTEX_MISSING"))?;
        let b = vertex_index
            .get(edge.vertex_ids[1].as_str())
            .copied()
            .ok_or_else(|| error("CPU_STITCHED_EDGE_VERTEX_MISSING"))?;
        let key = (a.min(b), a.max(b));
        if edge_bindings_by_key
            .insert(key, edge.edge_id.clone())
            .is_some()
        {
            return Err(error("CPU_STITCHED_DUPLICATE_EDGE_ENDPOINT"));
        }
    }
    let source_edges = edge_bindings_by_key
        .into_iter()
        .map(|((a, b), edge_id)| HighStitchedEdgeBinding {
            edge_id,
            vertex_indices: [a, b],
        })
        .collect::<Vec<_>>();
    let max_triangles = max_triangles_per_face
        .checked_mul(faces.len())
        .ok_or_else(|| error("CPU_STITCHED_TRIANGLE_BUDGET_OVERFLOW"))?;
    let step_hash = hash_value(&json!({
        "revision_id": revision.revision_id.clone(),
        "revision_sha256": revision.canonical_sha256.clone(),
        "source_vertex_ids": vertex_ids.clone(),
        "source_edges": source_edges.clone(),
        "source_face_ids": face_ids.clone(),
        "faces": faces.clone(),
        "policy": STITCHED_EVALUATOR_CONTRACT,
    }))?;
    let steps = vec![HighEvaluatorStep::StitchedSubdivision(
        HighStitchedSubdivisionStep {
            step_id: format!("stitched-subdivision-{}", &step_hash[..24]),
            backend: SubdivisionBackend::CpuRegularQuad,
            part_id: part.part_id.clone(),
            material_zone_id: part.material_zone_id.clone(),
            source_node_ids: vec![part.source_node_id.clone()],
            source_revision_id: revision.revision_id.clone(),
            source_revision_sha256: revision.canonical_sha256.clone(),
            source_vertex_ids: vertex_ids,
            source_edges,
            source_face_ids: face_ids,
            control_points,
            faces,
            subdivision_levels,
            max_triangles,
        },
    )];
    let mut request = AuthoringMeshV2HighRequest {
        schema_version: REQUEST_SCHEMA_VERSION.to_owned(),
        operation: OPERATION.to_owned(),
        revision_sha256: revision.canonical_sha256.clone(),
        revision,
        part_inputs: Vec::new(),
        steps,
        budgets,
        canonical_sha256: String::new(),
    };
    request.canonical_sha256 = hash_without_field(&request, "canonical_sha256")?;
    Ok(request)
}

/// Recompute and validate the embedded deterministic readback. This is useful
/// to a future Runtime adapter without granting the worker any persistence.
pub fn verify_readback(
    result: &AuthoringMeshV2HighResult,
) -> Result<AuthoringMeshV2HighReadback, AuthoringMeshV2HighError> {
    if result.schema_version != RESULT_SCHEMA_VERSION || result.operation != OPERATION {
        return Err(error("RESULT_SCHEMA_MISMATCH"));
    }
    if !is_sha256(&result.revision_sha256)
        || result.revision_sha256.is_empty()
        || !is_sha256(&result.evaluation.source_mesh_sha256)
    {
        return Err(error("RESULT_HASH_INVALID"));
    }
    let source_hash = hash_value(&result.source_mesh)?;
    let contract_valid = match result.evaluation.evaluator_contract.policy.as_str() {
        STITCHED_SUBDIVISION_POLICY => {
            result.evaluation.evaluator_contract.continuity
                == "shared-edge-and-shared-vertex-indexing@2"
        }
        SUBDIVISION_POLICY => {
            result.evaluation.evaluator_contract.continuity == "shared-within-patch-only@1"
        }
        _ => false,
    };
    if source_hash != result.evaluation.source_mesh_sha256
        || result.evaluation.base_parts != result.source_mesh.parts
        || !contract_valid
        || !result.evaluation.non_destructive
        || result.evaluation.runtime_write_performed
        || result.evaluation.production_stage_advanced
        || result.evaluation.candidate_confirmed
        || result.evaluation.version_created
        || result.evaluation.export_performed
    {
        return Err(error("RESULT_SOURCE_BINDING_INVALID"));
    }
    if result.source_mesh.parts.len() > 1 {
        validate_complete_part_set(
            &result.source_mesh,
            &result.evaluation.evaluated_parts,
            &result.evaluation.step_results,
        )?;
    }
    if result.replay_count != 2
        || !result.replay_byte_exact
        || !result.non_destructive
        || result.runtime_write_performed
        || result.production_stage_advanced
        || result.candidate_confirmed
        || result.version_created
        || result.export_performed
        || result.quality_status != "structural_only"
        || result.high_worker_algorithm_sha256 != algorithm_sha256()
    {
        return Err(error("RESULT_NON_DESTRUCTIVE_POLICY_INVALID"));
    }
    let evaluation_hash = hash_value(&result.evaluation)?;
    let mut expected = AuthoringMeshV2HighReadback {
        schema_version: READBACK_SCHEMA_VERSION.to_owned(),
        mesh_id: result.mesh_id.clone(),
        lineage_id: result.lineage_id.clone(),
        revision_id: result.revision_id.clone(),
        revision_sha256: result.revision_sha256.clone(),
        projected_source_mesh_sha256: source_hash,
        source_vertex_count: result
            .source_mesh
            .parts
            .iter()
            .map(|part| part.positions_m.len())
            .sum(),
        source_triangle_count: result
            .source_mesh
            .parts
            .iter()
            .map(|part| part.indices.len())
            .sum(),
        evaluated_part_count: result.evaluation.evaluated_parts.len(),
        evaluated_triangle_count: result.evaluation.evaluated_triangle_count,
        high_evaluation_sha256: evaluation_hash,
        high_worker_algorithm_sha256: result.high_worker_algorithm_sha256.clone(),
        replay_count: result.replay_count,
        replay_byte_exact: result.replay_byte_exact,
        non_destructive: result.non_destructive,
        runtime_write_performed: result.runtime_write_performed,
        production_stage_advanced: result.production_stage_advanced,
        candidate_confirmed: result.candidate_confirmed,
        version_created: result.version_created,
        export_performed: result.export_performed,
        limitations: result.limitations.clone(),
        canonical_sha256: String::new(),
    };
    expected.canonical_sha256 = hash_without_field(&expected, "canonical_sha256")?;
    if result.readback != expected {
        return Err(error("RESULT_READBACK_MISMATCH"));
    }
    if result.canonical_sha256 != hash_without_field(result, "canonical_sha256")? {
        return Err(error("RESULT_CANONICAL_HASH_MISMATCH"));
    }
    Ok(expected)
}

fn validate_complete_part_set(
    source_mesh: &HighEvaluatorSourceMesh,
    evaluated_parts: &[crate::evaluator::HighEvaluatedPart],
    step_results: &[crate::evaluator::HighEvaluatorStepResult],
) -> Result<(), AuthoringMeshV2HighError> {
    if evaluated_parts.len() != source_mesh.parts.len()
        || step_results.len() != source_mesh.parts.len()
    {
        return Err(error("RESULT_MULTIPART_EVALUATION_SET_MISMATCH"));
    }
    for ((source, evaluated), step_result) in source_mesh
        .parts
        .iter()
        .zip(evaluated_parts)
        .zip(step_results)
    {
        let source_node_ids = if source.source_node_ids.is_empty() {
            vec![source.source_node_id.clone()]
        } else {
            source.source_node_ids.clone()
        };
        if evaluated.part_id != source.part_id
            || evaluated.source_node_id != source.source_node_id
            || evaluated.source_node_ids != source_node_ids
            || evaluated.material_zone_id != source.material_zone_id
            || evaluated.source_operand_ids != vec![source.operand_id.clone()]
            || step_result.output_part_id != evaluated.output_part_id
            || step_result.status != "passed"
        {
            return Err(error("RESULT_MULTIPART_LINEAGE_OR_ORDER_MISMATCH"));
        }
    }
    Ok(())
}

/// Verify the JSON representation that crossed the Worker boundary before it
/// is deserialized into the typed result.  Evaluator positions are f32 while
/// the JSON source is represented by `serde_json::Number`; a typed round trip
/// can therefore change the canonical spelling of a coordinate.  Hashes and
/// readback assertions must be checked against the raw wire value, otherwise
/// an honest Worker response is rejected (or a changed response is accepted)
/// after deserialization.
pub fn verify_wire_result(value: &Value) -> Result<(), AuthoringMeshV2HighError> {
    let object = value
        .as_object()
        .ok_or_else(|| error("RESULT_SCHEMA_MISMATCH"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some(RESULT_SCHEMA_VERSION)
        || object.get("operation").and_then(Value::as_str) != Some(OPERATION)
    {
        return Err(error("RESULT_SCHEMA_MISMATCH"));
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| error("RESULT_CANONICAL_HASH_MISMATCH"))?;
    let mut root_preimage = value.clone();
    root_preimage["canonical_sha256"] = Value::String(String::new());
    if canonical != sha256_digest(&canonical_bytes(&root_preimage)) {
        return Err(error("RESULT_CANONICAL_HASH_MISMATCH"));
    }

    let source_mesh = value
        .get("source_mesh")
        .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
    let source_mesh_hash = sha256_digest(&canonical_bytes(source_mesh));
    let evaluation = value
        .get("evaluation")
        .and_then(Value::as_object)
        .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
    if evaluation.get("non_destructive") != Some(&Value::Bool(true))
        || evaluation.get("runtime_write_performed") != Some(&Value::Bool(false))
        || evaluation.get("production_stage_advanced") != Some(&Value::Bool(false))
        || evaluation.get("candidate_confirmed") != Some(&Value::Bool(false))
        || evaluation.get("version_created") != Some(&Value::Bool(false))
        || evaluation.get("export_performed") != Some(&Value::Bool(false))
        || evaluation.get("replay_count") != Some(&Value::from(2_u64))
        || evaluation.get("replay_byte_exact") != Some(&Value::Bool(true))
        || evaluation.get("quality_status") != Some(&Value::String("structural_only".to_owned()))
    {
        return Err(error("RESULT_NON_DESTRUCTIVE_POLICY_INVALID"));
    }
    let evaluator_contract = evaluation
        .get("evaluator_contract")
        .and_then(Value::as_object)
        .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
    let contract_valid = match evaluator_contract.get("policy").and_then(Value::as_str) {
        Some(STITCHED_EVALUATOR_CONTRACT) => {
            evaluator_contract.get("continuity").and_then(Value::as_str)
                == Some("shared-edge-and-shared-vertex-indexing@2")
        }
        Some(SUBDIVISION_POLICY) => {
            evaluator_contract.get("continuity").and_then(Value::as_str)
                == Some("shared-within-patch-only@1")
        }
        _ => false,
    };
    if !contract_valid {
        return Err(error("RESULT_SOURCE_BINDING_INVALID"));
    }
    if evaluation.get("source_mesh_sha256").and_then(Value::as_str)
        != Some(source_mesh_hash.as_str())
        || evaluation.get("base_parts") != source_mesh.get("parts")
    {
        return Err(error("RESULT_SOURCE_BINDING_INVALID"));
    }
    let evaluated_parts = evaluation
        .get("evaluated_parts")
        .and_then(Value::as_array)
        .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
    let step_results = evaluation
        .get("step_results")
        .and_then(Value::as_array)
        .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
    for step in step_results {
        let output_part_id = step
            .get("output_part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
        let output = evaluated_parts
            .iter()
            .find(|part| part.get("output_part_id").and_then(Value::as_str) == Some(output_part_id))
            .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
        let mesh = json!({
            "positions_m": output
                .get("positions_m")
                .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?,
            "indices": output
                .get("indices")
                .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?,
        });
        if step.get("output_sha256").and_then(Value::as_str)
            != Some(sha256_digest(&canonical_bytes(&mesh)).as_str())
        {
            return Err(error("RESULT_EVALUATION_OUTPUT_HASH_MISMATCH"));
        }
    }
    let mut evaluation_preimage = Value::Object(evaluation.clone());
    evaluation_preimage["canonical_sha256"] = Value::String(String::new());
    let evaluation_canonical_hash = sha256_digest(&canonical_bytes(&evaluation_preimage));
    if evaluation.get("canonical_sha256").and_then(Value::as_str)
        != Some(evaluation_canonical_hash.as_str())
    {
        return Err(error("RESULT_EVALUATION_CANONICAL_HASH_MISMATCH"));
    }
    // Readback binds the full, already canonicalized evaluation object (the
    // typed verifier's `hash_value(&evaluation)` semantics), not its semantic
    // preimage with the nested canonical field blanked.
    let evaluation_hash = sha256_digest(&canonical_bytes(&Value::Object(evaluation.clone())));

    let readback = value
        .get("readback")
        .and_then(Value::as_object)
        .ok_or_else(|| error("RESULT_READBACK_MISMATCH"))?;
    let source_parts = source_mesh
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| error("RESULT_READBACK_MISMATCH"))?;
    let source_vertex_count: u64 = source_parts
        .iter()
        .map(|part| {
            part.get("positions_m")
                .and_then(Value::as_array)
                .map(|points| points.len() as u64)
                .ok_or_else(|| error("RESULT_READBACK_MISMATCH"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum();
    let source_triangle_count: u64 = source_parts
        .iter()
        .map(|part| {
            part.get("indices")
                .and_then(Value::as_array)
                .map(|triangles| triangles.len() as u64)
                .ok_or_else(|| error("RESULT_READBACK_MISMATCH"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum();
    let base_parts = evaluation
        .get("base_parts")
        .and_then(Value::as_array)
        .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
    let base_triangle_count: u64 = base_parts
        .iter()
        .map(|part| {
            part.get("indices")
                .and_then(Value::as_array)
                .map(|triangles| triangles.len() as u64)
                .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum();
    let actual_evaluated_triangle_count: u64 = evaluated_parts
        .iter()
        .map(|part| {
            part.get("indices")
                .and_then(Value::as_array)
                .map(|triangles| triangles.len() as u64)
                .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum();
    if evaluation
        .get("base_triangle_count")
        .and_then(Value::as_u64)
        != Some(base_triangle_count)
        || evaluation
            .get("evaluated_triangle_count")
            .and_then(Value::as_u64)
            != Some(actual_evaluated_triangle_count)
        || evaluation.get("triangle_count").and_then(Value::as_u64)
            != Some(base_triangle_count + actual_evaluated_triangle_count)
    {
        return Err(error("RESULT_SOURCE_BINDING_INVALID"));
    }
    let evaluated_triangle_count = evaluation
        .get("evaluated_triangle_count")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("RESULT_READBACK_MISMATCH"))?;
    let expected_readback = [
        (
            "schema_version",
            Value::String(READBACK_SCHEMA_VERSION.to_owned()),
        ),
        ("mesh_id", value["mesh_id"].clone()),
        ("lineage_id", value["lineage_id"].clone()),
        ("revision_id", value["revision_id"].clone()),
        ("revision_sha256", value["revision_sha256"].clone()),
        (
            "projected_source_mesh_sha256",
            Value::String(source_mesh_hash.clone()),
        ),
        ("source_vertex_count", Value::from(source_vertex_count)),
        ("source_triangle_count", Value::from(source_triangle_count)),
        (
            "evaluated_part_count",
            Value::from(evaluated_parts.len() as u64),
        ),
        (
            "evaluated_triangle_count",
            Value::from(evaluated_triangle_count),
        ),
        ("high_evaluation_sha256", Value::String(evaluation_hash)),
        (
            "high_worker_algorithm_sha256",
            value["high_worker_algorithm_sha256"].clone(),
        ),
        ("replay_count", value["replay_count"].clone()),
        ("replay_byte_exact", value["replay_byte_exact"].clone()),
        ("non_destructive", value["non_destructive"].clone()),
        (
            "runtime_write_performed",
            value["runtime_write_performed"].clone(),
        ),
        (
            "production_stage_advanced",
            value["production_stage_advanced"].clone(),
        ),
        ("candidate_confirmed", value["candidate_confirmed"].clone()),
        ("version_created", value["version_created"].clone()),
        ("export_performed", value["export_performed"].clone()),
        ("limitations", value["limitations"].clone()),
    ];
    for (field, expected) in expected_readback {
        if readback.get(field) != Some(&expected) {
            return Err(error(format!("RESULT_READBACK_{field}_MISMATCH")));
        }
    }
    let mut readback_preimage = Value::Object(readback.clone());
    readback_preimage["canonical_sha256"] = Value::String(String::new());
    if readback.get("canonical_sha256").and_then(Value::as_str)
        != Some(sha256_digest(&canonical_bytes(&readback_preimage)).as_str())
    {
        return Err(error("RESULT_READBACK_CANONICAL_HASH_MISMATCH"));
    }

    let flags = [
        ("replay_count", Value::from(2_u64)),
        ("replay_byte_exact", Value::Bool(true)),
        ("non_destructive", Value::Bool(true)),
        ("runtime_write_performed", Value::Bool(false)),
        ("production_stage_advanced", Value::Bool(false)),
        ("candidate_confirmed", Value::Bool(false)),
        ("version_created", Value::Bool(false)),
        ("export_performed", Value::Bool(false)),
        (
            "quality_status",
            Value::String("structural_only".to_owned()),
        ),
    ];
    for (field, expected) in flags {
        if object.get(field) != Some(&expected) {
            return Err(error(format!("RESULT_{field}_POLICY_INVALID")));
        }
    }
    if object
        .get("high_worker_algorithm_sha256")
        .and_then(Value::as_str)
        != Some(algorithm_sha256().as_str())
    {
        return Err(error("RESULT_ALGORITHM_INVALID"));
    }
    let source_parts = source_mesh
        .get("parts")
        .and_then(Value::as_array)
        .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
    if source_parts.len() > 1 {
        if evaluated_parts.len() != source_parts.len() || step_results.len() != source_parts.len() {
            return Err(error("RESULT_MULTIPART_EVALUATION_SET_MISMATCH"));
        }
        for ((source, evaluated), step_result) in
            source_parts.iter().zip(evaluated_parts).zip(step_results)
        {
            let source_part_id = source
                .get("part_id")
                .and_then(Value::as_str)
                .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
            let source_node_id = source
                .get("source_node_id")
                .and_then(Value::as_str)
                .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
            let source_node_ids = source
                .get("source_node_ids")
                .and_then(Value::as_array)
                .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                .unwrap_or_else(|| vec![source_node_id]);
            let source_material_zone_id = source
                .get("material_zone_id")
                .and_then(Value::as_str)
                .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
            let source_operand_id = source
                .get("operand_id")
                .and_then(Value::as_str)
                .ok_or_else(|| error("RESULT_SOURCE_BINDING_INVALID"))?;
            if evaluated.get("part_id").and_then(Value::as_str) != Some(source_part_id)
                || evaluated.get("source_node_id").and_then(Value::as_str) != Some(source_node_id)
                || evaluated
                    .get("source_node_ids")
                    .and_then(Value::as_array)
                    .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                    .unwrap_or_else(|| vec![source_node_id])
                    != source_node_ids
                || evaluated.get("material_zone_id").and_then(Value::as_str)
                    != Some(source_material_zone_id)
                || evaluated.get("source_operand_ids")
                    != Some(&Value::Array(vec![Value::String(
                        source_operand_id.to_owned(),
                    )]))
                || step_result.get("output_part_id") != evaluated.get("output_part_id")
                || step_result.get("status") != Some(&Value::String("passed".to_owned()))
            {
                return Err(error("RESULT_MULTIPART_LINEAGE_OR_ORDER_MISMATCH"));
            }
        }
    }
    // Force the typed deserializer to enforce the closed nested shape, ID
    // policy, finite coordinates, topology bounds and evaluator flags.  Hash
    // fields are intentionally checked above against the raw representation.
    serde_json::from_value::<AuthoringMeshV2HighResult>(value.clone())
        .map_err(|_| error("RESULT_TYPED_SHAPE_INVALID"))?;
    Ok(())
}

fn evaluate_once(
    request: &AuthoringMeshV2HighRequest,
) -> Result<AuthoringMeshV2HighResult, AuthoringMeshV2HighError> {
    let source_mesh = project_revision_with_parts(&request.revision, &request.part_inputs)?;
    let evaluator_request = evaluator_request(source_mesh.clone(), request)?;
    let evaluation = evaluate_high(&evaluator_request)?;
    let mut limitations = vec![
        "AUTHORING_MESH_V2_ORIGINAL_ONLY@1".to_owned(),
        "EVALUATED_SIDECAR_NOT_USED_AS_AUTHORITY@1".to_owned(),
        "EVALUATOR_CONTRACT:NativeHighEvaluatorContract@1".to_owned(),
        "NO_RUNTIME_WRITE_OR_STAGE_ADVANCEMENT@1".to_owned(),
    ];
    if evaluation.evaluator_contract.policy == STITCHED_EVALUATOR_CONTRACT {
        limitations.extend([
            "STITCHED_CPU_CATMULL_CLARK_POLYGON_MANIFOLD_BOUNDARY@2".to_owned(),
            "SHARED_EDGE_AND_VERTEX_INDEXING@2".to_owned(),
        ]);
    } else {
        limitations.push("CPU_REGULAR_QUAD_SUBSET_ONLY@1".to_owned());
    }
    let mut result = AuthoringMeshV2HighResult {
        schema_version: RESULT_SCHEMA_VERSION.to_owned(),
        operation: OPERATION.to_owned(),
        mesh_id: request.revision.mesh_id.clone(),
        lineage_id: request.revision.lineage_id.clone(),
        revision_id: request.revision.revision_id.clone(),
        revision_index: request.revision.revision_index,
        revision_sha256: request.revision_sha256.clone(),
        high_worker_algorithm_sha256: algorithm_sha256(),
        source_mesh,
        evaluation,
        readback: AuthoringMeshV2HighReadback {
            schema_version: READBACK_SCHEMA_VERSION.to_owned(),
            mesh_id: String::new(),
            lineage_id: String::new(),
            revision_id: String::new(),
            revision_sha256: String::new(),
            projected_source_mesh_sha256: String::new(),
            source_vertex_count: 0,
            source_triangle_count: 0,
            evaluated_part_count: 0,
            evaluated_triangle_count: 0,
            high_evaluation_sha256: String::new(),
            high_worker_algorithm_sha256: String::new(),
            replay_count: 2,
            replay_byte_exact: true,
            non_destructive: true,
            runtime_write_performed: false,
            production_stage_advanced: false,
            candidate_confirmed: false,
            version_created: false,
            export_performed: false,
            limitations: limitations.clone(),
            canonical_sha256: String::new(),
        },
        replay_count: 2,
        replay_byte_exact: true,
        non_destructive: true,
        runtime_write_performed: false,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        quality_status: "structural_only".to_owned(),
        limitations,
        canonical_sha256: String::new(),
    };
    result.readback = build_readback(&result)?;
    result.canonical_sha256 = hash_without_field(&result, "canonical_sha256")?;
    Ok(result)
}

fn build_readback(
    result: &AuthoringMeshV2HighResult,
) -> Result<AuthoringMeshV2HighReadback, AuthoringMeshV2HighError> {
    let source_hash = hash_value(&result.source_mesh)?;
    let mut readback = AuthoringMeshV2HighReadback {
        schema_version: READBACK_SCHEMA_VERSION.to_owned(),
        mesh_id: result.mesh_id.clone(),
        lineage_id: result.lineage_id.clone(),
        revision_id: result.revision_id.clone(),
        revision_sha256: result.revision_sha256.clone(),
        projected_source_mesh_sha256: source_hash,
        source_vertex_count: result
            .source_mesh
            .parts
            .iter()
            .map(|part| part.positions_m.len())
            .sum(),
        source_triangle_count: result
            .source_mesh
            .parts
            .iter()
            .map(|part| part.indices.len())
            .sum(),
        evaluated_part_count: result.evaluation.evaluated_parts.len(),
        evaluated_triangle_count: result.evaluation.evaluated_triangle_count,
        high_evaluation_sha256: hash_value(&result.evaluation)?,
        high_worker_algorithm_sha256: result.high_worker_algorithm_sha256.clone(),
        replay_count: result.replay_count,
        replay_byte_exact: result.replay_byte_exact,
        non_destructive: result.non_destructive,
        runtime_write_performed: result.runtime_write_performed,
        production_stage_advanced: result.production_stage_advanced,
        candidate_confirmed: result.candidate_confirmed,
        version_created: result.version_created,
        export_performed: result.export_performed,
        limitations: result.limitations.clone(),
        canonical_sha256: String::new(),
    };
    readback.canonical_sha256 = hash_without_field(&readback, "canonical_sha256")?;
    Ok(readback)
}

fn evaluator_request(
    source_mesh: HighEvaluatorSourceMesh,
    request: &AuthoringMeshV2HighRequest,
) -> Result<HighEvaluatorRequest, AuthoringMeshV2HighError> {
    let mut evaluator_request = HighEvaluatorRequest {
        schema_version: crate::evaluator::REQUEST_SCHEMA_VERSION.to_owned(),
        operation: crate::evaluator::OPERATION.to_owned(),
        source_mesh,
        source_mesh_sha256: String::new(),
        steps: request.steps.clone(),
        budgets: request.budgets.clone(),
        canonical_sha256: String::new(),
    };
    evaluator_request.source_mesh_sha256 = hash_value(&evaluator_request.source_mesh)?;
    evaluator_request.canonical_sha256 =
        hash_without_field(&evaluator_request, "canonical_sha256")?;
    Ok(evaluator_request)
}

fn validate_part_inputs(
    revision: &AuthoringMeshV2Revision,
    part_inputs: &[AuthoringMeshV2HighPartInput],
) -> Result<(), AuthoringMeshV2HighError> {
    if part_inputs.is_empty() || part_inputs.len() > 128 {
        return Err(error("PART_INPUT_SET_BUDGET_INVALID"));
    }
    let mut part_ids = BTreeSet::new();
    let mut operand_ids = BTreeSet::new();
    let mut source_node_ids = BTreeSet::new();
    let mut output_hashes = BTreeSet::new();
    for (expected_index, input) in part_inputs.iter().enumerate() {
        if input.part_index != expected_index as u32 {
            return Err(error("PART_INPUT_ORDER_NOT_CONTIGUOUS"));
        }
        for (value, label) in [
            (&input.operand_id, "part_input.operand_id"),
            (&input.part_id, "part_input.part_id"),
            (&input.source_node_id, "part_input.source_node_id"),
            (&input.material_zone_id, "part_input.material_zone_id"),
        ] {
            validate_id(value, label)?;
        }
        let input_source_node_ids = effective_source_node_ids(input)?;
        if !part_ids.insert(input.part_id.clone())
            || !operand_ids.insert(input.operand_id.clone())
            || !output_hashes.insert(input.source_part_output_sha256.clone())
        {
            return Err(error("PART_INPUT_ID_OR_OUTPUT_HASH_DUPLICATED"));
        }
        for source_node_id in input_source_node_ids {
            if !source_node_ids.insert(source_node_id) {
                return Err(error("PART_INPUT_SOURCE_NODE_ID_DUPLICATED"));
            }
        }
        if !is_sha256(&input.source_part_output_sha256) {
            return Err(error("PART_INPUT_SOURCE_OUTPUT_HASH_INVALID"));
        }
        if input.source_element_lineage.is_empty()
            || input.source_element_lineage.len() > MAX_ID_LENGTH
        {
            return Err(error("PART_INPUT_SOURCE_LINEAGE_INVALID"));
        }
        for lineage_id in &input.source_element_lineage {
            validate_id(lineage_id, "part_input.source_element_lineage")?;
        }
        if input.control_points.len() < 3 || input.control_points.len() > MAX_VERTICES {
            return Err(error("PART_INPUT_VERTEX_BUDGET_INVALID"));
        }
        if input.faces.is_empty() || input.faces.len() > MAX_FACES {
            return Err(error("PART_INPUT_FACE_BUDGET_INVALID"));
        }
        if input.source_vertex_ids.len() != input.control_points.len()
            || input.source_face_ids.len() != input.faces.len()
        {
            return Err(error("PART_INPUT_SOURCE_BINDING_COUNT_INVALID"));
        }
        let mut vertex_ids = BTreeSet::new();
        for vertex_id in &input.source_vertex_ids {
            validate_id(vertex_id, "part_input.source_vertex_id")?;
            if !vertex_ids.insert(vertex_id) {
                return Err(error("PART_INPUT_SOURCE_VERTEX_ID_DUPLICATED"));
            }
        }
        let mut face_ids = BTreeSet::new();
        for face_id in &input.source_face_ids {
            validate_id(face_id, "part_input.source_face_id")?;
            if !face_ids.insert(face_id) {
                return Err(error("PART_INPUT_SOURCE_FACE_ID_DUPLICATED"));
            }
        }
        if input.source_edges.is_empty() || input.source_edges.len() > MAX_EDGES {
            return Err(error("PART_INPUT_EDGE_BUDGET_INVALID"));
        }
        let mut supplied_edges = BTreeSet::new();
        let mut edge_ids = BTreeSet::new();
        for edge in &input.source_edges {
            validate_id(&edge.edge_id, "part_input.edge_id")?;
            if !edge_ids.insert(edge.edge_id.clone()) {
                return Err(error("PART_INPUT_EDGE_ID_DUPLICATED"));
            }
            let [a, b] = edge.vertex_indices;
            if a == b
                || a as usize >= input.control_points.len()
                || b as usize >= input.control_points.len()
            {
                return Err(error("PART_INPUT_EDGE_ENDPOINT_INVALID"));
            }
            if !supplied_edges.insert((a.min(b), a.max(b))) {
                return Err(error("PART_INPUT_EDGE_ENDPOINT_DUPLICATED"));
            }
        }
        let mut derived_edges = BTreeSet::new();
        for face in &input.faces {
            if !(3..=MAX_FACE_DEGREE).contains(&face.len()) {
                return Err(error("PART_INPUT_FACE_DEGREE_INVALID"));
            }
            let mut face_vertices = BTreeSet::new();
            for (corner, index) in face.iter().enumerate() {
                if *index as usize >= input.control_points.len() || !face_vertices.insert(index) {
                    return Err(error("PART_INPUT_FACE_VERTEX_INVALID"));
                }
                let next = face[(corner + 1) % face.len()];
                derived_edges.insert(((*index).min(next), (*index).max(next)));
            }
            // The evaluator performs the strict finite/area/manifold checks;
            // this boundary only establishes that the source edge bindings
            // describe precisely the same polygon graph.
        }
        if supplied_edges != derived_edges {
            return Err(error("PART_INPUT_EDGE_BINDING_MISMATCH"));
        }
        for point in &input.control_points {
            if point
                .iter()
                .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE_M as f32)
            {
                return Err(error("PART_INPUT_CONTROL_POINT_INVALID"));
            }
        }
    }

    if let Some(binding) = &revision.source_binding {
        let binding_lineage = format!("source-binding-{}", binding.canonical_sha256);
        let matches = part_inputs
            .iter()
            .filter(|input| {
                input.part_id == binding.part_id
                    && input.material_zone_id == binding.material_zone_id
                    && input.source_element_lineage.contains(&binding_lineage)
            })
            .count();
        if matches != 1 {
            return Err(error("PART_INPUT_SOURCE_BINDING_NOT_EXACT"));
        }
    }
    if let Some(binding) = &revision.foundation_source_binding {
        let matches = part_inputs
            .iter()
            .filter(|input| {
                input.part_id == binding.part_id
                    && input.source_node_id == binding.authoring_mesh_revision_id
                    && input.material_zone_id == binding.material_zone_id
            })
            .count();
        if matches != 1 {
            return Err(error("PART_INPUT_FOUNDATION_BINDING_NOT_EXACT"));
        }
    }
    Ok(())
}

/// Normalize the compatibility scalar and the composite source-node array at
/// the Worker boundary.  Empty arrays are accepted only for legacy callers;
/// every current Runtime multipart request emits the complete array.
fn effective_source_node_ids(
    input: &AuthoringMeshV2HighPartInput,
) -> Result<Vec<String>, AuthoringMeshV2HighError> {
    let values = if input.source_node_ids.is_empty() {
        vec![input.source_node_id.clone()]
    } else {
        input.source_node_ids.clone()
    };
    if values.is_empty() || values.len() > 16 || values[0] != input.source_node_id {
        return Err(error("PART_INPUT_SOURCE_NODE_SET_INVALID"));
    }
    let mut seen = BTreeSet::new();
    for value in &values {
        validate_id(value, "part_input.source_node_id")?;
        if !seen.insert(value) {
            return Err(error("PART_INPUT_SOURCE_NODE_ID_DUPLICATED"));
        }
    }
    Ok(values)
}

fn validate_request(request: &AuthoringMeshV2HighRequest) -> Result<(), AuthoringMeshV2HighError> {
    if request.schema_version != REQUEST_SCHEMA_VERSION || request.operation != OPERATION {
        return Err(error("REQUEST_SCHEMA_OR_OPERATION_MISMATCH"));
    }
    if !is_sha256(&request.revision_sha256) || !is_sha256(&request.canonical_sha256) {
        return Err(error("REQUEST_HASH_INVALID"));
    }
    validate_revision(&request.revision)?;
    if request.revision.canonical_sha256 != request.revision_sha256 {
        return Err(error("REQUEST_REVISION_HASH_MISMATCH"));
    }
    if request.part_inputs.is_empty() {
        for step in &request.steps {
            if let HighEvaluatorStep::StitchedSubdivision(step) = step {
                validate_stitched_step_binding(&request.revision, step)?;
            }
        }
    } else {
        validate_part_inputs(&request.revision, &request.part_inputs)?;
        validate_multipart_steps(request)?;
    }
    if request.steps.is_empty() {
        return Err(error("REQUEST_STEPS_EMPTY"));
    }
    if request.canonical_sha256 != hash_without_field(request, "canonical_sha256")? {
        return Err(error("REQUEST_CANONICAL_HASH_MISMATCH"));
    }
    Ok(())
}

fn validate_execution_request(
    request: &AuthoringMeshV2HighExecutionRequest,
) -> Result<(), AuthoringMeshV2HighError> {
    if request.schema_version != EXECUTION_REQUEST_SCHEMA_VERSION
        || request.operation != EXECUTION_OPERATION
    {
        return Err(error("EXECUTION_REQUEST_SCHEMA_OR_OPERATION_MISMATCH"));
    }
    if !is_sha256(&request.revision_sha256) || !is_sha256(&request.canonical_sha256) {
        return Err(error("EXECUTION_REQUEST_HASH_INVALID"));
    }
    if request.revision.canonical_sha256 != request.revision_sha256 {
        return Err(error("EXECUTION_REQUEST_REVISION_HASH_MISMATCH"));
    }
    if !(1..=2).contains(&request.subdivision_levels) || request.max_triangles_per_face == 0 {
        return Err(error("EXECUTION_REQUEST_POLICY_INVALID"));
    }
    validate_revision(&request.revision)?;
    // An empty set is accepted only as a compatibility bridge for existing
    // single-revision fixtures.  Runtime's multipart path always supplies the
    // complete ordered set and is validated below before evaluation.
    if !request.part_inputs.is_empty() {
        validate_part_inputs(&request.revision, &request.part_inputs)?;
    }
    if request.canonical_sha256 != hash_without_field(request, "canonical_sha256")? {
        return Err(error("EXECUTION_REQUEST_CANONICAL_HASH_MISMATCH"));
    }
    Ok(())
}

fn validate_multipart_steps(
    request: &AuthoringMeshV2HighRequest,
) -> Result<(), AuthoringMeshV2HighError> {
    if request.steps.len() != request.part_inputs.len() {
        return Err(error("REQUEST_MULTIPART_STEP_SET_MISMATCH"));
    }
    let mut seen = BTreeSet::new();
    for (input, step) in request.part_inputs.iter().zip(&request.steps) {
        let HighEvaluatorStep::StitchedSubdivision(step) = step else {
            return Err(error("REQUEST_MULTIPART_STEP_KIND_INVALID"));
        };
        if !seen.insert(step.part_id.clone())
            || step.part_id != input.part_id
            || step.material_zone_id != input.material_zone_id
            || (!step.source_node_ids.is_empty()
                && step.source_node_ids != effective_source_node_ids(input)?)
            || step.source_revision_id != request.revision.revision_id
            || step.source_revision_sha256 != request.revision.canonical_sha256
            || step.source_vertex_ids != input.source_vertex_ids
            || step.source_edges != input.source_edges
            || step.source_face_ids != input.source_face_ids
            || step.control_points != input.control_points
            || step.faces != input.faces
            || !(1..=2).contains(&step.subdivision_levels)
            || step.max_triangles == 0
        {
            return Err(error("REQUEST_MULTIPART_STEP_SOURCE_BINDING_MISMATCH"));
        }
    }
    Ok(())
}

fn validate_stitched_step_binding(
    revision: &AuthoringMeshV2Revision,
    step: &HighStitchedSubdivisionStep,
) -> Result<(), AuthoringMeshV2HighError> {
    let source = project_revision(revision)?;
    let part = source
        .parts
        .first()
        .ok_or_else(|| error("PROJECTION_NO_PART"))?;
    if step.source_revision_id != revision.revision_id
        || step.source_revision_sha256 != revision.canonical_sha256
        || step.part_id != part.part_id
        || step.material_zone_id != part.material_zone_id
        || step.control_points != part.positions_m
    {
        return Err(error("REQUEST_STITCHED_SOURCE_BINDING_MISMATCH"));
    }
    let vertex_map = revision
        .original
        .vertices
        .iter()
        .map(|vertex| (vertex.vertex_id.as_str(), vertex.position_m))
        .collect::<BTreeMap<_, _>>();
    let vertex_ids = vertex_map
        .keys()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    if step.source_vertex_ids != vertex_ids {
        return Err(error("REQUEST_STITCHED_VERTEX_BINDING_MISMATCH"));
    }
    let vertex_index = vertex_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index as u32))
        .collect::<BTreeMap<_, _>>();
    let half_edges = revision
        .original
        .half_edges
        .iter()
        .map(|half_edge| (half_edge.half_edge_id.as_str(), half_edge))
        .collect::<BTreeMap<_, _>>();
    let face_map = revision
        .original
        .faces
        .iter()
        .map(|face| (face.face_id.as_str(), face))
        .collect::<BTreeMap<_, _>>();
    let mut expected_face_ids = Vec::with_capacity(face_map.len());
    let mut expected_faces = Vec::with_capacity(face_map.len());
    for (face_id, face) in face_map {
        let indices = face
            .half_edge_ids
            .iter()
            .map(|half_edge_id| {
                let half_edge = half_edges
                    .get(half_edge_id.as_str())
                    .ok_or_else(|| error("CPU_FACE_HALF_EDGE_MISSING"))?;
                vertex_index
                    .get(half_edge.origin_vertex_id.as_str())
                    .copied()
                    .ok_or_else(|| error("CPU_STITCHED_VERTEX_MISSING"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        expected_face_ids.push(face_id.to_owned());
        expected_faces.push(indices);
    }
    if step.source_face_ids != expected_face_ids || step.faces != expected_faces {
        return Err(error("REQUEST_STITCHED_FACE_BINDING_MISMATCH"));
    }
    let mut edge_bindings_by_key = BTreeMap::<(u32, u32), String>::new();
    for edge in &revision.original.edges {
        let a = vertex_index
            .get(edge.vertex_ids[0].as_str())
            .copied()
            .ok_or_else(|| error("CPU_STITCHED_EDGE_VERTEX_MISSING"))?;
        let b = vertex_index
            .get(edge.vertex_ids[1].as_str())
            .copied()
            .ok_or_else(|| error("CPU_STITCHED_EDGE_VERTEX_MISSING"))?;
        if edge_bindings_by_key
            .insert((a.min(b), a.max(b)), edge.edge_id.clone())
            .is_some()
        {
            return Err(error("CPU_STITCHED_DUPLICATE_EDGE_ENDPOINT"));
        }
    }
    let expected_edges = edge_bindings_by_key
        .into_iter()
        .map(|((a, b), edge_id)| HighStitchedEdgeBinding {
            edge_id,
            vertex_indices: [a, b],
        })
        .collect::<Vec<_>>();
    if step.source_edges != expected_edges {
        return Err(error("REQUEST_STITCHED_EDGE_BINDING_MISMATCH"));
    }
    Ok(())
}

fn validate_revision(revision: &AuthoringMeshV2Revision) -> Result<(), AuthoringMeshV2HighError> {
    if revision.schema_version != REVISION_SCHEMA_VERSION
        || revision.id_policy != ID_POLICY
        || revision.original.namespace != ORIGINAL_NAMESPACE
        || revision.original.lineage_id != revision.lineage_id
    {
        return Err(error("REVISION_POLICY_MISMATCH"));
    }
    for (value, label) in [
        (&revision.mesh_id, "mesh_id"),
        (&revision.lineage_id, "lineage_id"),
        (&revision.revision_id, "revision_id"),
    ] {
        validate_id(value, label)?;
    }
    if revision.parent_revision_ids.len() > MAX_PARENTS {
        return Err(error("REVISION_PARENT_BUDGET_EXCEEDED"));
    }
    let mut parents = BTreeSet::new();
    for parent in &revision.parent_revision_ids {
        validate_id(parent, "parent_revision_id")?;
        if parent == &revision.revision_id || !parents.insert(parent) {
            return Err(error("REVISION_PARENT_DAG_INVALID"));
        }
    }
    if (revision.revision_index == 0 && !revision.parent_revision_ids.is_empty())
        || (revision.revision_index > 0 && revision.parent_revision_ids.is_empty())
    {
        return Err(error("REVISION_INDEX_PARENT_BINDING_INVALID"));
    }
    if revision.original.canonical_sha256
        != hash_without_field(&revision.original, "canonical_sha256")?
        || revision.canonical_sha256 != hash_without_field(revision, "canonical_sha256")?
    {
        return Err(error("REVISION_CANONICAL_HASH_MISMATCH"));
    }
    validate_original(&revision.original, revision)?;
    if let Some(evaluated) = &revision.evaluated {
        validate_id(&evaluated.artifact_id, "evaluated.artifact_id")?;
        if evaluated.namespace != EVALUATED_NAMESPACE
            || evaluated.source_revision_id != revision.revision_id
            || evaluated.correspondence_status.is_empty()
            || evaluated.correspondence_status.len() > MAX_ID_LENGTH
            || !is_sha256(&evaluated.artifact_sha256)
            || !is_sha256(&evaluated.readback_sha256)
            || evaluated.canonical_sha256 != hash_without_field(evaluated, "canonical_sha256")?
        {
            return Err(error("REVISION_EVALUATED_SIDECAR_INVALID"));
        }
    }
    if revision.source_binding.is_some() && revision.foundation_source_binding.is_some() {
        return Err(error("REVISION_SOURCE_BINDINGS_MUTUALLY_EXCLUSIVE"));
    }
    if let Some(source_binding) = &revision.source_binding {
        validate_source_binding(source_binding, revision)?;
    }
    if let Some(foundation_source_binding) = &revision.foundation_source_binding {
        validate_foundation_source_binding(foundation_source_binding, revision)?;
    }
    if let Some(operation) = &revision.operation {
        validate_operation(operation, revision)?;
    }
    Ok(())
}

fn validate_original(
    original: &Original,
    revision: &AuthoringMeshV2Revision,
) -> Result<(), AuthoringMeshV2HighError> {
    if original.vertices.len() < 3
        || original.vertices.len() > MAX_VERTICES
        || original.edges.is_empty()
        || original.edges.len() > MAX_EDGES
        || original.half_edges.is_empty()
        || original.half_edges.len() > MAX_HALF_EDGES
        || original.corners.is_empty()
        || original.corners.len() > MAX_CORNERS
        || original.faces.is_empty()
        || original.faces.len() > MAX_FACES
    {
        return Err(error("REVISION_TOPOLOGY_BUDGET_INVALID"));
    }
    let vertices = unique_map(
        &original.vertices,
        |value| value.vertex_id.as_str(),
        "vertex",
    )?;
    let edges = unique_map(&original.edges, |value| value.edge_id.as_str(), "edge")?;
    let half_edges = unique_map(
        &original.half_edges,
        |value| value.half_edge_id.as_str(),
        "half_edge",
    )?;
    let corners = unique_map(
        &original.corners,
        |value| value.corner_id.as_str(),
        "corner",
    )?;
    let faces = unique_map(&original.faces, |value| value.face_id.as_str(), "face")?;
    let loops = unique_map(&original.loops, |value| value.loop_id.as_str(), "loop")?;
    let rings = unique_map(&original.rings, |value| value.ring_id.as_str(), "ring")?;
    let mut active_ids = BTreeSet::new();
    for id in vertices
        .keys()
        .chain(edges.keys())
        .chain(half_edges.keys())
        .chain(corners.keys())
        .chain(faces.keys())
        .chain(loops.keys())
        .chain(rings.keys())
    {
        if !active_ids.insert((*id).to_owned()) {
            return Err(error("REVISION_ACTIVE_ID_REUSED"));
        }
    }
    for vertex in vertices.values() {
        to_position(vertex.position_m, "vertex")?;
    }
    for edge in edges.values() {
        if edge.vertex_ids[0] == edge.vertex_ids[1]
            || !vertices.contains_key(edge.vertex_ids[0].as_str())
            || !vertices.contains_key(edge.vertex_ids[1].as_str())
            || !(1..=2).contains(&edge.half_edge_ids.len())
            || edge.boundary != (edge.half_edge_ids.len() == 1)
            || edge
                .half_edge_ids
                .iter()
                .any(|id| !half_edges.contains_key(id.as_str()))
            || distance(
                vertices[edge.vertex_ids[0].as_str()].position_m,
                vertices[edge.vertex_ids[1].as_str()].position_m,
            ) <= MIN_EDGE_LENGTH_M
        {
            return Err(error("REVISION_EDGE_INVALID"));
        }
        if has_duplicate(&edge.half_edge_ids) {
            return Err(error("REVISION_EDGE_HALF_EDGE_DUPLICATE"));
        }
    }
    for half_edge in half_edges.values() {
        let edge = edges
            .get(half_edge.edge_id.as_str())
            .ok_or_else(|| error("REVISION_HALF_EDGE_EDGE_MISSING"))?;
        let next = half_edges
            .get(half_edge.next_id.as_str())
            .ok_or_else(|| error("REVISION_HALF_EDGE_NEXT_MISSING"))?;
        let previous = half_edges
            .get(half_edge.prev_id.as_str())
            .ok_or_else(|| error("REVISION_HALF_EDGE_PREV_MISSING"))?;
        let corner = corners
            .get(half_edge.corner_id.as_str())
            .ok_or_else(|| error("REVISION_HALF_EDGE_CORNER_MISSING"))?;
        if !vertices.contains_key(half_edge.origin_vertex_id.as_str())
            || !faces.contains_key(half_edge.face_id.as_str())
            || next.face_id != half_edge.face_id
            || previous.face_id != half_edge.face_id
            || next.prev_id != half_edge.half_edge_id
            || previous.next_id != half_edge.half_edge_id
            || corner.half_edge_id != half_edge.half_edge_id
            || corner.face_id != half_edge.face_id
            || corner.vertex_id != half_edge.origin_vertex_id
            || half_edge.boundary != edge.boundary
        {
            return Err(error("REVISION_HALF_EDGE_INVARIANT_INVALID"));
        }
        let end = &next.origin_vertex_id;
        if !((half_edge.origin_vertex_id == edge.vertex_ids[0] && *end == edge.vertex_ids[1])
            || (half_edge.origin_vertex_id == edge.vertex_ids[1] && *end == edge.vertex_ids[0]))
        {
            return Err(error("REVISION_HALF_EDGE_ENDPOINT_INVALID"));
        }
        match (&half_edge.twin_id, edge.half_edge_ids.len()) {
            (None, 1) => {}
            (Some(twin_id), 2) => {
                let twin = half_edges
                    .get(twin_id.as_str())
                    .ok_or_else(|| error("REVISION_TWIN_MISSING"))?;
                if twin.twin_id.as_deref() != Some(half_edge.half_edge_id.as_str())
                    || twin.edge_id != half_edge.edge_id
                    || twin.face_id == half_edge.face_id
                    || twin.origin_vertex_id != *end
                    || half_edges[twin.next_id.as_str()].origin_vertex_id
                        != half_edge.origin_vertex_id
                {
                    return Err(error("REVISION_TWIN_SYMMETRY_INVALID"));
                }
            }
            _ => return Err(error("REVISION_TWIN_BOUNDARY_POLICY_INVALID")),
        }
    }
    for corner in corners.values() {
        if corner.ordinal > MAX_FACE_DEGREE as u32
            || !half_edges.contains_key(corner.half_edge_id.as_str())
            || !vertices.contains_key(corner.vertex_id.as_str())
            || !faces.contains_key(corner.face_id.as_str())
            || !optional_finite(corner.uv0.map(|value| value.to_vec()))
            || !optional_finite(corner.normal.map(|value| value.to_vec()))
            || !optional_finite(corner.tangent.map(|value| value.to_vec()))
        {
            return Err(error("REVISION_CORNER_INVALID"));
        }
    }
    let mut owned_half_edges = BTreeSet::new();
    for face in faces.values() {
        if !(3..=MAX_FACE_DEGREE).contains(&face.half_edge_ids.len())
            || !loops.contains_key(face.loop_id.as_str())
            || has_duplicate(&face.half_edge_ids)
        {
            return Err(error("REVISION_FACE_CYCLE_INVALID"));
        }
        let mut points = Vec::with_capacity(face.half_edge_ids.len());
        for (index, half_edge_id) in face.half_edge_ids.iter().enumerate() {
            if !owned_half_edges.insert(half_edge_id.clone()) {
                return Err(error("REVISION_FACE_HALF_EDGE_REUSED"));
            }
            let half_edge = half_edges
                .get(half_edge_id.as_str())
                .ok_or_else(|| error("REVISION_FACE_HALF_EDGE_MISSING"))?;
            if half_edge.face_id != face.face_id
                || half_edge.next_id != face.half_edge_ids[(index + 1) % face.half_edge_ids.len()]
                || half_edge.prev_id
                    != face.half_edge_ids
                        [(index + face.half_edge_ids.len() - 1) % face.half_edge_ids.len()]
            {
                return Err(error("REVISION_FACE_CYCLE_NEXT_PREV_INVALID"));
            }
            points.push(vertices[half_edge.origin_vertex_id.as_str()].position_m);
        }
        let area = (1..points.len() - 1)
            .map(|index| triangle_area(points[0], points[index], points[index + 1]))
            .sum::<f64>();
        let loop_record = &loops[face.loop_id.as_str()];
        if area <= MIN_FACE_AREA_M2
            || loop_record.face_id != face.face_id
            || loop_record.half_edge_ids != face.half_edge_ids
            || face.boundary
                != face
                    .half_edge_ids
                    .iter()
                    .any(|id| half_edges[id.as_str()].boundary)
            || loop_record.boundary != face.boundary
        {
            return Err(error("REVISION_FACE_GEOMETRY_OR_LOOP_INVALID"));
        }
    }
    if owned_half_edges.len() != half_edges.len() {
        return Err(error("REVISION_HALF_EDGE_UNOWNED"));
    }
    for loop_record in loops.values() {
        if !faces.contains_key(loop_record.face_id.as_str())
            || loop_record.half_edge_ids.len() < 3
            || has_duplicate(&loop_record.half_edge_ids)
            || loop_record
                .half_edge_ids
                .iter()
                .any(|id| !half_edges.contains_key(id.as_str()))
        {
            return Err(error("REVISION_LOOP_INVALID"));
        }
    }
    for ring in rings.values() {
        if !ring.boundary
            || ring.edge_ids.is_empty()
            || ring
                .edge_ids
                .iter()
                .any(|id| edges.get(id.as_str()).is_none_or(|edge| !edge.boundary))
        {
            return Err(error("REVISION_RING_INVALID"));
        }
    }
    validate_tombstones(&original.tombstones, &active_ids, revision.revision_index)?;
    Ok(())
}

fn validate_source_binding(
    binding: &AuthoringMeshV2SourceBinding,
    revision: &AuthoringMeshV2Revision,
) -> Result<(), AuthoringMeshV2HighError> {
    if binding.schema_version != SOURCE_BINDING_SCHEMA_VERSION {
        return Err(error("REVISION_SOURCE_BINDING_SCHEMA_INVALID"));
    }
    for (value, label) in [
        (&binding.project_id, "source_binding.project_id"),
        (&binding.candidate_id, "source_binding.candidate_id"),
        (&binding.artifact_id, "source_binding.artifact_id"),
        (&binding.source_node_id, "source_binding.source_node_id"),
        (&binding.part_id, "source_binding.part_id"),
        (&binding.material_zone_id, "source_binding.material_zone_id"),
    ] {
        validate_id(value, label)?;
    }
    if !matches!(
        binding.source_operator_id.as_str(),
        "forgecad.geometry.primitive@2"
            | "forgecad.geometry.profile-extrude@1"
            | "forgecad.geometry.authoring-mesh@1"
    ) {
        return Err(error("REVISION_SOURCE_BINDING_OPERATOR_INVALID"));
    }
    for (value, label) in [
        (
            &binding.candidate_state_sha256,
            "source_binding.candidate_state_sha256",
        ),
        (&binding.artifact_sha256, "source_binding.artifact_sha256"),
        (
            &binding.artifact_readback_sha256,
            "source_binding.artifact_readback_sha256",
        ),
        (
            &binding.geometry_program_sha256,
            "source_binding.geometry_program_sha256",
        ),
        (
            &binding.source_parameters_sha256,
            "source_binding.source_parameters_sha256",
        ),
        (
            &binding.part_output_sha256,
            "source_binding.part_output_sha256",
        ),
    ] {
        if !is_sha256(value) {
            return Err(error(format!(
                "REVISION_SOURCE_BINDING_HASH_INVALID:{label}"
            )));
        }
    }
    if binding
        .position_m
        .iter()
        .chain(binding.rotation_rad.iter())
        .any(|value| !value.is_finite())
    {
        return Err(error("REVISION_SOURCE_BINDING_TRANSFORM_INVALID"));
    }
    if binding.canonical_sha256 != hash_without_field(binding, "canonical_sha256")? {
        return Err(error("REVISION_SOURCE_BINDING_CANONICAL_HASH_MISMATCH"));
    }
    // A candidate binding is provenance only.  The revision identity and its
    // original topology remain the sole source of authored truth.
    let _ = revision;
    Ok(())
}

fn validate_foundation_source_binding(
    binding: &AuthoringMeshV2FoundationSourceBinding,
    revision: &AuthoringMeshV2Revision,
) -> Result<(), AuthoringMeshV2HighError> {
    if binding.schema_version != FOUNDATION_SOURCE_BINDING_SCHEMA_VERSION {
        return Err(error("REVISION_FOUNDATION_SOURCE_BINDING_SCHEMA_INVALID"));
    }
    for (value, label) in [
        (&binding.project_id, "foundation.project_id"),
        (&binding.materialization_id, "foundation.materialization_id"),
        (&binding.record_id, "foundation.record_id"),
        (
            &binding.foundation_request_id,
            "foundation.foundation_request_id",
        ),
        (&binding.source_asset_id, "foundation.source_asset_id"),
        (&binding.part_id, "foundation.part_id"),
        (&binding.material_zone_id, "foundation.material_zone_id"),
        (&binding.authoring_mesh_id, "foundation.authoring_mesh_id"),
        (
            &binding.authoring_mesh_lineage_id,
            "foundation.authoring_mesh_lineage_id",
        ),
        (
            &binding.authoring_mesh_revision_id,
            "foundation.authoring_mesh_revision_id",
        ),
    ] {
        validate_id(value, label)?;
    }
    for (value, label) in [
        (
            &binding.foundation_request_sha256,
            "foundation.foundation_request_sha256",
        ),
        (
            &binding.foundation_result_object_sha256,
            "foundation.foundation_result_object_sha256",
        ),
        (
            &binding.topology_object_sha256,
            "foundation.topology_object_sha256",
        ),
        (
            &binding.socket_map_object_sha256,
            "foundation.socket_map_object_sha256",
        ),
        (
            &binding.rig_map_object_sha256,
            "foundation.rig_map_object_sha256",
        ),
        (
            &binding.fps_presentation_package_object_sha256,
            "foundation.fps_presentation_package_object_sha256",
        ),
        (
            &binding.source_asset_sha256,
            "foundation.source_asset_sha256",
        ),
        (
            &binding.source_part_topology_sha256,
            "foundation.source_part_topology_sha256",
        ),
    ] {
        if !is_sha256(value) {
            return Err(error(format!(
                "REVISION_FOUNDATION_SOURCE_BINDING_HASH_INVALID:{label}"
            )));
        }
    }
    if binding.source_asset_role.is_empty() || binding.source_asset_role.len() > MAX_ID_LENGTH {
        return Err(error(
            "REVISION_FOUNDATION_SOURCE_BINDING_ASSET_ROLE_INVALID",
        ));
    }
    if binding.binding_policy != "foundation-import-part-to-authoring-mesh-v2-source@1"
        || binding.materialization_profile != "part-bounded-authoring-mesh-v2-genesis@1"
        || !binding.source_only
        || binding.quality_status != "structural_only"
        || binding.review_status != "DRAFT_UNREVIEWED"
        || binding.canonicalization_policy != "canonical-json-sha256-excluding-canonical-sha256@1"
    {
        return Err(error("REVISION_FOUNDATION_SOURCE_BINDING_POLICY_INVALID"));
    }
    if binding.authoring_mesh_id != revision.mesh_id
        || binding.authoring_mesh_lineage_id != revision.lineage_id
        || binding.authoring_mesh_revision_id != revision.revision_id
    {
        return Err(error(
            "REVISION_FOUNDATION_SOURCE_BINDING_REVISION_MISMATCH",
        ));
    }
    if binding.canonical_sha256 != hash_without_field(binding, "canonical_sha256")? {
        return Err(error(
            "REVISION_FOUNDATION_SOURCE_BINDING_CANONICAL_HASH_MISMATCH",
        ));
    }
    Ok(())
}

fn validate_operation(
    operation: &TopologyOperation,
    revision: &AuthoringMeshV2Revision,
) -> Result<(), AuthoringMeshV2HighError> {
    if operation.schema_version != OPERATION_SCHEMA_VERSION
        || operation.parent_revision_id
            != revision
                .parent_revision_ids
                .first()
                .cloned()
                .unwrap_or_default()
        || operation.canonical_sha256 != hash_without_field(operation, "canonical_sha256")?
        || operation.operation_lineage_sha256.is_empty()
        || !is_sha256(&operation.operation_lineage_sha256)
        || operation.locality_policy.is_empty()
    {
        return Err(error("REVISION_OPERATION_INVALID"));
    }
    match &operation.kind {
        TopologyOperationKind::SplitEdge => {
            if operation.source_elements.len() != 1
                || operation.source_elements[0].kind != ElementKind::Edge
                || !operation.tombstones.iter().all(|tombstone| {
                    tombstone.retired_revision_index == revision.revision_index
                        && tombstone.operation_lineage_sha256 == operation.operation_lineage_sha256
                })
            {
                return Err(error("REVISION_SPLIT_EDGE_OPERATION_INVALID"));
            }
        }
        TopologyOperationKind::FaceExtrude => {
            if operation.source_elements.len() != 1
                || operation.source_elements[0].kind != ElementKind::Face
                || !operation.tombstones.is_empty()
                || !operation.retired_elements.is_empty()
            {
                return Err(error("REVISION_FACE_EXTRUDE_OPERATION_INVALID"));
            }
        }
        TopologyOperationKind::MoveVertices => {
            if !(1..=32).contains(&operation.source_elements.len())
                || operation
                    .source_elements
                    .iter()
                    .any(|element| element.kind != ElementKind::Vertex)
                || operation
                    .source_elements
                    .windows(2)
                    .any(|pair| pair[0].id.as_str() >= pair[1].id.as_str())
                || operation.source_elements.iter().any(|element| {
                    !revision
                        .original
                        .vertices
                        .iter()
                        .any(|vertex| vertex.vertex_id == element.id)
                })
                || !operation.generated_elements.is_empty()
                || !operation.retired_elements.is_empty()
                || !operation.tombstones.is_empty()
            {
                return Err(error("REVISION_MOVE_VERTICES_OPERATION_INVALID"));
            }
        }
        TopologyOperationKind::OpenFrameNotch => {
            if operation.source_elements.len() != 4
                || operation
                    .source_elements
                    .iter()
                    .any(|element| element.kind != ElementKind::Face)
                || operation.generated_elements.is_empty()
                || operation.retired_elements.is_empty()
                || operation.tombstones.is_empty()
                || operation
                    .source_elements
                    .windows(2)
                    .any(|pair| pair[0].id.as_str() >= pair[1].id.as_str())
                || operation.source_elements.iter().any(|element| {
                    !revision
                        .original
                        .faces
                        .iter()
                        .any(|face| face.face_id == element.id)
                        && !revision.original.tombstones.iter().any(|tombstone| {
                            tombstone.element.kind == element.kind
                                && tombstone.element.id == element.id
                        })
                })
            {
                return Err(error("REVISION_OPEN_FRAME_NOTCH_OPERATION_INVALID"));
            }
        }
        TopologyOperationKind::RearStockVoidRailBow => {
            if operation.source_elements.len() != 2
                || operation
                    .source_elements
                    .iter()
                    .any(|element| element.kind != ElementKind::Edge)
                || operation.generated_elements.is_empty()
                || operation.retired_elements.is_empty()
                || operation.tombstones.is_empty()
                || operation
                    .source_elements
                    .windows(2)
                    .any(|pair| pair[0].id.as_str() >= pair[1].id.as_str())
                || operation.source_elements.iter().any(|element| {
                    !revision
                        .original
                        .edges
                        .iter()
                        .any(|edge| edge.edge_id == element.id)
                        && !revision.original.tombstones.iter().any(|tombstone| {
                            tombstone.element.kind == element.kind
                                && tombstone.element.id == element.id
                        })
                })
            {
                return Err(error("REVISION_REAR_STOCK_VOID_RAIL_BOW_OPERATION_INVALID"));
            }
        }
        TopologyOperationKind::RearStockVoidBoundaryBridge => {
            if operation.source_elements.len() != 2
                || operation
                    .source_elements
                    .iter()
                    .any(|element| element.kind != ElementKind::Edge)
                || operation.generated_elements.is_empty()
                || operation.retired_elements.is_empty()
                || operation.tombstones.is_empty()
                || operation.locality_policy
                    != "rear-stock-void-upper-inner-boundary-bridge-fixed-five-station-depth-wedge@1"
                || operation
                    .source_elements
                    .windows(2)
                    .any(|pair| pair[0].id.as_str() >= pair[1].id.as_str())
                || operation.source_elements.iter().any(|element| {
                    !revision
                        .original
                        .edges
                        .iter()
                        .any(|edge| edge.edge_id == element.id)
                        && !revision.original.tombstones.iter().any(|tombstone| {
                            tombstone.element.kind == element.kind
                                && tombstone.element.id == element.id
                        })
                })
            {
                return Err(error(
                    "REVISION_REAR_STOCK_VOID_BOUNDARY_BRIDGE_OPERATION_INVALID",
                ));
            }
        }
    }
    validate_id(&operation.operation_id, "operation_id")?;
    for element in operation
        .source_elements
        .iter()
        .chain(operation.generated_elements.iter())
        .chain(operation.retired_elements.iter())
    {
        validate_id(&element.id, "operation.element_id")?;
    }
    for tombstone in &operation.tombstones {
        validate_id(&tombstone.element.id, "operation.tombstone_id")?;
        if tombstone.retired_revision_index != revision.revision_index
            || tombstone.operation_lineage_sha256 != operation.operation_lineage_sha256
            || tombstone.reason.is_empty()
        {
            return Err(error("REVISION_OPERATION_TOMBSTONE_INVALID"));
        }
    }
    Ok(())
}

fn validate_tombstones(
    tombstones: &[Tombstone],
    active_ids: &BTreeSet<String>,
    revision_index: u64,
) -> Result<(), AuthoringMeshV2HighError> {
    let mut retired = BTreeSet::new();
    for tombstone in tombstones {
        validate_id(&tombstone.element.id, "tombstone.element_id")?;
        if tombstone.retired_revision_index > revision_index
            || !is_sha256(&tombstone.operation_lineage_sha256)
            || tombstone.reason.is_empty()
            || active_ids.contains(&tombstone.element.id)
            || !retired.insert((tombstone.element.kind.clone(), tombstone.element.id.clone()))
        {
            return Err(error("REVISION_TOMBSTONE_INVALID"));
        }
    }
    Ok(())
}

fn unique_map<'a, T>(
    values: &'a [T],
    id: impl Fn(&T) -> &str,
    label: &str,
) -> Result<BTreeMap<String, &'a T>, AuthoringMeshV2HighError> {
    let mut result = BTreeMap::new();
    for value in values {
        let value_id = id(value);
        validate_id(value_id, label)?;
        if result.insert(value_id.to_owned(), value).is_some() {
            return Err(error("REVISION_DUPLICATE_ELEMENT_ID"));
        }
    }
    Ok(result)
}

fn has_duplicate<T: Ord + Clone>(values: &[T]) -> bool {
    let mut set = BTreeSet::new();
    values.iter().any(|value| !set.insert(value.clone()))
}

fn optional_finite(values: Option<Vec<f64>>) -> bool {
    values.is_none_or(|values| values.iter().all(|value| value.is_finite()))
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let delta = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt()
}

fn triangle_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    0.5 * (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt()
}

fn to_position(position: [f64; 3], context: &str) -> Result<[f32; 3], AuthoringMeshV2HighError> {
    if position
        .iter()
        .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE_M)
    {
        return Err(error(format!("REVISION_POSITION_INVALID:{context}")));
    }
    to_f32_position(position, context)
}

fn to_f32_position(
    position: [f64; 3],
    context: &str,
) -> Result<[f32; 3], AuthoringMeshV2HighError> {
    let result = [position[0] as f32, position[1] as f32, position[2] as f32];
    if result.iter().any(|value| !value.is_finite()) {
        return Err(error(format!(
            "REVISION_POSITION_UNREPRESENTABLE:{context}"
        )));
    }
    Ok(result)
}

fn validate_id(value: &str, label: &str) -> Result<(), AuthoringMeshV2HighError> {
    if value.is_empty()
        || value.len() > MAX_ID_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
    {
        return Err(error(format!("REVISION_ID_INVALID:{label}")));
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hash_value<T: Serialize>(value: &T) -> Result<String, AuthoringMeshV2HighError> {
    let value = serde_json::to_value(value)?;
    Ok(sha256_digest(&canonical_bytes(&value)))
}

fn hash_without_field<T: Serialize>(
    value: &T,
    field: &str,
) -> Result<String, AuthoringMeshV2HighError> {
    let mut value = serde_json::to_value(value)?;
    value[field] = Value::String(String::new());
    Ok(sha256_digest(&canonical_bytes(&value)))
}

fn sha256_digest(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn error(message: impl Into<String>) -> AuthoringMeshV2HighError {
    AuthoringMeshV2HighError(format!("AUTHORING_MESH_V2_HIGH_INVALID:{}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: char) -> String {
        seed.to_string().repeat(64)
    }

    fn revision() -> AuthoringMeshV2Revision {
        let vertices = vec![
            Vertex {
                vertex_id: "v0".to_owned(),
                position_m: [0.0, 0.0, 0.0],
            },
            Vertex {
                vertex_id: "v1".to_owned(),
                position_m: [1.0, 0.0, 0.0],
            },
            Vertex {
                vertex_id: "v2".to_owned(),
                position_m: [1.0, 1.0, 0.0],
            },
            Vertex {
                vertex_id: "v3".to_owned(),
                position_m: [0.0, 1.0, 0.0],
            },
        ];
        let half_edges = vec![
            HalfEdge {
                half_edge_id: "he0".to_owned(),
                origin_vertex_id: "v0".to_owned(),
                edge_id: "e0".to_owned(),
                face_id: "f0".to_owned(),
                corner_id: "c0".to_owned(),
                next_id: "he1".to_owned(),
                prev_id: "he3".to_owned(),
                twin_id: None,
                boundary: true,
            },
            HalfEdge {
                half_edge_id: "he1".to_owned(),
                origin_vertex_id: "v1".to_owned(),
                edge_id: "e1".to_owned(),
                face_id: "f0".to_owned(),
                corner_id: "c1".to_owned(),
                next_id: "he2".to_owned(),
                prev_id: "he0".to_owned(),
                twin_id: None,
                boundary: true,
            },
            HalfEdge {
                half_edge_id: "he2".to_owned(),
                origin_vertex_id: "v2".to_owned(),
                edge_id: "e2".to_owned(),
                face_id: "f0".to_owned(),
                corner_id: "c2".to_owned(),
                next_id: "he3".to_owned(),
                prev_id: "he1".to_owned(),
                twin_id: None,
                boundary: true,
            },
            HalfEdge {
                half_edge_id: "he3".to_owned(),
                origin_vertex_id: "v3".to_owned(),
                edge_id: "e3".to_owned(),
                face_id: "f0".to_owned(),
                corner_id: "c3".to_owned(),
                next_id: "he0".to_owned(),
                prev_id: "he2".to_owned(),
                twin_id: None,
                boundary: true,
            },
        ];
        let corners = (0..4)
            .map(|index| Corner {
                corner_id: format!("c{index}"),
                half_edge_id: format!("he{index}"),
                vertex_id: format!("v{index}"),
                face_id: "f0".to_owned(),
                ordinal: index,
                uv0: None,
                normal: None,
                tangent: None,
                seam: false,
            })
            .collect();
        let edges = (0..4)
            .map(|index| Edge {
                edge_id: format!("e{index}"),
                vertex_ids: [format!("v{index}"), format!("v{}", (index + 1) % 4)],
                half_edge_ids: vec![format!("he{index}")],
                boundary: true,
            })
            .collect();
        let face = Face {
            face_id: "f0".to_owned(),
            half_edge_ids: vec![
                "he0".to_owned(),
                "he1".to_owned(),
                "he2".to_owned(),
                "he3".to_owned(),
            ],
            loop_id: "l0".to_owned(),
            boundary: true,
        };
        let loop_record = LoopRecord {
            loop_id: "l0".to_owned(),
            face_id: "f0".to_owned(),
            half_edge_ids: face.half_edge_ids.clone(),
            boundary: true,
        };
        let mut original = Original {
            namespace: ORIGINAL_NAMESPACE.to_owned(),
            lineage_id: "lineage0".to_owned(),
            vertices,
            edges,
            half_edges,
            corners,
            faces: vec![face],
            loops: vec![loop_record],
            rings: Vec::new(),
            tombstones: Vec::new(),
            canonical_sha256: String::new(),
        };
        original.canonical_sha256 = hash_without_field(&original, "canonical_sha256").unwrap();
        let mut revision = AuthoringMeshV2Revision {
            schema_version: REVISION_SCHEMA_VERSION.to_owned(),
            mesh_id: "mesh0".to_owned(),
            lineage_id: "lineage0".to_owned(),
            revision_id: "revision0".to_owned(),
            parent_revision_ids: Vec::new(),
            revision_index: 0,
            operation: None,
            original,
            evaluated: None,
            source_binding: None,
            foundation_source_binding: None,
            id_policy: ID_POLICY.to_owned(),
            canonical_sha256: String::new(),
        };
        revision.canonical_sha256 = hash_without_field(&revision, "canonical_sha256").unwrap();
        revision
    }

    #[test]
    fn v2_projection_is_closed_and_deterministic() {
        let revision = revision();
        let first = project_revision(&revision).expect("projection");
        let second = project_revision(&revision).expect("projection replay");
        assert_eq!(first, second);
        assert_eq!(first.parts[0].positions_m.len(), 4);
        assert_eq!(first.parts[0].indices, vec![[0, 1, 2], [0, 2, 3]]);
    }

    #[test]
    fn cpu_v2_bridge_is_non_destructive_and_replay_exact() {
        let request = cpu_request(
            revision(),
            1,
            32,
            HighEvaluatorBudgets {
                max_steps: 4,
                max_output_vertices: 1024,
                max_output_triangles: 2048,
            },
        )
        .expect("CPU request");
        let first = evaluate(&request).expect("V2 High evaluation");
        let second = evaluate(&request).expect("V2 High replay");
        assert_eq!(first, second);
        assert_eq!(first.evaluation.base_triangle_count, 2);
        assert_eq!(first.evaluation.evaluated_triangle_count, 8);
        assert!(first.non_destructive);
        assert!(!first.runtime_write_performed);
        verify_readback(&first).expect("strict readback");
    }

    #[test]
    fn v2_high_glb_artifact_adapter_is_hash_bound_and_byte_exact() {
        let request = cpu_request(
            revision(),
            1,
            32,
            HighEvaluatorBudgets {
                max_steps: 4,
                max_output_vertices: 1024,
                max_output_triangles: 2048,
            },
        )
        .expect("CPU request");
        let result = evaluate(&request).expect("V2 High evaluation");
        let first = crate::glb::lower_authoring_mesh_v2_high_result_with_cohort(
            &result,
            Some(&"a".repeat(64)),
        )
        .expect("V2 High GLB lowering");
        let second = crate::glb::lower_authoring_mesh_v2_high_result_with_cohort(
            &result,
            Some(&"a".repeat(64)),
        )
        .expect("V2 High GLB replay");
        assert_eq!(first.glb, second.glb);
        assert_eq!(first.glb_sha256, second.glb_sha256);
        assert_eq!(first.readback, second.readback);
        assert_eq!(first.readback.glb_sha256, first.glb_sha256);
        assert_eq!(first.readback.artifact_sha256, result.canonical_sha256);
        assert_eq!(first.readback.high_result_sha256, result.canonical_sha256);
        assert_eq!(
            first.readback.high_worker_build_cohort_sha256,
            Some("a".repeat(64))
        );

        let mut malformed_hash = result.clone();
        malformed_hash.canonical_sha256 = "0".repeat(64);
        assert!(crate::glb::lower_authoring_mesh_v2_high_result(&malformed_hash).is_err());

        let mut duplicate_parts = result.clone();
        duplicate_parts
            .evaluation
            .evaluated_parts
            .push(duplicate_parts.evaluation.evaluated_parts[0].clone());
        assert!(crate::glb::lower_authoring_mesh_v2_high_result(&duplicate_parts).is_err());
    }

    #[test]
    fn source_binding_projection_preserves_semantic_ids_and_opaque_lineage() {
        let mut revision = revision();
        let mut binding = AuthoringMeshV2SourceBinding {
            schema_version: SOURCE_BINDING_SCHEMA_VERSION.to_owned(),
            project_id: "project-source".to_owned(),
            candidate_id: "candidate-source".to_owned(),
            candidate_state_sha256: hash('a'),
            artifact_id: "artifact-source".to_owned(),
            artifact_sha256: hash('b'),
            artifact_readback_sha256: hash('c'),
            geometry_program_sha256: hash('d'),
            source_node_id: "source-node".to_owned(),
            part_id: "source-part".to_owned(),
            material_zone_id: "source-zone".to_owned(),
            solid: true,
            source_operator_id: "forgecad.geometry.primitive@2".to_owned(),
            source_parameters_sha256: hash('e'),
            part_output_sha256: hash('f'),
            position_m: [0.0, 0.0, 0.0],
            rotation_rad: [0.0, 0.0, 0.0],
            canonical_sha256: String::new(),
        };
        binding.canonical_sha256 = hash_without_field(&binding, "canonical_sha256").unwrap();
        revision.source_binding = Some(binding.clone());
        revision.canonical_sha256 = hash_without_field(&revision, "canonical_sha256").unwrap();

        let projected = project_revision(&revision).expect("source-bound projection");
        let part = &projected.parts[0];
        assert_eq!(part.source_node_id, binding.source_node_id);
        assert_eq!(part.part_id, binding.part_id);
        assert_eq!(part.material_zone_id, binding.material_zone_id);
        let lineage = format!("source-binding-{}", binding.canonical_sha256);
        assert!(part.source_element_lineage.contains(&lineage));
        assert!(part.source_element_lineage.iter().all(|value| {
            value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        }));
        assert!(!part
            .source_element_lineage
            .iter()
            .any(|value| value.starts_with("source-binding:")));
        let request = cpu_request(
            revision,
            1,
            32,
            HighEvaluatorBudgets {
                max_steps: 4,
                max_output_vertices: 1024,
                max_output_triangles: 2048,
            },
        )
        .expect("source-bound CPU request");
        let result = evaluate(&request).expect("source-bound CPU evaluation");
        assert_eq!(result.source_mesh.parts[0].source_node_id, "source-node");
        assert_eq!(result.source_mesh.parts[0].part_id, "source-part");
        assert_eq!(result.source_mesh.parts[0].material_zone_id, "source-zone");
    }

    #[test]
    fn foundation_binding_projection_preserves_part_material_and_revision_lineage() {
        let mut revision = revision();
        let mut binding = AuthoringMeshV2FoundationSourceBinding {
            schema_version: FOUNDATION_SOURCE_BINDING_SCHEMA_VERSION.to_owned(),
            project_id: "project-foundation".to_owned(),
            materialization_id: "materialization-foundation".to_owned(),
            record_id: "record-foundation".to_owned(),
            foundation_request_id: "foundation-request".to_owned(),
            foundation_request_sha256: hash('a'),
            foundation_result_object_sha256: hash('b'),
            topology_object_sha256: hash('c'),
            socket_map_object_sha256: hash('d'),
            rig_map_object_sha256: hash('e'),
            fps_presentation_package_object_sha256: hash('f'),
            source_asset_id: "source-asset".to_owned(),
            source_asset_sha256: hash('a'),
            source_asset_role: "weapon-foundation".to_owned(),
            part_id: "foundation-part".to_owned(),
            material_zone_id: "foundation-zone".to_owned(),
            source_part_topology_sha256: hash('b'),
            authoring_mesh_id: revision.mesh_id.clone(),
            authoring_mesh_lineage_id: revision.lineage_id.clone(),
            authoring_mesh_revision_id: revision.revision_id.clone(),
            binding_policy: "foundation-import-part-to-authoring-mesh-v2-source@1".to_owned(),
            materialization_profile: "part-bounded-authoring-mesh-v2-genesis@1".to_owned(),
            source_only: true,
            quality_status: "structural_only".to_owned(),
            review_status: "DRAFT_UNREVIEWED".to_owned(),
            canonicalization_policy: "canonical-json-sha256-excluding-canonical-sha256@1"
                .to_owned(),
            canonical_sha256: String::new(),
        };
        binding.canonical_sha256 = hash_without_field(&binding, "canonical_sha256").unwrap();
        revision.foundation_source_binding = Some(binding.clone());
        revision.canonical_sha256 = hash_without_field(&revision, "canonical_sha256").unwrap();

        let projected = project_revision(&revision).expect("foundation-bound projection");
        let part = &projected.parts[0];
        assert_eq!(part.source_node_id, revision.revision_id);
        assert_eq!(part.part_id, binding.part_id);
        assert_eq!(part.material_zone_id, binding.material_zone_id);
        let lineage = format!("foundation-source-binding-{}", binding.canonical_sha256);
        assert!(part.source_element_lineage.contains(&lineage));
        assert!(part.source_element_lineage.iter().all(|value| {
            value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        }));
    }

    #[test]
    fn execution_envelope_derives_steps_and_uses_v2_readback_names() {
        let revision = revision();
        let mut request = AuthoringMeshV2HighExecutionRequest {
            schema_version: EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            operation: EXECUTION_OPERATION.to_owned(),
            revision_sha256: revision.canonical_sha256.clone(),
            revision,
            part_inputs: Vec::new(),
            subdivision_levels: 1,
            max_triangles_per_face: 32,
            budgets: HighEvaluatorBudgets {
                max_steps: 4,
                max_output_vertices: 1024,
                max_output_triangles: 2048,
            },
            canonical_sha256: String::new(),
        };
        request.canonical_sha256 = hash_without_field(&request, "canonical_sha256").unwrap();

        let bytes = run_execution_json(&serde_json::to_vec(&request).unwrap())
            .expect("closed execution envelope");
        let mut wire_value: Value = serde_json::from_slice(&bytes).unwrap();
        let wire_hash = wire_value["canonical_sha256"].as_str().unwrap().to_owned();
        wire_value["canonical_sha256"] = Value::String(String::new());
        assert_eq!(wire_hash, sha256_digest(&canonical_bytes(&wire_value)));
        assert_eq!(
            wire_value["readback"]["high_evaluation_sha256"],
            Value::String(hash_value(&wire_value["evaluation"]).unwrap())
        );
        assert_eq!(
            wire_value["readback"]["projected_source_mesh_sha256"],
            Value::String(hash_value(&wire_value["source_mesh"]).unwrap())
        );
        let result: AuthoringMeshV2HighResult = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(result.schema_version, "AuthoringMeshV2HighResult@2");
        assert_eq!(
            result.readback.schema_version,
            "AuthoringMeshV2HighReadback@2"
        );
        assert!(result.readback.high_evaluation_sha256.len() == 64);
        assert_eq!(result.high_worker_algorithm_sha256, algorithm_sha256());
        assert_eq!(
            result.readback.high_worker_algorithm_sha256,
            result.high_worker_algorithm_sha256
        );
        assert_eq!(result.evaluation.evaluated_triangle_count, 8);

        let mut legacy_gap = serde_json::to_value(&request).unwrap();
        legacy_gap["steps"] = Value::Array(Vec::new());
        assert!(run_execution_json(&serde_json::to_vec(&legacy_gap).unwrap()).is_err());
    }

    #[test]
    fn wire_result_round_trips_through_v2_glb_lowering_without_typed_hash_drift() {
        let revision = revision();
        let mut request = AuthoringMeshV2HighExecutionRequest {
            schema_version: EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            operation: EXECUTION_OPERATION.to_owned(),
            revision_sha256: revision.canonical_sha256.clone(),
            revision,
            part_inputs: Vec::new(),
            subdivision_levels: 1,
            max_triangles_per_face: 32,
            budgets: HighEvaluatorBudgets {
                max_steps: 4,
                max_output_vertices: 1024,
                max_output_triangles: 2048,
            },
            canonical_sha256: String::new(),
        };
        request.canonical_sha256 = hash_without_field(&request, "canonical_sha256").unwrap();
        let bytes = run_execution_json(&serde_json::to_vec(&request).unwrap()).unwrap();
        let wire: Value = serde_json::from_slice(&bytes).unwrap();
        let wire_hash = wire["canonical_sha256"].as_str().unwrap().to_owned();
        let lowered =
            crate::glb::lower_authoring_mesh_v2_high_result_wire(&wire, Some(&"a".repeat(64)))
                .expect("raw wire result must lower");
        assert_eq!(lowered.artifact_sha256, wire_hash);
        assert_eq!(lowered.readback.high_result_sha256, wire_hash);
        assert_eq!(lowered.glb_sha256, sha256_digest(&lowered.glb));
        assert!(!lowered.glb.is_empty());
    }

    #[test]
    fn wire_result_hash_tampering_is_rejected_at_the_raw_boundary() {
        let revision = revision();
        let mut request = AuthoringMeshV2HighExecutionRequest {
            schema_version: EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
            operation: EXECUTION_OPERATION.to_owned(),
            revision_sha256: revision.canonical_sha256.clone(),
            revision,
            part_inputs: Vec::new(),
            subdivision_levels: 1,
            max_triangles_per_face: 32,
            budgets: HighEvaluatorBudgets {
                max_steps: 4,
                max_output_vertices: 1024,
                max_output_triangles: 2048,
            },
            canonical_sha256: String::new(),
        };
        request.canonical_sha256 = hash_without_field(&request, "canonical_sha256").unwrap();
        let bytes = run_execution_json(&serde_json::to_vec(&request).unwrap()).unwrap();
        let wire: Value = serde_json::from_slice(&bytes).unwrap();

        let mut source = wire.clone();
        source["source_mesh"]["parts"][0]["positions_m"][0][0] = Value::from(9.0);
        recanonicalize_wire_root(&mut source);
        assert!(verify_wire_result(&source).is_err());

        let mut evaluation = wire.clone();
        evaluation["evaluation"]["source_mesh_sha256"] = Value::String("0".repeat(64));
        recanonicalize_wire_root(&mut evaluation);
        assert!(verify_wire_result(&evaluation).is_err());

        let mut readback = wire.clone();
        readback["readback"]["high_evaluation_sha256"] = Value::String("0".repeat(64));
        recanonicalize_wire_root(&mut readback);
        assert!(verify_wire_result(&readback).is_err());

        let mut result_hash = wire;
        result_hash["canonical_sha256"] = Value::String("0".repeat(64));
        assert!(verify_wire_result(&result_hash).is_err());
    }

    fn recanonicalize_wire_root(value: &mut Value) {
        value["canonical_sha256"] = Value::String(String::new());
        value["canonical_sha256"] = Value::String(sha256_digest(&canonical_bytes(value)));
    }

    #[test]
    fn evaluated_sidecar_is_not_used_as_authority() {
        let mut revision = revision();
        let mut evaluated = EvaluatedSidecar {
            namespace: EVALUATED_NAMESPACE.to_owned(),
            source_revision_id: revision.revision_id.clone(),
            artifact_id: "artifact0".to_owned(),
            artifact_sha256: hash('a'),
            readback_sha256: hash('b'),
            correspondence_status: "non_bijective".to_owned(),
            canonical_sha256: String::new(),
        };
        evaluated.canonical_sha256 = hash_without_field(&evaluated, "canonical_sha256").unwrap();
        revision.evaluated = Some(evaluated);
        revision.canonical_sha256 = hash_without_field(&revision, "canonical_sha256").unwrap();
        let projected = project_revision(&revision).expect("project original only");
        assert_eq!(projected.parts[0].positions_m.len(), 4);
        assert_eq!(projected.parts[0].indices.len(), 2);
    }
}
