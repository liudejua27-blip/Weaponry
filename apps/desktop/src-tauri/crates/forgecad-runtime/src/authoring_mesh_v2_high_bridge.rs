//! Runtime-owned bridge from an immutable AuthoringMesh@2 revision to the
//! direct, first-party V2 High Worker.
//!
//! This is intentionally a narrow structural bridge.  The caller supplies
//! only immutable identities; Runtime reloads the revision and candidate
//! lineage, derives the fixed execution envelope, validates the Worker
//! result/readback, and only then commits three CAS roots plus one Store row.
//! It never accepts topology or Worker steps, never creates a GLB, and never
//! promotes High, visual, human, engine, or commercial status.

use super::{
    authoring_mesh_v2, authoring_mesh_v2_durable, canonical_json_bytes, canonical_json_hash,
    geometry_worker, is_opaque_id, is_sha256, sha256_hex, Runtime, RuntimeError,
};
use forgecad_contracts::{AuthoringMeshRevision, AuthoringMeshV2SourceBinding};
use forgecad_store::{
    AuthoringMeshV2HighBridgeCasBundle, AuthoringMeshV2HighBridgeCommit,
    AuthoringMeshV2HighBridgeStoreRecord, CasObject, KnifeSourceBindingStoreRecord,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub(crate) const BRIDGE_SCHEMA_VERSION: &str = "AuthoringMeshV2HighBridge@1";
pub(crate) const PREPARE_SCHEMA_VERSION: &str = "AuthoringMeshV2HighBridgePrepareRequest@1";
pub(crate) const GET_SCHEMA_VERSION: &str = "AuthoringMeshV2HighBridgeGetRequest@1";
pub(crate) const RESULT_SCHEMA_VERSION: &str = "AuthoringMeshV2HighBridgeResult@1";
pub(crate) const PREPARE_OPERATION: &str = "authoring_mesh_v2_high_bridge_prepare";
pub(crate) const GET_OPERATION: &str = "authoring_mesh_v2_high_bridge_get";
const REVISION_SCHEMA_VERSION: &str = "AuthoringMeshRevision@2";
const EXECUTION_SCHEMA_VERSION: &str = "AuthoringMeshV2HighExecutionRequest@2";
const EXECUTION_OPERATION: &str = "forgecad.production.authoring-mesh-v2-high-execute@1";
const HIGH_OPERATION: &str = "forgecad.production.authoring-mesh-v2-high-evaluate@1";
const HIGH_RESULT_SCHEMA_VERSION: &str = "AuthoringMeshV2HighResult@2";
const HIGH_READBACK_SCHEMA_VERSION: &str = "AuthoringMeshV2HighReadback@2";
const EVALUATOR_CONTRACT: &str = "forgecad-owned-cpu-catmull-clark-stitched-polygon@2";
const SUBDIVISION_BACKEND: &str = "cpu_regular_quad";
const SOURCE_SCOPE: &str = "materialized-v2-revision-part-set@1";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const MAIN_CANONICALIZATION: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const REQUEST_CANONICALIZATION: &str = "canonical-json-sha256-excluding-input-sha256@1";
const ARTIFACT_HASH_POLICY: &str =
    "artifact-sha256-equals-object-sha256-until-semantic-artifact-contract@1";
const COHORT_POLICY: &str = "same-worker-build-cohort-required-for-durable-link@1";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MAX_JSON_BYTES: u64 = 8 * 1024 * 1024;
const MAX_BRIDGE_BYTES: u64 = 1024 * 1024;
const HIGH_MAX_OUTPUT_VERTICES: u64 = 32_768;
const HIGH_MAX_OUTPUT_TRIANGLES: u64 = 600_000;
const BRIDGE_OBJECT_KIND: &str = "authoring-mesh-v2-high-bridge@1";
const RESULT_OBJECT_KIND: &str = "authoring-mesh-v2-high-result@2";
const READBACK_OBJECT_KIND: &str = "authoring-mesh-v2-high-readback@2";

const SCOPE_LIMITATIONS: [&str; 5] = [
    "RUNTIME_DERIVES_COMPLETE_ORDERED_PART_INPUTS",
    "RUNTIME_CONSTRUCTS_CPU_STITCHED_STEPS",
    "NO_CALLER_SUPPLIED_REVISION_TOPOLOGY",
    "NO_OPEN_SUBDIVISION_BACKEND",
    "VERIFIED_PRESERVED_PARTS_FROM_MATERIALIZED_GLB",
];

/// The public High contracts require RFC3339 UTC. Historical Runtime records
/// use epoch seconds, so keep this formatter local to the new High vertical
/// slice instead of changing unrelated durable families or weakening schema.
pub(crate) fn contract_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096).div_euclid(365);
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_part = (5 * doy + 2).div_euclid(153);
    let day = doy - (153 * month_part + 2).div_euclid(5) + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + i64::from(month <= 2);
    (year, month, day)
}

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "bridge_id",
    "source_scope",
    "source_revision_schema_version",
    "mesh_id",
    "lineage_id",
    "revision_id",
    "revision_index",
    "revision_sha256",
    "revision_object_sha256",
    "source_binding_id",
    "source_binding_sha256",
    "source_binding_object_sha256",
    "materialized_candidate_id",
    "materialized_candidate_state_sha256",
    "materialized_program_sha256",
    "materialized_program_object_sha256",
    "materialized_artifact_id",
    "materialized_artifact_sha256",
    "materialized_artifact_object_sha256",
    "materialized_artifact_readback_sha256",
    "materialized_artifact_readback_object_sha256",
    "representation_plan_sha256",
    "source_node_id",
    "part_id",
    "material_zone_id",
    "solid",
    "source_part_output_sha256",
    "preserved_part_ids",
    "materialized_artifact_hash_policy",
    "high_execution_request_schema_version",
    "high_execution_operation",
    "high_operation",
    "high_result_schema_version",
    "high_readback_schema_version",
    "high_evaluator_contract",
    "high_subdivision_backend",
    "high_subdivision_levels",
    "high_max_triangles_per_face",
    "high_max_output_vertices",
    "high_max_output_triangles",
    "scope_limitations",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "bridge_id",
    "bridge_sha256",
    "bridge_object_sha256",
    "source_scope",
    "source_revision_schema_version",
    "mesh_id",
    "lineage_id",
    "revision_id",
    "revision_index",
    "revision_sha256",
    "revision_object_sha256",
    "source_binding_id",
    "source_binding_sha256",
    "source_binding_object_sha256",
    "materialized_candidate_id",
    "materialized_candidate_state_sha256",
    "materialized_program_sha256",
    "materialized_program_object_sha256",
    "materialized_artifact_id",
    "materialized_artifact_sha256",
    "materialized_artifact_object_sha256",
    "materialized_artifact_readback_sha256",
    "materialized_artifact_readback_object_sha256",
    "representation_plan_sha256",
    "source_node_id",
    "part_id",
    "material_zone_id",
    "solid",
    "source_part_output_sha256",
    "preserved_part_ids",
    "materialized_artifact_hash_policy",
    "high_execution_request_schema_version",
    "high_execution_operation",
    "high_operation",
    "high_result_schema_version",
    "high_readback_schema_version",
    "high_evaluator_contract",
    "high_subdivision_backend",
    "high_subdivision_levels",
    "high_max_triangles_per_face",
    "high_max_output_vertices",
    "high_max_output_triangles",
    "high_execution_request_sha256",
    "high_evaluation_sha256",
    "high_result_sha256",
    "high_result_object_sha256",
    "high_readback_sha256",
    "high_readback_object_sha256",
    "high_worker_algorithm_sha256",
    "high_worker_build_cohort_sha256",
    "high_replay_count",
    "high_replay_byte_exact",
    "high_non_destructive",
    "high_projected_source_mesh_sha256",
    "high_source_vertex_count",
    "high_source_triangle_count",
    "high_evaluated_part_count",
    "high_evaluated_triangle_count",
    "scope_limitations",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

#[derive(Debug, Clone)]
struct Identity {
    values: Map<String, Value>,
}

#[derive(Debug, Clone)]
struct Request {
    identity: Identity,
    input_sha256: String,
    idempotency_key: String,
    max_response_bytes: usize,
}

#[derive(Debug, Clone)]
struct GetRequest {
    identity: Identity,
    bridge_sha256: String,
    bridge_object_sha256: String,
    high: HighMetadata,
    input_sha256: String,
    max_response_bytes: usize,
}

#[derive(Debug, Clone)]
struct HighMetadata {
    high_execution_request_sha256: String,
    high_evaluation_sha256: String,
    high_result_sha256: String,
    high_result_object_sha256: String,
    high_readback_sha256: String,
    high_readback_object_sha256: String,
    high_worker_algorithm_sha256: String,
    high_worker_build_cohort_sha256: String,
    high_replay_count: u64,
    high_replay_byte_exact: bool,
    high_non_destructive: bool,
    high_projected_source_mesh_sha256: String,
    high_source_vertex_count: u64,
    high_source_triangle_count: u64,
    high_evaluated_part_count: u64,
    high_evaluated_triangle_count: u64,
}

/// The materialized candidate is a verified, immutable input to High.  Keep
/// the decoded bytes here rather than re-reading or reconstructing them from
/// caller data: the program establishes semantic part order and the GLB is
/// the only source for preserved geometry.
#[derive(Debug, Clone)]
struct MaterializedCandidateSources {
    program: Value,
    artifact_bytes: Vec<u8>,
    readback: Value,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "AUTHORING_MESH_V2_HIGH_BRIDGE_INVALID: {}",
        message.into()
    ))
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("request must be an object"))?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid("request fields differ from the closed envelope"));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} must be a string")))
}

fn id(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    let value = text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} must be an opaque ID")));
    }
    Ok(value.to_owned())
}

fn sha(object: &Map<String, Value>, field: &str) -> Result<String, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} must be a SHA-256")));
    }
    Ok(value.to_owned())
}

fn bool_value(
    object: &Map<String, Value>,
    field: &str,
    expected: bool,
) -> Result<(), RuntimeError> {
    if object.get(field).and_then(Value::as_bool) != Some(expected) {
        return Err(invalid(format!("{field} policy differs")));
    }
    Ok(())
}

fn u64_value(object: &Map<String, Value>, field: &str, max: u64) -> Result<u64, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_u64)
        .filter(|value| *value <= max)
        .ok_or_else(|| invalid(format!("{field} is outside its bounded range")))
}

fn input_hash(request: &Value, object: &Map<String, Value>) -> Result<String, RuntimeError> {
    let hash = sha(object, "input_sha256")?;
    let mut preimage = request.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != hash {
        return Err(invalid("input_sha256 does not match request"));
    }
    Ok(hash)
}

fn parse_identity(object: &Map<String, Value>) -> Result<Identity, RuntimeError> {
    // Do the common constants explicitly; the shape differs between Main and
    // the two public envelopes, so only fields present in this map are read.
    for (field, expected) in [
        ("source_scope", SOURCE_SCOPE),
        ("source_revision_schema_version", REVISION_SCHEMA_VERSION),
        ("materialized_artifact_hash_policy", ARTIFACT_HASH_POLICY),
        (
            "high_execution_request_schema_version",
            EXECUTION_SCHEMA_VERSION,
        ),
        ("high_execution_operation", EXECUTION_OPERATION),
        ("high_operation", HIGH_OPERATION),
        ("high_result_schema_version", HIGH_RESULT_SCHEMA_VERSION),
        ("high_readback_schema_version", HIGH_READBACK_SCHEMA_VERSION),
        ("high_evaluator_contract", EVALUATOR_CONTRACT),
        ("high_subdivision_backend", SUBDIVISION_BACKEND),
    ] {
        if text(object, field)? != expected {
            return Err(invalid(format!("{field} constant differs")));
        }
    }
    if u64_value(object, "revision_index", 1_000_000)? > 1_000_000
        || u64_value(object, "high_subdivision_levels", 2)? != 1
        || u64_value(object, "high_max_triangles_per_face", 32)? != 32
        || u64_value(object, "high_max_output_vertices", HIGH_MAX_OUTPUT_VERTICES)?
            != HIGH_MAX_OUTPUT_VERTICES
        || u64_value(
            object,
            "high_max_output_triangles",
            HIGH_MAX_OUTPUT_TRIANGLES,
        )? != HIGH_MAX_OUTPUT_TRIANGLES
    {
        return Err(invalid("fixed High policy differs"));
    }
    for field in [
        "project_id",
        "bridge_id",
        "mesh_id",
        "lineage_id",
        "revision_id",
        "source_binding_id",
        "materialized_candidate_id",
        "materialized_artifact_id",
        "source_node_id",
        "part_id",
        "material_zone_id",
    ] {
        let _ = id(object, field)?;
    }
    for field in [
        "revision_sha256",
        "revision_object_sha256",
        "source_binding_sha256",
        "source_binding_object_sha256",
        "materialized_candidate_state_sha256",
        "materialized_program_sha256",
        "materialized_program_object_sha256",
        "materialized_artifact_sha256",
        "materialized_artifact_object_sha256",
        "materialized_artifact_readback_sha256",
        "materialized_artifact_readback_object_sha256",
        "representation_plan_sha256",
        "source_part_output_sha256",
    ] {
        let _ = sha(object, field)?;
    }
    if object.get("solid").and_then(Value::as_bool).is_none() {
        return Err(invalid("solid must be boolean"));
    }
    let preserved = object
        .get("preserved_part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("preserved_part_ids must be an array"))?;
    let mut preserved_ids = HashSet::new();
    for value in preserved {
        let value = value
            .as_str()
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("preserved_part_ids contains invalid ID"))?;
        if !preserved_ids.insert(value) {
            return Err(invalid("preserved_part_ids contains duplicates"));
        }
    }
    if preserved_ids.contains(text(object, "part_id")?) {
        return Err(invalid("target part is preserved"));
    }
    let limitations = object
        .get("scope_limitations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("scope_limitations must be an array"))?;
    let expected = SCOPE_LIMITATIONS
        .iter()
        .map(|value| Value::String((*value).to_owned()))
        .collect::<Vec<_>>();
    if limitations != expected.as_slice() {
        return Err(invalid("scope_limitations differs"));
    }
    Ok(Identity {
        values: object.clone(),
    })
}

fn parse_prepare(value: &Value) -> Result<Request, RuntimeError> {
    let object = exact_object(value, PREPARE_FIELDS)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA_VERSION
        || text(object, "operation")? != PREPARE_OPERATION
        || text(object, "writer_policy")? != WRITER_POLICY
        || text(object, "canonicalization_policy")? != REQUEST_CANONICALIZATION
        || object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES)
    {
        return Err(invalid("prepare policy or response budget differs"));
    }
    bool_value(object, "runtime_write_performed", false)?;
    let max_response_bytes = MAX_RESPONSE_BYTES as usize;
    let input_sha256 = input_hash(value, object)?;
    Ok(Request {
        identity: parse_identity(object)?,
        input_sha256,
        idempotency_key: id(object, "idempotency_key")?,
        max_response_bytes,
    })
}

