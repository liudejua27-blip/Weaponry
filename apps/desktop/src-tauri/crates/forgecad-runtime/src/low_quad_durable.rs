//! Runtime-owned durable explicit Low quad-draft source materialization.
//!
//! The existing `production_low_retopology` Worker operation is a bounded
//! triangle edge-collapse path.  This module is deliberately separate: it
//! admits a caller-supplied explicit all-quad draft, runs that Worker twice,
//! writes only a CAS/SQLite source bundle, and replays the same source on a
//! later get/restart.  It never promotes a Low stage, confirms a candidate,
//! creates a version, exports, or changes the triangle edge-collapse path.
//!
//! This file is intentionally an integration seam.  The parent Runtime lane
//! should add one `mod low_quad_durable;` declaration after reviewing the
//! additive contract/store modules.

use super::{
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, now_string, sha256_hex,
    strict_glb_inspection, Runtime, RuntimeError, MAX_DERIVED_JSON_BYTES,
    MAX_GEOMETRY_ARTIFACT_BYTES,
};
use base64::Engine;
use forgecad_contracts::{
    LowQuadDraftDurableRecord, LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND,
    LOW_QUAD_DRAFT_DURABLE_ARTIFACT_READBACK_SCHEMA_VERSION,
    LOW_QUAD_DRAFT_DURABLE_CANONICALIZATION_POLICY,
    LOW_QUAD_DRAFT_DURABLE_GET_RESULT_SCHEMA_VERSION, LOW_QUAD_DRAFT_DURABLE_GET_SCHEMA_VERSION,
    LOW_QUAD_DRAFT_DURABLE_LIMITATIONS, LOW_QUAD_DRAFT_DURABLE_LINK_SCHEMA_VERSION,
    LOW_QUAD_DRAFT_DURABLE_MAX_GLB_BYTES, LOW_QUAD_DRAFT_DURABLE_MAX_JSON_BYTES,
    LOW_QUAD_DRAFT_DURABLE_MAX_RESPONSE_BYTES, LOW_QUAD_DRAFT_DURABLE_OBJECT_KIND,
    LOW_QUAD_DRAFT_DURABLE_OPERATION_GET, LOW_QUAD_DRAFT_DURABLE_OPERATION_PREPARE,
    LOW_QUAD_DRAFT_DURABLE_POLICY, LOW_QUAD_DRAFT_DURABLE_PREPARE_RESULT_SCHEMA_VERSION,
    LOW_QUAD_DRAFT_DURABLE_PREPARE_SCHEMA_VERSION, LOW_QUAD_DRAFT_DURABLE_READBACK_KIND,
    LOW_QUAD_DRAFT_DURABLE_RECORD_SCHEMA_VERSION, LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND,
    LOW_QUAD_DRAFT_DURABLE_WRITER_POLICY, PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND,
};
use forgecad_store::{CasObject, CasReservation};
use forgecad_worker_protocol::{
    PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM, PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION,
    PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY,
    PRODUCTION_WEAPON_LOW_QUAD_DRAFT_REQUEST_SCHEMA_VERSION,
    PRODUCTION_WEAPON_LOW_QUAD_DRAFT_RESULT_SCHEMA_VERSION,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const JSON_MIME: &str = "application/json";
const GLB_MIME: &str = "model/gltf-binary";
const GEOMETRY_WORKER_BINARY: &str = "forgecad-geometry-worker";
const MATERIALIZATION_STATUS: &str = "runtime-owned-durable-low-quad-draft-source-only@1";
const ARTIFACT_READBACK_KIND: &str = LOW_QUAD_DRAFT_DURABLE_READBACK_KIND;
const MAX_SOURCE_JSON_BYTES: u64 = LOW_QUAD_DRAFT_DURABLE_MAX_JSON_BYTES;
const MAX_WORKER_RESULT_BYTES: u64 = LOW_QUAD_DRAFT_DURABLE_MAX_JSON_BYTES;
const MAX_SOURCE_GLB_BYTES: u64 = LOW_QUAD_DRAFT_DURABLE_MAX_GLB_BYTES;

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "source_high_artifact_id",
    "source_high_artifact_object_sha256",
    "source_high_artifact_sha256",
    "source_high_artifact_readback_object_sha256",
    "source_high_artifact_readback_sha256",
    "low_quad_draft_worker_request",
    "low_quad_draft_worker_request_sha256",
    "idempotency_key",
    "max_response_bytes",
    "source_only",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "link_id",
    "link_object_sha256",
    "source_high_artifact_id",
    "source_high_artifact_sha256",
    "worker_result_object_sha256",
    "worker_result_sha256",
    "artifact_object_sha256",
    "artifact_sha256",
    "readback_object_sha256",
    "readback_sha256",
    "idempotency_key",
    "source_only",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

#[derive(Debug, Clone)]
struct PrepareRequest {
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    base_version_id: Option<String>,
    source_high_artifact_id: String,
    source_high_artifact_object_sha256: String,
    source_high_artifact_sha256: String,
    source_high_artifact_readback_object_sha256: String,
    source_high_artifact_readback_sha256: String,
    worker_request: Value,
    worker_request_sha256: String,
    idempotency_key: String,
    input_sha256: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "LOW_QUAD_DRAFT_DURABLE_INVALID: {}",
        message.into()
    ))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(invalid(format!("{context} has unknown or missing fields")));
    }
    Ok(object)
}

fn required_id(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} is not an opaque id")))
}

fn required_hash(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} is not a SHA-256")))
}

fn nullable_id(object: &Map<String, Value>, field: &str) -> Result<Option<String>, RuntimeError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_opaque_id(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!("{field} is not a nullable opaque id"))),
    }
}

fn required_bool(
    object: &Map<String, Value>,
    field: &str,
    expected: bool,
) -> Result<(), RuntimeError> {
    if object.get(field) != Some(&Value::Bool(expected)) {
        return Err(invalid(format!("{field} policy differs")));
    }
    Ok(())
}

fn request_input_hash(value: &Value) -> Result<String, RuntimeError> {
    let mut preimage = value.clone();
    let object = preimage
        .as_object_mut()
        .ok_or_else(|| invalid("request must be an object"))?;
    object.remove("input_sha256");
    object.remove("idempotency_key");
    Ok(canonical_json_hash(&preimage))
}

fn check_worker_request(request: &Value, outer: &Map<String, Value>) -> Result<(), RuntimeError> {
    let worker = exact_object(
        request,
        &[
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
        ],
        "low_quad_draft_worker_request",
    )?;
    if worker.get("schema_version").and_then(Value::as_str)
        != Some(PRODUCTION_WEAPON_LOW_QUAD_DRAFT_REQUEST_SCHEMA_VERSION)
        || worker.get("preview_only") != Some(&Value::Bool(true))
        || worker.get("project_id") != outer.get("project_id")
        || worker.get("source_high_artifact_sha256") != outer.get("source_high_artifact_sha256")
        || worker.get("source_high_artifact_readback_sha256")
            != outer.get("source_high_artifact_readback_sha256")
        || worker.get("low_retopology_policy").and_then(Value::as_str)
            != Some(PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY)
        || worker.get("algorithm").and_then(Value::as_str)
            != Some(PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM)
    {
        return Err(invalid("nested Low quad Worker binding or policy differs"));
    }
    let worker_canonical = worker
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("nested Worker canonical_sha256 is invalid"))?;
    let mut preimage = request.clone();
    preimage
        .as_object_mut()
        .expect("Low quad Worker request object validated above")
        .remove("canonical_sha256");
    if canonical_json_hash(&preimage) != worker_canonical {
        return Err(invalid("nested Worker canonical_sha256 differs"));
    }
    let nested_hash = outer
        .get("low_quad_draft_worker_request_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("nested Worker request hash is missing"))?;
    if nested_hash != worker_canonical {
        return Err(invalid("nested Worker request hash differs"));
    }
    let draft = worker
        .get("draft")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("nested Worker draft is not an object"))?;
    let lineage = draft
        .get("source_lineage")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("nested Worker source lineage is not an object"))?;
    for (field, outer_field) in [
        ("source_high_artifact_sha256", "source_high_artifact_sha256"),
        (
            "source_high_artifact_readback_sha256",
            "source_high_artifact_readback_sha256",
        ),
    ] {
        if lineage.get(field) != outer.get(outer_field) {
            return Err(invalid("nested Worker draft lineage differs"));
        }
    }
    Ok(())
}

