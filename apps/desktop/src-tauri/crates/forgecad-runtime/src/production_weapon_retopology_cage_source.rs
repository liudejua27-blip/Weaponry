//! Runtime-owned, source-only Low retopology and topology-correspondent Cage.
//!
//! This is deliberately separate from the formal High->Low Bake records.  It
//! consumes one already admitted High artifact, runs the two closed geometry
//! Worker operations twice, and commits only a non-promoting source bundle.
//! It is not artist-authored topology, a bake, a stage transition, or a visual
//! quality claim.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, sha256_hex,
    strict_glb_inspection, Runtime, RuntimeError, MAX_DERIVED_JSON_BYTES,
    MAX_GEOMETRY_ARTIFACT_BYTES,
};
use base64::Engine;
use forgecad_contracts::{
    PRODUCTION_WEAPON_CAGE_ARTIFACT_KIND, PRODUCTION_WEAPON_CAGE_ARTIFACT_RECEIPT_KIND,
    PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND, PRODUCTION_WEAPON_HIGH_ARTIFACT_RECEIPT_KIND,
    PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_KIND,
    PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_POLICY,
    PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_SCHEMA_VERSION, PRODUCTION_WEAPON_LOW_ARTIFACT_KIND,
    PRODUCTION_WEAPON_LOW_ARTIFACT_RECEIPT_KIND,
};
use forgecad_store::{CasObject, CasReservation};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const PREPARE_SCHEMA: &str = "ProductionWeaponRetopologyCageSourceBundlePrepareRequest@1";
const GET_SCHEMA: &str = "ProductionWeaponRetopologyCageSourceBundleGetRequest@1";
const PREPARE_RESULT_SCHEMA: &str = "ProductionWeaponRetopologyCageSourceBundlePrepareResult@1";
const GET_RESULT_SCHEMA: &str = "ProductionWeaponRetopologyCageSourceBundleGetResult@1";
const BUNDLE_SCHEMA: &str = "ProductionWeaponRetopologyCageSourceBundle@1";
const RECEIPT_SCHEMA: &str = "ProductionWeaponRetopologyCageSourceBundleReceipt@1";
const POLICY: &str = "bounded-low-retopology-topology-correspondent-cage-source-only@1";
const LOW_WORKER_POLICY: &str = "bounded-closed-manifold-triangulated-edge-collapse@1";
const LOW_WORKER_ALGORITHM: &str = "deterministic-shortest-safe-edge-collapse@1";
const CAGE_WORKER_POLICY: &str = "exact-low-topology-per-vertex-normal-offset@1";
const CAGE_WORKER_ALGORITHM: &str = "deterministic-welded-area-normal-offset@1";
const STATUS: &str = "runtime-owned-durable-production-weapon-retopology-cage-source-bundle";
const JSON_MIME: &str = "application/json";
const GLB_MIME: &str = "model/gltf-binary";
const LOW_MESH_KIND: &str = "production-weapon-low-mesh";
const OFFSET_FIELD_KIND: &str = "production-weapon-cage-offset-field";
const BUNDLE_RECEIPT_KIND: &str = "production-weapon-retopology-cage-source-bundle-receipt";
const LOW_MESH_SCHEMA: &str = "ProductionWeaponLowMesh@1";
const OFFSET_FIELD_SCHEMA: &str = "ProductionWeaponCageOffsetField@1";
const LOW_READBACK_SCHEMA: &str = "ProductionWeaponLowArtifactReadback@1";
const CAGE_READBACK_SCHEMA: &str = "ProductionWeaponCageArtifactReadback@1";
// Whole-assembly Low correspondence repeats stable Part/vertex lineage for
// every exported hard-edge/UV-seam vertex. Keep it bounded independently from
// small generic derived records; 8 MiB covers the admitted production asset
// cohort without opening an unbounded JSON path.
const MAX_SOURCE_BUNDLE_JSON_BYTES: u64 = 8 * 1024 * 1024;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "bundle_key_sha256",
    "project_id",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "source_high_artifact_sha256",
    "source_high_artifact_readback_object_sha256",
    "target_triangle_count",
    "max_collapses",
    "locked_vertices",
    "offset_m",
    "max_offset_m",
    "max_coordinate_abs_m",
    "low_retopology_policy",
    "cage_policy",
    "input_sha256",
    "idempotency_key",
];
const GET_FIELDS: &[&str] = &[
    "schema_version",
    "bundle_key_sha256",
    "project_id",
    "source_candidate_id",
];

#[derive(Debug, Clone)]
struct Request {
    expected_bundle_key: Option<String>,
    project_id: String,
    source_candidate_id: String,
    source_candidate_state_sha256: String,
    source_high_artifact_sha256: String,
    source_high_artifact_readback_object_sha256: String,
    target_triangle_count: u64,
    max_collapses: u64,
    locked_vertices: Vec<Value>,
    offset_m: f64,
    max_offset_m: f64,
    max_coordinate_abs_m: f64,
    request_sha256: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "RETOPOLOGY_CAGE_SOURCE_INVALID: {}",
        message.into()
    ))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} must be an object")))?;
    if object.len() != fields.len()
        || fields.iter().any(|field| !object.contains_key(*field))
        || object.keys().any(|key| !fields.contains(&key.as_str()))
    {
        return Err(invalid(format!("{label} contains an unknown field")));
    }
    Ok(object)
}

fn required_id(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|v| is_opaque_id(v))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} is not an opaque id")))
}

fn required_hash(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|v| is_sha256(v))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} is not a SHA-256")))
}

fn required_f64(
    object: &Map<String, Value>,
    field: &str,
    positive: bool,
) -> Result<f64, RuntimeError> {
    let value = object
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid(format!("{field} is not finite")))?;
    if !value.is_finite() || (positive && value <= 0.0) || (!positive && value < 0.0) {
        return Err(invalid(format!("{field} is outside its bound")));
    }
    Ok(value)
}

fn required_u64(object: &Map<String, Value>, field: &str) -> Result<u64, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .filter(|v| *v > 0 && *v <= 1_000_000)
        .ok_or_else(|| invalid(format!("{field} is outside its bound")))
}

fn parse_prepare(value: &Value) -> Result<Request, RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS, PREPARE_SCHEMA)?;
    if object.get("schema_version").and_then(Value::as_str) != Some(PREPARE_SCHEMA) {
        return Err(invalid("prepare schema differs"));
    }
    let expected_bundle_key = match object.get("bundle_key_sha256") {
        None | Some(Value::Null) => None,
        Some(_) => Some(required_hash(object, "bundle_key_sha256")?),
    };
    let project_id = required_id(object, "project_id")?;
    let source_candidate_id = required_id(object, "source_candidate_id")?;
    let source_candidate_state_sha256 = required_hash(object, "source_candidate_state_sha256")?;
    let source_high_artifact_sha256 = required_hash(object, "source_high_artifact_sha256")?;
    let source_high_artifact_readback_object_sha256 =
        required_hash(object, "source_high_artifact_readback_object_sha256")?;
    let target_triangle_count = required_u64(object, "target_triangle_count")?;
    let max_collapses = required_u64(object, "max_collapses")?;
    if target_triangle_count == 0 || max_collapses == 0 {
        return Err(invalid("retopology budget is empty"));
    }
    let locked_vertices = object
        .get("locked_vertices")
        .and_then(Value::as_array)
        .filter(|values| values.len() <= 16_384)
        .ok_or_else(|| invalid("locked_vertices is invalid"))?
        .clone();
    for item in &locked_vertices {
        let item = item
            .as_object()
            .ok_or_else(|| invalid("locked vertex is not an object"))?;
        if item.len() != 2
            || item
                .get("primitive_ordinal")
                .and_then(Value::as_u64)
                .is_none()
            || item.get("vertex_index").and_then(Value::as_u64).is_none()
        {
            return Err(invalid("locked vertex fields are invalid"));
        }
    }
    let offset_m = required_f64(object, "offset_m", true)?;
    let max_offset_m = required_f64(object, "max_offset_m", true)?;
    let max_coordinate_abs_m = required_f64(object, "max_coordinate_abs_m", true)?;
    if offset_m > max_offset_m || max_offset_m > 1.0 || max_coordinate_abs_m > 1_000.0 {
        return Err(invalid("cage offset bounds are invalid"));
    }
    if object.get("low_retopology_policy").and_then(Value::as_str) != Some(POLICY)
        || object.get("cage_policy").and_then(Value::as_str) != Some(POLICY)
    {
        return Err(invalid("source-only policies differ"));
    }
    let input_sha256 = required_hash(object, "input_sha256")?;
    required_id(object, "idempotency_key")?;
    let mut input = value.clone();
    let input_object = input.as_object_mut().expect("prepare object");
    input_object.remove("input_sha256");
    input_object.remove("idempotency_key");
    if canonical_json_hash(&input) != input_sha256 {
        return Err(invalid("input_sha256 does not bind the request"));
    }
    Ok(Request {
        expected_bundle_key,
        project_id,
        source_candidate_id,
        source_candidate_state_sha256,
        source_high_artifact_sha256,
        source_high_artifact_readback_object_sha256,
        target_triangle_count,
        max_collapses,
        locked_vertices,
        offset_m,
        max_offset_m,
        max_coordinate_abs_m,
        request_sha256: input_sha256,
    })
}

fn parse_get(value: &Value) -> Result<(String, String, String), RuntimeError> {
    let object = exact_object(value, GET_FIELDS, GET_SCHEMA)?;
    if object.get("schema_version").and_then(Value::as_str) != Some(GET_SCHEMA) {
        return Err(invalid("get schema differs"));
    }
    Ok((
        required_hash(object, "bundle_key_sha256")?,
        required_id(object, "project_id")?,
        required_id(object, "source_candidate_id")?,
    ))
}