fn high_metadata(object: &Map<String, Value>) -> Result<HighMetadata, RuntimeError> {
    let high = HighMetadata {
        high_execution_request_sha256: sha(object, "high_execution_request_sha256")?,
        high_evaluation_sha256: sha(object, "high_evaluation_sha256")?,
        high_result_sha256: sha(object, "high_result_sha256")?,
        high_result_object_sha256: sha(object, "high_result_object_sha256")?,
        high_readback_sha256: sha(object, "high_readback_sha256")?,
        high_readback_object_sha256: sha(object, "high_readback_object_sha256")?,
        high_worker_algorithm_sha256: sha(object, "high_worker_algorithm_sha256")?,
        high_worker_build_cohort_sha256: sha(object, "high_worker_build_cohort_sha256")?,
        high_replay_count: object
            .get("high_replay_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("high_replay_count must be integer"))?,
        high_replay_byte_exact: object
            .get("high_replay_byte_exact")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("high_replay_byte_exact must be boolean"))?,
        high_non_destructive: object
            .get("high_non_destructive")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("high_non_destructive must be boolean"))?,
        high_projected_source_mesh_sha256: sha(object, "high_projected_source_mesh_sha256")?,
        high_source_vertex_count: object
            .get("high_source_vertex_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("high_source_vertex_count must be integer"))?,
        high_source_triangle_count: object
            .get("high_source_triangle_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("high_source_triangle_count must be integer"))?,
        high_evaluated_part_count: object
            .get("high_evaluated_part_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("high_evaluated_part_count must be integer"))?,
        high_evaluated_triangle_count: object
            .get("high_evaluated_triangle_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("high_evaluated_triangle_count must be integer"))?,
    };
    if high.high_replay_count != 2
        || !high.high_replay_byte_exact
        || !high.high_non_destructive
        || !is_sha256(&high.high_worker_build_cohort_sha256)
        || high.high_source_vertex_count == 0
        || high.high_source_vertex_count > 32_768
        || high.high_source_triangle_count == 0
        || high.high_source_triangle_count > 65_536
        || high.high_evaluated_part_count == 0
        || high.high_evaluated_part_count > 128
        || high.high_evaluated_triangle_count == 0
        || high.high_evaluated_triangle_count > 600_000
    {
        return Err(invalid(
            "High metadata is outside the closed structural bounds",
        ));
    }
    Ok(high)
}

fn parse_get(value: &Value) -> Result<GetRequest, RuntimeError> {
    let object = exact_object(value, GET_FIELDS)?;
    if text(object, "schema_version")? != GET_SCHEMA_VERSION
        || text(object, "operation")? != GET_OPERATION
        || text(object, "writer_policy")? != WRITER_POLICY
        || text(object, "canonicalization_policy")? != REQUEST_CANONICALIZATION
        || object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES)
    {
        return Err(invalid("get policy or response budget differs"));
    }
    bool_value(object, "runtime_write_performed", false)?;
    bool_value(object, "persistent_user_data_touched", false)?;
    Ok(GetRequest {
        identity: parse_identity(object)?,
        bridge_sha256: sha(object, "bridge_sha256")?,
        bridge_object_sha256: sha(object, "bridge_object_sha256")?,
        high: high_metadata(object)?,
        input_sha256: input_hash(value, object)?,
        max_response_bytes: MAX_RESPONSE_BYTES as usize,
    })
}

fn require_response_size(value: &Value, max: usize) -> Result<Value, RuntimeError> {
    let bytes = canonical_json_bytes(value).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() > max {
        return Err(invalid("response exceeds max_response_bytes"));
    }
    Ok(value.clone())
}

fn identity_text<'a>(identity: &'a Identity, field: &str) -> &'a str {
    identity
        .values
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("")
}
fn identity_u64(identity: &Identity, field: &str) -> u64 {
    identity
        .values
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or(0)
}
fn identity_value(identity: &Identity, field: &str) -> Value {
    identity.values.get(field).cloned().unwrap_or(Value::Null)
}

fn load_lineage(
    runtime: &Runtime,
    identity: &Identity,
) -> Result<(AuthoringMeshRevision, KnifeSourceBindingStoreRecord), RuntimeError> {
    let project_id = identity_text(identity, "project_id");
    let source_binding_id = identity_text(identity, "source_binding_id");
    let source_binding_sha256 = identity_text(identity, "source_binding_sha256");
    let source_binding_object_sha256 = identity_text(identity, "source_binding_object_sha256");
    let binding = runtime
        .store
        .get_knife_source_binding(project_id, source_binding_id, source_binding_sha256)?
        .ok_or_else(|| invalid("exact SourceBinding is not durable"))?;
    if binding.source_binding_object_sha256 != source_binding_object_sha256
        || binding.project_id != project_id
        || binding.authoring_mesh_id != identity_text(identity, "mesh_id")
        || binding.authoring_mesh_lineage_id != identity_text(identity, "lineage_id")
        || binding.authoring_mesh_revision_id != identity_text(identity, "revision_id")
        || binding.authoring_mesh_revision_index != identity_u64(identity, "revision_index")
        || binding.authoring_mesh_revision_sha256 != identity_text(identity, "revision_sha256")
        || binding.authoring_mesh_revision_object_sha256
            != identity_text(identity, "revision_object_sha256")
    {
        return Err(invalid(
            "SourceBinding does not exactly bind the requested revision",
        ));
    }
    let durable = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(
            project_id,
            identity_text(identity, "revision_id"),
        )?
        .ok_or_else(|| invalid("AuthoringMeshRevision@2 is not durable"))?;
    if durable.mesh_id != identity_text(identity, "mesh_id")
        || durable.lineage_id != identity_text(identity, "lineage_id")
        || durable.revision_index != identity_u64(identity, "revision_index")
        || durable.revision_sha256 != identity_text(identity, "revision_sha256")
        || durable.revision_object_sha256 != identity_text(identity, "revision_object_sha256")
    {
        return Err(invalid("durable revision identity differs"));
    }
    let revision = authoring_mesh_v2_durable::revision_from_cas(runtime, &durable)?;
    if revision.schema_version != REVISION_SCHEMA_VERSION
        || revision.mesh_id.0 != identity_text(identity, "mesh_id")
        || revision.lineage_id.0 != identity_text(identity, "lineage_id")
        || revision.revision_id.0 != identity_text(identity, "revision_id")
        || revision.revision_index != identity_u64(identity, "revision_index")
        || revision.canonical_sha256 != identity_text(identity, "revision_sha256")
    {
        return Err(invalid(
            "revision CAS is not the exact requested AuthoringMesh@2",
        ));
    }
    let embedded = revision
        .source_binding
        .as_ref()
        .ok_or_else(|| invalid("revision has no embedded SourceBinding"))?;
    authoring_mesh_v2::validate_source_binding(embedded)
        .map_err(|error| invalid(format!("embedded SourceBinding is invalid: {error}")))?;
    // The SourceBinding is the immutable source-candidate anchor.  The
    // materialized candidate below is a later Runtime-owned projection and
    // is intentionally allowed to have a different candidate identity.
    if embedded.project_id != binding.project_id
        || embedded.candidate_id != binding.source_candidate_id
        || embedded.candidate_state_sha256 != binding.source_candidate_state_sha256
        || embedded.source_node_id != identity_text(identity, "source_node_id")
        || embedded.part_id != identity_text(identity, "part_id")
        || embedded.material_zone_id != identity_text(identity, "material_zone_id")
        || embedded.solid
            != identity
                .values
                .get("solid")
                .and_then(Value::as_bool)
                .ok_or_else(|| invalid("solid binding is missing"))?
        || embedded.part_output_sha256 != identity_text(identity, "source_part_output_sha256")
    {
        return Err(invalid(
            "embedded SourceBinding does not match the exact source and target projection",
        ));
    }
    Ok((revision, binding))
}

fn load_candidate_sources(
    runtime: &Runtime,
    identity: &Identity,
    binding: &KnifeSourceBindingStoreRecord,
    embedded: &AuthoringMeshV2SourceBinding,
) -> Result<MaterializedCandidateSources, RuntimeError> {
    validate_source_candidate(runtime, identity, binding, embedded)?;
    let candidate_id = identity_text(identity, "materialized_candidate_id");
    let candidate = runtime
        .candidate(candidate_id)?
        .ok_or_else(|| invalid("materialized candidate is not durable"))?;
    if candidate.project_id != identity_text(identity, "project_id")
        || candidate.canonical_sha256
            != identity_text(identity, "materialized_candidate_state_sha256")
        || !matches!(candidate.state.as_str(), "prepared" | "reviewable")
        || !candidate.quality_hard_gate_passed
        || candidate.prepared_object_sha256.as_deref()
            != Some(identity_text(
                identity,
                "materialized_artifact_object_sha256",
            ))
    {
        return Err(invalid(
            "materialized candidate state/artifact identity differs",
        ));
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .ok_or_else(|| invalid("materialized GeometryCandidateEvidence is absent"))?;
    if evidence.geometry_program_sha256 != identity_text(identity, "materialized_program_sha256")
        || evidence.geometry_program_object_sha256
            != identity_text(identity, "materialized_program_object_sha256")
        || evidence.artifact_object_sha256
            != identity_text(identity, "materialized_artifact_object_sha256")
        || evidence.artifact_readback_object_sha256
            != identity_text(identity, "materialized_artifact_readback_object_sha256")
        || evidence.artifact_readback_object_sha256.is_empty()
    {
        return Err(invalid(
            "materialized candidate evidence is not exactly bound",
        ));
    }
    let mut program_bytes = None;
    let mut artifact_bytes = None;
    let mut readback_bytes = None;
    for (hash, mime, kind, max) in [
        (
            identity_text(identity, "materialized_program_object_sha256"),
            "application/json",
            "geometry-program-v2",
            MAX_JSON_BYTES,
        ),
        (
            identity_text(identity, "materialized_artifact_object_sha256"),
            "model/gltf-binary",
            "geometry-glb",
            64 * 1024 * 1024,
        ),
        (
            identity_text(identity, "materialized_artifact_readback_object_sha256"),
            "application/json",
            "geometry-artifact-readback-v2",
            MAX_JSON_BYTES,
        ),
    ] {
        let object = runtime
            .store
            .get_object(hash)?
            .ok_or_else(|| invalid("materialized source CAS metadata is absent"))?;
        if object.mime != mime
            || object.kind != kind
            || object.size_bytes == 0
            || object.size_bytes > max
        {
            return Err(invalid("materialized source CAS metadata is invalid"));
        }
        let bytes = runtime.cas_read_bounded(hash, max)?;
        if sha256_hex(&bytes) != hash {
            return Err(invalid("materialized source CAS hash mismatch"));
        }
        match kind {
            "geometry-program-v2" => program_bytes = Some(bytes),
            "geometry-glb" => artifact_bytes = Some(bytes),
            "geometry-artifact-readback-v2" => readback_bytes = Some(bytes),
            _ => unreachable!("closed materialized source CAS kind list"),
        }
    }
    let program_bytes =
        program_bytes.ok_or_else(|| invalid("materialized GeometryProgram is absent"))?;
    let artifact_bytes =
        artifact_bytes.ok_or_else(|| invalid("materialized Geometry GLB is absent"))?;
    let readback_bytes =
        readback_bytes.ok_or_else(|| invalid("materialized ArtifactReadback is absent"))?;
    let program: Value = serde_json::from_slice(&program_bytes)
        .map_err(|error| invalid(format!("materialized GeometryProgram JSON: {error}")))?;
    let readback: Value = serde_json::from_slice(&readback_bytes)
        .map_err(|error| invalid(format!("materialized ArtifactReadback JSON: {error}")))?;
    validate_materialized_sources(
        identity,
        &program,
        &program_bytes,
        &artifact_bytes,
        &readback,
        &readback_bytes,
    )?;
    Ok(MaterializedCandidateSources {
        program,
        artifact_bytes,
        readback,
    })
}

fn validate_materialized_sources(
    identity: &Identity,
    program: &Value,
    program_bytes: &[u8],
    artifact_bytes: &[u8],
    readback: &Value,
    readback_bytes: &[u8],
) -> Result<(), RuntimeError> {
    if !program.is_object()
        || program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || program.get("canonical_sha256").is_some()
        || canonical_json_bytes(program).map_err(|error| invalid(error.to_string()))?
            != program_bytes
        || canonical_json_hash(program) != identity_text(identity, "materialized_program_sha256")
    {
        return Err(invalid(
            "materialized GeometryProgram semantic/object hash drifted",
        ));
    }
    let readback_object = readback
        .as_object()
        .ok_or_else(|| invalid("materialized ArtifactReadback is not an object"))?;
    let readback_hash = readback_object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("materialized ArtifactReadback canonical hash is absent"))?;
    let mut readback_preimage = readback.clone();
    readback_preimage["canonical_sha256"] = Value::String(String::new());
    if readback_object
        .get("schema_version")
        .and_then(Value::as_str)
        != Some("ArtifactReadback@2")
        || readback_object.get("candidate_id").and_then(Value::as_str)
            != Some(identity_text(identity, "materialized_candidate_id"))
        || readback_object.get("artifact_id").and_then(Value::as_str)
            != Some(identity_text(identity, "materialized_artifact_id"))
        || readback_object.get("object_sha256").and_then(Value::as_str)
            != Some(identity_text(
                identity,
                "materialized_artifact_object_sha256",
            ))
        || readback_object
            .get("program_sha256")
            .and_then(Value::as_str)
            != Some(identity_text(identity, "materialized_program_sha256"))
        || canonical_json_bytes(readback).map_err(|error| invalid(error.to_string()))?
            != readback_bytes
        || canonical_json_hash(&readback_preimage) != readback_hash
        || readback_hash != identity_text(identity, "materialized_artifact_readback_sha256")
    {
        return Err(invalid(
            "materialized ArtifactReadback semantic/object hash drifted",
        ));
    }
    if artifact_bytes.is_empty() {
        return Err(invalid("materialized Geometry GLB is empty"));
    }
    let part_outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 128)
        .ok_or_else(|| invalid("materialized GeometryProgram part_outputs are invalid"))?;
    let part_bindings = readback_object
        .get("part_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("materialized ArtifactReadback part_bindings are missing"))?;
    let expected_binding_count = part_outputs
        .iter()
        .map(|output| output_node_ids(output).map(|nodes| nodes.len()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .sum::<usize>();
    if part_bindings.len() != expected_binding_count {
        return Err(invalid("materialized part binding count differs"));
    }
    let expected_part_ids = part_outputs
        .iter()
        .map(|output| output.get("part_id").and_then(Value::as_str).unwrap_or(""))
        .collect::<Vec<_>>();
    let mut expected_binding_keys = BTreeSet::new();
    let mut expected_node_ids = BTreeSet::new();
    for output in part_outputs {
        let output_part_id = output.get("part_id").and_then(Value::as_str).unwrap_or("");
        for source_node_id in output_node_ids(output)? {
            if !expected_binding_keys.insert((output_part_id.to_owned(), source_node_id.clone()))
                || !expected_node_ids.insert(source_node_id.clone())
            {
                return Err(invalid("materialized Part/node binding is duplicated"));
            }
        }
    }
    if expected_part_ids.iter().any(|value| value.is_empty())
        || expected_node_ids.iter().any(|value| value.is_empty())
        || readback_object
            .get("part_ids")
            .and_then(Value::as_array)
            .map(|values| values.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            != Some(expected_part_ids.clone())
        || readback_object
            .get("source_node_ids")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<BTreeSet<_>>()
            })
            != Some(expected_node_ids)
    {
        return Err(invalid("materialized part order or node binding differs"));
    }
    let mut actual_binding_keys = BTreeSet::new();
    for binding in part_bindings {
        let output_part_id = binding
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("materialized part binding part_id is missing"))?;
        let output_node = binding
            .get("source_node_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("materialized part binding source_node_id is invalid"))?;
        if !actual_binding_keys.insert((output_part_id.to_owned(), output_node.to_owned()))
            || !expected_binding_keys.contains(&(output_part_id.to_owned(), output_node.to_owned()))
        {
            return Err(invalid("materialized part binding set is not exact"));
        }
        let output = part_outputs
            .iter()
            .find(|output| output.get("part_id").and_then(Value::as_str) == Some(output_part_id))
            .ok_or_else(|| invalid("materialized part binding part is not declared"))?;
        if binding.get("material_zone_id").and_then(Value::as_str)
            != output.get("material_zone_id").and_then(Value::as_str)
            || binding.get("solid").and_then(Value::as_bool)
                != output.get("solid").and_then(Value::as_bool)
        {
            return Err(invalid("materialized part binding differs from program"));
        }
    }
    if actual_binding_keys != expected_binding_keys {
        return Err(invalid("materialized part binding set is not exact"));
    }
    let _ = identity_text(identity, "materialized_artifact_readback_object_sha256");
    Ok(())
}

fn json_string(value: &Value, field: &str) -> Result<String, RuntimeError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} must be an opaque ID")))
}