fn parse_prepare(value: &Value) -> Result<PrepareRequest, RuntimeError> {
    let object = exact_object(
        value,
        PREPARE_FIELDS,
        LOW_QUAD_DRAFT_DURABLE_PREPARE_SCHEMA_VERSION,
    )?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(LOW_QUAD_DRAFT_DURABLE_PREPARE_SCHEMA_VERSION)
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(LOW_QUAD_DRAFT_DURABLE_MAX_RESPONSE_BYTES)
        || object.get("writer_policy").and_then(Value::as_str)
            != Some(LOW_QUAD_DRAFT_DURABLE_WRITER_POLICY)
        || object
            .get("canonicalization_policy")
            .and_then(Value::as_str)
            != Some(LOW_QUAD_DRAFT_DURABLE_CANONICALIZATION_POLICY)
    {
        return Err(invalid("prepare schema or policy fields differ"));
    }
    required_bool(object, "source_only", true)?;
    required_bool(object, "runtime_write_performed", false)?;
    let input_sha256 = required_hash(object, "input_sha256")?;
    if request_input_hash(value)? != input_sha256 {
        return Err(invalid("input_sha256 does not bind the prepare request"));
    }
    let worker_request = object
        .get("low_quad_draft_worker_request")
        .filter(|value| value.is_object())
        .cloned()
        .ok_or_else(|| invalid("nested Worker request is missing"))?;
    check_worker_request(&worker_request, object)?;
    let worker_request_sha256 = required_hash(object, "low_quad_draft_worker_request_sha256")?;
    let base_version_id = nullable_id(object, "base_version_id")?;
    Ok(PrepareRequest {
        project_id: required_id(object, "project_id")?,
        candidate_id: required_id(object, "candidate_id")?,
        candidate_state_sha256: required_hash(object, "candidate_state_sha256")?,
        base_version_id,
        source_high_artifact_id: required_id(object, "source_high_artifact_id")?,
        source_high_artifact_object_sha256: required_hash(
            object,
            "source_high_artifact_object_sha256",
        )?,
        source_high_artifact_sha256: required_hash(object, "source_high_artifact_sha256")?,
        source_high_artifact_readback_object_sha256: required_hash(
            object,
            "source_high_artifact_readback_object_sha256",
        )?,
        source_high_artifact_readback_sha256: required_hash(
            object,
            "source_high_artifact_readback_sha256",
        )?,
        worker_request,
        worker_request_sha256,
        idempotency_key: required_id(object, "idempotency_key")?,
        input_sha256,
    })
}

fn parse_get(value: &Value) -> Result<&Map<String, Value>, RuntimeError> {
    let object = exact_object(value, GET_FIELDS, LOW_QUAD_DRAFT_DURABLE_GET_SCHEMA_VERSION)?;
    if object.get("schema_version").and_then(Value::as_str)
        != Some(LOW_QUAD_DRAFT_DURABLE_GET_SCHEMA_VERSION)
        || object.get("operation").and_then(Value::as_str)
            != Some(LOW_QUAD_DRAFT_DURABLE_OPERATION_GET)
        || object.get("writer_policy").and_then(Value::as_str)
            != Some(LOW_QUAD_DRAFT_DURABLE_WRITER_POLICY)
    {
        return Err(invalid("get schema or policy fields differ"));
    }
    required_bool(object, "source_only", true)?;
    required_bool(object, "runtime_write_performed", false)?;
    required_bool(object, "persistent_user_data_touched", false)?;
    let input = required_hash(object, "input_sha256")?;
    let mut preimage = value.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != input {
        return Err(invalid("input_sha256 does not bind the get request"));
    }
    Ok(object)
}

fn source_preflight(
    runtime: &Runtime,
    request: &PrepareRequest,
) -> Result<(forgecad_store::NativeHighDurableRecord, Vec<u8>), RuntimeError> {
    let candidate = runtime
        .candidate(&request.candidate_id)?
        .ok_or_else(|| invalid("candidate is unavailable"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != request.candidate_state_sha256
        || candidate.base_version_id != request.base_version_id
    {
        return Err(invalid(
            "candidate project/state/base-version binding differs",
        ));
    }
    let high = runtime
        .store
        .get_native_high_durable_by_candidate(&request.candidate_id)?
        .ok_or_else(|| invalid("durable Native High source is unavailable"))?;
    if high.project_id != request.project_id
        || high.candidate_id != request.candidate_id
        || high.candidate_state_sha256 != request.candidate_state_sha256
        || high.base_version_id != request.base_version_id
        || high.high_artifact_id != request.source_high_artifact_id
        || high.high_artifact_object_sha256 != request.source_high_artifact_object_sha256
        || high.high_artifact_sha256 != request.source_high_artifact_sha256
        || high.high_artifact_readback_object_sha256
            != request.source_high_artifact_readback_object_sha256
        || high.high_artifact_readback_sha256 != request.source_high_artifact_readback_sha256
    {
        return Err(invalid("source Native High durable link binding differs"));
    }
    let artifact_object = runtime
        .store
        .get_object(&request.source_high_artifact_object_sha256)?
        .ok_or_else(|| invalid("source High CAS object is unavailable"))?;
    if artifact_object.mime != GLB_MIME
        || artifact_object.kind != PRODUCTION_WEAPON_HIGH_ARTIFACT_KIND
        || artifact_object.size_bytes == 0
        || artifact_object.size_bytes > MAX_SOURCE_GLB_BYTES
    {
        return Err(invalid("source High CAS metadata differs"));
    }
    let artifact_bytes = runtime.cas_read_bounded(
        &request.source_high_artifact_object_sha256,
        MAX_SOURCE_GLB_BYTES.min(MAX_GEOMETRY_ARTIFACT_BYTES),
    )?;
    if sha256_hex(&artifact_bytes) != request.source_high_artifact_object_sha256 {
        return Err(invalid("source High GLB CAS hash differs"));
    }
    // Native High uses its own closed GLB lineage envelope rather than the
    // GeometryProgram `extras.forgecad.program_sha256` shape. Reuse the same
    // independent Runtime parser that admitted the durable Native High
    // source; applying the generic GeometryProgram inspector here would
    // reject every valid Native High artifact before Low materialization.
    let high_inspection = super::native_high_glb_readback::inspect_native_high_glb(&artifact_bytes)
        .map_err(|error| invalid(error.to_string()))?;
    if high_inspection.get("glb_sha256").and_then(Value::as_str)
        != Some(request.source_high_artifact_sha256.as_str())
        || high_inspection
            .get("source_artifact_id")
            .and_then(Value::as_str)
            != Some(request.source_high_artifact_id.as_str())
        || high_inspection
            .get("source_artifact_sha256")
            .and_then(Value::as_str)
            != Some(high.high_mesh_artifact_sha256.as_str())
    {
        return Err(invalid("source Native High GLB lineage differs"));
    }
    let readback_object = runtime
        .store
        .get_object(&request.source_high_artifact_readback_object_sha256)?
        .ok_or_else(|| invalid("source High readback CAS object is unavailable"))?;
    if readback_object.mime != JSON_MIME
        || readback_object.kind != "native-high-glb-materialize-result"
        || readback_object.size_bytes == 0
        || readback_object.size_bytes > MAX_SOURCE_JSON_BYTES
    {
        return Err(invalid("source High readback CAS metadata differs"));
    }
    let readback_bytes = runtime.cas_read_bounded(
        &request.source_high_artifact_readback_object_sha256,
        MAX_SOURCE_JSON_BYTES.min(MAX_DERIVED_JSON_BYTES),
    )?;
    if sha256_hex(&readback_bytes) != request.source_high_artifact_readback_object_sha256 {
        return Err(invalid("source High readback CAS hash differs"));
    }
    let readback: Value = serde_json::from_slice(&readback_bytes)
        .map_err(|_| invalid("source High readback JSON is invalid"))?;
    forgecad_worker_protocol::validate_native_high_glb_materialize_result(&readback)
        .map_err(invalid)?;
    if readback.get("canonical_sha256").and_then(Value::as_str)
        != Some(request.source_high_artifact_readback_sha256.as_str())
    {
        return Err(invalid("source High readback canonical binding differs"));
    }
    let mut readback_preimage = readback.clone();
    readback_preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&readback_preimage) != request.source_high_artifact_readback_sha256 {
        return Err(invalid("source High readback canonical hash is invalid"));
    }
    let embedded_glb = base64::engine::general_purpose::STANDARD
        .decode(
            readback
                .get("glb_base64")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("source High readback GLB is missing"))?,
        )
        .map_err(|_| invalid("source High readback GLB base64 is invalid"))?;
    if embedded_glb != artifact_bytes
        || readback.get("glb_sha256").and_then(Value::as_str)
            != Some(request.source_high_artifact_sha256.as_str())
    {
        return Err(invalid("source High readback GLB bytes differ"));
    }
    super::native_high_glb_readback::inspect_and_validate_against_worker_readback(
        &artifact_bytes,
        &readback["strict_readback"],
    )
    .map_err(|error| invalid(error.to_string()))?;
    Ok((high, artifact_bytes))
}

fn run_worker(request: &PrepareRequest) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    let first = super::geometry_worker::execute_sibling_worker_with_metadata(
        GEOMETRY_WORKER_BINARY,
        PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION,
        request.worker_request.clone(),
    )
    .map_err(|error| invalid(error.to_string()))?;
    let second = super::geometry_worker::execute_sibling_worker_with_metadata(
        GEOMETRY_WORKER_BINARY,
        PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION,
        request.worker_request.clone(),
    )
    .map_err(|error| invalid(error.to_string()))?;
    validate_worker_result(&first.result, request)?;
    validate_worker_result(&second.result, request)?;
    if first.result != second.result || first.build_cohort_sha256 != second.build_cohort_sha256 {
        return Err(invalid("Low quad Worker replay or build cohort differs"));
    }
    let cohort = first
        .build_cohort_sha256
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("Low quad Worker build cohort is unavailable"))?;
    let encoded = first
        .result
        .get("low_quad_draft_glb_base64")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Low quad Worker GLB is missing"))?;
    let glb = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid("Low quad Worker GLB base64 is invalid"))?;
    if glb.is_empty() || glb.len() as u64 > MAX_SOURCE_GLB_BYTES {
        return Err(invalid("Low quad Worker GLB exceeds its bound"));
    }
    Ok((first.result, glb, cohort))
}

