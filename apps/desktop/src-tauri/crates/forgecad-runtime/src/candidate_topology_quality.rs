//! Candidate-wide, Runtime-owned topology quality for the production-stage
//! `gray-model -> topology` boundary.
//!
//! This module deliberately consumes the exact candidate GLB and its durable
//! `ArtifactReadback@2`/`GeometryCandidateEvidence@1` lineage.  It does not
//! infer an authoring cage from an evaluated GLB and it does not turn
//! structural metrics into an edge-flow, artistic, likeness, or engine claim.

use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, is_opaque_id, is_sha256, sha256_hex,
    strict_glb_inspection, validate_artifact_readback_v2_output,
    validate_geometry_candidate_evidence_output, validate_geometry_quality_report_v2_output,
    verify_output_canonical_hash, Runtime, RuntimeError, MAX_DERIVED_JSON_BYTES,
    MAX_GEOMETRY_ARTIFACT_BYTES, TOPOLOGY_SNAPSHOT_POLICY,
};
use forgecad_contracts::{
    CandidateTopologyQualityHardGate, CandidateTopologyQualityMetrics,
    CandidateTopologyQualityRecord, CandidateTopologyQualityThresholds,
};
use forgecad_store::CasObject;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

const PREPARE_SCHEMA: &str = "CandidateTopologyQualityPrepareRequest@1";
const GET_SCHEMA: &str = "CandidateTopologyQualityGetRequest@1";
const PREPARE_RESULT_SCHEMA: &str = "CandidateTopologyQualityPrepareResult@1";
const GET_RESULT_SCHEMA: &str = "CandidateTopologyQualityGetResult@1";
const QUALITY_KIND: &str = "candidate-topology-quality-report";
const JSON_MIME: &str = "application/json";
const MAX_JSON_BYTES: u64 = 1024 * 1024;
const MAX_PARTS: usize = 512;
const MAX_TRIANGLES: u64 = 250_000;
const MAX_VERTICES: u64 = 250_000;
const MAX_EDGES: u64 = 500_000;
const MAX_SNAPSHOT_FACES: u64 = 512;
const MAX_TRIANGLE_ASPECT_RATIO: f64 = 100.0;
const MAX_VERTEX_VALENCE: u64 = 64;
const MIN_TRIANGLE_AREA_M2: f64 = 1.0e-10;
const MIN_SEMANTIC_COVERAGE: f64 = 1.0;
const NORMAL_UNIT_EPSILON: f64 = 1.0e-3;
const UV_AREA_EPSILON: f64 = 1.0e-8;
const TOPOLOGY_QUALITY_POLICY: &str = "candidate-topology-hard-gate@1";
const MATERIALIZATION_STATUS: &str = "runtime-owned-durable-candidate-topology-quality";

#[derive(Debug)]
struct RequestBinding {
    topology_quality_id: String,
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    artifact_id: String,
    artifact_sha256: String,
    artifact_readback_sha256: String,
    artifact_readback_object_sha256: String,
    geometry_candidate_evidence_sha256: String,
    geometry_program_sha256: String,
    geometry_program_object_sha256: String,
    operator_catalog_sha256: String,
    readback_config_sha256: String,
    part_inventory_sha256: String,
    part_ids: Vec<String>,
    requested_snapshot_hashes: Vec<String>,
    authoring_topology_status: String,
    requested_authoring_hashes: Vec<Option<String>>,
    topology_quality_policy: String,
    topology_quality_policy_sha256: String,
    from_stage: String,
    to_stage: String,
    input_sha256: String,
    request_sha256: String,
}

#[derive(Debug)]
struct SnapshotAnalysis {
    value: Value,
    canonical_sha256: String,
}

fn topology_error(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("TOPOLOGY_QUALITY_INVALID: {}", message.into()))
}

fn required_id(object: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| topology_error(format!("{key} is not a valid identifier")))
}

fn required_hash(object: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| topology_error(format!("{key} is not a SHA-256")))
}

fn required_text(object: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .map(str::to_owned)
        .ok_or_else(|| topology_error(format!("{key} is not valid text")))
}

fn required_hash_list(object: &Map<String, Value>, key: &str) -> Result<Vec<String>, RuntimeError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= MAX_PARTS)
        .ok_or_else(|| topology_error(format!("{key} is not a bounded list")))?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let hash = value
            .as_str()
            .filter(|value| is_sha256(value))
            .ok_or_else(|| topology_error(format!("{key} contains an invalid hash")))?;
        if result.iter().any(|existing| existing == hash) {
            return Err(topology_error(format!("{key} contains duplicates")));
        }
        result.push(hash.to_owned());
    }
    Ok(result)
}

fn required_id_list(object: &Map<String, Value>, key: &str) -> Result<Vec<String>, RuntimeError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= MAX_PARTS)
        .ok_or_else(|| topology_error(format!("{key} is not a bounded list")))?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let id = value
            .as_str()
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| topology_error(format!("{key} contains an invalid identifier")))?;
        if result.iter().any(|existing| existing == id) {
            return Err(topology_error(format!("{key} contains duplicates")));
        }
        result.push(id.to_owned());
    }
    Ok(result)
}

fn nullable_hash_list(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Vec<Option<String>>, RuntimeError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= MAX_PARTS)
        .ok_or_else(|| topology_error(format!("{key} is not a bounded list")))?;
    values
        .iter()
        .map(|value| {
            if value.is_null() {
                Ok(None)
            } else {
                value
                    .as_str()
                    .filter(|value| is_sha256(value))
                    .map(|value| Some(value.to_owned()))
                    .ok_or_else(|| topology_error(format!("{key} contains an invalid hash")))
            }
        })
        .collect()
}