/// Return the complete ordered source-node set for one semantic Part.  The
/// scalar `source_node_id` remains the compatibility owner; new materialized
/// programs may carry `input_node_ids` with multiple independent source
/// nodes, which must stay one typed Part rather than being split or dropped.
fn output_node_ids(value: &Value) -> Result<Vec<String>, RuntimeError> {
    let values = value
        .get("input_node_ids")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty() && values.len() <= 16)
        .ok_or_else(|| invalid("materialized part input_node_ids are missing"))?;
    let mut result = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let value = value
            .as_str()
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("materialized part input node is invalid"))?;
        if !seen.insert(value.to_owned()) {
            return Err(invalid("materialized part input nodes are duplicated"));
        }
        result.push(value.to_owned());
    }
    Ok(result)
}

fn json_id_value(value: &Value, field: &str) -> Result<String, RuntimeError> {
    value
        .as_str()
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid(format!("{field} must be an opaque ID")))
}

fn f32_position(value: &Value, field: &str) -> Result<[f32; 3], RuntimeError> {
    let values = value
        .as_array()
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid(format!("{field} must be a three-vector")))?;
    let mut result = [0.0; 3];
    for (index, value) in values.iter().enumerate() {
        let value = value
            .as_f64()
            .filter(|value| value.is_finite() && value.abs() <= 10.0)
            .ok_or_else(|| invalid(format!("{field} contains an invalid coordinate")))?;
        result[index] = value as f32;
        if !result[index].is_finite() {
            return Err(invalid(format!("{field} contains an invalid coordinate")));
        }
    }
    Ok(result)
}

