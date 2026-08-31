//! ForgeCAD-owned Native High Worker source slice.
//!
//! This crate is deliberately standalone while the Runtime integration is
//! being reviewed.  It accepts a closed, hash-bound AuthoringMesh and a
//! non-destructive DetailGraph, then emits a deterministic HighMeshArtifact
//! projection.  The base mesh is never rewritten: support-loop patches and
//! floating details are separate primitives, while creases are explicit
//! metadata-only edges.  There is no filesystem, network, process, script,
//! CAS, SQLite, or external DCC dependency in this worker.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub mod authoring_mesh_v2;
pub mod evaluator;
pub mod glb;
pub mod module;

pub use glb::{
    inspect_high_mesh_glb, lower_authoring_mesh_v2_high_result,
    lower_authoring_mesh_v2_high_result_with_cohort, lower_high_mesh_artifact,
    lower_high_mesh_artifact_bytes, readback_authoring_mesh_v2_high_glb, readback_high_mesh_glb,
    AuthoringMeshV2HighGlbArtifact, AuthoringMeshV2HighGlbReadback, HighGlbArtifact, HighGlbError,
    HighGlbReadback,
};

pub const REQUEST_SCHEMA_VERSION: &str = "HighMeshWorkerRequest@1";
pub const ARTIFACT_SCHEMA_VERSION: &str = "HighMeshArtifact@1";
pub const AUTHORING_MESH_SCHEMA_VERSION: &str = "AuthoringMeshCanonical@1";
pub const DETAIL_GRAPH_SCHEMA_VERSION: &str = "DetailGraph@1";
pub const OPERATION: &str = "forgecad.production.high-mesh-prepare@1";
pub const POLICY: &str = "forgecad-native-high-detail-graph@1";
pub const ALGORITHM: &str =
    "forgecad-native-high-mesh@1|base-preserve|support-loop-arc@2|crease-metadata|detached-floater-box|no-rng-no-time-no-network";

const MAX_REQUEST_BYTES: usize = 8 * 1024 * 1024;
const MAX_PARTS: usize = 128;
const MAX_VERTICES_PER_PART: usize = 100_000;
const MAX_TRIANGLES_PER_PART: usize = 200_000;
const MAX_DETAIL_NODES: usize = 256;
const MAX_OUTPUT_VERTICES: usize = 300_000;
const MAX_OUTPUT_TRIANGLES: usize = 600_000;
const MAX_COORDINATE_ABS_M: f32 = 100.0;
const MAX_DETAIL_SIZE_M: f32 = 10.0;
const EPSILON: f32 = 1.0e-6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighWorkerError(pub String);

impl fmt::Display for HighWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for HighWorkerError {}