fn validate_worker_result(value: &Value, request: &PrepareRequest) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("Low quad Worker result is not an object"))?;
    for (field, expected) in [
        (
            "schema_version",
            PRODUCTION_WEAPON_LOW_QUAD_DRAFT_RESULT_SCHEMA_VERSION,
        ),
        ("operation", PRODUCTION_WEAPON_LOW_QUAD_DRAFT_OPERATION),
        ("project_id", request.project_id.as_str()),
        (
            "source_high_artifact_sha256",
            request.source_high_artifact_sha256.as_str(),
        ),
        (
            "source_high_artifact_readback_sha256",
            request.source_high_artifact_readback_sha256.as_str(),
        ),
        (
            "low_retopology_policy",
            PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY,
        ),
        ("algorithm", PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM),
        ("edge_flow_status", "DRAFT_UNREVIEWED"),
        ("quality_status", "structural_only"),
        ("validator_status", "passed"),
    ] {
        if object.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(invalid(format!("Worker result {field} binding differs")));
        }
    }
    for (field, expected) in [
        ("explicit_quad_faces", true),
        ("auto_retopology_performed", false),
        ("retopology_derived", false),
        ("artist_authored_quad_topology", false),
        ("hard_gate_passed", true),
        ("runtime_write_performed", false),
        ("production_stage_advanced", false),
        ("promotion_eligible", false),
        ("candidate_confirmed", false),
        ("version_created", false),
        ("export_performed", false),
    ] {
        if object.get(field).and_then(Value::as_bool) != Some(expected) {
            return Err(invalid(format!("Worker result {field} flag differs")));
        }
    }
    let canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("Worker result canonical_sha256 is invalid"))?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical {
        return Err(invalid("Worker result canonical_sha256 differs"));
    }
    let glb_hash = object
        .get("low_quad_draft_artifact_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("Worker result GLB hash is invalid"))?;
    let encoded = object
        .get("low_quad_draft_glb_base64")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Worker result GLB bytes are missing"))?;
    let glb = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| invalid("Worker result GLB base64 is invalid"))?;
    if sha256_hex(&glb) != glb_hash || !strict_glb_inspection(&glb)?.hard_gate_passed {
        return Err(invalid("Worker result GLB hash or strict readback differs"));
    }
    Ok(())
}

fn canonical_json_object(mut value: Value) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    value
        .as_object_mut()
        .ok_or_else(|| invalid("durable JSON object is not an object"))?
        .insert("canonical_sha256".to_owned(), Value::String(String::new()));
    let canonical = canonical_json_hash(&value);
    value
        .as_object_mut()
        .expect("object validated above")
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(canonical.clone()),
        );
    let bytes = canonical_json_bytes(&value).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > MAX_SOURCE_JSON_BYTES {
        return Err(invalid("durable JSON object exceeds its bound"));
    }
    Ok((value, bytes, canonical))
}

fn put_reserved(
    runtime: &Runtime,
    reservation: &CasReservation,
    bytes: &[u8],
    mime: &str,
    kind: &str,
    created_at: &str,
    objects: &mut Vec<CasObject>,
) -> Result<CasObject, RuntimeError> {
    let object =
        runtime
            .store
            .put_object_reserved(reservation, bytes, None, mime, kind, created_at)?;
    objects.push(object.clone());
    Ok(object)
}

fn release(runtime: &Runtime, reservation: &CasReservation, objects: &[CasObject], cleanup: bool) {
    for object in objects.iter().rev() {
        let _ = runtime
            .store
            .release_cas_reservation_object(reservation, object, cleanup);
    }
}

fn read_json_cas(runtime: &Runtime, hash: &str, kind: &str) -> Result<Value, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid("durable JSON CAS object is unavailable"))?;
    if object.mime != JSON_MIME
        || object.kind != kind
        || object.size_bytes == 0
        || object.size_bytes > MAX_SOURCE_JSON_BYTES
    {
        return Err(invalid("durable JSON CAS metadata differs"));
    }
    let bytes = runtime.cas_read_bounded(hash, MAX_SOURCE_JSON_BYTES)?;
    if sha256_hex(&bytes) != hash {
        return Err(invalid("durable JSON CAS hash differs"));
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid("durable JSON CAS bytes are invalid"))
}

fn read_glb_cas(runtime: &Runtime, hash: &str, kind: &str) -> Result<Vec<u8>, RuntimeError> {
    let object = runtime
        .store
        .get_object(hash)?
        .ok_or_else(|| invalid("durable Low quad GLB is unavailable"))?;
    if object.mime != GLB_MIME
        || object.kind != kind
        || object.size_bytes == 0
        || object.size_bytes > MAX_SOURCE_GLB_BYTES
    {
        return Err(invalid("durable Low quad GLB metadata differs"));
    }
    let bytes =
        runtime.cas_read_bounded(hash, MAX_SOURCE_GLB_BYTES.min(MAX_GEOMETRY_ARTIFACT_BYTES))?;
    if sha256_hex(&bytes) != hash || !strict_glb_inspection(&bytes)?.hard_gate_passed {
        return Err(invalid("durable Low quad GLB strict readback failed"));
    }
    Ok(bytes)
}

fn build_worker_request_from_result(
    result: &Value,
    record: &LowQuadDraftDurableRecord,
) -> Result<Value, RuntimeError> {
    let mut request = json!({
        "schema_version":PRODUCTION_WEAPON_LOW_QUAD_DRAFT_REQUEST_SCHEMA_VERSION,
        "preview_only":true,
        "project_id":record.project_id,
        "source_high_artifact_sha256":record.source_high_artifact_sha256,
        "source_high_artifact_readback_sha256":record.source_high_artifact_readback_sha256,
        "source_high_part_id":result["source_high_part_id"],
        "source_high_node_id":result["source_high_node_id"],
        "source_high_material_zone_id":result["source_high_material_zone_id"],
        "draft":result["draft"],
        "max_vertices":result["vertex_budget"],
        "max_edges":result["edge_budget"],
        "max_faces":result["face_budget"],
        "low_retopology_policy":PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY,
        "algorithm":PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM,
        "canonical_sha256":""
    });
    request["canonical_sha256"] = Value::String({
        let mut preimage = request.clone();
        preimage
            .as_object_mut()
            .expect("reconstructed Low quad Worker request object")
            .remove("canonical_sha256");
        canonical_json_hash(&preimage)
    });
    Ok(request)
}

fn validate_link_value(
    value: &Value,
    record: &LowQuadDraftDurableRecord,
) -> Result<(), RuntimeError> {
    let canonical = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("durable Low quad link canonical hash is invalid"))?;
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != canonical {
        return Err(invalid("durable Low quad link canonical hash differs"));
    }
    for (field, expected) in [
        ("schema_version", LOW_QUAD_DRAFT_DURABLE_LINK_SCHEMA_VERSION),
        ("operation", LOW_QUAD_DRAFT_DURABLE_OPERATION_PREPARE),
        ("project_id", record.project_id.as_str()),
        ("candidate_id", record.candidate_id.as_str()),
        (
            "candidate_state_sha256",
            record.candidate_state_sha256.as_str(),
        ),
        (
            "source_high_artifact_id",
            record.source_high_artifact_id.as_str(),
        ),
        (
            "source_high_artifact_object_sha256",
            record.source_high_artifact_object_sha256.as_str(),
        ),
        (
            "source_high_artifact_sha256",
            record.source_high_artifact_sha256.as_str(),
        ),
        (
            "source_high_artifact_readback_object_sha256",
            record.source_high_artifact_readback_object_sha256.as_str(),
        ),
        (
            "source_high_artifact_readback_sha256",
            record.source_high_artifact_readback_sha256.as_str(),
        ),
        (
            "worker_result_object_sha256",
            record.worker_result_object_sha256.as_str(),
        ),
        ("worker_result_sha256", record.worker_result_sha256.as_str()),
        (
            "artifact_object_sha256",
            record.artifact_object_sha256.as_str(),
        ),
        ("artifact_sha256", record.artifact_sha256.as_str()),
        (
            "readback_object_sha256",
            record.readback_object_sha256.as_str(),
        ),
        ("readback_sha256", record.readback_sha256.as_str()),
        ("low_retopology_policy", LOW_QUAD_DRAFT_DURABLE_POLICY),
        ("edge_flow_status", "DRAFT_UNREVIEWED"),
        ("quality_status", "structural_only"),
        ("validator_status", "passed"),
        ("materialization_status", MATERIALIZATION_STATUS),
        ("writer_policy", LOW_QUAD_DRAFT_DURABLE_WRITER_POLICY),
    ] {
        if value.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(invalid(format!("durable Low quad link {field} differs")));
        }
    }
    for (field, expected) in [
        ("hard_gate_passed", true),
        ("explicit_quad_faces", true),
        ("auto_retopology_performed", false),
        ("retopology_derived", false),
        ("artist_authored_quad_topology", false),
        ("promotion_eligible", false),
        ("runtime_write_performed", true),
        ("production_stage_advanced", false),
        ("candidate_confirmed", false),
        ("version_created", false),
        ("export_performed", false),
    ] {
        if value.get(field).and_then(Value::as_bool) != Some(expected) {
            return Err(invalid(format!(
                "durable Low quad link {field} flag differs"
            )));
        }
    }
    Ok(())
}

