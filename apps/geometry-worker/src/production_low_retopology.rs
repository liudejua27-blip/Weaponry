//! Deterministic, bounded triangulated Low retopology source kernel.
//!
//! This kernel is deliberately smaller than a production authoring system. It
//! derives a new closed triangle topology from an admitted High diagnostic
//! mesh and records exact source correspondence. It never claims artist-made
//! quad flow, UV quality, stage advancement, or commercial art quality.

use crate::integrity::{self, DiagnosticMesh, DiagnosticPrimitive};
use crate::GeometryError;
use base64::Engine;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const AREA_EPSILON: f64 = 1.0e-12;
const VOLUME_EPSILON: f64 = 1.0e-12;
const MAX_SOURCE_GLB_BYTES: usize = 64 * 1024 * 1024;
pub const REQUEST_SCHEMA_VERSION: &str = "LowRetopologyWorkerRequest@1";
pub const RESULT_SCHEMA_VERSION: &str = "LowRetopologyWorkerResult@1";
pub const POLICY: &str = "bounded-closed-manifold-triangulated-edge-collapse@1";
pub const ALGORITHM: &str = "deterministic-shortest-safe-edge-collapse@1";
const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "preview_only",
    "source_high_artifact_sha256",
    "high_glb_base64",
    "target_triangle_count",
    "max_collapses",
    "locked_vertices",
    "retopology_policy",
    "algorithm",
    "canonical_sha256",
];