impl From<serde_json::Error> for HighWorkerError {
    fn from(error: serde_json::Error) -> Self {
        Self(format!("HIGH_WORKER_JSON_INVALID:{error}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighMeshWorkerRequest {
    pub schema_version: String,
    pub operation: String,
    /// Runtime-owned durable canonical payload.  It is deliberately kept as
    /// JSON at this standalone boundary so the worker can validate the exact
    /// product contract without importing Runtime/MCP/CAS crates.
    pub source_authoring_mesh: Value,
    pub source_authoring_mesh_sha256: String,
    pub detail_graph: DetailGraph,
    pub detail_graph_canonical_sha256: String,
    pub budgets: HighWorkerBudgets,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringMeshSource {
    pub schema_version: String,
    pub parts: Vec<AuthoringPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoringPart {
    pub part_id: String,
    pub source_node_id: String,
    pub material_zone_id: String,
    pub vertices: Vec<[f32; 3]>,
    pub indices: Vec<[u32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailGraph {
    pub schema_version: String,
    pub nodes: Vec<DetailNode>,
}

/// One closed node shape is used for all detail kinds.  Kind-specific fields
/// are validated fail-closed in `validate_detail_node`; unknown keys are
/// rejected by serde before execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailNode {
    pub node_id: String,
    pub kind: String,
    pub parent_part_id: String,
    #[serde(default)]
    pub parent_node_id: Option<String>,
    #[serde(default)]
    pub source_edge: Option<String>,
    #[serde(default)]
    pub width_m: Option<f32>,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub sharpness: Option<f32>,
    #[serde(default)]
    pub center_m: Option<[f32; 3]>,
    #[serde(default)]
    pub size_m: Option<[f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HighWorkerBudgets {
    pub max_detail_nodes: u32,
    pub max_output_vertices: u32,
    pub max_output_triangles: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HighMeshArtifact {
    pub schema_version: String,
    pub operation: String,
    pub policy: String,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub source_authoring_mesh_sha256: String,
    pub detail_graph_canonical_sha256: String,
    pub request_sha256: String,
    pub input_sha256: String,
    pub high_worker_algorithm_sha256: String,
    pub high_worker_build_cohort_sha256: String,
    pub replay_count: u32,
    pub replay_byte_exact: bool,
    pub base_parts: Vec<HighMeshPrimitive>,
    pub detail_primitives: Vec<HighMeshPrimitive>,
    pub detail_lineage: Vec<DetailLineage>,
    pub part_ids: Vec<String>,
    pub material_zone_ids: Vec<String>,
    pub triangle_count: u64,
    pub base_triangle_count: u64,
    pub detail_triangle_count: u64,
    pub non_destructive: bool,
    pub high_topology_status: String,
    pub high_authoring_topology_status: String,
    pub uv_status: String,
    pub tangent_status: String,
    pub structural_status: String,
    pub visual_status: String,
    pub human_status: String,
    pub engine_status: String,
    pub distribution_status: String,
    pub quality_status: String,
    pub hard_gate_passed: bool,
    pub runtime_write_performed: bool,
    pub production_stage_advanced: bool,
    pub candidate_confirmed: bool,
    pub version_created: bool,
    pub export_performed: bool,
    pub canonical_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HighMeshPrimitive {
    pub primitive_id: String,
    pub kind: String,
    pub part_id: String,
    /// Complete source-node lineage for one semantic Part.  Legacy artifacts
    /// leave this empty and use the scalar compatibility field below.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_node_ids: Vec<String>,
    pub source_node_id: String,
    pub material_zone_id: String,
    pub source_element_lineage: Vec<String>,
    pub geometry: HighMeshGeometry,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HighMeshGeometry {
    pub positions_m: Vec<[f32; 3]>,
    pub indices: Vec<[u32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DetailLineage {
    pub detail_node_id: String,
    pub detail_kind: String,
    pub parent_part_id: String,
    pub parent_node_id: Option<String>,
    pub source_edge: Option<String>,
    pub source_vertex_ids: Vec<String>,
    pub source_loop_ids: Vec<String>,
    pub source_face_ids: Vec<String>,
    pub output_primitive_id: String,
    pub relation: String,
    pub geometry_delta: String,
    pub non_destructive: bool,
}

#[derive(Debug, Clone, Copy)]
struct EdgeInfo {
    face_count: usize,
    face_normal: [f32; 3],
}

#[derive(Debug, Clone)]
struct StableEdge {
    edge_id: String,
    vertex_ids: [String; 2],
    vertex_indices: (u32, u32),
    loop_ids: Vec<String>,
    face_ids: Vec<String>,
    /// Per-face in-plane directions from the edge toward the face interior.
    /// These are derived from the canonical face cycles and allow the bounded
    /// support-loop evaluator to form a deterministic chamfer arc instead of
    /// emitting two unrelated offset strips.
    face_inward: Vec<[f32; 3]>,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Debug, Clone)]
struct ValidatedInput {
    parts: BTreeMap<String, AuthoringPart>,
    edges: BTreeMap<(String, u32, u32), EdgeInfo>,
    stable_edges: BTreeMap<String, StableEdge>,
    stable_vertex_ids: Vec<String>,
    stable_face_ids: Vec<String>,
    stable_loop_ids: Vec<String>,
    canonical_mesh_id: String,
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    head_candidate_id: String,
    head_candidate_state_sha256: String,
    source_lineage_sha256: String,
    part_id: String,
    authoring_node_id: String,
}

/// Execute a closed request.  The request is evaluated twice before its
/// artifact digest is finalized; any replay divergence fails closed.
pub fn run(request: &HighMeshWorkerRequest) -> Result<HighMeshArtifact, HighWorkerError> {
    validate_request(request)?;
    let request_sha256 = sha256_value(request)?;
    let input_sha256 = sha256_pair(&request.source_authoring_mesh, &request.detail_graph)?;

    let first = evaluate(request, &request_sha256, &input_sha256)?;
    let second = evaluate(request, &request_sha256, &input_sha256)?;
    let first_preimage = artifact_preimage(&first)?;
    let second_preimage = artifact_preimage(&second)?;
    if canonical_bytes(&first_preimage) != canonical_bytes(&second_preimage) {
        return Err(HighWorkerError(
            "HIGH_WORKER_REPLAY_NON_DETERMINISTIC".to_owned(),
        ));
    }

    let digest = sha256_bytes(&canonical_bytes(&first_preimage));
    let mut artifact = first;
    artifact.artifact_sha256 = digest.clone();
    artifact.canonical_sha256 = digest.clone();
    artifact.artifact_id = format!("high-mesh-{}", &digest[..24]);
    Ok(artifact)
}

/// Parse one JSON request and return a JSON artifact.  This is useful to the
/// sibling one-shot binary and keeps the typed contract at the crate boundary.
pub fn run_json(input: &[u8]) -> Result<Vec<u8>, HighWorkerError> {
    if input.len() > MAX_REQUEST_BYTES {
        return Err(HighWorkerError("HIGH_WORKER_REQUEST_TOO_LARGE".to_owned()));
    }
    let request: HighMeshWorkerRequest = serde_json::from_slice(input)?;
    let artifact = run(&request)?;
    let value = serde_json::to_value(artifact)?;
    Ok(canonical_bytes(&value))
}

#[derive(Debug, Clone)]
struct CanonicalSource {
    parts: Vec<AuthoringPart>,
    stable_edges: BTreeMap<String, StableEdge>,
    stable_vertex_ids: Vec<String>,
    stable_face_ids: Vec<String>,
    stable_loop_ids: Vec<String>,
    canonical_mesh_id: String,
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    head_candidate_id: String,
    head_candidate_state_sha256: String,
    source_lineage_sha256: String,
    part_id: String,
    authoring_node_id: String,
}

const CANONICAL_MESH_FIELDS: &[&str] = &[
    "schema_version",
    "canonical_mesh_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "authoring_node_id",
    "part_id",
    "source_program_object_sha256",
    "source_program_sha256",
    "source_artifact_object_sha256",
    "source_artifact_sha256",
    "source_artifact_readback_object_sha256",
    "source_artifact_readback_sha256",
    "source_lineage_sha256",
    "representation",
    "storage_policy",
    "writer_policy",
    "original_identity",
    "evaluated_identity",
    "cross_version_stable",
    "cross_version_stability",
    "counts",
    "vertices",
    "edges",
    "half_edges",
    "corners",
    "faces",
    "loops",
    "rings",
    "topology",
    "canonicalization_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "quality_status",
    "canonical_sha256",
];

fn source_object<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>, HighWorkerError> {
    value
        .as_object()
        .ok_or_else(|| HighWorkerError(format!("HIGH_WORKER_CANONICAL_FIELD_INVALID:{context}")))
}

fn source_exact_object<'a>(
    value: &'a Value,
    expected: &[&str],
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>, HighWorkerError> {
    let object = source_object(value, context)?;
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(HighWorkerError(format!(
            "HIGH_WORKER_CANONICAL_FIELDS_INVALID:{context}"
        )));
    }
    Ok(object)
}

fn source_text<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a str, HighWorkerError> {
    object.get(key).and_then(Value::as_str).ok_or_else(|| {
        HighWorkerError(format!(
            "HIGH_WORKER_CANONICAL_TEXT_INVALID:{context}:{key}"
        ))
    })
}

fn source_id(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<String, HighWorkerError> {
    let value = source_text(object, key, context)?;
    require_stable_id(value, key)?;
    Ok(value.to_owned())
}

fn source_sha(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<String, HighWorkerError> {
    let value = source_text(object, key, context)?;
    if !is_sha256(value) {
        return Err(HighWorkerError(format!(
            "HIGH_WORKER_CANONICAL_SHA_INVALID:{context}:{key}"
        )));
    }
    Ok(value.to_owned())
}

fn source_array<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<&'a Vec<Value>, HighWorkerError> {
    object.get(key).and_then(Value::as_array).ok_or_else(|| {
        HighWorkerError(format!(
            "HIGH_WORKER_CANONICAL_ARRAY_INVALID:{context}:{key}"
        ))
    })
}

fn source_position(value: &Value, context: &str) -> Result<[f32; 3], HighWorkerError> {
    let values = value.as_array().ok_or_else(|| {
        HighWorkerError(format!("HIGH_WORKER_CANONICAL_POSITION_INVALID:{context}"))
    })?;
    if values.len() != 3 {
        return Err(HighWorkerError(format!(
            "HIGH_WORKER_CANONICAL_POSITION_INVALID:{context}"
        )));
    }
    let position = [
        values[0].as_f64().unwrap_or(f64::NAN) as f32,
        values[1].as_f64().unwrap_or(f64::NAN) as f32,
        values[2].as_f64().unwrap_or(f64::NAN) as f32,
    ];
    if position
        .iter()
        .any(|value| !value.is_finite() || value.abs() > MAX_COORDINATE_ABS_M)
    {
        return Err(HighWorkerError(format!(
            "HIGH_WORKER_CANONICAL_POSITION_INVALID:{context}"
        )));
    }
    Ok(position)
}

fn source_string_array(
    object: &serde_json::Map<String, Value>,
    key: &str,
    context: &str,
) -> Result<Vec<String>, HighWorkerError> {
    let values = source_array(object, key, context)?;
    let mut result = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let id = value.as_str().ok_or_else(|| {
            HighWorkerError(format!(
                "HIGH_WORKER_CANONICAL_ID_INVALID:{context}:{key}:{index}"
            ))
        })?;
        require_stable_id(id, key)?;
        result.push(id.to_owned());
    }
    Ok(result)
}

fn sorted_ids<T>(
    items: &[T],
    mut id: impl FnMut(&T) -> &str,
    label: &str,
) -> Result<(), HighWorkerError> {
    let mut previous = None::<&str>;
    for item in items {
        let current = id(item);
        require_stable_id(current, label)?;
        if previous.is_some_and(|previous| previous >= current) {
            return Err(HighWorkerError(format!(
                "HIGH_WORKER_CANONICAL_IDS_NOT_SORTED:{label}"
            )));
        }
        previous = Some(current);
    }
    Ok(())
}

fn validate_element_lineage(
    value: &Value,
    _element_id: &str,
    _source_lineage_sha256: &str,
    context: &str,
) -> Result<(), HighWorkerError> {
    let object = source_exact_object(
        value,
        &[
            "original_element_ids",
            "evaluated_element_ids",
            "correspondence_kind",
            "correspondence_sha256",
        ],
        context,
    )?;
    let original = source_string_array(object, "original_element_ids", context)?;
    let evaluated = source_string_array(object, "evaluated_element_ids", context)?;
    let mut all = original.clone();
    all.extend(evaluated.iter().cloned());
    let mut unique = BTreeSet::new();
    if all.iter().any(|id| !unique.insert(id)) {
        return Err(HighWorkerError(format!(
            "HIGH_WORKER_STABLE_LINEAGE_DUPLICATE:{context}"
        )));
    }
    let kind = source_text(object, "correspondence_kind", context)?;
    if !matches!(
        kind,
        "not_materialized" | "one_to_many" | "many_to_one" | "many_to_many" | "unknown"
    ) {
        return Err(HighWorkerError(format!(
            "HIGH_WORKER_STABLE_LINEAGE_KIND_INVALID:{context}"
        )));
    }
    let correspondence_sha = source_sha(object, "correspondence_sha256", context)?;
    let expected_correspondence_sha = sha256_value(&serde_json::json!({
        "original_element_ids": original,
        "evaluated_element_ids": evaluated,
        "correspondence_kind": kind,
    }))?;
    if correspondence_sha != expected_correspondence_sha {
        return Err(HighWorkerError(format!(
            "HIGH_WORKER_STABLE_LINEAGE_HASH_MISMATCH:{context}"
        )));
    }
    Ok(())
}

fn validate_canonical_source(
    request: &HighMeshWorkerRequest,
) -> Result<CanonicalSource, HighWorkerError> {
    let adapter = source_exact_object(
        &request.source_authoring_mesh,
        &[
            "schema_version",
            "canonical_mesh",
            "candidate_id",
            "candidate_state_sha256",
            "head_candidate_id",
            "head_candidate_state_sha256",
            "source_mesh_sha256",
        ],
        "HighWorkerAuthoringMeshAdapter@1",
    )?;
    if source_text(adapter, "schema_version", "adapter")? != "HighWorkerAuthoringMeshAdapter@1" {
        return Err(HighWorkerError(
            "HIGH_WORKER_SOURCE_ADAPTER_SCHEMA_MISMATCH".to_owned(),
        ));
    }
    if sha256_value(&request.source_authoring_mesh)? != request.source_authoring_mesh_sha256 {
        return Err(HighWorkerError(
            "HIGH_WORKER_SOURCE_ADAPTER_HASH_MISMATCH".to_owned(),
        ));
    }
    let canonical_value = adapter
        .get("canonical_mesh")
        .ok_or_else(|| HighWorkerError("HIGH_WORKER_CANONICAL_MESH_MISSING".to_owned()))?;
    let canonical = source_exact_object(
        canonical_value,
        CANONICAL_MESH_FIELDS,
        "AuthoringMeshCanonical@1",
    )?;
    if source_text(canonical, "schema_version", "canonical")? != AUTHORING_MESH_SCHEMA_VERSION
        || source_text(canonical, "representation", "canonical")?
            != "runtime-owned-original-half-edge@1"
        || source_text(canonical, "storage_policy", "canonical")?
            != "runtime-owned-sqlite-cas-canonical-authoring-mesh@1"
        || source_text(canonical, "writer_policy", "canonical")?
            != "forgecad-runtime-only-state-writer@1"
        || source_text(canonical, "canonicalization_policy", "canonical")?
            != "canonical-json-sha256-excluding-canonical-sha256@1"
        || source_text(canonical, "quality_status", "canonical")? != "structural_only"
    {
        return Err(HighWorkerError(
            "HIGH_WORKER_CANONICAL_POLICY_MISMATCH".to_owned(),
        ));
    }
    if canonical.get("runtime_write_performed") != Some(&Value::Bool(true))
        || canonical.get("persistent_user_data_touched") != Some(&Value::Bool(true))
        || canonical.get("stage_advanced") != Some(&Value::Bool(false))
        || canonical.get("candidate_confirmed") != Some(&Value::Bool(false))
        || canonical.get("version_created") != Some(&Value::Bool(false))
        || canonical.get("export_performed") != Some(&Value::Bool(false))
        || canonical.get("cross_version_stable") != Some(&Value::Bool(false))
    {
        return Err(HighWorkerError(
            "HIGH_WORKER_CANONICAL_STATE_POLICY_MISMATCH".to_owned(),
        ));
    }
    let canonical_sha256 = source_sha(canonical, "canonical_sha256", "canonical")?;
    let mut preimage = canonical_value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if sha256_value(&preimage)? != canonical_sha256 {
        return Err(HighWorkerError(
            "HIGH_WORKER_SOURCE_CANONICAL_HASH_MISMATCH".to_owned(),
        ));
    }
    let source_mesh_sha256 = source_sha(adapter, "source_mesh_sha256", "adapter")?;
    if source_mesh_sha256 != canonical_sha256 {
        return Err(HighWorkerError(
            "HIGH_WORKER_SOURCE_MESH_HASH_MISMATCH".to_owned(),
        ));
    }
    let candidate_id = source_id(canonical, "candidate_id", "canonical")?;
    let candidate_state_sha256 = source_sha(canonical, "candidate_state_sha256", "canonical")?;
    let adapter_candidate_id = source_id(adapter, "candidate_id", "adapter")?;
    let adapter_candidate_state_sha256 = source_sha(adapter, "candidate_state_sha256", "adapter")?;
    let head_candidate_id = source_id(adapter, "head_candidate_id", "adapter")?;
    let head_candidate_state_sha256 =
        source_sha(adapter, "head_candidate_state_sha256", "adapter")?;
    if adapter_candidate_id != candidate_id
        || adapter_candidate_state_sha256 != candidate_state_sha256
        || head_candidate_id != adapter_candidate_id
        || head_candidate_state_sha256 != adapter_candidate_state_sha256
    {
        return Err(HighWorkerError(
            "HIGH_WORKER_CANDIDATE_HEAD_BINDING_MISMATCH".to_owned(),
        ));
    }
    let project_id = source_id(canonical, "project_id", "canonical")?;
    let canonical_mesh_id = source_id(canonical, "canonical_mesh_id", "canonical")?;
    let part_id = source_id(canonical, "part_id", "canonical")?;
    let authoring_node_id = source_id(canonical, "authoring_node_id", "canonical")?;
    let source_lineage_sha256 = source_sha(canonical, "source_lineage_sha256", "canonical")?;
    let original_identity = source_exact_object(
        canonical.get("original_identity").ok_or_else(|| {
            HighWorkerError("HIGH_WORKER_CANONICAL_ORIGINAL_IDENTITY_MISSING".to_owned())
        })?,
        &[
            "identity_id",
            "namespace",
            "identity_kind",
            "element_id_policy",
            "topology_sha256",
            "source_lineage_sha256",
            "stability_scope",
        ],
        "original_identity",
    )?;
    if source_text(original_identity, "namespace", "original_identity")? != "original"
        || source_text(original_identity, "identity_kind", "original_identity")?
            != "runtime-owned-original-authoring@1"
        || source_text(original_identity, "element_id_policy", "original_identity")?
            != "lineage-scoped-opaque-not-cross-version-stable@1"
        || source_text(original_identity, "stability_scope", "original_identity")?
            != "same-canonical-mesh-lineage-only@1"
        || source_sha(
            original_identity,
            "source_lineage_sha256",
            "original_identity",
        )? != source_lineage_sha256
    {
        return Err(HighWorkerError(
            "HIGH_WORKER_STABLE_ID_POLICY_MISMATCH".to_owned(),
        ));
    }
    let evaluated_identity = source_object(
        canonical.get("evaluated_identity").ok_or_else(|| {
            HighWorkerError("HIGH_WORKER_CANONICAL_EVALUATED_IDENTITY_MISSING".to_owned())
        })?,
        "evaluated_identity",
    )?;
    if source_text(evaluated_identity, "namespace", "evaluated_identity")? != "evaluated"
        || source_text(
            evaluated_identity,
            "correspondence_policy",
            "evaluated_identity",
        )? != "non-bijective-derived-only@1"
        || source_text(
            evaluated_identity,
            "source_lineage_sha256",
            "evaluated_identity",
        )? != source_lineage_sha256
    {
        return Err(HighWorkerError(
            "HIGH_WORKER_EVALUATED_LINEAGE_POLICY_MISMATCH".to_owned(),
        ));
    }
    let stability = source_exact_object(
        canonical
            .get("cross_version_stability")
            .ok_or_else(|| HighWorkerError("HIGH_WORKER_CANONICAL_STABILITY_MISSING".to_owned()))?,
        &[
            "status",
            "scope",
            "stable_id_claim",
            "deleted_id_reuse_policy",
            "new_id_policy",
            "evaluated_id_policy",
        ],
        "cross_version_stability",
    )?;
    if source_text(stability, "status", "cross_version_stability")? != "not-proven@1"
        || source_text(stability, "scope", "cross_version_stability")?
            != "same-canonical-mesh-lineage-only@1"
        || source_text(stability, "stable_id_claim", "cross_version_stability")?
            != "none-across-revisions@1"
    {
        return Err(HighWorkerError(
            "HIGH_WORKER_CROSS_VERSION_STABILITY_UNPROVEN".to_owned(),
        ));
    }

    let vertex_values = source_array(canonical, "vertices", "canonical")?;
    if vertex_values.is_empty() || vertex_values.len() > MAX_VERTICES_PER_PART {
        return Err(HighWorkerError(
            "HIGH_WORKER_CANONICAL_VERTEX_BUDGET_INVALID".to_owned(),
        ));
    }
    let mut vertices = Vec::<(String, [f32; 3], String)>::with_capacity(vertex_values.len());
    for value in vertex_values {
        let object = source_exact_object(
            value,
            &[
                "vertex_id",
                "position_m",
                "outgoing_half_edge_id",
                "boundary",
                "lineage",
            ],
            "canonical vertex",
        )?;
        let id = source_id(object, "vertex_id", "canonical vertex")?;
        let position = source_position(
            object.get("position_m").ok_or_else(|| {
                HighWorkerError("HIGH_WORKER_CANONICAL_VERTEX_POSITION_MISSING".to_owned())
            })?,
            "canonical vertex",
        )?;
        let outgoing = source_id(object, "outgoing_half_edge_id", "canonical vertex")?;
        validate_element_lineage(
            object.get("lineage").expect("exact vertex lineage"),
            &id,
            &source_lineage_sha256,
            "canonical vertex lineage",
        )?;
        vertices.push((id, position, outgoing));
    }
    sorted_ids(&vertices, |item| item.0.as_str(), "vertex")?;
    let vertex_index = vertices
        .iter()
        .enumerate()
        .map(|(index, item)| (item.0.clone(), index as u32))
        .collect::<BTreeMap<_, _>>();

    let edge_values = source_array(canonical, "edges", "canonical")?;
    if edge_values.is_empty() || edge_values.len() > MAX_TRIANGLES_PER_PART {
        return Err(HighWorkerError(
            "HIGH_WORKER_CANONICAL_EDGE_BUDGET_INVALID".to_owned(),
        ));
    }
    let mut edge_rows = Vec::<(String, [String; 2], Vec<String>)>::with_capacity(edge_values.len());
    for value in edge_values {
        let object = source_exact_object(
            value,
            &[
                "edge_id",
                "vertex_ids",
                "half_edge_ids",
                "boundary",
                "hard_edge",
                "crease",
                "uv_seam",
                "lineage",
            ],
            "canonical edge",
        )?;
        let id = source_id(object, "edge_id", "canonical edge")?;
        let endpoint_ids = source_string_array(object, "vertex_ids", "canonical edge")?;
        if endpoint_ids.len() != 2
            || endpoint_ids[0] == endpoint_ids[1]
            || endpoint_ids.iter().any(|id| !vertex_index.contains_key(id))
        {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_EDGE_ENDPOINT_INVALID".to_owned(),
            ));
        }
        let half_edge_ids = source_string_array(object, "half_edge_ids", "canonical edge")?;
        if half_edge_ids.is_empty() || half_edge_ids.len() > 2 {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_EDGE_HALF_EDGE_INVALID".to_owned(),
            ));
        }
        validate_element_lineage(
            object.get("lineage").expect("exact edge lineage"),
            &id,
            &source_lineage_sha256,
            "canonical edge lineage",
        )?;
        edge_rows.push((
            id,
            [endpoint_ids[0].clone(), endpoint_ids[1].clone()],
            half_edge_ids,
        ));
    }
    sorted_ids(&edge_rows, |item| item.0.as_str(), "edge")?;

    let half_edge_values = source_array(canonical, "half_edges", "canonical")?;
    let mut half_edges = BTreeMap::<
        String,
        (
            String,
            String,
            String,
            String,
            Option<String>,
            String,
            String,
        ),
    >::new();
    for value in half_edge_values {
        let object = source_exact_object(
            value,
            &[
                "half_edge_id",
                "origin_vertex_id",
                "edge_id",
                "face_id",
                "corner_id",
                "twin_id",
                "next_id",
                "prev_id",
                "boundary",
                "orientation",
                "lineage",
            ],
            "canonical half-edge",
        )?;
        let id = source_id(object, "half_edge_id", "canonical half-edge")?;
        let origin = source_id(object, "origin_vertex_id", "canonical half-edge")?;
        let edge_id = source_id(object, "edge_id", "canonical half-edge")?;
        let face_id = source_id(object, "face_id", "canonical half-edge")?;
        let corner_id = source_id(object, "corner_id", "canonical half-edge")?;
        let twin_id = match object.get("twin_id") {
            Some(Value::Null) => None,
            Some(Value::String(value)) => {
                require_stable_id(value, "twin_id")?;
                Some(value.clone())
            }
            _ => {
                return Err(HighWorkerError(
                    "HIGH_WORKER_CANONICAL_HALF_EDGE_TWIN_INVALID".to_owned(),
                ))
            }
        };
        let next_id = source_id(object, "next_id", "canonical half-edge")?;
        let prev_id = source_id(object, "prev_id", "canonical half-edge")?;
        validate_element_lineage(
            object.get("lineage").expect("exact half-edge lineage"),
            &id,
            &source_lineage_sha256,
            "canonical half-edge lineage",
        )?;
        if half_edges
            .insert(
                id,
                (
                    origin, edge_id, face_id, corner_id, twin_id, next_id, prev_id,
                ),
            )
            .is_some()
        {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_DUPLICATE_HALF_EDGE".to_owned(),
            ));
        }
    }

    let corner_values = source_array(canonical, "corners", "canonical")?;
    let mut corners = BTreeMap::<String, (String, String, String)>::new();
    for value in corner_values {
        let object = source_exact_object(
            value,
            &[
                "corner_id",
                "face_id",
                "half_edge_id",
                "vertex_id",
                "ordinal",
                "lineage",
            ],
            "canonical corner",
        )?;
        let id = source_id(object, "corner_id", "canonical corner")?;
        let face_id = source_id(object, "face_id", "canonical corner")?;
        let half_edge_id = source_id(object, "half_edge_id", "canonical corner")?;
        let vertex_id = source_id(object, "vertex_id", "canonical corner")?;
        validate_element_lineage(
            object.get("lineage").expect("exact corner lineage"),
            &id,
            &source_lineage_sha256,
            "canonical corner lineage",
        )?;
        if corners
            .insert(id, (face_id, half_edge_id, vertex_id))
            .is_some()
        {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_DUPLICATE_CORNER".to_owned(),
            ));
        }
    }

    let face_values = source_array(canonical, "faces", "canonical")?;
    let mut faces = Vec::<(String, Vec<String>, String)>::with_capacity(face_values.len());
    for value in face_values {
        let object = source_exact_object(
            value,
            &[
                "face_id",
                "first_half_edge_id",
                "corner_ids",
                "degree",
                "boundary",
                "lineage",
            ],
            "canonical face",
        )?;
        let id = source_id(object, "face_id", "canonical face")?;
        let first_half_edge_id = source_id(object, "first_half_edge_id", "canonical face")?;
        let corner_ids = source_string_array(object, "corner_ids", "canonical face")?;
        if corner_ids.len() < 3 || corner_ids.len() > 32 {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_FACE_DEGREE_INVALID".to_owned(),
            ));
        }
        let degree = object
            .get("degree")
            .and_then(Value::as_u64)
            .filter(|value| *value == corner_ids.len() as u64)
            .ok_or_else(|| {
                HighWorkerError("HIGH_WORKER_CANONICAL_FACE_DEGREE_MISMATCH".to_owned())
            })?;
        let _ = degree;
        validate_element_lineage(
            object.get("lineage").expect("exact face lineage"),
            &id,
            &source_lineage_sha256,
            "canonical face lineage",
        )?;
        faces.push((id, corner_ids, first_half_edge_id));
    }
    sorted_ids(&faces, |item| item.0.as_str(), "face")?;

    let loop_values = source_array(canonical, "loops", "canonical")?;
    let mut loops = BTreeMap::<String, (String, Vec<String>, String)>::new();
    for value in loop_values {
        let object = source_exact_object(
            value,
            &[
                "loop_id",
                "face_id",
                "first_half_edge_id",
                "half_edge_ids",
                "boundary",
                "lineage",
            ],
            "canonical loop",
        )?;
        let id = source_id(object, "loop_id", "canonical loop")?;
        let face_id = source_id(object, "face_id", "canonical loop")?;
        let first_half_edge_id = source_id(object, "first_half_edge_id", "canonical loop")?;
        let half_edge_ids = source_string_array(object, "half_edge_ids", "canonical loop")?;
        if half_edge_ids.len() < 3 || half_edge_ids.len() > 32 {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_LOOP_DEGREE_INVALID".to_owned(),
            ));
        }
        validate_element_lineage(
            object.get("lineage").expect("exact loop lineage"),
            &id,
            &source_lineage_sha256,
            "canonical loop lineage",
        )?;
        if loops
            .insert(id, (face_id, half_edge_ids, first_half_edge_id))
            .is_some()
        {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_DUPLICATE_LOOP".to_owned(),
            ));
        }
    }

    let face_ids = faces.iter().map(|item| item.0.clone()).collect::<Vec<_>>();
    let loop_ids = loops.keys().cloned().collect::<Vec<_>>();
    let vertex_ids = vertices
        .iter()
        .map(|item| item.0.clone())
        .collect::<Vec<_>>();
    sorted_ids(&loop_ids, |item| item.as_str(), "loop")?;
    let mut face_vertices = BTreeMap::<String, Vec<String>>::new();
    let mut edge_by_pair = BTreeMap::<(String, String), String>::new();
    let mut stable_edges = BTreeMap::<String, StableEdge>::new();
    for (id, endpoint_ids, half_edge_ids) in &edge_rows {
        let pair = if endpoint_ids[0] < endpoint_ids[1] {
            (endpoint_ids[0].clone(), endpoint_ids[1].clone())
        } else {
            (endpoint_ids[1].clone(), endpoint_ids[0].clone())
        };
        if edge_by_pair.insert(pair, id.clone()).is_some() {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_DUPLICATE_EDGE_ENDPOINTS".to_owned(),
            ));
        }
        let a = *vertex_index
            .get(&endpoint_ids[0])
            .expect("validated edge vertex");
        let b = *vertex_index
            .get(&endpoint_ids[1])
            .expect("validated edge vertex");
        let _ = half_edge_ids;
        stable_edges.insert(
            id.clone(),
            StableEdge {
                edge_id: id.clone(),
                vertex_ids: endpoint_ids.clone(),
                vertex_indices: ordered_edge(a, b),
                loop_ids: Vec::new(),
                face_ids: Vec::new(),
                face_inward: Vec::new(),
            },
        );
    }

    let mut triangles = Vec::<[u32; 3]>::new();
    let mut face_normals = BTreeMap::<String, [f32; 3]>::new();
    let mut face_edge_ids = BTreeMap::<String, Vec<String>>::new();
    for (face_id, corner_ids, first_half_edge_id) in &faces {
        let first = half_edges.get(first_half_edge_id).ok_or_else(|| {
            HighWorkerError("HIGH_WORKER_CANONICAL_FACE_FIRST_HALF_EDGE_UNKNOWN".to_owned())
        })?;
        if first.2 != *face_id {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_FACE_HALF_EDGE_FACE_MISMATCH".to_owned(),
            ));
        }
        let mut face_vertex_ids = Vec::with_capacity(corner_ids.len());
        for corner_id in corner_ids {
            let corner = corners.get(corner_id).ok_or_else(|| {
                HighWorkerError("HIGH_WORKER_CANONICAL_FACE_CORNER_UNKNOWN".to_owned())
            })?;
            if corner.0 != *face_id {
                return Err(HighWorkerError(
                    "HIGH_WORKER_CANONICAL_FACE_CORNER_FACE_MISMATCH".to_owned(),
                ));
            }
            if !vertex_index.contains_key(&corner.2) || !half_edges.contains_key(&corner.1) {
                return Err(HighWorkerError(
                    "HIGH_WORKER_CANONICAL_FACE_CORNER_REFERENCE_INVALID".to_owned(),
                ));
            }
            face_vertex_ids.push(corner.2.clone());
        }
        let mut edge_ids = Vec::with_capacity(face_vertex_ids.len());
        for index in 0..face_vertex_ids.len() {
            let a = &face_vertex_ids[index];
            let b = &face_vertex_ids[(index + 1) % face_vertex_ids.len()];
            let key = if a < b {
                (a.clone(), b.clone())
            } else {
                (b.clone(), a.clone())
            };
            edge_ids.push(edge_by_pair.get(&key).cloned().ok_or_else(|| {
                HighWorkerError("HIGH_WORKER_CANONICAL_FACE_EDGE_REFERENCE_INVALID".to_owned())
            })?);
        }
        let positions = face_vertex_ids
            .iter()
            .map(|id| vertices[vertex_index[id] as usize].1)
            .collect::<Vec<_>>();
        let normal = face_normal(positions[0], positions[1], positions[2])?;
        for index in 1..positions.len() - 1 {
            let triangle = [
                vertex_index[&face_vertex_ids[0]],
                vertex_index[&face_vertex_ids[index]],
                vertex_index[&face_vertex_ids[index + 1]],
            ];
            if triangle[0] == triangle[1]
                || triangle[1] == triangle[2]
                || triangle[0] == triangle[2]
                || triangle_area(
                    vertices[triangle[0] as usize].1,
                    vertices[triangle[1] as usize].1,
                    vertices[triangle[2] as usize].1,
                ) <= EPSILON
            {
                return Err(HighWorkerError(
                    "HIGH_WORKER_CANONICAL_FACE_TRIANGLE_INVALID".to_owned(),
                ));
            }
            triangles.push(triangle);
        }
        face_normals.insert(face_id.clone(), normal);
        face_edge_ids.insert(face_id.clone(), edge_ids);
        face_vertices.insert(face_id.clone(), face_vertex_ids);
    }

    for (loop_id, (face_id, half_edge_ids, first_half_edge_id)) in &loops {
        if !faces.iter().any(|face| face.0 == *face_id)
            || !half_edge_ids.iter().any(|id| id == first_half_edge_id)
        {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_LOOP_REFERENCE_INVALID".to_owned(),
            ));
        }
        for half_edge_id in half_edge_ids {
            let half_edge = half_edges.get(half_edge_id).ok_or_else(|| {
                HighWorkerError("HIGH_WORKER_CANONICAL_LOOP_HALF_EDGE_UNKNOWN".to_owned())
            })?;
            let edge = stable_edges.get_mut(&half_edge.1).ok_or_else(|| {
                HighWorkerError("HIGH_WORKER_CANONICAL_LOOP_EDGE_UNKNOWN".to_owned())
            })?;
            edge.loop_ids.push(loop_id.clone());
        }
    }
    for edge in stable_edges.values_mut() {
        edge.loop_ids.sort();
        edge.loop_ids.dedup();
    }

    for (edge_id, edge) in stable_edges.iter_mut() {
        let expected_half_edges = edge_rows
            .iter()
            .find(|row| row.0 == *edge_id)
            .map(|row| &row.2)
            .expect("stable edge row");
        let mut face_ids_for_edge = BTreeSet::new();
        for half_edge_id in expected_half_edges {
            let half_edge = half_edges.get(half_edge_id).ok_or_else(|| {
                HighWorkerError("HIGH_WORKER_CANONICAL_EDGE_HALF_EDGE_UNKNOWN".to_owned())
            })?;
            if half_edge.1 != *edge_id {
                return Err(HighWorkerError(
                    "HIGH_WORKER_CANONICAL_EDGE_HALF_EDGE_MISMATCH".to_owned(),
                ));
            }
            face_ids_for_edge.insert(half_edge.2.clone());
        }
        if face_ids_for_edge.is_empty() || face_ids_for_edge.len() > 2 {
            return Err(HighWorkerError(
                "HIGH_WORKER_CANONICAL_EDGE_FACE_INCIDENT_INVALID".to_owned(),
            ));
        }
        edge.face_ids = face_ids_for_edge.into_iter().collect();
        edge.face_ids.sort();
        let midpoint = scale(
            add(
                vertices[edge.vertex_indices.0 as usize].1,
                vertices[edge.vertex_indices.1 as usize].1,
            ),
            0.5,
        );
        let tangent = normalize(sub(
            vertices[edge.vertex_indices.1 as usize].1,
            vertices[edge.vertex_indices.0 as usize].1,
        ))?;
        edge.face_inward = edge
            .face_ids
            .iter()
            .map(|face_id| {
                let face_vertex_ids = face_vertices.get(face_id).ok_or_else(|| {
                    HighWorkerError("HIGH_WORKER_CANONICAL_EDGE_FACE_VERTICES_MISSING".to_owned())
                })?;
                let opposite_id = face_vertex_ids
                    .iter()
                    .find(|vertex_id| {
                        *vertex_id != &edge.vertex_ids[0] && *vertex_id != &edge.vertex_ids[1]
                    })
                    .ok_or_else(|| {
                        HighWorkerError(
                            "HIGH_WORKER_CANONICAL_EDGE_FACE_INTERIOR_VERTEX_MISSING".to_owned(),
                        )
                    })?;
                let delta = sub(vertices[vertex_index[opposite_id] as usize].1, midpoint);
                let in_plane = sub(delta, scale(tangent, dot(delta, tangent)));
                normalize(in_plane)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let normal = face_normals
            .get(&edge.face_ids[0])
            .copied()
            .ok_or_else(|| {
                HighWorkerError("HIGH_WORKER_CANONICAL_EDGE_FACE_NORMAL_MISSING".to_owned())
            })?;
        let vertex_indices = edge.vertex_indices;
        let mut info_normal = normal;
        if edge.face_ids.len() == 2 {
            info_normal = normalize(add(normal, face_normals[&edge.face_ids[1]]))?;
        }
        let _ = (vertex_indices, info_normal);
    }

    let mut index_part = AuthoringPart {
        part_id: part_id.clone(),
        source_node_id: authoring_node_id.clone(),
        material_zone_id: format!("material-zone:{part_id}"),
        vertices: vertices.iter().map(|item| item.1).collect(),
        indices: triangles,
    };
    if index_part.indices.is_empty() {
        return Err(HighWorkerError(
            "HIGH_WORKER_CANONICAL_NO_TRIANGLES".to_owned(),
        ));
    }
    let mut numeric_edges = BTreeMap::<(String, u32, u32), EdgeInfo>::new();
    for edge in stable_edges.values() {
        let info_normal = face_normals[&edge.face_ids[0]];
        let key = (
            part_id.clone(),
            edge.vertex_indices.0,
            edge.vertex_indices.1,
        );
        numeric_edges.insert(
            key,
            EdgeInfo {
                face_count: edge.face_ids.len(),
                face_normal: info_normal,
            },
        );
    }
    let _ = &mut index_part;
    Ok(CanonicalSource {
        parts: vec![index_part],
        stable_edges,
        stable_vertex_ids: vertex_ids,
        stable_face_ids: face_ids,
        stable_loop_ids: loop_ids,
        canonical_mesh_id,
        project_id,
        candidate_id,
        candidate_state_sha256,
        head_candidate_id,
        head_candidate_state_sha256,
        source_lineage_sha256,
        part_id,
        authoring_node_id,
    })
}

fn validate_request(request: &HighMeshWorkerRequest) -> Result<ValidatedInput, HighWorkerError> {
    if request.schema_version != REQUEST_SCHEMA_VERSION {
        return Err(HighWorkerError(
            "HIGH_WORKER_REQUEST_SCHEMA_MISMATCH".to_owned(),
        ));
    }
    if request.operation != OPERATION {
        return Err(HighWorkerError(
            "HIGH_WORKER_OPERATION_NOT_ALLOWED".to_owned(),
        ));
    }
    if request.detail_graph.schema_version != DETAIL_GRAPH_SCHEMA_VERSION {
        return Err(HighWorkerError(
            "HIGH_WORKER_DETAIL_GRAPH_SCHEMA_MISMATCH".to_owned(),
        ));
    }
    if !is_sha256(&request.source_authoring_mesh_sha256)
        || !is_sha256(&request.detail_graph_canonical_sha256)
        || !is_sha256(&request.canonical_sha256)
    {
        return Err(HighWorkerError("HIGH_WORKER_HASH_INVALID".to_owned()));
    }
    let mut request_preimage = serde_json::to_value(request)?;
    request_preimage["canonical_sha256"] = Value::String(String::new());
    if sha256_value(&request_preimage)? != request.canonical_sha256 {
        return Err(HighWorkerError(
            "HIGH_WORKER_REQUEST_CANONICAL_MISMATCH".to_owned(),
        ));
    }
    if sha256_value(&request.source_authoring_mesh)? != request.source_authoring_mesh_sha256 {
        return Err(HighWorkerError(
            "HIGH_WORKER_SOURCE_HASH_MISMATCH".to_owned(),
        ));
    }
    if sha256_value(&request.detail_graph)? != request.detail_graph_canonical_sha256 {
        return Err(HighWorkerError(
            "HIGH_WORKER_DETAIL_GRAPH_HASH_MISMATCH".to_owned(),
        ));
    }
    let canonical = validate_canonical_source(request)?;
    let budgets = &request.budgets;
    if budgets.max_detail_nodes == 0
        || budgets.max_detail_nodes as usize > MAX_DETAIL_NODES
        || budgets.max_output_vertices == 0
        || budgets.max_output_vertices as usize > MAX_OUTPUT_VERTICES
        || budgets.max_output_triangles == 0
        || budgets.max_output_triangles as usize > MAX_OUTPUT_TRIANGLES
    {
        return Err(HighWorkerError("HIGH_WORKER_BUDGET_INVALID".to_owned()));
    }
    if canonical.parts.is_empty() || canonical.parts.len() > MAX_PARTS {
        return Err(HighWorkerError("HIGH_WORKER_PART_COUNT_INVALID".to_owned()));
    }
    let mut parts = BTreeMap::new();
    let mut previous_part = None::<&str>;
    let mut edges = BTreeMap::new();
    for part in &canonical.parts {
        require_id(&part.part_id, "part_id")?;
        require_id(&part.source_node_id, "source_node_id")?;
        require_id(&part.material_zone_id, "material_zone_id")?;
        if previous_part.is_some_and(|previous| previous >= part.part_id.as_str()) {
            return Err(HighWorkerError(
                "HIGH_WORKER_PARTS_MUST_BE_LEXICOGRAPHICALLY_SORTED".to_owned(),
            ));
        }
        previous_part = Some(&part.part_id);
        if part.vertices.is_empty() || part.vertices.len() > MAX_VERTICES_PER_PART {
            return Err(HighWorkerError(format!(
                "HIGH_WORKER_VERTEX_BUDGET_INVALID:{}",
                part.part_id
            )));
        }
        if part.indices.is_empty() || part.indices.len() > MAX_TRIANGLES_PER_PART {
            return Err(HighWorkerError(format!(
                "HIGH_WORKER_TRIANGLE_BUDGET_INVALID:{}",
                part.part_id
            )));
        }
        for vertex in &part.vertices {
            for coordinate in vertex {
                if !coordinate.is_finite() || coordinate.abs() > MAX_COORDINATE_ABS_M {
                    return Err(HighWorkerError(format!(
                        "HIGH_WORKER_VERTEX_INVALID:{}",
                        part.part_id
                    )));
                }
            }
        }
        for triangle in &part.indices {
            let [a, b, c] = *triangle;
            if a == b
                || b == c
                || a == c
                || [a, b, c]
                    .iter()
                    .any(|index| *index as usize >= part.vertices.len())
                || triangle_area(
                    part.vertices[a as usize],
                    part.vertices[b as usize],
                    part.vertices[c as usize],
                ) <= EPSILON
            {
                return Err(HighWorkerError(format!(
                    "HIGH_WORKER_TRIANGLE_INVALID:{}",
                    part.part_id
                )));
            }
            let normal = face_normal(
                part.vertices[a as usize],
                part.vertices[b as usize],
                part.vertices[c as usize],
            )?;
            for (edge_a, edge_b) in [(a, b), (b, c), (c, a)] {
                let (low, high) = ordered_edge(edge_a, edge_b);
                let key = (part.part_id.clone(), low, high);
                let entry = edges.entry(key).or_insert(EdgeInfo {
                    face_count: 0,
                    face_normal: [0.0; 3],
                });
                entry.face_count += 1;
                entry.face_normal = add(entry.face_normal, normal);
                if entry.face_count > 2 {
                    return Err(HighWorkerError(format!(
                        "HIGH_WORKER_NON_MANIFOLD_EDGE:{}:{}-{}",
                        part.part_id, low, high
                    )));
                }
            }
        }
        if parts.insert(part.part_id.clone(), part.clone()).is_some() {
            return Err(HighWorkerError("HIGH_WORKER_DUPLICATE_PART".to_owned()));
        }
    }
    if request.detail_graph.nodes.is_empty()
        || request.detail_graph.nodes.len() > budgets.max_detail_nodes as usize
    {
        return Err(HighWorkerError(
            "HIGH_WORKER_DETAIL_NODE_BUDGET_EXCEEDED".to_owned(),
        ));
    }
    let mut node_ids = BTreeSet::new();
    let mut node_parts = BTreeMap::<String, String>::new();
    for (index, node) in request.detail_graph.nodes.iter().enumerate() {
        require_id(&node.node_id, "detail node_id")?;
        if index > 0 && request.detail_graph.nodes[index - 1].node_id >= node.node_id {
            return Err(HighWorkerError(
                "HIGH_WORKER_DETAIL_NODES_MUST_BE_LEXICOGRAPHICALLY_SORTED".to_owned(),
            ));
        }
        if !node_ids.insert(node.node_id.clone()) {
            return Err(HighWorkerError(
                "HIGH_WORKER_DUPLICATE_DETAIL_NODE".to_owned(),
            ));
        }
        if !parts.contains_key(&node.parent_part_id) {
            return Err(HighWorkerError(format!(
                "HIGH_WORKER_DETAIL_PART_NOT_FOUND:{}",
                node.parent_part_id
            )));
        }
        if let Some(parent) = &node.parent_node_id {
            if !node_ids.contains(parent) {
                return Err(HighWorkerError(format!(
                    "HIGH_WORKER_DETAIL_PARENT_NOT_PRECEDING:{}",
                    node.node_id
                )));
            }
            if node_parts.get(parent) != Some(&node.parent_part_id) {
                return Err(HighWorkerError(format!(
                    "HIGH_WORKER_DETAIL_PARENT_PART_MISMATCH:{}",
                    node.node_id
                )));
            }
        }
        validate_detail_node(node, &parts, &edges, &canonical.stable_edges)?;
        node_parts.insert(node.node_id.clone(), node.parent_part_id.clone());
    }
    Ok(ValidatedInput {
        parts,
        edges,
        stable_edges: canonical.stable_edges,
        stable_vertex_ids: canonical.stable_vertex_ids,
        stable_face_ids: canonical.stable_face_ids,
        stable_loop_ids: canonical.stable_loop_ids,
        canonical_mesh_id: canonical.canonical_mesh_id,
        project_id: canonical.project_id,
        candidate_id: canonical.candidate_id,
        candidate_state_sha256: canonical.candidate_state_sha256,
        head_candidate_id: canonical.head_candidate_id,
        head_candidate_state_sha256: canonical.head_candidate_state_sha256,
        source_lineage_sha256: canonical.source_lineage_sha256,
        part_id: canonical.part_id,
        authoring_node_id: canonical.authoring_node_id,
    })
}

fn validate_detail_node(
    node: &DetailNode,
    parts: &BTreeMap<String, AuthoringPart>,
    edges: &BTreeMap<(String, u32, u32), EdgeInfo>,
    stable_edges: &BTreeMap<String, StableEdge>,
) -> Result<(), HighWorkerError> {
    match node.kind.as_str() {
        "support_loop" => {
            let edge = normalized_source_edge(node, stable_edges)?;
            require_edge(edges, &node.parent_part_id, edge)?;
            let width = node
                .width_m
                .ok_or_else(|| HighWorkerError("HIGH_WORKER_SUPPORT_WIDTH_MISSING".to_owned()))?;
            let count = node
                .count
                .ok_or_else(|| HighWorkerError("HIGH_WORKER_SUPPORT_COUNT_MISSING".to_owned()))?;
            if !width.is_finite() || !(width > 0.0 && width <= 0.25) || !(1..=3).contains(&count) {
                return Err(HighWorkerError(
                    "HIGH_WORKER_SUPPORT_PARAMETERS_INVALID".to_owned(),
                ));
            }
            if node.sharpness.is_some() || node.center_m.is_some() || node.size_m.is_some() {
                return Err(HighWorkerError(
                    "HIGH_WORKER_SUPPORT_FIELDS_INVALID".to_owned(),
                ));
            }
        }
        "crease" => {
            let edge = normalized_source_edge(node, stable_edges)?;
            require_edge(edges, &node.parent_part_id, edge)?;
            let sharpness = node.sharpness.ok_or_else(|| {
                HighWorkerError("HIGH_WORKER_CREASE_SHARPNESS_MISSING".to_owned())
            })?;
            if !sharpness.is_finite() || !(0.0..=10.0).contains(&sharpness) {
                return Err(HighWorkerError(
                    "HIGH_WORKER_CREASE_SHARPNESS_INVALID".to_owned(),
                ));
            }
            if node.width_m.is_some()
                || node.count.is_some()
                || node.center_m.is_some()
                || node.size_m.is_some()
            {
                return Err(HighWorkerError(
                    "HIGH_WORKER_CREASE_FIELDS_INVALID".to_owned(),
                ));
            }
        }
        "floating_detail" => {
            let center = node
                .center_m
                .ok_or_else(|| HighWorkerError("HIGH_WORKER_FLOATER_CENTER_MISSING".to_owned()))?;
            let size = node
                .size_m
                .ok_or_else(|| HighWorkerError("HIGH_WORKER_FLOATER_SIZE_MISSING".to_owned()))?;
            validate_vec(
                center,
                MAX_COORDINATE_ABS_M,
                "HIGH_WORKER_FLOATER_CENTER_INVALID",
            )?;
            validate_positive_vec(size, MAX_DETAIL_SIZE_M, "HIGH_WORKER_FLOATER_SIZE_INVALID")?;
            if node.source_edge.is_some()
                || node.width_m.is_some()
                || node.count.is_some()
                || node.sharpness.is_some()
            {
                return Err(HighWorkerError(
                    "HIGH_WORKER_FLOATER_FIELDS_INVALID".to_owned(),
                ));
            }
            let bounds = part_bounds(parts.get(&node.parent_part_id).expect("validated part"))?;
            if aabb_intersects(
                bounds,
                Bounds {
                    min: sub(center, scale(size, 0.5)),
                    max: add(center, scale(size, 0.5)),
                },
            ) {
                return Err(HighWorkerError(format!(
                    "HIGH_WORKER_FLOATING_DETAIL_INTERSECTS_BASE:{}",
                    node.node_id
                )));
            }
        }
        _ => {
            return Err(HighWorkerError(format!(
                "HIGH_WORKER_DETAIL_KIND_NOT_ALLOWED:{}",
                node.kind
            )))
        }
    }
    let _ = parts;
    Ok(())
}

fn stable_base_lineage(input: &ValidatedInput) -> Vec<String> {
    let mut lineage = Vec::with_capacity(
        input.stable_vertex_ids.len()
            + input.stable_edges.len()
            + input.stable_loop_ids.len()
            + input.stable_face_ids.len()
            + 3,
    );
    lineage.extend(
        input
            .stable_vertex_ids
            .iter()
            .map(|id| format!("vertex:{id}")),
    );
    lineage.extend(input.stable_edges.keys().map(|id| format!("edge:{id}")));
    lineage.extend(input.stable_loop_ids.iter().map(|id| format!("loop:{id}")));
    lineage.extend(input.stable_face_ids.iter().map(|id| format!("face:{id}")));
    lineage.push(format!("part:{}", input.part_id));
    lineage.push(format!("node:{}", input.authoring_node_id));
    lineage.push(format!("lineage:{}", input.source_lineage_sha256));
    lineage
}

fn evaluate(
    request: &HighMeshWorkerRequest,
    request_sha256: &str,
    input_sha256: &str,
) -> Result<HighMeshArtifact, HighWorkerError> {
    let validated = validate_request(request)?;
    let mut base_parts = Vec::with_capacity(validated.parts.len());
    let mut detail_primitives = Vec::new();
    let mut detail_lineage = Vec::new();
    let mut base_triangle_count = 0_u64;
    let mut detail_triangle_count = 0_u64;
    let mut output_vertices = 0_usize;
    let mut output_triangles = 0_usize;

    for part in validated.parts.values() {
        let primitive_id = format!("base:{}", part.part_id);
        base_triangle_count += part.indices.len() as u64;
        output_vertices += part.vertices.len();
        output_triangles += part.indices.len();
        base_parts.push(HighMeshPrimitive {
            primitive_id,
            kind: "authoring_base".to_owned(),
            part_id: part.part_id.clone(),
            source_node_ids: Vec::new(),
            source_node_id: part.source_node_id.clone(),
            material_zone_id: part.material_zone_id.clone(),
            source_element_lineage: stable_base_lineage(&validated),
            geometry: HighMeshGeometry {
                positions_m: part.vertices.clone(),
                indices: part.indices.clone(),
            },
        });
    }
    if output_vertices > request.budgets.max_output_vertices as usize
        || output_triangles > request.budgets.max_output_triangles as usize
    {
        return Err(HighWorkerError(
            "HIGH_WORKER_BASE_OUTPUT_BUDGET_EXCEEDED".to_owned(),
        ));
    }

    for node in &request.detail_graph.nodes {
        let part = validated.parts.get(&node.parent_part_id).ok_or_else(|| {
            HighWorkerError("HIGH_WORKER_DETAIL_PART_NOT_FOUND_DURING_EVALUATION".to_owned())
        })?;
        let (primitive, lineage) = match node.kind.as_str() {
            "support_loop" => {
                build_support_patch(node, part, &validated.edges, &validated.stable_edges)?
            }
            "crease" => {
                build_crease_metadata(node, part, &validated.edges, &validated.stable_edges)?
            }
            "floating_detail" => build_floating_detail(node, part)?,
            _ => unreachable!("validated detail kind"),
        };
        output_vertices += primitive.geometry.positions_m.len();
        output_triangles += primitive.geometry.indices.len();
        detail_triangle_count += primitive.geometry.indices.len() as u64;
        if output_vertices > request.budgets.max_output_vertices as usize
            || output_triangles > request.budgets.max_output_triangles as usize
        {
            return Err(HighWorkerError(
                "HIGH_WORKER_OUTPUT_BUDGET_EXCEEDED".to_owned(),
            ));
        }
        detail_primitives.push(primitive);
        detail_lineage.push(lineage);
    }

    let part_ids = validated.parts.keys().cloned().collect::<Vec<_>>();
    let material_zone_ids = validated
        .parts
        .values()
        .map(|part| part.material_zone_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(HighMeshArtifact {
        schema_version: ARTIFACT_SCHEMA_VERSION.to_owned(),
        operation: OPERATION.to_owned(),
        policy: POLICY.to_owned(),
        artifact_id: String::new(),
        artifact_sha256: String::new(),
        source_authoring_mesh_sha256: request.source_authoring_mesh_sha256.clone(),
        detail_graph_canonical_sha256: request.detail_graph_canonical_sha256.clone(),
        request_sha256: request_sha256.to_owned(),
        input_sha256: input_sha256.to_owned(),
        high_worker_algorithm_sha256: sha256_bytes(ALGORITHM.as_bytes()),
        high_worker_build_cohort_sha256: sha256_bytes(
            format!("{}|{}", ALGORITHM, env!("CARGO_PKG_VERSION")).as_bytes(),
        ),
        replay_count: 2,
        replay_byte_exact: true,
        base_parts,
        detail_primitives,
        detail_lineage,
        part_ids,
        material_zone_ids,
        triangle_count: (base_triangle_count + detail_triangle_count),
        base_triangle_count,
        detail_triangle_count,
        non_destructive: true,
        high_topology_status: "base-preserved-detail-primitives".to_owned(),
        high_authoring_topology_status: "source-preserved".to_owned(),
        uv_status: "NOT_RUN".to_owned(),
        tangent_status: "NOT_RUN".to_owned(),
        structural_status: "PASS_SOURCE_STRUCTURAL".to_owned(),
        visual_status: "NOT_RUN".to_owned(),
        human_status: "NOT_RUN".to_owned(),
        engine_status: "NOT_RUN".to_owned(),
        distribution_status: "NOT_RUN".to_owned(),
        quality_status: "structural_only".to_owned(),
        hard_gate_passed: false,
        runtime_write_performed: false,
        production_stage_advanced: false,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        canonical_sha256: String::new(),
    })
}

fn artifact_preimage(artifact: &HighMeshArtifact) -> Result<Value, HighWorkerError> {
    let mut value = serde_json::to_value(artifact)?;
    value["artifact_id"] = Value::String(String::new());
    value["artifact_sha256"] = Value::String(String::new());
    value["canonical_sha256"] = Value::String(String::new());
    Ok(value)
}

fn build_support_patch(
    node: &DetailNode,
    part: &AuthoringPart,
    edges: &BTreeMap<(String, u32, u32), EdgeInfo>,
    stable_edges: &BTreeMap<String, StableEdge>,
) -> Result<(HighMeshPrimitive, DetailLineage), HighWorkerError> {
    let edge_id = node.source_edge.as_deref().ok_or_else(|| {
        HighWorkerError(format!("HIGH_WORKER_SOURCE_EDGE_MISSING:{}", node.node_id))
    })?;
    let stable_edge = stable_edges.get(edge_id).ok_or_else(|| {
        HighWorkerError(format!(
            "HIGH_WORKER_SOURCE_EDGE_NOT_FOUND:{}:{}",
            part.part_id, edge_id
        ))
    })?;
    let edge = stable_edge.vertex_indices;
    let info = require_edge(edges, &part.part_id, edge)?;
    let a = part.vertices[edge.0 as usize];
    let b = part.vertices[edge.1 as usize];
    let tangent = normalize(sub(b, a))?;
    let width = node.width_m.expect("validated width");
    let count = node.count.expect("validated count");

    // A closed, two-face edge can produce a real bounded bevel/support arc.
    // The base AuthoringMesh remains untouched; this is an additive evaluated
    // primitive with stable source lineage.  Boundary edges retain the legacy
    // two-sided patch because there is no second face from which to derive the
    // arc and rejecting them would unnecessarily break the existing contract.
    if stable_edge.face_inward.len() == 2 {
        let inward_a = stable_edge.face_inward[0];
        let inward_b = stable_edge.face_inward[1];
        let inward_dot = dot(inward_a, inward_b);
        if !inward_dot.is_finite() || inward_dot < -0.25 || inward_dot >= 0.999 {
            return Err(HighWorkerError(
                "HIGH_WORKER_SUPPORT_EDGE_NOT_BOUNDED_CONVEX".to_owned(),
            ));
        }
        let mut positions = Vec::with_capacity(count as usize * 8);
        let mut indices = Vec::with_capacity(count as usize * 4);
        for segment in 0..count {
            let t0 = segment as f32 / count as f32;
            let t1 = (segment + 1) as f32 / count as f32;
            let offset0 = support_arc_offset(inward_a, inward_b, t0, width)?;
            let offset1 = support_arc_offset(inward_a, inward_b, t1, width)?;
            let p0_a = add(a, offset0);
            let p0_b = add(b, offset0);
            let p1_b = add(b, offset1);
            let p1_a = add(a, offset1);
            let start = positions.len() as u32;
            // Emit both windings so the detached source-only primitive remains
            // visible in the fixed renderer without changing the GLB contract.
            positions.extend([p0_a, p0_b, p1_b, p1_a, p0_a, p0_b, p1_b, p1_a]);
            indices.extend([
                [start, start + 1, start + 2],
                [start, start + 2, start + 3],
                [start + 4, start + 6, start + 5],
                [start + 4, start + 7, start + 6],
            ]);
        }
        let primitive_id = format!("detail:{}", node.node_id);
        let mut source_element_lineage = vec![format!("edge:{}", stable_edge.edge_id)];
        source_element_lineage.extend(stable_edge.face_ids.iter().map(|id| format!("face:{id}")));
        source_element_lineage.extend(stable_edge.loop_ids.iter().map(|id| format!("loop:{id}")));
        source_element_lineage.push(format!("part:{}", part.part_id));
        let primitive = HighMeshPrimitive {
            primitive_id: primitive_id.clone(),
            kind: "support_loop_patch".to_owned(),
            part_id: part.part_id.clone(),
            source_node_ids: Vec::new(),
            source_node_id: node.node_id.clone(),
            material_zone_id: part.material_zone_id.clone(),
            source_element_lineage,
            geometry: HighMeshGeometry {
                positions_m: positions,
                indices,
            },
        };
        let lineage = DetailLineage {
            detail_node_id: node.node_id.clone(),
            detail_kind: node.kind.clone(),
            parent_part_id: part.part_id.clone(),
            parent_node_id: node.parent_node_id.clone(),
            source_edge: Some(stable_edge.edge_id.clone()),
            source_vertex_ids: stable_edge.vertex_ids.to_vec(),
            source_loop_ids: stable_edge.loop_ids.clone(),
            source_face_ids: stable_edge.face_ids.clone(),
            output_primitive_id: primitive_id,
            relation: "attached_edge_patch".to_owned(),
            geometry_delta: "additive_support_surface".to_owned(),
            non_destructive: true,
        };
        return Ok((primitive, lineage));
    }

    let normal = normalize(info.face_normal)?;
    let side = normalize(cross(tangent, normal))?;
    let mut positions = Vec::with_capacity(count as usize * 8);
    let mut indices = Vec::with_capacity(count as usize * 4);
    for loop_index in 0..count {
        let inner = width * loop_index as f32;
        let outer = width * (loop_index + 1) as f32;
        let start = positions.len() as u32;
        let plus_inner_a = add(a, scale(side, inner));
        let plus_inner_b = add(b, scale(side, inner));
        let plus_outer_b = add(b, scale(side, outer));
        let plus_outer_a = add(a, scale(side, outer));
        let minus_inner_a = sub(a, scale(side, inner));
        let minus_inner_b = sub(b, scale(side, inner));
        let minus_outer_b = sub(b, scale(side, outer));
        let minus_outer_a = sub(a, scale(side, outer));
        positions.extend([
            plus_inner_a,
            plus_inner_b,
            plus_outer_b,
            plus_outer_a,
            minus_inner_a,
            minus_inner_b,
            minus_outer_b,
            minus_outer_a,
        ]);
        indices.extend([
            [start, start + 1, start + 2],
            [start, start + 2, start + 3],
            [start + 4, start + 6, start + 5],
            [start + 4, start + 7, start + 6],
        ]);
    }
    let primitive_id = format!("detail:{}", node.node_id);
    let mut source_element_lineage = vec![format!("edge:{}", stable_edge.edge_id)];
    source_element_lineage.extend(stable_edge.face_ids.iter().map(|id| format!("face:{id}")));
    source_element_lineage.extend(stable_edge.loop_ids.iter().map(|id| format!("loop:{id}")));
    source_element_lineage.push(format!("part:{}", part.part_id));
    let primitive = HighMeshPrimitive {
        primitive_id: primitive_id.clone(),
        kind: "support_loop_patch".to_owned(),
        part_id: part.part_id.clone(),
        source_node_ids: Vec::new(),
        source_node_id: node.node_id.clone(),
        material_zone_id: part.material_zone_id.clone(),
        source_element_lineage,
        geometry: HighMeshGeometry {
            positions_m: positions,
            indices,
        },
    };
    let lineage = DetailLineage {
        detail_node_id: node.node_id.clone(),
        detail_kind: node.kind.clone(),
        parent_part_id: part.part_id.clone(),
        parent_node_id: node.parent_node_id.clone(),
        source_edge: Some(stable_edge.edge_id.clone()),
        source_vertex_ids: stable_edge.vertex_ids.to_vec(),
        source_loop_ids: stable_edge.loop_ids.clone(),
        source_face_ids: stable_edge.face_ids.clone(),
        output_primitive_id: primitive_id,
        relation: "attached_edge_patch".to_owned(),
        geometry_delta: "additive_support_surface".to_owned(),
        non_destructive: true,
    };
    Ok((primitive, lineage))
}

fn build_crease_metadata(
    node: &DetailNode,
    part: &AuthoringPart,
    edges: &BTreeMap<(String, u32, u32), EdgeInfo>,
    stable_edges: &BTreeMap<String, StableEdge>,
) -> Result<(HighMeshPrimitive, DetailLineage), HighWorkerError> {
    let edge_id = node.source_edge.as_deref().ok_or_else(|| {
        HighWorkerError(format!("HIGH_WORKER_SOURCE_EDGE_MISSING:{}", node.node_id))
    })?;
    let stable_edge = stable_edges.get(edge_id).ok_or_else(|| {
        HighWorkerError(format!(
            "HIGH_WORKER_SOURCE_EDGE_NOT_FOUND:{}:{}",
            part.part_id, edge_id
        ))
    })?;
    let edge = stable_edge.vertex_indices;
    let _ = require_edge(edges, &part.part_id, edge)?;
    let primitive_id = format!("detail:{}", node.node_id);
    let primitive = HighMeshPrimitive {
        primitive_id: primitive_id.clone(),
        kind: "crease_metadata".to_owned(),
        part_id: part.part_id.clone(),
        source_node_ids: Vec::new(),
        source_node_id: node.node_id.clone(),
        material_zone_id: part.material_zone_id.clone(),
        source_element_lineage: vec![format!("edge:{}", stable_edge.edge_id)],
        geometry: HighMeshGeometry {
            positions_m: Vec::new(),
            indices: Vec::new(),
        },
    };
    let lineage = DetailLineage {
        detail_node_id: node.node_id.clone(),
        detail_kind: node.kind.clone(),
        parent_part_id: part.part_id.clone(),
        parent_node_id: node.parent_node_id.clone(),
        source_edge: Some(stable_edge.edge_id.clone()),
        source_vertex_ids: stable_edge.vertex_ids.to_vec(),
        source_loop_ids: stable_edge.loop_ids.clone(),
        source_face_ids: stable_edge.face_ids.clone(),
        output_primitive_id: primitive_id,
        relation: "attached_edge_metadata".to_owned(),
        geometry_delta: "none_crease_attribute_only".to_owned(),
        non_destructive: true,
    };
    Ok((primitive, lineage))
}

fn build_floating_detail(
    node: &DetailNode,
    part: &AuthoringPart,
) -> Result<(HighMeshPrimitive, DetailLineage), HighWorkerError> {
    let center = node.center_m.expect("validated center");
    let size = node.size_m.expect("validated size");
    let half = scale(size, 0.5);
    let [hx, hy, hz] = half;
    let [cx, cy, cz] = center;
    let positions = vec![
        [cx - hx, cy - hy, cz - hz],
        [cx + hx, cy - hy, cz - hz],
        [cx + hx, cy + hy, cz - hz],
        [cx - hx, cy + hy, cz - hz],
        [cx - hx, cy - hy, cz + hz],
        [cx + hx, cy - hy, cz + hz],
        [cx + hx, cy + hy, cz + hz],
        [cx - hx, cy + hy, cz + hz],
    ];
    let indices = vec![
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [3, 7, 6],
        [3, 6, 2],
        [0, 4, 7],
        [0, 7, 3],
        [1, 2, 6],
        [1, 6, 5],
    ];
    let primitive_id = format!("detail:{}", node.node_id);
    let primitive = HighMeshPrimitive {
        primitive_id: primitive_id.clone(),
        kind: "floating_detail_box".to_owned(),
        part_id: part.part_id.clone(),
        source_node_ids: Vec::new(),
        source_node_id: node.node_id.clone(),
        material_zone_id: part.material_zone_id.clone(),
        source_element_lineage: vec![format!("part:{}", part.part_id)],
        geometry: HighMeshGeometry {
            positions_m: positions,
            indices,
        },
    };
    let lineage = DetailLineage {
        detail_node_id: node.node_id.clone(),
        detail_kind: node.kind.clone(),
        parent_part_id: part.part_id.clone(),
        parent_node_id: node.parent_node_id.clone(),
        source_edge: None,
        source_vertex_ids: Vec::new(),
        source_loop_ids: Vec::new(),
        source_face_ids: Vec::new(),
        output_primitive_id: primitive_id,
        relation: "detached_child_part".to_owned(),
        geometry_delta: "additive_floating_detail".to_owned(),
        non_destructive: true,
    };
    Ok((primitive, lineage))
}

fn normalized_source_edge(
    node: &DetailNode,
    stable_edges: &BTreeMap<String, StableEdge>,
) -> Result<(u32, u32), HighWorkerError> {
    let edge_id = node.source_edge.as_deref().ok_or_else(|| {
        HighWorkerError(format!("HIGH_WORKER_SOURCE_EDGE_MISSING:{}", node.node_id))
    })?;
    stable_edges
        .get(edge_id)
        .map(|edge| edge.vertex_indices)
        .ok_or_else(|| {
            HighWorkerError(format!(
                "HIGH_WORKER_SOURCE_EDGE_NOT_FOUND:{}:{}",
                node.parent_part_id, edge_id
            ))
        })
}

fn require_edge<'a>(
    edges: &'a BTreeMap<(String, u32, u32), EdgeInfo>,
    part_id: &str,
    edge: (u32, u32),
) -> Result<&'a EdgeInfo, HighWorkerError> {
    edges
        .get(&(part_id.to_owned(), edge.0, edge.1))
        .ok_or_else(|| {
            HighWorkerError(format!(
                "HIGH_WORKER_SOURCE_EDGE_NOT_FOUND:{}:{}-{}",
                part_id, edge.0, edge.1
            ))
        })
}