fn revision_part_input(
    revision: &AuthoringMeshRevision,
    identity: &Identity,
    source_node_ids: &[String],
    source_part_output_sha256: &str,
) -> Result<Value, RuntimeError> {
    if source_node_ids.is_empty()
        || source_node_ids.len() > 16
        || source_node_ids.iter().any(|value| !is_opaque_id(value))
    {
        return Err(invalid("materialized target source node set is invalid"));
    }
    let revision_value =
        serde_json::to_value(revision).map_err(|error| invalid(error.to_string()))?;
    let original = revision_value
        .get("original")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("AuthoringMeshRevision original topology is missing"))?;
    let vertices = original
        .get("vertices")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("AuthoringMeshRevision vertices are missing"))?;
    let mut vertex_positions = BTreeMap::<String, [f32; 3]>::new();
    for vertex in vertices {
        let vertex_id = json_string(vertex, "vertex_id")?;
        if vertex_positions
            .insert(
                vertex_id,
                f32_position(
                    vertex
                        .get("position_m")
                        .ok_or_else(|| invalid("revision vertex position is missing"))?,
                    "revision vertex position",
                )?,
            )
            .is_some()
        {
            return Err(invalid("revision has duplicate vertex IDs"));
        }
    }
    if vertex_positions.len() < 3 {
        return Err(invalid(
            "revision source topology has fewer than three vertices",
        ));
    }
    let vertex_indices = vertex_positions
        .keys()
        .enumerate()
        .map(|(index, id)| (id.clone(), index as u32))
        .collect::<BTreeMap<_, _>>();
    let half_edges = original
        .get("half_edges")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("revision half_edges are missing"))?
        .iter()
        .map(|half_edge| {
            Ok::<_, RuntimeError>((
                json_string(half_edge, "half_edge_id")?,
                json_string(half_edge, "origin_vertex_id")?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let edge_records = original
        .get("edges")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("revision edges are missing"))?;
    let mut edge_by_vertices = BTreeMap::<(String, String), String>::new();
    for edge in edge_records {
        let edge_id = json_string(edge, "edge_id")?;
        let vertex_ids = edge
            .get("vertex_ids")
            .and_then(Value::as_array)
            .filter(|values| values.len() == 2)
            .ok_or_else(|| invalid("revision edge endpoints are invalid"))?;
        let a = json_id_value(&vertex_ids[0], "edge vertex ID")?;
        let b = json_id_value(&vertex_ids[1], "edge vertex ID")?;
        let key = if a <= b { (a, b) } else { (b, a) };
        if edge_by_vertices.insert(key, edge_id).is_some() {
            return Err(invalid("revision has duplicate edge endpoints"));
        }
    }
    let faces = original
        .get("faces")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("revision faces are missing"))?;
    let mut ordered_faces = BTreeMap::<String, Vec<String>>::new();
    for face in faces {
        let face_id = json_string(face, "face_id")?;
        let half_edge_ids = face
            .get("half_edge_ids")
            .and_then(Value::as_array)
            .filter(|values| values.len() >= 3)
            .ok_or_else(|| invalid("revision face boundary is invalid"))?;
        let ids = half_edge_ids
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| is_opaque_id(value))
                    .map(str::to_owned)
                    .ok_or_else(|| invalid("revision face half-edge ID is invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if ordered_faces.insert(face_id, ids).is_some() {
            return Err(invalid("revision has duplicate face IDs"));
        }
    }
    let mut output_faces = Vec::<Vec<u32>>::new();
    let mut source_face_ids = Vec::<String>::new();
    let mut derived_edges = BTreeMap::<(u32, u32), String>::new();
    for (face_id, half_edge_ids) in &ordered_faces {
        let mut face_vertices = Vec::with_capacity(half_edge_ids.len());
        for half_edge_id in half_edge_ids {
            let vertex_id = half_edges
                .get(half_edge_id)
                .ok_or_else(|| invalid("revision face references a missing half-edge"))?;
            let index = *vertex_indices
                .get(vertex_id)
                .ok_or_else(|| invalid("revision half-edge references a missing vertex"))?;
            face_vertices.push(index);
        }
        if face_vertices.len() < 3 {
            return Err(invalid("revision face has fewer than three corners"));
        }
        for corner in 0..face_vertices.len() {
            let a = face_vertices[corner];
            let b = face_vertices[(corner + 1) % face_vertices.len()];
            if a == b {
                return Err(invalid("revision face has a repeated vertex"));
            }
            let a_id = vertex_positions
                .keys()
                .nth(a as usize)
                .ok_or_else(|| invalid("revision vertex index is invalid"))?;
            let b_id = vertex_positions
                .keys()
                .nth(b as usize)
                .ok_or_else(|| invalid("revision vertex index is invalid"))?;
            let key_ids = if a_id <= b_id {
                (a_id.clone(), b_id.clone())
            } else {
                (b_id.clone(), a_id.clone())
            };
            let edge_id = edge_by_vertices
                .get(&key_ids)
                .ok_or_else(|| invalid("revision face edge is not in the edge table"))?;
            derived_edges.insert((a.min(b), a.max(b)), edge_id.clone());
        }
        output_faces.push(face_vertices);
        source_face_ids.push(face_id.clone());
    }
    if output_faces.is_empty() {
        return Err(invalid("revision source topology has no faces"));
    }
    let source_edges = derived_edges
        .into_iter()
        .map(|((a, b), edge_id)| json!({"edge_id": edge_id, "vertex_indices": [a, b]}))
        .collect::<Vec<_>>();
    let mut lineage = BTreeSet::new();
    for field in ["mesh_id", "lineage_id", "revision_id"] {
        lineage.insert(json_string(&revision_value, field)?);
    }
    lineage.extend(source_face_ids.iter().cloned());
    lineage.extend(edge_by_vertices.values().cloned());
    lineage.extend(vertex_positions.keys().cloned());
    lineage.extend(half_edges.keys().cloned());
    let required_source_binding = revision
        .source_binding
        .as_ref()
        .map(|binding| format!("source-binding-{}", binding.canonical_sha256));
    if let Some(binding) = required_source_binding.as_ref() {
        lineage.insert(binding.clone());
    }
    let mut bounded_lineage = Vec::with_capacity(128);
    if let Some(binding) = required_source_binding.as_ref() {
        bounded_lineage.push(binding.clone());
    }
    bounded_lineage.extend(
        lineage
            .into_iter()
            .filter(|value| Some(value) != required_source_binding.as_ref())
            .take(128usize.saturating_sub(bounded_lineage.len())),
    );
    Ok(json!({
        "part_index": 0,
        "operand_id": identity_text(identity, "mesh_id"),
        "part_id": identity_text(identity, "part_id"),
        "source_node_id": source_node_ids[0],
        "source_node_ids": source_node_ids,
        "material_zone_id": identity_text(identity, "material_zone_id"),
        // The Worker bounds lineage entries independently from topology. Keep
        // the canonical lexical prefix when a dense authored mesh has more
        // than the transport's 128-entry lineage budget; topology itself is
        // still carried losslessly in the typed arrays below.
        "source_element_lineage": bounded_lineage,
        "source_part_output_sha256": source_part_output_sha256,
        "source_vertex_ids": vertex_positions.keys().cloned().collect::<Vec<_>>(),
        "source_edges": source_edges,
        "source_face_ids": source_face_ids,
        "control_points": vertex_positions.values().copied().collect::<Vec<_>>(),
        "faces": output_faces,
    }))
}

fn materialized_glb_part_input(
    triangles: &[crate::integrity::TopologyTriangleSource],
    part_id: &str,
    source_node_ids: &[String],
    material_zone_id: &str,
    _solid: bool,
    source_part_output_sha256: &str,
    artifact_sha256: &str,
) -> Result<Value, RuntimeError> {
    if source_node_ids.is_empty()
        || source_node_ids.iter().any(|value| !is_opaque_id(value))
        || source_node_ids.iter().collect::<BTreeSet<_>>().len() != source_node_ids.len()
    {
        return Err(invalid("materialized part source node set is invalid"));
    }
    if triangles.is_empty() {
        return Err(invalid("materialized preserved part has no triangles"));
    }
    let mut positions = Vec::<[f32; 3]>::new();
    let mut indices = HashMap::<[u32; 3], u32>::new();
    let mut faces = Vec::<Vec<u32>>::new();
    for triangle in triangles {
        let mut face = Vec::with_capacity(3);
        for corner in &triangle.corners {
            let key = corner.position.map(f32::to_bits);
            let index = if let Some(index) = indices.get(&key) {
                *index
            } else {
                let index = positions.len() as u32;
                positions.push(corner.position);
                indices.insert(key, index);
                index
            };
            face.push(index);
        }
        if face[0] == face[1] || face[1] == face[2] || face[0] == face[2] {
            return Err(invalid(
                "materialized preserved part has a degenerate triangle",
            ));
        }
        faces.push(face);
    }
    let source_edges = {
        let mut edges = BTreeMap::<(u32, u32), String>::new();
        for face in &faces {
            for index in 0..3 {
                let a = face[index];
                let b = face[(index + 1) % 3];
                let key = (a.min(b), a.max(b));
                let next = edges.len();
                edges
                    .entry(key)
                    .or_insert_with(|| format!("glb-edge-{next}"));
            }
        }
        edges
            .into_iter()
            .map(|((a, b), edge_id)| json!({"edge_id": edge_id, "vertex_indices": [a, b]}))
            .collect::<Vec<_>>()
    };
    let source_face_ids = (0..faces.len())
        .map(|index| format!("glb-face-{index}"))
        .collect::<Vec<_>>();
    let source_vertex_ids = (0..positions.len())
        .map(|index| format!("glb-vertex-{index}"))
        .collect::<Vec<_>>();
    Ok(json!({
        "part_index": 0,
        "operand_id": format!("preserved-{part_id}"),
        "part_id": part_id,
        // Keep the first node as the compatibility owner while retaining all
        // node identities for composite semantic Parts.
        "source_node_id": source_node_ids[0],
        "source_node_ids": source_node_ids,
        "material_zone_id": material_zone_id,
        "source_element_lineage": [
            format!("glb-source-{}", &artifact_sha256[..24]),
            format!("glb-node-{}", source_node_ids[0]),
            format!("glb-output-{}", &source_part_output_sha256[..24]),
        ],
        "source_part_output_sha256": source_part_output_sha256,
        "source_vertex_ids": source_vertex_ids,
        "source_edges": source_edges,
        "source_face_ids": source_face_ids,
        "control_points": positions,
        "faces": faces,
    }))
}

fn build_part_inputs(
    revision: &AuthoringMeshRevision,
    identity: &Identity,
    sources: &MaterializedCandidateSources,
) -> Result<Vec<Value>, RuntimeError> {
    let outputs = sources
        .program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("materialized GeometryProgram part_outputs are missing"))?;
    let topology = crate::integrity::extract_topology_mesh(&sources.artifact_bytes, 250_000)
        .map_err(|error| invalid(format!("materialized Geometry GLB topology: {error}")))?;
    let readback_bindings = sources
        .readback
        .get("part_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("materialized ArtifactReadback part_bindings are missing"))?;
    let target_part_id = identity_text(identity, "part_id");
    let preserved = identity
        .values
        .get("preserved_part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("preserved_part_ids are missing"))?
        .iter()
        .map(|value| value.as_str().unwrap_or("").to_owned())
        .collect::<BTreeSet<_>>();
    let mut output_parts = BTreeSet::new();
    let mut result = Vec::with_capacity(outputs.len());
    let mut used_triangle_count = 0usize;
    for output in outputs {
        let part_id = json_string(output, "part_id")?;
        if !output_parts.insert(part_id.clone()) {
            return Err(invalid(
                "materialized GeometryProgram has duplicate part IDs",
            ));
        }
        let input_nodes = output_node_ids(output)?;
        let material_zone_id = json_string(output, "material_zone_id")?;
        let solid = output
            .get("solid")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("materialized part solid flag is missing"))?;
        let output_sha256 = canonical_json_hash(output);
        let triangles = topology
            .triangles
            .iter()
            .filter(|triangle| {
                triangle.part_id == part_id
                    && input_nodes
                        .iter()
                        .any(|node| node.as_str() == triangle.source_node_id.as_str())
                    && triangle.material_zone_id == material_zone_id
                    && triangle.solid == solid
            })
            .cloned()
            .collect::<Vec<_>>();
        if triangles.is_empty() {
            return Err(invalid(format!(
                "materialized part {part_id} has no verified GLB geometry"
            )));
        }
        for node in &input_nodes {
            let node = node.as_str();
            let node_triangles = topology
                .triangles
                .iter()
                .filter(|triangle| {
                    triangle.part_id == part_id
                        && triangle.source_node_id == node
                        && triangle.material_zone_id == material_zone_id
                        && triangle.solid == solid
                })
                .count();
            let readback_binding = readback_bindings
                .iter()
                .find(|binding| {
                    binding.get("part_id").and_then(Value::as_str) == Some(part_id.as_str())
                        && binding.get("source_node_id").and_then(Value::as_str) == Some(node)
                })
                .ok_or_else(|| invalid("materialized ArtifactReadback part binding is missing"))?;
            if readback_binding
                .get("triangle_count")
                .and_then(Value::as_u64)
                != Some(node_triangles as u64)
            {
                return Err(invalid("materialized GLB/readback triangle count differs"));
            }
        }
        used_triangle_count = used_triangle_count
            .checked_add(triangles.len())
            .ok_or_else(|| invalid("materialized triangle count overflowed"))?;
        let input = if part_id == target_part_id {
            if output.get("material_zone_id").and_then(Value::as_str)
                != Some(identity_text(identity, "material_zone_id"))
                || solid
                    != identity
                        .values
                        .get("solid")
                        .and_then(Value::as_bool)
                        .ok_or_else(|| invalid("target solid binding is missing"))?
            {
                return Err(invalid("materialized target part binding differs"));
            }
            revision_part_input(revision, identity, &input_nodes, &output_sha256)?
        } else {
            if !preserved.contains(&part_id) {
                return Err(invalid(
                    "materialized program contains an undeclared preserved part",
                ));
            }
            materialized_glb_part_input(
                &triangles,
                &part_id,
                &input_nodes,
                &material_zone_id,
                solid,
                &output_sha256,
                identity_text(identity, "materialized_artifact_object_sha256"),
            )?
        };
        result.push(input);
    }
    if !output_parts.contains(target_part_id)
        || output_parts.len() != preserved.len() + 1
        || preserved
            .iter()
            .any(|part_id| !output_parts.contains(part_id))
    {
        return Err(invalid(
            "materialized part set does not match SourceBinding projection",
        ));
    }
    if used_triangle_count != topology.triangles.len()
        || sources
            .readback
            .get("triangle_count")
            .and_then(Value::as_u64)
            != Some(used_triangle_count as u64)
    {
        return Err(invalid(
            "materialized GLB contains an undeclared part binding",
        ));
    }
    for (index, input) in result.iter_mut().enumerate() {
        input["part_index"] = Value::from(index as u64);
    }
    Ok(result)
}

/// Re-check the immutable candidate that the SourceBinding names.  It is
/// deliberately separate from the later materialized candidate: a
/// source-bound materializer creates a new candidate while preserving the
/// original candidate/program/part lineage.  Conflating the two IDs would
/// reject the real Dragonfang chain and would erase the most important
/// provenance edge in the High bridge.
fn validate_source_candidate(
    runtime: &Runtime,
    identity: &Identity,
    binding: &KnifeSourceBindingStoreRecord,
    embedded: &AuthoringMeshV2SourceBinding,
) -> Result<(), RuntimeError> {
    let source = runtime
        .candidate(&binding.source_candidate_id)?
        .ok_or_else(|| invalid("SourceBinding source candidate is not durable"))?;
    if source.project_id != identity_text(identity, "project_id")
        || source.canonical_sha256 != binding.source_candidate_state_sha256
        || !matches!(source.state.as_str(), "prepared" | "reviewable")
        || !source.quality_hard_gate_passed
        || source.prepared_object_sha256.as_deref() != Some(embedded.artifact_sha256.as_str())
        || source.prepared_object_id.as_deref() != Some(embedded.artifact_id.as_str())
    {
        return Err(invalid(
            "SourceBinding source candidate state/artifact identity differs",
        ));
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&binding.source_candidate_id)?
        .ok_or_else(|| invalid("SourceBinding source candidate evidence is not durable"))?;
    if evidence.project_id != identity_text(identity, "project_id")
        || evidence.geometry_program_sha256 != embedded.geometry_program_sha256
        || evidence.artifact_object_sha256 != embedded.artifact_sha256
    {
        return Err(invalid(
            "SourceBinding source candidate evidence identity differs",
        ));
    }

    let program_object = runtime
        .store
        .get_object(&evidence.geometry_program_object_sha256)?
        .ok_or_else(|| invalid("SourceBinding source GeometryProgram CAS metadata is absent"))?;
    if program_object.mime != "application/json"
        || program_object.kind != "geometry-program-v2"
        || program_object.size_bytes == 0
        || program_object.size_bytes > MAX_JSON_BYTES
    {
        return Err(invalid(
            "SourceBinding source GeometryProgram CAS metadata is invalid",
        ));
    }
    let program_bytes =
        runtime.cas_read_bounded(&evidence.geometry_program_object_sha256, MAX_JSON_BYTES)?;
    if sha256_hex(&program_bytes) != evidence.geometry_program_object_sha256 {
        return Err(invalid(
            "SourceBinding source GeometryProgram CAS hash mismatch",
        ));
    }
    let program: Value = serde_json::from_slice(&program_bytes).map_err(|error| {
        invalid(format!(
            "SourceBinding source GeometryProgram JSON: {error}"
        ))
    })?;
    if !program.is_object()
        || program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || program.get("canonical_sha256").is_some()
        || canonical_json_bytes(&program).map_err(|error| invalid(error.to_string()))?
            != program_bytes
        || canonical_json_hash(&program) != embedded.geometry_program_sha256
    {
        return Err(invalid(
            "SourceBinding source GeometryProgram semantic/object hash drifted",
        ));
    }

    let artifact_object = runtime
        .store
        .get_object(&evidence.artifact_object_sha256)?
        .ok_or_else(|| invalid("SourceBinding source artifact CAS metadata is absent"))?;
    if artifact_object.mime != "model/gltf-binary"
        || artifact_object.kind != "geometry-glb"
        || artifact_object.size_bytes == 0
        || artifact_object.size_bytes > 64 * 1024 * 1024
    {
        return Err(invalid(
            "SourceBinding source artifact CAS metadata is invalid",
        ));
    }
    let artifact_bytes =
        runtime.cas_read_bounded(&evidence.artifact_object_sha256, 64 * 1024 * 1024)?;
    if sha256_hex(&artifact_bytes) != embedded.artifact_sha256 {
        return Err(invalid("SourceBinding source artifact CAS hash mismatch"));
    }

    let readback_object = runtime
        .store
        .get_object(&evidence.artifact_readback_object_sha256)?
        .ok_or_else(|| invalid("SourceBinding source ArtifactReadback CAS metadata is absent"))?;
    if readback_object.mime != "application/json"
        || readback_object.kind != "geometry-artifact-readback-v2"
        || readback_object.size_bytes == 0
        || readback_object.size_bytes > MAX_JSON_BYTES
    {
        return Err(invalid(
            "SourceBinding source ArtifactReadback CAS metadata is invalid",
        ));
    }
    let readback_bytes =
        runtime.cas_read_bounded(&evidence.artifact_readback_object_sha256, MAX_JSON_BYTES)?;
    if sha256_hex(&readback_bytes) != evidence.artifact_readback_object_sha256 {
        return Err(invalid(
            "SourceBinding source ArtifactReadback CAS hash mismatch",
        ));
    }
    let readback: Value = serde_json::from_slice(&readback_bytes).map_err(|error| {
        invalid(format!(
            "SourceBinding source ArtifactReadback JSON: {error}"
        ))
    })?;
    let readback_hash = readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("SourceBinding source ArtifactReadback canonical hash is absent"))?;
    let mut readback_preimage = readback.clone();
    readback_preimage["canonical_sha256"] = Value::String(String::new());
    if readback.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
        || readback.get("candidate_id").and_then(Value::as_str)
            != Some(binding.source_candidate_id.as_str())
        || readback.get("object_sha256").and_then(Value::as_str)
            != Some(embedded.artifact_sha256.as_str())
        || readback.get("program_sha256").and_then(Value::as_str)
            != Some(embedded.geometry_program_sha256.as_str())
        || canonical_json_bytes(&readback).map_err(|error| invalid(error.to_string()))?
            != readback_bytes
        || canonical_json_hash(&readback_preimage) != readback_hash
        || readback_hash != embedded.artifact_readback_sha256
    {
        return Err(invalid(
            "SourceBinding source ArtifactReadback semantic/object hash drifted",
        ));
    }
    Ok(())
}

fn build_execution_request(
    revision: &AuthoringMeshRevision,
    part_inputs: Vec<Value>,
) -> Result<(Value, String), RuntimeError> {
    let revision_value =
        serde_json::to_value(revision).map_err(|error| invalid(error.to_string()))?;
    let max_steps = part_inputs.len();
    let max_output_vertices = part_inputs.iter().try_fold(0usize, |total, input| {
        let source_vertices = input
            .get("control_points")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let source_edges = input
            .get("source_edges")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let source_faces = input
            .get("faces")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        // One Catmull-Clark level emits the original vertices plus one
        // edge point per source edge and one face point per source face.
        // The Worker enforces its budget against that evaluated mesh,
        // not against the smaller source control cage.
        let evaluated_vertices = source_vertices
            .checked_add(source_edges)
            .and_then(|value| value.checked_add(source_faces))
            .ok_or_else(|| invalid("derived High vertex budget overflowed"))?;
        total
            .checked_add(evaluated_vertices)
            .ok_or_else(|| invalid("derived High vertex budget overflowed"))
    })?;
    let max_output_triangles = part_inputs
        .iter()
        .try_fold(0usize, |total, input| {
            let faces = input.get("faces").and_then(Value::as_array);
            let source_triangles = faces
                .map(|faces| {
                    faces
                        .iter()
                        .map(|face| {
                            face.as_array()
                                .map_or(0, |face| face.len().saturating_sub(2))
                        })
                        .sum::<usize>()
                })
                .unwrap_or(0);
            // One Catmull–Clark level emits one quad per source-face corner;
            // the evaluator triangulates each emitted quad into two triangles.
            // Derive the exact same upper bound as the Worker so a valid
            // multi-Part request cannot be rejected solely because Runtime
            // under-budgeted triangle output.
            let evaluated_triangles = faces
                .map(|faces| {
                    faces
                        .iter()
                        .map(|face| {
                            face.as_array()
                                .map_or(0, |face| face.len().saturating_mul(2))
                        })
                        .sum::<usize>()
                })
                .unwrap_or(0);
            total
                .checked_add(source_triangles)
                .and_then(|value| value.checked_add(evaluated_triangles))
        })
        .ok_or_else(|| invalid("derived High triangle budget overflowed"))?;
    if !(1..=128).contains(&max_steps)
        || max_output_vertices == 0
        || max_output_vertices > 32_768
        || max_output_triangles == 0
        || max_output_triangles > 600_000
    {
        return Err(invalid(
            "derived High part budget is outside the closed bounds",
        ));
    }
    let mut request = json!({
        "schema_version": EXECUTION_SCHEMA_VERSION,
        "operation": EXECUTION_OPERATION,
        "revision": revision_value,
        "revision_sha256": revision.canonical_sha256,
        "part_inputs": part_inputs,
        "subdivision_levels": 1,
        "max_triangles_per_face": 32,
        "budgets": {
            "max_steps": max_steps,
            "max_output_vertices": max_output_vertices,
            "max_output_triangles": max_output_triangles
        },
        "canonical_sha256": ""
    });
    let semantic = canonical_json_hash(&request);
    request["canonical_sha256"] = Value::String(semantic.clone());
    Ok((request, semantic))
}

