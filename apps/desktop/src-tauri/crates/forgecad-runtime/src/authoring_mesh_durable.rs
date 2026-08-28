//! Runtime-owned durable AuthoringMesh producer/readback.
//!
//! The source of truth for this module is the already validated
//! `authoring_mesh_get` projection. This module only derives and persists
//! three independent public JSON objects: the original/canonical mesh, the
//! evaluated artifact sidecar, and the link joining them. The object SHA is
//! intentionally external to each payload, so no payload has a self-hash
//! fixed point.

use super::{
    authoring_mesh, canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, now_string,
    sha256_hex, Runtime, RuntimeError,
};
use forgecad_contracts::CandidateRecord;
use forgecad_store::{
    AuthoringMeshDurableRecord, AuthoringMeshProjectionIndexRecord,
    AUTHORING_MESH_ARTIFACT_OBJECT_KIND, AUTHORING_MESH_CANONICAL_OBJECT_KIND,
    AUTHORING_MESH_DURABLE_RECORD_SCHEMA_VERSION, AUTHORING_MESH_LINK_OBJECT_KIND,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_CAS_JSON_BYTES: u64 = 1024 * 1024;
const AUTHORING_MESH_POLICY_SHA256: &str =
    "aa72cadabba90ddb43dd0014cfa434ab9b13f4e072b09258072f37334c72e709";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const ORIGINAL_REPRESENTATION: &str = "runtime-owned-original-half-edge@1";
const CANONICAL_STORAGE_POLICY: &str = "runtime-owned-sqlite-cas-canonical-authoring-mesh@1";
const ARTIFACT_KIND: &str = "runtime-owned-authoring-mesh-evaluated-sidecar@1";
const ARTIFACT_STORAGE_POLICY: &str = "runtime-owned-cas-sidecar-no-authoring-source-reversal@1";
const CORRESPONDENCE_POLICY: &str = "non-bijective-derived-only@1";
const LINK_POLICY: &str = "canonical-original-plus-evaluated-sidecar-exact-lineage@1";
const LINK_STATUS: &str = "runtime-owned-durable-authoring-mesh-link@1";
const DURABLE_RECORD_STATUS: &str = "runtime-owned-store-authoring-mesh-durable-record@1";
const IDEMPOTENCY_POLICY: &str = "same-input-hash-replays-without-new-record@1";
const REPLAY_POLICY: &str = "same-candidate-program-artifact-readback-binding-required@1";
const PROJECTION_STATUS: &str = "runtime-owned-store-authoring-mesh-projection-index@1";
const JSON_MIME: &str = "application/json";
const LIMITATIONS: &[&str] = &[
    "RUNTIME_SOLE_WRITER",
    "NO_STAGE_ADVANCEMENT",
    "NO_CANDIDATE_CONFIRM",
    "NO_VERSION_CREATED",
    "NO_EXPORT",
    "EVALUATED_IDENTITY_NON_BIJECTIVE",
    "CROSS_VERSION_STABILITY_NOT_PROVEN",
    "STRUCTURAL_ONLY_NOT_COMMERCIAL_QUALITY",
];

const PREPARE_REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "base_version_id",
    "authoring_node_id",
    "part_id",
    "source_program_object_sha256",
    "source_program_sha256",
    "source_artifact_id",
    "source_artifact_object_sha256",
    "source_artifact_sha256",
    "source_artifact_readback_object_sha256",
    "source_artifact_readback_sha256",
    "source_lineage_sha256",
    "expected_canonical_mesh_sha256",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];
const GET_REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "candidate_id",
    "canonical_mesh_id",
    "canonical_mesh_sha256",
    "artifact_id",
    "artifact_sha256",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];
const CANONICAL_FIELDS: &[&str] = &[
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
const ARTIFACT_FIELDS: &[&str] = &[
    "schema_version",
    "artifact_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "authoring_node_id",
    "part_id",
    "canonical_mesh_id",
    "canonical_mesh_object_sha256",
    "canonical_mesh_sha256",
    "source_program_object_sha256",
    "source_program_sha256",
    "evaluated_artifact_object_sha256",
    "evaluated_artifact_sha256",
    "evaluated_artifact_readback_object_sha256",
    "evaluated_artifact_readback_sha256",
    "correspondence_policy",
    "artifact_kind",
    "storage_policy",
    "writer_policy",
    "replay_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "quality_status",
    "canonicalization_policy",
    "canonical_sha256",
];
const LINK_FIELDS: &[&str] = &[
    "schema_version",
    "link_id",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "canonical_mesh_id",
    "canonical_mesh_object_sha256",
    "canonical_mesh_sha256",
    "artifact_id",
    "artifact_object_sha256",
    "artifact_sha256",
    "artifact_readback_object_sha256",
    "artifact_readback_sha256",
    "source_program_object_sha256",
    "source_program_sha256",
    "link_policy",
    "writer_policy",
    "materialization_status",
    "idempotency_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "quality_status",
    "canonicalization_policy",
    "canonical_sha256",
];
const PREPARE_RESULT_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "base_version_id",
    "canonical_mesh_id",
    "canonical_mesh_object_sha256",
    "canonical_mesh_sha256",
    "canonical_mesh",
    "artifact_id",
    "artifact_object_sha256",
    "artifact_sha256",
    "artifact_readback_object_sha256",
    "artifact_readback_sha256",
    "artifact",
    "link_id",
    "link_object_sha256",
    "durable_link",
    "source_program_object_sha256",
    "source_program_sha256",
    "source_lineage_sha256",
    "request_input_sha256",
    "idempotency_key",
    "replayed",
    "restart_hash_verified",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "quality_status",
    "limitations",
    "canonicalization_policy",
    "canonical_sha256",
];
const GET_RESULT_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "canonical_mesh_id",
    "canonical_mesh_object_sha256",
    "canonical_mesh_sha256",
    "canonical_mesh",
    "artifact_id",
    "artifact_object_sha256",
    "artifact_sha256",
    "artifact_readback_object_sha256",
    "artifact_readback_sha256",
    "artifact",
    "link_id",
    "link_object_sha256",
    "durable_link",
    "source_program_object_sha256",
    "source_program_sha256",
    "source_lineage_sha256",
    "request_input_sha256",
    "replayed",
    "restart_hash_verified",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "stage_advanced",
    "candidate_confirmed",
    "version_created",
    "export_performed",
    "quality_status",
    "limitations",
    "canonicalization_policy",
    "canonical_sha256",
];

#[derive(Clone, Debug)]
struct Binding {
    project_id: String,
    candidate_id: String,
    candidate_state_sha256: String,
    base_version_id: Option<String>,
    authoring_node_id: String,
    part_id: String,
    source_program_object_sha256: String,
    source_program_sha256: String,
    source_artifact_id: String,
    source_artifact_object_sha256: String,
    source_artifact_sha256: String,
    source_artifact_readback_object_sha256: String,
    source_artifact_readback_sha256: String,
    source_lineage_sha256: String,
    operator_catalog_sha256: String,
    readback_config_sha256: String,
}

#[derive(Clone, Debug)]
struct Payloads {
    projection: Value,
    projection_bytes: Vec<u8>,
    projection_object_sha256: String,
    canonical: Value,
    canonical_bytes: Vec<u8>,
    canonical_object_sha256: String,
    artifact: Value,
    artifact_bytes: Vec<u8>,
    artifact_object_sha256: String,
    link: Value,
    link_bytes: Vec<u8>,
    link_object_sha256: String,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "AUTHORING_MESH_DURABLE_INVALID: {}",
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
        return Err(invalid(format!("{context} fields differ")));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{key} must be a string")))
}

fn identifier<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, key)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{key} is not an identifier")));
    }
    Ok(value)
}

fn sha<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, key)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{key} is not a SHA-256")));
    }
    Ok(value)
}

fn nullable_identifier(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, RuntimeError> {
    match object.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_opaque_id(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!("{key} must be a nullable identifier"))),
    }
}

fn bool_const(object: &Map<String, Value>, key: &str, expected: bool) -> Result<(), RuntimeError> {
    if object.get(key).and_then(Value::as_bool) != Some(expected) {
        return Err(invalid(format!("{key} differs from the durable contract")));
    }
    Ok(())
}

fn check_input_hash(value: &Value, object: &Map<String, Value>) -> Result<String, RuntimeError> {
    let input_sha256 = sha(object, "input_sha256")?.to_owned();
    let mut without_hash = value.clone();
    without_hash["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&without_hash) != input_sha256 {
        return Err(invalid("input_sha256 does not match the closed request"));
    }
    Ok(input_sha256)
}