fn part_bounds(part: &AuthoringPart) -> Result<Bounds, HighWorkerError> {
    let mut bounds = Bounds {
        min: part.vertices[0],
        max: part.vertices[0],
    };
    for vertex in &part.vertices[1..] {
        for axis in 0..3 {
            bounds.min[axis] = bounds.min[axis].min(vertex[axis]);
            bounds.max[axis] = bounds.max[axis].max(vertex[axis]);
        }
    }
    Ok(bounds)
}

fn aabb_intersects(left: Bounds, right: Bounds) -> bool {
    (0..3).all(|axis| {
        left.min[axis] <= right.max[axis] + EPSILON && right.min[axis] <= left.max[axis] + EPSILON
    })
}

fn face_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Result<[f32; 3], HighWorkerError> {
    normalize(cross(sub(b, a), sub(c, a)))
}

fn triangle_area(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f32 {
    let cross_product = cross(sub(b, a), sub(c, a));
    0.5 * dot(cross_product, cross_product).sqrt()
}

fn validate_vec(value: [f32; 3], max_abs: f32, code: &str) -> Result<(), HighWorkerError> {
    if value
        .iter()
        .any(|component| !component.is_finite() || component.abs() > max_abs)
    {
        return Err(HighWorkerError(code.to_owned()));
    }
    Ok(())
}

fn validate_positive_vec(value: [f32; 3], max_abs: f32, code: &str) -> Result<(), HighWorkerError> {
    validate_vec(value, max_abs, code)?;
    if value.iter().any(|component| *component <= EPSILON) {
        return Err(HighWorkerError(code.to_owned()));
    }
    Ok(())
}

fn require_id(value: &str, label: &str) -> Result<(), HighWorkerError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        || value.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b':'))
        })
    {
        return Err(HighWorkerError(format!("HIGH_WORKER_ID_INVALID:{label}")));
    }
    Ok(())
}