fn source_preflight(
    runtime: &Runtime,
    request: &Request,
) -> Result<(Vec<u8>, String), RuntimeError> {
    let candidate = runtime
        .candidate(&request.source_candidate_id)?
        .ok_or_else(|| invalid("source candidate is unavailable"))?;
    let _prepared_object_id = candidate
        .prepared_object_id
        .as_deref()
        .filter(|id| is_opaque_id(id))
        .ok_or_else(|| invalid("source candidate prepared object id is unavailable"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != request.source_candidate_state_sha256
        || candidate.prepared_object_sha256.as_deref()
            != Some(request.source_high_artifact_sha256.as_str())
    {
        return Err(invalid(
            "source candidate/project/state/high artifact binding differs",
        ));
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&request.source_candidate_id)?
        .ok_or_else(|| invalid("source geometry evidence is unavailable"))?;
    if evidence.project_id != request.project_id
        || evidence.artifact_object_sha256 != request.source_high_artifact_sha256
        || evidence.artifact_readback_object_sha256
            != request.source_high_artifact_readback_object_sha256
    {
        return Err(invalid("source candidate evidence binding differs"));
    }
    let high_object = runtime
        .store
        .get_object(&request.source_high_artifact_sha256)?
        .ok_or_else(|| invalid("source High object is unavailable"))?;
    if high_object.mime != GLB_MIME
        || !matches!(
            high_object.kind.as_str(),
            "geometry-glb"
                | "appearance-glb"
                | "appearance-v2-glb"
                | PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND
        )
        || high_object.size_bytes == 0
        || high_object.size_bytes > MAX_GEOMETRY_ARTIFACT_BYTES
    {
        return Err(invalid("source High CAS metadata differs"));
    }
    let readback_object = runtime
        .store
        .get_object(&request.source_high_artifact_readback_object_sha256)?
        .ok_or_else(|| invalid("source High readback object is unavailable"))?;
    if readback_object.mime != JSON_MIME
        || !matches!(
            readback_object.kind.as_str(),
            "geometry-artifact-readback-v2"
                | "appearance-v2-artifact-readback"
                | PRODUCTION_WEAPON_HIGH_ARTIFACT_RECEIPT_KIND
        )
    {
        return Err(invalid("source High readback metadata differs"));
    }
    let bytes = runtime.cas_read_bounded(
        &request.source_high_artifact_sha256,
        MAX_GEOMETRY_ARTIFACT_BYTES,
    )?;
    let inspection = strict_glb_inspection(&bytes)?;
    if sha256_hex(&bytes) != request.source_high_artifact_sha256 || !inspection.hard_gate_passed {
        return Err(invalid("source High GLB strict readback failed"));
    }
    let replay_readback = if high_object.kind == PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND {
        let mut value = super::artifact_readback_v2_value(
            &request.source_high_artifact_sha256,
            &request.source_candidate_id,
            &inspection,
            high_object.size_bytes,
        );
        value["object_sha256"] = Value::String(request.source_high_artifact_sha256.clone());
        value["canonical_sha256"] = Value::String(String::new());
        let canonical = canonical_json_hash(&value);
        value["canonical_sha256"] = Value::String(canonical);
        value
    } else {
        runtime.artifact_readback(
            &request.source_high_artifact_sha256,
            &request.source_candidate_id,
        )?
    };
    validate_artifact_readback_binding(
        &replay_readback,
        &request.source_high_artifact_sha256,
        &request.source_candidate_id,
    )?;
    let readback_bytes = runtime.cas_read_bounded(
        &request.source_high_artifact_readback_object_sha256,
        MAX_DERIVED_JSON_BYTES,
    )?;
    let readback: Value = serde_json::from_slice(&readback_bytes)
        .map_err(|_| invalid("source High readback JSON is invalid"))?;
    if readback != replay_readback
        || readback
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|hash| is_sha256(hash))
            .is_none()
    {
        return Err(invalid("source High readback canonical is missing"));
    }
    Ok((bytes, candidate.canonical_sha256))
}

fn validate_artifact_readback_binding(
    readback: &Value,
    artifact_sha256: &str,
    candidate_id: &str,
) -> Result<(), RuntimeError> {
    if readback.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
        || readback.get("artifact_id").and_then(Value::as_str) != Some(artifact_sha256)
        || readback.get("object_sha256").and_then(Value::as_str) != Some(artifact_sha256)
        || readback.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || readback.get("mime").and_then(Value::as_str) != Some(GLB_MIME)
        || readback.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
        || readback.get("validator_status").and_then(Value::as_str) != Some("passed")
    {
        return Err(invalid(
            "source ArtifactReadback@2 binding or hard gate differs",
        ));
    }
    Ok(())
}

fn worker_request_low(request: &Request, high: &[u8]) -> Result<Value, RuntimeError> {
    let mut value = json!({
        "schema_version":"LowRetopologyWorkerRequest@1", "preview_only":true,
        "source_high_artifact_sha256":request.source_high_artifact_sha256,
        "high_glb_base64":base64::engine::general_purpose::STANDARD.encode(high),
        "target_triangle_count":request.target_triangle_count, "max_collapses":request.max_collapses,
        "locked_vertices":request.locked_vertices, "retopology_policy":LOW_WORKER_POLICY,
        "algorithm":LOW_WORKER_ALGORITHM, "canonical_sha256":""
    });
    let mut preimage = value.clone();
    preimage
        .as_object_mut()
        .expect("Low Worker request object")
        .remove("canonical_sha256");
    let hash = canonical_json_hash(&preimage);
    value["canonical_sha256"] = Value::String(hash);
    Ok(value)
}

fn worker_request_cage(
    request: &Request,
    low_sha: &str,
    low: &[u8],
) -> Result<Value, RuntimeError> {
    let mut value = json!({
        "schema_version":"CageOffsetWorkerRequest@1", "preview_only":true,
        "source_low_artifact_sha256":low_sha,
        "low_glb_base64":base64::engine::general_purpose::STANDARD.encode(low),
        "offset_m":request.offset_m, "max_offset_m":request.max_offset_m,
        "max_coordinate_abs_m":request.max_coordinate_abs_m,
        "offset_field_policy":CAGE_WORKER_POLICY,
        "algorithm":CAGE_WORKER_ALGORITHM, "canonical_sha256":""
    });
    let mut preimage = value.clone();
    preimage
        .as_object_mut()
        .expect("Cage Worker request object")
        .remove("canonical_sha256");
    let hash = canonical_json_hash(&preimage);
    value["canonical_sha256"] = Value::String(hash);
    Ok(value)
}

fn result_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| invalid(format!("Worker result missing {field}")))
}

fn validate_worker_result(
    result: &Value,
    schema: &str,
    operation: &str,
    source_field: &str,
    source_sha256: &str,
    policy_field: &str,
    policy: &str,
    algorithm: &str,
    low: bool,
) -> Result<(), RuntimeError> {
    let object = result
        .as_object()
        .ok_or_else(|| invalid("Worker result is not an object"))?;
    for (field, expected) in [
        ("schema_version", schema),
        ("operation", operation),
        (source_field, source_sha256),
        (policy_field, policy),
        ("algorithm", algorithm),
        ("quality_status", "structural_only"),
    ] {
        if object.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(invalid(format!("Worker result {field} binding differs")));
        }
    }
    for field in [
        "runtime_write_performed",
        "production_stage_advanced",
        "promotion_eligible",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        if object.get(field).and_then(Value::as_bool) != Some(false) {
            return Err(invalid(format!("Worker result {field} flag differs")));
        }
    }
    if low {
        if object.get("retopology_derived").and_then(Value::as_bool) != Some(true)
            || object
                .get("artist_authored_quad_topology")
                .and_then(Value::as_bool)
                != Some(false)
            || object.get("edge_flow_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        {
            return Err(invalid("Low Worker result source-only flags differ"));
        }
    } else if object
        .get("exact_topology_correspondence")
        .and_then(Value::as_bool)
        != Some(true)
        || object.get("offset_field_derived").and_then(Value::as_bool) != Some(true)
        || object.get("containment_status").and_then(Value::as_str)
            != Some("STRUCTURAL_OFFSET_ONLY")
    {
        return Err(invalid("Cage Worker result source-only flags differ"));
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| is_sha256(hash))
        .ok_or_else(|| invalid("Worker result canonical hash is invalid"))?;
    let mut normalized = result.clone();
    normalized["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&normalized) != canonical {
        return Err(invalid(format!(
            "{schema} Worker result canonical hash differs"
        )));
    }
    Ok(())
}

fn worker_cohort(
    worker: &super::geometry_worker::SiblingWorkerResult,
) -> Result<&str, RuntimeError> {
    worker
        .build_cohort_sha256
        .as_deref()
        .filter(|cohort| is_sha256(cohort))
        .ok_or_else(|| invalid("Worker build cohort is missing or invalid"))
}

#[derive(Debug, Clone)]
struct ClosedCorrespondenceSummary {
    part_pairs: Value,
    part_ids: Vec<String>,
    material_zone_ids: Vec<String>,
    mapping_sha256: String,
}

fn exact_mapping_object<'a>(
    value: &'a Value,
    fields: &[&str],
    label: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{label} is not an object")))?;
    if object.len() != fields.len()
        || object.keys().any(|key| !fields.contains(&key.as_str()))
        || fields.iter().any(|field| !object.contains_key(*field))
    {
        return Err(invalid(format!(
            "{label} contains an unknown or missing field"
        )));
    }
    Ok(object)
}

fn mapping_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invalid(format!("correspondence {field} is missing")))
}

fn mapping_u64(object: &Map<String, Value>, field: &str) -> Result<u64, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("correspondence {field} is invalid")))
}

fn mapping_vec3(value: &Value, label: &str) -> Result<[f64; 3], RuntimeError> {
    let values = value
        .as_array()
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid(format!("{label} must be a three-component vector")))?;
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        result[index] = value
            .as_f64()
            .filter(|value| value.is_finite())
            .ok_or_else(|| invalid(format!("{label} contains a non-finite component")))?;
    }
    Ok(result)
}

fn vector_distance(left: [f64; 3], right: [f64; 3]) -> f64 {
    ((left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2) + (left[2] - right[2]).powi(2))
        .sqrt()
}