fn check_request_policy(
    object: &Map<String, Value>,
    schema: &str,
    with_canonicalization: bool,
) -> Result<(), RuntimeError> {
    if text(object, "schema_version")? != schema || text(object, "writer_policy")? != WRITER_POLICY
    {
        return Err(invalid("request policy differs"));
    }
    bool_const(object, "runtime_write_performed", false)?;
    if with_canonicalization {
        if text(object, "canonicalization_policy")? != CANONICALIZATION_POLICY {
            return Err(invalid("canonicalization_policy differs"));
        }
        if object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
        {
            return Err(invalid("max_response_bytes differs"));
        }
    } else {
        bool_const(object, "persistent_user_data_touched", false)?;
    }
    Ok(())
}

fn check_idempotency_key(value: &str) -> Result<(), RuntimeError> {
    if value.is_empty()
        || value.len() > 128
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || value
            .bytes()
            .skip(1)
            .any(|byte| !(byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte)))
    {
        return Err(invalid("idempotency_key is malformed"));
    }
    Ok(())
}

fn verify_payload_hash(value: &Value, context: &str) -> Result<String, RuntimeError> {
    let supplied = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{context}.canonical_sha256 is missing")))?;
    if !is_sha256(supplied) {
        return Err(invalid(format!("{context}.canonical_sha256 is invalid")));
    }
    let mut without_hash = value.clone();
    without_hash["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&without_hash) != supplied {
        return Err(invalid(format!(
            "{context}.canonical_sha256 mismatches payload"
        )));
    }
    Ok(supplied.to_owned())
}

fn bytes_for(value: &Value, context: &str) -> Result<Vec<u8>, RuntimeError> {
    let bytes = canonical_json_bytes(value).map_err(|error| invalid(error.to_string()))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_CAS_JSON_BYTES {
        return Err(invalid(format!("{context} exceeds the 1 MiB CAS bound")));
    }
    Ok(bytes)
}

fn prepare_binding(
    runtime: &Runtime,
    object: &Map<String, Value>,
) -> Result<Binding, RuntimeError> {
    let project_id = identifier(object, "project_id")?.to_owned();
    let candidate_id = identifier(object, "source_candidate_id")?.to_owned();
    let candidate_state_sha256 = sha(object, "source_candidate_state_sha256")?.to_owned();
    let base_version_id = nullable_identifier(object, "base_version_id")?;
    let authoring_node_id = identifier(object, "authoring_node_id")?.to_owned();
    let part_id = identifier(object, "part_id")?.to_owned();
    let source_program_object_sha256 = sha(object, "source_program_object_sha256")?.to_owned();
    let source_program_sha256 = sha(object, "source_program_sha256")?.to_owned();
    let source_artifact_id = identifier(object, "source_artifact_id")?.to_owned();
    let source_artifact_object_sha256 = sha(object, "source_artifact_object_sha256")?.to_owned();
    let source_artifact_sha256 = sha(object, "source_artifact_sha256")?.to_owned();
    let source_artifact_readback_object_sha256 =
        sha(object, "source_artifact_readback_object_sha256")?.to_owned();
    let source_artifact_readback_sha256 =
        sha(object, "source_artifact_readback_sha256")?.to_owned();
    let source_lineage_sha256 = sha(object, "source_lineage_sha256")?.to_owned();
    let candidate = runtime
        .candidate(&candidate_id)?
        .ok_or_else(|| invalid("source candidate is unavailable"))?;
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&candidate_id)?
        .ok_or_else(|| invalid("source geometry candidate evidence is unavailable"))?;
    validate_candidate_evidence(
        &candidate,
        &evidence,
        &project_id,
        &candidate_state_sha256,
        base_version_id.as_ref(),
        &source_program_object_sha256,
        &source_program_sha256,
        &source_artifact_id,
        &source_artifact_object_sha256,
        &source_artifact_sha256,
        &source_artifact_readback_object_sha256,
        &source_artifact_readback_sha256,
    )?;
    Ok(Binding {
        project_id,
        candidate_id,
        candidate_state_sha256,
        base_version_id,
        authoring_node_id,
        part_id,
        source_program_object_sha256,
        source_program_sha256,
        source_artifact_id,
        source_artifact_object_sha256,
        source_artifact_sha256,
        source_artifact_readback_object_sha256,
        source_artifact_readback_sha256,
        source_lineage_sha256,
        operator_catalog_sha256: evidence.operator_catalog_sha256,
        readback_config_sha256: evidence.readback_config_sha256,
    })
}

fn validate_candidate_evidence(
    candidate: &CandidateRecord,
    evidence: &forgecad_contracts::GeometryCandidateEvidenceRecord,
    project_id: &str,
    candidate_state_sha256: &str,
    base_version_id: Option<&String>,
    program_object_sha256: &str,
    program_sha256: &str,
    artifact_id: &str,
    artifact_object_sha256: &str,
    artifact_sha256: &str,
    readback_object_sha256: &str,
    readback_sha256: &str,
) -> Result<(), RuntimeError> {
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != candidate_state_sha256
        || candidate.base_version_id.as_ref() != base_version_id
        || candidate.prepared_object_id.as_deref() != Some(artifact_id)
        || candidate.prepared_object_sha256.as_deref() != Some(artifact_object_sha256)
    {
        return Err(invalid(
            "candidate state, artifact id or base version is not bound",
        ));
    }
    // Current GeometryCandidateEvidence@1 has no second independent artifact
    // canonical hash. Its artifact object SHA is therefore the only accepted
    // evaluated artifact SHA for this durable slice; accepting a caller-only
    // alias here would break candidate/readback binding.
    if artifact_sha256 != artifact_object_sha256
        || evidence.project_id != project_id
        || evidence.candidate_id != candidate.candidate_id
        || evidence.geometry_program_object_sha256 != program_object_sha256
        || evidence.geometry_program_sha256 != program_sha256
        || evidence.artifact_object_sha256 != artifact_object_sha256
        || evidence.artifact_readback_object_sha256 != readback_object_sha256
    {
        return Err(invalid(
            "candidate/program/artifact/readback evidence binding differs",
        ));
    }
    if !is_sha256(readback_sha256) {
        return Err(invalid("source_artifact_readback_sha256 is invalid"));
    }
    Ok(())
}

fn old_projection_request(binding: &Binding) -> Value {
    json!({
        "schema_version": "AuthoringMeshRequest@1",
        "project_id": binding.project_id,
        "candidate_id": binding.candidate_id,
        "artifact_id": binding.source_artifact_object_sha256,
        "artifact_readback_sha256": binding.source_artifact_readback_sha256,
        "program_sha256": binding.source_program_sha256,
        "operator_catalog_sha256": binding.operator_catalog_sha256,
        "readback_config_sha256": binding.readback_config_sha256,
        "authoring_node_id": binding.authoring_node_id,
        "part_id": binding.part_id,
        "authoring_mesh_policy_sha256": AUTHORING_MESH_POLICY_SHA256,
        "max_response_bytes": MAX_RESPONSE_BYTES,
    })
}

fn projection_lineage(projection: &Value) -> Result<&Map<String, Value>, RuntimeError> {
    projection
        .get("lineage")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("AuthoringMesh projection lineage is missing"))
}