fn require_stable_id(value: &str, label: &str) -> Result<(), HighWorkerError> {
    if value.is_empty()
        || value.len() > 128
        || value.bytes().any(|byte| {
            !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b':'))
        })
    {
        return Err(HighWorkerError(format!(
            "HIGH_WORKER_STABLE_ID_INVALID:{label}"
        )));
    }
    Ok(())
}

fn ordered_edge(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: [f32; 3], scalar: f32) -> [f32; 3] {
    [a[0] * scalar, a[1] * scalar, a[2] * scalar]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(value: [f32; 3]) -> Result<[f32; 3], HighWorkerError> {
    let length = dot(value, value).sqrt();
    if !length.is_finite() || length <= EPSILON {
        return Err(HighWorkerError("HIGH_WORKER_VECTOR_DEGENERATE".to_owned()));
    }
    Ok(scale(value, 1.0 / length))
}

/// Interpolate the two face-interior directions on the unit sphere.  The
/// fixed arc gives 1..=3 segments a deterministic chamfer profile while the
/// bounded convexity check in `build_support_patch` rejects the degenerate
/// opposite-direction case before this helper is called.
fn support_arc_offset(
    inward_a: [f32; 3],
    inward_b: [f32; 3],
    t: f32,
    width: f32,
) -> Result<[f32; 3], HighWorkerError> {
    if !t.is_finite() || !(0.0..=1.0).contains(&t) {
        return Err(HighWorkerError(
            "HIGH_WORKER_SUPPORT_ARC_PARAMETER_INVALID".to_owned(),
        ));
    }
    let blended = add(scale(inward_a, 1.0 - t), scale(inward_b, t));
    Ok(scale(normalize(blended)?, width))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_pair<T: Serialize, U: Serialize>(
    first: &T,
    second: &U,
) -> Result<String, HighWorkerError> {
    let value = serde_json::json!({"source": first, "detail_graph": second});
    Ok(sha256_bytes(&canonical_bytes(&value)))
}

fn sha256_value<T: Serialize>(value: &T) -> Result<String, HighWorkerError> {
    Ok(sha256_bytes(&canonical_bytes(&serde_json::to_value(
        value,
    )?)))
}

fn sha256_bytes(value: &[u8]) -> String {
    let digest = Sha256::digest(value);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Canonical JSON encoding with lexicographically sorted object keys.  Array
/// order remains meaningful because mesh and detail node order is part of the
/// typed lineage contract.
pub fn canonical_bytes(value: &Value) -> Vec<u8> {
    let mut output = String::new();
    write_canonical(value, &mut output);
    output.into_bytes()
}

fn write_canonical(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => {
            output.push_str(&serde_json::to_string(value).expect("string serialization"))
        }
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                write_canonical(value, output);
            }
            output.push(']');
        }
        Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            output.push('{');
            for (index, (key, value)) in entries.into_iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                output.push_str(&serde_json::to_string(key).expect("key serialization"));
                output.push(':');
                write_canonical(value, output);
            }
            output.push('}');
        }
    }
}