#[derive(Debug, Clone)]
pub struct LowRetopologyPolicy {
    pub target_triangle_count: usize,
    pub max_collapses: usize,
    /// Stable source vertices that may not be removed. The pair is
    /// `(primitive_ordinal, source_vertex_index)`.
    pub locked_vertices: BTreeSet<(usize, u32)>,
    /// Derived source edges that must not be collapsed. These are deliberately
    /// not a wire field: the Worker derives them from the admitted GLB's
    /// corner attributes (split-normal/tangent seams), while callers can still only
    /// supply the existing vertex-lock list. The pair is
    /// `(primitive_ordinal, source_vertex_index_a, source_vertex_index_b)`.
    pub protected_edges: BTreeSet<(usize, u32, u32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LowVertexCorrespondence {
    pub low_vertex_index: u32,
    pub source_vertex_indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LowFaceCorrespondence {
    pub low_face_index: u32,
    pub source_face_index: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedLowPrimitive {
    pub part_id: String,
    pub source_node_id: String,
    pub material_zone_id: String,
    pub solid: bool,
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub vertex_correspondence: Vec<LowVertexCorrespondence>,
    pub face_correspondence: Vec<LowFaceCorrespondence>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DerivedLowMesh {
    pub primitives: Vec<DerivedLowPrimitive>,
    pub source_triangle_count: usize,
    pub low_triangle_count: usize,
    pub retopology_derived: bool,
    pub artist_authored_quad_topology: bool,
    pub edge_flow_status: String,
    pub quality_status: String,
    pub production_stage_advanced: bool,
    pub promotion_eligible: bool,
}

/// Closed Worker payload adapter for the pure retopology kernel. It returns a
/// canonical mesh/correspondence projection only; CAS ownership and GLB
/// lowering remain Runtime/producer responsibilities.
pub fn run(payload: &Map<String, Value>) -> Result<Value, GeometryError> {
    if payload
        .keys()
        .any(|field| !REQUEST_FIELDS.contains(&field.as_str()))
        || payload.len() != REQUEST_FIELDS.len()
        || payload.get("schema_version").and_then(Value::as_str) != Some(REQUEST_SCHEMA_VERSION)
        || payload.get("preview_only").and_then(Value::as_bool) != Some(true)
        || payload.get("retopology_policy").and_then(Value::as_str) != Some(POLICY)
        || payload.get("algorithm").and_then(Value::as_str) != Some(ALGORITHM)
    {
        return Err(invalid("LOW_RETOPOLOGY_REQUEST_INVALID"));
    }
    let canonical = required_sha(payload, "canonical_sha256")?;
    let mut preimage = payload.clone();
    preimage.remove("canonical_sha256");
    if crate::canonical_hash(&Value::Object(preimage)) != canonical {
        return Err(invalid("LOW_RETOPOLOGY_REQUEST_CANONICAL_MISMATCH"));
    }
    let source_hash = required_sha(payload, "source_high_artifact_sha256")?;
    let encoded = payload
        .get("high_glb_base64")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("LOW_RETOPOLOGY_SOURCE_GLB_MISSING"))?;
    if encoded.len() > MAX_SOURCE_GLB_BYTES * 2 {
        return Err(invalid("LOW_RETOPOLOGY_SOURCE_GLB_TOO_LARGE"));
    }
    let glb = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid("LOW_RETOPOLOGY_SOURCE_GLB_INVALID"))?;
    if glb.is_empty() || glb.len() > MAX_SOURCE_GLB_BYTES || sha256_hex(&glb) != source_hash {
        return Err(invalid("LOW_RETOPOLOGY_SOURCE_HASH_MISMATCH"));
    }
    let inspection = integrity::inspect_glb(&glb)?;
    if !inspection.hard_gate_passed {
        return Err(invalid("LOW_RETOPOLOGY_SOURCE_READBACK_FAILED"));
    }
    let mesh = integrity::extract_diagnostic_mesh(&glb, 1_000_000)?;
    let target_triangle_count = required_usize(payload, "target_triangle_count")?;
    let max_collapses = required_usize(payload, "max_collapses")?;
    let locked_vertices = parse_locked_vertices(payload)?;
    let protected_edges = derive_attribute_seam_edges(&glb, &mesh)?;
    if locked_vertices.iter().any(|(primitive, vertex)| {
        mesh.primitives
            .get(*primitive)
            .is_none_or(|source| *vertex as usize >= source.positions.len())
    }) {
        return Err(invalid("LOW_RETOPOLOGY_LOCKED_VERTEX_OUT_OF_RANGE"));
    }
    let derived = derive_bounded_low(
        &mesh,
        &LowRetopologyPolicy {
            target_triangle_count,
            max_collapses,
            locked_vertices,
            protected_edges,
        },
    )?;
    let low_mesh = low_mesh_value(&derived);
    // Bind the exact JSON value that crosses the Worker wire. Some source
    // coordinates contain signed zero / f32 spellings whose in-memory serde
    // number representation is normalized during serialization. Runtime
    // validates the received value, so the mapping hash must use that same
    // wire-normalized representation.
    let low_mesh_sha256 = wire_value_hash(&low_mesh)?;
    let low_artifact = lower_low_glb(&derived, &low_mesh_sha256)?;
    let low_artifact_sha256 = sha256_hex(&low_artifact.glb);
    let low_readback = integrity::inspect_glb(&low_artifact.glb)?;
    if !low_readback.hard_gate_passed
        || low_readback.triangle_count as usize != derived.low_triangle_count
    {
        return Err(invalid(format!(
            "LOW_RETOPOLOGY_DERIVED_READBACK_FAILED: failures={:?} expected_triangles={} readback_triangles={} boundary={} non_manifold={} winding={} tangent_handedness={}",
            low_readback.failure_codes,
            derived.low_triangle_count,
            low_readback.triangle_count,
            low_readback.boundary_edge_count,
            low_readback.non_manifold_edge_count,
            low_readback.winding_error_count,
            low_readback.tangent_handedness_error_count
        )));
    }
    let mut result = json!({
        "schema_version":RESULT_SCHEMA_VERSION,
        "operation":forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION,
        "source_high_artifact_sha256":source_hash,
        "retopology_policy":POLICY,
        "algorithm":ALGORITHM,
        "algorithm_sha256":sha256_hex(ALGORITHM.as_bytes()),
        "source_triangle_count":derived.source_triangle_count,
        "low_triangle_count":derived.low_triangle_count,
        "low_mesh":low_mesh,
        "low_mesh_sha256":low_mesh_sha256,
        "low_artifact_sha256":low_artifact_sha256,
        "low_glb_base64":base64::engine::general_purpose::STANDARD.encode(&low_artifact.glb),
        "low_artifact_readback":low_readback.report_value(),
        "low_program_sha256":low_artifact.program_sha256,
        "retopology_derived":true,
        "artist_authored_quad_topology":false,
        "edge_flow_status":"NOT_PROVEN",
        "quality_status":"structural_only",
        "runtime_write_performed":false,
        "production_stage_advanced":false,
        "promotion_eligible":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(wire_canonical_hash(&result)?);
    Ok(result)
}

const QUAD_DRAFT_SCHEMA_VERSION: &str = "LowQuadRetopologyDraft@1";
const QUAD_DRAFT_REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "preview_only",
    "project_id",
    "source_high_artifact_sha256",
    "source_high_artifact_readback_sha256",
    "source_high_part_id",
    "source_high_node_id",
    "source_high_material_zone_id",
    "draft",
    "max_vertices",
    "max_edges",
    "max_faces",
    "low_retopology_policy",
    "algorithm",
    "canonical_sha256",
];

/// Validate and compile an explicit quad retopology draft.
///
/// This is intentionally a different operation from [`run`].  It does not
/// derive topology from a High mesh and it does not perform edge collapse.  A
/// caller must provide a complete, explicit `authoring-mesh@1` source with
/// only four-sided faces.  The Worker then performs a bounded structural
/// validation, lowers that source to a triangle GLB solely for strict
/// readback, and returns the original quad/edge-flow draft as the editable
/// representation.  The output is a draft, not an artist approval or a
/// commercial Low topology claim.
pub fn run_quad_draft(payload: &Map<String, Value>) -> Result<Value, GeometryError> {
    if payload
        .keys()
        .any(|field| !QUAD_DRAFT_REQUEST_FIELDS.contains(&field.as_str()))
        || payload.len() != QUAD_DRAFT_REQUEST_FIELDS.len()
        || payload.get("schema_version").and_then(Value::as_str)
            != Some(
                forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_REQUEST_SCHEMA_VERSION,
            )
        || payload.get("preview_only").and_then(Value::as_bool) != Some(true)
        || payload.get("low_retopology_policy").and_then(Value::as_str)
            != Some(forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY)
        || payload.get("algorithm").and_then(Value::as_str)
            != Some(forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM)
    {
        return Err(invalid("LOW_QUAD_DRAFT_REQUEST_INVALID"));
    }
    let canonical = required_sha(payload, "canonical_sha256")?;
    let mut preimage = payload.clone();
    preimage.remove("canonical_sha256");
    if crate::canonical_hash(&Value::Object(preimage)) != canonical {
        return Err(invalid("LOW_QUAD_DRAFT_REQUEST_CANONICAL_MISMATCH"));
    }

    let project_id = quad_required_id(payload, "project_id")?;
    let source_high_artifact_sha256 = required_sha(payload, "source_high_artifact_sha256")?;
    let source_high_artifact_readback_sha256 =
        required_sha(payload, "source_high_artifact_readback_sha256")?;
    let source_high_part_id = quad_required_id(payload, "source_high_part_id")?;
    let source_high_node_id = quad_required_id(payload, "source_high_node_id")?;
    let source_high_material_zone_id = quad_required_id(payload, "source_high_material_zone_id")?;
    let max_vertices = quad_budget(payload, "max_vertices")?;
    let max_edges = quad_budget(payload, "max_edges")?;
    let max_faces = quad_budget(payload, "max_faces")?;
    let draft = payload
        .get("draft")
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_DRAFT_MISSING"))?;
    let draft_object = draft
        .as_object()
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_DRAFT_INVALID"))?;
    if draft_object.len() != 3
        || draft_object.keys().any(|key| {
            !["schema_version", "source_lineage", "authoring_mesh"].contains(&key.as_str())
        })
        || draft_object.get("schema_version").and_then(Value::as_str)
            != Some(QUAD_DRAFT_SCHEMA_VERSION)
    {
        return Err(invalid("LOW_QUAD_DRAFT_DRAFT_SCHEMA_INVALID"));
    }
    let source_lineage = draft_object
        .get("source_lineage")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_SOURCE_LINEAGE_INVALID"))?;
    if source_lineage.len() != 5
        || source_lineage.keys().any(|key| {
            ![
                "source_high_artifact_sha256",
                "source_high_artifact_readback_sha256",
                "source_high_part_id",
                "source_high_node_id",
                "source_high_material_zone_id",
            ]
            .contains(&key.as_str())
        })
    {
        return Err(invalid("LOW_QUAD_DRAFT_SOURCE_LINEAGE_FIELDS_INVALID"));
    }
    if source_lineage
        .get("source_high_artifact_sha256")
        .and_then(Value::as_str)
        != Some(source_high_artifact_sha256.as_str())
        || source_lineage
            .get("source_high_artifact_readback_sha256")
            .and_then(Value::as_str)
            != Some(source_high_artifact_readback_sha256.as_str())
        || source_lineage
            .get("source_high_part_id")
            .and_then(Value::as_str)
            != Some(source_high_part_id.as_str())
        || source_lineage
            .get("source_high_node_id")
            .and_then(Value::as_str)
            != Some(source_high_node_id.as_str())
        || source_lineage
            .get("source_high_material_zone_id")
            .and_then(Value::as_str)
            != Some(source_high_material_zone_id.as_str())
    {
        return Err(invalid("LOW_QUAD_DRAFT_SOURCE_LINEAGE_MISMATCH"));
    }
    let source_lineage_value = Value::Object(source_lineage.clone());
    let source_lineage_sha256 = crate::canonical_hash(&source_lineage_value);
    let authoring_mesh = draft_object
        .get("authoring_mesh")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_AUTHORING_MESH_INVALID"))?;
    let authoring_mesh_value = Value::Object(authoring_mesh.clone());
    let boundary_edge_count = validate_quad_authoring_mesh(
        authoring_mesh,
        max_vertices,
        max_edges,
        max_faces,
        &source_high_part_id,
        &source_high_node_id,
    )?;
    let (edge_flow, quad_face_count) = quad_edge_flow(authoring_mesh)?;
    if quad_face_count == 0 {
        return Err(invalid("LOW_QUAD_DRAFT_NO_QUAD_FACES"));
    }
    let render_triangle_count = quad_face_count
        .checked_mul(2)
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_TRIANGLE_BUDGET_OVERFLOW"))?;
    if render_triangle_count > max_faces.saturating_mul(2) {
        return Err(invalid("LOW_QUAD_DRAFT_TRIANGLE_BUDGET_EXCEEDED"));
    }

    // Reuse the fixed authoring-mesh compiler as the only artifact producer.
    // The quad draft remains the source of truth; this GLB exists only to
    // prove that the explicit source can be consumed by the existing strict
    // renderer/readback path.
    let mut program = json!({
        "schema_version":"GeometryProgram@2",
        "project_id":project_id,
        "representation_plan_sha256":source_high_artifact_sha256,
        "operator_catalog_sha256":crate::operator_catalog_sha256(),
        "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
        "budgets":{
            "max_nodes":1,
            "max_triangles":render_triangle_count,
            "max_glb_bytes":64 * 1024 * 1024,
            "max_worker_memory_bytes":512 * 1024 * 1024,
            "max_runtime_ms":10_000
        },
        "nodes":[{
            "node_id":source_high_node_id,
            "operator_id":"forgecad.geometry.authoring-mesh@1",
            "inputs":[],
            "parameters":authoring_mesh_value
        }],
        "part_outputs":[{
            "part_id":source_high_part_id,
            "input_node_ids":[source_high_node_id],
            "material_zone_id":source_high_material_zone_id,
            "solid":boundary_edge_count == 0
        }],
        "canonical_sha256":""
    });
    let mut program_preimage = program
        .as_object_mut()
        .expect("quad draft program object")
        .clone();
    program_preimage.remove("canonical_sha256");
    program["canonical_sha256"] =
        Value::String(crate::canonical_hash(&Value::Object(program_preimage)));
    let artifact = crate::compile_geometry_program(&program)
        .map_err(|error| invalid(format!("LOW_QUAD_DRAFT_COMPILE_FAILED:{error}")))?;
    if artifact.triangle_count as usize != render_triangle_count {
        return Err(invalid("LOW_QUAD_DRAFT_COMPILE_TRIANGLE_COUNT_MISMATCH"));
    }
    let low_artifact_sha256 = sha256_hex(&artifact.glb);
    let readback = integrity::inspect_glb(&artifact.glb)?;
    if !readback.hard_gate_passed
        || readback.triangle_count as usize != render_triangle_count
        || readback.part_ids != [source_high_part_id.clone()]
        || readback.source_node_ids != [source_high_node_id.clone()]
        || readback.material_zone_ids != [source_high_material_zone_id.clone()]
    {
        return Err(invalid(format!(
            "LOW_QUAD_DRAFT_READBACK_FAILED: failures={:?} triangles={} parts={:?} sources={:?} zones={:?}",
            readback.failure_codes,
            readback.triangle_count,
            readback.part_ids,
            readback.source_node_ids,
            readback.material_zone_ids
        )));
    }
    let draft_sha256 = crate::canonical_hash(draft);
    let authoring_mesh_sha256 = crate::canonical_hash(&authoring_mesh_value);
    let edge_flow_sha256 = crate::canonical_hash(&edge_flow);
    let mut result = json!({
        "schema_version":forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_RESULT_SCHEMA_VERSION,
        "operation":forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION,
        "project_id":project_id,
        "source_high_artifact_sha256":source_high_artifact_sha256,
        "source_high_artifact_readback_sha256":source_high_artifact_readback_sha256,
        "source_high_part_id":source_high_part_id,
        "source_high_node_id":source_high_node_id,
        "source_high_material_zone_id":source_high_material_zone_id,
        "source_lineage":source_lineage_value,
        "source_lineage_sha256":source_lineage_sha256,
        "draft":draft,
        "draft_sha256":draft_sha256,
        "authoring_mesh_sha256":authoring_mesh_sha256,
        "edge_flow":edge_flow,
        "edge_flow_sha256":edge_flow_sha256,
        "low_quad_draft_artifact_sha256":low_artifact_sha256,
        "low_quad_draft_artifact_kind":"production-weapon-low-quad-draft-glb",
        "low_quad_draft_mime":"model/gltf-binary",
        "low_quad_draft_size_bytes":artifact.glb.len(),
        "low_quad_draft_glb_base64":base64::engine::general_purpose::STANDARD.encode(&artifact.glb),
        "low_quad_draft_readback":readback.report_value(),
        "low_geometry_program_sha256":artifact.program_sha256,
        "quad_face_count":quad_face_count,
        "render_triangle_count":render_triangle_count,
        "vertex_budget":max_vertices,
        "edge_budget":max_edges,
        "face_budget":max_faces,
        "low_retopology_policy":forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY,
        "algorithm":forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM,
        "algorithm_sha256":sha256_hex(forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM.as_bytes()),
        "explicit_quad_faces":true,
        "auto_retopology_performed":false,
        "retopology_derived":false,
        "artist_authored_quad_topology":false,
        "edge_flow_status":"DRAFT_UNREVIEWED",
        "quality_status":"structural_only",
        "validator_status":"passed",
        "hard_gate_passed":true,
        "runtime_write_performed":false,
        "production_stage_advanced":false,
        "promotion_eligible":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(wire_canonical_hash(&result)?);
    Ok(result)
}

fn quad_required_id(payload: &Map<String, Value>, field: &str) -> Result<String, GeometryError> {
    let value = payload
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
        .ok_or_else(|| invalid(format!("LOW_QUAD_DRAFT_{field}_INVALID")))?;
    Ok(value.to_owned())
}

fn quad_budget(payload: &Map<String, Value>, field: &str) -> Result<usize, GeometryError> {
    payload
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| (1..=8192).contains(value))
        .ok_or_else(|| invalid(format!("LOW_QUAD_DRAFT_{field}_INVALID")))
}

fn quad_object<'a>(
    value: &'a Value,
    fields: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, GeometryError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("LOW_QUAD_DRAFT_{label}_INVALID")))?;
    if object.len() != fields.len()
        || fields.iter().any(|field| !object.contains_key(*field))
        || object.keys().any(|field| !fields.contains(&field.as_str()))
    {
        return Err(invalid(format!("LOW_QUAD_DRAFT_{label}_FIELDS_INVALID")));
    }
    Ok(object)
}

fn quad_value_id(value: Option<&Value>, label: &str) -> Result<String, GeometryError> {
    let id = value
        .and_then(Value::as_str)
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
        })
        .ok_or_else(|| invalid(format!("LOW_QUAD_DRAFT_{label}_ID_INVALID")))?;
    Ok(id.to_owned())
}