fn normalize_source_bundle_numbers(value: &mut Value) {
    match value {
        Value::Number(number) if number.is_f64() => {
            if let Some(number) = number.as_f64() {
                let rounded = (number * 1_000_000_000.0).round() / 1_000_000_000.0;
                let rounded = if rounded == -0.0 { 0.0 } else { rounded };
                if let Some(number) = serde_json::Number::from_f64(rounded) {
                    *value = Value::Number(number);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                normalize_source_bundle_numbers(value);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                normalize_source_bundle_numbers(value);
            }
        }
        _ => {}
    }
}

fn validate_closed_high_low_cage(
    high: &super::integrity::GlbIntegrity,
    low: &super::integrity::GlbIntegrity,
    cage: &super::integrity::GlbIntegrity,
    mapping: &Value,
    cage_mesh: Option<&Value>,
    offset_field: &Value,
    max_offset_m: f64,
    mapping_expected_sha256: Option<&str>,
) -> Result<ClosedCorrespondenceSummary, RuntimeError> {
    if !high.hard_gate_passed || !low.hard_gate_passed || !cage.hard_gate_passed {
        return Err(invalid(
            "High/Low/Cage strict readback is not admissible for correspondence",
        ));
    }
    if high.part_bindings.is_empty()
        || high.part_bindings.len() != low.part_bindings.len()
        || low.part_bindings.len() != cage.part_bindings.len()
        || low.triangle_count != cage.triangle_count
    {
        return Err(invalid(
            "High/Low/Cage primitive binding counts differ for correspondence",
        ));
    }
    let mapping = mapping
        .as_array()
        .filter(|values| !values.is_empty())
        .ok_or_else(|| invalid("High/Low correspondence mapping is empty"))?;
    if mapping.len() != high.part_bindings.len() {
        return Err(invalid(
            "High/Low correspondence has no one-to-one primitive binding",
        ));
    }
    let cage_mesh = match cage_mesh {
        None => None,
        Some(value) => Some(
            value
                .as_array()
                .filter(|values| values.len() == mapping.len())
                .ok_or_else(|| invalid("Cage mesh primitive correspondence is invalid"))?,
        ),
    };
    let offset_field = offset_field
        .as_array()
        .ok_or_else(|| invalid("Cage offset field is not an array"))?;
    let mut offset_cursor = 0usize;
    let mut part_pairs = Vec::with_capacity(mapping.len());
    let mut part_ids = Vec::with_capacity(mapping.len());
    let mut material_zone_ids = Vec::new();
    let mut seen_parts = BTreeSet::new();
    let mut seen_material_zones = BTreeSet::new();

    for (primitive_ordinal, mapping_value) in mapping.iter().enumerate() {
        let mapping_object = exact_mapping_object(
            mapping_value,
            &[
                "part_id",
                "source_node_id",
                "material_zone_id",
                "solid",
                "positions",
                "indices",
                "vertex_correspondence",
                "face_correspondence",
            ],
            "Low primitive correspondence",
        )?;
        let high_binding = high
            .part_bindings
            .get(primitive_ordinal)
            .ok_or_else(|| invalid("High primitive binding is missing"))?;
        let low_binding = low
            .part_bindings
            .get(primitive_ordinal)
            .ok_or_else(|| invalid("Low primitive binding is missing"))?;
        let cage_binding = cage
            .part_bindings
            .get(primitive_ordinal)
            .ok_or_else(|| invalid("Cage primitive binding is missing"))?;
        let part_id = mapping_string(mapping_object, "part_id")?;
        let source_node_id = mapping_string(mapping_object, "source_node_id")?;
        let material_zone_id = mapping_string(mapping_object, "material_zone_id")?;
        let solid = mapping_object
            .get("solid")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("Low primitive solid binding is invalid"))?;
        if part_id != high_binding.part_id
            || part_id != low_binding.part_id
            || part_id != cage_binding.part_id
            || source_node_id != high_binding.source_node_id
            || source_node_id != low_binding.source_node_id
            || source_node_id != cage_binding.source_node_id
            || material_zone_id != high_binding.material_zone_id
            || material_zone_id != low_binding.material_zone_id
            || material_zone_id != cage_binding.material_zone_id
            || solid != high_binding.solid
            || solid != low_binding.solid
            || solid != cage_binding.solid
        {
            return Err(invalid(format!(
                "High/Low/Cage cross-Part or cross-material correspondence at primitive {primitive_ordinal}"
            )));
        }
        if seen_parts.insert(part_id.to_owned()) {
            part_ids.push(part_id.to_owned());
        }
        if seen_material_zones.insert(material_zone_id.to_owned()) {
            material_zone_ids.push(material_zone_id.to_owned());
        }

        let positions = mapping_object
            .get("positions")
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty())
            .ok_or_else(|| invalid("Low primitive positions are missing"))?;
        let indices = mapping_object
            .get("indices")
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty() && values.len() % 3 == 0)
            .ok_or_else(|| invalid("Low primitive indices are invalid"))?;
        if indices.len() / 3 != low_binding.triangle_count as usize
            || indices.iter().any(|index| {
                index
                    .as_u64()
                    .is_none_or(|index| index >= positions.len() as u64)
            })
        {
            return Err(invalid(format!(
                "Low primitive topology/count is not bound at primitive {primitive_ordinal}"
            )));
        }
        for (vertex_index, position) in positions.iter().enumerate() {
            let position = mapping_vec3(position, "Low primitive position")?;
            let cage_position = cage_mesh
                .as_ref()
                .map(|meshes| {
                    let object = exact_mapping_object(
                        &meshes[primitive_ordinal],
                        &[
                            "part_id",
                            "source_node_id",
                            "material_zone_id",
                            "solid",
                            "positions",
                            "indices",
                        ],
                        "Cage primitive",
                    )?;
                    if mapping_string(object, "part_id")? != part_id
                        || mapping_string(object, "source_node_id")? != source_node_id
                        || mapping_string(object, "material_zone_id")? != material_zone_id
                        || object.get("solid").and_then(Value::as_bool) != Some(solid)
                        || object.get("indices") != Some(&Value::Array(indices.clone()))
                    {
                        return Err(invalid(format!(
                            "Cage primitive topology/identity differs at primitive {primitive_ordinal}"
                        )));
                    }
                    let positions = object
                        .get("positions")
                        .and_then(Value::as_array)
                        .filter(|values| values.len() == positions.len())
                        .ok_or_else(|| invalid("Cage primitive positions are invalid"))?;
                    mapping_vec3(
                        positions.get(vertex_index).ok_or_else(|| {
                            invalid("Cage primitive vertex correspondence is missing")
                        })?,
                        "Cage primitive position",
                    )
                })
                .transpose()?;
            if let Some(cage_position) = cage_position {
                let displacement = vector_distance(position, cage_position);
                if !displacement.is_finite() || displacement <= 1.0e-9 {
                    return Err(invalid(format!(
                        "Cage displacement is missing at primitive {primitive_ordinal} vertex {vertex_index}"
                    )));
                }
            }
        }

        let vertex_correspondence = mapping_object
            .get("vertex_correspondence")
            .and_then(Value::as_array)
            .filter(|values| values.len() == positions.len())
            .ok_or_else(|| invalid("Low vertex correspondence is incomplete"))?;
        for (expected_index, value) in vertex_correspondence.iter().enumerate() {
            let object = exact_mapping_object(
                value,
                &["low_vertex_index", "source_vertex_indices"],
                "Low vertex correspondence",
            )?;
            if mapping_u64(object, "low_vertex_index")? != expected_index as u64 {
                return Err(invalid("Low vertex correspondence ordering differs"));
            }
            let source_indices = object
                .get("source_vertex_indices")
                .and_then(Value::as_array)
                .filter(|values| !values.is_empty())
                .ok_or_else(|| invalid("Low vertex correspondence is unmapped"))?;
            let mut seen_source_indices = BTreeSet::new();
            for source_index in source_indices {
                let source_index = source_index
                    .as_u64()
                    .ok_or_else(|| invalid("Low vertex correspondence index is invalid"))?;
                let max_source_vertex_count = high_binding
                    .triangle_count
                    .checked_mul(3)
                    .ok_or_else(|| invalid("High primitive vertex bound overflowed"))?;
                if source_index >= max_source_vertex_count {
                    return Err(invalid(
                        "Low vertex correspondence source index is out of High primitive bounds",
                    ));
                }
                if !seen_source_indices.insert(source_index) {
                    return Err(invalid("Low vertex correspondence is ambiguous"));
                }
            }
        }

        let face_correspondence = mapping_object
            .get("face_correspondence")
            .and_then(Value::as_array)
            .filter(|values| values.len() == indices.len() / 3)
            .ok_or_else(|| invalid("Low face correspondence is incomplete"))?;
        for (expected_index, value) in face_correspondence.iter().enumerate() {
            let object = exact_mapping_object(
                value,
                &["low_face_index", "source_face_index"],
                "Low face correspondence",
            )?;
            if mapping_u64(object, "low_face_index")? != expected_index as u64
                || mapping_u64(object, "source_face_index")? >= high_binding.triangle_count
            {
                return Err(invalid(
                    "Low face correspondence is unmapped or out of range",
                ));
            }
        }

        let cage_mesh_object = cage_mesh.as_ref().map(|meshes| {
            exact_mapping_object(
                &meshes[primitive_ordinal],
                &[
                    "part_id",
                    "source_node_id",
                    "material_zone_id",
                    "solid",
                    "positions",
                    "indices",
                ],
                "Cage primitive",
            )
        });
        if let Some(cage_mesh_object) = cage_mesh_object {
            let cage_mesh_object = cage_mesh_object?;
            if cage_mesh_object.get("indices") != Some(&Value::Array(indices.clone())) {
                return Err(invalid("Cage index correspondence differs"));
            }
        }

        let expected_position_count = positions.len();
        let end = offset_cursor
            .checked_add(expected_position_count)
            .ok_or_else(|| invalid("Cage offset field count overflowed"))?;
        let entries = offset_field
            .get(offset_cursor..end)
            .ok_or_else(|| invalid("Cage offset field has missing vertices"))?;
        for (vertex_index, entry) in entries.iter().enumerate() {
            let entry = exact_mapping_object(
                entry,
                &[
                    "primitive_ordinal",
                    "vertex_index",
                    "part_id",
                    "source_position",
                    "normal",
                    "offset_m",
                    "derived_position",
                ],
                "Cage offset entry",
            )?;
            if mapping_u64(entry, "primitive_ordinal")? != primitive_ordinal as u64
                || mapping_u64(entry, "vertex_index")? != vertex_index as u64
                || mapping_string(entry, "part_id")? != part_id
            {
                return Err(invalid(
                    "Cage offset field has a missing or cross-Part vertex correspondence",
                ));
            }
            let source_position = mapping_vec3(
                entry
                    .get("source_position")
                    .ok_or_else(|| invalid("Cage offset source position is missing"))?,
                "Cage offset source position",
            )?;
            if vector_distance(
                source_position,
                mapping_vec3(&positions[vertex_index], "Low primitive position")?,
            ) > 1.0e-5
            {
                return Err(invalid("Cage offset source position differs from Low"));
            }
            let normal = mapping_vec3(
                entry
                    .get("normal")
                    .ok_or_else(|| invalid("Cage offset normal is missing"))?,
                "Cage offset normal",
            )?;
            let normal_length = (normal[0].powi(2) + normal[1].powi(2) + normal[2].powi(2)).sqrt();
            let offset_m = entry
                .get("offset_m")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && *value > 0.0 && *value <= max_offset_m)
                .ok_or_else(|| invalid("Cage offset magnitude is outside its bound"))?;
            if !normal_length.is_finite() || normal_length <= 1.0e-9 {
                return Err(invalid("Cage offset normal is invalid"));
            }
            let derived_position = mapping_vec3(
                entry
                    .get("derived_position")
                    .ok_or_else(|| invalid("Cage derived position is missing"))?,
                "Cage derived position",
            )?;
            if let Some(meshes) = cage_mesh.as_ref() {
                let cage_object = exact_mapping_object(
                    &meshes[primitive_ordinal],
                    &[
                        "part_id",
                        "source_node_id",
                        "material_zone_id",
                        "solid",
                        "positions",
                        "indices",
                    ],
                    "Cage primitive",
                )?;
                let cage_positions = cage_object
                    .get("positions")
                    .and_then(Value::as_array)
                    .ok_or_else(|| invalid("Cage primitive positions are missing"))?;
                if vector_distance(
                    derived_position,
                    mapping_vec3(&cage_positions[vertex_index], "Cage primitive position")?,
                ) > 1.0e-5
                {
                    return Err(invalid("Cage offset field does not bind Cage position"));
                }
            }
            let displacement = vector_distance(
                derived_position,
                mapping_vec3(&positions[vertex_index], "Low primitive position")?,
            );
            if !displacement.is_finite() || displacement <= 1.0e-9 {
                return Err(invalid("Cage offset does not displace the Low vertex"));
            }
            let expected_derived_position = [
                source_position[0] + normal[0] * offset_m,
                source_position[1] + normal[1] * offset_m,
                source_position[2] + normal[2] * offset_m,
            ];
            if vector_distance(derived_position, expected_derived_position) > 1.0e-4 {
                return Err(invalid(
                    "Cage offset field is not the declared normal displacement",
                ));
            }
            let _ = offset_m;
        }
        offset_cursor = end;

        let vertex_map = canonical_json_hash(mapping_object.get("vertex_correspondence").unwrap());
        let face_map = canonical_json_hash(mapping_object.get("face_correspondence").unwrap());
        part_pairs.push(json!({
            "part_id":part_id,
            "high_part_id":high_binding.part_id,
            "low_part_id":low_binding.part_id,
            "cage_part_id":cage_binding.part_id,
            "material_zone_id":material_zone_id,
            "high_source_node_id":high_binding.source_node_id,
            "low_source_node_id":low_binding.source_node_id,
            "cage_source_node_id":cage_binding.source_node_id,
            "high_face_count":high_binding.triangle_count,
            "low_face_count":low_binding.triangle_count,
            "cage_face_count":cage_binding.triangle_count,
            "vertex_map_sha256":vertex_map,
            "face_map_sha256":face_map,
            "mapping_status":"PASS_SOURCE_STRUCTURAL"
        }));
    }
    if offset_cursor != offset_field.len() {
        return Err(invalid("Cage offset field has extra vertices"));
    }
    let mapping_sha256 = canonical_json_hash(&Value::Array(mapping.to_owned()));
    if mapping_expected_sha256.is_some_and(|expected| expected != mapping_sha256) {
        return Err(invalid("High/Low correspondence mapping hash differs"));
    }
    Ok(ClosedCorrespondenceSummary {
        part_pairs: Value::Array(part_pairs),
        part_ids,
        material_zone_ids,
        mapping_sha256,
    })
}

fn validate_cage_worker_diagnostic(value: &Value) -> Result<(), RuntimeError> {
    let diagnostic = value
        .get("diagnostic")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Cage Worker diagnostic is missing"))?;
    if diagnostic.get("status").and_then(Value::as_str) != Some("NOT_RUN_NO_HIGH_REFERENCE")
        || diagnostic.get("policy").and_then(Value::as_str)
            != Some("low-topology-offset-only-no-high-ray-diagnostic@1")
        || diagnostic.get("out_of_range_count").and_then(Value::as_u64) != Some(0)
    {
        return Err(invalid(
            "Cage Worker diagnostic must remain explicitly not-run without High",
        ));
    }
    for field in [
        "self_intersection_count",
        "cross_part_count",
        "skew_count",
        "penetration_count",
    ] {
        if !diagnostic.get(field).is_some_and(Value::is_null) {
            return Err(invalid(format!(
                "Cage Worker diagnostic {field} must be unavailable without High"
            )));
        }
    }
    Ok(())
}