fn validate_worker_result(
    result: &Value,
    expected: &Identity,
    expected_part_inputs: &[Value],
    execution_sha256: &str,
    cohort: &str,
) -> Result<(Value, HighMetadata), RuntimeError> {
    let object = result
        .as_object()
        .ok_or_else(|| invalid("Worker result is not an object"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some(HIGH_RESULT_SCHEMA_VERSION)
        || object.get("operation").and_then(Value::as_str) != Some(HIGH_OPERATION)
        || object.get("mesh_id").and_then(Value::as_str) != Some(identity_text(expected, "mesh_id"))
        || object.get("lineage_id").and_then(Value::as_str)
            != Some(identity_text(expected, "lineage_id"))
        || object.get("revision_id").and_then(Value::as_str)
            != Some(identity_text(expected, "revision_id"))
        || object.get("revision_index").and_then(Value::as_u64)
            != Some(identity_u64(expected, "revision_index"))
        || object.get("revision_sha256").and_then(Value::as_str)
            != Some(identity_text(expected, "revision_sha256"))
    {
        return Err(invalid(
            "Worker result revision or operation binding differs",
        ));
    }
    let result_canonical = sha(object, "canonical_sha256")?;
    let mut result_preimage = result.clone();
    result_preimage["canonical_sha256"] = Value::String(String::new());
    let recomputed_result_canonical = canonical_json_hash(&result_preimage);
    if recomputed_result_canonical != result_canonical {
        return Err(invalid(format!(
            "Worker result canonical hash mismatch: declared={result_canonical} recomputed={recomputed_result_canonical}"
        )));
    }
    let readback = object
        .get("readback")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("Worker readback is missing"))?;
    if readback.get("schema_version").and_then(Value::as_str) != Some(HIGH_READBACK_SCHEMA_VERSION)
        || readback.get("mesh_id").and_then(Value::as_str)
            != Some(identity_text(expected, "mesh_id"))
        || readback.get("lineage_id").and_then(Value::as_str)
            != Some(identity_text(expected, "lineage_id"))
        || readback.get("revision_id").and_then(Value::as_str)
            != Some(identity_text(expected, "revision_id"))
        || readback.get("revision_sha256").and_then(Value::as_str)
            != Some(identity_text(expected, "revision_sha256"))
    {
        return Err(invalid("Worker readback revision binding differs"));
    }
    let readback_canonical = sha(readback, "canonical_sha256")?;
    let mut readback_preimage = Value::Object(readback.clone());
    readback_preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&readback_preimage) != readback_canonical {
        return Err(invalid("Worker readback canonical hash mismatch"));
    }
    let algorithm = sha(object, "high_worker_algorithm_sha256")?;
    if readback
        .get("high_worker_algorithm_sha256")
        .and_then(Value::as_str)
        != Some(algorithm.as_str())
        || cohort.is_empty()
        || !is_sha256(cohort)
        || object.get("replay_count").and_then(Value::as_u64) != Some(2)
        || object.get("replay_byte_exact").and_then(Value::as_bool) != Some(true)
        || object.get("non_destructive").and_then(Value::as_bool) != Some(true)
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || object
            .get("production_stage_advanced")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("candidate_confirmed").and_then(Value::as_bool) != Some(false)
        || object.get("version_created").and_then(Value::as_bool) != Some(false)
        || object.get("export_performed").and_then(Value::as_bool) != Some(false)
        || object.get("quality_status").and_then(Value::as_str) != Some("structural_only")
    {
        return Err(invalid(
            "Worker replay/cohort/non-destructive policy is invalid",
        ));
    }
    let evaluation = object
        .get("evaluation")
        .ok_or_else(|| invalid("Worker evaluation is missing"))?;
    if evaluation.get("structural_status").and_then(Value::as_str) != Some("PASS_SOURCE_STRUCTURAL")
        || evaluation.get("visual_status").and_then(Value::as_str) != Some("NOT_RUN")
        || evaluation.get("human_status").and_then(Value::as_str) != Some("NOT_RUN")
        || evaluation.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || evaluation
            .get("evaluator_contract")
            .and_then(|value| value.get("policy"))
            .and_then(Value::as_str)
            != Some(EVALUATOR_CONTRACT)
    {
        return Err(invalid("Worker evaluator structural contract is invalid"));
    }
    let evaluation_sha256 = canonical_json_hash(evaluation);
    if readback
        .get("high_evaluation_sha256")
        .and_then(Value::as_str)
        != Some(evaluation_sha256.as_str())
    {
        return Err(invalid("Worker evaluation/readback hash mismatch"));
    }
    let source_parts = object
        .get("source_mesh")
        .and_then(|value| value.get("parts"))
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Worker source projection is missing"))?;
    if source_parts.len() != expected_part_inputs.len() || source_parts.is_empty() {
        return Err(invalid("Worker source part count differs"));
    }
    for (index, (source_part, expected_part)) in
        source_parts.iter().zip(expected_part_inputs).enumerate()
    {
        for field in [
            "part_id",
            "source_node_id",
            "source_node_ids",
            "material_zone_id",
        ] {
            if source_part.get(field) != expected_part.get(field) {
                return Err(invalid(format!(
                    "Worker source part {index} {field} differs"
                )));
            }
        }
    }
    let source_mesh_hash = canonical_json_hash(
        object
            .get("source_mesh")
            .ok_or_else(|| invalid("source_mesh missing"))?,
    );
    if readback
        .get("projected_source_mesh_sha256")
        .and_then(Value::as_str)
        != Some(source_mesh_hash.as_str())
    {
        return Err(invalid("Worker source mesh hash mismatch"));
    }
    let evaluated_parts = evaluation
        .get("evaluated_parts")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("evaluated parts missing"))?;
    if evaluated_parts.len() != expected_part_inputs.len() {
        return Err(invalid("Worker evaluated part count differs"));
    }
    for (index, (part, expected_part)) in
        evaluated_parts.iter().zip(expected_part_inputs).enumerate()
    {
        for field in [
            "part_id",
            "source_node_id",
            "source_node_ids",
            "material_zone_id",
        ] {
            if part.get(field) != expected_part.get(field) {
                return Err(invalid(format!(
                    "Worker evaluated part {index} {field} differs"
                )));
            }
        }
    }
    let high = HighMetadata {
        high_execution_request_sha256: execution_sha256.to_owned(),
        high_evaluation_sha256: evaluation_sha256,
        high_result_sha256: result_canonical,
        high_result_object_sha256: sha256_hex(
            &canonical_json_bytes(result).map_err(|error| invalid(error.to_string()))?,
        ),
        high_readback_sha256: readback_canonical,
        high_readback_object_sha256: sha256_hex(
            &canonical_json_bytes(&Value::Object(readback.clone()))
                .map_err(|error| invalid(error.to_string()))?,
        ),
        high_worker_algorithm_sha256: algorithm,
        high_worker_build_cohort_sha256: cohort.to_owned(),
        high_replay_count: 2,
        high_replay_byte_exact: true,
        high_non_destructive: true,
        high_projected_source_mesh_sha256: source_mesh_hash,
        high_source_vertex_count: readback
            .get("source_vertex_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("source_vertex_count missing"))?,
        high_source_triangle_count: readback
            .get("source_triangle_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("source_triangle_count missing"))?,
        high_evaluated_part_count: readback
            .get("evaluated_part_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("evaluated_part_count missing"))?,
        high_evaluated_triangle_count: readback
            .get("evaluated_triangle_count")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("evaluated_triangle_count missing"))?,
    };
    if readback
        .get("high_evaluation_sha256")
        .and_then(Value::as_str)
        != Some(high.high_evaluation_sha256.as_str())
        || readback.get("replay_count").and_then(Value::as_u64) != Some(2)
        || readback.get("replay_byte_exact").and_then(Value::as_bool) != Some(true)
        || readback.get("non_destructive").and_then(Value::as_bool) != Some(true)
        || readback
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(invalid("Worker readback policy/counts invalid"));
    }
    Ok((result.clone(), high))
}

fn main_from_record(record: &AuthoringMeshV2HighBridgeStoreRecord) -> Result<Value, RuntimeError> {
    let value = serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?;
    let mut object = value
        .as_object()
        .cloned()
        .ok_or_else(|| invalid("bridge record is not an object"))?;
    object.insert(
        "schema_version".to_owned(),
        Value::String(BRIDGE_SCHEMA_VERSION.to_owned()),
    );
    for field in [
        "bridge_sha256",
        "bridge_object_sha256",
        "request_input_sha256",
        "idempotency_key",
    ] {
        object.remove(field);
    }
    Ok(Value::Object(object))
}