#[derive(Debug, Clone)]
struct QuadLoopRecord {
    face_id: String,
    edge_id: String,
    vertex_id: String,
    ordinal: usize,
    edge_forward: bool,
}

fn validate_quad_authoring_mesh(
    parameters: &Map<String, Value>,
    max_vertices: usize,
    max_edges: usize,
    max_faces: usize,
    source_part_id: &str,
    source_node_id: &str,
) -> Result<usize, GeometryError> {
    let expected_fields = [
        "shape",
        "topology_policy",
        "vertices",
        "edges",
        "loops",
        "faces",
        "position_m",
        "rotation_rad",
    ];
    if parameters.len() != expected_fields.len()
        || expected_fields
            .iter()
            .any(|field| !parameters.contains_key(*field))
        || parameters
            .keys()
            .any(|field| !expected_fields.contains(&field.as_str()))
        || parameters.get("shape").and_then(Value::as_str) != Some("authoring-mesh")
        || parameters.get("topology_policy").and_then(Value::as_str)
            != Some("triangle-quad-manifold-with-boundary@1")
    {
        return Err(invalid("LOW_QUAD_DRAFT_AUTHORING_MESH_SCHEMA_INVALID"));
    }
    let vertices = parameters
        .get("vertices")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_VERTICES_INVALID"))?;
    let edges = parameters
        .get("edges")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_EDGES_INVALID"))?;
    let loops = parameters
        .get("loops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_LOOPS_INVALID"))?;
    let faces = parameters
        .get("faces")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_FACES_INVALID"))?;
    if vertices.is_empty()
        || edges.is_empty()
        || faces.is_empty()
        || vertices.len() > max_vertices
        || edges.len() > max_edges
        || faces.len() > max_faces
    {
        return Err(invalid("LOW_QUAD_DRAFT_TOPOLOGY_BUDGET_EXCEEDED"));
    }
    let vertex_ids = vertices
        .iter()
        .map(|value| {
            let object = quad_object(value, &["element_id", "position_m"], "vertex")?;
            quad_value_id(object.get("element_id"), "vertex")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let vertex_set = vertex_ids.iter().cloned().collect::<BTreeSet<_>>();
    if vertex_set.len() != vertex_ids.len() || vertex_ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid("LOW_QUAD_DRAFT_VERTEX_ORDER_INVALID"));
    }
    let edge_ids = edges
        .iter()
        .map(|value| {
            let object = quad_object(value, &["element_id", "vertex_ids"], "edge")?;
            let id = quad_value_id(object.get("element_id"), "edge")?;
            let endpoints = object
                .get("vertex_ids")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 2)
                .ok_or_else(|| invalid("LOW_QUAD_DRAFT_EDGE_ENDPOINTS_INVALID"))?;
            let first = quad_value_id(endpoints.first(), "edge_vertex")?;
            let second = quad_value_id(endpoints.get(1), "edge_vertex")?;
            if first >= second || !vertex_set.contains(&first) || !vertex_set.contains(&second) {
                return Err(invalid("LOW_QUAD_DRAFT_EDGE_ENDPOINTS_INVALID"));
            }
            Ok(id)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if edge_ids.len() != edge_ids.iter().collect::<BTreeSet<_>>().len()
        || edge_ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid("LOW_QUAD_DRAFT_EDGE_ORDER_INVALID"));
    }
    let edge_set = edge_ids.iter().cloned().collect::<BTreeSet<_>>();
    let mut loop_by_id = BTreeMap::<String, QuadLoopRecord>::new();
    for value in loops {
        let object = quad_object(
            value,
            &[
                "element_id",
                "face_id",
                "ordinal",
                "vertex_id",
                "edge_id",
                "edge_forward",
            ],
            "loop",
        )?;
        let id = quad_value_id(object.get("element_id"), "loop")?;
        let face_id = quad_value_id(object.get("face_id"), "loop_face")?;
        let vertex_id = quad_value_id(object.get("vertex_id"), "loop_vertex")?;
        let edge_id = quad_value_id(object.get("edge_id"), "loop_edge")?;
        let ordinal = object
            .get("ordinal")
            .and_then(Value::as_u64)
            .filter(|value| *value < 4)
            .map(|value| value as usize)
            .ok_or_else(|| invalid("LOW_QUAD_DRAFT_LOOP_ORDINAL_INVALID"))?;
        if !vertex_set.contains(&vertex_id) || !edge_set.contains(&edge_id) {
            return Err(invalid("LOW_QUAD_DRAFT_LOOP_REFERENCE_INVALID"));
        }
        if loop_by_id
            .insert(
                id,
                QuadLoopRecord {
                    face_id,
                    edge_id,
                    vertex_id,
                    ordinal,
                    edge_forward: object
                        .get("edge_forward")
                        .and_then(Value::as_bool)
                        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_LOOP_DIRECTION_INVALID"))?,
                },
            )
            .is_some()
        {
            return Err(invalid("LOW_QUAD_DRAFT_LOOP_DUPLICATE"));
        }
    }
    if loops.len() != faces.len().saturating_mul(4) {
        return Err(invalid("LOW_QUAD_DRAFT_LOOP_BUDGET_MISMATCH"));
    }
    let face_ids = faces
        .iter()
        .map(|value| {
            let object = quad_object(value, &["element_id", "loop_ids"], "face")?;
            let face_id = quad_value_id(object.get("element_id"), "face")?;
            let loop_ids = object
                .get("loop_ids")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 4)
                .ok_or_else(|| invalid("LOW_QUAD_DRAFT_FACE_NOT_QUAD"))?;
            if loop_ids
                .iter()
                .map(|value| quad_value_id(Some(value), "face_loop"))
                .collect::<Result<Vec<_>, _>>()?
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            {
                return Err(invalid("LOW_QUAD_DRAFT_FACE_LOOP_DUPLICATE"));
            }
            Ok((face_id, loop_ids.clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let face_id_set = face_ids
        .iter()
        .map(|(face_id, _)| face_id.clone())
        .collect::<BTreeSet<_>>();
    if face_id_set.len() != face_ids.len()
        || face_ids
            .iter()
            .map(|(face_id, _)| face_id.as_str())
            .collect::<Vec<_>>()
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid("LOW_QUAD_DRAFT_FACE_ORDER_INVALID"));
    }
    let mut used_loops = BTreeSet::new();
    let mut edge_incidence = BTreeMap::<String, Vec<(String, bool)>>::new();
    for (face_id, loop_values) in &face_ids {
        for (ordinal, loop_value) in loop_values.iter().enumerate() {
            let loop_id = quad_value_id(Some(loop_value), "face_loop")?;
            let current = loop_by_id
                .get(&loop_id)
                .ok_or_else(|| invalid("LOW_QUAD_DRAFT_FACE_LOOP_UNKNOWN"))?;
            let next_loop_id = quad_value_id(
                loop_values.get((ordinal + 1) % loop_values.len()),
                "face_loop",
            )?;
            let next = loop_by_id
                .get(&next_loop_id)
                .ok_or_else(|| invalid("LOW_QUAD_DRAFT_FACE_LOOP_UNKNOWN"))?;
            if current.face_id != *face_id
                || current.ordinal != ordinal
                || next.face_id != *face_id
                || !used_loops.insert(loop_id)
            {
                return Err(invalid("LOW_QUAD_DRAFT_FACE_LOOP_BINDING_INVALID"));
            }
            edge_incidence
                .entry(current.edge_id.clone())
                .or_default()
                .push((face_id.clone(), current.edge_forward));
            // The compiler performs the exact endpoint/winding and geometric
            // checks.  Keeping this pass ID-only ensures the edge-flow output
            // cannot be fabricated from a triangle index list.
            if next.vertex_id == current.vertex_id {
                return Err(invalid("LOW_QUAD_DRAFT_FACE_ZERO_LENGTH_EDGE"));
            }
        }
    }
    if used_loops.len() != loop_by_id.len() {
        return Err(invalid("LOW_QUAD_DRAFT_UNOWNED_LOOP"));
    }
    let mut boundary_edge_count = 0usize;
    for edge_id in &edge_ids {
        let incidence = edge_incidence
            .get(edge_id)
            .ok_or_else(|| invalid("LOW_QUAD_DRAFT_UNUSED_EDGE"))?;
        match incidence.as_slice() {
            [single] => {
                let _ = single;
                boundary_edge_count += 1;
            }
            [first, second] => {
                if first.0 == second.0 || first.1 == second.1 {
                    return Err(invalid("LOW_QUAD_DRAFT_NON_MANIFOLD_ORIENTATION"));
                }
            }
            _ => return Err(invalid("LOW_QUAD_DRAFT_NON_MANIFOLD_ORIENTATION")),
        }
    }
    if source_part_id.is_empty() || source_node_id.is_empty() {
        return Err(invalid("LOW_QUAD_DRAFT_SOURCE_BINDING_INVALID"));
    }
    Ok(boundary_edge_count)
}