fn correspondence_value(
    source_high_artifact_sha256: &str,
    low_artifact_sha256: &str,
    cage_artifact_sha256: &str,
    low_mesh_object_sha256: &str,
    summary: &ClosedCorrespondenceSummary,
    mapping: &Value,
    worker_algorithm_sha256: &str,
    worker_build_cohort_sha256: &str,
) -> Value {
    json!({
        "schema_version":PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_SCHEMA_VERSION,
        "source_high_artifact_sha256":source_high_artifact_sha256,
        "low_artifact_sha256":low_artifact_sha256,
        "cage_artifact_sha256":cage_artifact_sha256,
        "correspondence_policy":PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_POLICY,
        "correspondence_policy_sha256":sha256_hex(PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_POLICY.as_bytes()),
        "stable_identity_policy":"source-primitive-part-node-material@1",
        "mapping":mapping,
        "mapping_sha256":summary.mapping_sha256,
        "mapping_object_sha256":low_mesh_object_sha256,
        "mapping_canonical_sha256":summary.mapping_sha256,
        "cage_topology_correspondence_sha256":summary.mapping_sha256,
        "part_ids":summary.part_ids,
        "material_zone_ids":summary.material_zone_ids,
        "part_pairs":summary.part_pairs,
        "unmapped_count":0,
        "ambiguous_count":0,
        "cross_part_count":0,
        "cross_material_zone_count":0,
        "mapping_status":"PASS_SOURCE_STRUCTURAL",
        "validator_status":"passed",
        "structural_status":"PASS_SOURCE_STRUCTURAL",
        "visual_status":"NOT_PROVEN",
        "human_status":"NOT_RUN",
        "engine_status":"NOT_RUN",
        "distribution_status":"NOT_RUN",
        "quality_status":"structural_only",
        "hard_gate_passed":true,
        "worker_algorithm_sha256":worker_algorithm_sha256,
        "worker_build_cohort_sha256":worker_build_cohort_sha256,
        "worker_replay_count":2,
        "replay_byte_exact":true,
        "runtime_write_performed":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    })
}

fn validate_persisted_correspondence(
    correspondence: &Value,
    summary: &ClosedCorrespondenceSummary,
    mapping: &Value,
    source_high_artifact_sha256: &str,
    low_artifact_sha256: &str,
    cage_artifact_sha256: &str,
    low_mesh_object_sha256: &str,
) -> Result<(), RuntimeError> {
    if correspondence.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_SCHEMA_VERSION)
        || correspondence
            .get("source_high_artifact_sha256")
            .and_then(Value::as_str)
            != Some(source_high_artifact_sha256)
        || correspondence
            .get("low_artifact_sha256")
            .and_then(Value::as_str)
            != Some(low_artifact_sha256)
        || correspondence
            .get("cage_artifact_sha256")
            .and_then(Value::as_str)
            != Some(cage_artifact_sha256)
        || correspondence.get("mapping") != Some(mapping)
        || correspondence.get("part_pairs") != Some(&summary.part_pairs)
        || correspondence.get("part_ids")
            != Some(&Value::Array(
                summary
                    .part_ids
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ))
        || correspondence.get("material_zone_ids")
            != Some(&Value::Array(
                summary
                    .material_zone_ids
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ))
        || correspondence.get("mapping_sha256").and_then(Value::as_str)
            != Some(summary.mapping_sha256.as_str())
        || correspondence
            .get("mapping_canonical_sha256")
            .and_then(Value::as_str)
            != Some(summary.mapping_sha256.as_str())
        || correspondence
            .get("mapping_object_sha256")
            .and_then(Value::as_str)
            != Some(low_mesh_object_sha256)
        || correspondence
            .get("cage_topology_correspondence_sha256")
            .and_then(Value::as_str)
            != Some(summary.mapping_sha256.as_str())
        || correspondence.get("unmapped_count").and_then(Value::as_u64) != Some(0)
        || correspondence
            .get("ambiguous_count")
            .and_then(Value::as_u64)
            != Some(0)
        || correspondence
            .get("cross_part_count")
            .and_then(Value::as_u64)
            != Some(0)
        || correspondence
            .get("cross_material_zone_count")
            .and_then(Value::as_u64)
            != Some(0)
        || correspondence.get("mapping_status").and_then(Value::as_str)
            != Some("PASS_SOURCE_STRUCTURAL")
        || correspondence
            .get("validator_status")
            .and_then(Value::as_str)
            != Some("passed")
        || correspondence
            .get("hard_gate_passed")
            .and_then(Value::as_bool)
            != Some(true)
        || correspondence
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid(
            "persisted High/Low correspondence is not an exact closed structural proof",
        ));
    }
    Ok(())
}

fn normalized_object(mut value: Value) -> Result<(Vec<u8>, String), RuntimeError> {
    value
        .as_object_mut()
        .ok_or_else(|| invalid("derived JSON is not an object"))?
        .insert("canonical_sha256".to_owned(), Value::String(String::new()));
    // Rebind after each JSON parse until the CAS representation validates its
    // own canonical hash. f32-origin numbers can need an additional lexical
    // normalization once the hash string itself is inserted into the object.
    let mut wire_value = value;
    for _ in 0..8 {
        let mut preimage = wire_value.clone();
        preimage["canonical_sha256"] = Value::String(String::new());
        let canonical_sha256 = canonical_json_hash(&preimage);
        wire_value["canonical_sha256"] = Value::String(canonical_sha256.clone());
        let bytes = canonical_json_bytes(&wire_value).map_err(|e| invalid(e.to_string()))?;
        if bytes.len() > MAX_SOURCE_BUNDLE_JSON_BYTES as usize {
            return Err(invalid("derived source-bundle JSON exceeds 8 MiB"));
        }
        let persisted: Value = serde_json::from_slice(&bytes)
            .map_err(|_| invalid("derived JSON persisted normalization failed"))?;
        let mut persisted_preimage = persisted.clone();
        persisted_preimage["canonical_sha256"] = Value::String(String::new());
        if persisted.get("canonical_sha256").and_then(Value::as_str)
            == Some(canonical_json_hash(&persisted_preimage).as_str())
        {
            return Ok((bytes, canonical_sha256));
        }
        wire_value = persisted;
    }
    Err(invalid(
        "derived JSON canonical wire value did not converge",
    ))
}

fn put_json(
    runtime: &Runtime,
    reservation: &CasReservation,
    value: Value,
    kind: &str,
    created_at: &str,
    objects: &mut Vec<CasObject>,
) -> Result<CasObject, RuntimeError> {
    let (bytes, _) = normalized_object(value)?;
    let object = runtime.store.put_object_reserved(
        reservation,
        &bytes,
        None,
        JSON_MIME,
        kind,
        created_at,
    )?;
    objects.push(object.clone());
    Ok(object)
}

fn json_object_sha256(value: &Value) -> Result<String, RuntimeError> {
    let (bytes, _) = normalized_object(value.clone())?;
    Ok(sha256_hex(&bytes))
}

fn release(runtime: &Runtime, reservation: &CasReservation, objects: &[CasObject], cleanup: bool) {
    for object in objects {
        let _ = runtime.store.release_cas_reservation_object(
            reservation,
            object,
            cleanup && object.created_new,
        );
    }
}

fn bundle_key(value: &Value) -> String {
    let mut normalized = value.clone();
    let object = normalized.as_object_mut().expect("bundle object");
    for field in [
        "bundle_key_sha256",
        "receipt_object_sha256",
        "canonical_sha256",
        "created_at",
    ] {
        object.insert(field.to_owned(), Value::String(String::new()));
    }
    canonical_json_hash(&normalized)
}

fn same_bundle_value(left: &Value, right: &Value) -> bool {
    let normalize = |value: &Value| {
        let mut value = value.clone();
        if let Some(object) = value.as_object_mut() {
            object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
            object.insert("created_at".to_owned(), Value::String(String::new()));
        }
        value
    };
    canonical_json_bytes(&normalize(left)).ok() == canonical_json_bytes(&normalize(right)).ok()
}

fn record_value(
    request: &Request,
    source_state: &str,
    source_readback: &str,
    low: &Value,
    cage: &Value,
    low_readback: &str,
    cage_readback: &str,
    low_mesh: &str,
    correspondence: &str,
    offset: &str,
    receipt: &str,
    created_at: &str,
) -> Value {
    let mut value = json!({
        "schema_version":BUNDLE_SCHEMA, "bundle_key_sha256":"", "project_id":request.project_id,
        "source_candidate_id":request.source_candidate_id, "source_candidate_state_sha256":source_state,
        "source_high_artifact_sha256":request.source_high_artifact_sha256,
        "source_high_artifact_readback_object_sha256":source_readback,
        "low_artifact_sha256":low["low_artifact_sha256"], "low_artifact_readback_object_sha256":low_readback,
        "cage_artifact_sha256":cage["cage_artifact_sha256"], "cage_artifact_readback_object_sha256":cage_readback,
        "low_mesh_object_sha256":low_mesh, "correspondence_object_sha256":correspondence,
        "cage_offset_field_object_sha256":offset, "receipt_object_sha256":receipt,
        "low_retopology_policy":POLICY, "cage_policy":POLICY, "source_status":STATUS,
        "quality_status":"structural_only", "visual_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN", "commercial_engine_status":"NOT_RUN",
        "runtime_write_performed":true, "production_stage_advanced":false,
        "candidate_confirmed":false, "version_created":false, "export_performed":false,
        "request_sha256":request.request_sha256, "canonical_sha256":"", "created_at":created_at
    });
    let key = bundle_key(&value);
    value["bundle_key_sha256"] = Value::String(key.clone());
    value["canonical_sha256"] = Value::String(key);
    value
}

fn make_receipt(
    record: &Value,
    request: &Request,
    low: &Value,
    cage: &Value,
    low_mesh: &str,
    correspondence: &str,
    offset: &str,
    cohort: &str,
) -> Value {
    let mut receipt = json!({
        "schema_version":RECEIPT_SCHEMA, "bundle_key_sha256":record["bundle_key_sha256"],
        "source_high_artifact_sha256":record["source_high_artifact_sha256"],
        "source_candidate_state_sha256":request.source_candidate_state_sha256,
        "source_high_artifact_readback_object_sha256":request.source_high_artifact_readback_object_sha256,
        "low_artifact_sha256":low["low_artifact_sha256"], "cage_artifact_sha256":cage["cage_artifact_sha256"],
        "low_mesh_object_sha256":low_mesh, "correspondence_object_sha256":correspondence,
        "cage_offset_field_object_sha256":offset, "low_worker_replay_count":2,
        "cage_worker_replay_count":2, "worker_build_cohort_sha256":cohort,
        "request_sha256":request.request_sha256,
        "target_triangle_count":request.target_triangle_count,
        "max_collapses":request.max_collapses,
        "locked_vertices":request.locked_vertices,
        "offset_m":request.offset_m,
        "max_offset_m":request.max_offset_m,
        "max_coordinate_abs_m":request.max_coordinate_abs_m,
        "low_result_canonical_sha256":low["canonical_sha256"],
        "cage_result_canonical_sha256":cage["canonical_sha256"],
        "low_mesh_sha256":low["low_mesh_sha256"],
        "cage_mesh_sha256":cage["cage_mesh_sha256"],
        "offset_field_sha256":cage["offset_field_sha256"],
        "quality_status":"structural_only", "visual_quality_status":"NOT_PROVEN",
        "production_stage_advanced":false, "candidate_confirmed":false,
        "version_created":false, "export_performed":false, "canonical_sha256":""
    });
    receipt["canonical_sha256"] = Value::String(canonical_json_hash(&receipt));
    receipt
}

fn read_json_cas(
    runtime: &Runtime,
    hash: &str,
    expected_kind: &str,
    expected_schema: Option<&str>,
) -> Result<Value, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid("derived JSON object is unavailable"))?;
    if object.mime != JSON_MIME
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > MAX_SOURCE_BUNDLE_JSON_BYTES
    {
        return Err(invalid("derived JSON metadata differs"));
    }
    let bytes = runtime.cas_read_bounded(hash, MAX_SOURCE_BUNDLE_JSON_BYTES)?;
    if sha256_hex(&bytes) != hash {
        return Err(invalid("derived JSON CAS hash differs"));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("derived JSON CAS bytes are invalid"))?;
    if expected_schema
        .is_some_and(|schema| value.get("schema_version").and_then(Value::as_str) != Some(schema))
    {
        return Err(invalid("derived JSON schema differs"));
    }
    let canonical = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("derived JSON canonical hash is missing"))?;
    let mut normalized = value.clone();
    normalized["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&normalized) != canonical {
        return Err(invalid("derived JSON canonical hash differs"));
    }
    Ok(value)
}