fn validate_get_record(
    record: &AuthoringMeshV2HighBridgeStoreRecord,
    request: &GetRequest,
) -> Result<(), RuntimeError> {
    let identity = &request.identity;
    for (field, actual, expected) in [
        (
            "high_execution_request_sha256",
            record.high_execution_request_sha256.as_str(),
            request.high.high_execution_request_sha256.as_str(),
        ),
        (
            "high_evaluation_sha256",
            record.high_evaluation_sha256.as_str(),
            request.high.high_evaluation_sha256.as_str(),
        ),
        (
            "high_result_sha256",
            record.high_result_sha256.as_str(),
            request.high.high_result_sha256.as_str(),
        ),
        (
            "high_result_object_sha256",
            record.high_result_object_sha256.as_str(),
            request.high.high_result_object_sha256.as_str(),
        ),
        (
            "high_readback_sha256",
            record.high_readback_sha256.as_str(),
            request.high.high_readback_sha256.as_str(),
        ),
        (
            "high_readback_object_sha256",
            record.high_readback_object_sha256.as_str(),
            request.high.high_readback_object_sha256.as_str(),
        ),
        (
            "high_worker_algorithm_sha256",
            record.high_worker_algorithm_sha256.as_str(),
            request.high.high_worker_algorithm_sha256.as_str(),
        ),
        (
            "high_worker_build_cohort_sha256",
            record.high_worker_build_cohort_sha256.as_str(),
            request.high.high_worker_build_cohort_sha256.as_str(),
        ),
        (
            "high_projected_source_mesh_sha256",
            record.high_projected_source_mesh_sha256.as_str(),
            request.high.high_projected_source_mesh_sha256.as_str(),
        ),
    ] {
        if actual != expected {
            return Err(invalid(format!(
                "Get {field} differs from the durable record"
            )));
        }
    }
    for (field, actual, expected) in [
        (
            "high_replay_count",
            record.high_replay_count,
            request.high.high_replay_count,
        ),
        (
            "high_source_vertex_count",
            record.high_source_vertex_count,
            request.high.high_source_vertex_count,
        ),
        (
            "high_source_triangle_count",
            record.high_source_triangle_count,
            request.high.high_source_triangle_count,
        ),
        (
            "high_evaluated_part_count",
            record.high_evaluated_part_count,
            request.high.high_evaluated_part_count,
        ),
        (
            "high_evaluated_triangle_count",
            record.high_evaluated_triangle_count,
            request.high.high_evaluated_triangle_count,
        ),
    ] {
        if actual != expected {
            return Err(invalid(format!(
                "Get {field} differs from the durable record"
            )));
        }
    }
    let requested_solid = identity
        .values
        .get("solid")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("Get solid binding is missing"))?;
    let requested_preserved = identity
        .values
        .get("preserved_part_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Get preserved_part_ids binding is missing"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid("Get preserved_part_ids contains a non-string"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if record.high_replay_byte_exact != request.high.high_replay_byte_exact
        || record.high_non_destructive != request.high.high_non_destructive
        || record.solid != requested_solid
        || record.preserved_part_ids != requested_preserved
    {
        return Err(invalid(
            "Get boolean or preserved-part binding differs from the durable record",
        ));
    }
    Ok(())
}

fn record_from_main(
    main: Value,
    request: &Request,
    high: &HighMetadata,
) -> Result<AuthoringMeshV2HighBridgeStoreRecord, RuntimeError> {
    let mut object = main
        .as_object()
        .cloned()
        .ok_or_else(|| invalid("bridge main is not object"))?;
    object.insert(
        "bridge_sha256".to_owned(),
        object
            .get("canonical_sha256")
            .cloned()
            .unwrap_or(Value::Null),
    );
    object.insert(
        "bridge_object_sha256".to_owned(),
        Value::String(String::new()),
    );
    object.insert(
        "request_input_sha256".to_owned(),
        Value::String(request.input_sha256.clone()),
    );
    object.insert(
        "idempotency_key".to_owned(),
        Value::String(request.idempotency_key.clone()),
    );
    object.insert(
        "high_execution_request_sha256".to_owned(),
        Value::String(high.high_execution_request_sha256.clone()),
    );
    object.insert(
        "high_evaluation_sha256".to_owned(),
        Value::String(high.high_evaluation_sha256.clone()),
    );
    object.insert(
        "high_result_sha256".to_owned(),
        Value::String(high.high_result_sha256.clone()),
    );
    object.insert(
        "high_result_object_sha256".to_owned(),
        Value::String(high.high_result_object_sha256.clone()),
    );
    object.insert(
        "high_readback_sha256".to_owned(),
        Value::String(high.high_readback_sha256.clone()),
    );
    object.insert(
        "high_readback_object_sha256".to_owned(),
        Value::String(high.high_readback_object_sha256.clone()),
    );
    object.insert(
        "high_worker_algorithm_sha256".to_owned(),
        Value::String(high.high_worker_algorithm_sha256.clone()),
    );
    object.insert(
        "high_worker_build_cohort_sha256".to_owned(),
        Value::String(high.high_worker_build_cohort_sha256.clone()),
    );
    object.insert(
        "high_replay_count".to_owned(),
        Value::from(high.high_replay_count),
    );
    object.insert(
        "high_replay_byte_exact".to_owned(),
        Value::from(high.high_replay_byte_exact),
    );
    object.insert(
        "high_non_destructive".to_owned(),
        Value::from(high.high_non_destructive),
    );
    object.insert(
        "high_projected_source_mesh_sha256".to_owned(),
        Value::String(high.high_projected_source_mesh_sha256.clone()),
    );
    object.insert(
        "high_source_vertex_count".to_owned(),
        Value::from(high.high_source_vertex_count),
    );
    object.insert(
        "high_source_triangle_count".to_owned(),
        Value::from(high.high_source_triangle_count),
    );
    object.insert(
        "high_evaluated_part_count".to_owned(),
        Value::from(high.high_evaluated_part_count),
    );
    object.insert(
        "high_evaluated_triangle_count".to_owned(),
        Value::from(high.high_evaluated_triangle_count),
    );
    serde_json::from_value(Value::Object(object))
        .map_err(|error| invalid(format!("bridge Store record fields are invalid: {error}")))
}

fn finalize_main(mut main: Value) -> Result<Value, RuntimeError> {
    main["canonical_sha256"] = Value::String(String::new());
    let semantic = canonical_json_hash(&main);
    main["canonical_sha256"] = Value::String(semantic);
    Ok(main)
}

fn stage_object(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    bytes: &[u8],
    expected: &str,
    kind: &str,
    max: u64,
) -> Result<CasObject, RuntimeError> {
    if bytes.len() as u64 > max {
        return Err(invalid("CAS object exceeds bounded size"));
    }
    let object = runtime.store.put_object_reserved(
        reservation,
        bytes,
        Some(expected),
        "application/json",
        kind,
        &contract_timestamp(),
    )?;
    Ok(object)
}

fn result_envelope(
    record: &AuthoringMeshV2HighBridgeStoreRecord,
    request_input_sha256: &str,
    operation: &str,
    request_kind: &str,
    status: &str,
    idempotency_key: Option<&str>,
    replayed: bool,
    runtime_write: bool,
) -> Result<Value, RuntimeError> {
    let bridge = main_from_record(record)?;
    let mut object = Map::new();
    object.insert(
        "schema_version".to_owned(),
        Value::String(RESULT_SCHEMA_VERSION.to_owned()),
    );
    object.insert("operation".to_owned(), Value::String(operation.to_owned()));
    object.insert(
        "request_kind".to_owned(),
        Value::String(request_kind.to_owned()),
    );
    object.insert("status".to_owned(), Value::String(status.to_owned()));
    object.insert(
        "project_id".to_owned(),
        Value::String(record.project_id.clone()),
    );
    object.insert(
        "bridge_id".to_owned(),
        Value::String(record.bridge_id.clone()),
    );
    object.insert(
        "bridge_sha256".to_owned(),
        Value::String(record.bridge_sha256.clone()),
    );
    object.insert(
        "bridge_object_sha256".to_owned(),
        Value::String(record.bridge_object_sha256.clone()),
    );
    object.insert("bridge".to_owned(), bridge);
    for field in [
        "source_scope",
        "mesh_id",
        "lineage_id",
        "revision_id",
        "revision_index",
        "revision_sha256",
        "revision_object_sha256",
        "source_binding_id",
        "source_binding_sha256",
        "source_binding_object_sha256",
        "materialized_candidate_id",
        "materialized_candidate_state_sha256",
        "materialized_program_sha256",
        "materialized_program_object_sha256",
        "materialized_artifact_id",
        "materialized_artifact_sha256",
        "materialized_artifact_object_sha256",
        "materialized_artifact_readback_sha256",
        "materialized_artifact_readback_object_sha256",
        "representation_plan_sha256",
        "source_node_id",
        "part_id",
        "material_zone_id",
        "solid",
        "source_part_output_sha256",
        "preserved_part_ids",
        "high_execution_operation",
        "high_execution_request_sha256",
        "high_evaluation_sha256",
        "high_result_sha256",
        "high_result_object_sha256",
        "high_readback_sha256",
        "high_readback_object_sha256",
        "high_worker_algorithm_sha256",
        "high_worker_build_cohort_sha256",
        "high_replay_count",
        "high_replay_byte_exact",
        "high_non_destructive",
        "high_structural_status",
        "high_status",
        "quality_status",
        "visual_status",
        "human_status",
        "engine_status",
        "high_mesh_created",
        "high_stage_unlocked",
        "production_stage_advanced",
        "candidate_confirmed",
        "version_created",
        "export_performed",
    ] {
        object.insert(field.to_owned(), record_value(record, field)?);
    }
    object.insert(
        "request_input_sha256".to_owned(),
        Value::String(request_input_sha256.to_owned()),
    );
    object.insert(
        "idempotency_key".to_owned(),
        idempotency_key.map_or(Value::Null, |value| Value::String(value.to_owned())),
    );
    object.insert("replayed".to_owned(), Value::Bool(replayed));
    let inserted = status == "prepared" && !replayed;
    object.insert(
        "store_effect".to_owned(),
        Value::String(if inserted { "inserted" } else { "not-touched" }.to_owned()),
    );
    object.insert(
        "cas_effect".to_owned(),
        Value::String(if inserted { "inserted" } else { "not-touched" }.to_owned()),
    );
    object.insert(
        "atomicity_status".to_owned(),
        Value::String(if inserted { "committed" } else { "not-touched" }.to_owned()),
    );
    object.insert(
        "store_commit_status".to_owned(),
        Value::String(if inserted { "committed" } else { "not-touched" }.to_owned()),
    );
    object.insert(
        "cas_commit_status".to_owned(),
        Value::String(if inserted { "committed" } else { "not-touched" }.to_owned()),
    );
    object.insert(
        "runtime_write_performed".to_owned(),
        Value::Bool(runtime_write),
    );
    object.insert(
        "persistent_user_data_touched".to_owned(),
        Value::Bool(runtime_write),
    );
    object.insert("partial_result_exposed".to_owned(), Value::Bool(false));
    object.insert(
        "writer_policy".to_owned(),
        Value::String(WRITER_POLICY.to_owned()),
    );
    object.insert(
        "canonicalization_policy".to_owned(),
        Value::String(MAIN_CANONICALIZATION.to_owned()),
    );
    object.insert("canonical_sha256".to_owned(), Value::String(String::new()));
    let mut result = Value::Object(object);
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    Ok(result)
}

fn record_value(
    record: &AuthoringMeshV2HighBridgeStoreRecord,
    field: &str,
) -> Result<Value, RuntimeError> {
    let value = serde_json::to_value(record).map_err(|error| invalid(error.to_string()))?;
    Ok(value.get(field).cloned().unwrap_or(Value::Null))
}

fn build_and_commit(
    runtime: &Runtime,
    request: &Request,
    result: Value,
    high: &HighMetadata,
) -> Result<(AuthoringMeshV2HighBridgeStoreRecord, bool), RuntimeError> {
    let result_bytes = canonical_json_bytes(&result).map_err(|error| invalid(error.to_string()))?;
    let readback = result
        .get("readback")
        .cloned()
        .ok_or_else(|| invalid("Worker result readback missing"))?;
    let readback_bytes =
        canonical_json_bytes(&readback).map_err(|error| invalid(error.to_string()))?;
    let mut main = json!({
        "schema_version": BRIDGE_SCHEMA_VERSION,
        "bridge_id": identity_value(&request.identity, "bridge_id"), "project_id": identity_value(&request.identity, "project_id"),
        "source_scope": SOURCE_SCOPE, "source_revision_schema_version": REVISION_SCHEMA_VERSION,
        "mesh_id": identity_value(&request.identity, "mesh_id"), "lineage_id": identity_value(&request.identity, "lineage_id"),
        "revision_id": identity_value(&request.identity, "revision_id"), "revision_index": identity_value(&request.identity, "revision_index"),
        "revision_sha256": identity_value(&request.identity, "revision_sha256"), "revision_object_sha256": identity_value(&request.identity, "revision_object_sha256"),
        "source_binding_id": identity_value(&request.identity, "source_binding_id"), "source_binding_sha256": identity_value(&request.identity, "source_binding_sha256"),
        "source_binding_object_sha256": identity_value(&request.identity, "source_binding_object_sha256"), "materialized_candidate_id": identity_value(&request.identity, "materialized_candidate_id"),
        "materialized_candidate_state_sha256": identity_value(&request.identity, "materialized_candidate_state_sha256"), "materialized_program_sha256": identity_value(&request.identity, "materialized_program_sha256"),
        "materialized_program_object_sha256": identity_value(&request.identity, "materialized_program_object_sha256"), "materialized_artifact_id": identity_value(&request.identity, "materialized_artifact_id"),
        "materialized_artifact_sha256": identity_value(&request.identity, "materialized_artifact_sha256"), "materialized_artifact_object_sha256": identity_value(&request.identity, "materialized_artifact_object_sha256"),
        "materialized_artifact_readback_sha256": identity_value(&request.identity, "materialized_artifact_readback_sha256"), "materialized_artifact_readback_object_sha256": identity_value(&request.identity, "materialized_artifact_readback_object_sha256"),
        "representation_plan_sha256": identity_value(&request.identity, "representation_plan_sha256"), "source_node_id": identity_value(&request.identity, "source_node_id"),
        "part_id": identity_value(&request.identity, "part_id"), "material_zone_id": identity_value(&request.identity, "material_zone_id"), "solid": identity_value(&request.identity, "solid"),
        "source_part_output_sha256": identity_value(&request.identity, "source_part_output_sha256"), "preserved_part_ids": identity_value(&request.identity, "preserved_part_ids"),
        "materialized_artifact_hash_policy": ARTIFACT_HASH_POLICY, "high_execution_request_schema_version": EXECUTION_SCHEMA_VERSION,
        "high_execution_operation": EXECUTION_OPERATION, "high_operation": HIGH_OPERATION, "high_result_schema_version": HIGH_RESULT_SCHEMA_VERSION,
        "high_readback_schema_version": HIGH_READBACK_SCHEMA_VERSION, "high_evaluator_contract": EVALUATOR_CONTRACT, "high_subdivision_backend": SUBDIVISION_BACKEND,
        "high_subdivision_levels": 1, "high_max_triangles_per_face": 32, "high_max_output_vertices": HIGH_MAX_OUTPUT_VERTICES, "high_max_output_triangles": HIGH_MAX_OUTPUT_TRIANGLES,
        "high_execution_request_sha256": high.high_execution_request_sha256, "high_evaluation_sha256": high.high_evaluation_sha256,
        "high_result_sha256": high.high_result_sha256, "high_result_object_sha256": high.high_result_object_sha256,
        "high_readback_sha256": high.high_readback_sha256, "high_readback_object_sha256": high.high_readback_object_sha256,
        "high_worker_algorithm_sha256": high.high_worker_algorithm_sha256, "high_worker_build_cohort_sha256": high.high_worker_build_cohort_sha256,
        "high_replay_count": high.high_replay_count, "high_replay_byte_exact": high.high_replay_byte_exact, "high_non_destructive": high.high_non_destructive,
        "high_projected_source_mesh_sha256": high.high_projected_source_mesh_sha256, "high_source_vertex_count": high.high_source_vertex_count,
        "high_source_triangle_count": high.high_source_triangle_count, "high_evaluated_part_count": high.high_evaluated_part_count, "high_evaluated_triangle_count": high.high_evaluated_triangle_count,
        "cohort_policy": COHORT_POLICY, "scope_limitations": SCOPE_LIMITATIONS, "high_structural_status": "PASS_SOURCE_STRUCTURAL", "high_status": "NOT_RUN", "quality_status": "structural_only", "visual_status": "NOT_RUN", "human_status": "NOT_RUN", "engine_status": "NOT_RUN",
        "high_mesh_created": false, "high_stage_unlocked": false, "production_stage_advanced": false, "candidate_confirmed": false, "version_created": false, "export_performed": false,
        "runtime_write_performed": true, "persistent_user_data_touched": true, "writer_policy": WRITER_POLICY, "canonicalization_policy": MAIN_CANONICALIZATION,
        "canonical_sha256": "", "created_at": contract_timestamp()
    });
    main = finalize_main(main)?;
    let semantic = main
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("main canonical hash missing"))?
        .to_owned();
    let mut record = record_from_main(main.clone(), request, high)?;
    record.bridge_sha256 = semantic.clone();
    let main_bytes = canonical_json_bytes(&main).map_err(|error| invalid(error.to_string()))?;
    record.bridge_object_sha256 = sha256_hex(&main_bytes);
    let expected_cohort = super::build_cohort_sha256()
        .ok_or_else(|| invalid("same-cohort Runtime/Worker build required"))?;
    if expected_cohort != high.high_worker_build_cohort_sha256 {
        return Err(invalid("Worker build cohort differs from Runtime"));
    }
    let reservation = runtime.store.begin_cas_reservation();
    let bridge_object = stage_object(
        runtime,
        &reservation,
        &main_bytes,
        &record.bridge_object_sha256,
        BRIDGE_OBJECT_KIND,
        MAX_BRIDGE_BYTES,
    )?;
    let result_object = match stage_object(
        runtime,
        &reservation,
        &result_bytes,
        &record.high_result_object_sha256,
        RESULT_OBJECT_KIND,
        MAX_JSON_BYTES,
    ) {
        Ok(value) => value,
        Err(error) => {
            let _ =
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, &bridge_object, true);
            return Err(error);
        }
    };
    let readback_object = match stage_object(
        runtime,
        &reservation,
        &readback_bytes,
        &record.high_readback_object_sha256,
        READBACK_OBJECT_KIND,
        MAX_JSON_BYTES,
    ) {
        Ok(value) => value,
        Err(error) => {
            let _ =
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, &bridge_object, true);
            let _ =
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, &result_object, true);
            return Err(error);
        }
    };
    let commit = AuthoringMeshV2HighBridgeCommit {
        record: record.clone(),
        cas: AuthoringMeshV2HighBridgeCasBundle {
            bridge: bridge_object.record.clone(),
            high_result: result_object.record.clone(),
            high_readback: readback_object.record.clone(),
        },
    };
    match runtime
        .store
        .record_authoring_mesh_v2_high_bridge_with_replay(&commit)
    {
        Ok(value) => Ok(value),
        Err(error) => {
            let _ =
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, &bridge_object, true);
            let _ =
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, &result_object, true);
            let _ =
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, &readback_object, true);
            Err(error.into())
        }
    }
}

pub(crate) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_prepare(value)?;
    if let Some(existing) = runtime.store.get_authoring_mesh_v2_high_bridge(
        &identity_text(&request.identity, "project_id"),
        &request.idempotency_key,
    )? {
        if existing.request_input_sha256 != request.input_sha256 {
            return Err(invalid("idempotency key is bound to another request"));
        }
        let result = result_envelope(
            &existing,
            &request.input_sha256,
            PREPARE_OPERATION,
            "prepare",
            "replayed",
            None,
            true,
            false,
        )?;
        return require_response_size(&result, request.max_response_bytes);
    }
    // Do not launch a Worker when this Runtime has no immutable build-cohort
    // identity.  A result from an unbound development process cannot become
    // durable bridge truth; report NOT_RUN before any external execution or
    // CAS reservation instead of letting a later persistence check obscure
    // the actual gate.
    let runtime_cohort = super::build_cohort_sha256().ok_or_else(|| {
        invalid("same-cohort Runtime/Worker build required; High bridge remains NOT_RUN")
    })?;
    let (revision, binding) = load_lineage(runtime, &request.identity)?;
    let embedded = revision
        .source_binding
        .as_ref()
        .ok_or_else(|| invalid("revision has no embedded SourceBinding"))?;
    let sources = load_candidate_sources(runtime, &request.identity, &binding, embedded)?;
    let part_inputs = build_part_inputs(&revision, &request.identity, &sources)?;
    let (execution, execution_sha256) = build_execution_request(&revision, part_inputs.clone())?;
    let worker = geometry_worker::production_weapon_authoring_mesh_v2_high(&execution)
        .map_err(|error| invalid(error.to_string()))?;
    let cohort = worker
        .build_cohort_sha256
        .ok_or_else(|| invalid("same-cohort Runtime/Worker build required"))?;
    if cohort != runtime_cohort {
        return Err(invalid("Worker build cohort differs from Runtime"));
    }
    let (result, high) = validate_worker_result(
        &worker.result,
        &request.identity,
        &part_inputs,
        &execution_sha256,
        &cohort,
    )?;
    if high.high_execution_request_sha256 != execution_sha256 {
        return Err(invalid("execution request hash mismatch"));
    }
    let (record, replayed) = build_and_commit(runtime, &request, result, &high)?;
    let response = result_envelope(
        &record,
        &request.input_sha256,
        PREPARE_OPERATION,
        "prepare",
        if replayed { "replayed" } else { "prepared" },
        if replayed {
            None
        } else {
            Some(&request.idempotency_key)
        },
        replayed,
        !replayed,
    )?;
    require_response_size(&response, request.max_response_bytes)
}

