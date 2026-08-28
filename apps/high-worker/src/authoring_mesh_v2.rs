//! Closed `AuthoringMesh@2` to Native High projection.
//!
//! The Runtime-owned AuthoringMesh V2 kernel deliberately is not a dependency
//! of this standalone worker.  This module mirrors its serialized revision
//! contract at the process boundary, validates the original half-edge graph,
//! and projects only original positions/faces into the existing non-destructive
//! High evaluator.  Evaluated sidecars and caller supplied element identities
//! never become High authoring truth.

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
pub const RESULT_SCHEMA_VERSION: &str = "AuthoringMeshV2HighResult@1";
pub const READBACK_SCHEMA_VERSION: &str = "AuthoringMeshV2HighReadback@1";
pub const OPERATION: &str = "forgecad.production.authoring-mesh-v2-high-evaluate@1";
pub const REVISION_SCHEMA_VERSION: &str = "AuthoringMeshRevision@2";
pub const ORIGINAL_SCHEMA_VERSION: &str = "AuthoringMesh@2";
pub const OPERATION_SCHEMA_VERSION: &str = "AuthoringMeshTopologyOperation@2";
pub const ID_POLICY: &str = "runtime-derived-lineage-operation-parent-stable-no-reuse@2";
pub const ORIGINAL_NAMESPACE: &str = "original";
pub const EVALUATED_NAMESPACE: &str = "evaluated";
pub const STITCHED_EVALUATOR_CONTRACT: &str = "forgecad-owned-cpu-catmull-clark-stitched-quad@1";

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
    pub source_binding: Option<Value>,
    pub id_policy: String,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshV2HighRequest {
    pub schema_version: String,
    pub operation: String,
    pub revision: AuthoringMeshV2Revision,
    pub revision_sha256: String,
    pub steps: Vec<HighEvaluatorStep>,
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
    pub high_result_sha256: String,
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

    let material_zone_hash = hash_value(&json!({
        "mesh_id": revision.mesh_id.clone(),
        "lineage_id": revision.lineage_id.clone(),
    }))?;
    let material_zone_id = format!("material-zone-{}", &material_zone_hash[..24]);
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
        source_element_lineage.insert(format!("source-binding:{}", hash_value(source_binding)?));
    }
    Ok(HighEvaluatorSourceMesh {
        schema_version: "HighEvaluatorSourceMesh@1".to_owned(),
        parts: vec![HighEvaluatorPart {
            operand_id: revision.mesh_id.clone(),
            part_id: revision.mesh_id.clone(),
            source_node_id: revision.revision_id.clone(),
            material_zone_id,
            source_element_lineage: source_element_lineage.into_iter().collect(),
            positions_m: vertices.values().copied().collect(),
            indices,
        }],
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
    Ok(canonical_bytes(&serde_json::to_value(result)?))
}

/// Convenience constructor for the fixed CPU path. It accepts only all-quad
/// authored faces and emits one stitched topology step. The original V2
/// revision remains immutable; the evaluated sibling owns shared edge/vertex
/// indexing and retains stable source IDs in the evaluator lineage.
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

/// Build the explicit `StitchedSubdivision` evaluator contract for an
/// AuthoringMesh V2 revision. `max_triangles_per_face` is retained at this
/// convenience boundary for compatibility and is multiplied by the authored
/// face count with checked arithmetic before entering the worker contract.
pub fn cpu_stitched_request(
    revision: AuthoringMeshV2Revision,
    subdivision_levels: usize,
    max_triangles_per_face: usize,
    budgets: HighEvaluatorBudgets,
) -> Result<AuthoringMeshV2HighRequest, AuthoringMeshV2HighError> {
    if subdivision_levels > 2 || max_triangles_per_face == 0 {
        return Err(error("CPU_POLICY_INVALID"));
    }
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
        if face.half_edge_ids.len() != 4 {
            return Err(error("CPU_REGULAR_QUAD_REQUIRES_QUAD_FACES"));
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
        faces.push([indices[0], indices[1], indices[2], indices[3]]);
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
                == "shared-edge-and-shared-vertex-indexing@1"
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
    if result.replay_count != 2
        || !result.replay_byte_exact
        || !result.non_destructive
        || result.runtime_write_performed
        || result.production_stage_advanced
        || result.candidate_confirmed
        || result.version_created
        || result.export_performed
        || result.quality_status != "structural_only"
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
        high_result_sha256: evaluation_hash,
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

fn evaluate_once(
    request: &AuthoringMeshV2HighRequest,
) -> Result<AuthoringMeshV2HighResult, AuthoringMeshV2HighError> {
    let source_mesh = project_revision(&request.revision)?;
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
            "STITCHED_CPU_CATMULL_CLARK_ALL_QUAD_MANIFOLD_BOUNDARY@1".to_owned(),
            "SHARED_EDGE_AND_VERTEX_INDEXING@1".to_owned(),
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
            high_result_sha256: String::new(),
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
        high_result_sha256: hash_value(&result.evaluation)?,
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
    for step in &request.steps {
        if let HighEvaluatorStep::StitchedSubdivision(step) = step {
            validate_stitched_step_binding(&request.revision, step)?;
        }
    }
    if request.steps.is_empty() {
        return Err(error("REQUEST_STEPS_EMPTY"));
    }
    if request.canonical_sha256 != hash_without_field(request, "canonical_sha256")? {
        return Err(error("REQUEST_CANONICAL_HASH_MISMATCH"));
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
        if face.half_edge_ids.len() != 4 {
            return Err(error("CPU_REGULAR_QUAD_REQUIRES_QUAD_FACES"));
        }
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
        expected_faces.push([indices[0], indices[1], indices[2], indices[3]]);
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
                        && tombstone.operation_lineage_sha256
                            == operation.operation_lineage_sha256
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