fn read_glb_cas(
    runtime: &Runtime,
    hash: &str,
    expected_kind: &str,
) -> Result<Vec<u8>, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid("derived GLB object is unavailable"))?;
    if object.mime != GLB_MIME
        || object.kind != expected_kind
        || object.size_bytes == 0
        || object.size_bytes > MAX_GEOMETRY_ARTIFACT_BYTES
    {
        return Err(invalid("derived GLB metadata differs"));
    }
    let bytes = runtime.cas_read_bounded(hash, MAX_GEOMETRY_ARTIFACT_BYTES)?;
    if sha256_hex(&bytes) != hash || !strict_glb_inspection(&bytes)?.hard_gate_passed {
        return Err(invalid("derived GLB strict readback failed"));
    }
    Ok(bytes)
}

fn validate_receipt_binding(record: &Value, receipt: &Value) -> Result<String, RuntimeError> {
    for (receipt_field, record_field) in [
        ("bundle_key_sha256", "bundle_key_sha256"),
        ("source_high_artifact_sha256", "source_high_artifact_sha256"),
        ("low_artifact_sha256", "low_artifact_sha256"),
        ("cage_artifact_sha256", "cage_artifact_sha256"),
        ("low_mesh_object_sha256", "low_mesh_object_sha256"),
        (
            "correspondence_object_sha256",
            "correspondence_object_sha256",
        ),
        (
            "cage_offset_field_object_sha256",
            "cage_offset_field_object_sha256",
        ),
    ] {
        if receipt.get(receipt_field) != record.get(record_field) {
            return Err(invalid("bundle receipt binding differs"));
        }
    }
    if receipt.get("schema_version").and_then(Value::as_str) != Some(RECEIPT_SCHEMA)
        || receipt.get("source_candidate_state_sha256")
            != record.get("source_candidate_state_sha256")
        || receipt.get("source_high_artifact_readback_object_sha256")
            != record.get("source_high_artifact_readback_object_sha256")
        || receipt.get("request_sha256") != record.get("request_sha256")
        || receipt.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || receipt.get("visual_quality_status").and_then(Value::as_str) != Some("NOT_PROVEN")
        || receipt
            .get("production_stage_advanced")
            .and_then(Value::as_bool)
            != Some(false)
        || receipt.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || receipt.get("version_created").and_then(Value::as_bool) != Some(false)
        || receipt.get("export_performed").and_then(Value::as_bool) != Some(false)
        || receipt
            .get("low_worker_replay_count")
            .and_then(Value::as_u64)
            != Some(2)
        || receipt
            .get("cage_worker_replay_count")
            .and_then(Value::as_u64)
            != Some(2)
    {
        return Err(invalid("bundle receipt request or status binding differs"));
    }
    let canonical = receipt
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("bundle receipt canonical hash is invalid"))?;
    let mut normalized = receipt.clone();
    normalized["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&normalized) != canonical {
        return Err(invalid("bundle receipt canonical hash differs"));
    }
    let cohort = receipt
        .get("worker_build_cohort_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("bundle receipt worker cohort is invalid"))?;
    Ok(cohort.to_owned())
}

fn request_from_record_and_receipt(
    record: &Value,
    receipt: &Value,
) -> Result<Request, RuntimeError> {
    let target_triangle_count = record_u64(receipt, "target_triangle_count")?;
    let max_collapses = record_u64(receipt, "max_collapses")?;
    let locked_vertices = receipt
        .get("locked_vertices")
        .and_then(Value::as_array)
        .filter(|values| values.len() <= 16_384)
        .ok_or_else(|| invalid("bundle receipt locked_vertices is invalid"))?
        .clone();
    for item in &locked_vertices {
        let item = item
            .as_object()
            .ok_or_else(|| invalid("bundle receipt locked vertex is invalid"))?;
        if item.len() != 2
            || item
                .get("primitive_ordinal")
                .and_then(Value::as_u64)
                .is_none()
            || item.get("vertex_index").and_then(Value::as_u64).is_none()
        {
            return Err(invalid("bundle receipt locked vertex fields are invalid"));
        }
    }
    let offset_m = receipt
        .get("offset_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| invalid("bundle receipt offset_m is invalid"))?;
    let max_offset_m = receipt
        .get("max_offset_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| invalid("bundle receipt max_offset_m is invalid"))?;
    let max_coordinate_abs_m = receipt
        .get("max_coordinate_abs_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| invalid("bundle receipt max_coordinate_abs_m is invalid"))?;
    if offset_m > max_offset_m || max_offset_m > 1.0 || max_coordinate_abs_m > 1_000.0 {
        return Err(invalid("bundle receipt cage bounds are invalid"));
    }
    let request_sha256 = receipt
        .get("request_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("bundle receipt request hash is invalid"))?;
    Ok(Request {
        expected_bundle_key: record
            .get("bundle_key_sha256")
            .and_then(Value::as_str)
            .map(str::to_owned),
        project_id: record_string(record, "project_id")?,
        source_candidate_id: record_string(record, "source_candidate_id")?,
        source_candidate_state_sha256: record_string(record, "source_candidate_state_sha256")?,
        source_high_artifact_sha256: record_string(record, "source_high_artifact_sha256")?,
        source_high_artifact_readback_object_sha256: record_string(
            record,
            "source_high_artifact_readback_object_sha256",
        )?,
        target_triangle_count,
        max_collapses,
        locked_vertices,
        offset_m,
        max_offset_m,
        max_coordinate_abs_m,
        request_sha256: request_sha256.to_owned(),
    })
}

fn record_string(value: &Value, field: &str) -> Result<String, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("bundle field {field} is missing")))
}

fn record_u64(value: &Value, field: &str) -> Result<u64, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0 && *value <= 1_000_000)
        .ok_or_else(|| invalid(format!("bundle receipt {field} is invalid")))
}

fn restart_revalidate(runtime: &Runtime, record: &Value) -> Result<(), RuntimeError> {
    if bundle_key(record) != record_string(record, "bundle_key_sha256")?
        || record_string(record, "canonical_sha256")? != record_string(record, "bundle_key_sha256")?
    {
        return Err(invalid("stored source bundle canonical binding differs"));
    }
    let receipt_hash = record_string(record, "receipt_object_sha256")?;
    let receipt = read_json_cas(
        runtime,
        &receipt_hash,
        BUNDLE_RECEIPT_KIND,
        Some(RECEIPT_SCHEMA),
    )?;
    let receipt_cohort = validate_receipt_binding(record, &receipt)?;
    let request = request_from_record_and_receipt(record, &receipt)?;
    let (high, source_state) = source_preflight(runtime, &request)?;
    if source_state != record_string(record, "source_candidate_state_sha256")? {
        return Err(invalid("restart source candidate state changed"));
    }

    let low_hash = record_string(record, "low_artifact_sha256")?;
    let cage_hash = record_string(record, "cage_artifact_sha256")?;
    let low_bytes = read_glb_cas(runtime, &low_hash, PRODUCTION_WEAPON_LOW_ARTIFACT_KIND)?;
    let cage_bytes = read_glb_cas(runtime, &cage_hash, PRODUCTION_WEAPON_CAGE_ARTIFACT_KIND)?;
    let low_readback = read_json_cas(
        runtime,
        &record_string(record, "low_artifact_readback_object_sha256")?,
        PRODUCTION_WEAPON_LOW_ARTIFACT_RECEIPT_KIND,
        Some(LOW_READBACK_SCHEMA),
    )?;
    let cage_readback = read_json_cas(
        runtime,
        &record_string(record, "cage_artifact_readback_object_sha256")?,
        PRODUCTION_WEAPON_CAGE_ARTIFACT_RECEIPT_KIND,
        Some(CAGE_READBACK_SCHEMA),
    )?;
    let low_mesh = read_json_cas(
        runtime,
        &record_string(record, "low_mesh_object_sha256")?,
        LOW_MESH_KIND,
        Some(LOW_MESH_SCHEMA),
    )?;
    let correspondence = read_json_cas(
        runtime,
        &record_string(record, "correspondence_object_sha256")?,
        PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_KIND,
        Some(PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_SCHEMA_VERSION),
    )?;
    let offset = read_json_cas(
        runtime,
        &record_string(record, "cage_offset_field_object_sha256")?,
        OFFSET_FIELD_KIND,
        Some(OFFSET_FIELD_SCHEMA),
    )?;
    let high_inspection = strict_glb_inspection(&high)?;
    let low_inspection = strict_glb_inspection(&low_bytes)?;
    let cage_inspection = strict_glb_inspection(&cage_bytes)?;
    let low_mapping = low_mesh
        .get("mesh")
        .ok_or_else(|| invalid("stored Low mesh mapping is missing"))?;
    let offset_field = offset
        .get("offset_field")
        .ok_or_else(|| invalid("stored Cage offset field is missing"))?;
    let summary = validate_closed_high_low_cage(
        &high_inspection,
        &low_inspection,
        &cage_inspection,
        low_mapping,
        None,
        offset_field,
        request.max_offset_m,
        low_mesh.get("low_mesh_sha256").and_then(Value::as_str),
    )?;
    validate_persisted_correspondence(
        &correspondence,
        &summary,
        low_mapping,
        &request.source_high_artifact_sha256,
        &low_hash,
        &cage_hash,
        &record_string(record, "low_mesh_object_sha256")?,
    )?;
    if low_readback.get("artifact_sha256").and_then(Value::as_str) != Some(low_hash.as_str())
        || low_readback
            .get("source_high_artifact_sha256")
            .and_then(Value::as_str)
            != Some(request.source_high_artifact_sha256.as_str())
        || cage_readback.get("artifact_sha256").and_then(Value::as_str) != Some(cage_hash.as_str())
        || cage_readback
            .get("source_low_artifact_sha256")
            .and_then(Value::as_str)
            != Some(low_hash.as_str())
        || low_mesh.get("low_artifact_sha256").and_then(Value::as_str) != Some(low_hash.as_str())
        || low_mesh
            .get("source_high_artifact_sha256")
            .and_then(Value::as_str)
            != Some(request.source_high_artifact_sha256.as_str())
        || correspondence
            .get("low_artifact_sha256")
            .and_then(Value::as_str)
            != Some(low_hash.as_str())
        || correspondence
            .get("source_high_artifact_sha256")
            .and_then(Value::as_str)
            != Some(request.source_high_artifact_sha256.as_str())
        || correspondence
            .get("cage_artifact_sha256")
            .and_then(Value::as_str)
            != Some(cage_hash.as_str())
        || offset
            .get("source_low_artifact_sha256")
            .and_then(Value::as_str)
            != Some(low_hash.as_str())
        || offset.get("cage_artifact_sha256").and_then(Value::as_str) != Some(cage_hash.as_str())
        || offset.get("offset_field_policy").and_then(Value::as_str) != Some(CAGE_WORKER_POLICY)
    {
        return Err(invalid("restart derived JSON binding differs"));
    }

    let low_payload = worker_request_low(&request, &high)?;
    let low_first = super::geometry_worker::production_weapon_low_retopology(&low_payload)
        .map_err(|error| invalid(error.to_string()))?;
    let low_second = super::geometry_worker::production_weapon_low_retopology(&low_payload)
        .map_err(|error| invalid(error.to_string()))?;
    validate_worker_result(
        &low_first.result,
        "LowRetopologyWorkerResult@1",
        "production_weapon_low_retopology",
        "source_high_artifact_sha256",
        &request.source_high_artifact_sha256,
        "retopology_policy",
        LOW_WORKER_POLICY,
        LOW_WORKER_ALGORITHM,
        true,
    )?;
    validate_worker_result(
        &low_second.result,
        "LowRetopologyWorkerResult@1",
        "production_weapon_low_retopology",
        "source_high_artifact_sha256",
        &request.source_high_artifact_sha256,
        "retopology_policy",
        LOW_WORKER_POLICY,
        LOW_WORKER_ALGORITHM,
        true,
    )?;
    if low_first.result != low_second.result
        || low_first.build_cohort_sha256 != low_second.build_cohort_sha256
    {
        return Err(invalid("restart Low Worker replay differs"));
    }
    let low_cohort = worker_cohort(&low_first)?;
    if low_cohort != receipt_cohort
        || low_first
            .result
            .get("low_artifact_sha256")
            .and_then(Value::as_str)
            != Some(low_hash.as_str())
        || low_first
            .result
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != receipt
                .get("low_result_canonical_sha256")
                .and_then(Value::as_str)
    {
        return Err(invalid("restart Low Worker binding differs"));
    }
    let worker_low_bytes = base64::engine::general_purpose::STANDARD
        .decode(result_string(&low_first.result, "low_glb_base64")?.as_bytes())
        .map_err(|_| invalid("restart Low Worker GLB base64 is invalid"))?;
    if worker_low_bytes != low_bytes
        || low_readback.get("worker_readback") != low_first.result.get("low_artifact_readback")
    {
        return Err(invalid("restart Low GLB/readback bytes differ"));
    }

    let cage_payload = worker_request_cage(&request, &low_hash, &low_bytes)?;
    let cage_first = super::geometry_worker::production_weapon_cage_offset(&cage_payload)
        .map_err(|error| invalid(error.to_string()))?;
    let cage_second = super::geometry_worker::production_weapon_cage_offset(&cage_payload)
        .map_err(|error| invalid(error.to_string()))?;
    validate_worker_result(
        &cage_first.result,
        "CageOffsetWorkerResult@1",
        "production_weapon_cage_offset",
        "source_low_artifact_sha256",
        &low_hash,
        "offset_field_policy",
        CAGE_WORKER_POLICY,
        CAGE_WORKER_ALGORITHM,
        false,
    )?;
    validate_worker_result(
        &cage_second.result,
        "CageOffsetWorkerResult@1",
        "production_weapon_cage_offset",
        "source_low_artifact_sha256",
        &low_hash,
        "offset_field_policy",
        CAGE_WORKER_POLICY,
        CAGE_WORKER_ALGORITHM,
        false,
    )?;
    if cage_first.result != cage_second.result
        || cage_first.build_cohort_sha256 != cage_second.build_cohort_sha256
    {
        return Err(invalid("restart Cage Worker replay differs"));
    }
    let cage_cohort = worker_cohort(&cage_first)?;
    if cage_cohort != low_cohort
        || cage_first
            .result
            .get("cage_artifact_sha256")
            .and_then(Value::as_str)
            != Some(cage_hash.as_str())
        || cage_first
            .result
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != receipt
                .get("cage_result_canonical_sha256")
                .and_then(Value::as_str)
    {
        return Err(invalid("restart Cage Worker binding differs"));
    }
    let worker_cage_bytes = base64::engine::general_purpose::STANDARD
        .decode(result_string(&cage_first.result, "cage_glb_base64")?.as_bytes())
        .map_err(|_| invalid("restart Cage Worker GLB base64 is invalid"))?;
    let mut replay_offset_field = cage_first
        .result
        .get("offset_field")
        .cloned()
        .ok_or_else(|| invalid("restart Cage offset field is missing"))?;
    normalize_source_bundle_numbers(&mut replay_offset_field);
    let replay_offset_field_sha256 = canonical_json_hash(&replay_offset_field);
    if worker_cage_bytes != cage_bytes
        || cage_readback.get("worker_readback") != cage_first.result.get("cage_artifact_readback")
        || offset.get("offset_field") != Some(&replay_offset_field)
        || offset.get("offset_field_sha256").and_then(Value::as_str)
            != Some(replay_offset_field_sha256.as_str())
    {
        return Err(invalid("restart Cage GLB/readback/offset bytes differ"));
    }
    Ok(())
}