fn bind_projection_lineage(projection: &Value, binding: &Binding) -> Result<(), RuntimeError> {
    let lineage = projection_lineage(projection)?;
    for (key, expected) in [
        ("project_id", binding.project_id.as_str()),
        ("candidate_id", binding.candidate_id.as_str()),
        (
            "artifact_id",
            binding.source_artifact_object_sha256.as_str(),
        ),
        (
            "artifact_readback_sha256",
            binding.source_artifact_readback_sha256.as_str(),
        ),
        ("program_sha256", binding.source_program_sha256.as_str()),
        (
            "operator_catalog_sha256",
            binding.operator_catalog_sha256.as_str(),
        ),
        (
            "readback_config_sha256",
            binding.readback_config_sha256.as_str(),
        ),
        ("authoring_node_id", binding.authoring_node_id.as_str()),
        ("part_id", binding.part_id.as_str()),
    ] {
        if lineage.get(key).and_then(Value::as_str) != Some(expected) {
            return Err(invalid(format!("projection lineage field {key} differs")));
        }
    }
    let lineage_sha256 = lineage
        .get("lineage_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("projection lineage_sha256 is missing"))?;
    if !is_sha256(lineage_sha256) || lineage_sha256 != binding.source_lineage_sha256 {
        return Err(invalid(
            "source_lineage_sha256 differs from projection lineage",
        ));
    }
    Ok(())
}

fn canonical_from_projection(projection: &Value, binding: &Binding) -> Result<Value, RuntimeError> {
    let projection_object = projection
        .as_object()
        .ok_or_else(|| invalid("AuthoringMesh projection must be an object"))?;
    let projection_schema = projection_object
        .get("schema_version")
        .and_then(Value::as_str);
    if projection_schema != Some("AuthoringMesh@1") {
        return Err(invalid("source projection schema differs"));
    }
    bind_projection_lineage(projection, binding)?;
    let canonical_mesh_id = projection_object
        .get("mesh_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("projection mesh_id is invalid"))?;
    let mesh_sha256 = projection_object
        .get("mesh_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("projection mesh_sha256 is invalid"))?;
    let lineage_sha256 = binding.source_lineage_sha256.as_str();
    let original_id = projection_object
        .get("original_identity")
        .and_then(|value| value.get("identity_id"))
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("projection original identity is invalid"))?;
    let evaluated_id = projection_object
        .get("evaluated_identity")
        .and_then(|value| value.get("identity_id"))
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("projection evaluated identity is invalid"))?;
    let mut canonical = json!({
        "schema_version": "AuthoringMeshCanonical@1",
        "canonical_mesh_id": canonical_mesh_id,
        "project_id": binding.project_id,
        "candidate_id": binding.candidate_id,
        "candidate_state_sha256": binding.candidate_state_sha256,
        "base_version_id": binding.base_version_id,
        "authoring_node_id": binding.authoring_node_id,
        "part_id": binding.part_id,
        "source_program_object_sha256": binding.source_program_object_sha256,
        "source_program_sha256": binding.source_program_sha256,
        "source_artifact_object_sha256": binding.source_artifact_object_sha256,
        "source_artifact_sha256": binding.source_artifact_sha256,
        "source_artifact_readback_object_sha256": binding.source_artifact_readback_object_sha256,
        "source_artifact_readback_sha256": binding.source_artifact_readback_sha256,
        "source_lineage_sha256": lineage_sha256,
        "representation": ORIGINAL_REPRESENTATION,
        "storage_policy": CANONICAL_STORAGE_POLICY,
        "writer_policy": WRITER_POLICY,
        "original_identity": {
            "identity_id": original_id,
            "namespace": "original",
            "identity_kind": "runtime-owned-original-authoring@1",
            "element_id_policy": "lineage-scoped-opaque-not-cross-version-stable@1",
            "topology_sha256": mesh_sha256,
            "source_lineage_sha256": lineage_sha256,
            "stability_scope": "same-canonical-mesh-lineage-only@1"
        },
        "evaluated_identity": {
            "identity_id": evaluated_id,
            "namespace": "evaluated",
            "identity_kind": "runtime-derived-evaluated-artifact-readback@1",
            "element_id_policy": "artifact-local-no-authoring-bijection@1",
            "correspondence_policy": CORRESPONDENCE_POLICY,
            "artifact_object_sha256": binding.source_artifact_object_sha256,
            "artifact_readback_sha256": binding.source_artifact_readback_sha256,
            "source_lineage_sha256": lineage_sha256,
            "cross_version_stable": false
        },
        "cross_version_stable": false,
        "cross_version_stability": {
            "status": "not-proven@1",
            "scope": "same-canonical-mesh-lineage-only@1",
            "stable_id_claim": "none-across-revisions@1",
            "deleted_id_reuse_policy": "not-proven-and-not-a-contract@1",
            "new_id_policy": "lineage-operation-parent-derived-draft-only@1",
            "evaluated_id_policy": "artifact-local-unstable-derived-only@1"
        },
        "counts": projection_object.get("counts").cloned().ok_or_else(|| invalid("projection counts missing"))?,
        "vertices": projection_object.get("vertices").cloned().ok_or_else(|| invalid("projection vertices missing"))?,
        "edges": projection_object.get("edges").cloned().ok_or_else(|| invalid("projection edges missing"))?,
        "half_edges": projection_object.get("half_edges").cloned().ok_or_else(|| invalid("projection half_edges missing"))?,
        "corners": projection_object.get("corners").cloned().ok_or_else(|| invalid("projection corners missing"))?,
        "faces": projection_object.get("faces").cloned().ok_or_else(|| invalid("projection faces missing"))?,
        "loops": projection_object.get("loops").cloned().ok_or_else(|| invalid("projection loops missing"))?,
        "rings": projection_object.get("rings").cloned().ok_or_else(|| invalid("projection rings missing"))?,
        "topology": projection_object.get("topology").cloned().ok_or_else(|| invalid("projection topology missing"))?,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "runtime_write_performed": true,
        "persistent_user_data_touched": true,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": "structural_only",
        "canonical_sha256": ""
    });
    exact_object(&canonical, CANONICAL_FIELDS, "AuthoringMeshCanonical@1")?;
    canonical["canonical_sha256"] = Value::String(canonical_json_hash(&canonical));
    verify_payload_hash(&canonical, "AuthoringMeshCanonical@1")?;
    Ok(canonical)
}

fn durable_artifact_id(
    canonical_mesh_id: &str,
    canonical_mesh_sha256: &str,
    source_lineage_sha256: &str,
) -> String {
    let seed = canonical_json_hash(&json!({
        "schema_version": "AuthoringMeshArtifact@1",
        "canonical_mesh_id": canonical_mesh_id,
        "canonical_mesh_sha256": canonical_mesh_sha256,
        "source_lineage_sha256": source_lineage_sha256,
        "artifact_kind": ARTIFACT_KIND,
    }));
    format!("authoring-artifact-{}", &seed[..32])
}

fn artifact_from_canonical(
    canonical: &Value,
    binding: &Binding,
    canonical_object_sha256: &str,
) -> Result<Value, RuntimeError> {
    let canonical_mesh_id = canonical
        .get("canonical_mesh_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("canonical_mesh_id is missing"))?;
    let canonical_mesh_sha256 = canonical
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("canonical mesh hash is missing"))?;
    let artifact_id = durable_artifact_id(
        canonical_mesh_id,
        canonical_mesh_sha256,
        &binding.source_lineage_sha256,
    );
    let mut artifact = json!({
        "schema_version": "AuthoringMeshArtifact@1",
        "artifact_id": artifact_id,
        "project_id": binding.project_id,
        "candidate_id": binding.candidate_id,
        "candidate_state_sha256": binding.candidate_state_sha256,
        "base_version_id": binding.base_version_id,
        "authoring_node_id": binding.authoring_node_id,
        "part_id": binding.part_id,
        "canonical_mesh_id": canonical_mesh_id,
        "canonical_mesh_object_sha256": canonical_object_sha256,
        "canonical_mesh_sha256": canonical_mesh_sha256,
        "source_program_object_sha256": binding.source_program_object_sha256,
        "source_program_sha256": binding.source_program_sha256,
        "evaluated_artifact_object_sha256": binding.source_artifact_object_sha256,
        "evaluated_artifact_sha256": binding.source_artifact_sha256,
        "evaluated_artifact_readback_object_sha256": binding.source_artifact_readback_object_sha256,
        "evaluated_artifact_readback_sha256": binding.source_artifact_readback_sha256,
        "correspondence_policy": CORRESPONDENCE_POLICY,
        "artifact_kind": ARTIFACT_KIND,
        "storage_policy": ARTIFACT_STORAGE_POLICY,
        "writer_policy": WRITER_POLICY,
        "replay_policy": REPLAY_POLICY,
        "runtime_write_performed": true,
        "persistent_user_data_touched": true,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": "structural_only",
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    exact_object(&artifact, ARTIFACT_FIELDS, "AuthoringMeshArtifact@1")?;
    artifact["canonical_sha256"] = Value::String(canonical_json_hash(&artifact));
    verify_payload_hash(&artifact, "AuthoringMeshArtifact@1")?;
    Ok(artifact)
}

fn link_from_payloads(
    canonical: &Value,
    artifact: &Value,
    binding: &Binding,
    canonical_object_sha256: &str,
    artifact_object_sha256: &str,
) -> Result<Value, RuntimeError> {
    let canonical_mesh_id = canonical
        .get("canonical_mesh_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("canonical_mesh_id is missing"))?;
    let canonical_mesh_sha256 = canonical
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("canonical mesh hash is missing"))?;
    let artifact_id = artifact
        .get("artifact_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("durable artifact id is missing"))?;
    let artifact_sha256 = artifact
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("durable artifact hash is missing"))?;
    let link_seed = json!({
        "schema_version": "AuthoringMeshLink@1",
        "project_id": binding.project_id,
        "candidate_id": binding.candidate_id,
        "candidate_state_sha256": binding.candidate_state_sha256,
        "base_version_id": binding.base_version_id,
        "canonical_mesh_id": canonical_mesh_id,
        "canonical_mesh_sha256": canonical_mesh_sha256,
        "artifact_id": artifact_id,
        "artifact_object_sha256": artifact_object_sha256,
        "artifact_sha256": artifact_sha256,
        "source_lineage_sha256": binding.source_lineage_sha256,
        "authoring_node_id": binding.authoring_node_id,
        "part_id": binding.part_id,
    });
    let link_id = format!("link-{}", &canonical_json_hash(&link_seed)[..32]);
    let mut link = json!({
        "schema_version": "AuthoringMeshLink@1",
        "link_id": link_id,
        "project_id": binding.project_id,
        "candidate_id": binding.candidate_id,
        "candidate_state_sha256": binding.candidate_state_sha256,
        "base_version_id": binding.base_version_id,
        "canonical_mesh_id": canonical_mesh_id,
        "canonical_mesh_object_sha256": canonical_object_sha256,
        "canonical_mesh_sha256": canonical_mesh_sha256,
        "artifact_id": artifact_id,
        "artifact_object_sha256": artifact_object_sha256,
        "artifact_sha256": artifact_sha256,
        "artifact_readback_object_sha256": binding.source_artifact_readback_object_sha256,
        "artifact_readback_sha256": binding.source_artifact_readback_sha256,
        "source_program_object_sha256": binding.source_program_object_sha256,
        "source_program_sha256": binding.source_program_sha256,
        "link_policy": LINK_POLICY,
        "writer_policy": WRITER_POLICY,
        "materialization_status": LINK_STATUS,
        "idempotency_policy": IDEMPOTENCY_POLICY,
        "runtime_write_performed": true,
        "persistent_user_data_touched": true,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": "structural_only",
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    exact_object(&link, LINK_FIELDS, "AuthoringMeshLink@1")?;
    link["canonical_sha256"] = Value::String(canonical_json_hash(&link));
    verify_payload_hash(&link, "AuthoringMeshLink@1")?;
    Ok(link)
}

fn build_payloads(projection: Value, binding: &Binding) -> Result<Payloads, RuntimeError> {
    bind_projection_lineage(&projection, binding)?;
    let projection_bytes = bytes_for(&projection, "AuthoringMesh projection")?;
    let projection_object_sha256 = sha256_hex(&projection_bytes);
    let canonical = canonical_from_projection(&projection, binding)?;
    let canonical_bytes = bytes_for(&canonical, "AuthoringMeshCanonical@1")?;
    let canonical_object_sha256 = sha256_hex(&canonical_bytes);
    let artifact = artifact_from_canonical(&canonical, binding, &canonical_object_sha256)?;
    let artifact_bytes = bytes_for(&artifact, "AuthoringMeshArtifact@1")?;
    let artifact_object_sha256 = sha256_hex(&artifact_bytes);
    let link = link_from_payloads(
        &canonical,
        &artifact,
        binding,
        &canonical_object_sha256,
        &artifact_object_sha256,
    )?;
    let link_bytes = bytes_for(&link, "AuthoringMeshLink@1")?;
    let link_object_sha256 = sha256_hex(&link_bytes);
    Ok(Payloads {
        projection,
        projection_bytes,
        projection_object_sha256,
        canonical,
        canonical_bytes,
        canonical_object_sha256,
        artifact,
        artifact_bytes,
        artifact_object_sha256,
        link,
        link_bytes,
        link_object_sha256,
    })
}

fn read_json_object(
    runtime: &Runtime,
    sha256: &str,
    kind: &str,
) -> Result<(Value, Vec<u8>), RuntimeError> {
    let record = runtime
        .store
        .get_object(sha256)?
        .ok_or_else(|| invalid(format!("CAS object {sha256} is unavailable")))?;
    if record.sha256 != sha256 || record.mime != JSON_MIME || record.kind != kind {
        return Err(invalid(format!(
            "CAS object {sha256} metadata is not {kind}"
        )));
    }
    let bytes = runtime.cas_read_bounded(sha256, MAX_CAS_JSON_BYTES)?;
    if sha256_hex(&bytes) != sha256 {
        return Err(invalid(format!(
            "CAS object {sha256} hash readback differs"
        )));
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
    Ok((value, bytes))
}

fn verify_payload_objects(runtime: &Runtime, payloads: &Payloads) -> Result<(), RuntimeError> {
    for (sha256, kind, bytes, context) in [
        (
            &payloads.canonical_object_sha256,
            AUTHORING_MESH_CANONICAL_OBJECT_KIND,
            &payloads.canonical_bytes,
            "canonical",
        ),
        (
            &payloads.artifact_object_sha256,
            AUTHORING_MESH_ARTIFACT_OBJECT_KIND,
            &payloads.artifact_bytes,
            "artifact",
        ),
        (
            &payloads.link_object_sha256,
            AUTHORING_MESH_LINK_OBJECT_KIND,
            &payloads.link_bytes,
            "link",
        ),
    ] {
        let (value, stored_bytes) = read_json_object(runtime, sha256, kind)?;
        let expected_value = match context {
            "canonical" => payloads.canonical.clone(),
            "artifact" => payloads.artifact.clone(),
            _ => payloads.link.clone(),
        };
        if &stored_bytes != bytes || value != expected_value {
            return Err(invalid(format!("durable {context} CAS readback differs")));
        }
    }
    Ok(())
}

fn projection_index(binding: &Binding, payloads: &Payloads) -> AuthoringMeshProjectionIndexRecord {
    AuthoringMeshProjectionIndexRecord {
        schema_version: "AuthoringMeshProjectionIndex@1".to_owned(),
        mesh_id: payloads
            .canonical
            .get("canonical_mesh_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        project_id: binding.project_id.clone(),
        candidate_id: binding.candidate_id.clone(),
        candidate_state_sha256: binding.candidate_state_sha256.clone(),
        // The Store-local index deliberately uses the evaluated GLB object as
        // artifact_id. Public durable payloads retain source_artifact_id.
        artifact_id: binding.source_artifact_object_sha256.clone(),
        artifact_sha256: binding.source_artifact_object_sha256.clone(),
        artifact_readback_sha256: binding.source_artifact_readback_sha256.clone(),
        program_sha256: binding.source_program_sha256.clone(),
        operator_catalog_sha256: binding.operator_catalog_sha256.clone(),
        readback_config_sha256: binding.readback_config_sha256.clone(),
        authoring_node_id: binding.authoring_node_id.clone(),
        part_id: binding.part_id.clone(),
        mesh_object_sha256: payloads.projection_object_sha256.clone(),
        mesh_sha256: payloads
            .projection
            .get("mesh_sha256")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        materialization_status: PROJECTION_STATUS.to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    }
}

fn put_payloads_and_index(
    runtime: &Runtime,
    binding: &Binding,
    payloads: &Payloads,
    request_input_sha256: &str,
    idempotency_key: &str,
) -> Result<bool, RuntimeError> {
    let reservation = runtime.store.begin_cas_reservation();
    let projection_object = runtime.store.put_object_reserved(
        &reservation,
        &payloads.projection_bytes,
        Some(&payloads.projection_object_sha256),
        JSON_MIME,
        "authoring-mesh",
        &now_string(),
    )?;
    let canonical_object = runtime.store.put_object_reserved(
        &reservation,
        &payloads.canonical_bytes,
        Some(&payloads.canonical_object_sha256),
        JSON_MIME,
        AUTHORING_MESH_CANONICAL_OBJECT_KIND,
        &now_string(),
    )?;
    let artifact_object = runtime.store.put_object_reserved(
        &reservation,
        &payloads.artifact_bytes,
        Some(&payloads.artifact_object_sha256),
        JSON_MIME,
        AUTHORING_MESH_ARTIFACT_OBJECT_KIND,
        &now_string(),
    )?;
    let link_object = runtime.store.put_object_reserved(
        &reservation,
        &payloads.link_bytes,
        Some(&payloads.link_object_sha256),
        JSON_MIME,
        AUTHORING_MESH_LINK_OBJECT_KIND,
        &now_string(),
    )?;
    let (stored_projection, projection_replayed) = runtime
        .store
        .record_authoring_mesh_projection_index_with_replay(
            &projection_index(binding, payloads),
            &projection_object.record,
        )?;
    if stored_projection.mesh_object_sha256 != payloads.projection_object_sha256 {
        return Err(invalid("projection index readback selected another object"));
    }
    let link_id = payloads
        .link
        .get("link_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("durable link id is missing"))?;
    let durable_record = AuthoringMeshDurableRecord {
        schema_version: AUTHORING_MESH_DURABLE_RECORD_SCHEMA_VERSION.to_owned(),
        project_id: binding.project_id.clone(),
        candidate_id: binding.candidate_id.clone(),
        candidate_state_sha256: binding.candidate_state_sha256.clone(),
        base_version_id: binding.base_version_id.clone(),
        canonical_mesh_id: payloads.canonical["canonical_mesh_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        canonical_mesh_object_sha256: payloads.canonical_object_sha256.clone(),
        canonical_mesh_sha256: payloads.canonical["canonical_sha256"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        artifact_id: payloads.artifact["artifact_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        artifact_object_sha256: payloads.artifact_object_sha256.clone(),
        artifact_sha256: payloads.artifact["canonical_sha256"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
        artifact_readback_object_sha256: binding.source_artifact_readback_object_sha256.clone(),
        artifact_readback_sha256: binding.source_artifact_readback_sha256.clone(),
        link_id: link_id.to_owned(),
        link_object_sha256: payloads.link_object_sha256.clone(),
        source_program_object_sha256: binding.source_program_object_sha256.clone(),
        source_program_sha256: binding.source_program_sha256.clone(),
        source_artifact_object_sha256: binding.source_artifact_object_sha256.clone(),
        source_artifact_sha256: binding.source_artifact_sha256.clone(),
        source_artifact_readback_object_sha256: binding
            .source_artifact_readback_object_sha256
            .clone(),
        source_artifact_readback_sha256: binding.source_artifact_readback_sha256.clone(),
        operator_catalog_sha256: binding.operator_catalog_sha256.clone(),
        readback_config_sha256: binding.readback_config_sha256.clone(),
        authoring_node_id: binding.authoring_node_id.clone(),
        part_id: binding.part_id.clone(),
        request_input_sha256: request_input_sha256.to_owned(),
        idempotency_key: idempotency_key.to_owned(),
        materialization_status: DURABLE_RECORD_STATUS.to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    let (stored_durable, durable_replayed) = runtime
        .store
        .record_authoring_mesh_durable_record_with_replay(
            &durable_record,
            &canonical_object.record,
            &artifact_object.record,
            &link_object.record,
        )?;
    if stored_durable.canonical_mesh_object_sha256 != payloads.canonical_object_sha256
        || stored_durable.artifact_object_sha256 != payloads.artifact_object_sha256
        || stored_durable.link_object_sha256 != payloads.link_object_sha256
        || stored_durable.request_input_sha256 != request_input_sha256
        || stored_durable.idempotency_key != idempotency_key
    {
        return Err(invalid("durable Store record readback differs"));
    }
    for object in [
        &projection_object,
        &canonical_object,
        &artifact_object,
        &link_object,
    ] {
        runtime
            .store
            .release_cas_reservation_object(&reservation, object, false)?;
    }
    Ok(durable_replayed || projection_replayed)
}

fn candidate_and_evidence(
    runtime: &Runtime,
    binding: &Binding,
) -> Result<
    (
        CandidateRecord,
        forgecad_contracts::GeometryCandidateEvidenceRecord,
    ),
    RuntimeError,
> {
    let candidate = runtime
        .candidate(&binding.candidate_id)?
        .ok_or_else(|| invalid("candidate is unavailable"))?;
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&binding.candidate_id)?
        .ok_or_else(|| invalid("geometry candidate evidence is unavailable"))?;
    if candidate.project_id != binding.project_id
        || candidate.canonical_sha256 != binding.candidate_state_sha256
        || candidate.base_version_id != binding.base_version_id
        || candidate.prepared_object_id.as_deref() != Some(binding.source_artifact_id.as_str())
        || candidate.prepared_object_sha256.as_deref()
            != Some(binding.source_artifact_object_sha256.as_str())
        || evidence.geometry_program_object_sha256 != binding.source_program_object_sha256
        || evidence.geometry_program_sha256 != binding.source_program_sha256
        || evidence.artifact_object_sha256 != binding.source_artifact_object_sha256
        || evidence.artifact_readback_object_sha256
            != binding.source_artifact_readback_object_sha256
    {
        return Err(invalid(
            "candidate/program/artifact/readback binding differs on replay",
        ));
    }
    Ok((candidate, evidence))
}

fn build_from_projection(
    runtime: &Runtime,
    binding: &Binding,
    projection: Value,
) -> Result<Payloads, RuntimeError> {
    let payloads = build_payloads(projection, binding)?;
    // Ensure the source evidence remains available and candidate-bound after a
    // Store/CAS readback, not merely during the initial projection call.
    let _ = candidate_and_evidence(runtime, binding)?;
    Ok(payloads)
}

fn load_projection_index(
    runtime: &Runtime,
    binding: &Binding,
    canonical_mesh_id: &str,
) -> Result<AuthoringMeshProjectionIndexRecord, RuntimeError> {
    let index = runtime
        .store
        .get_authoring_mesh_projection_index(&binding.candidate_id, canonical_mesh_id)?
        .ok_or_else(|| invalid("durable AuthoringMesh projection index is unavailable"))?;
    if index.project_id != binding.project_id
        || index.candidate_state_sha256 != binding.candidate_state_sha256
        || index.artifact_sha256 != binding.source_artifact_object_sha256
        || index.artifact_id != binding.source_artifact_object_sha256
        || index.artifact_readback_sha256 != binding.source_artifact_readback_sha256
        || index.program_sha256 != binding.source_program_sha256
        || index.operator_catalog_sha256 != binding.operator_catalog_sha256
        || index.readback_config_sha256 != binding.readback_config_sha256
        || index.authoring_node_id != binding.authoring_node_id
        || index.part_id != binding.part_id
    {
        return Err(invalid("durable projection index binding differs"));
    }
    Ok(index)
}

fn read_projection_for_index(
    runtime: &Runtime,
    index: &AuthoringMeshProjectionIndexRecord,
) -> Result<Value, RuntimeError> {
    let (projection, bytes) =
        read_json_object(runtime, &index.mesh_object_sha256, "authoring-mesh")?;
    if sha256_hex(&bytes) != index.mesh_object_sha256
        || projection.get("mesh_id").and_then(Value::as_str) != Some(index.mesh_id.as_str())
        || projection.get("mesh_sha256").and_then(Value::as_str) != Some(index.mesh_sha256.as_str())
    {
        return Err(invalid("durable projection index object readback differs"));
    }
    Ok(projection)
}

fn verify_durable_record(
    record: &AuthoringMeshDurableRecord,
    binding: &Binding,
    payloads: &Payloads,
) -> Result<(), RuntimeError> {
    let link_id = payloads
        .link
        .get("link_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("durable link id is missing"))?;
    let artifact_id = payloads
        .artifact
        .get("artifact_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("durable artifact id is missing"))?;
    let artifact_sha256 = payloads
        .artifact
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("durable artifact hash is missing"))?;
    if record.schema_version != AUTHORING_MESH_DURABLE_RECORD_SCHEMA_VERSION
        || record.project_id != binding.project_id
        || record.candidate_id != binding.candidate_id
        || record.candidate_state_sha256 != binding.candidate_state_sha256
        || record.base_version_id != binding.base_version_id
        || record.canonical_mesh_id
            != payloads.canonical["canonical_mesh_id"]
                .as_str()
                .unwrap_or_default()
        || record.canonical_mesh_object_sha256 != payloads.canonical_object_sha256
        || record.canonical_mesh_sha256
            != payloads.canonical["canonical_sha256"]
                .as_str()
                .unwrap_or_default()
        || record.artifact_id != artifact_id
        || record.artifact_object_sha256 != payloads.artifact_object_sha256
        || record.artifact_sha256 != artifact_sha256
        || record.artifact_readback_object_sha256 != binding.source_artifact_readback_object_sha256
        || record.artifact_readback_sha256 != binding.source_artifact_readback_sha256
        || record.link_id != link_id
        || record.link_object_sha256 != payloads.link_object_sha256
        || record.source_program_object_sha256 != binding.source_program_object_sha256
        || record.source_program_sha256 != binding.source_program_sha256
        || record.source_artifact_object_sha256 != binding.source_artifact_object_sha256
        || record.source_artifact_sha256 != binding.source_artifact_sha256
        || record.source_artifact_readback_object_sha256
            != binding.source_artifact_readback_object_sha256
        || record.source_artifact_readback_sha256 != binding.source_artifact_readback_sha256
        || record.operator_catalog_sha256 != binding.operator_catalog_sha256
        || record.readback_config_sha256 != binding.readback_config_sha256
        || record.authoring_node_id != binding.authoring_node_id
        || record.part_id != binding.part_id
        || record.materialization_status != DURABLE_RECORD_STATUS
    {
        return Err(invalid("Store durable record differs from public payloads"));
    }
    Ok(())
}

fn limitations_value() -> Value {
    Value::Array(
        LIMITATIONS
            .iter()
            .map(|value| Value::String((*value).to_owned()))
            .collect(),
    )
}

fn prepare_result(
    binding: &Binding,
    payloads: &Payloads,
    request_input_sha256: &str,
    idempotency_key: &str,
    replayed: bool,
) -> Result<Value, RuntimeError> {
    let canonical_mesh_sha256 = payloads
        .canonical
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("canonical mesh hash is missing from result"))?;
    let link_id = payloads
        .link
        .get("link_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("link id is missing from result"))?;
    let mut result = json!({
        "schema_version": "AuthoringMeshPrepareResult@1",
        "project_id": binding.project_id,
        "source_candidate_id": binding.candidate_id,
        "source_candidate_state_sha256": binding.candidate_state_sha256,
        "base_version_id": binding.base_version_id,
        "canonical_mesh_id": payloads.canonical["canonical_mesh_id"].clone(),
        "canonical_mesh_object_sha256": payloads.canonical_object_sha256,
        "canonical_mesh_sha256": canonical_mesh_sha256,
        "canonical_mesh": payloads.canonical,
        "artifact_id": payloads.artifact["artifact_id"].clone(),
        "artifact_object_sha256": payloads.artifact_object_sha256,
        "artifact_sha256": payloads.artifact["canonical_sha256"].clone(),
        "artifact_readback_object_sha256": binding.source_artifact_readback_object_sha256,
        "artifact_readback_sha256": binding.source_artifact_readback_sha256,
        "artifact": payloads.artifact,
        "link_id": link_id,
        "link_object_sha256": payloads.link_object_sha256,
        "durable_link": payloads.link,
        "source_program_object_sha256": binding.source_program_object_sha256,
        "source_program_sha256": binding.source_program_sha256,
        "source_lineage_sha256": binding.source_lineage_sha256,
        "request_input_sha256": request_input_sha256,
        "idempotency_key": idempotency_key,
        "replayed": replayed,
        "restart_hash_verified": true,
        "runtime_write_performed": true,
        "persistent_user_data_touched": true,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": "structural_only",
        "limitations": limitations_value(),
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    exact_object(
        &result,
        PREPARE_RESULT_FIELDS,
        "AuthoringMeshPrepareResult@1",
    )?;
    if bytes_for(&result, "AuthoringMeshPrepareResult@1")?.len() > MAX_RESPONSE_BYTES {
        return Err(invalid("prepare result exceeds max_response_bytes"));
    }
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    verify_payload_hash(&result, "AuthoringMeshPrepareResult@1")?;
    if bytes_for(&result, "AuthoringMeshPrepareResult@1")?.len() > MAX_RESPONSE_BYTES {
        return Err(invalid("prepare result exceeds max_response_bytes"));
    }
    Ok(result)
}

fn get_result(
    binding: &Binding,
    payloads: &Payloads,
    request_input_sha256: &str,
) -> Result<Value, RuntimeError> {
    let canonical_mesh_sha256 = payloads
        .canonical
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("canonical mesh hash is missing from result"))?;
    let link_id = payloads
        .link
        .get("link_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("link id is missing from result"))?;
    let mut result = json!({
        "schema_version": "AuthoringMeshGetResult@1",
        "project_id": binding.project_id,
        "candidate_id": binding.candidate_id,
        "candidate_state_sha256": binding.candidate_state_sha256,
        "base_version_id": binding.base_version_id,
        "canonical_mesh_id": payloads.canonical["canonical_mesh_id"].clone(),
        "canonical_mesh_object_sha256": payloads.canonical_object_sha256,
        "canonical_mesh_sha256": canonical_mesh_sha256,
        "canonical_mesh": payloads.canonical,
        "artifact_id": payloads.artifact["artifact_id"].clone(),
        "artifact_object_sha256": payloads.artifact_object_sha256,
        "artifact_sha256": payloads.artifact["canonical_sha256"].clone(),
        "artifact_readback_object_sha256": binding.source_artifact_readback_object_sha256,
        "artifact_readback_sha256": binding.source_artifact_readback_sha256,
        "artifact": payloads.artifact,
        "link_id": link_id,
        "link_object_sha256": payloads.link_object_sha256,
        "durable_link": payloads.link,
        "source_program_object_sha256": binding.source_program_object_sha256,
        "source_program_sha256": binding.source_program_sha256,
        "source_lineage_sha256": binding.source_lineage_sha256,
        "request_input_sha256": request_input_sha256,
        "replayed": true,
        "restart_hash_verified": true,
        "runtime_write_performed": false,
        "persistent_user_data_touched": false,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": "structural_only",
        "limitations": limitations_value(),
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    exact_object(&result, GET_RESULT_FIELDS, "AuthoringMeshGetResult@1")?;
    if bytes_for(&result, "AuthoringMeshGetResult@1")?.len() > MAX_RESPONSE_BYTES {
        return Err(invalid("get result exceeds max_response_bytes"));
    }
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    verify_payload_hash(&result, "AuthoringMeshGetResult@1")?;
    if bytes_for(&result, "AuthoringMeshGetResult@1")?.len() > MAX_RESPONSE_BYTES {
        return Err(invalid("get result exceeds max_response_bytes"));
    }
    Ok(result)
}

pub(super) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        PREPARE_REQUEST_FIELDS,
        "AuthoringMeshPrepareRequest@1",
    )?;
    check_request_policy(object, "AuthoringMeshPrepareRequest@1", true)?;
    let request_input_sha256 = check_input_hash(request, object)?;
    let idempotency_key = text(object, "idempotency_key")?.to_owned();
    check_idempotency_key(&idempotency_key)?;
    let binding = prepare_binding(runtime, object)?;
    let expected_canonical_mesh_sha256 = sha(object, "expected_canonical_mesh_sha256")?;
    let projection = authoring_mesh::get(runtime, &old_projection_request(&binding))?;
    bind_projection_lineage(&projection, &binding)?;
    let canonical_mesh_id = projection
        .get("mesh_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("source projection mesh_id is missing"))?
        .to_owned();
    let existing = runtime
        .store
        .get_authoring_mesh_projection_index(&binding.candidate_id, &canonical_mesh_id)?;
    let (payloads, replayed) = if existing.is_some() {
        let index = existing.expect("checked above");
        let stored_projection = read_projection_for_index(runtime, &index)?;
        let payloads = build_from_projection(runtime, &binding, stored_projection)?;
        let replayed = put_payloads_and_index(
            runtime,
            &binding,
            &payloads,
            &request_input_sha256,
            &idempotency_key,
        )?;
        (payloads, replayed)
    } else {
        let payloads = build_from_projection(runtime, &binding, projection)?;
        if payloads
            .canonical
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(expected_canonical_mesh_sha256)
        {
            return Err(invalid(
                "expected_canonical_mesh_sha256 differs from derived canonical mesh",
            ));
        }
        let replayed = put_payloads_and_index(
            runtime,
            &binding,
            &payloads,
            &request_input_sha256,
            &idempotency_key,
        )?;
        let index = load_projection_index(runtime, &binding, &canonical_mesh_id)?;
        let stored_projection = read_projection_for_index(runtime, &index)?;
        let persisted = build_from_projection(runtime, &binding, stored_projection)?;
        (persisted, replayed)
    };
    if payloads
        .canonical
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(expected_canonical_mesh_sha256)
    {
        return Err(invalid(
            "expected_canonical_mesh_sha256 differs from persisted canonical mesh",
        ));
    }
    verify_payload_objects(runtime, &payloads)?;
    prepare_result(
        &binding,
        &payloads,
        &request_input_sha256,
        &idempotency_key,
        replayed,
    )
}

fn binding_for_get(
    runtime: &Runtime,
    object: &Map<String, Value>,
) -> Result<(Binding, AuthoringMeshDurableRecord, Value), RuntimeError> {
    let project_id = identifier(object, "project_id")?.to_owned();
    let candidate_id = identifier(object, "candidate_id")?.to_owned();
    let canonical_mesh_id = identifier(object, "canonical_mesh_id")?.to_owned();
    let canonical_mesh_sha256 = sha(object, "canonical_mesh_sha256")?.to_owned();
    let requested_artifact_id = identifier(object, "artifact_id")?.to_owned();
    let requested_artifact_sha256 = sha(object, "artifact_sha256")?.to_owned();
    let candidate = runtime
        .candidate(&candidate_id)?
        .ok_or_else(|| invalid("candidate is unavailable"))?;
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&candidate_id)?
        .ok_or_else(|| invalid("geometry candidate evidence is unavailable"))?;
    if candidate.project_id != project_id
        || evidence.project_id != project_id
        || evidence.candidate_id != candidate_id
    {
        return Err(invalid("get candidate/artifact binding differs"));
    }
    let durable_record = runtime
        .store
        .get_authoring_mesh_durable_record_by_mesh(&candidate_id, &canonical_mesh_id)?
        .ok_or_else(|| invalid("durable AuthoringMesh record is unavailable"))?;
    if durable_record.project_id != project_id
        || durable_record.candidate_id != candidate_id
        || candidate.canonical_sha256 != durable_record.candidate_state_sha256
        || candidate.base_version_id != durable_record.base_version_id
        || candidate.prepared_object_sha256.as_deref()
            != Some(durable_record.source_artifact_object_sha256.as_str())
        || evidence.artifact_object_sha256 != durable_record.source_artifact_object_sha256
        || durable_record.source_artifact_sha256 != durable_record.source_artifact_object_sha256
        || durable_record.canonical_mesh_id != canonical_mesh_id
    {
        return Err(invalid("get durable record source binding differs"));
    }
    let (canonical_value, _) = read_json_object(
        runtime,
        &durable_record.canonical_mesh_object_sha256,
        AUTHORING_MESH_CANONICAL_OBJECT_KIND,
    )?;
    let source_lineage_sha256 = canonical_value
        .get("source_lineage_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("durable canonical source lineage is invalid"))?
        .to_owned();
    let index = runtime
        .store
        .get_authoring_mesh_projection_index(&candidate_id, &canonical_mesh_id)?
        .ok_or_else(|| invalid("durable AuthoringMesh projection index is unavailable"))?;
    let projection = read_projection_for_index(runtime, &index)?;
    let projection_lineage_sha256 = projection_lineage(&projection)?
        .get("lineage_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("durable projection lineage is invalid"))?
        .to_owned();
    if projection_lineage_sha256 != source_lineage_sha256 {
        return Err(invalid(
            "durable canonical/projection source lineage differs",
        ));
    }
    let binding = Binding {
        project_id,
        candidate_id,
        candidate_state_sha256: durable_record.candidate_state_sha256.clone(),
        base_version_id: durable_record.base_version_id.clone(),
        authoring_node_id: durable_record.authoring_node_id.clone(),
        part_id: durable_record.part_id.clone(),
        source_program_object_sha256: durable_record.source_program_object_sha256.clone(),
        source_program_sha256: durable_record.source_program_sha256.clone(),
        source_artifact_id: candidate
            .prepared_object_id
            .clone()
            .ok_or_else(|| invalid("candidate source artifact id is missing"))?,
        source_artifact_object_sha256: durable_record.source_artifact_object_sha256.clone(),
        source_artifact_sha256: durable_record.source_artifact_sha256.clone(),
        source_artifact_readback_object_sha256: durable_record
            .source_artifact_readback_object_sha256
            .clone(),
        source_artifact_readback_sha256: durable_record.source_artifact_readback_sha256.clone(),
        source_lineage_sha256,
        operator_catalog_sha256: durable_record.operator_catalog_sha256.clone(),
        readback_config_sha256: durable_record.readback_config_sha256.clone(),
    };
    bind_projection_lineage(&projection, &binding)?;
    if canonical_mesh_sha256.is_empty()
        || requested_artifact_id.is_empty()
        || requested_artifact_sha256.is_empty()
    {
        return Err(invalid("canonical_mesh_sha256 is empty"));
    }
    if durable_record.artifact_id != requested_artifact_id
        || durable_record.artifact_sha256 != requested_artifact_sha256
    {
        return Err(invalid("get request does not match Store durable sidecar"));
    }
    Ok((binding, durable_record, projection))
}

pub(super) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_REQUEST_FIELDS, "AuthoringMeshGetRequest@1")?;
    check_request_policy(object, "AuthoringMeshGetRequest@1", false)?;
    let request_input_sha256 = check_input_hash(request, object)?;
    let (binding, durable_record, projection) = binding_for_get(runtime, object)?;
    let payloads = build_from_projection(runtime, &binding, projection)?;
    let requested_artifact_id = object
        .get("artifact_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("artifact_id is missing"))?;
    let requested_artifact_sha256 = object
        .get("artifact_sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("artifact_sha256 is missing"))?;
    if payloads
        .canonical
        .get("canonical_mesh_id")
        .and_then(Value::as_str)
        != Some(durable_record.canonical_mesh_id.as_str())
        || payloads
            .canonical
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != object.get("canonical_mesh_sha256").and_then(Value::as_str)
        || payloads.artifact.get("artifact_id").and_then(Value::as_str)
            != Some(requested_artifact_id)
        || payloads
            .artifact
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(requested_artifact_sha256)
    {
        return Err(invalid(
            "get canonical/artifact hash does not match durable payload",
        ));
    }
    verify_durable_record(&durable_record, &binding, &payloads)?;
    verify_payload_objects(runtime, &payloads)?;
    get_result(&binding, &payloads, &request_input_sha256)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    struct RestartFixture {
        root: std::path::PathBuf,
        project_id: String,
        candidate_id: String,
        canonical_mesh_id: String,
        canonical_mesh_sha256: String,
        artifact_id: String,
        artifact_sha256: String,
        link_id: String,
        source_lineage_sha256: String,
        prepare: Value,
        record: forgecad_store::AuthoringMeshDurableRecord,
        public_objects: Vec<Value>,
        object_hashes: Vec<String>,
        object_bytes: Vec<Vec<u8>>,
        projection_object_sha256: String,
    }

    fn authoring_program(project_id: &str) -> Value {
        let mut program = serde_json::json!({
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
                "node_id":"restart-authored-panel",
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
                "part_id":"restart-authored-panel",
                "input_node_ids":["restart-authored-panel"],
                "material_zone_id":"zone-authored-shell",
                "solid":false
            }]
        });
        let hash = crate::hash_geometry_program_with_runtime_worker(&program)
            .expect("GeometryProgram hash");
        program["canonical_sha256"] = hash["canonical_sha256"].clone();
        program
    }

    fn public_object(
        runtime: &Runtime,
        object_sha256: &str,
        kind: &str,
        expected: &Value,
    ) -> Vec<u8> {
        let record = runtime
            .store
            .get_object(object_sha256)
            .expect("CAS metadata")
            .expect("CAS object");
        assert_eq!(record.sha256, object_sha256);
        assert_eq!(record.mime, "application/json");
        assert_eq!(record.kind, kind);
        let bytes = runtime.cas_read(object_sha256).expect("CAS bytes");
        assert_eq!(crate::sha256_hex(&bytes), object_sha256);
        let value: Value = serde_json::from_slice(&bytes).expect("public JSON");
        assert_eq!(&value, expected);
        let payload_hash = value["canonical_sha256"]
            .as_str()
            .expect("public payload canonical hash");
        let mut without_hash = value.clone();
        without_hash["canonical_sha256"] = Value::String(String::new());
        assert_eq!(payload_hash, crate::canonical_json_hash(&without_hash));
        bytes
    }

    #[test]
    fn authoring_mesh_durable_prepare_get_survives_runtime_restart_with_public_object_hashes() {
        let root = std::env::temp_dir().join(format!(
            "forgecad-authoring-mesh-durable-restart-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("restart root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");

        let fixture = {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("initial Runtime");
            let project = runtime
                .create_project(
                    "AuthoringMesh durable restart",
                    serde_json::json!({"profile":"test"}),
                )
                .expect("project");
            let prepared = runtime
                .prepare_geometry_candidate(
                    &project.project_id,
                    None,
                    serde_json::json!({
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
            let mut binding = Binding {
                project_id: project.project_id.clone(),
                candidate_id: candidate_id.clone(),
                candidate_state_sha256: candidate.canonical_sha256.clone(),
                base_version_id: candidate.base_version_id.clone(),
                authoring_node_id: "restart-authored-panel".to_owned(),
                part_id: "restart-authored-panel".to_owned(),
                source_program_object_sha256: evidence.geometry_program_object_sha256.clone(),
                source_program_sha256: evidence.geometry_program_sha256.clone(),
                source_artifact_id,
                source_artifact_object_sha256: source_artifact_object_sha256.clone(),
                source_artifact_sha256: source_artifact_object_sha256.clone(),
                source_artifact_readback_object_sha256: evidence
                    .artifact_readback_object_sha256
                    .clone(),
                source_artifact_readback_sha256,
                source_lineage_sha256: "0".repeat(64),
                operator_catalog_sha256: evidence.operator_catalog_sha256.clone(),
                readback_config_sha256: evidence.readback_config_sha256.clone(),
            };
            let projection = authoring_mesh::get(&runtime, &old_projection_request(&binding))
                .expect("source AuthoringMesh projection");
            binding.source_lineage_sha256 = projection["lineage"]["lineage_sha256"]
                .as_str()
                .expect("source projection lineage SHA")
                .to_owned();
            let expected_payloads =
                build_payloads(projection.clone(), &binding).expect("expected public payloads");
            let expected_canonical_mesh_sha256 = expected_payloads.canonical["canonical_sha256"]
                .as_str()
                .expect("expected canonical mesh SHA")
                .to_owned();
            let mut request = serde_json::json!({
                "schema_version":"AuthoringMeshPrepareRequest@1",
                "project_id":project.project_id,
                "source_candidate_id":candidate_id,
                "source_candidate_state_sha256":candidate.canonical_sha256,
                "base_version_id":candidate.base_version_id,
                "authoring_node_id":"restart-authored-panel",
                "part_id":"restart-authored-panel",
                "source_program_object_sha256":evidence.geometry_program_object_sha256,
                "source_program_sha256":evidence.geometry_program_sha256,
                "source_artifact_id":candidate.prepared_object_id,
                "source_artifact_object_sha256":source_artifact_object_sha256,
                "source_artifact_sha256":source_artifact_object_sha256,
                "source_artifact_readback_object_sha256":evidence.artifact_readback_object_sha256,
                "source_artifact_readback_sha256":binding.source_artifact_readback_sha256,
                "source_lineage_sha256":binding.source_lineage_sha256,
                "expected_canonical_mesh_sha256":expected_canonical_mesh_sha256,
                "idempotency_key":"authoring-mesh-restart-once",
                "max_response_bytes":1048576,
                "runtime_write_performed":false,
                "writer_policy":WRITER_POLICY,
                "canonicalization_policy":CANONICALIZATION_POLICY,
                "input_sha256":""
            });
            request["input_sha256"] = Value::String(crate::canonical_json_hash(&request));
            let first = runtime
                .authoring_mesh_durable_prepare(&request)
                .expect("durable AuthoringMesh prepare");
            assert_eq!(first["replayed"], false);
            assert_eq!(first["restart_hash_verified"], true);
            assert_eq!(first["canonical_mesh"], expected_payloads.canonical);
            assert_eq!(first["artifact"], expected_payloads.artifact);
            assert_eq!(first["durable_link"], expected_payloads.link);
            assert_ne!(
                first["artifact_id"].as_str(),
                candidate.prepared_object_id.as_deref()
            );
            assert_eq!(first["stage_advanced"], false);
            assert_eq!(first["candidate_confirmed"], false);
            assert_eq!(first["version_created"], false);
            assert_eq!(first["export_performed"], false);
            assert!(runtime
                .versions(Some(&project.project_id))
                .expect("initial versions")
                .is_empty());

            let record = runtime
                .store
                .get_authoring_mesh_durable_record_by_mesh(
                    &candidate_id,
                    first["canonical_mesh_id"].as_str().expect("mesh id"),
                )
                .expect("durable record by mesh")
                .expect("durable record");
            assert_eq!(
                first["canonical_mesh_object_sha256"].as_str(),
                Some(record.canonical_mesh_object_sha256.as_str())
            );
            assert_eq!(
                first["artifact_object_sha256"].as_str(),
                Some(record.artifact_object_sha256.as_str())
            );
            assert_eq!(
                first["link_object_sha256"].as_str(),
                Some(record.link_object_sha256.as_str())
            );
            assert_eq!(
                runtime
                    .store
                    .get_authoring_mesh_durable_record_by_link_id(&record.link_id)
                    .expect("durable record by link")
                    .expect("durable link record"),
                record
            );
            let object_hashes = vec![
                record.canonical_mesh_object_sha256.clone(),
                record.artifact_object_sha256.clone(),
                record.link_object_sha256.clone(),
            ];
            let public_objects = vec![
                first["canonical_mesh"].clone(),
                first["artifact"].clone(),
                first["durable_link"].clone(),
            ];
            let object_kinds = [
                AUTHORING_MESH_CANONICAL_OBJECT_KIND,
                AUTHORING_MESH_ARTIFACT_OBJECT_KIND,
                AUTHORING_MESH_LINK_OBJECT_KIND,
            ];
            let object_bytes = object_hashes
                .iter()
                .zip(&public_objects)
                .zip(object_kinds)
                .map(|((hash, value), kind)| public_object(&runtime, hash, kind, value))
                .collect::<Vec<_>>();
            let projection_index = runtime
                .store
                .get_authoring_mesh_projection_index(
                    &candidate_id,
                    first["canonical_mesh_id"].as_str().expect("mesh id"),
                )
                .expect("projection index")
                .expect("projection index row");
            let projection_bytes = runtime
                .cas_read(&projection_index.mesh_object_sha256)
                .expect("projection CAS bytes");
            let projection_readback: Value =
                serde_json::from_slice(&projection_bytes).expect("projection JSON");
            assert_eq!(
                crate::sha256_hex(&projection_bytes),
                projection_index.mesh_object_sha256
            );
            assert_eq!(
                projection_readback["lineage"]["lineage_sha256"],
                Value::String(binding.source_lineage_sha256.clone())
            );

            let fixture = RestartFixture {
                root,
                project_id: project.project_id,
                candidate_id,
                canonical_mesh_id: record.canonical_mesh_id.clone(),
                canonical_mesh_sha256: record.canonical_mesh_sha256.clone(),
                artifact_id: record.artifact_id.clone(),
                artifact_sha256: record.artifact_sha256.clone(),
                link_id: record.link_id.clone(),
                source_lineage_sha256: binding.source_lineage_sha256,
                prepare: first,
                record,
                public_objects,
                object_hashes,
                object_bytes,
                projection_object_sha256: projection_index.mesh_object_sha256,
            };
            drop(runtime);
            fixture
        };

        let reopened = Runtime::open_with_cas(
            fixture.root.join("runtime.sqlite"),
            fixture.root.join("cas"),
        )
        .expect("reopened Runtime");
        let record_by_mesh = reopened
            .store
            .get_authoring_mesh_durable_record_by_mesh(
                &fixture.candidate_id,
                &fixture.canonical_mesh_id,
            )
            .expect("reopened durable record by mesh")
            .expect("reopened durable record");
        assert_eq!(record_by_mesh, fixture.record);
        let record_by_link = reopened
            .store
            .get_authoring_mesh_durable_record_by_link_id(&fixture.link_id)
            .expect("reopened durable record by link")
            .expect("reopened durable link record");
        assert_eq!(record_by_link, fixture.record);

        let mut get_request = serde_json::json!({
            "schema_version":"AuthoringMeshGetRequest@1",
            "project_id":fixture.project_id,
            "candidate_id":fixture.candidate_id,
            "canonical_mesh_id":fixture.canonical_mesh_id,
            "canonical_mesh_sha256":fixture.canonical_mesh_sha256,
            "artifact_id":fixture.artifact_id,
            "artifact_sha256":fixture.artifact_sha256,
            "writer_policy":WRITER_POLICY,
            "runtime_write_performed":false,
            "persistent_user_data_touched":false,
            "input_sha256":""
        });
        get_request["input_sha256"] = Value::String(crate::canonical_json_hash(&get_request));
        let get = reopened
            .authoring_mesh_durable_get(&get_request)
            .expect("durable AuthoringMesh get after restart");
        assert_eq!(get["replayed"], true);
        assert_eq!(get["restart_hash_verified"], true);
        assert_eq!(get["canonical_mesh"], fixture.public_objects[0]);
        assert_eq!(get["artifact"], fixture.public_objects[1]);
        assert_eq!(get["durable_link"], fixture.public_objects[2]);
        assert_eq!(
            get["canonical_mesh_object_sha256"].as_str(),
            Some(fixture.record.canonical_mesh_object_sha256.as_str())
        );
        assert_eq!(
            get["artifact_object_sha256"].as_str(),
            Some(fixture.record.artifact_object_sha256.as_str())
        );
        assert_eq!(
            get["link_object_sha256"].as_str(),
            Some(fixture.record.link_object_sha256.as_str())
        );
        assert_eq!(
            get["source_lineage_sha256"].as_str(),
            Some(fixture.source_lineage_sha256.as_str())
        );
        assert_eq!(get["stage_advanced"], false);
        assert_eq!(get["candidate_confirmed"], false);
        assert_eq!(get["version_created"], false);
        assert_eq!(get["export_performed"], false);

        let object_kinds = [
            AUTHORING_MESH_CANONICAL_OBJECT_KIND,
            AUTHORING_MESH_ARTIFACT_OBJECT_KIND,
            AUTHORING_MESH_LINK_OBJECT_KIND,
        ];
        for (((hash, expected_bytes), expected_value), kind) in fixture
            .object_hashes
            .iter()
            .zip(&fixture.object_bytes)
            .zip(&fixture.public_objects)
            .zip(object_kinds)
        {
            let bytes = public_object(&reopened, hash, kind, expected_value);
            assert_eq!(&bytes, expected_bytes);
        }
        let projection_index = reopened
            .store
            .get_authoring_mesh_projection_index(&fixture.candidate_id, &fixture.canonical_mesh_id)
            .expect("reopened projection index")
            .expect("reopened projection row");
        assert_eq!(
            projection_index.mesh_object_sha256,
            fixture.projection_object_sha256
        );
        let projection_bytes = reopened
            .cas_read(&projection_index.mesh_object_sha256)
            .expect("reopened projection bytes");
        let projection: Value = serde_json::from_slice(&projection_bytes).expect("projection");
        assert_eq!(
            projection["lineage"]["lineage_sha256"],
            Value::String(fixture.source_lineage_sha256.clone())
        );
        assert_eq!(
            crate::sha256_hex(&projection_bytes),
            projection_index.mesh_object_sha256
        );

        let candidate = reopened
            .candidate(&fixture.candidate_id)
            .expect("reopened candidate")
            .expect("candidate");
        let evidence = reopened
            .store
            .get_geometry_candidate_evidence(&fixture.candidate_id)
            .expect("reopened evidence")
            .expect("evidence");
        assert_eq!(candidate.state, "reviewable");
        assert_eq!(
            candidate.canonical_sha256,
            fixture.prepare["source_candidate_state_sha256"]
                .as_str()
                .expect("candidate state SHA")
        );
        assert_eq!(
            evidence.geometry_program_object_sha256,
            fixture.record.source_program_object_sha256
        );
        assert_eq!(
            evidence.geometry_program_sha256,
            fixture.record.source_program_sha256
        );
        assert_eq!(
            evidence.artifact_object_sha256,
            fixture.record.source_artifact_object_sha256
        );
        assert_eq!(
            evidence.artifact_readback_object_sha256,
            fixture.record.source_artifact_readback_object_sha256
        );
        assert!(reopened
            .versions(Some(&fixture.project_id))
            .expect("reopened versions")
            .is_empty());
        drop(reopened);
        fs::remove_dir_all(&fixture.root).expect("restart fixture cleanup");
    }
}