fn quad_edge_flow(parameters: &Map<String, Value>) -> Result<(Value, usize), GeometryError> {
    let faces = parameters
        .get("faces")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_FACES_INVALID"))?;
    let loops = parameters
        .get("loops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("LOW_QUAD_DRAFT_LOOPS_INVALID"))?;
    let mut loop_by_id = BTreeMap::<String, QuadLoopRecord>::new();
    for value in loops {
        let object = quad_object(
            value,
            &[
                "element_id",
                "face_id",
                "ordinal",
                "vertex_id",
                "edge_id",
                "edge_forward",
            ],
            "flow_loop",
        )?;
        let id = quad_value_id(object.get("element_id"), "flow_loop")?;
        loop_by_id.insert(
            id,
            QuadLoopRecord {
                face_id: quad_value_id(object.get("face_id"), "flow_face")?,
                edge_id: quad_value_id(object.get("edge_id"), "flow_edge")?,
                vertex_id: quad_value_id(object.get("vertex_id"), "flow_vertex")?,
                ordinal: object
                    .get("ordinal")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid("LOW_QUAD_DRAFT_FLOW_ORDINAL_INVALID"))?
                    as usize,
                edge_forward: object
                    .get("edge_forward")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| invalid("LOW_QUAD_DRAFT_FLOW_DIRECTION_INVALID"))?,
            },
        );
    }
    let mut edge_faces = BTreeMap::<String, Vec<String>>::new();
    let mut vertex_face_valence = BTreeMap::<String, usize>::new();
    let mut flow_faces = Vec::with_capacity(faces.len());
    for value in faces {
        let object = quad_object(value, &["element_id", "loop_ids"], "flow_face")?;
        let face_id = quad_value_id(object.get("element_id"), "flow_face")?;
        let loop_ids = object
            .get("loop_ids")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 4)
            .ok_or_else(|| invalid("LOW_QUAD_DRAFT_FLOW_FACE_NOT_QUAD"))?;
        let mut edge_ids = Vec::with_capacity(4);
        let mut vertex_ids = Vec::with_capacity(4);
        for loop_id in loop_ids {
            let loop_id = quad_value_id(Some(loop_id), "flow_face_loop")?;
            let record = loop_by_id
                .get(&loop_id)
                .ok_or_else(|| invalid("LOW_QUAD_DRAFT_FLOW_LOOP_UNKNOWN"))?;
            edge_ids.push(record.edge_id.clone());
            vertex_ids.push(record.vertex_id.clone());
            edge_faces
                .entry(record.edge_id.clone())
                .or_default()
                .push(face_id.clone());
            *vertex_face_valence
                .entry(record.vertex_id.clone())
                .or_default() += 1;
        }
        flow_faces.push(json!({
            "face_id":face_id,
            "vertex_ids":vertex_ids,
            "edge_ids":edge_ids
        }));
    }
    let mut adjacent_faces = Vec::new();
    let mut boundary_edge_count = 0usize;
    for face in &flow_faces {
        let edge_ids = face
            .get("edge_ids")
            .and_then(Value::as_array)
            .expect("edge flow edge ids");
        let mut adjacent = Vec::with_capacity(edge_ids.len());
        for edge_id in edge_ids {
            let edge_id = edge_id.as_str().expect("edge ID string");
            let faces = edge_faces
                .get(edge_id)
                .ok_or_else(|| invalid("LOW_QUAD_DRAFT_FLOW_EDGE_UNKNOWN"))?;
            let face_id = face
                .get("face_id")
                .and_then(Value::as_str)
                .expect("face ID string");
            match faces.as_slice() {
                [single] => {
                    if single.as_str() != face_id {
                        return Err(invalid("LOW_QUAD_DRAFT_FLOW_EDGE_BINDING_INVALID"));
                    }
                    boundary_edge_count += 1;
                    adjacent.push(Value::Array(Vec::new()));
                }
                [first, second] => {
                    adjacent.push(Value::String(
                        [first, second]
                            .into_iter()
                            .find(|candidate| candidate.as_str() != face_id)
                            .expect("closed edge has an adjacent face")
                            .to_owned(),
                    ));
                }
                _ => return Err(invalid("LOW_QUAD_DRAFT_FLOW_EDGE_INCIDENT_INVALID")),
            }
        }
        adjacent_faces.push(json!({
            "face_id":face.get("face_id"),
            "vertex_ids":face.get("vertex_ids"),
            "edge_ids":face.get("edge_ids"),
            "adjacent_face_ids":adjacent
        }));
    }
    let mut valence_histogram = BTreeMap::<String, usize>::new();
    for valence in vertex_face_valence.values() {
        *valence_histogram.entry(valence.to_string()).or_default() += 1;
    }
    let mut histogram = Vec::new();
    for (valence, count) in valence_histogram {
        histogram.push(
            json!({"face_valence":valence.parse::<usize>().expect("valence"),"vertex_count":count}),
        );
    }
    let face_count = adjacent_faces.len();
    let mut edge_flow = json!({
        "schema_version":"LowQuadEdgeFlow@1",
        "policy":"explicit-quad-adjacency-only@1",
        "quad_faces":adjacent_faces,
        "quad_face_count":face_count,
        "edge_count":edge_faces.len(),
        "boundary_edge_count":boundary_edge_count,
        "non_manifold_edge_count":0,
        "vertex_face_valence_histogram":histogram,
        "status":"DRAFT_UNREVIEWED",
        "edge_flow_proven":false,
        "artist_review_status":"NOT_RUN",
        "canonical_sha256":""
    });
    let mut edge_flow_preimage = edge_flow.as_object_mut().expect("edge-flow object").clone();
    edge_flow_preimage.remove("canonical_sha256");
    edge_flow["canonical_sha256"] =
        Value::String(crate::canonical_hash(&Value::Object(edge_flow_preimage)));
    Ok((edge_flow, face_count))
}

/// Canonicalize the result after the same JSON wire round-trip used by the
/// isolated Worker envelope.  Some f32-origin numbers have a lexical form in
/// the in-memory `serde_json::Number` that is normalized when the parent
/// Runtime parses the response; hashing the wire projection keeps restart
/// verification deterministic across that process boundary.
fn wire_canonical_hash(value: &Value) -> Result<String, GeometryError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| invalid("LOW_RETOPOLOGY_RESULT_CANONICAL_SERIALIZE_FAILED"))?;
    let mut wire: Value = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("LOW_RETOPOLOGY_RESULT_CANONICAL_PARSE_FAILED"))?;
    wire["canonical_sha256"] = Value::String(String::new());
    Ok(crate::canonical_hash(&wire))
}

fn wire_value_hash(value: &Value) -> Result<String, GeometryError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| invalid("LOW_RETOPOLOGY_VALUE_CANONICAL_SERIALIZE_FAILED"))?;
    let wire: Value = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("LOW_RETOPOLOGY_VALUE_CANONICAL_PARSE_FAILED"))?;
    Ok(crate::canonical_hash(&wire))
}

fn lower_low_glb(
    mesh: &DerivedLowMesh,
    program_sha256: &str,
) -> Result<crate::GeometryArtifact, GeometryError> {
    let mut parts = Vec::<crate::PartMesh>::new();
    let primitive_count = mesh.primitives.len();
    let atlas_grid = (1usize..)
        .find(|value| value.saturating_mul(*value) >= primitive_count)
        .ok_or_else(|| invalid("LOW_RETOPOLOGY_UV_ATLAS_GRID_INVALID"))?;
    for (primitive_ordinal, primitive) in mesh.primitives.iter().enumerate() {
        let normals = logical_vertex_normals(&primitive.positions, &primitive.indices)?;
        let (positions, normals, mut uvs, tangents, indices, uv_chart_count, uv_chart_ids) =
            crate::triangulate_uv_charts(
                &primitive.positions,
                &normals,
                &primitive.indices,
                true,
                false,
            )?;
        // `triangulate_uv_charts` creates a non-overlapping chart set for one
        // primitive. Pack those local charts into a deterministic square grid
        // so independent Parts no longer all occupy the full 0..1 atlas.
        // The fixed two-percent tile inset leaves room for later 2K dilation;
        // scaling and translation preserve the admitted tangent orientation.
        let tile_x = primitive_ordinal % atlas_grid;
        let tile_y = primitive_ordinal / atlas_grid;
        let grid = atlas_grid as f32;
        const TILE_INSET: f32 = 0.02;
        for uv in &mut uvs {
            uv[0] = (tile_x as f32 + TILE_INSET + uv[0] * (1.0 - 2.0 * TILE_INSET)) / grid;
            uv[1] = (tile_y as f32 + TILE_INSET + uv[1] * (1.0 - 2.0 * TILE_INSET)) / grid;
        }
        let source = crate::PartSourceMesh {
            source_node_id: primitive.source_node_id.clone(),
            operator_id: "forgecad.worker.low-retopology@1".to_owned(),
            lineage_source_node_ids: vec![primitive.source_node_id.clone()],
            positions,
            normals,
            uvs,
            tangents,
            indices,
            uv_chart_count,
            uv_chart_ids,
        };
        if let Some(part) = parts
            .iter_mut()
            .find(|part| part.part_id == primitive.part_id)
        {
            if part.material_zone_id != primitive.material_zone_id || part.solid != primitive.solid
            {
                return Err(invalid("LOW_RETOPOLOGY_PART_BINDING_MISMATCH"));
            }
            part.sources.push(source);
        } else {
            parts.push(crate::PartMesh {
                part_id: primitive.part_id.clone(),
                material_zone_id: primitive.material_zone_id.clone(),
                solid: primitive.solid,
                sources: vec![source],
                material: crate::material_for_zone(&primitive.material_zone_id),
            });
        }
    }
    let triangle_count = parts
        .iter()
        .flat_map(|part| &part.sources)
        .map(|source| source.indices.len() as u64 / 3)
        .sum::<u64>();
    if triangle_count as usize != mesh.low_triangle_count {
        return Err(invalid("LOW_RETOPOLOGY_LOWERING_COUNT_MISMATCH"));
    }
    let glb = crate::write_glb(
        &parts,
        program_sha256,
        triangle_count,
        "ArtifactReadback@2",
        Some(&crate::operator_catalog_sha256()),
        None,
    )?;
    Ok(crate::GeometryArtifact {
        glb,
        part_ids: crate::ordered_unique_part_ids(&parts),
        triangle_count,
        program_sha256: program_sha256.to_owned(),
        uv_status: "passed".to_owned(),
        tangent_status: "passed".to_owned(),
        material_zone_ids: crate::ordered_unique_material_zone_ids(&parts),
    })
}

fn logical_vertex_normals(
    positions: &[[f32; 3]],
    indices: &[u32],
) -> Result<Vec<[f32; 3]>, GeometryError> {
    let mut sums = vec![[0.0f64; 3]; positions.len()];
    for triangle in indices.chunks_exact(3) {
        let a = to64(positions[triangle[0] as usize]);
        let b = to64(positions[triangle[1] as usize]);
        let c = to64(positions[triangle[2] as usize]);
        let normal = cross64(sub64(b, a), sub64(c, a));
        if length64(normal) <= AREA_EPSILON {
            return Err(invalid("LOW_RETOPOLOGY_LOWERING_DEGENERATE_FACE"));
        }
        for index in triangle {
            let sum = &mut sums[*index as usize];
            *sum = [sum[0] + normal[0], sum[1] + normal[1], sum[2] + normal[2]];
        }
    }
    sums.into_iter()
        .map(|sum| {
            let length = length64(sum);
            if length <= AREA_EPSILON || !length.is_finite() {
                return Err(invalid("LOW_RETOPOLOGY_LOWERING_NORMAL_INVALID"));
            }
            Ok([
                (sum[0] / length) as f32,
                (sum[1] / length) as f32,
                (sum[2] / length) as f32,
            ])
        })
        .collect()
}