fn request_binding(value: &Value) -> Result<RequestBinding, RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "topology_quality_id",
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "artifact_id",
            "artifact_sha256",
            "artifact_readback_sha256",
            "artifact_readback_object_sha256",
            "geometry_candidate_evidence_sha256",
            "geometry_program_sha256",
            "geometry_program_object_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "part_inventory_sha256",
            "part_ids",
            "part_topology_snapshot_sha256s",
            "authoring_topology_status",
            "part_authoring_topology_sha256s",
            "topology_quality_policy",
            "topology_quality_policy_sha256",
            "from_stage",
            "to_stage",
            "input_sha256",
            "idempotency_key",
        ],
        PREPARE_SCHEMA,
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some(PREPARE_SCHEMA) {
        return Err(topology_error("request schema_version differs"));
    }
    let topology_quality_policy = required_text(object, "topology_quality_policy")?;
    let topology_quality_policy_sha256 = required_hash(object, "topology_quality_policy_sha256")?;
    if topology_quality_policy != TOPOLOGY_QUALITY_POLICY
        || sha256_hex(TOPOLOGY_QUALITY_POLICY.as_bytes()) != topology_quality_policy_sha256
    {
        return Err(topology_error("topology quality policy differs"));
    }
    let from_stage = required_text(object, "from_stage")?;
    let to_stage = required_text(object, "to_stage")?;
    if from_stage != "gray-model" || to_stage != "topology" {
        return Err(topology_error("only gray-model to topology is supported"));
    }
    let part_ids = required_id_list(object, "part_ids")?;
    let requested_snapshot_hashes = required_hash_list(object, "part_topology_snapshot_sha256s")?;
    let requested_authoring_hashes = nullable_hash_list(object, "part_authoring_topology_sha256s")?;
    if part_ids.len() != requested_snapshot_hashes.len()
        || part_ids.len() != requested_authoring_hashes.len()
    {
        return Err(topology_error(
            "Part, snapshot and authoring lists must have identical lengths",
        ));
    }
    let authoring_topology_status = required_text(object, "authoring_topology_status")?;
    if !matches!(
        authoring_topology_status.as_str(),
        "complete" | "partial" | "not-available"
    ) {
        return Err(topology_error("authoring_topology_status is unsupported"));
    }
    let input_sha256 = required_hash(object, "input_sha256")?;
    let _idempotency_key = required_id(object, "idempotency_key")?;
    let mut input_binding = object.clone();
    input_binding.remove("input_sha256");
    input_binding.remove("idempotency_key");
    let expected_input_sha256 = canonical_json_hash(&Value::Object(input_binding.clone()));
    if input_sha256 != expected_input_sha256 {
        return Err(topology_error(format!(
            "input_sha256 differs; expected {expected_input_sha256}"
        )));
    }
    let request_sha256 = canonical_json_hash(&Value::Object(input_binding));
    Ok(RequestBinding {
        topology_quality_id: required_id(object, "topology_quality_id")?,
        project_id: required_id(object, "project_id")?,
        candidate_id: required_id(object, "candidate_id")?,
        candidate_state_sha256: required_hash(object, "candidate_state_sha256")?,
        artifact_id: required_id(object, "artifact_id")?,
        artifact_sha256: required_hash(object, "artifact_sha256")?,
        artifact_readback_sha256: required_hash(object, "artifact_readback_sha256")?,
        artifact_readback_object_sha256: required_hash(object, "artifact_readback_object_sha256")?,
        geometry_candidate_evidence_sha256: required_hash(
            object,
            "geometry_candidate_evidence_sha256",
        )?,
        geometry_program_sha256: required_hash(object, "geometry_program_sha256")?,
        geometry_program_object_sha256: required_hash(object, "geometry_program_object_sha256")?,
        operator_catalog_sha256: required_hash(object, "operator_catalog_sha256")?,
        readback_config_sha256: required_hash(object, "readback_config_sha256")?,
        part_inventory_sha256: required_hash(object, "part_inventory_sha256")?,
        part_ids,
        requested_snapshot_hashes,
        authoring_topology_status,
        requested_authoring_hashes,
        topology_quality_policy,
        topology_quality_policy_sha256,
        from_stage,
        to_stage,
        input_sha256,
        request_sha256,
    })
}

fn get_binding(value: &Value) -> Result<(String, String, String), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "topology_quality_id",
            "project_id",
            "candidate_id",
        ],
        GET_SCHEMA,
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some(GET_SCHEMA) {
        return Err(topology_error("request schema_version differs"));
    }
    Ok((
        required_id(object, "topology_quality_id")?,
        required_id(object, "project_id")?,
        required_id(object, "candidate_id")?,
    ))
}

fn quality_thresholds() -> CandidateTopologyQualityThresholds {
    CandidateTopologyQualityThresholds {
        max_triangle_aspect_ratio: MAX_TRIANGLE_ASPECT_RATIO,
        max_vertex_valence: MAX_VERTEX_VALENCE,
        min_triangle_area_m2: MIN_TRIANGLE_AREA_M2,
        min_semantic_part_coverage: MIN_SEMANTIC_COVERAGE,
        min_semantic_material_zone_coverage: MIN_SEMANTIC_COVERAGE,
        min_semantic_source_node_coverage: MIN_SEMANTIC_COVERAGE,
    }
}

fn length3(value: &[f64; 3]) -> f64 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

fn vec3(value: &Value) -> Result<[f64; 3], RuntimeError> {
    let values = value
        .as_array()
        .filter(|values| values.len() == 3)
        .ok_or_else(|| topology_error("snapshot vector is not a vec3"))?;
    let result = [
        values[0]
            .as_f64()
            .ok_or_else(|| topology_error("snapshot vector is not finite"))?,
        values[1]
            .as_f64()
            .ok_or_else(|| topology_error("snapshot vector is not finite"))?,
        values[2]
            .as_f64()
            .ok_or_else(|| topology_error("snapshot vector is not finite"))?,
    ];
    if result.iter().any(|value| !value.is_finite()) {
        return Err(topology_error("snapshot vector is not finite"));
    }
    Ok(result)
}