/// Build a canonical request suitable for a caller that wants to calculate
/// `canonical_sha256` before sending the request.
pub fn request_preimage(request: &HighMeshWorkerRequest) -> Result<Value, HighWorkerError> {
    let mut value = serde_json::to_value(request)?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_sha() -> String {
        "1".repeat(64)
    }

    fn fixture_lineage(element_id: &str) -> Value {
        let mut lineage = serde_json::json!({
            "original_element_ids": [element_id],
            "evaluated_element_ids": [],
            "correspondence_kind": "not_materialized",
            "correspondence_sha256": "",
        });
        lineage["correspondence_sha256"] = Value::String(
            sha256_value(&serde_json::json!({
                "original_element_ids": [element_id],
                "evaluated_element_ids": [],
                "correspondence_kind": "not_materialized",
            }))
            .expect("fixture lineage hash"),
        );
        lineage
    }

    fn canonical_mesh() -> Value {
        let positions = [
            [0.0_f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 0.2],
            [1.0, 0.0, 0.2],
            [1.0, 1.0, 0.2],
            [0.0, 1.0, 0.2],
        ];
        let vertex_id = |index: usize| format!("v{index:03}");
        let edge_id = |a: usize, b: usize| {
            let (low, high) = if a < b { (a, b) } else { (b, a) };
            format!("e-v{low:03}-v{high:03}")
        };
        let face_specs: [(&str, [usize; 4]); 6] = [
            ("f-bottom", [0, 3, 2, 1]),
            ("f-front", [0, 1, 5, 4]),
            ("f-left", [0, 4, 7, 3]),
            ("f-rear", [3, 7, 6, 2]),
            ("f-right", [1, 2, 6, 5]),
            ("f-top", [4, 5, 6, 7]),
        ];

        #[derive(Clone)]
        struct RawHalfEdge {
            id: String,
            origin: usize,
            destination: usize,
            edge_id: String,
            face_id: String,
            corner_id: String,
            next_id: String,
            prev_id: String,
        }

        let mut raw_half_edges = Vec::with_capacity(24);
        for (face_id, cycle) in face_specs {
            for ordinal in 0..cycle.len() {
                let origin = cycle[ordinal];
                let destination = cycle[(ordinal + 1) % cycle.len()];
                raw_half_edges.push(RawHalfEdge {
                    id: format!("he-{face_id}-{ordinal}"),
                    origin,
                    destination,
                    edge_id: edge_id(origin, destination),
                    face_id: face_id.to_owned(),
                    corner_id: format!("corner-{face_id}-{ordinal}"),
                    next_id: format!("he-{face_id}-{}", (ordinal + 1) % cycle.len()),
                    prev_id: format!("he-{face_id}-{}", (ordinal + cycle.len() - 1) % cycle.len()),
                });
            }
        }

        let directed_half_edges = raw_half_edges
            .iter()
            .map(|half_edge| {
                (
                    (half_edge.origin, half_edge.destination),
                    half_edge.id.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut edge_half_edges = BTreeMap::<String, Vec<String>>::new();
        let mut edge_endpoints = BTreeMap::<String, [String; 2]>::new();
        let mut outgoing_half_edges = BTreeMap::<usize, String>::new();
        for half_edge in &raw_half_edges {
            edge_half_edges
                .entry(half_edge.edge_id.clone())
                .or_default()
                .push(half_edge.id.clone());
            let low = vertex_id(half_edge.origin.min(half_edge.destination));
            let high = vertex_id(half_edge.origin.max(half_edge.destination));
            edge_endpoints
                .entry(half_edge.edge_id.clone())
                .or_insert([low, high]);
            outgoing_half_edges
                .entry(half_edge.origin)
                .or_insert_with(|| half_edge.id.clone());
        }

        let vertices = positions
            .iter()
            .enumerate()
            .map(|(index, position)| {
                let id = vertex_id(index);
                serde_json::json!({
                    "vertex_id": id.clone(),
                    "position_m": position,
                    "outgoing_half_edge_id": outgoing_half_edges[&index],
                    "boundary": false,
                    "lineage": fixture_lineage(&id),
                })
            })
            .collect::<Vec<_>>();

        let edges = edge_endpoints
            .iter()
            .map(|(id, endpoints)| {
                serde_json::json!({
                    "edge_id": id,
                    "vertex_ids": endpoints,
                    "half_edge_ids": edge_half_edges[id],
                    "boundary": false,
                    "hard_edge": true,
                    "crease": 0,
                    "uv_seam": false,
                    "lineage": fixture_lineage(id),
                })
            })
            .collect::<Vec<_>>();

        let half_edges = raw_half_edges
            .iter()
            .map(|half_edge| {
                let twin_id = directed_half_edges
                    .get(&(half_edge.destination, half_edge.origin))
                    .expect("closed cube half-edge twin");
                let orientation =
                    if vertex_id(half_edge.origin) == edge_endpoints[&half_edge.edge_id][0] {
                        "forward"
                    } else {
                        "reverse"
                    };
                serde_json::json!({
                    "half_edge_id": half_edge.id,
                    "origin_vertex_id": vertex_id(half_edge.origin),
                    "edge_id": half_edge.edge_id,
                    "face_id": half_edge.face_id,
                    "corner_id": half_edge.corner_id,
                    "twin_id": twin_id,
                    "next_id": half_edge.next_id,
                    "prev_id": half_edge.prev_id,
                    "boundary": false,
                    "orientation": orientation,
                    "lineage": fixture_lineage(&half_edge.id),
                })
            })
            .collect::<Vec<_>>();

        let corners = raw_half_edges
            .iter()
            .enumerate()
            .map(|(ordinal, half_edge)| {
                serde_json::json!({
                    "corner_id": half_edge.corner_id,
                    "face_id": half_edge.face_id,
                    "half_edge_id": half_edge.id,
                    "vertex_id": vertex_id(half_edge.origin),
                    "ordinal": ordinal % 4,
                    "lineage": fixture_lineage(&half_edge.corner_id),
                })
            })
            .collect::<Vec<_>>();

        let faces = face_specs
            .iter()
            .map(|(face_id, cycle)| {
                let corner_ids = (0..cycle.len())
                    .map(|ordinal| format!("corner-{face_id}-{ordinal}"))
                    .collect::<Vec<_>>();
                let id = (*face_id).to_owned();
                serde_json::json!({
                    "face_id": id.clone(),
                    "first_half_edge_id": format!("he-{face_id}-0"),
                    "corner_ids": corner_ids,
                    "degree": cycle.len(),
                    "boundary": false,
                    "lineage": fixture_lineage(&id),
                })
            })
            .collect::<Vec<_>>();

        let loops = face_specs
            .iter()
            .map(|(face_id, cycle)| {
                let half_edge_ids = (0..cycle.len())
                    .map(|ordinal| format!("he-{face_id}-{ordinal}"))
                    .collect::<Vec<_>>();
                let id = format!("loop-{face_id}");
                serde_json::json!({
                    "loop_id": id.clone(),
                    "face_id": face_id,
                    "first_half_edge_id": half_edge_ids[0],
                    "half_edge_ids": half_edge_ids,
                    "boundary": false,
                    "lineage": fixture_lineage(&id),
                })
            })
            .collect::<Vec<_>>();

        let lineage = fixture_sha();
        let mut canonical = serde_json::json!({
            "schema_version": AUTHORING_MESH_SCHEMA_VERSION,
            "canonical_mesh_id": "canonical-mesh-fixture",
            "project_id": "project-fixture",
            "candidate_id": "candidate-fixture",
            "candidate_state_sha256": fixture_sha(),
            "base_version_id": Value::Null,
            "authoring_node_id": "receiver-source",
            "part_id": "receiver",
            "source_program_object_sha256": fixture_sha(),
            "source_program_sha256": fixture_sha(),
            "source_artifact_object_sha256": fixture_sha(),
            "source_artifact_sha256": fixture_sha(),
            "source_artifact_readback_object_sha256": fixture_sha(),
            "source_artifact_readback_sha256": fixture_sha(),
            "source_lineage_sha256": lineage,
            "representation": "runtime-owned-original-half-edge@1",
            "storage_policy": "runtime-owned-sqlite-cas-canonical-authoring-mesh@1",
            "writer_policy": "forgecad-runtime-only-state-writer@1",
            "original_identity": {
                "identity_id": "identity-original-fixture",
                "namespace": "original",
                "identity_kind": "runtime-owned-original-authoring@1",
                "element_id_policy": "lineage-scoped-opaque-not-cross-version-stable@1",
                "topology_sha256": fixture_sha(),
                "source_lineage_sha256": fixture_sha(),
                "stability_scope": "same-canonical-mesh-lineage-only@1",
            },
            "evaluated_identity": {
                "identity_id": "identity-evaluated-fixture",
                "namespace": "evaluated",
                "identity_kind": "runtime-derived-evaluated-artifact-readback@1",
                "element_id_policy": "artifact-local-no-authoring-bijection@1",
                "correspondence_policy": "non-bijective-derived-only@1",
                "artifact_object_sha256": fixture_sha(),
                "artifact_readback_sha256": fixture_sha(),
                "source_lineage_sha256": fixture_sha(),
                "cross_version_stable": false,
            },
            "cross_version_stable": false,
            "cross_version_stability": {
                "status": "not-proven@1",
                "scope": "same-canonical-mesh-lineage-only@1",
                "stable_id_claim": "none-across-revisions@1",
                "deleted_id_reuse_policy": "not-proven-and-not-a-contract@1",
                "new_id_policy": "lineage-operation-parent-derived-draft-only@1",
                "evaluated_id_policy": "artifact-local-unstable-derived-only@1",
            },
            "counts": {
                "vertex_count": 8,
                "edge_count": 12,
                "half_edge_count": 24,
                "corner_count": 24,
                "face_count": 6,
                "loop_count": 6,
                "ring_count": 0,
                "boundary_edge_count": 0,
                "boundary_half_edge_count": 0,
                "hard_edge_count": 12,
                "crease_edge_count": 0,
                "uv_seam_count": 0,
            },
            "vertices": vertices,
            "edges": edges,
            "half_edges": half_edges,
            "corners": corners,
            "faces": faces,
            "loops": loops,
            "rings": [],
            "topology": {
                "boundary_edge_count": 0,
                "boundary_half_edge_count": 0,
                "non_manifold_edge_count": 0,
                "orientation_conflict_count": 0,
                "status": "closed_manifold",
                "validation_status": "passed",
                "rejection_policy": "fail-closed-on-non-manifold@1",
                "face_cycle_policy": "next-prev-complete-mutual@1",
                "twin_policy": "boundary-only-null-symmetric@1",
                "boundary_policy": "single-half-edge-per-boundary-edge@1",
            },
            "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
            "runtime_write_performed": true,
            "persistent_user_data_touched": true,
            "stage_advanced": false,
            "candidate_confirmed": false,
            "version_created": false,
            "export_performed": false,
            "quality_status": "structural_only",
            "canonical_sha256": "",
        });
        let canonical_sha256 = sha256_value(&canonical).expect("canonical fixture hash");
        canonical["canonical_sha256"] = Value::String(canonical_sha256.clone());
        canonical
    }

    fn source_adapter() -> Value {
        let canonical = canonical_mesh();
        let canonical_sha256 = canonical["canonical_sha256"]
            .as_str()
            .expect("canonical fixture sha")
            .to_owned();
        let candidate_state_sha256 = canonical["candidate_state_sha256"]
            .as_str()
            .expect("candidate fixture sha")
            .to_owned();
        serde_json::json!({
            "schema_version": "HighWorkerAuthoringMeshAdapter@1",
            "canonical_mesh": canonical,
            "candidate_id": "candidate-fixture",
            "candidate_state_sha256": candidate_state_sha256,
            "head_candidate_id": "candidate-fixture",
            "head_candidate_state_sha256": fixture_sha(),
            "source_mesh_sha256": canonical_sha256,
        })
    }

    pub(crate) fn request() -> HighMeshWorkerRequest {
        let detail_graph = DetailGraph {
            schema_version: DETAIL_GRAPH_SCHEMA_VERSION.to_owned(),
            nodes: vec![
                DetailNode {
                    node_id: "crease-top".to_owned(),
                    kind: "crease".to_owned(),
                    parent_part_id: "receiver".to_owned(),
                    parent_node_id: None,
                    source_edge: Some("e-v001-v005".to_owned()),
                    width_m: None,
                    count: None,
                    sharpness: Some(3.0),
                    center_m: None,
                    size_m: None,
                },
                DetailNode {
                    node_id: "floater-side".to_owned(),
                    kind: "floating_detail".to_owned(),
                    parent_part_id: "receiver".to_owned(),
                    parent_node_id: None,
                    source_edge: None,
                    width_m: None,
                    count: None,
                    sharpness: None,
                    center_m: Some([0.5, 0.5, 0.45]),
                    size_m: Some([0.2, 0.2, 0.05]),
                },
                DetailNode {
                    node_id: "support-top".to_owned(),
                    kind: "support_loop".to_owned(),
                    parent_part_id: "receiver".to_owned(),
                    parent_node_id: None,
                    source_edge: Some("e-v000-v001".to_owned()),
                    width_m: Some(0.02),
                    count: Some(2),
                    sharpness: None,
                    center_m: None,
                    size_m: None,
                },
            ],
        };
        let mut request = HighMeshWorkerRequest {
            schema_version: REQUEST_SCHEMA_VERSION.to_owned(),
            operation: OPERATION.to_owned(),
            source_authoring_mesh: source_adapter(),
            source_authoring_mesh_sha256: String::new(),
            detail_graph,
            detail_graph_canonical_sha256: String::new(),
            budgets: HighWorkerBudgets {
                max_detail_nodes: 16,
                max_output_vertices: 1024,
                max_output_triangles: 2048,
            },
            canonical_sha256: String::new(),
        };
        request.source_authoring_mesh_sha256 =
            sha256_value(&request.source_authoring_mesh).unwrap();
        request.detail_graph_canonical_sha256 = sha256_value(&request.detail_graph).unwrap();
        request.canonical_sha256 = sha256_value(&request_preimage(&request).unwrap()).unwrap();
        request
    }

    #[test]
    fn high_artifact_replay_is_byte_exact_and_preserves_base() {
        let request = request();
        let result = run(&request).expect("high artifact");
        assert_eq!(result.replay_count, 2);
        assert!(result.replay_byte_exact);
        assert!(result.non_destructive);
        assert_eq!(result.base_triangle_count, 12);
        assert_eq!(result.detail_lineage.len(), 3);
        assert_eq!(
            result
                .detail_primitives
                .iter()
                .filter(|primitive| primitive.kind == "crease_metadata")
                .count(),
            1
        );
        assert_eq!(
            result
                .detail_primitives
                .iter()
                .filter(|primitive| primitive.kind == "support_loop_patch")
                .count(),
            1
        );
        assert_eq!(
            result
                .detail_primitives
                .iter()
                .filter(|primitive| primitive.kind == "floating_detail_box")
                .count(),
            1
        );
        let repeat = run(&request).expect("repeat high artifact");
        assert_eq!(result, repeat);
    }

    #[test]
    fn interior_support_loop_emits_a_bounded_face_arc() {
        let result = run(&request()).expect("high artifact");
        let support = result
            .detail_primitives
            .iter()
            .find(|primitive| primitive.kind == "support_loop_patch")
            .expect("support loop primitive");
        assert_eq!(support.geometry.positions_m.len(), 16);
        assert_eq!(support.geometry.indices.len(), 8);

        // The fixture edge is shared by orthogonal faces.  Its middle arc
        // sample must move into both face interiors instead of remaining on a
        // single flat offset strip.
        let middle = support.geometry.positions_m[2];
        assert!(middle[1] > 0.0 && middle[2] > 0.0);
        assert!((middle[1] - middle[2]).abs() < 1.0e-5);
    }

    #[test]
    fn floater_intersection_fails_closed() {
        let mut request = request();
        request.detail_graph.nodes[1].center_m = Some([0.5, 0.5, 0.1]);
        request.detail_graph_canonical_sha256 = sha256_value(&request.detail_graph).unwrap();
        request.canonical_sha256 = sha256_value(&request_preimage(&request).unwrap()).unwrap();
        let error = run(&request).expect_err("intersection should fail");
        assert!(error.0.contains("FLOATING_DETAIL_INTERSECTS_BASE"));
    }

    #[test]
    fn request_rejects_caller_detail_lineage_fields() {
        let request = request();
        let mut value = serde_json::to_value(request).unwrap();
        value["detail_graph"]["nodes"][0]["source_element_lineage"] = Value::Array(Vec::new());
        let error = run_json(&serde_json::to_vec(&value).unwrap()).expect_err("unknown field");
        assert!(error.0.contains("unknown field"));
    }
}