pub(crate) fn get(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_get(value)?;
    let i = &request.identity;
    let record = runtime
        .store
        .get_authoring_mesh_v2_high_bridge_exact(
            identity_text(i, "project_id"),
            identity_text(i, "bridge_id"),
            &request.bridge_sha256,
            &request.bridge_object_sha256,
            identity_text(i, "source_binding_id"),
            identity_text(i, "source_binding_sha256"),
            identity_text(i, "source_binding_object_sha256"),
            identity_text(i, "mesh_id"),
            identity_text(i, "lineage_id"),
            identity_text(i, "revision_id"),
            identity_u64(i, "revision_index"),
            identity_text(i, "revision_sha256"),
            identity_text(i, "revision_object_sha256"),
            identity_text(i, "materialized_candidate_id"),
            identity_text(i, "materialized_candidate_state_sha256"),
            identity_text(i, "materialized_program_sha256"),
            identity_text(i, "materialized_program_object_sha256"),
            identity_text(i, "materialized_artifact_id"),
            identity_text(i, "materialized_artifact_sha256"),
            identity_text(i, "materialized_artifact_object_sha256"),
            identity_text(i, "materialized_artifact_readback_sha256"),
            identity_text(i, "materialized_artifact_readback_object_sha256"),
            identity_text(i, "representation_plan_sha256"),
            identity_text(i, "source_node_id"),
            identity_text(i, "part_id"),
            identity_text(i, "material_zone_id"),
            identity_text(i, "source_part_output_sha256"),
            &request.high.high_execution_request_sha256,
            identity_text(i, "high_execution_operation"),
            identity_text(i, "high_operation"),
            &request.high.high_result_sha256,
            &request.high.high_result_object_sha256,
            &request.high.high_readback_sha256,
            &request.high.high_readback_object_sha256,
            &request.high.high_worker_algorithm_sha256,
            &request.high.high_worker_build_cohort_sha256,
        )?
        .ok_or_else(|| invalid("High bridge exact record was not found"))?;
    validate_get_record(&record, &request)?;
    // Get does not trust caller-supplied high metadata; the exact Store lookup
    // above compared every identity that is part of the public request.
    let response = result_envelope(
        &record,
        &request.input_sha256,
        GET_OPERATION,
        "get",
        "found",
        None,
        false,
        false,
    )?;
    require_response_size(&response, request.max_response_bytes)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn recompute_input_hash(value: &mut Value) {
        value["input_sha256"] = Value::String(String::new());
        value["input_sha256"] = Value::String(canonical_json_hash(value));
    }

    fn fixture(name: &str) -> Value {
        let path = format!(
            "{}/../../../../../packages/forgecad-contracts/fixtures/authoring-mesh-v2-high-bridge/positive/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(
            &std::fs::read_to_string(path).expect("High bridge contract fixture must exist"),
        )
        .expect("High bridge contract fixture must be valid JSON")
    }

    #[test]
    fn prepare_parser_accepts_closed_contract_and_rejects_extra_field() {
        let request = fixture("dragonfang-high-bridge-prepare-request.json");
        let parsed = parse_prepare(&request).expect("positive prepare fixture");
        assert_eq!(parsed.max_response_bytes, MAX_RESPONSE_BYTES as usize);
        assert_eq!(parsed.identity.values.len(), PREPARE_FIELDS.len());

        let mut extra = request;
        extra["unexpected"] = Value::Bool(true);
        let error = parse_prepare(&extra).expect_err("extra prepare field must fail closed");
        assert!(error
            .to_string()
            .contains("request fields differ from the closed envelope"));
    }

    #[test]
    fn get_parser_binds_high_metadata_and_rejects_hash_drift() {
        let request = fixture("dragonfang-high-bridge-get-request.json");
        let parsed = parse_get(&request).expect("positive get fixture");
        assert_eq!(parsed.high.high_replay_count, 2);
        assert!(parsed.high.high_replay_byte_exact);
        assert!(parsed.high.high_non_destructive);
        let mut record_value = fixture("dragonfang-high-bridge.json");
        record_value["bridge_sha256"] = record_value["canonical_sha256"].clone();
        record_value["bridge_object_sha256"] = request["bridge_object_sha256"].clone();
        record_value["request_input_sha256"] = request["input_sha256"].clone();
        record_value["idempotency_key"] = Value::String("dragonfang-high-bridge-key".to_owned());
        let record: AuthoringMeshV2HighBridgeStoreRecord =
            serde_json::from_value(record_value).expect("Store record fixture");
        validate_get_record(&record, &parsed).expect("matching High metadata");

        let mut drifted = request;
        drifted["high_evaluation_sha256"] = Value::String("0".repeat(64));
        drifted["input_sha256"] = Value::String(String::new());
        drifted["input_sha256"] = Value::String(canonical_json_hash(&drifted));
        let drifted =
            parse_get(&drifted).expect("hash-shaped drift remains structurally parseable");
        let error = validate_get_record(&record, &drifted)
            .expect_err("high metadata drift must fail closed at exact lookup");
        assert!(error.to_string().contains("high_evaluation_sha256"));
    }

    #[test]
    fn no_cohort_prepare_is_not_run_and_does_not_create_store_state() {
        if crate::build_cohort_sha256().is_some() {
            eprintln!("no-cohort gate test is only meaningful without a build cohort");
            return;
        }
        let runtime = Runtime::ephemeral().expect("ephemeral Runtime");
        let request = fixture("dragonfang-high-bridge-prepare-request.json");
        let error = runtime
            .authoring_mesh_v2_high_bridge_prepare(&request)
            .expect_err("unbound Runtime must not run High Worker");
        assert!(error
            .to_string()
            .contains("same-cohort Runtime/Worker build required"));
    }

    fn materializer_request(source: &Value) -> Value {
        let mut request = json!({
            "schema_version": "AuthoringMeshV2CandidateMaterializeRequest@1",
            "operation": "authoring_mesh_v2_candidate_materialize",
            "project_id": source["project_id"],
            "mesh_id": source["authoring_mesh_id"],
            "lineage_id": source["authoring_mesh_lineage_id"],
            "revision_id": source["authoring_mesh_revision_id"],
            "revision_index": source["authoring_mesh_revision_index"],
            "revision_sha256": source["authoring_mesh_revision_sha256"],
            "revision_object_sha256": source["authoring_mesh_revision_object_sha256"],
            "source_binding_id": source["source_binding_id"],
            "source_binding_sha256": source["source_binding_sha256"],
            "source_binding_object_sha256": source["source_binding_object_sha256"],
            "base_version_id": null,
            "idempotency_key": "high-bridge-materializer-live",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        recompute_input_hash(&mut request);
        request
    }

    fn high_prepare_request(
        source: &Value,
        materialized: &Value,
        evidence: &forgecad_contracts::GeometryCandidateEvidenceRecord,
        bridge_id: &str,
    ) -> Value {
        let candidate = &materialized["candidate"];
        let artifact = &materialized["artifact"];
        let mut request = json!({
            "schema_version": PREPARE_SCHEMA_VERSION,
            "operation": PREPARE_OPERATION,
            "project_id": source["project_id"],
            "bridge_id": bridge_id,
            "source_scope": SOURCE_SCOPE,
            "source_revision_schema_version": REVISION_SCHEMA_VERSION,
            "mesh_id": source["authoring_mesh_id"],
            "lineage_id": source["authoring_mesh_lineage_id"],
            "revision_id": source["authoring_mesh_revision_id"],
            "revision_index": source["authoring_mesh_revision_index"],
            "revision_sha256": source["authoring_mesh_revision_sha256"],
            "revision_object_sha256": source["authoring_mesh_revision_object_sha256"],
            "source_binding_id": source["source_binding_id"],
            "source_binding_sha256": source["source_binding_sha256"],
            "source_binding_object_sha256": source["source_binding_object_sha256"],
            "materialized_candidate_id": candidate["candidate_id"],
            "materialized_candidate_state_sha256": candidate["canonical_sha256"],
            "materialized_program_sha256": evidence.geometry_program_sha256,
            "materialized_program_object_sha256": evidence.geometry_program_object_sha256,
            "materialized_artifact_id": candidate["prepared_object_id"],
            "materialized_artifact_sha256": candidate["prepared_object_sha256"],
            "materialized_artifact_object_sha256": evidence.artifact_object_sha256,
            "materialized_artifact_readback_sha256": artifact["canonical_sha256"],
            "materialized_artifact_readback_object_sha256": evidence.artifact_readback_object_sha256,
            "representation_plan_sha256": materialized["representation_plan_sha256"],
            "source_node_id": materialized["source_node_id"],
            "part_id": materialized["source_part_id"],
            "material_zone_id": materialized["source_material_zone_id"],
            "solid": materialized["source_solid"],
            "source_part_output_sha256": materialized["source_part_output_sha256"],
            "preserved_part_ids": materialized["preserved_part_ids"],
            "materialized_artifact_hash_policy": ARTIFACT_HASH_POLICY,
            "high_execution_request_schema_version": EXECUTION_SCHEMA_VERSION,
            "high_execution_operation": EXECUTION_OPERATION,
            "high_operation": HIGH_OPERATION,
            "high_result_schema_version": HIGH_RESULT_SCHEMA_VERSION,
            "high_readback_schema_version": HIGH_READBACK_SCHEMA_VERSION,
            "high_evaluator_contract": EVALUATOR_CONTRACT,
            "high_subdivision_backend": SUBDIVISION_BACKEND,
            "high_subdivision_levels": 1,
            "high_max_triangles_per_face": 32,
            "high_max_output_vertices": HIGH_MAX_OUTPUT_VERTICES,
            "high_max_output_triangles": HIGH_MAX_OUTPUT_TRIANGLES,
            "scope_limitations": SCOPE_LIMITATIONS,
            "idempotency_key": "high-bridge-live",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        recompute_input_hash(&mut request);
        request
    }

    /// Build the complete source-bound High Bridge prerequisite for a sibling
    /// V2 artifact test.  This stays test-only so the production Runtime API
    /// cannot acquire fixture-specific setup or bypass the public bridge.
    pub(crate) fn prepare_live_high_bridge_for_artifact(runtime: &Runtime, suffix: &str) -> Value {
        let source_request =
            crate::weaponry_knife_source_binding::test_multi_part_source_binding_request(
                runtime, suffix,
            );
        let source = runtime
            .knife_source_binding_prepare(&source_request)
            .expect("SourceBinding prerequisite");
        let materialized = runtime
            .authoring_mesh_v2_candidate_materialize(&materializer_request(&source))
            .expect("source-bound materialization prerequisite");
        let candidate_id = materialized["candidate"]["candidate_id"]
            .as_str()
            .expect("materialized candidate id");
        let evidence = runtime
            .store
            .get_geometry_candidate_evidence(candidate_id)
            .expect("materialized evidence lookup")
            .expect("materialized evidence");
        let bridge_id = format!("high-bridge-{suffix}");
        let request = high_prepare_request(&source, &materialized, &evidence, &bridge_id);
        runtime
            .authoring_mesh_v2_high_bridge_prepare(&request)
            .expect("High Bridge prerequisite")
    }

    fn high_get_request(first: &Value) -> Value {
        let bridge = &first["bridge"];
        let fixture = fixture("dragonfang-high-bridge-get-request.json");
        let mut request = fixture;
        for field in GET_FIELDS {
            if let Some(value) = bridge.get(*field) {
                request[*field] = value.clone();
            } else if let Some(value) = first.get(*field) {
                request[*field] = value.clone();
            }
        }
        // The durable bridge carries the Main-record canonicalization policy;
        // the read envelope has its own request/input-hash policy.  Reassert
        // the closed transport fields after copying identity bindings so this
        // helper exercises the same public Get contract as MCP callers.
        request["schema_version"] = Value::String(GET_SCHEMA_VERSION.to_owned());
        request["operation"] = Value::String(GET_OPERATION.to_owned());
        request["max_response_bytes"] = Value::from(MAX_RESPONSE_BYTES);
        request["runtime_write_performed"] = Value::Bool(false);
        request["persistent_user_data_touched"] = Value::Bool(false);
        request["writer_policy"] = Value::String(WRITER_POLICY.to_owned());
        request["canonicalization_policy"] = Value::String(REQUEST_CANONICALIZATION.to_owned());
        request["bridge_sha256"] = first["bridge_sha256"].clone();
        request["bridge_object_sha256"] = first["bridge_object_sha256"].clone();
        recompute_input_hash(&mut request);
        request
    }

    fn assert_high_result_shape(value: &Value, status: &str, runtime_write: bool) {
        let expected = fixture("dragonfang-high-bridge-result-prepared.json");
        let actual = value
            .as_object()
            .expect("High bridge result must be an object")
            .keys()
            .collect::<BTreeSet<_>>();
        let expected = expected
            .as_object()
            .expect("High bridge result fixture must be an object")
            .keys()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual, expected,
            "High bridge result fields must remain closed"
        );
        assert_eq!(value["status"], status);
        assert_eq!(value["runtime_write_performed"], runtime_write);
        assert_eq!(value["persistent_user_data_touched"], runtime_write);
        assert_eq!(value["high_status"], "NOT_RUN");
        assert_eq!(value["quality_status"], "structural_only");
        assert_eq!(value["visual_status"], "NOT_RUN");
        assert_eq!(value["human_status"], "NOT_RUN");
        assert_eq!(value["engine_status"], "NOT_RUN");
        assert_eq!(value["high_mesh_created"], false);
        assert_eq!(value["high_stage_unlocked"], false);
        assert_eq!(value["production_stage_advanced"], false);
        assert_eq!(value["candidate_confirmed"], false);
        assert_eq!(value["version_created"], false);
        assert_eq!(value["export_performed"], false);
        assert_eq!(value["high_replay_count"], 2);
        assert_eq!(value["high_replay_byte_exact"], true);
        assert_eq!(value["high_non_destructive"], true);
        assert_eq!(
            value["high_worker_algorithm_sha256"].as_str().map(str::len),
            Some(64)
        );
        assert_eq!(
            value["high_worker_build_cohort_sha256"],
            crate::build_cohort_sha256().expect("same-cohort test build")
        );
    }

    #[test]
    fn live_same_cohort_high_bridge_replays_gets_and_reopens_without_false_high_claims() {
        if crate::build_cohort_sha256().is_none() {
            eprintln!("High bridge live test requires FORGECAD_BUILD_COHORT_SHA256");
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "forgecad-high-bridge-live-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("High bridge test root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let runtime = Runtime::open_with_cas(&database, &cas).expect("file-backed Runtime");

        let source_request =
            crate::weaponry_knife_source_binding::test_multi_part_source_binding_request(
                &runtime,
                "high-bridge-live",
            );
        let source = runtime
            .knife_source_binding_prepare(&source_request)
            .expect("SourceBinding prepare");
        let materialized = runtime
            .authoring_mesh_v2_candidate_materialize(&materializer_request(&source))
            .expect("source-bound materialization");
        let candidate_id = materialized["candidate"]["candidate_id"]
            .as_str()
            .expect("materialized candidate id");
        let evidence = runtime
            .store
            .get_geometry_candidate_evidence(candidate_id)
            .expect("materialized evidence lookup")
            .expect("materialized evidence");
        let request = high_prepare_request(&source, &materialized, &evidence, "high-bridge-live");
        let before_high_cas = runtime
            .store
            .cas()
            .list_objects()
            .expect("CAS listing before High")
            .len();

        let first = runtime
            .authoring_mesh_v2_high_bridge_prepare(&request)
            .expect("High bridge prepare");
        assert_high_result_shape(&first, "prepared", true);
        assert_eq!(first["high_execution_operation"], EXECUTION_OPERATION);
        assert_eq!(
            first["high_worker_algorithm_sha256"].as_str().map(str::len),
            Some(64)
        );
        let after_first_cas = runtime
            .store
            .cas()
            .list_objects()
            .expect("CAS listing after High")
            .len();
        assert_eq!(after_first_cas - before_high_cas, 3);

        let replay = runtime
            .authoring_mesh_v2_high_bridge_prepare(&request)
            .expect("High bridge exact replay");
        assert_high_result_shape(&replay, "replayed", false);
        assert_eq!(replay["bridge_sha256"], first["bridge_sha256"]);
        assert_eq!(
            replay["bridge_object_sha256"],
            first["bridge_object_sha256"]
        );
        assert_eq!(replay["store_effect"], "not-touched");
        assert_eq!(replay["cas_effect"], "not-touched");
        assert_eq!(
            runtime
                .store
                .cas()
                .list_objects()
                .expect("CAS listing after replay")
                .len(),
            after_first_cas
        );

        let mut conflict = request.clone();
        conflict["bridge_id"] = Value::String("high-bridge-live-conflict".to_owned());
        recompute_input_hash(&mut conflict);
        let error = runtime
            .authoring_mesh_v2_high_bridge_prepare(&conflict)
            .expect_err("same idempotency key conflict must fail closed");
        assert!(error.to_string().contains("idempotency key"));
        assert_eq!(
            runtime
                .store
                .cas()
                .list_objects()
                .expect("CAS listing after conflict")
                .len(),
            after_first_cas
        );

        let get_request = high_get_request(&first);
        let found = runtime
            .authoring_mesh_v2_high_bridge_get(&get_request)
            .expect("High bridge exact get");
        assert_high_result_shape(&found, "found", false);
        assert_eq!(found["bridge_sha256"], first["bridge_sha256"]);
        assert_eq!(found["idempotency_key"], Value::Null);
        assert_eq!(
            runtime
                .store
                .cas()
                .list_objects()
                .expect("CAS listing after get")
                .len(),
            after_first_cas
        );

        drop(runtime);
        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopen Runtime");
        let restarted = reopened
            .authoring_mesh_v2_high_bridge_get(&get_request)
            .expect("High bridge restart get");
        assert_high_result_shape(&restarted, "found", false);
        assert_eq!(restarted["bridge_sha256"], first["bridge_sha256"]);
        assert_eq!(
            restarted["bridge_object_sha256"],
            first["bridge_object_sha256"]
        );
        assert_eq!(
            reopened
                .store
                .cas()
                .list_objects()
                .expect("CAS listing after restart")
                .len(),
            after_first_cas
        );
        println!(
            "WPN_HIGH_BRIDGE_LIVE_EVIDENCE={}",
            serde_json::to_string(&json!({
                "schema_version":"WeaponryAuthoringMeshV2HighBridgeLiveEvidence@1",
                "bridge_sha256":first["bridge_sha256"],
                "bridge_object_sha256":first["bridge_object_sha256"],
                "high_worker_algorithm_sha256":first["high_worker_algorithm_sha256"],
                "high_worker_build_cohort_sha256":first["high_worker_build_cohort_sha256"],
                "high_result_sha256":first["high_result_sha256"],
                "high_readback_sha256":first["high_readback_sha256"],
                "cas_object_delta":after_first_cas - before_high_cas,
                "prepare_status":first["status"],
                "replay_status":replay["status"],
                "get_status":found["status"],
                "restart_get_status":restarted["status"],
                "replay_byte_exact":first["high_replay_byte_exact"],
                "restart_hash_verified":true,
                "high_structural_status":"PASS_SOURCE_STRUCTURAL",
                "high_status":first["high_status"],
                "quality_status":first["quality_status"],
                "visual_status":first["visual_status"],
                "human_status":first["human_status"],
                "engine_status":first["engine_status"],
                "commercial_quality":"NOT_PROVEN"
            }))
            .expect("serialize bounded High bridge live evidence")
        );
        drop(reopened);
        std::fs::remove_dir_all(root).expect("remove High bridge test root");
    }

    fn high_artifact_prepare_request(bridge_result: &Value) -> Value {
        let mut request = json!({
            "schema_version": "AuthoringMeshV2HighArtifactPrepareRequest@1",
            "operation": "authoring_mesh_v2_high_artifact_prepare",
            "project_id": bridge_result["project_id"],
            "high_artifact_id": "dragonfang-high-artifact-live",
            "high_bridge_id": bridge_result["bridge_id"],
            "high_bridge_sha256": bridge_result["bridge_sha256"],
            "high_bridge_object_sha256": bridge_result["bridge_object_sha256"],
            "idempotency_key": "dragonfang-high-artifact-live-key",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        recompute_input_hash(&mut request);
        request
    }

    fn high_artifact_get_request(result: &Value) -> Value {
        let mut request = json!({
            "schema_version": "AuthoringMeshV2HighArtifactGetRequest@1",
            "operation": "authoring_mesh_v2_high_artifact_get",
            "project_id": result["project_id"],
            "high_artifact_id": result["high_artifact_id"],
            "high_artifact_sha256": result["high_artifact_sha256"],
            "high_artifact_object_sha256": result["high_artifact_object_sha256"],
            "high_artifact_readback_sha256": result["high_artifact_readback_sha256"],
            "high_artifact_readback_object_sha256": result["high_artifact_readback_object_sha256"],
            "high_artifact_receipt_sha256": result["high_artifact_receipt_sha256"],
            "high_artifact_receipt_object_sha256": result["high_artifact_receipt_object_sha256"],
            "high_bridge_id": result["high_bridge_id"],
            "high_bridge_sha256": result["high_bridge_sha256"],
            "high_bridge_object_sha256": result["high_bridge_object_sha256"],
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "persistent_user_data_touched": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": REQUEST_CANONICALIZATION,
            "input_sha256": ""
        });
        recompute_input_hash(&mut request);
        request
    }

    fn assert_high_artifact_result(value: &Value, status: &str, runtime_write: bool) {
        let expected: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../packages/forgecad-contracts/fixtures/authoring-mesh-v2-high-artifact/positive/dragonfang-high-artifact-result-prepared.json"
        )))
        .expect("High artifact result fixture");
        assert_eq!(
            value
                .as_object()
                .expect("High artifact result object")
                .keys()
                .collect::<BTreeSet<_>>(),
            expected
                .as_object()
                .expect("High artifact fixture object")
                .keys()
                .collect::<BTreeSet<_>>()
        );
        assert_eq!(value["status"], status);
        assert_eq!(value["runtime_write_performed"], runtime_write);
        assert_eq!(value["persistent_user_data_touched"], runtime_write);
        assert_eq!(value["high_mesh_created"], true);
        assert_eq!(value["high_artifact_hard_gate_passed"], true);
        assert_eq!(value["high_artifact_status"], "PASS_SOURCE_STRUCTURAL");
        assert_eq!(value["high_status"], "NOT_RUN");
        assert_eq!(value["quality_status"], "structural_only");
        assert_eq!(value["visual_status"], "NOT_RUN");
        assert_eq!(value["human_status"], "NOT_RUN");
        assert_eq!(value["engine_status"], "NOT_RUN");
        assert_eq!(value["distribution_status"], "NOT_RUN");
        assert_eq!(value["high_stage_unlocked"], false);
        assert_eq!(value["production_stage_advanced"], false);
        assert_eq!(value["candidate_confirmed"], false);
        assert_eq!(value["version_created"], false);
        assert_eq!(value["export_performed"], false);
        assert_eq!(
            value["high_artifact"]["high_artifact_readback_schema_version"],
            "AuthoringMeshV2HighArtifactStoreReadback@1"
        );
        assert_eq!(
            value["strict_readback"]["schema_version"],
            "AuthoringMeshV2HighArtifactReadback@1"
        );
    }

    #[test]
    fn live_same_cohort_high_artifact_replays_gets_and_reopens_with_strict_glb_readback() {
        if crate::build_cohort_sha256().is_none() {
            eprintln!("High artifact live test requires FORGECAD_BUILD_COHORT_SHA256");
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "forgecad-high-artifact-live-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("High artifact test root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let runtime = Runtime::open_with_cas(&database, &cas).expect("file-backed Runtime");

        let source_request =
            crate::weaponry_knife_source_binding::test_multi_part_source_binding_request(
                &runtime,
                "high-artifact-live",
            );
        let source = runtime
            .knife_source_binding_prepare(&source_request)
            .expect("SourceBinding prepare");
        let materialized = runtime
            .authoring_mesh_v2_candidate_materialize(&materializer_request(&source))
            .expect("source-bound materialization");
        let candidate_id = materialized["candidate"]["candidate_id"]
            .as_str()
            .expect("materialized candidate id");
        let evidence = runtime
            .store
            .get_geometry_candidate_evidence(candidate_id)
            .expect("materialized evidence lookup")
            .expect("materialized evidence");
        let bridge_request = high_prepare_request(
            &source,
            &materialized,
            &evidence,
            "high-bridge-for-artifact-live",
        );
        let bridge = runtime
            .authoring_mesh_v2_high_bridge_prepare(&bridge_request)
            .expect("High bridge prepare");
        let request = high_artifact_prepare_request(&bridge);
        let before = runtime
            .store
            .cas()
            .list_objects()
            .expect("CAS before High artifact")
            .len();
        let first = runtime
            .authoring_mesh_v2_high_artifact_prepare(&request)
            .expect("High artifact prepare");
        assert_high_artifact_result(&first, "prepared", true);
        let after = runtime
            .store
            .cas()
            .list_objects()
            .expect("CAS after High artifact")
            .len();
        assert_eq!(after - before, 3);
        let glb = runtime
            .cas_read_bounded(
                first["high_artifact_object_sha256"]
                    .as_str()
                    .expect("High artifact object SHA"),
                96 * 1024 * 1024,
            )
            .expect("High artifact GLB readback");
        assert_eq!(&glb[..4], b"glTF");
        assert_eq!(first["glb_size_bytes"], glb.len() as u64);

        let replay = runtime
            .authoring_mesh_v2_high_artifact_prepare(&request)
            .expect("High artifact exact replay");
        assert_high_artifact_result(&replay, "replayed", false);
        assert_eq!(
            replay["high_artifact_sha256"],
            first["high_artifact_sha256"]
        );
        assert_eq!(replay["store_effect"], "not-touched");
        assert_eq!(replay["cas_effect"], "not-touched");
        assert_eq!(
            runtime
                .store
                .cas()
                .list_objects()
                .expect("CAS after replay")
                .len(),
            after
        );

        let get_request = high_artifact_get_request(&first);
        let found = runtime
            .authoring_mesh_v2_high_artifact_get(&get_request)
            .expect("High artifact exact get");
        assert_high_artifact_result(&found, "found", false);
        assert_eq!(found["idempotency_key"], Value::Null);

        drop(runtime);
        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopen Runtime");
        let restarted = reopened
            .authoring_mesh_v2_high_artifact_get(&get_request)
            .expect("High artifact restart get");
        assert_high_artifact_result(&restarted, "found", false);
        assert_eq!(
            restarted["high_artifact_object_sha256"],
            first["high_artifact_object_sha256"]
        );
        println!(
            "WPN_HIGH_ARTIFACT_LIVE_EVIDENCE={}",
            serde_json::to_string(&json!({
                "schema_version":"WeaponryAuthoringMeshV2HighArtifactLiveEvidence@1",
                "high_artifact_sha256":first["high_artifact_sha256"],
                "high_artifact_object_sha256":first["high_artifact_object_sha256"],
                "high_artifact_readback_sha256":first["high_artifact_readback_sha256"],
                "high_artifact_readback_object_sha256":first["high_artifact_readback_object_sha256"],
                "high_artifact_receipt_sha256":first["high_artifact_receipt_sha256"],
                "high_artifact_receipt_object_sha256":first["high_artifact_receipt_object_sha256"],
                "high_worker_build_cohort_sha256":first["high_worker_build_cohort_sha256"],
                "glb_size_bytes":first["glb_size_bytes"],
                "triangle_count":first["strict_readback"]["triangle_count"],
                "cas_object_delta":after-before,
                "prepare_status":first["status"],
                "replay_status":replay["status"],
                "get_status":found["status"],
                "restart_get_status":restarted["status"],
                "restart_hash_verified":true,
                "high_artifact_status":first["high_artifact_status"],
                "high_status":first["high_status"],
                "quality_status":first["quality_status"],
                "visual_status":first["visual_status"],
                "human_status":first["human_status"],
                "engine_status":first["engine_status"],
                "commercial_quality":"NOT_PROVEN"
            }))
            .expect("serialize bounded High artifact evidence")
        );
        drop(reopened);
        std::fs::remove_dir_all(root).expect("remove High artifact test root");
    }
}