fn sub3(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn dot3(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn snapshot_metrics(
    inspection: &super::integrity::GlbIntegrity,
    readback: &Value,
    snapshots: &[SnapshotAnalysis],
) -> Result<CandidateTopologyQualityMetrics, RuntimeError> {
    let bindings = readback
        .get("part_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| topology_error("ArtifactReadback part_bindings is missing"))?;
    let solid_by_part = bindings
        .iter()
        .map(|binding| {
            let part_id = binding
                .get("part_id")
                .and_then(Value::as_str)
                .ok_or_else(|| topology_error("ArtifactReadback Part binding is invalid"))?;
            let solid = binding
                .get("solid")
                .and_then(Value::as_bool)
                .ok_or_else(|| topology_error("ArtifactReadback solid binding is invalid"))?;
            Ok((part_id.to_owned(), solid))
        })
        .collect::<Result<BTreeMap<_, _>, RuntimeError>>()?;

    let mut boundary_edge_count = 0u64;
    let mut non_manifold_edge_count = 0u64;
    let mut orientation_conflict_count = 0u64;
    let mut solid_boundary_violation_count = 0u64;
    let mut vertex_count = 0u64;
    let mut edge_count = 0u64;
    let mut min_triangle_area_m2 = f64::INFINITY;
    let mut max_triangle_aspect_ratio = 0.0f64;
    let mut max_vertex_valence = 0u64;
    let mut normal_non_finite_count = 0u64;
    let mut normal_non_unit_count = 0u64;
    let mut normal_alignment_error_count = 0u64;
    let mut uv_degenerate_triangle_count = 0u64;

    for snapshot in snapshots {
        let object = snapshot
            .value
            .as_object()
            .ok_or_else(|| topology_error("TopologySnapshot is not an object"))?;
        let part_id = object
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| topology_error("TopologySnapshot part_id is missing"))?;
        let solid = *solid_by_part
            .get(part_id)
            .ok_or_else(|| topology_error("TopologySnapshot Part is absent from readback"))?;
        let topology = object
            .get("topology")
            .and_then(Value::as_object)
            .ok_or_else(|| topology_error("TopologySnapshot topology is missing"))?;
        let boundaries = topology
            .get("boundary_edge_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| topology_error("TopologySnapshot boundary count is invalid"))?;
        let non_manifold = topology
            .get("non_manifold_edge_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| topology_error("TopologySnapshot manifold count is invalid"))?;
        let orientation = topology
            .get("orientation_conflict_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| topology_error("TopologySnapshot orientation count is invalid"))?;
        boundary_edge_count = boundary_edge_count.saturating_add(boundaries);
        non_manifold_edge_count = non_manifold_edge_count.saturating_add(non_manifold);
        orientation_conflict_count = orientation_conflict_count.saturating_add(orientation);
        if solid {
            solid_boundary_violation_count =
                solid_boundary_violation_count.saturating_add(boundaries);
        }

        let vertices = object
            .get("vertices")
            .and_then(Value::as_array)
            .ok_or_else(|| topology_error("TopologySnapshot vertices are missing"))?;
        let edges = object
            .get("edges")
            .and_then(Value::as_array)
            .ok_or_else(|| topology_error("TopologySnapshot edges are missing"))?;
        let faces = object
            .get("faces")
            .and_then(Value::as_array)
            .ok_or_else(|| topology_error("TopologySnapshot faces are missing"))?;
        let corners = object
            .get("corners")
            .and_then(Value::as_array)
            .ok_or_else(|| topology_error("TopologySnapshot corners are missing"))?;
        vertex_count = vertex_count.saturating_add(vertices.len() as u64);
        edge_count = edge_count.saturating_add(edges.len() as u64);
        for vertex in vertices {
            let valence = vertex
                .get("incident_edge_ids")
                .and_then(Value::as_array)
                .ok_or_else(|| topology_error("TopologySnapshot vertex valence is invalid"))?
                .len() as u64;
            max_vertex_valence = max_vertex_valence.max(valence);
        }
        let mut corners_by_id = BTreeMap::<String, &Value>::new();
        for corner in corners {
            let corner_id = corner
                .get("corner_id")
                .and_then(Value::as_str)
                .ok_or_else(|| topology_error("TopologySnapshot corner ID is invalid"))?;
            corners_by_id.insert(corner_id.to_owned(), corner);
            let normal = vec3(
                corner
                    .get("normal")
                    .ok_or_else(|| topology_error("TopologySnapshot normal is missing"))?,
            )?;
            if !normal.iter().all(|value| value.is_finite()) {
                normal_non_finite_count = normal_non_finite_count.saturating_add(1);
            } else if (length3(&normal) - 1.0).abs() > NORMAL_UNIT_EPSILON {
                normal_non_unit_count = normal_non_unit_count.saturating_add(1);
            }
        }
        for face in faces {
            let area = face
                .get("area_m2")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
                .ok_or_else(|| topology_error("TopologySnapshot face area is invalid"))?;
            min_triangle_area_m2 = min_triangle_area_m2.min(area);
            let face_normal = vec3(
                face.get("normal")
                    .ok_or_else(|| topology_error("TopologySnapshot face normal is missing"))?,
            )?;
            let corner_ids = face
                .get("corner_ids")
                .and_then(Value::as_array)
                .filter(|values| values.len() == 3)
                .ok_or_else(|| topology_error("TopologySnapshot face corners are invalid"))?;
            let mut positions = Vec::with_capacity(3);
            for corner_id in corner_ids {
                let corner_id = corner_id
                    .as_str()
                    .ok_or_else(|| topology_error("TopologySnapshot face corner ID is invalid"))?;
                let corner = corners_by_id
                    .get(corner_id)
                    .ok_or_else(|| topology_error("TopologySnapshot face corner is missing"))?;
                let position = vec3(corner.get("position_m").ok_or_else(|| {
                    topology_error("TopologySnapshot corner position is missing")
                })?)?;
                let normal =
                    vec3(corner.get("normal").ok_or_else(|| {
                        topology_error("TopologySnapshot corner normal is missing")
                    })?)?;
                if dot3(face_normal, normal) <= 0.0 {
                    normal_alignment_error_count = normal_alignment_error_count.saturating_add(1);
                }
                let uv = corner
                    .get("texcoord_0")
                    .and_then(Value::as_array)
                    .filter(|values| values.len() == 2)
                    .ok_or_else(|| topology_error("TopologySnapshot UV is invalid"))?;
                if !uv[0].as_f64().is_some_and(f64::is_finite)
                    || !uv[1].as_f64().is_some_and(f64::is_finite)
                {
                    return Err(topology_error("TopologySnapshot UV is not finite"));
                }
                positions.push(position);
            }
            let a = sub3(positions[1], positions[0]);
            let b = sub3(positions[2], positions[0]);
            let c = sub3(positions[2], positions[1]);
            let side_a = length3(&a);
            let side_b = length3(&b);
            let side_c = length3(&c);
            let longest = side_a.max(side_b).max(side_c);
            let aspect = if area > 0.0 {
                longest * longest / (2.0 * area)
            } else {
                f64::INFINITY
            };
            if !aspect.is_finite() {
                return Err(topology_error("triangle aspect ratio is not finite"));
            }
            max_triangle_aspect_ratio = max_triangle_aspect_ratio.max(aspect);
            let uv_area = {
                let uv0 = corners_by_id
                    .get(corner_ids[0].as_str().unwrap_or_default())
                    .and_then(|value| value.get("texcoord_0"))
                    .and_then(Value::as_array);
                let uv1 = corners_by_id
                    .get(corner_ids[1].as_str().unwrap_or_default())
                    .and_then(|value| value.get("texcoord_0"))
                    .and_then(Value::as_array);
                let uv2 = corners_by_id
                    .get(corner_ids[2].as_str().unwrap_or_default())
                    .and_then(|value| value.get("texcoord_0"))
                    .and_then(Value::as_array);
                match (uv0, uv1, uv2) {
                    (Some(uv0), Some(uv1), Some(uv2)) => {
                        let u0 = uv0[0].as_f64().unwrap_or(f64::NAN);
                        let v0 = uv0[1].as_f64().unwrap_or(f64::NAN);
                        let u1 = uv1[0].as_f64().unwrap_or(f64::NAN);
                        let v1 = uv1[1].as_f64().unwrap_or(f64::NAN);
                        let u2 = uv2[0].as_f64().unwrap_or(f64::NAN);
                        let v2 = uv2[1].as_f64().unwrap_or(f64::NAN);
                        (u1 - u0) * (v2 - v0) - (v1 - v0) * (u2 - u0)
                    }
                    _ => f64::NAN,
                }
            };
            if !uv_area.is_finite() || uv_area.abs() <= UV_AREA_EPSILON {
                uv_degenerate_triangle_count = uv_degenerate_triangle_count.saturating_add(1);
            }
        }
    }

    let integrity = readback
        .get("integrity")
        .and_then(Value::as_object)
        .ok_or_else(|| topology_error("ArtifactReadback integrity is missing"))?;
    let coverage = |key: &str| {
        integrity
            .get(key)
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
            .unwrap_or(0.0)
    };
    let min_triangle_area_m2 = if min_triangle_area_m2.is_finite() {
        min_triangle_area_m2
    } else {
        0.0
    };
    Ok(CandidateTopologyQualityMetrics {
        invalid_index_count: inspection.invalid_index_count,
        non_finite_count: inspection.non_finite_count,
        degenerate_triangle_count: inspection.degenerate_triangle_count,
        boundary_edge_count,
        non_manifold_edge_count,
        orientation_conflict_count,
        winding_error_count: inspection.winding_error_count,
        part_count: snapshots.len() as u64,
        solid_part_count: solid_by_part.values().filter(|solid| **solid).count() as u64,
        non_solid_part_count: solid_by_part.values().filter(|solid| !**solid).count() as u64,
        solid_boundary_violation_count,
        triangle_count: inspection.triangle_count,
        vertex_count,
        edge_count,
        min_triangle_area_m2,
        max_triangle_aspect_ratio,
        max_vertex_valence,
        normal_non_finite_count,
        normal_non_unit_count,
        normal_alignment_error_count,
        uv_non_finite_count: inspection.uv_non_finite_count,
        uv_degenerate_triangle_count,
        tangent_non_finite_count: inspection.tangent_non_finite_count,
        tangent_orthogonality_error_count: inspection.tangent_orthogonality_error_count,
        tangent_handedness_error_count: inspection.tangent_handedness_error_count,
        semantic_part_coverage: coverage("part_coverage"),
        semantic_material_zone_coverage: coverage("material_zone_coverage"),
        semantic_source_node_coverage: coverage("source_coverage"),
    })
}

fn hard_gate(metrics: &CandidateTopologyQualityMetrics) -> CandidateTopologyQualityHardGate {
    CandidateTopologyQualityHardGate {
        finite_geometry: metrics.non_finite_count == 0
            && metrics.normal_non_finite_count == 0
            && metrics.uv_non_finite_count == 0
            && metrics.tangent_non_finite_count == 0,
        valid_indices: metrics.invalid_index_count == 0,
        non_degenerate_triangles: metrics.degenerate_triangle_count == 0
            && metrics.min_triangle_area_m2 >= MIN_TRIANGLE_AREA_M2,
        boundary_policy: metrics.solid_boundary_violation_count == 0,
        manifold: metrics.non_manifold_edge_count == 0,
        orientation: metrics.orientation_conflict_count == 0
            && metrics.winding_error_count == 0
            && metrics.normal_alignment_error_count == 0,
        counts_within_budget: metrics.part_count <= MAX_PARTS as u64
            && metrics.triangle_count <= MAX_TRIANGLES
            && metrics.vertex_count <= MAX_VERTICES
            && metrics.edge_count <= MAX_EDGES,
        triangle_aspect_ratio: metrics.max_triangle_aspect_ratio <= MAX_TRIANGLE_ASPECT_RATIO,
        vertex_valence: metrics.max_vertex_valence <= MAX_VERTEX_VALENCE,
        normal_integrity: metrics.normal_non_finite_count == 0
            && metrics.normal_non_unit_count == 0
            && metrics.normal_alignment_error_count == 0,
        uv_integrity: metrics.uv_non_finite_count == 0 && metrics.uv_degenerate_triangle_count == 0,
        tangent_integrity: metrics.tangent_non_finite_count == 0
            && metrics.tangent_orthogonality_error_count == 0
            && metrics.tangent_handedness_error_count == 0,
        semantic_coverage: metrics.semantic_part_coverage >= MIN_SEMANTIC_COVERAGE
            && metrics.semantic_material_zone_coverage >= MIN_SEMANTIC_COVERAGE
            && metrics.semantic_source_node_coverage >= MIN_SEMANTIC_COVERAGE,
    }
}

fn all_gate_fields(gate: &CandidateTopologyQualityHardGate) -> bool {
    gate.finite_geometry
        && gate.valid_indices
        && gate.non_degenerate_triangles
        && gate.boundary_policy
        && gate.manifold
        && gate.orientation
        && gate.counts_within_budget
        && gate.triangle_aspect_ratio
        && gate.vertex_valence
        && gate.normal_integrity
        && gate.uv_integrity
        && gate.tangent_integrity
        && gate.semantic_coverage
}

fn part_inventory_sha256(readback: &Value) -> Result<String, RuntimeError> {
    let parts = readback
        .get("part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| topology_error("ArtifactReadback part_ids is missing"))?;
    let bindings = readback
        .get("part_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| topology_error("ArtifactReadback part_bindings is missing"))?;
    Ok(canonical_json_hash(&json!({
        "part_ids":parts,
        "part_bindings":bindings,
    })))
}

fn load_authoring_hashes(
    runtime: &Runtime,
    binding: &RequestBinding,
    evidence: &forgecad_contracts::GeometryCandidateEvidenceRecord,
    readback: &Value,
) -> Result<(String, Vec<Option<String>>), RuntimeError> {
    let program_bytes = runtime.cas_read_bounded(
        &evidence.geometry_program_object_sha256,
        MAX_DERIVED_JSON_BYTES,
    )?;
    let program: Value = serde_json::from_slice(&program_bytes)
        .map_err(|error| topology_error(format!("GeometryProgram JSON is invalid: {error}")))?;
    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| topology_error("GeometryProgram nodes are missing"))?;
    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| topology_error("GeometryProgram part_outputs are missing"))?;
    let bindings = readback
        .get("part_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| topology_error("ArtifactReadback part_bindings are missing"))?;
    let mut hashes = Vec::with_capacity(binding.part_ids.len());
    for part_id in &binding.part_ids {
        let Some(readback_binding) = bindings
            .iter()
            .find(|value| value.get("part_id").and_then(Value::as_str) == Some(part_id.as_str()))
        else {
            return Err(topology_error(
                "Part is absent from ArtifactReadback bindings",
            ));
        };
        let source_node_id = readback_binding
            .get("source_node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| topology_error("Part source_node_id is missing"))?;
        let direct_node = nodes.iter().find(|node| {
            node.get("node_id").and_then(Value::as_str) == Some(source_node_id)
                && node.get("operator_id").and_then(Value::as_str)
                    == Some("forgecad.geometry.authoring-mesh@1")
                && node
                    .get("inputs")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
        });
        let direct_output = outputs.iter().any(|part| {
            part.get("part_id").and_then(Value::as_str) == Some(part_id.as_str())
                && part
                    .get("input_node_ids")
                    .and_then(Value::as_array)
                    .is_some_and(|inputs| {
                        inputs.len() == 1 && inputs[0].as_str() == Some(source_node_id)
                    })
        });
        if direct_node.is_none() || !direct_output {
            hashes.push(None);
            continue;
        }
        let request = json!({
            "schema_version":"AuthoringTopologyRequest@1",
            "project_id":binding.project_id,
            "candidate_id":binding.candidate_id,
            "artifact_id":binding.artifact_sha256,
            "artifact_readback_sha256":binding.artifact_readback_sha256,
            "program_sha256":binding.geometry_program_sha256,
            "operator_catalog_sha256":binding.operator_catalog_sha256,
            "readback_config_sha256":binding.readback_config_sha256,
            "authoring_node_id":source_node_id,
            "part_id":part_id,
            "authoring_topology_policy_sha256":"a6fb36a530e49537673b66d65ecb6e4fb4f51ffb3e7d01a0980be71f28cb367d",
            "max_response_bytes":1048576,
        });
        let value = runtime.authoring_topology(&request)?;
        let hash = value
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| topology_error("AuthoringTopology canonical hash is missing"))?;
        hashes.push(Some(hash.to_owned()));
    }
    let status = if hashes.iter().all(Option::is_some) {
        "complete"
    } else if hashes.iter().any(Option::is_some) {
        "partial"
    } else {
        "not-available"
    };
    Ok((status.to_owned(), hashes))
}

fn record_value(record: &CandidateTopologyQualityRecord) -> Result<Value, RuntimeError> {
    serde_json::to_value(record)
        .map_err(|error| topology_error(format!("quality record serialization failed: {error}")))
}

fn build_record(
    binding: &RequestBinding,
    metrics: CandidateTopologyQualityMetrics,
    snapshots: &[SnapshotAnalysis],
    authoring_status: String,
    authoring_hashes: Vec<Option<String>>,
    created_at: String,
) -> Result<CandidateTopologyQualityRecord, RuntimeError> {
    let gate = hard_gate(&metrics);
    let hard_gate_passed = all_gate_fields(&gate);
    let topology_status = if metrics.non_manifold_edge_count > 0
        || metrics.orientation_conflict_count > 0
        || metrics.winding_error_count > 0
    {
        "non_manifold"
    } else if metrics.boundary_edge_count > 0 {
        "open_surface"
    } else {
        "closed_manifold"
    };
    let mut record = CandidateTopologyQualityRecord {
        schema_version: "CandidateTopologyQuality@1".to_owned(),
        topology_quality_id: binding.topology_quality_id.clone(),
        project_id: binding.project_id.clone(),
        candidate_id: binding.candidate_id.clone(),
        candidate_state_sha256: binding.candidate_state_sha256.clone(),
        artifact_id: binding.artifact_id.clone(),
        artifact_sha256: binding.artifact_sha256.clone(),
        artifact_readback_sha256: binding.artifact_readback_sha256.clone(),
        artifact_readback_object_sha256: binding.artifact_readback_object_sha256.clone(),
        geometry_candidate_evidence_sha256: binding.geometry_candidate_evidence_sha256.clone(),
        geometry_program_sha256: binding.geometry_program_sha256.clone(),
        geometry_program_object_sha256: binding.geometry_program_object_sha256.clone(),
        operator_catalog_sha256: binding.operator_catalog_sha256.clone(),
        readback_config_sha256: binding.readback_config_sha256.clone(),
        part_inventory_sha256: binding.part_inventory_sha256.clone(),
        part_ids: binding.part_ids.clone(),
        part_topology_snapshot_sha256s: snapshots
            .iter()
            .map(|snapshot| snapshot.canonical_sha256.clone())
            .collect(),
        authoring_topology_status: authoring_status,
        part_authoring_topology_sha256s: authoring_hashes,
        topology_quality_policy: binding.topology_quality_policy.clone(),
        topology_quality_policy_sha256: binding.topology_quality_policy_sha256.clone(),
        from_stage: binding.from_stage.clone(),
        to_stage: binding.to_stage.clone(),
        topology_status: topology_status.to_owned(),
        thresholds: quality_thresholds(),
        metrics,
        hard_gate: gate,
        validator_status: if hard_gate_passed {
            "passed".to_owned()
        } else {
            "failed".to_owned()
        },
        hard_gate_passed,
        edge_flow_status: "NOT_PROVEN".to_owned(),
        artistic_quality_status: "NOT_PROVEN".to_owned(),
        visual_quality_status: "NOT_PROVEN".to_owned(),
        materialization_status: MATERIALIZATION_STATUS.to_owned(),
        quality_status: "structural_only".to_owned(),
        runtime_write_performed: true,
        candidate_confirmed: false,
        version_created: false,
        export_performed: false,
        request_sha256: binding.request_sha256.clone(),
        input_sha256: binding.input_sha256.clone(),
        canonical_sha256: String::new(),
        created_at,
    };
    let mut value = record_value(&record)?;
    value["canonical_sha256"] = Value::String(String::new());
    record.canonical_sha256 = canonical_json_hash(&value);
    Ok(record)
}

fn result_value(
    record: &CandidateTopologyQualityRecord,
    replayed: bool,
    schema_version: &str,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    let result = json!({
        "schema_version":schema_version,
        "topology_quality":record_value(record)?,
        "replayed":replayed,
        "runtime_write":runtime_write,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
    });
    Ok(result)
}

fn validate_current_bindings(
    runtime: &Runtime,
    binding: &RequestBinding,
) -> Result<
    (
        forgecad_contracts::CandidateRecord,
        forgecad_contracts::GeometryCandidateEvidenceRecord,
        Value,
        super::integrity::GlbIntegrity,
    ),
    RuntimeError,
> {
    let candidate = runtime
        .candidate(&binding.candidate_id)?
        .ok_or_else(|| topology_error("candidate is unavailable"))?;
    if candidate.project_id != binding.project_id
        || candidate.canonical_sha256 != binding.candidate_state_sha256
        || candidate.prepared_object_id.as_deref() != Some(binding.artifact_id.as_str())
        || candidate.prepared_object_sha256.as_deref() != Some(binding.artifact_sha256.as_str())
        || candidate.manifest_hash.as_deref() != Some(binding.artifact_sha256.as_str())
    {
        return Err(topology_error(
            "candidate/project/state/artifact binding differs",
        ));
    }
    if !candidate.quality_hard_gate_passed {
        return Err(topology_error(
            "gray-model candidate structural hard gate has not passed",
        ));
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&binding.candidate_id)?
        .ok_or_else(|| topology_error("GeometryCandidateEvidence is unavailable"))?;
    let evidence_value = serde_json::to_value(&evidence).map_err(|error| {
        topology_error(format!("GeometryCandidateEvidence is invalid: {error}"))
    })?;
    validate_geometry_candidate_evidence_output(&evidence_value)?;
    if evidence.project_id != binding.project_id
        || evidence.candidate_id != binding.candidate_id
        || evidence.canonical_sha256 != binding.geometry_candidate_evidence_sha256
        || evidence.artifact_object_sha256 != binding.artifact_sha256
        || evidence.artifact_readback_object_sha256 != binding.artifact_readback_object_sha256
        || evidence.geometry_program_sha256 != binding.geometry_program_sha256
        || evidence.geometry_program_object_sha256 != binding.geometry_program_object_sha256
        || evidence.operator_catalog_sha256 != binding.operator_catalog_sha256
        || evidence.readback_config_sha256 != binding.readback_config_sha256
    {
        return Err(topology_error("GeometryCandidateEvidence binding differs"));
    }
    let artifact = runtime
        .store
        .get_object(&binding.artifact_sha256)?
        .ok_or_else(|| topology_error("candidate GLB metadata is unavailable"))?;
    if artifact.mime != "model/gltf-binary"
        || !matches!(
            artifact.kind.as_str(),
            "geometry-glb" | "appearance-glb" | "appearance-v2-glb"
        )
        || artifact.size_bytes == 0
        || artifact.size_bytes > MAX_GEOMETRY_ARTIFACT_BYTES
    {
        return Err(topology_error(
            "candidate GLB metadata is outside the closed profile",
        ));
    }
    let bytes = runtime.cas_read_bounded(&binding.artifact_sha256, MAX_GEOMETRY_ARTIFACT_BYTES)?;
    let inspection = strict_glb_inspection(&bytes)?;
    runtime.revalidate_v2_geometry_evidence(&candidate, &inspection, &evidence)?;
    let readback = runtime.artifact_readback(&binding.artifact_sha256, &binding.candidate_id)?;
    validate_artifact_readback_v2_output(&readback)?;
    if readback.get("canonical_sha256").and_then(Value::as_str)
        != Some(binding.artifact_readback_sha256.as_str())
        || readback.get("artifact_id").and_then(Value::as_str)
            != Some(binding.artifact_sha256.as_str())
        || readback.get("candidate_id").and_then(Value::as_str)
            != Some(binding.candidate_id.as_str())
        || readback.get("program_sha256").and_then(Value::as_str)
            != Some(binding.geometry_program_sha256.as_str())
        || readback
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(binding.operator_catalog_sha256.as_str())
        || readback
            .get("readback_config_sha256")
            .and_then(Value::as_str)
            != Some(binding.readback_config_sha256.as_str())
        || readback.get("validator_status").and_then(Value::as_str) != Some("passed")
        || readback.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
    {
        return Err(topology_error(
            "ArtifactReadback@2 binding or hard gate differs",
        ));
    }
    let stored_readback_bytes = runtime.cas_read_bounded(
        &binding.artifact_readback_object_sha256,
        MAX_DERIVED_JSON_BYTES,
    )?;
    let stored_readback: Value =
        serde_json::from_slice(&stored_readback_bytes).map_err(|error| {
            topology_error(format!("ArtifactReadback CAS JSON is invalid: {error}"))
        })?;
    validate_artifact_readback_v2_output(&stored_readback)?;
    if stored_readback != readback {
        return Err(topology_error(
            "ArtifactReadback CAS bytes differ from Runtime replay",
        ));
    }
    let quality_bytes = runtime.cas_read_bounded(
        &evidence.quality_report_object_sha256,
        MAX_DERIVED_JSON_BYTES,
    )?;
    let quality: Value = serde_json::from_slice(&quality_bytes).map_err(|error| {
        topology_error(format!("GeometryQualityReport JSON is invalid: {error}"))
    })?;
    validate_geometry_quality_report_v2_output(&quality)?;
    if quality.get("candidate_id").and_then(Value::as_str) != Some(binding.candidate_id.as_str())
        || quality.get("artifact_sha256").and_then(Value::as_str)
            != Some(binding.artifact_sha256.as_str())
        || quality.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
    {
        return Err(topology_error("GeometryQualityReport binding differs"));
    }
    Ok((candidate, evidence, readback, inspection))
}

fn clean_reservation(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    objects: &[CasObject],
    cleanup_new: bool,
) {
    for object in objects {
        let _ = runtime.store.release_cas_reservation_object(
            reservation,
            object,
            cleanup_new && object.created_new,
        );
    }
}

impl Runtime {
    /// Materialize one candidate-wide, objective topology report. The MCP
    /// write surface exposes this only through its hidden write opt-in; this
    /// Runtime method remains a prepare only: it never confirms, versions,
    /// exports, or advances the production head.
    pub fn candidate_topology_quality_prepare(
        &self,
        request: Value,
    ) -> Result<Value, RuntimeError> {
        let binding = request_binding(&request)?;
        if let Some(existing) = self
            .store
            .get_candidate_topology_quality(&binding.topology_quality_id)?
        {
            if existing.project_id != binding.project_id
                || existing.candidate_id != binding.candidate_id
                || existing.request_sha256 != binding.request_sha256
                || existing.input_sha256 != binding.input_sha256
            {
                return Err(topology_error(
                    "immutable topology quality id is already retargeted",
                ));
            }
            return result_value(&existing, true, PREPARE_RESULT_SCHEMA, true);
        }

        let (candidate, evidence, readback, inspection) =
            validate_current_bindings(self, &binding)?;
        let computed_part_inventory = part_inventory_sha256(&readback)?;
        if computed_part_inventory != binding.part_inventory_sha256 {
            return Err(topology_error(
                "part_inventory_sha256 differs from ArtifactReadback",
            ));
        }
        let readback_part_ids = readback
            .get("part_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| topology_error("ArtifactReadback part_ids is missing"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| is_opaque_id(value))
                    .map(str::to_owned)
                    .ok_or_else(|| topology_error("ArtifactReadback part_id is invalid"))
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        if readback_part_ids != binding.part_ids {
            return Err(topology_error(
                "requested Part inventory differs from ArtifactReadback",
            ));
        }

        let mut snapshots = Vec::with_capacity(binding.part_ids.len());
        for part_id in &binding.part_ids {
            let value = self
                .topology_snapshot(
                    &binding.project_id,
                    &binding.artifact_sha256,
                    &binding.candidate_id,
                    part_id,
                    &binding.artifact_readback_sha256,
                    &binding.geometry_program_sha256,
                    &binding.operator_catalog_sha256,
                    &binding.readback_config_sha256,
                    &sha256_hex(TOPOLOGY_SNAPSHOT_POLICY.as_bytes()),
                    MAX_SNAPSHOT_FACES,
                )
                .map_err(|error| {
                    RuntimeError::InvalidInput(format!(
                        "TOPOLOGY_QUALITY_BLOCKED: Part {part_id} could not produce a bounded TopologySnapshot: {error}"
                    ))
                })?;
            verify_output_canonical_hash(&value, "TopologySnapshot@1")?;
            let canonical_sha256 = value
                .get("canonical_sha256")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| topology_error("TopologySnapshot canonical hash is missing"))?
                .to_owned();
            let bytes = canonical_json_bytes(&value).map_err(|error| {
                topology_error(format!("TopologySnapshot JSON is invalid: {error}"))
            })?;
            if bytes.len() > MAX_JSON_BYTES as usize {
                return Err(topology_error(
                    "TopologySnapshot exceeds the 1 MiB CAS bound",
                ));
            }
            snapshots.push(SnapshotAnalysis {
                value,
                canonical_sha256,
            });
        }
        let snapshot_hashes = snapshots
            .iter()
            .map(|snapshot| snapshot.canonical_sha256.clone())
            .collect::<Vec<_>>();
        if snapshot_hashes != binding.requested_snapshot_hashes {
            return Err(topology_error(
                "requested TopologySnapshot hashes differ from Runtime replay",
            ));
        }

        let (authoring_status, authoring_hashes) =
            load_authoring_hashes(self, &binding, &evidence, &readback)?;
        if authoring_status != binding.authoring_topology_status
            || authoring_hashes != binding.requested_authoring_hashes
        {
            return Err(topology_error(
                "authoring topology status or hashes differ from Runtime replay",
            ));
        }
        let metrics = snapshot_metrics(&inspection, &readback, &snapshots)?;
        if metrics.part_count == 0 || metrics.part_count > MAX_PARTS as u64 {
            return Err(topology_error(
                "candidate Part count is outside the bounded policy",
            ));
        }
        let created_at = candidate.updated_at.clone();
        let record = build_record(
            &binding,
            metrics,
            &snapshots,
            authoring_status,
            authoring_hashes,
            created_at,
        )?;
        let record_value = record_value(&record)?;
        let record_bytes = canonical_json_bytes(&record_value).map_err(|error| {
            topology_error(format!("CandidateTopologyQuality JSON is invalid: {error}"))
        })?;
        if record_bytes.len() > MAX_JSON_BYTES as usize {
            return Err(topology_error(
                "CandidateTopologyQuality exceeds the 1 MiB CAS bound",
            ));
        }

        let reservation = self.store.begin_cas_reservation();
        let quality_object = match self.store.put_object_reserved(
            &reservation,
            &record_bytes,
            None,
            JSON_MIME,
            QUALITY_KIND,
            &record.created_at,
        ) {
            Ok(object) => object,
            Err(error) => {
                clean_reservation(self, &reservation, &[], true);
                return Err(error.into());
            }
        };
        match self
            .store
            .record_candidate_topology_quality_with_replay(&record, &quality_object.record)
        {
            Ok((stored, replayed)) => {
                clean_reservation(
                    self,
                    &reservation,
                    std::slice::from_ref(&quality_object),
                    false,
                );
                return result_value(&stored, replayed, PREPARE_RESULT_SCHEMA, true);
            }
            Err(error) => {
                clean_reservation(
                    self,
                    &reservation,
                    std::slice::from_ref(&quality_object),
                    true,
                );
                return Err(error.into());
            }
        }
    }

    /// Read one immutable topology quality report. Store performs the
    /// durable CAS/FK validation; this method adds the caller scope and
    /// current-candidate binding and never repairs reachability.
    pub fn candidate_topology_quality_get(&self, request: Value) -> Result<Value, RuntimeError> {
        let (topology_quality_id, project_id, candidate_id) = get_binding(&request)?;
        let record = self
            .store
            .get_candidate_topology_quality(&topology_quality_id)?
            .ok_or_else(|| topology_error("candidate topology quality is unavailable"))?;
        if record.project_id != project_id || record.candidate_id != candidate_id {
            return Err(topology_error("candidate topology quality scope differs"));
        }
        let candidate = self
            .candidate(&candidate_id)?
            .ok_or_else(|| topology_error("candidate is unavailable"))?;
        if candidate.project_id != project_id
            || candidate.canonical_sha256 != record.candidate_state_sha256
        {
            return Err(topology_error(
                "candidate topology quality state binding differs",
            ));
        }
        Ok(result_value(&record, true, GET_RESULT_SCHEMA, false)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passing_metrics() -> CandidateTopologyQualityMetrics {
        CandidateTopologyQualityMetrics {
            invalid_index_count: 0,
            non_finite_count: 0,
            degenerate_triangle_count: 0,
            boundary_edge_count: 0,
            non_manifold_edge_count: 0,
            orientation_conflict_count: 0,
            winding_error_count: 0,
            part_count: 1,
            solid_part_count: 1,
            non_solid_part_count: 0,
            solid_boundary_violation_count: 0,
            triangle_count: 1,
            vertex_count: 3,
            edge_count: 3,
            min_triangle_area_m2: 1.0,
            max_triangle_aspect_ratio: 1.0,
            max_vertex_valence: 2,
            normal_non_finite_count: 0,
            normal_non_unit_count: 0,
            normal_alignment_error_count: 0,
            uv_non_finite_count: 0,
            uv_degenerate_triangle_count: 0,
            tangent_non_finite_count: 0,
            tangent_orthogonality_error_count: 0,
            tangent_handedness_error_count: 0,
            semantic_part_coverage: 1.0,
            semantic_material_zone_coverage: 1.0,
            semantic_source_node_coverage: 1.0,
        }
    }

    #[test]
    fn hard_gate_accepts_bounded_structural_metrics() {
        assert!(all_gate_fields(&hard_gate(&passing_metrics())));
    }

    #[test]
    fn hard_gate_blocks_solid_boundary_and_aspect_budget() {
        let mut metrics = passing_metrics();
        metrics.solid_boundary_violation_count = 1;
        metrics.max_triangle_aspect_ratio = MAX_TRIANGLE_ASPECT_RATIO + 1.0;
        let gate = hard_gate(&metrics);
        assert!(!gate.boundary_policy);
        assert!(!gate.triangle_aspect_ratio);
        assert!(!all_gate_fields(&gate));
    }
}