fn low_mesh_value(mesh: &DerivedLowMesh) -> Value {
    // `lower_low_glb` deliberately expands every triangle corner so hard
    // normals and UV seams remain explicit in the exported Low.  The durable
    // correspondence must bind that exported topology, not the compact
    // logical mesh that existed immediately after edge collapse.  Expanding
    // the mapping here in the same deterministic face/corner order keeps the
    // Low GLB, Cage input and later bake rays on one exact vertex/index truth.
    Value::Array(
        mesh.primitives
            .iter()
            .map(|primitive| {
                let mut positions = Vec::with_capacity(primitive.indices.len());
                let mut indices = Vec::with_capacity(primitive.indices.len());
                let mut vertex_correspondence = Vec::with_capacity(primitive.indices.len());
                for logical_index in &primitive.indices {
                    let logical_index = *logical_index as usize;
                    let expanded_index = positions.len() as u32;
                    positions.push(primitive.positions[logical_index]);
                    indices.push(expanded_index);
                    vertex_correspondence.push(json!({
                        "low_vertex_index":expanded_index,
                        "source_vertex_indices":primitive.vertex_correspondence[logical_index].source_vertex_indices
                    }));
                }
                json!({
                    "part_id":primitive.part_id,
                    "source_node_id":primitive.source_node_id,
                    "material_zone_id":primitive.material_zone_id,
                    "solid":primitive.solid,
                    "positions":positions,
                    "indices":indices,
                    "vertex_correspondence":vertex_correspondence,
                    "face_correspondence":primitive.face_correspondence.iter().map(|entry| json!({
                        "low_face_index":entry.low_face_index,
                        "source_face_index":entry.source_face_index
                    })).collect::<Vec<_>>()
                })
            })
            .collect(),
    )
}

fn parse_locked_vertices(
    payload: &Map<String, Value>,
) -> Result<BTreeSet<(usize, u32)>, GeometryError> {
    let values = payload
        .get("locked_vertices")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("LOW_RETOPOLOGY_LOCKED_VERTICES_INVALID"))?;
    if values.len() > 16_384 {
        return Err(invalid("LOW_RETOPOLOGY_LOCKED_VERTICES_TOO_LARGE"));
    }
    let mut result = BTreeSet::new();
    for value in values {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("LOW_RETOPOLOGY_LOCKED_VERTEX_INVALID"))?;
        if object.len() != 2
            || !object.contains_key("primitive_ordinal")
            || !object.contains_key("vertex_index")
        {
            return Err(invalid("LOW_RETOPOLOGY_LOCKED_VERTEX_INVALID"));
        }
        let primitive = object
            .get("primitive_ordinal")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| invalid("LOW_RETOPOLOGY_LOCKED_VERTEX_INVALID"))?;
        let vertex = object
            .get("vertex_index")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| invalid("LOW_RETOPOLOGY_LOCKED_VERTEX_INVALID"))?;
        if !result.insert((primitive, vertex)) {
            return Err(invalid("LOW_RETOPOLOGY_LOCKED_VERTEX_DUPLICATE"));
        }
    }
    Ok(result)
}

fn required_sha(payload: &Map<String, Value>, field: &str) -> Result<String, GeometryError> {
    let value = payload
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("LOW_RETOPOLOGY_HASH_INVALID"))?;
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(invalid("LOW_RETOPOLOGY_HASH_INVALID"));
    }
    Ok(value.to_owned())
}

fn required_usize(payload: &Map<String, Value>, field: &str) -> Result<usize, GeometryError> {
    payload
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0 && *value <= 1_000_000)
        .ok_or_else(|| invalid("LOW_RETOPOLOGY_BUDGET_INVALID"))
}

#[derive(Debug, Clone, PartialEq)]
struct CornerAttributes {
    /// Retained from the strict source projection for auditability. UV-only
    /// differences are intentionally not a lock under the current Low wire
    /// contract; see `corner_attributes_differ`.
    uv: [f32; 2],
    normal: [f32; 3],
    tangent: [f32; 4],
}

#[derive(Debug, Clone)]
struct EdgeAttributeRecord {
    source_vertices: (u32, u32),
    attributes: [CornerAttributes; 2],
}

type SemanticPrimitiveKey = (String, String, String, bool);

/// Derive split-normal/tangent seam edges from the strict topology projection.
///
/// `extract_diagnostic_mesh` intentionally exposes only positions and indices,
/// because it is not an authoring topology. The Worker can nevertheless keep
/// seams safe without widening that shared projection: the ordered topology
/// view is used only here to compare corner attributes, and the resulting
/// source-edge locks are passed to the pure kernel as an internal invariant.
fn derive_attribute_seam_edges(
    glb: &[u8],
    diagnostic: &DiagnosticMesh,
) -> Result<BTreeSet<(usize, u32, u32)>, GeometryError> {
    let topology = integrity::extract_topology_mesh(glb, 250_000)?;
    let mut grouped =
        BTreeMap::<SemanticPrimitiveKey, Vec<integrity::TopologyTriangleSource>>::new();
    for triangle in topology.triangles {
        grouped
            .entry((
                triangle.part_id.clone(),
                triangle.source_node_id.clone(),
                triangle.material_zone_id.clone(),
                triangle.solid,
            ))
            .or_default()
            .push(triangle);
    }
    let mut cursors = BTreeMap::<SemanticPrimitiveKey, usize>::new();
    let mut edge_attributes =
        BTreeMap::<(usize, [i64; 3], [i64; 3]), Vec<EdgeAttributeRecord>>::new();

    for (primitive_ordinal, primitive) in diagnostic.primitives.iter().enumerate() {
        let key = (
            primitive.part_id.clone(),
            primitive.source_node_id.clone(),
            primitive.material_zone_id.clone(),
            primitive.solid,
        );
        let triangles = grouped
            .get(&key)
            .ok_or_else(|| invalid("LOW_RETOPOLOGY_TOPOLOGY_LINEAGE_MISSING"))?;
        let cursor = cursors.entry(key.clone()).or_default();
        let triangle_count = primitive.indices.len() / 3;
        let end = cursor
            .checked_add(triangle_count)
            .ok_or_else(|| invalid("LOW_RETOPOLOGY_TOPOLOGY_LINEAGE_OVERFLOW"))?;
        let source_triangles = triangles
            .get(*cursor..end)
            .ok_or_else(|| invalid("LOW_RETOPOLOGY_TOPOLOGY_FACE_COUNT_MISMATCH"))?;

        for (face, topology_triangle) in primitive
            .indices
            .chunks_exact(3)
            .zip(source_triangles.iter())
        {
            let source_indices = face
                .iter()
                .map(|source_index| *source_index)
                .collect::<Vec<_>>();
            let topology_corners = topology_triangle.corners.clone();
            let mut matched = [0u32; 3];
            let mut used = BTreeSet::new();
            for (corner_index, corner) in topology_corners.iter().enumerate() {
                let source_index = source_indices
                    .iter()
                    .copied()
                    .find(|source_index| {
                        !used.contains(source_index)
                            && primitive.positions[*source_index as usize] == corner.position
                    })
                    .ok_or_else(|| invalid("LOW_RETOPOLOGY_TOPOLOGY_POSITION_MISMATCH"))?;
                used.insert(source_index);
                matched[corner_index] = source_index;
            }
            for (edge_index_a, edge_index_b) in [(0usize, 1usize), (1, 2), (2, 0)] {
                let source_a = matched[edge_index_a];
                let source_b = matched[edge_index_b];
                let position_a = weld_position_key(primitive.positions[source_a as usize]);
                let position_b = weld_position_key(primitive.positions[source_b as usize]);
                let attributes_a = corner_attributes(&topology_corners[edge_index_a]);
                let attributes_b = corner_attributes(&topology_corners[edge_index_b]);
                let (key, source_vertices, attributes) = if position_a <= position_b {
                    (
                        (primitive_ordinal, position_a, position_b),
                        (source_a, source_b),
                        [attributes_a, attributes_b],
                    )
                } else {
                    (
                        (primitive_ordinal, position_b, position_a),
                        (source_b, source_a),
                        [attributes_b, attributes_a],
                    )
                };
                edge_attributes
                    .entry(key)
                    .or_default()
                    .push(EdgeAttributeRecord {
                        source_vertices,
                        attributes,
                    });
            }
        }
        *cursor = end;
        if *cursor > triangles.len() {
            return Err(invalid("LOW_RETOPOLOGY_TOPOLOGY_FACE_COUNT_MISMATCH"));
        }
    }

    if cursors.iter().any(|(key, cursor)| {
        grouped
            .get(key)
            .is_some_and(|triangles| cursor != &triangles.len())
    }) {
        return Err(invalid("LOW_RETOPOLOGY_TOPOLOGY_PRIMITIVE_COUNT_MISMATCH"));
    }

    let mut protected = BTreeSet::new();
    for ((primitive_ordinal, _, _), records) in &edge_attributes {
        for pair in records.windows(2) {
            if corner_attributes_differ(&pair[0].attributes[0], &pair[1].attributes[0])
                || corner_attributes_differ(&pair[0].attributes[1], &pair[1].attributes[1])
            {
                let first = pair[0].source_vertices;
                let second = pair[1].source_vertices;
                protected.insert((
                    *primitive_ordinal,
                    first.0.min(first.1),
                    first.0.max(first.1),
                ));
                protected.insert((
                    *primitive_ordinal,
                    second.0.min(second.1),
                    second.0.max(second.1),
                ));
            }
        }
    }
    Ok(protected)
}

fn corner_attributes(corner: &integrity::TopologyCornerSource) -> CornerAttributes {
    CornerAttributes {
        uv: corner.texcoord_0,
        normal: corner.normal,
        tangent: corner.tangent,
    }
}

fn corner_attributes_differ(first: &CornerAttributes, second: &CornerAttributes) -> bool {
    // UV-only differences are intentionally not promoted to hard locks here.
    // The current High artifacts may contain one chart per source triangle;
    // treating every such chart boundary as immutable would make a planar
    // diagonal uncollapsible. UV layout remains a later Hero-UV contract.
    dot64(to64(first.normal), to64(second.normal)) < 0.9999
        || first
            .tangent
            .iter()
            .zip(second.tangent.iter())
            .any(|(a, b)| (f64::from(*a) - f64::from(*b)).abs() > 1.0e-4)
}

fn weld_position_key(position: [f32; 3]) -> [i64; 3] {
    position.map(|component| (f64::from(component) * 1_000_000.0).round() as i64)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone)]