fn build_link(
    request: &PrepareRequest,
    record_values: (&str, &str, &str, u64, &str, &str),
    worker: &Value,
    created_at: &str,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    let (worker_object, artifact_object, readback_object, artifact_size, worker_sha, readback_sha) =
        record_values;
    let seed = canonical_json_hash(&json!({
        "project_id":request.project_id,
        "candidate_id":request.candidate_id,
        "candidate_state_sha256":request.candidate_state_sha256,
        "source_high_artifact_sha256":request.source_high_artifact_sha256,
        "worker_result_object_sha256":worker_object,
        "worker_result_sha256":worker_sha,
        "artifact_object_sha256":artifact_object,
        "artifact_sha256":worker["low_quad_draft_artifact_sha256"],
        "readback_object_sha256":readback_object,
        "readback_sha256":readback_sha,
        "input_sha256":request.input_sha256
    }));
    let value = json!({
        "schema_version":LOW_QUAD_DRAFT_DURABLE_LINK_SCHEMA_VERSION,
        "operation":LOW_QUAD_DRAFT_DURABLE_OPERATION_PREPARE,
        "link_id":format!("low-quad-draft-link-{}", &seed[..24]),
        "project_id":request.project_id,
        "candidate_id":request.candidate_id,
        "candidate_state_sha256":request.candidate_state_sha256,
        "base_version_id":request.base_version_id,
        "source_high_artifact_id":request.source_high_artifact_id,
        "source_high_artifact_object_sha256":request.source_high_artifact_object_sha256,
        "source_high_artifact_sha256":request.source_high_artifact_sha256,
        "source_high_artifact_readback_object_sha256":request.source_high_artifact_readback_object_sha256,
        "source_high_artifact_readback_sha256":request.source_high_artifact_readback_sha256,
        "worker_result_object_sha256":worker_object,
        "worker_result_sha256":worker_sha,
        "artifact_object_sha256":artifact_object,
        "artifact_sha256":worker["low_quad_draft_artifact_sha256"],
        "artifact_size_bytes":artifact_size,
        "readback_object_sha256":readback_object,
        "readback_sha256":readback_sha,
        "low_retopology_policy":LOW_QUAD_DRAFT_DURABLE_POLICY,
        "edge_flow_status":"DRAFT_UNREVIEWED",
        "quality_status":"structural_only",
        "visual_status":"NOT_RUN",
        "human_status":"NOT_RUN",
        "engine_status":"NOT_RUN",
        "distribution_status":"NOT_RUN",
        "validator_status":"passed",
        "hard_gate_passed":true,
        "explicit_quad_faces":true,
        "auto_retopology_performed":false,
        "retopology_derived":false,
        "artist_authored_quad_topology":false,
        "promotion_eligible":false,
        "runtime_write_performed":true,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "writer_policy":LOW_QUAD_DRAFT_DURABLE_WRITER_POLICY,
        "materialization_status":MATERIALIZATION_STATUS,
        "limitations":LOW_QUAD_DRAFT_DURABLE_LIMITATIONS,
        "request_sha256":request.worker_request_sha256,
        "input_sha256":request.input_sha256,
        "idempotency_key":request.idempotency_key,
        "canonical_sha256":"",
        "created_at":created_at
    });
    let (value, bytes, canonical) = canonical_json_object(value)?;
    Ok((value, bytes, canonical))
}

fn build_readback(
    request: &PrepareRequest,
    worker: &Value,
    artifact_sha256: &str,
    worker_result_sha256: &str,
) -> Result<(Value, Vec<u8>, String), RuntimeError> {
    canonical_json_object(json!({
        "schema_version":LOW_QUAD_DRAFT_DURABLE_ARTIFACT_READBACK_SCHEMA_VERSION,
        "artifact_sha256":artifact_sha256,
        "artifact_object_sha256":artifact_sha256,
        "source_high_artifact_sha256":request.source_high_artifact_sha256,
        "source_high_artifact_readback_sha256":request.source_high_artifact_readback_sha256,
        "worker_result_sha256":worker_result_sha256,
        "worker_readback":worker["low_quad_draft_readback"],
        "validator_status":"passed",
        "hard_gate_passed":true,
        "quality_status":"structural_only",
        "edge_flow_status":"DRAFT_UNREVIEWED",
        "promotion_eligible":false,
        "production_stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "canonical_sha256":""
    }))
}

fn record_canonical(record: &LowQuadDraftDurableRecord) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn restart_revalidate(
    runtime: &Runtime,
    record: &LowQuadDraftDurableRecord,
) -> Result<Value, RuntimeError> {
    let request = PrepareRequest {
        project_id: record.project_id.clone(),
        candidate_id: record.candidate_id.clone(),
        candidate_state_sha256: record.candidate_state_sha256.clone(),
        base_version_id: record.base_version_id.clone(),
        source_high_artifact_id: record.source_high_artifact_id.clone(),
        source_high_artifact_object_sha256: record.source_high_artifact_object_sha256.clone(),
        source_high_artifact_sha256: record.source_high_artifact_sha256.clone(),
        source_high_artifact_readback_object_sha256: record
            .source_high_artifact_readback_object_sha256
            .clone(),
        source_high_artifact_readback_sha256: record.source_high_artifact_readback_sha256.clone(),
        worker_request: Value::Null,
        worker_request_sha256: record.request_sha256.clone(),
        idempotency_key: record.idempotency_key.clone(),
        input_sha256: record.input_sha256.clone(),
    };
    let _ = source_preflight(runtime, &request)?;
    let worker_result = read_json_cas(
        runtime,
        &record.worker_result_object_sha256,
        LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND,
    )?;
    if worker_result
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(record.worker_result_sha256.as_str())
    {
        return Err(invalid("stored Low quad Worker result canonical differs"));
    }
    let worker_request = build_worker_request_from_result(&worker_result, record)?;
    let mut request = request;
    request.worker_request = worker_request;
    check_worker_request(
        &request.worker_request,
        &json!({
            "project_id":request.project_id,
            "source_high_artifact_sha256":request.source_high_artifact_sha256,
            "source_high_artifact_readback_sha256":request.source_high_artifact_readback_sha256,
            "low_quad_draft_worker_request_sha256":request.worker_request_sha256
        })
        .as_object()
        .expect("restart outer object"),
    )?;
    let (replayed_result, replayed_glb, cohort) = run_worker(&request)?;
    if replayed_result != worker_result
        || cohort != record.worker_build_cohort_sha256
        || sha256_hex(&replayed_glb) != record.artifact_sha256
    {
        return Err(invalid("stored Low quad Worker replay differs"));
    }
    let artifact = read_glb_cas(
        runtime,
        &record.artifact_object_sha256,
        LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND,
    )?;
    if artifact != replayed_glb || record.artifact_object_sha256 != record.artifact_sha256 {
        return Err(invalid("stored Low quad artifact bytes differ"));
    }
    let readback = read_json_cas(
        runtime,
        &record.readback_object_sha256,
        ARTIFACT_READBACK_KIND,
    )?;
    if readback.get("schema_version").and_then(Value::as_str)
        != Some(LOW_QUAD_DRAFT_DURABLE_ARTIFACT_READBACK_SCHEMA_VERSION)
        || readback.get("artifact_sha256").and_then(Value::as_str)
            != Some(record.artifact_sha256.as_str())
        || readback.get("worker_result_sha256").and_then(Value::as_str)
            != Some(record.worker_result_sha256.as_str())
        || readback.get("canonical_sha256").and_then(Value::as_str)
            != Some(record.readback_sha256.as_str())
    {
        return Err(invalid("stored Low quad readback binding differs"));
    }
    let link = read_json_cas(
        runtime,
        &record.link_object_sha256,
        LOW_QUAD_DRAFT_DURABLE_OBJECT_KIND,
    )?;
    validate_link_value(&link, record)?;
    Ok(link)
}

fn output(
    schema_version: &str,
    operation: &str,
    record: &LowQuadDraftDurableRecord,
    link: Value,
    replayed: bool,
    runtime_write_performed: bool,
) -> Result<Value, RuntimeError> {
    let mut output = json!({
        "schema_version":schema_version,
        "operation":operation,
        "project_id":record.project_id,
        "candidate_id":record.candidate_id,
        "candidate_state_sha256":record.candidate_state_sha256,
        "base_version_id":record.base_version_id,
        "link_id":record.link_id,
        "link_object_sha256":record.link_object_sha256,
        "durable_link":link,
        "worker_result_object_sha256":record.worker_result_object_sha256,
        "worker_result_sha256":record.worker_result_sha256,
        "artifact_object_sha256":record.artifact_object_sha256,
        "artifact_sha256":record.artifact_sha256,
        "readback_object_sha256":record.readback_object_sha256,
        "readback_sha256":record.readback_sha256,
        "request_input_sha256":record.input_sha256,
        "idempotency_key":record.idempotency_key,
        "replayed":replayed,
        "restart_hash_verified":true,
        "runtime_write_performed":runtime_write_performed,
        "persistent_user_data_touched":runtime_write_performed,
        "production_stage_advanced":false,
        "promotion_eligible":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"structural_only",
        "edge_flow_status":"DRAFT_UNREVIEWED",
        "limitations":LOW_QUAD_DRAFT_DURABLE_LIMITATIONS,
        "canonicalization_policy":LOW_QUAD_DRAFT_DURABLE_CANONICALIZATION_POLICY,
        "canonical_sha256":""
    });
    output["canonical_sha256"] = Value::String(canonical_json_hash(&output));
    let bytes = canonical_json_bytes(&output).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() as u64 > LOW_QUAD_DRAFT_DURABLE_MAX_RESPONSE_BYTES {
        return Err(invalid("Low quad durable response exceeds its bound"));
    }
    Ok(output)
}