fn output(
    schema: &str,
    record: Value,
    replayed: bool,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    Ok(
        json!({"schema_version":schema, "bundle_key_sha256":record["bundle_key_sha256"], "bundle":record,
        "replayed":replayed, "restart_hash_verified":true, "runtime_write":runtime_write,
        "quality_status":"structural_only", "visual_quality_status":"NOT_PROVEN",
        "human_review_status":"NOT_RUN", "commercial_engine_status":"NOT_RUN",
        "production_stage_advanced":false, "candidate_confirmed":false,
        "version_created":false, "export_performed":false}),
    )
}

impl Runtime {
    pub fn production_weapon_retopology_cage_source_prepare(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        self.production_weapon_retopology_cage_source_bundle_prepare(value)
    }

    pub fn production_weapon_retopology_cage_source_bundle_prepare(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let request = parse_prepare(&value)?;
        let (high, source_state) = source_preflight(self, &request)?;
        let high_inspection = strict_glb_inspection(&high)?;
        let low_payload = worker_request_low(&request, &high)?;
        let low_first = super::geometry_worker::production_weapon_low_retopology(&low_payload)
            .map_err(|e| invalid(e.to_string()))?;
        let low_second = super::geometry_worker::production_weapon_low_retopology(&low_payload)
            .map_err(|e| invalid(e.to_string()))?;
        validate_worker_result(
            &low_first.result,
            "LowRetopologyWorkerResult@1",
            "production_weapon_low_retopology",
            "source_high_artifact_sha256",
            &request.source_high_artifact_sha256,
            "retopology_policy",
            LOW_WORKER_POLICY,
            LOW_WORKER_ALGORITHM,
            true,
        )?;
        validate_worker_result(
            &low_second.result,
            "LowRetopologyWorkerResult@1",
            "production_weapon_low_retopology",
            "source_high_artifact_sha256",
            &request.source_high_artifact_sha256,
            "retopology_policy",
            LOW_WORKER_POLICY,
            LOW_WORKER_ALGORITHM,
            true,
        )?;
        if low_first.result != low_second.result
            || low_first.build_cohort_sha256 != low_second.build_cohort_sha256
        {
            return Err(invalid("Low Worker replay or cohort changed"));
        }
        let low_cohort = worker_cohort(&low_first)?;
        let low_sha = result_string(&low_first.result, "low_artifact_sha256")?.to_owned();
        let low_glb = base64::engine::general_purpose::STANDARD
            .decode(result_string(&low_first.result, "low_glb_base64")?.as_bytes())
            .map_err(|_| invalid("Low GLB base64 invalid"))?;
        if sha256_hex(&low_glb) != low_sha || !strict_glb_inspection(&low_glb)?.hard_gate_passed {
            return Err(invalid("Low GLB strict readback failed"));
        }
        let cage_payload = worker_request_cage(&request, &low_sha, &low_glb)?;
        let cage_first = super::geometry_worker::production_weapon_cage_offset(&cage_payload)
            .map_err(|e| invalid(e.to_string()))?;
        let cage_second = super::geometry_worker::production_weapon_cage_offset(&cage_payload)
            .map_err(|e| invalid(e.to_string()))?;
        validate_worker_result(
            &cage_first.result,
            "CageOffsetWorkerResult@1",
            "production_weapon_cage_offset",
            "source_low_artifact_sha256",
            &low_sha,
            "offset_field_policy",
            CAGE_WORKER_POLICY,
            CAGE_WORKER_ALGORITHM,
            false,
        )?;
        validate_worker_result(
            &cage_second.result,
            "CageOffsetWorkerResult@1",
            "production_weapon_cage_offset",
            "source_low_artifact_sha256",
            &low_sha,
            "offset_field_policy",
            CAGE_WORKER_POLICY,
            CAGE_WORKER_ALGORITHM,
            false,
        )?;
        if cage_first.result != cage_second.result
            || cage_first.build_cohort_sha256 != cage_second.build_cohort_sha256
        {
            return Err(invalid("Cage Worker replay or cohort changed"));
        }
        let cage_cohort = worker_cohort(&cage_first)?;
        if low_cohort != cage_cohort {
            return Err(invalid("Low/Cage Worker cohort differs"));
        }
        let cage_sha = result_string(&cage_first.result, "cage_artifact_sha256")?.to_owned();
        let cage_glb = base64::engine::general_purpose::STANDARD
            .decode(result_string(&cage_first.result, "cage_glb_base64")?.as_bytes())
            .map_err(|_| invalid("Cage GLB base64 invalid"))?;
        if sha256_hex(&cage_glb) != cage_sha || !strict_glb_inspection(&cage_glb)?.hard_gate_passed
        {
            return Err(invalid("Cage GLB strict readback failed"));
        }
        let low_inspection = strict_glb_inspection(&low_glb)?;
        let cage_inspection = strict_glb_inspection(&cage_glb)?;
        let low_value = &low_first.result;
        let cage_value = &cage_first.result;
        validate_cage_worker_diagnostic(cage_value)?;
        let mut low_mapping = low_value
            .get("low_mesh")
            .cloned()
            .ok_or_else(|| invalid("Low Worker correspondence mapping is missing"))?;
        let mut cage_mapping = cage_value
            .get("cage_mesh")
            .cloned()
            .ok_or_else(|| invalid("Cage Worker mesh correspondence is missing"))?;
        let mut offset_field = cage_value
            .get("offset_field")
            .cloned()
            .ok_or_else(|| invalid("Cage Worker offset field is missing"))?;
        normalize_source_bundle_numbers(&mut low_mapping);
        normalize_source_bundle_numbers(&mut cage_mapping);
        normalize_source_bundle_numbers(&mut offset_field);
        let runtime_mapping_sha256 = canonical_json_hash(&low_mapping);
        let runtime_offset_field_sha256 = canonical_json_hash(&offset_field);
        let summary = validate_closed_high_low_cage(
            &high_inspection,
            &low_inspection,
            &cage_inspection,
            &low_mapping,
            Some(&cage_mapping),
            &offset_field,
            request.max_offset_m,
            Some(&runtime_mapping_sha256),
        )?;
        let low_mesh_value = json!({"schema_version":LOW_MESH_SCHEMA, "source_high_artifact_sha256":request.source_high_artifact_sha256, "low_artifact_sha256":low_sha, "mesh":low_mapping, "low_mesh_sha256":runtime_mapping_sha256, "canonical_sha256":""});
        let mut correspondence_value = correspondence_value(
            &request.source_high_artifact_sha256,
            &low_sha,
            &cage_sha,
            "",
            &summary,
            low_mesh_value
                .get("mesh")
                .ok_or_else(|| invalid("normalized Low correspondence mapping is missing"))?,
            low_value
                .get("algorithm_sha256")
                .and_then(Value::as_str)
                .filter(|value| is_sha256(value))
                .ok_or_else(|| invalid("Low Worker algorithm hash is missing"))?,
            low_cohort,
        );
        let offset_value = json!({"schema_version":OFFSET_FIELD_SCHEMA, "source_low_artifact_sha256":low_sha, "cage_artifact_sha256":cage_sha, "offset_field_policy":"exact-low-topology-per-vertex-normal-offset@1", "offset_field_sha256":runtime_offset_field_sha256, "offset_field":offset_field, "canonical_sha256":""});
        let low_readback_value = json!({"schema_version":LOW_READBACK_SCHEMA, "artifact_sha256":low_sha, "source_high_artifact_sha256":request.source_high_artifact_sha256, "worker_readback":low_value["low_artifact_readback"], "canonical_sha256":""});
        let cage_readback_value = json!({"schema_version":CAGE_READBACK_SCHEMA, "artifact_sha256":cage_sha, "source_low_artifact_sha256":low_sha, "worker_readback":cage_value["cage_artifact_readback"], "canonical_sha256":""});
        let created_at = super::now_string();
        let low_readback_sha = json_object_sha256(&low_readback_value)?;
        let cage_readback_sha = json_object_sha256(&cage_readback_value)?;
        let low_mesh_sha = json_object_sha256(&low_mesh_value)?;
        correspondence_value["mapping_object_sha256"] = Value::String(low_mesh_sha.clone());
        let correspondence_sha = json_object_sha256(&correspondence_value)?;
        let offset_sha = json_object_sha256(&offset_value)?;
        let provisional = record_value(
            &request,
            &source_state,
            &request.source_high_artifact_readback_object_sha256,
            low_value,
            cage_value,
            &low_readback_sha,
            &cage_readback_sha,
            &low_mesh_sha,
            &correspondence_sha,
            &offset_sha,
            "",
            &created_at,
        );
        let receipt_value = make_receipt(
            &provisional,
            &request,
            low_value,
            cage_value,
            &low_mesh_sha,
            &correspondence_sha,
            &offset_sha,
            low_cohort,
        );
        let receipt_sha = json_object_sha256(&receipt_value)?;
        let record = record_value(
            &request,
            &source_state,
            &request.source_high_artifact_readback_object_sha256,
            low_value,
            cage_value,
            &low_readback_sha,
            &cage_readback_sha,
            &low_mesh_sha,
            &correspondence_sha,
            &offset_sha,
            &receipt_sha,
            &created_at,
        );
        if request
            .expected_bundle_key
            .as_deref()
            .is_some_and(|key| key != record["bundle_key_sha256"].as_str().unwrap_or_default())
        {
            return Err(invalid(
                "caller bundle key does not bind derived source bundle",
            ));
        }
        if let Some(existing) = self
            .store
            .get_production_weapon_retopology_cage_source_bundle(
                record["bundle_key_sha256"].as_str().unwrap_or_default(),
            )?
        {
            if !same_bundle_value(&existing, &record) {
                return Err(invalid(
                    "source bundle key conflicts with a different binding",
                ));
            }
            return output(PREPARE_RESULT_SCHEMA, existing, true, true);
        }
        let reservation = self.store.begin_cas_reservation();
        let mut objects = Vec::new();
        let put_glb = |runtime: &Runtime,
                       bytes: &[u8],
                       kind: &str,
                       objects: &mut Vec<CasObject>|
         -> Result<CasObject, RuntimeError> {
            let object = runtime.store.put_object_reserved(
                &reservation,
                bytes,
                None,
                GLB_MIME,
                kind,
                &created_at,
            )?;
            objects.push(object.clone());
            Ok(object)
        };
        let low_object = match put_glb(
            self,
            &low_glb,
            PRODUCTION_WEAPON_LOW_ARTIFACT_KIND,
            &mut objects,
        ) {
            Ok(v) => v,
            Err(e) => {
                release(self, &reservation, &objects, true);
                return Err(e);
            }
        };
        if low_object.record.sha256 != low_sha {
            release(self, &reservation, &objects, true);
            return Err(invalid("Low GLB CAS hash differs from Worker result"));
        }
        let low_readback_object = match put_json(
            self,
            &reservation,
            low_readback_value,
            PRODUCTION_WEAPON_LOW_ARTIFACT_RECEIPT_KIND,
            &created_at,
            &mut objects,
        ) {
            Ok(v) => v,
            Err(e) => {
                release(self, &reservation, &objects, true);
                return Err(e);
            }
        };
        if low_readback_object.record.sha256 != low_readback_sha {
            release(self, &reservation, &objects, true);
            return Err(invalid(
                "Low readback CAS hash differs from canonical bytes",
            ));
        }
        let cage_object = match put_glb(
            self,
            &cage_glb,
            PRODUCTION_WEAPON_CAGE_ARTIFACT_KIND,
            &mut objects,
        ) {
            Ok(v) => v,
            Err(e) => {
                release(self, &reservation, &objects, true);
                return Err(e);
            }
        };
        if cage_object.record.sha256 != cage_sha {
            release(self, &reservation, &objects, true);
            return Err(invalid("Cage GLB CAS hash differs from Worker result"));
        }
        let cage_readback_object = match put_json(
            self,
            &reservation,
            cage_readback_value,
            PRODUCTION_WEAPON_CAGE_ARTIFACT_RECEIPT_KIND,
            &created_at,
            &mut objects,
        ) {
            Ok(v) => v,
            Err(e) => {
                release(self, &reservation, &objects, true);
                return Err(e);
            }
        };
        if cage_readback_object.record.sha256 != cage_readback_sha {
            release(self, &reservation, &objects, true);
            return Err(invalid(
                "Cage readback CAS hash differs from canonical bytes",
            ));
        }
        let low_mesh_object = match put_json(
            self,
            &reservation,
            low_mesh_value,
            LOW_MESH_KIND,
            &created_at,
            &mut objects,
        ) {
            Ok(v) => v,
            Err(e) => {
                release(self, &reservation, &objects, true);
                return Err(e);
            }
        };
        let correspondence_object = match put_json(
            self,
            &reservation,
            correspondence_value,
            PRODUCTION_WEAPON_HIGH_LOW_CORRESPONDENCE_KIND,
            &created_at,
            &mut objects,
        ) {
            Ok(v) => v,
            Err(e) => {
                release(self, &reservation, &objects, true);
                return Err(e);
            }
        };
        let offset_object = match put_json(
            self,
            &reservation,
            offset_value,
            OFFSET_FIELD_KIND,
            &created_at,
            &mut objects,
        ) {
            Ok(v) => v,
            Err(e) => {
                release(self, &reservation, &objects, true);
                return Err(e);
            }
        };
        if low_mesh_object.record.sha256 != low_mesh_sha
            || correspondence_object.record.sha256 != correspondence_sha
            || offset_object.record.sha256 != offset_sha
        {
            release(self, &reservation, &objects, true);
            return Err(invalid(
                "derived JSON CAS hash differs from canonical bytes",
            ));
        }
        let receipt_object = match put_json(
            self,
            &reservation,
            receipt_value,
            BUNDLE_RECEIPT_KIND,
            &created_at,
            &mut objects,
        ) {
            Ok(v) => v,
            Err(e) => {
                release(self, &reservation, &objects, true);
                return Err(e);
            }
        };
        if receipt_object.record.sha256 != receipt_sha {
            release(self, &reservation, &objects, true);
            return Err(invalid(
                "bundle receipt CAS hash differs from canonical bytes",
            ));
        }
        let stored = match self
            .store
            .record_production_weapon_retopology_cage_source_bundle(&record, &receipt_object.record)
        {
            Ok(v) => v,
            Err(e) => {
                release(self, &reservation, &objects, true);
                return Err(e.into());
            }
        };
        release(self, &reservation, &objects, false);
        output(PREPARE_RESULT_SCHEMA, stored, false, true)
    }