struct WorkingMesh {
    positions: Vec<[f32; 3]>,
    faces: Vec<[u32; 3]>,
    face_sources: Vec<u32>,
    vertex_sources: Vec<BTreeSet<u32>>,
    locked: BTreeSet<u32>,
    /// Split-normal/tangent seam edges derived from the source GLB. Part
    /// boundaries are already isolated by primitive; hard geometric edges are
    /// computed per collapse pass and excluded from candidates.
    protected_edges: BTreeSet<(u32, u32)>,
}

/// Derive a strictly smaller Low mesh.
///
/// Reduction is enforced for the complete assembly, not for every primitive.
/// A hard-surface assembly routinely contains already-minimal closed parts
/// (boxes, caps, short rail blocks) whose every edge is a protected crease.
/// Requiring each such Part to lose a face makes an otherwise reducible weapon
/// fail before the Worker reaches its denser panels. Minimal Parts therefore
/// pass through byte-deterministically; the final assembly must still be
/// strictly smaller and must reach the caller's global triangle target.
pub fn derive_bounded_low(
    high: &DiagnosticMesh,
    policy: &LowRetopologyPolicy,
) -> Result<DerivedLowMesh, GeometryError> {
    if high.primitives.is_empty()
        || high.triangle_count == 0
        || policy.target_triangle_count == 0
        || policy.target_triangle_count >= high.triangle_count
        || policy.max_collapses == 0
    {
        return Err(invalid("LOW_RETOPOLOGY_POLICY_INVALID"));
    }

    let mut primitives = Vec::with_capacity(high.primitives.len());
    let mut remaining_target = policy.target_triangle_count;
    let mut remaining_source = high.triangle_count;
    let mut collapse_budget = policy.max_collapses;

    for (primitive_ordinal, primitive) in high.primitives.iter().enumerate() {
        let source_triangles = primitive.indices.len() / 3;
        if source_triangles < 4 || !primitive.solid {
            return Err(invalid("LOW_RETOPOLOGY_REQUIRES_CLOSED_SOLID_PARTS"));
        }
        let minimum = 4usize;
        let proportional = if remaining_source == 0 {
            minimum
        } else {
            ((remaining_target as f64 * source_triangles as f64 / remaining_source as f64).round()
                as usize)
                .clamp(minimum, source_triangles.saturating_sub(1))
        };
        let primitive_protected_edges = policy
            .protected_edges
            .iter()
            .filter_map(|(ordinal, first, second)| {
                (*ordinal == primitive_ordinal).then_some((*first, *second))
            })
            .collect::<BTreeSet<_>>();
        let mut working = weld_primitive(
            primitive,
            primitive_ordinal,
            &policy.locked_vertices,
            &primitive_protected_edges,
        )?;
        validate_closed_manifold(&working)?;
        while working.faces.len() > proportional && collapse_budget > 0 {
            let Some(next) = best_safe_collapse(&working)? else {
                break;
            };
            working = next;
            collapse_budget -= 1;
        }
        remaining_target = remaining_target.saturating_sub(working.faces.len());
        remaining_source = remaining_source.saturating_sub(source_triangles);
        primitives.push(finish_primitive(primitive, working));
    }

    let low_triangle_count = primitives
        .iter()
        .map(|primitive| primitive.indices.len() / 3)
        .sum::<usize>();
    if low_triangle_count == 0
        || low_triangle_count >= high.triangle_count
        || low_triangle_count > policy.target_triangle_count
    {
        return Err(invalid("LOW_RETOPOLOGY_TARGET_NOT_REACHED"));
    }
    Ok(DerivedLowMesh {
        primitives,
        source_triangle_count: high.triangle_count,
        low_triangle_count,
        retopology_derived: true,
        artist_authored_quad_topology: false,
        edge_flow_status: "NOT_PROVEN".to_owned(),
        quality_status: "structural_only".to_owned(),
        production_stage_advanced: false,
        promotion_eligible: false,
    })
}

fn weld_primitive(
    primitive: &DiagnosticPrimitive,
    primitive_ordinal: usize,
    locked_vertices: &BTreeSet<(usize, u32)>,
    protected_source_edges: &BTreeSet<(u32, u32)>,
) -> Result<WorkingMesh, GeometryError> {
    if primitive.indices.is_empty() || primitive.indices.len() % 3 != 0 {
        return Err(invalid("LOW_RETOPOLOGY_INDEX_PAYLOAD_INVALID"));
    }
    // Match the strict GLB readback's one-micrometer positional weld. Exact
    // f32-bit identity can treat transform-rounding duplicates as separate
    // vertices, then create winding conflicts after an otherwise safe edge
    // collapse is lowered back to GLB.
    let mut welded_by_position = BTreeMap::<[i64; 3], u32>::new();
    let mut positions = Vec::new();
    let mut vertex_sources = Vec::<BTreeSet<u32>>::new();
    let mut source_to_welded = vec![0u32; primitive.positions.len()];
    let mut locked = BTreeSet::new();
    for (source_index, position) in primitive.positions.iter().copied().enumerate() {
        if position.iter().any(|value| !value.is_finite()) {
            return Err(invalid("LOW_RETOPOLOGY_NON_FINITE_POSITION"));
        }
        let key = position.map(|component| (f64::from(component) * 1_000_000.0).round() as i64);
        let welded = if let Some(index) = welded_by_position.get(&key) {
            *index
        } else {
            let index = positions.len() as u32;
            welded_by_position.insert(key, index);
            positions.push(position);
            vertex_sources.push(BTreeSet::new());
            index
        };
        source_to_welded[source_index] = welded;
        vertex_sources[welded as usize].insert(source_index as u32);
        if locked_vertices.contains(&(primitive_ordinal, source_index as u32)) {
            locked.insert(welded);
        }
    }
    let protected_edges = protected_source_edges
        .iter()
        .filter_map(|(first, second)| {
            if *first as usize >= source_to_welded.len()
                || *second as usize >= source_to_welded.len()
            {
                return None;
            }
            let first = source_to_welded[*first as usize];
            let second = source_to_welded[*second as usize];
            (first != second).then_some(if first < second {
                (first, second)
            } else {
                (second, first)
            })
        })
        .collect();
    let mut faces = Vec::new();
    let mut face_sources = Vec::new();
    for (face_index, triangle) in primitive.indices.chunks_exact(3).enumerate() {
        if triangle
            .iter()
            .any(|index| *index as usize >= source_to_welded.len())
        {
            return Err(invalid("LOW_RETOPOLOGY_INDEX_OUT_OF_RANGE"));
        }
        let face = [
            source_to_welded[triangle[0] as usize],
            source_to_welded[triangle[1] as usize],
            source_to_welded[triangle[2] as usize],
        ];
        if has_duplicate_vertex(face) || triangle_area(&positions, face) <= AREA_EPSILON {
            return Err(invalid("LOW_RETOPOLOGY_DEGENERATE_SOURCE_FACE"));
        }
        faces.push(face);
        face_sources.push(face_index as u32);
    }
    Ok(WorkingMesh {
        positions,
        faces,
        face_sources,
        vertex_sources,
        locked,
        protected_edges,
    })
}

fn best_safe_collapse(mesh: &WorkingMesh) -> Result<Option<WorkingMesh>, GeometryError> {
    Ok(best_safe_collapse_with_edge(mesh)?.map(|(_, candidate)| candidate))
}

fn best_safe_collapse_with_edge(
    mesh: &WorkingMesh,
) -> Result<Option<((u32, u32), WorkingMesh)>, GeometryError> {
    const HARD_EDGE_COSINE_THRESHOLD: f64 = 0.7071067811865476;
    let hard_edges = geometric_hard_edges(mesh, HARD_EDGE_COSINE_THRESHOLD);
    let mut candidates = BTreeSet::<(u64, u32, u32)>::new();
    for face in &mesh.faces {
        for [a, b] in [[face[0], face[1]], [face[1], face[2]], [face[2], face[0]]] {
            let (a, b) = if a < b { (a, b) } else { (b, a) };
            if mesh.locked.contains(&a)
                || mesh.locked.contains(&b)
                || mesh.protected_edges.contains(&(a, b))
                || hard_edges.contains(&(a, b))
            {
                continue;
            }
            candidates.insert((
                edge_cost_bits(mesh.positions[a as usize], mesh.positions[b as usize]),
                a,
                b,
            ));
        }
    }
    // A geometric crease is a protected edge, not merely a cost penalty. If
    // only creases remain, returning None makes the bounded policy fail closed
    // instead of silently destroying a commercial hard-surface boundary.
    for (_, keep, remove) in candidates {
        let mut candidate = mesh.clone();
        let midpoint = mul3(
            add3(
                candidate.positions[keep as usize],
                candidate.positions[remove as usize],
            ),
            0.5,
        );
        candidate.positions[keep as usize] = midpoint;
        let removed_sources = candidate.vertex_sources[remove as usize].clone();
        candidate.vertex_sources[keep as usize].extend(removed_sources);
        let mut faces = Vec::new();
        let mut face_sources = Vec::new();
        let mut unique = BTreeSet::new();
        for (face, source) in candidate
            .faces
            .iter()
            .copied()
            .zip(candidate.face_sources.iter().copied())
        {
            let mapped = [
                if face[0] == remove { keep } else { face[0] },
                if face[1] == remove { keep } else { face[1] },
                if face[2] == remove { keep } else { face[2] },
            ];
            if has_duplicate_vertex(mapped) {
                continue;
            }
            if triangle_area(&candidate.positions, mapped) <= AREA_EPSILON {
                continue;
            }
            let mut key = mapped;
            key.sort_unstable();
            if unique.insert(key) {
                faces.push(mapped);
                face_sources.push(source);
            }
        }
        candidate.faces = faces;
        candidate.face_sources = face_sources;
        if candidate.faces.len() >= mesh.faces.len() || candidate.faces.len() < 4 {
            continue;
        }
        if validate_closed_manifold(&candidate).is_ok() {
            return Ok(Some(((keep, remove), candidate)));
        }
    }
    Ok(None)
}