impl Runtime {
    pub fn low_quad_draft_durable_prepare(&self, value: Value) -> Result<Value, RuntimeError> {
        let request = parse_prepare(&value)?;
        if let Some(existing) = self
            .store
            .get_low_quad_draft_durable(&request.project_id, &request.idempotency_key)?
        {
            if existing.input_sha256 != request.input_sha256 {
                return Err(invalid("idempotency key is bound to another input"));
            }
            let link = restart_revalidate(self, &existing)?;
            return output(
                LOW_QUAD_DRAFT_DURABLE_PREPARE_RESULT_SCHEMA_VERSION,
                LOW_QUAD_DRAFT_DURABLE_OPERATION_PREPARE,
                &existing,
                link,
                true,
                true,
            );
        }
        let _ = source_preflight(self, &request)?;
        let (worker_result, artifact_bytes, cohort) = run_worker(&request)?;
        let worker_result_sha256 = worker_result
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| invalid("Low quad Worker result canonical hash is missing"))?
            .to_owned();
        let artifact_sha256 = sha256_hex(&artifact_bytes);
        if worker_result
            .get("low_quad_draft_artifact_sha256")
            .and_then(Value::as_str)
            != Some(artifact_sha256.as_str())
        {
            return Err(invalid("Worker artifact hash differs from bytes"));
        }
        let worker_result_value = canonical_json_object(worker_result)?;
        let readback_value = build_readback(
            &request,
            &worker_result_value.0,
            &artifact_sha256,
            &worker_result_sha256,
        )?;
        let created_at = now_string();
        let reservation = self.store.begin_cas_reservation();
        let mut objects = Vec::new();
        let worker_object = match put_reserved(
            self,
            &reservation,
            &worker_result_value.1,
            JSON_MIME,
            LOW_QUAD_DRAFT_DURABLE_WORKER_RESULT_KIND,
            &created_at,
            &mut objects,
        ) {
            Ok(object) => object,
            Err(error) => {
                release(self, &reservation, &objects, true);
                return Err(error);
            }
        };
        let artifact_object = match put_reserved(
            self,
            &reservation,
            &artifact_bytes,
            GLB_MIME,
            LOW_QUAD_DRAFT_DURABLE_ARTIFACT_KIND,
            &created_at,
            &mut objects,
        ) {
            Ok(object) => object,
            Err(error) => {
                release(self, &reservation, &objects, true);
                return Err(error);
            }
        };
        let readback_object = match put_reserved(
            self,
            &reservation,
            &readback_value.1,
            JSON_MIME,
            LOW_QUAD_DRAFT_DURABLE_READBACK_KIND,
            &created_at,
            &mut objects,
        ) {
            Ok(object) => object,
            Err(error) => {
                release(self, &reservation, &objects, true);
                return Err(error);
            }
        };
        let (link_value, link_bytes, _link_canonical) = build_link(
            &request,
            (
                &worker_object.record.sha256,
                &artifact_object.record.sha256,
                &readback_object.record.sha256,
                artifact_bytes.len() as u64,
                &worker_result_sha256,
                &readback_value.2,
            ),
            &worker_result_value.0,
            &created_at,
        )?;
        let link_object = match put_reserved(
            self,
            &reservation,
            &link_bytes,
            JSON_MIME,
            LOW_QUAD_DRAFT_DURABLE_OBJECT_KIND,
            &created_at,
            &mut objects,
        ) {
            Ok(object) => object,
            Err(error) => {
                release(self, &reservation, &objects, true);
                return Err(error);
            }
        };
        let link_id = link_value
            .get("link_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("Low quad link_id is missing"))?
            .to_owned();
        let mut record = LowQuadDraftDurableRecord {
            schema_version: LOW_QUAD_DRAFT_DURABLE_RECORD_SCHEMA_VERSION.to_owned(),
            project_id: request.project_id.clone(),
            candidate_id: request.candidate_id.clone(),
            candidate_state_sha256: request.candidate_state_sha256.clone(),
            base_version_id: request.base_version_id.clone(),
            source_high_artifact_id: request.source_high_artifact_id.clone(),
            source_high_artifact_object_sha256: request.source_high_artifact_object_sha256.clone(),
            source_high_artifact_sha256: request.source_high_artifact_sha256.clone(),
            source_high_artifact_readback_object_sha256: request
                .source_high_artifact_readback_object_sha256
                .clone(),
            source_high_artifact_readback_sha256: request
                .source_high_artifact_readback_sha256
                .clone(),
            worker_result_object_sha256: worker_object.record.sha256.clone(),
            worker_result_sha256: worker_result_sha256.clone(),
            artifact_object_sha256: artifact_object.record.sha256.clone(),
            artifact_sha256: artifact_sha256.clone(),
            artifact_size_bytes: artifact_bytes.len() as u64,
            readback_object_sha256: readback_object.record.sha256.clone(),
            readback_sha256: readback_value.2.clone(),
            link_id,
            link_object_sha256: link_object.record.sha256.clone(),
            request_sha256: request.worker_request_sha256.clone(),
            input_sha256: request.input_sha256.clone(),
            idempotency_key: request.idempotency_key.clone(),
            worker_build_cohort_sha256: cohort,
            materialization_status: MATERIALIZATION_STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at,
        };
        record.canonical_sha256 = record_canonical(&record)?;
        let source_object = self
            .store
            .get_object(&request.source_high_artifact_object_sha256)?
            .ok_or_else(|| invalid("source High object disappeared before commit"))?;
        let source_readback_object = self
            .store
            .get_object(&request.source_high_artifact_readback_object_sha256)?
            .ok_or_else(|| invalid("source High readback disappeared before commit"))?;
        let stored = match self.store.record_low_quad_draft_durable_with_replay(
            &record,
            &source_object,
            &source_readback_object,
            &worker_object.record,
            &artifact_object.record,
            &readback_object.record,
            &link_object.record,
        ) {
            Ok(value) => value,
            Err(error) => {
                release(self, &reservation, &objects, true);
                return Err(error.into());
            }
        };
        release(self, &reservation, &objects, false);
        let link = if stored.1 {
            restart_revalidate(self, &stored.0)?
        } else {
            link_value
        };
        output(
            LOW_QUAD_DRAFT_DURABLE_PREPARE_RESULT_SCHEMA_VERSION,
            LOW_QUAD_DRAFT_DURABLE_OPERATION_PREPARE,
            &stored.0,
            link,
            stored.1,
            true,
        )
    }