    pub fn production_weapon_retopology_cage_source_bundle_get(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        let (key, project_id, candidate_id) = parse_get(&value)?;
        let record = self
            .store
            .get_production_weapon_retopology_cage_source_bundle(&key)?
            .ok_or_else(|| invalid("retopology cage source bundle is unavailable"))?;
        if record.get("project_id").and_then(Value::as_str) != Some(project_id.as_str())
            || record.get("source_candidate_id").and_then(Value::as_str)
                != Some(candidate_id.as_str())
        {
            return Err(invalid("source bundle get scope differs"));
        }
        restart_revalidate(self, &record)?;
        output(GET_RESULT_SCHEMA, record, false, false)
    }

    pub fn production_weapon_retopology_cage_source_get(
        &self,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        self.production_weapon_retopology_cage_source_bundle_get(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use uuid::Uuid;
    #[test]
    fn closed_parser_rejects_unknown_fields() {
        let mut value = json!({"schema_version":PREPARE_SCHEMA});
        value["unknown"] = Value::Bool(true);
        assert!(parse_prepare(&value).is_err());
    }
    #[test]
    fn bundle_key_ignores_receipt_and_created_at() {
        let base = json!({"bundle_key_sha256":"","receipt_object_sha256":"a","canonical_sha256":"","created_at":"1","source":"x"});
        let mut other = base.clone();
        other["receipt_object_sha256"] = Value::String("b".to_owned());
        other["created_at"] = Value::String("2".to_owned());
        assert_eq!(bundle_key(&base), bundle_key(&other));
    }

    #[test]
    fn normalized_json_writes_canonical_before_cas_bytes() {
        let (bytes, canonical) = normalized_object(json!({
            "schema_version": "Test@1",
            "canonical_sha256": ""
        }))
        .expect("canonical JSON");
        let value: Value = serde_json::from_slice(&bytes).expect("JSON bytes");
        assert_eq!(value["canonical_sha256"].as_str(), Some(canonical.as_str()));
        let mut preimage = value.clone();
        preimage["canonical_sha256"] = Value::String(String::new());
        assert_eq!(canonical_json_hash(&preimage), canonical);
        assert!(!bytes.is_empty());
    }

    #[test]
    fn artifact_readback_binds_cas_sha_and_keeps_candidate_id_separate() {
        let prepared_id = "geometry-object-test";
        let artifact_sha256 = "a".repeat(64);
        let candidate_id = "candidate-test";
        let readback = json!({
            "schema_version": "ArtifactReadback@2",
            "artifact_id": artifact_sha256.clone(),
            "object_sha256": artifact_sha256,
            "candidate_id": candidate_id,
            "mime": GLB_MIME,
            "hard_gate_passed": true,
            "validator_status": "passed"
        });
        validate_artifact_readback_binding(&readback, &artifact_sha256, candidate_id)
            .expect("artifact id/hash binding");
        let mut retargeted = readback.clone();
        retargeted["artifact_id"] = Value::String(prepared_id.to_owned());
        assert!(
            validate_artifact_readback_binding(&retargeted, &artifact_sha256, candidate_id,)
                .is_err()
        );
    }

    #[test]
    fn low_worker_result_positive_fixture_requires_canonical_and_source_flags() {
        let source_hash = "a".repeat(64);
        let mut result = json!({
            "schema_version": "LowRetopologyWorkerResult@1",
            "operation": "production_weapon_low_retopology",
            "source_high_artifact_sha256": source_hash,
            "retopology_policy": LOW_WORKER_POLICY,
            "algorithm": LOW_WORKER_ALGORITHM,
            "quality_status": "structural_only",
            "runtime_write_performed": false,
            "production_stage_advanced": false,
            "promotion_eligible": false,
            "candidate_confirmed": false,
            "version_created": false,
            "export_performed": false,
            "retopology_derived": true,
            "artist_authored_quad_topology": false,
            "edge_flow_status": "NOT_PROVEN",
            "canonical_sha256": ""
        });
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_worker_result(
            &result,
            "LowRetopologyWorkerResult@1",
            "production_weapon_low_retopology",
            "source_high_artifact_sha256",
            &"a".repeat(64),
            "retopology_policy",
            LOW_WORKER_POLICY,
            LOW_WORKER_ALGORITHM,
            true,
        )
        .expect("positive Worker result");
        result["runtime_write_performed"] = Value::Bool(true);
        assert!(validate_worker_result(
            &result,
            "LowRetopologyWorkerResult@1",
            "production_weapon_low_retopology",
            "source_high_artifact_sha256",
            &"a".repeat(64),
            "retopology_policy",
            LOW_WORKER_POLICY,
            LOW_WORKER_ALGORITHM,
            true,
        )
        .is_err());
    }

    /// Public, same-cohort source-bundle fixture.  This is intentionally a
    /// small one-box geometry source: it exercises the real Geometry Worker,
    /// the real Low/Cage sibling Workers, SQLite/CAS transaction, replay and
    /// restart readback without starting a render or a 2K job.
    #[test]
    fn production_weapon_retopology_cage_source_public_fixture_is_durable_replayable_and_restart_verified(
    ) {
        if forgecad_contracts::build_cohort_sha256().is_none() {
            return;
        }

        let root = std::env::temp_dir().join(format!(
            "forgecad-production-retopology-cage-public-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("fixture root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");

        let (project_id, candidate_id, source_state, source_artifact, source_readback, request) = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("fixture runtime");
            let project = runtime
                .create_project(
                    "production retopology cage fixture",
                    json!({"profile":"mvp"}),
                )
                .expect("fixture project");
            let mut geometry = json!({
                "schema_version":"GeometryProgram@2",
                "project_id":project.project_id,
                "representation_plan_sha256":sha256_hex(b"production-retopology-cage-fixture"),
                "operator_catalog_sha256":super::super::operator_catalog_sha256(),
                "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                "budgets":{"max_nodes":1,"max_triangles":1000,"max_glb_bytes":67_108_864,"max_worker_memory_bytes":536_870_912,"max_runtime_ms":10_000},
                "nodes":[{
                    "node_id":"fixture-box-node",
                    "operator_id":"forgecad.geometry.primitive@2",
                    "inputs":[],
                    "parameters":{"shape":"box","size_m":[2.0,2.0,2.0],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}
                }],
                "part_outputs":[{"part_id":"fixture-box","input_node_ids":["fixture-box-node"],"material_zone_id":"zone-body","solid":true}]
            });
            geometry["canonical_sha256"] = Value::String(canonical_json_hash(&geometry));
            let prepared = runtime
                .prepare_geometry_candidate(
                    &project.project_id,
                    None,
                    json!({"typed":"geometry","geometry_program":geometry}),
                )
                .expect("fixture geometry candidate");
            let candidate = &prepared["candidate"];
            let candidate_id = candidate["candidate_id"]
                .as_str()
                .expect("fixture candidate id")
                .to_owned();
            let source_artifact = candidate["prepared_object_sha256"]
                .as_str()
                .expect("fixture source artifact SHA")
                .to_owned();
            let source_evidence = runtime
                .store
                .get_geometry_candidate_evidence(&candidate_id)
                .expect("fixture source evidence lookup")
                .expect("fixture source evidence");
            let source_candidate = runtime
                .candidate(&candidate_id)
                .expect("fixture source candidate lookup")
                .expect("fixture source candidate");
            let source_state = source_candidate.canonical_sha256;
            let source_readback = source_evidence.artifact_readback_object_sha256;
            let source_triangle_count = prepared["artifact"]["triangle_count"]
                .as_u64()
                .expect("fixture source triangle count");
            assert!(source_triangle_count > 0, "source must contain triangles");

            let mut request = json!({
                "schema_version":PREPARE_SCHEMA,
                "bundle_key_sha256":Value::Null,
                "project_id":project.project_id,
                "source_candidate_id":candidate_id,
                "source_candidate_state_sha256":source_state,
                "source_high_artifact_sha256":source_artifact,
                "source_high_artifact_readback_object_sha256":source_readback,
                "target_triangle_count":10,
                "max_collapses":8,
                "locked_vertices":[],
                "offset_m":0.001,
                "max_offset_m":0.01,
                "max_coordinate_abs_m":100.0,
                "low_retopology_policy":POLICY,
                "cage_policy":POLICY,
                "input_sha256":"",
                "idempotency_key":"production-retopology-cage-public-fixture-v1"
            });
            let mut input_preimage = request.clone();
            input_preimage
                .as_object_mut()
                .expect("fixture request object")
                .remove("input_sha256");
            input_preimage
                .as_object_mut()
                .expect("fixture request object")
                .remove("idempotency_key");
            request["input_sha256"] = Value::String(canonical_json_hash(&input_preimage));
            (
                project.project_id,
                candidate_id,
                source_state,
                source_artifact,
                source_readback,
                request,
            )
        };

        let runtime = Runtime::open_with_cas(&database, &cas).expect("fixture reopen writer");
        let before = runtime
            .store
            .cas()
            .list_objects()
            .expect("fixture CAS before");
        let first = runtime
            .production_weapon_retopology_cage_source_prepare(request.clone())
            .expect("source bundle prepare");
        assert_eq!(first["schema_version"], PREPARE_RESULT_SCHEMA);
        assert_eq!(first["replayed"], false);
        assert_eq!(first["runtime_write"], true);
        assert_eq!(first["quality_status"], "structural_only");
        assert_eq!(first["visual_quality_status"], "NOT_PROVEN");
        assert_eq!(first["production_stage_advanced"], false);
        assert_eq!(first["candidate_confirmed"], false);
        assert_eq!(first["version_created"], false);
        assert_eq!(first["export_performed"], false);

        let bundle = first["bundle"].clone();
        let bundle_key = bundle["bundle_key_sha256"]
            .as_str()
            .expect("bundle key")
            .to_owned();
        assert_eq!(bundle["project_id"], project_id);
        assert_eq!(bundle["source_candidate_id"], candidate_id);
        assert_eq!(bundle["source_candidate_state_sha256"], source_state);
        assert_eq!(bundle["source_high_artifact_sha256"], source_artifact);
        assert_eq!(
            bundle["source_high_artifact_readback_object_sha256"],
            source_readback
        );
        assert_eq!(bundle["low_retopology_policy"], POLICY);
        assert_eq!(bundle["cage_policy"], POLICY);

        let after_first = runtime
            .store
            .cas()
            .list_objects()
            .expect("fixture CAS after");
        let before_hashes = before
            .iter()
            .map(|object| {
                object
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("fixture CAS object filename")
                    .to_owned()
            })
            .collect::<BTreeSet<_>>();
        let added = after_first
            .iter()
            .map(|object| {
                object
                    .file_name()
                    .and_then(|name| name.to_str())
                    .expect("fixture CAS object filename")
                    .to_owned()
            })
            .filter(|hash| !before_hashes.contains(hash))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            added.len(),
            8,
            "one Low/Cage source bundle owns exactly 8 CAS objects"
        );
        assert!(
            !added.contains(&bundle_key),
            "parent binding key is not a CAS object"
        );
        for field in [
            "low_artifact_sha256",
            "low_artifact_readback_object_sha256",
            "cage_artifact_sha256",
            "cage_artifact_readback_object_sha256",
            "low_mesh_object_sha256",
            "correspondence_object_sha256",
            "cage_offset_field_object_sha256",
            "receipt_object_sha256",
        ] {
            assert!(
                added.contains(bundle[field].as_str().expect("bundle CAS hash")),
                "owned object {field} must be newly linked"
            );
        }
        let low_sha = bundle["low_artifact_sha256"].as_str().unwrap();
        let cage_sha = bundle["cage_artifact_sha256"].as_str().unwrap();
        let low_bytes = runtime.cas_read(low_sha).expect("Low GLB");
        let cage_bytes = runtime.cas_read(cage_sha).expect("Cage GLB");
        let low_inspection = strict_glb_inspection(&low_bytes).expect("Low inspection");
        let cage_inspection = strict_glb_inspection(&cage_bytes).expect("Cage inspection");
        let source_inspection =
            strict_glb_inspection(&runtime.cas_read(&source_artifact).expect("source GLB"))
                .expect("source inspection");
        assert!(low_inspection.triangle_count < source_inspection.triangle_count);
        assert_eq!(
            cage_inspection.triangle_count,
            low_inspection.triangle_count
        );
        assert_eq!(cage_inspection.part_ids, low_inspection.part_ids);
        assert_eq!(
            cage_inspection.source_node_ids,
            low_inspection.source_node_ids
        );
        assert_eq!(
            cage_inspection.material_zone_ids,
            low_inspection.material_zone_ids
        );
        let low_mesh = crate::integrity::extract_diagnostic_mesh(&low_bytes, 1_000_000)
            .expect("Low diagnostic mesh");
        let cage_mesh = crate::integrity::extract_diagnostic_mesh(&cage_bytes, 1_000_000)
            .expect("Cage diagnostic mesh");
        assert_eq!(cage_mesh.triangle_count, low_mesh.triangle_count);
        assert_eq!(cage_mesh.primitives.len(), low_mesh.primitives.len());
        for (low_primitive, cage_primitive) in
            low_mesh.primitives.iter().zip(cage_mesh.primitives.iter())
        {
            assert_eq!(cage_primitive.part_id, low_primitive.part_id);
            assert_eq!(cage_primitive.source_node_id, low_primitive.source_node_id);
            assert_eq!(
                cage_primitive.material_zone_id,
                low_primitive.material_zone_id
            );
            assert_eq!(cage_primitive.solid, low_primitive.solid);
            assert_eq!(cage_primitive.indices, low_primitive.indices);
        }
        let cage_readback = read_json_cas(
            &runtime,
            bundle["cage_artifact_readback_object_sha256"]
                .as_str()
                .unwrap(),
            PRODUCTION_WEAPON_CAGE_ARTIFACT_RECEIPT_KIND,
            Some(CAGE_READBACK_SCHEMA),
        )
        .expect("Cage readback");
        assert!(cage_readback
            .get("worker_readback")
            .is_some_and(Value::is_object));
        let offset = read_json_cas(
            &runtime,
            bundle["cage_offset_field_object_sha256"].as_str().unwrap(),
            OFFSET_FIELD_KIND,
            Some(OFFSET_FIELD_SCHEMA),
        )
        .expect("Cage offset field");
        assert_eq!(offset["offset_field_policy"], CAGE_WORKER_POLICY);
        assert_eq!(
            offset["offset_field"].as_array().unwrap().len(),
            low_mesh
                .primitives
                .iter()
                .map(|primitive| primitive.positions.len())
                .sum::<usize>()
        );

        let replay = runtime
            .production_weapon_retopology_cage_source_prepare(request.clone())
            .expect("source bundle replay");
        assert_eq!(replay["replayed"], true);
        assert_eq!(replay["bundle"], bundle);
        assert_eq!(runtime.store.cas().list_objects().unwrap(), after_first);

        let get_request = json!({
            "schema_version":GET_SCHEMA,
            "bundle_key_sha256":bundle_key,
            "project_id":project_id,
            "source_candidate_id":candidate_id
        });
        let before_get = runtime.store.cas().list_objects().unwrap();
        let get = runtime
            .production_weapon_retopology_cage_source_get(get_request.clone())
            .expect("source bundle get");
        assert_eq!(get["runtime_write"], false);
        assert_eq!(get["restart_hash_verified"], true);
        assert_eq!(get["bundle"], bundle);
        assert_eq!(runtime.store.cas().list_objects().unwrap(), before_get);
        drop(runtime);

        let reopened = Runtime::open_with_cas(&database, &cas).expect("fixture restart runtime");
        let before_restart = reopened.store.cas().list_objects().unwrap();
        let restarted = reopened
            .production_weapon_retopology_cage_source_get(get_request)
            .expect("source bundle restart get");
        assert_eq!(restarted["runtime_write"], false);
        assert_eq!(restarted["restart_hash_verified"], true);
        assert_eq!(restarted["bundle"], bundle);
        assert_eq!(reopened.store.cas().list_objects().unwrap(), before_restart);

        let bind_input = |mut candidate_request: Value| {
            let object = candidate_request
                .as_object_mut()
                .expect("retarget request object");
            object.remove("input_sha256");
            object.remove("idempotency_key");
            let hash = canonical_json_hash(&candidate_request);
            candidate_request["input_sha256"] = Value::String(hash);
            candidate_request
        };
        let mut retarget_candidate = request.clone();
        retarget_candidate["source_candidate_id"] =
            Value::String("candidate-retargeted".to_owned());
        let before_retarget = reopened.store.cas().list_objects().unwrap();
        assert!(reopened
            .production_weapon_retopology_cage_source_prepare(bind_input(retarget_candidate))
            .is_err());
        assert_eq!(
            reopened.store.cas().list_objects().unwrap(),
            before_retarget
        );

        let mut retarget_state = request;
        retarget_state["source_candidate_state_sha256"] = Value::String("b".repeat(64));
        let before_state_retarget = reopened.store.cas().list_objects().unwrap();
        assert!(reopened
            .production_weapon_retopology_cage_source_prepare(bind_input(retarget_state))
            .is_err());
        assert_eq!(
            reopened.store.cas().list_objects().unwrap(),
            before_state_retarget
        );

        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }
}