fn geometric_hard_edges(mesh: &WorkingMesh, cosine_threshold: f64) -> BTreeSet<(u32, u32)> {
    let mut edge_normals = BTreeMap::<(u32, u32), Vec<[f64; 3]>>::new();
    for face in &mesh.faces {
        let a = to64(mesh.positions[face[0] as usize]);
        let b = to64(mesh.positions[face[1] as usize]);
        let c = to64(mesh.positions[face[2] as usize]);
        let normal = cross64(sub64(b, a), sub64(c, a));
        let length = length64(normal);
        if length <= AREA_EPSILON || !length.is_finite() {
            continue;
        }
        let normal = [normal[0] / length, normal[1] / length, normal[2] / length];
        for [first, second] in [[face[0], face[1]], [face[1], face[2]], [face[2], face[0]]] {
            let edge = if first < second {
                (first, second)
            } else {
                (second, first)
            };
            edge_normals.entry(edge).or_default().push(normal);
        }
    }
    edge_normals
        .into_iter()
        .filter_map(|(edge, normals)| {
            (normals.len() == 2 && dot64(normals[0], normals[1]) < cosine_threshold).then_some(edge)
        })
        .collect()
}

fn validate_closed_manifold(mesh: &WorkingMesh) -> Result<(), GeometryError> {
    let mut welded_positions = BTreeMap::<[i64; 3], u32>::new();
    for vertex in mesh.faces.iter().flatten().copied() {
        let key = mesh.positions[vertex as usize]
            .map(|component| (f64::from(component) * 1_000_000.0).round() as i64);
        if welded_positions
            .insert(key, vertex)
            .is_some_and(|existing| existing != vertex)
        {
            return Err(invalid("LOW_RETOPOLOGY_SPATIAL_VERTEX_COLLISION"));
        }
    }
    let mut directed = BTreeMap::<(u32, u32), usize>::new();
    let mut undirected = BTreeMap::<(u32, u32), usize>::new();
    for face in &mesh.faces {
        if has_duplicate_vertex(*face) || triangle_area(&mesh.positions, *face) <= AREA_EPSILON {
            return Err(invalid("LOW_RETOPOLOGY_DEGENERATE_FACE"));
        }
        for [a, b] in [[face[0], face[1]], [face[1], face[2]], [face[2], face[0]]] {
            *directed.entry((a, b)).or_default() += 1;
            let edge = if a < b { (a, b) } else { (b, a) };
            *undirected.entry(edge).or_default() += 1;
        }
    }
    if undirected.values().any(|count| *count != 2)
        || directed
            .iter()
            .any(|(&(a, b), count)| *count != 1 || directed.get(&(b, a)) != Some(&1))
        || signed_volume(mesh).abs() <= VOLUME_EPSILON
    {
        return Err(invalid("LOW_RETOPOLOGY_MANIFOLD_VALIDATION_FAILED"));
    }
    Ok(())
}

fn finish_primitive(source: &DiagnosticPrimitive, mesh: WorkingMesh) -> DerivedLowPrimitive {
    let mut used = BTreeSet::<u32>::new();
    for face in &mesh.faces {
        used.extend(face);
    }
    let mut remap = BTreeMap::new();
    let mut positions = Vec::new();
    let mut vertex_correspondence = Vec::new();
    for old in used {
        let new = positions.len() as u32;
        remap.insert(old, new);
        positions.push(mesh.positions[old as usize]);
        vertex_correspondence.push(LowVertexCorrespondence {
            low_vertex_index: new,
            source_vertex_indices: mesh.vertex_sources[old as usize].iter().copied().collect(),
        });
    }
    let mut indices = Vec::with_capacity(mesh.faces.len() * 3);
    let mut face_correspondence = Vec::with_capacity(mesh.faces.len());
    for (low_face_index, (face, source_face_index)) in
        mesh.faces.iter().zip(mesh.face_sources.iter()).enumerate()
    {
        indices.extend([remap[&face[0]], remap[&face[1]], remap[&face[2]]]);
        face_correspondence.push(LowFaceCorrespondence {
            low_face_index: low_face_index as u32,
            source_face_index: *source_face_index,
        });
    }
    DerivedLowPrimitive {
        part_id: source.part_id.clone(),
        source_node_id: source.source_node_id.clone(),
        material_zone_id: source.material_zone_id.clone(),
        solid: source.solid,
        positions,
        indices,
        vertex_correspondence,
        face_correspondence,
    }
}

fn signed_volume(mesh: &WorkingMesh) -> f64 {
    mesh.faces
        .iter()
        .map(|face| {
            let a = to64(mesh.positions[face[0] as usize]);
            let b = to64(mesh.positions[face[1] as usize]);
            let c = to64(mesh.positions[face[2] as usize]);
            dot64(a, cross64(b, c)) / 6.0
        })
        .sum()
}

fn triangle_area(positions: &[[f32; 3]], face: [u32; 3]) -> f64 {
    let a = to64(positions[face[0] as usize]);
    let b = to64(positions[face[1] as usize]);
    let c = to64(positions[face[2] as usize]);
    length64(cross64(sub64(b, a), sub64(c, a))) * 0.5
}

fn has_duplicate_vertex(face: [u32; 3]) -> bool {
    face[0] == face[1] || face[1] == face[2] || face[2] == face[0]
}

fn edge_cost_bits(a: [f32; 3], b: [f32; 3]) -> u64 {
    let delta = sub64(to64(a), to64(b));
    dot64(delta, delta).to_bits()
}