    pub fn low_quad_draft_durable_get(&self, value: Value) -> Result<Value, RuntimeError> {
        let request = parse_get(&value)?;
        let project_id = required_id(request, "project_id")?;
        let idempotency_key = required_id(request, "idempotency_key")?;
        let record = self
            .store
            .get_low_quad_draft_durable(&project_id, &idempotency_key)?
            .ok_or_else(|| invalid("Low quad durable record is unavailable"))?;
        let expected = [
            ("candidate_id", record.candidate_id.as_str()),
            (
                "candidate_state_sha256",
                record.candidate_state_sha256.as_str(),
            ),
            ("link_id", record.link_id.as_str()),
            ("link_object_sha256", record.link_object_sha256.as_str()),
            (
                "source_high_artifact_id",
                record.source_high_artifact_id.as_str(),
            ),
            (
                "source_high_artifact_sha256",
                record.source_high_artifact_sha256.as_str(),
            ),
            (
                "worker_result_object_sha256",
                record.worker_result_object_sha256.as_str(),
            ),
            ("worker_result_sha256", record.worker_result_sha256.as_str()),
            (
                "artifact_object_sha256",
                record.artifact_object_sha256.as_str(),
            ),
            ("artifact_sha256", record.artifact_sha256.as_str()),
            (
                "readback_object_sha256",
                record.readback_object_sha256.as_str(),
            ),
            ("readback_sha256", record.readback_sha256.as_str()),
        ];
        let base_version_id = nullable_id(request, "base_version_id")?;
        if base_version_id != record.base_version_id
            || expected.iter().any(|(field, expected)| {
                request.get(*field).and_then(Value::as_str) != Some(*expected)
            })
        {
            return Err(invalid("Low quad durable get binding differs"));
        }
        let link = restart_revalidate(self, &record)?;
        output(
            LOW_QUAD_DRAFT_DURABLE_GET_RESULT_SCHEMA_VERSION,
            LOW_QUAD_DRAFT_DURABLE_OPERATION_GET,
            &record,
            link,
            false,
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    fn authoring_program(project_id: &str) -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project_id,
            "representation_plan_sha256":"b".repeat(64),
            "operator_catalog_sha256":crate::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":1,
                "max_triangles":32,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"low-quad-restart-panel",
                "operator_id":"forgecad.geometry.authoring-mesh@1",
                "inputs":[],
                "parameters":{
                    "shape":"authoring-mesh",
                    "topology_policy":"triangle-quad-manifold-with-boundary@1",
                    "vertices":[
                        {"element_id":"v0","position_m":[-1.0,-1.0,0.0]},
                        {"element_id":"v1","position_m":[1.0,-1.0,0.0]},
                        {"element_id":"v2","position_m":[1.0,1.0,0.0]},
                        {"element_id":"v3","position_m":[-1.0,1.0,0.0]}
                    ],
                    "edges":[
                        {"element_id":"e01","vertex_ids":["v0","v1"]},
                        {"element_id":"e03","vertex_ids":["v0","v3"]},
                        {"element_id":"e12","vertex_ids":["v1","v2"]},
                        {"element_id":"e23","vertex_ids":["v2","v3"]}
                    ],
                    "loops":[
                        {"element_id":"l0","face_id":"f0","ordinal":0,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
                        {"element_id":"l1","face_id":"f0","ordinal":1,"vertex_id":"v1","edge_id":"e12","edge_forward":true},
                        {"element_id":"l2","face_id":"f0","ordinal":2,"vertex_id":"v2","edge_id":"e23","edge_forward":true},
                        {"element_id":"l3","face_id":"f0","ordinal":3,"vertex_id":"v3","edge_id":"e03","edge_forward":false}
                    ],
                    "faces":[{"element_id":"f0","loop_ids":["l0","l1","l2","l3"]}],
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"low-quad-restart-panel",
                "input_node_ids":["low-quad-restart-panel"],
                "material_zone_id":"zone-low-quad-shell",
                "solid":false
            }]
        });
        let hash = crate::hash_geometry_program_with_runtime_worker(&program)
            .expect("GeometryProgram hash");
        program["canonical_sha256"] = hash["canonical_sha256"].clone();
        program
    }

    fn expected_canonical_mesh(
        projection: &Value,
        project_id: &str,
        candidate_id: &str,
        candidate_state_sha256: &str,
        base_version_id: Value,
        source_program_object_sha256: &str,
        source_program_sha256: &str,
        source_artifact_object_sha256: &str,
        source_artifact_sha256: &str,
        source_artifact_readback_object_sha256: &str,
        source_artifact_readback_sha256: &str,
        source_lineage_sha256: &str,
    ) -> Value {
        let canonical_mesh_id = projection["mesh_id"].as_str().expect("projection mesh id");
        let mesh_sha256 = projection["mesh_sha256"]
            .as_str()
            .expect("projection mesh sha");
        let original_id = projection["original_identity"]["identity_id"]
            .as_str()
            .expect("projection original identity");
        let evaluated_id = projection["evaluated_identity"]["identity_id"]
            .as_str()
            .expect("projection evaluated identity");
        let mut canonical = json!({
            "schema_version":"AuthoringMeshCanonical@1",
            "canonical_mesh_id":canonical_mesh_id,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "base_version_id":base_version_id,
            "authoring_node_id":"low-quad-restart-panel",
            "part_id":"low-quad-restart-panel",
            "source_program_object_sha256":source_program_object_sha256,
            "source_program_sha256":source_program_sha256,
            "source_artifact_object_sha256":source_artifact_object_sha256,
            "source_artifact_sha256":source_artifact_sha256,
            "source_artifact_readback_object_sha256":source_artifact_readback_object_sha256,
            "source_artifact_readback_sha256":source_artifact_readback_sha256,
            "source_lineage_sha256":source_lineage_sha256,
            "representation":"runtime-owned-original-half-edge@1",
            "storage_policy":"runtime-owned-sqlite-cas-canonical-authoring-mesh@1",
            "writer_policy":"forgecad-runtime-only-state-writer@1",
            "original_identity":{
                "identity_id":original_id,
                "namespace":"original",
                "identity_kind":"runtime-owned-original-authoring@1",
                "element_id_policy":"lineage-scoped-opaque-not-cross-version-stable@1",
                "topology_sha256":mesh_sha256,
                "source_lineage_sha256":source_lineage_sha256,
                "stability_scope":"same-canonical-mesh-lineage-only@1"
            },
            "evaluated_identity":{
                "identity_id":evaluated_id,
                "namespace":"evaluated",
                "identity_kind":"runtime-derived-evaluated-artifact-readback@1",
                "element_id_policy":"artifact-local-no-authoring-bijection@1",
                "correspondence_policy":"non-bijective-derived-only@1",
                "artifact_object_sha256":source_artifact_object_sha256,
                "artifact_readback_sha256":source_artifact_readback_sha256,
                "source_lineage_sha256":source_lineage_sha256,
                "cross_version_stable":false
            },
            "cross_version_stable":false,
            "cross_version_stability":{
                "status":"not-proven@1",
                "scope":"same-canonical-mesh-lineage-only@1",
                "stable_id_claim":"none-across-revisions@1",
                "deleted_id_reuse_policy":"not-proven-and-not-a-contract@1",
                "new_id_policy":"lineage-operation-parent-derived-draft-only@1",
                "evaluated_id_policy":"artifact-local-unstable-derived-only@1"
            },
            "counts":projection["counts"],
            "vertices":projection["vertices"],
            "edges":projection["edges"],
            "half_edges":projection["half_edges"],
            "corners":projection["corners"],
            "faces":projection["faces"],
            "loops":projection["loops"],
            "rings":projection["rings"],
            "topology":projection["topology"],
            "canonicalization_policy":"canonical-json-sha256-excluding-canonical-sha256@1",
            "runtime_write_performed":true,
            "persistent_user_data_touched":true,
            "stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "quality_status":"structural_only",
            "canonical_sha256":""
        });
        canonical["canonical_sha256"] = Value::String(canonical_json_hash(&canonical));
        canonical
    }

    fn high_request(
        canonical_mesh: Value,
        candidate_id: &str,
        candidate_state_sha256: &str,
    ) -> Value {
        let source_mesh_sha256 = canonical_mesh["canonical_sha256"]
            .as_str()
            .expect("canonical mesh hash")
            .to_owned();
        let source_authoring_mesh = json!({
            "schema_version":"HighWorkerAuthoringMeshAdapter@1",
            "canonical_mesh":canonical_mesh,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "head_candidate_id":candidate_id,
            "head_candidate_state_sha256":candidate_state_sha256,
            "source_mesh_sha256":source_mesh_sha256
        });
        let detail_graph = json!({
            "schema_version":"DetailGraph@1",
            "nodes":[{
                "node_id":"low-quad-floating-detail",
                "kind":"floating_detail",
                "parent_part_id":"low-quad-restart-panel",
                "parent_node_id":null,
                "source_edge":null,
                "width_m":null,
                "count":null,
                "sharpness":null,
                "center_m":[0.0,0.0,2.0],
                "size_m":[1.0,1.0,1.0]
            }]
        });
        let mut request = json!({
            "schema_version":"HighMeshWorkerRequest@1",
            "operation":"forgecad.production.high-mesh-prepare@1",
            "source_authoring_mesh":source_authoring_mesh,
            "source_authoring_mesh_sha256":"",
            "detail_graph":detail_graph,
            "detail_graph_canonical_sha256":"",
            "budgets":{
                "max_detail_nodes":16,
                "max_output_vertices":1024,
                "max_output_triangles":2048
            },
            "canonical_sha256":""
        });
        request["source_authoring_mesh_sha256"] =
            Value::String(canonical_json_hash(&request["source_authoring_mesh"]));
        request["detail_graph_canonical_sha256"] =
            Value::String(canonical_json_hash(&request["detail_graph"]));
        request["canonical_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn native_prepare_request(
        project_id: &str,
        candidate_id: &str,
        candidate_state_sha256: &str,
        base_version_id: Value,
        source_mesh_id: &str,
        source_mesh_object_sha256: &str,
        source_mesh_sha256: &str,
        high_mesh_request: Value,
        idempotency_key: &str,
    ) -> Value {
        let mut request = json!({
            "schema_version":"NativeHighDurablePrepareRequest@1",
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "base_version_id":base_version_id,
            "source_authoring_mesh_id":source_mesh_id,
            "source_authoring_mesh_object_sha256":source_mesh_object_sha256,
            "source_authoring_mesh_sha256":source_mesh_sha256,
            "high_mesh_request":high_mesh_request,
            "high_mesh_request_sha256":"",
            "idempotency_key":idempotency_key,
            "max_response_bytes":1048576,
            "source_only":true,
            "runtime_write_performed":false,
            "writer_policy":"forgecad-runtime-only-state-writer@1",
            "canonicalization_policy":LOW_QUAD_DRAFT_DURABLE_CANONICALIZATION_POLICY,
            "input_sha256":""
        });
        request["high_mesh_request_sha256"] =
            request["high_mesh_request"]["canonical_sha256"].clone();
        request["input_sha256"] = Value::String({
            let mut preimage = request.clone();
            preimage
                .as_object_mut()
                .expect("Native High request object")
                .remove("input_sha256");
            preimage
                .as_object_mut()
                .expect("Native High request object")
                .remove("idempotency_key");
            canonical_json_hash(&preimage)
        });
        request
    }

    fn low_quad_worker_request(
        project_id: &str,
        source_high_artifact_sha256: &str,
        source_high_artifact_readback_sha256: &str,
    ) -> Value {
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
            "vertices":vertices
                .iter()
                .map(|(id, position)| json!({"element_id":id,"position_m":position}))
                .collect::<Vec<_>>(),
            "edges":edges,
            "loops":loops,
            "faces":face_values,
            "position_m":[0.0,0.0,0.0],
            "rotation_rad":[0.0,0.0,0.0]
        });
        let source_lineage = json!({
            "source_high_artifact_sha256":source_high_artifact_sha256,
            "source_high_artifact_readback_sha256":source_high_artifact_readback_sha256,
            "source_high_part_id":"low-quad-restart-panel",
            "source_high_node_id":"low-quad-restart-panel",
            "source_high_material_zone_id":"zone-low-quad-shell"
        });
        let mut request = json!({
            "schema_version":PRODUCTION_WEAPON_LOW_QUAD_DRAFT_REQUEST_SCHEMA_VERSION,
            "preview_only":true,
            "project_id":project_id,
            "source_high_artifact_sha256":source_high_artifact_sha256,
            "source_high_artifact_readback_sha256":source_high_artifact_readback_sha256,
            "source_high_part_id":"low-quad-restart-panel",
            "source_high_node_id":"low-quad-restart-panel",
            "source_high_material_zone_id":"zone-low-quad-shell",
            "draft":{
                "schema_version":"LowQuadRetopologyDraft@1",
                "source_lineage":source_lineage,
                "authoring_mesh":authoring_mesh
            },
            "max_vertices":128,
            "max_edges":128,
            "max_faces":64,
            "low_retopology_policy":PRODUCTION_WEAPON_LOW_QUAD_DRAFT_POLICY,
            "algorithm":PRODUCTION_WEAPON_LOW_QUAD_DRAFT_ALGORITHM,
            "canonical_sha256":""
        });
        request["canonical_sha256"] = Value::String({
            let mut preimage = request.clone();
            preimage
                .as_object_mut()
                .expect("Low quad Worker request object")
                .remove("canonical_sha256");
            canonical_json_hash(&preimage)
        });
        request
    }

    fn low_prepare_request(
        project_id: &str,
        candidate_id: &str,
        candidate_state_sha256: &str,
        base_version_id: Value,
        source_high_artifact_id: &str,
        source_high_artifact_object_sha256: &str,
        source_high_artifact_sha256: &str,
        source_high_artifact_readback_object_sha256: &str,
        source_high_artifact_readback_sha256: &str,
        worker_request: Value,
        idempotency_key: &str,
    ) -> Value {
        let mut request = json!({
            "schema_version":LOW_QUAD_DRAFT_DURABLE_PREPARE_SCHEMA_VERSION,
            "project_id":project_id,
            "candidate_id":candidate_id,
            "candidate_state_sha256":candidate_state_sha256,
            "base_version_id":base_version_id,
            "source_high_artifact_id":source_high_artifact_id,
            "source_high_artifact_object_sha256":source_high_artifact_object_sha256,
            "source_high_artifact_sha256":source_high_artifact_sha256,
            "source_high_artifact_readback_object_sha256":source_high_artifact_readback_object_sha256,
            "source_high_artifact_readback_sha256":source_high_artifact_readback_sha256,
            "low_quad_draft_worker_request":worker_request,
            "low_quad_draft_worker_request_sha256":"",
            "idempotency_key":idempotency_key,
            "max_response_bytes":LOW_QUAD_DRAFT_DURABLE_MAX_RESPONSE_BYTES,
            "source_only":true,
            "runtime_write_performed":false,
            "writer_policy":LOW_QUAD_DRAFT_DURABLE_WRITER_POLICY,
            "canonicalization_policy":LOW_QUAD_DRAFT_DURABLE_CANONICALIZATION_POLICY,
            "input_sha256":""
        });
        request["low_quad_draft_worker_request_sha256"] =
            request["low_quad_draft_worker_request"]["canonical_sha256"].clone();
        request["input_sha256"] = Value::String(request_input_hash(&request).expect("input hash"));
        request
    }

    fn low_get_request(first: &Value, request: &Value) -> Value {
        let mut get = json!({
            "schema_version":LOW_QUAD_DRAFT_DURABLE_GET_SCHEMA_VERSION,
            "operation":LOW_QUAD_DRAFT_DURABLE_OPERATION_GET,
            "project_id":request["project_id"],
            "candidate_id":request["candidate_id"],
            "candidate_state_sha256":request["candidate_state_sha256"],
            "base_version_id":request["base_version_id"],
            "link_id":first["link_id"],
            "link_object_sha256":first["link_object_sha256"],
            "source_high_artifact_id":request["source_high_artifact_id"],
            "source_high_artifact_sha256":request["source_high_artifact_sha256"],
            "worker_result_object_sha256":first["worker_result_object_sha256"],
            "worker_result_sha256":first["worker_result_sha256"],
            "artifact_object_sha256":first["artifact_object_sha256"],
            "artifact_sha256":first["artifact_sha256"],
            "readback_object_sha256":first["readback_object_sha256"],
            "readback_sha256":first["readback_sha256"],
            "idempotency_key":request["idempotency_key"],
            "source_only":true,
            "writer_policy":LOW_QUAD_DRAFT_DURABLE_WRITER_POLICY,
            "runtime_write_performed":false,
            "persistent_user_data_touched":false,
            "input_sha256":""
        });
        get["input_sha256"] = Value::String({
            let mut preimage = get.clone();
            preimage["input_sha256"] = Value::String(String::new());
            canonical_json_hash(&preimage)
        });
        get
    }

    #[test]
    fn low_quad_durable_prepare_get_survives_runtime_restart_exactly_and_is_read_only() {
        let root = std::env::temp_dir().join(format!(
            "forgecad-low-quad-durable-restart-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("restart root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");

        let (request, first, candidate_json, durable_hashes, project_id) = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("initial Runtime");
            let project = runtime
                .create_project("Low quad durable restart", json!({"profile":"test"}))
                .expect("project");
            let prepared = runtime
                .prepare_geometry_candidate(
                    &project.project_id,
                    None,
                    json!({
                        "typed":"geometry",
                        "geometry_program":authoring_program(&project.project_id)
                    }),
                )
                .expect("source GeometryProgram candidate");
            let candidate_id = prepared["candidate"]["candidate_id"]
                .as_str()
                .expect("candidate id")
                .to_owned();
            let candidate = runtime
                .candidate(&candidate_id)
                .expect("candidate query")
                .expect("candidate");
            let candidate_json = serde_json::to_value(&candidate).expect("candidate JSON");
            let evidence = runtime
                .store
                .get_geometry_candidate_evidence(&candidate_id)
                .expect("evidence query")
                .expect("geometry evidence");
            let source_artifact_id = candidate
                .prepared_object_id
                .clone()
                .expect("source artifact id");
            let source_artifact_object_sha256 = candidate
                .prepared_object_sha256
                .clone()
                .expect("source artifact object SHA");
            let readback = runtime
                .artifact_readback(&source_artifact_object_sha256, &candidate_id)
                .expect("source ArtifactReadback");
            let source_artifact_readback_sha256 = readback["canonical_sha256"]
                .as_str()
                .expect("source ArtifactReadback SHA")
                .to_owned();
            let projection_request = json!({
                "schema_version":"AuthoringMeshRequest@1",
                "project_id":project.project_id,
                "candidate_id":candidate_id,
                "artifact_id":source_artifact_object_sha256,
                "artifact_readback_sha256":source_artifact_readback_sha256,
                "program_sha256":evidence.geometry_program_sha256,
                "operator_catalog_sha256":evidence.operator_catalog_sha256,
                "readback_config_sha256":evidence.readback_config_sha256,
                "authoring_node_id":"low-quad-restart-panel",
                "part_id":"low-quad-restart-panel",
                "authoring_mesh_policy_sha256":"aa72cadabba90ddb43dd0014cfa434ab9b13f4e072b09258072f37334c72e709",
                "max_response_bytes":1048576
            });
            let projection = crate::authoring_mesh::get(&runtime, &projection_request)
                .expect("source AuthoringMesh projection");
            let source_lineage_sha256 = projection["lineage"]["lineage_sha256"]
                .as_str()
                .expect("source lineage SHA")
                .to_owned();
            let base_version_id = candidate
                .base_version_id
                .clone()
                .map(Value::String)
                .unwrap_or(Value::Null);
            let expected_canonical = expected_canonical_mesh(
                &projection,
                &project.project_id,
                &candidate_id,
                &candidate.canonical_sha256,
                base_version_id.clone(),
                &evidence.geometry_program_object_sha256,
                &evidence.geometry_program_sha256,
                &source_artifact_object_sha256,
                &source_artifact_object_sha256,
                &evidence.artifact_readback_object_sha256,
                &source_artifact_readback_sha256,
                &source_lineage_sha256,
            );
            let mut source_request = json!({
                "schema_version":"AuthoringMeshPrepareRequest@1",
                "project_id":project.project_id,
                "source_candidate_id":candidate_id,
                "source_candidate_state_sha256":candidate.canonical_sha256,
                "base_version_id":base_version_id,
                "authoring_node_id":"low-quad-restart-panel",
                "part_id":"low-quad-restart-panel",
                "source_program_object_sha256":evidence.geometry_program_object_sha256,
                "source_program_sha256":evidence.geometry_program_sha256,
                "source_artifact_id":source_artifact_id,
                "source_artifact_object_sha256":source_artifact_object_sha256,
                "source_artifact_sha256":source_artifact_object_sha256,
                "source_artifact_readback_object_sha256":evidence.artifact_readback_object_sha256,
                "source_artifact_readback_sha256":source_artifact_readback_sha256,
                "source_lineage_sha256":source_lineage_sha256,
                "expected_canonical_mesh_sha256":expected_canonical["canonical_sha256"],
                "idempotency_key":"low-quad-authoring-mesh-once",
                "max_response_bytes":1048576,
                "runtime_write_performed":false,
                "writer_policy":"forgecad-runtime-only-state-writer@1",
                "canonicalization_policy":LOW_QUAD_DRAFT_DURABLE_CANONICALIZATION_POLICY,
                "input_sha256":""
            });
            source_request["input_sha256"] = Value::String(canonical_json_hash(&source_request));
            let source = runtime
                .authoring_mesh_durable_prepare(&source_request)
                .expect("durable AuthoringMesh source");
            assert_eq!(source["canonical_mesh"], expected_canonical);

            let native_request = native_prepare_request(
                &project.project_id,
                &candidate_id,
                &candidate.canonical_sha256,
                source["base_version_id"].clone(),
                source["canonical_mesh_id"]
                    .as_str()
                    .expect("source mesh id"),
                source["canonical_mesh_object_sha256"]
                    .as_str()
                    .expect("source mesh object SHA"),
                source["canonical_mesh_sha256"]
                    .as_str()
                    .expect("source mesh SHA"),
                high_request(
                    source["canonical_mesh"].clone(),
                    &candidate_id,
                    &candidate.canonical_sha256,
                ),
                "low-quad-native-high-once",
            );
            let high = runtime
                .native_high_durable_prepare(native_request)
                .expect("Native High source");
            assert_eq!(high["replayed"], false);
            assert_eq!(high["restart_hash_verified"], true);
            assert_eq!(high["production_stage_advanced"], false);
            assert_eq!(high["candidate_confirmed"], false);
            assert_eq!(high["version_created"], false);
            assert_eq!(high["export_performed"], false);

            let source_high_artifact_id = high["artifact_id"].as_str().expect("High artifact id");
            let source_high_artifact_object_sha256 = high["glb_object_sha256"]
                .as_str()
                .expect("High GLB object SHA");
            let source_high_artifact_sha256 = high["glb_sha256"].as_str().expect("High GLB SHA");
            let source_high_artifact_readback_object_sha256 = high["glb_readback_object_sha256"]
                .as_str()
                .expect("High GLB readback object SHA");
            let source_high_artifact_readback_sha256 = high["glb_readback_sha256"]
                .as_str()
                .expect("High GLB readback SHA");
            let worker_request = low_quad_worker_request(
                &project.project_id,
                source_high_artifact_sha256,
                source_high_artifact_readback_sha256,
            );
            let request = low_prepare_request(
                &project.project_id,
                &candidate_id,
                &candidate.canonical_sha256,
                candidate
                    .base_version_id
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
                source_high_artifact_id,
                source_high_artifact_object_sha256,
                source_high_artifact_sha256,
                source_high_artifact_readback_object_sha256,
                source_high_artifact_readback_sha256,
                worker_request,
                "low-quad-durable-once",
            );
            let candidates_before = serde_json::to_value(
                runtime
                    .candidates(&project.project_id)
                    .expect("candidates before"),
            )
            .expect("candidates JSON");
            let versions_before = serde_json::to_value(
                runtime
                    .versions(Some(&project.project_id))
                    .expect("versions before"),
            )
            .expect("versions JSON");
            let first = runtime
                .low_quad_draft_durable_prepare(request.clone())
                .expect("Low quad durable prepare");
            assert_eq!(first["replayed"], false);
            assert_eq!(first["restart_hash_verified"], true);
            assert_eq!(first["runtime_write_performed"], true);
            assert_eq!(first["persistent_user_data_touched"], true);
            assert_eq!(first["production_stage_advanced"], false);
            assert_eq!(first["promotion_eligible"], false);
            assert_eq!(first["candidate_confirmed"], false);
            assert_eq!(first["version_created"], false);
            assert_eq!(first["export_performed"], false);
            assert_eq!(first["quality_status"], "structural_only");
            assert_eq!(first["edge_flow_status"], "DRAFT_UNREVIEWED");
            assert_eq!(
                serde_json::to_value(
                    runtime
                        .candidate(&candidate_id)
                        .expect("candidate after")
                        .expect("candidate")
                )
                .expect("candidate JSON after"),
                candidate_json
            );
            assert_eq!(
                serde_json::to_value(
                    runtime
                        .candidates(&project.project_id)
                        .expect("candidates after")
                )
                .expect("candidates JSON after"),
                candidates_before
            );
            assert_eq!(
                serde_json::to_value(
                    runtime
                        .versions(Some(&project.project_id))
                        .expect("versions after")
                )
                .expect("versions JSON after"),
                versions_before
            );
            let objects_after_prepare = runtime
                .store
                .cas()
                .list_objects()
                .expect("CAS after prepare");
            let replay = runtime
                .low_quad_draft_durable_prepare(request.clone())
                .expect("same-key Low quad replay");
            assert_eq!(replay["replayed"], true);
            assert_eq!(replay["restart_hash_verified"], true);
            assert_eq!(replay["durable_link"], first["durable_link"]);
            for field in [
                "link_id",
                "link_object_sha256",
                "worker_result_object_sha256",
                "worker_result_sha256",
                "artifact_object_sha256",
                "artifact_sha256",
                "readback_object_sha256",
                "readback_sha256",
            ] {
                assert_eq!(replay[field], first[field], "replay {field}");
            }
            assert_eq!(
                runtime
                    .store
                    .cas()
                    .list_objects()
                    .expect("CAS after replay"),
                objects_after_prepare
            );
            let record = runtime
                .store
                .get_low_quad_draft_durable(&project.project_id, "low-quad-durable-once")
                .expect("Low durable record query")
                .expect("Low durable record");
            let durable_hashes = vec![
                record.source_high_artifact_object_sha256,
                record.source_high_artifact_readback_object_sha256,
                record.worker_result_object_sha256,
                record.artifact_object_sha256,
                record.readback_object_sha256,
                record.link_object_sha256,
                record.canonical_sha256,
            ];
            drop(runtime);
            (
                request,
                first,
                candidate_json,
                durable_hashes,
                project.project_id,
            )
        };

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopened Runtime");
        let get_request = low_get_request(&first, &request);
        let objects_before_get = reopened.store.cas().list_objects().expect("CAS before get");
        let get = reopened
            .low_quad_draft_durable_get(get_request)
            .expect("Low quad durable get after restart");
        assert_eq!(get["replayed"], false);
        assert_eq!(get["restart_hash_verified"], true);
        assert_eq!(get["runtime_write_performed"], false);
        assert_eq!(get["persistent_user_data_touched"], false);
        assert_eq!(get["durable_link"], first["durable_link"]);
        for field in [
            "project_id",
            "candidate_id",
            "candidate_state_sha256",
            "link_id",
            "link_object_sha256",
            "worker_result_object_sha256",
            "worker_result_sha256",
            "artifact_object_sha256",
            "artifact_sha256",
            "readback_object_sha256",
            "readback_sha256",
            "idempotency_key",
            "quality_status",
            "edge_flow_status",
        ] {
            assert_eq!(get[field], first[field], "restart exact field {field}");
        }
        for field in [
            "production_stage_advanced",
            "promotion_eligible",
            "candidate_confirmed",
            "version_created",
            "export_performed",
        ] {
            assert_eq!(get[field], false, "{field} must remain false");
        }
        assert_eq!(
            reopened.store.cas().list_objects().expect("CAS after get"),
            objects_before_get
        );
        assert_eq!(
            serde_json::to_value(
                reopened
                    .candidate(request["candidate_id"].as_str().expect("candidate id"))
                    .expect("candidate after restart")
                    .expect("candidate"),
            )
            .expect("candidate JSON after restart"),
            candidate_json
        );
        assert!(reopened
            .versions(Some(&project_id))
            .expect("versions after restart")
            .is_empty());
        let reopened_record = reopened
            .store
            .get_low_quad_draft_durable(&project_id, "low-quad-durable-once")
            .expect("Low durable record after restart")
            .expect("Low durable record after restart");
        assert_eq!(
            vec![
                reopened_record.source_high_artifact_object_sha256,
                reopened_record.source_high_artifact_readback_object_sha256,
                reopened_record.worker_result_object_sha256,
                reopened_record.artifact_object_sha256,
                reopened_record.readback_object_sha256,
                reopened_record.link_object_sha256,
                reopened_record.canonical_sha256,
            ],
            durable_hashes
        );
        drop(reopened);
        fs::remove_dir_all(root).expect("restart fixture cleanup");
    }

    #[test]
    fn durable_slice_keeps_quad_policy_and_no_promotion_flags() {
        assert_eq!(
            LOW_QUAD_DRAFT_DURABLE_POLICY,
            "runtime-owned-explicit-quad-draft-source-only@1"
        );
        assert!(LOW_QUAD_DRAFT_DURABLE_LIMITATIONS.contains(&"DRAFT_UNREVIEWED"));
        assert!(LOW_QUAD_DRAFT_DURABLE_LIMITATIONS.contains(&"PROMOTION_INELIGIBLE"));
    }

    #[test]
    fn durable_prepare_fields_are_closed() {
        let fields = PREPARE_FIELDS.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(fields.len(), PREPARE_FIELDS.len());
        assert!(fields.contains("low_quad_draft_worker_request"));
        assert!(fields.contains("source_high_artifact_readback_object_sha256"));
    }
}