fn to64(value: [f32; 3]) -> [f64; 3] {
    [value[0] as f64, value[1] as f64, value[2] as f64]
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mul3(a: [f32; 3], scalar: f32) -> [f32; 3] {
    [a[0] * scalar, a[1] * scalar, a[2] * scalar]
}

fn sub64(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot64(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross64(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length64(value: [f64; 3]) -> f64 {
    dot64(value, value).sqrt()
}

fn invalid(code: impl Into<String>) -> GeometryError {
    GeometryError::Invalid(code.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use serde_json::{json, Value};

    fn octahedron() -> DiagnosticMesh {
        let positions = vec![
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ];
        let indices = vec![
            0, 2, 4, 2, 1, 4, 1, 3, 4, 3, 0, 4, 2, 0, 5, 1, 2, 5, 3, 1, 5, 0, 3, 5,
        ];
        DiagnosticMesh {
            primitives: vec![DiagnosticPrimitive {
                part_id: "body".to_owned(),
                source_node_id: "body-node".to_owned(),
                material_zone_id: "zone-shell".to_owned(),
                solid: true,
                positions,
                indices,
            }],
            triangle_count: 8,
        }
    }

    fn box_artifact() -> crate::GeometryArtifact {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":"low-retopology-worker-fixture",
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":crate::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":2,"max_triangles":100,"max_glb_bytes":4194304,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[{"node_id":"body-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[2.0,2.0,2.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}],
            "part_outputs":[{"part_id":"body","input_node_ids":["body-node"],"material_zone_id":"zone-shell","solid":true}]
        });
        let preimage = program.as_object().unwrap().clone();
        program["canonical_sha256"] =
            Value::String(crate::canonical_hash(&Value::Object(preimage)));
        crate::compile_geometry_program(&program).unwrap()
    }

    fn box_mesh() -> DiagnosticMesh {
        let artifact = box_artifact();
        integrity::extract_diagnostic_mesh(&artifact.glb, 100).unwrap()
    }

    #[test]
    fn derives_smaller_closed_topology_with_correspondence() {
        let mesh = box_mesh();
        let result = derive_bounded_low(
            &mesh,
            &LowRetopologyPolicy {
                target_triangle_count: 10,
                max_collapses: 4,
                locked_vertices: BTreeSet::new(),
                protected_edges: BTreeSet::new(),
            },
        )
        .expect("safe collapse");
        assert!(result.low_triangle_count < result.source_triangle_count);
        assert_eq!(result.low_triangle_count, 10);
        assert!(result.retopology_derived);
        assert!(!result.artist_authored_quad_topology);
        assert!(!result.production_stage_advanced);
        assert!(!result.promotion_eligible);
        assert_eq!(result.primitives[0].face_correspondence.len(), 10);
    }

    #[test]
    fn replay_is_deterministic_and_locked_mesh_fails_closed() {
        let mesh = box_mesh();
        let policy = LowRetopologyPolicy {
            target_triangle_count: 10,
            max_collapses: 4,
            locked_vertices: BTreeSet::new(),
            protected_edges: BTreeSet::new(),
        };
        assert_eq!(
            derive_bounded_low(&mesh, &policy).unwrap(),
            derive_bounded_low(&mesh, &policy).unwrap()
        );

        let locked = LowRetopologyPolicy {
            target_triangle_count: 10,
            max_collapses: 4,
            locked_vertices: (0..mesh.primitives[0].positions.len() as u32)
                .map(|index| (0, index))
                .collect(),
            protected_edges: BTreeSet::new(),
        };
        assert!(derive_bounded_low(&mesh, &locked)
            .unwrap_err()
            .to_string()
            .contains("LOW_RETOPOLOGY_NO_SAFE_COLLAPSE"));
    }

    #[test]
    fn hard_edges_are_protected_when_a_smooth_collapse_exists() {
        let mesh = box_mesh();
        let primitive = &mesh.primitives[0];
        let working = weld_primitive(primitive, 0, &BTreeSet::new(), &BTreeSet::new()).unwrap();
        let hard_edges = geometric_hard_edges(&working, 0.7071067811865476);
        assert!(!hard_edges.is_empty());
        let (selected, _) = best_safe_collapse_with_edge(&working)
            .unwrap()
            .expect("box has a planar diagonal to collapse");
        assert!(!hard_edges.contains(&selected));
    }

    #[test]
    fn hard_edge_only_mesh_fails_closed() {
        let mesh = octahedron();
        let primitive = &mesh.primitives[0];
        let working = weld_primitive(primitive, 0, &BTreeSet::new(), &BTreeSet::new()).unwrap();
        let hard_edges = geometric_hard_edges(&working, 0.7071067811865476);
        assert_eq!(hard_edges.len(), 12);
        assert!(best_safe_collapse_with_edge(&working).unwrap().is_none());
    }

    #[test]
    fn source_attribute_seams_are_derived_without_a_wire_field() {
        let artifact = box_artifact();
        let mesh = integrity::extract_diagnostic_mesh(&artifact.glb, 100).unwrap();
        let protected = derive_attribute_seam_edges(&artifact.glb, &mesh).unwrap();
        assert!(!protected.is_empty());
        assert!(protected
            .iter()
            .all(|(primitive, first, second)| *primitive == 0 && first < second));
    }

    #[test]
    fn protected_source_edges_fail_closed_instead_of_crossing_the_constraint() {
        let mesh = octahedron();
        let mut protected_edges = BTreeSet::new();
        for face in mesh.primitives[0].indices.chunks_exact(3) {
            for [first, second] in [[face[0], face[1]], [face[1], face[2]], [face[2], face[0]]] {
                protected_edges.insert((0, first.min(second), first.max(second)));
            }
        }
        let error = derive_bounded_low(
            &mesh,
            &LowRetopologyPolicy {
                target_triangle_count: 6,
                max_collapses: 4,
                locked_vertices: BTreeSet::new(),
                protected_edges,
            },
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("LOW_RETOPOLOGY_NO_SAFE_COLLAPSE"));
    }

    #[test]
    fn open_or_noop_input_is_rejected() {
        let mut open = octahedron();
        open.primitives[0].indices.truncate(21);
        open.triangle_count = 7;
        let policy = LowRetopologyPolicy {
            target_triangle_count: 5,
            max_collapses: 4,
            locked_vertices: BTreeSet::new(),
            protected_edges: BTreeSet::new(),
        };
        assert!(derive_bounded_low(&open, &policy).is_err());
        let noop = LowRetopologyPolicy {
            target_triangle_count: 8,
            ..policy
        };
        assert!(derive_bounded_low(&octahedron(), &noop).is_err());
    }

    #[test]
    fn closed_worker_adapter_is_hash_bound_and_non_promoting() {
        let artifact = box_artifact();
        let source_hash = sha256_hex(&artifact.glb);
        let mut request = json!({
            "schema_version":REQUEST_SCHEMA_VERSION,
            "preview_only":true,
            "source_high_artifact_sha256":source_hash,
            "high_glb_base64":base64::engine::general_purpose::STANDARD.encode(&artifact.glb),
            "target_triangle_count":10,
            "max_collapses":8,
            "locked_vertices":[],
            "retopology_policy":POLICY,
            "algorithm":ALGORITHM,
            "canonical_sha256":""
        });
        let mut preimage = request.as_object().unwrap().clone();
        preimage.remove("canonical_sha256");
        request["canonical_sha256"] =
            Value::String(crate::canonical_hash(&Value::Object(preimage)));
        let first = run(request.as_object().unwrap()).unwrap();
        let replay = run(request.as_object().unwrap()).unwrap();
        assert_eq!(first, replay);
        let wire: Value = serde_json::from_slice(&serde_json::to_vec(&first).unwrap()).unwrap();
        let mut wire_preimage = wire;
        wire_preimage["canonical_sha256"] = Value::String(String::new());
        assert_eq!(
            first["canonical_sha256"],
            crate::canonical_hash(&wire_preimage)
        );
        let dispatched = crate::worker_result(&json!({
            "operation":forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_RETOPOLOGY_OPERATION,
            "payload":request.clone()
        }))
        .unwrap();
        assert_eq!(first, dispatched);
        assert_eq!(first["retopology_derived"], true);
        assert_eq!(first["artist_authored_quad_topology"], false);
        assert_eq!(first["production_stage_advanced"], false);
        assert_eq!(first["promotion_eligible"], false);
        let low_glb = base64::engine::general_purpose::STANDARD
            .decode(first["low_glb_base64"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            sha256_hex(&low_glb),
            first["low_artifact_sha256"].as_str().unwrap()
        );
        let low_mesh = integrity::extract_diagnostic_mesh(&low_glb, 100).unwrap();
        assert_eq!(low_mesh.triangle_count, 10);
        assert!(low_mesh.triangle_count < artifact.triangle_count as usize);
        let mut tampered = request;
        tampered["source_high_artifact_sha256"] = Value::String("b".repeat(64));
        assert!(run(tampered.as_object().unwrap()).is_err());
    }

    fn explicit_quad_cube_request() -> Value {
        let vertices = vec![
            ("v0", [-1.0, -1.0, -1.0]),
            ("v1", [1.0, -1.0, -1.0]),
            ("v2", [1.0, 1.0, -1.0]),
            ("v3", [-1.0, 1.0, -1.0]),
            ("v4", [-1.0, -1.0, 1.0]),
            ("v5", [1.0, -1.0, 1.0]),
            ("v6", [1.0, 1.0, 1.0]),
            ("v7", [-1.0, 1.0, 1.0]),
        ];
        let faces = vec![
            ("f0", vec!["v0", "v3", "v2", "v1"]),
            ("f1", vec!["v4", "v5", "v6", "v7"]),
            ("f2", vec!["v0", "v4", "v7", "v3"]),
            ("f3", vec!["v1", "v2", "v6", "v5"]),
            ("f4", vec!["v0", "v1", "v5", "v4"]),
            ("f5", vec!["v3", "v7", "v6", "v2"]),
        ];
        let mut edge_ids = BTreeSet::<(String, String)>::new();
        for (_, face) in &faces {
            for index in 0..face.len() {
                let first = face[index].to_owned();
                let second = face[(index + 1) % face.len()].to_owned();
                edge_ids.insert(if first < second {
                    (first, second)
                } else {
                    (second, first)
                });
            }
        }
        let edges = edge_ids
            .iter()
            .map(|(first, second)| {
                json!({
                    "element_id":format!("e-{first}-{second}"),
                    "vertex_ids":[first,second]
                })
            })
            .collect::<Vec<_>>();
        let mut loops = Vec::new();
        let mut face_values = Vec::new();
        for (face_id, face) in &faces {
            let mut loop_ids = Vec::new();
            for ordinal in 0..face.len() {
                let first = face[ordinal];
                let second = face[(ordinal + 1) % face.len()];
                let (left, right) = if first < second {
                    (first, second)
                } else {
                    (second, first)
                };
                let loop_id = format!("l-{face_id}-{ordinal}");
                loops.push(json!({
                    "element_id":loop_id,
                    "face_id":face_id,
                    "ordinal":ordinal,
                    "vertex_id":first,
                    "edge_id":format!("e-{left}-{right}"),
                    "edge_forward":first == left
                }));
                loop_ids.push(Value::String(loop_id));
            }
            face_values.push(json!({"element_id":face_id,"loop_ids":loop_ids}));
        }
        let authoring_mesh = json!({
            "shape":"authoring-mesh",
            "topology_policy":"triangle-quad-manifold-with-boundary@1",
            "vertices":vertices.iter().map(|(id,position)| json!({"element_id":id,"position_m":position})).collect::<Vec<_>>(),
            "edges":edges,
            "loops":loops,
            "faces":face_values,
            "position_m":[0.0,0.0,0.0],
            "rotation_rad":[0.0,0.0,0.0]
        });
        let source_lineage = json!({
            "source_high_artifact_sha256":"a".repeat(64),
            "source_high_artifact_readback_sha256":"b".repeat(64),
            "source_high_part_id":"body",
            "source_high_node_id":"body-node",
            "source_high_material_zone_id":"zone-shell"
        });
        let mut request = json!({
            "schema_version":forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_REQUEST_SCHEMA_VERSION,
            "preview_only":true,
            "project_id":"project-low-quad",
            "source_high_artifact_sha256":"a".repeat(64),
            "source_high_artifact_readback_sha256":"b".repeat(64),
            "source_high_part_id":"body",
            "source_high_node_id":"body-node",
            "source_high_material_zone_id":"zone-shell",
            "draft":{
                "schema_version":QUAD_DRAFT_SCHEMA_VERSION,
                "source_lineage":source_lineage,
                "authoring_mesh":authoring_mesh
            },
            "max_vertices":128,
            "max_edges":128,
            "max_faces":64,
            "low_retopology_policy":forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY,
            "algorithm":forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM,
            "canonical_sha256":""
        });
        let mut preimage = request.as_object().unwrap().clone();
        preimage.remove("canonical_sha256");
        request["canonical_sha256"] =
            Value::String(crate::canonical_hash(&Value::Object(preimage)));
        request
    }

    #[test]
    fn explicit_quad_draft_compiles_to_readback_without_auto_retopology_claim() {
        let request = explicit_quad_cube_request();
        let first = run_quad_draft(request.as_object().unwrap()).expect("quad draft");
        let second = run_quad_draft(request.as_object().unwrap()).expect("quad draft replay");
        assert_eq!(first, second);
        let dispatched = crate::worker_result(&json!({
            "operation":forgecad_worker_protocol::PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION,
            "payload":request.clone()
        }))
        .expect("quad draft dispatch");
        assert_eq!(first, dispatched);
        assert_eq!(first["explicit_quad_faces"], true);
        assert_eq!(first["auto_retopology_performed"], false);
        assert_eq!(first["retopology_derived"], false);
        assert_eq!(first["artist_authored_quad_topology"], false);
        assert_eq!(first["edge_flow_status"], "DRAFT_UNREVIEWED");
        assert_eq!(first["edge_flow"]["quad_face_count"], 6);
        assert_eq!(first["quad_face_count"], 6);
        assert_eq!(first["render_triangle_count"], 12);
        assert_eq!(first["low_quad_draft_readback"]["failure_codes"], json!([]));
        assert!(first["hard_gate_passed"].as_bool().unwrap());
        assert!(!first["low_quad_draft_glb_base64"]
            .as_str()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn explicit_quad_draft_rejects_triangle_faces_and_draft_hash_drift() {
        let mut request = explicit_quad_cube_request();
        request["draft"]["authoring_mesh"]["faces"][0]["loop_ids"] =
            json!(["l-f0-0", "l-f0-1", "l-f0-2"]);
        let mut preimage = request.as_object().unwrap().clone();
        preimage.remove("canonical_sha256");
        request["canonical_sha256"] =
            Value::String(crate::canonical_hash(&Value::Object(preimage)));
        let error = run_quad_draft(request.as_object().unwrap()).expect_err("triangle rejection");
        assert!(error.to_string().contains("LOW_QUAD_DRAFT_FACE_NOT_QUAD"));

        let mut drifted = explicit_quad_cube_request();
        drifted["draft"]["source_lineage"]["source_high_part_id"] = json!("other-part");
        let mut drift_preimage = drifted.as_object().unwrap().clone();
        drift_preimage.remove("canonical_sha256");
        drifted["canonical_sha256"] =
            Value::String(crate::canonical_hash(&Value::Object(drift_preimage)));
        let error = run_quad_draft(drifted.as_object().unwrap()).expect_err("lineage rejection");
        assert!(error
            .to_string()
            .contains("LOW_QUAD_DRAFT_SOURCE_LINEAGE_MISMATCH"));
    }
}
