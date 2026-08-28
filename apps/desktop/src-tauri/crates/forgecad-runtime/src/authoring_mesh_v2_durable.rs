//! Minimal durable bridge for the ForgeCAD-owned `AuthoringMesh@2` kernel.
//!
//! The kernel owns topology and stable identity.  This module only validates
//! the closed prepare/get envelope and delegates persistence to Store/CAS;
//! it never edits SQLite directly and never promotes a candidate/version.

use super::{
    authoring_mesh_v2::{
        AuthoringMeshV2EvaluatedBinding, AuthoringMeshV2GenesisInput, AuthoringMeshV2Revision,
    },
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, now_string, Runtime,
    RuntimeError,
};
use forgecad_contracts::{
    AuthoringMeshEdgeId, AuthoringMeshId, AuthoringMeshLineageId, AuthoringMeshMoveVerticesRequest,
    AuthoringMeshRevision, AuthoringMeshRevisionId, AuthoringMeshSplitEdgeRequest,
    AuthoringMeshTopologyOperationKind, AuthoringMeshVertexId,
    AUTHORING_MESH_V2_DURABLE_GET_REQUEST_SCHEMA_VERSION,
    AUTHORING_MESH_V2_DURABLE_PREPARE_REQUEST_SCHEMA_VERSION,
    AUTHORING_MESH_V2_DURABLE_RESULT_SCHEMA_VERSION,
};
use forgecad_store::{
    AuthoringMeshV2DurableRecord, AUTHORING_MESH_V2_DURABLE_RECORD_SCHEMA_VERSION,
    AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_CAS_JSON_BYTES: u64 = 1024 * 1024;
/// Foundation Parts are imported from an offline source mesh and can exceed
/// the generic durable response budget.  The full revision remains private to
/// Runtime/CAS; callers receive only the compact summary below.
const MAX_FOUNDATION_CAS_JSON_BYTES: u64 = 64 * 1024 * 1024;
const FOUNDATION_SUMMARY_MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const JSON_MIME: &str = "application/json";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const MATERIALIZATION_STATUS: &str = "runtime-owned-store-authoring-mesh-v2-durable-record@1";

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "operation",
    "mesh_id",
    "lineage_id",
    "parent_revision_id",
    "operation_id",
    "edge_id",
    "split_ratio_milli",
    "vertex_ids",
    "delta_m",
    "operation_lineage_sha256",
    "positions_m",
    "faces",
    "evaluated",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

const GET_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "mesh_id",
    "revision_id",
    "revision_sha256",
    "revision_object_sha256",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "AUTHORING_MESH_V2_DURABLE_INVALID: {}",
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
        return Err(invalid(format!(
            "{context} fields differ from the closed contract"
        )));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{field} must be a string")))
}

fn identifier<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not an opaque identifier")));
    }
    Ok(value)
}

fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} is not a SHA-256")));
    }
    Ok(value)
}

fn nullable_text(object: &Map<String, Value>, field: &str) -> Result<Option<String>, RuntimeError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() => Ok(Some(value.clone())),
        _ => Err(invalid(format!("{field} must be a nullable string"))),
    }
}

fn nullable_u64(object: &Map<String, Value>, field: &str) -> Result<Option<u64>, RuntimeError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| invalid(format!("{field} must be a non-negative integer"))),
        _ => Err(invalid(format!("{field} must be a nullable integer"))),
    }
}

fn nullable_vertex_ids(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<Vec<AuthoringMeshVertexId>>, RuntimeError> {
    let Some(value) = object.get(field) else {
        return Err(invalid(format!("{field} is missing")));
    };
    if value.is_null() {
        return Ok(None);
    }
    let values = value
        .as_array()
        .filter(|values| (1..=32).contains(&values.len()))
        .ok_or_else(|| invalid(format!("{field} must be null or an array of 1..32 IDs")))?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let id = value
            .as_str()
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid(format!("{field} contains an invalid vertex ID")))?;
        result.push(AuthoringMeshVertexId(id.to_owned()));
    }
    if result.iter().collect::<BTreeSet<_>>().len() != result.len() {
        return Err(invalid(format!("{field} must contain unique vertex IDs")));
    }
    Ok(Some(result))
}

fn nullable_deltas(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<Vec<[f64; 3]>>, RuntimeError> {
    let Some(value) = object.get(field) else {
        return Err(invalid(format!("{field} is missing")));
    };
    if value.is_null() {
        return Ok(None);
    }
    let parsed = serde_json::from_value::<Vec<[f64; 3]>>(value.clone()).map_err(|error| {
        invalid(format!(
            "{field} must be null or an array of finite vec3 deltas: {error}"
        ))
    })?;
    if !(1..=32).contains(&parsed.len())
        || parsed.iter().any(|delta| {
            delta
                .iter()
                .any(|value| !value.is_finite() || value.abs() > 1.0)
        })
    {
        return Err(invalid(format!(
            "{field} must contain 1..32 finite vec3 deltas inside [-1,1]m"
        )));
    }
    Ok(Some(parsed))
}

fn operation_name(kind: &AuthoringMeshTopologyOperationKind) -> &'static str {
    match kind {
        AuthoringMeshTopologyOperationKind::SplitEdge => "split_edge",
        AuthoringMeshTopologyOperationKind::FaceExtrude => "face_extrude",
        AuthoringMeshTopologyOperationKind::MoveVertices => "move_vertices",
        AuthoringMeshTopologyOperationKind::OpenFrameNotch => "open_frame_notch",
        AuthoringMeshTopologyOperationKind::RearStockVoidRailBow => "rear_stock_void_rail_bow",
        AuthoringMeshTopologyOperationKind::RearStockVoidBoundaryBridge => {
            "rear_stock_void_boundary_bridge"
        }
    }
}

fn bool_const(
    object: &Map<String, Value>,
    field: &str,
    expected: bool,
) -> Result<(), RuntimeError> {
    if object.get(field).and_then(Value::as_bool) != Some(expected) {
        return Err(invalid(format!(
            "{field} differs from the durable contract"
        )));
    }
    Ok(())
}

fn input_hash(value: &Value, object: &Map<String, Value>) -> Result<String, RuntimeError> {
    let input_sha256 = sha(object, "input_sha256")?.to_owned();
    let mut without_hash = value.clone();
    without_hash["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&without_hash) != input_sha256 {
        return Err(invalid("input_sha256 does not match the closed request"));
    }
    Ok(input_sha256)
}

fn check_prepare_policy(object: &Map<String, Value>) -> Result<(), RuntimeError> {
    if text(object, "schema_version")? != AUTHORING_MESH_V2_DURABLE_PREPARE_REQUEST_SCHEMA_VERSION
        || text(object, "writer_policy")? != WRITER_POLICY
        || text(object, "canonicalization_policy")? != CANONICALIZATION_POLICY
    {
        return Err(invalid("prepare request policy differs"));
    }
    bool_const(object, "runtime_write_performed", false)
}

fn check_get_policy(object: &Map<String, Value>) -> Result<(), RuntimeError> {
    if text(object, "schema_version")? != AUTHORING_MESH_V2_DURABLE_GET_REQUEST_SCHEMA_VERSION
        || text(object, "writer_policy")? != WRITER_POLICY
    {
        return Err(invalid("get request policy differs"));
    }
    bool_const(object, "runtime_write_performed", false)?;
    bool_const(object, "persistent_user_data_touched", false)
}

fn max_response_bytes(object: &Map<String, Value>) -> Result<usize, RuntimeError> {
    let value = object
        .get("max_response_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("max_response_bytes must be an integer"))?;
    let value = usize::try_from(value).map_err(|_| invalid("max_response_bytes is too large"))?;
    if !(1..=MAX_RESPONSE_BYTES).contains(&value) {
        return Err(invalid("max_response_bytes is outside the bounded budget"));
    }
    Ok(value)
}

fn evaluated_binding(
    value: &Value,
) -> Result<Option<AuthoringMeshV2EvaluatedBinding>, RuntimeError> {
    let Some(object) = value.as_object() else {
        if value.is_null() {
            return Ok(None);
        }
        return Err(invalid("evaluated must be null or an object"));
    };
    let fields = [
        "artifact_id",
        "artifact_sha256",
        "readback_sha256",
        "correspondence_status",
    ];
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err(invalid("evaluated fields differ from the closed contract"));
    }
    let artifact_id = object
        .get("artifact_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("evaluated.artifact_id is invalid"))?;
    let artifact_sha256 = object
        .get("artifact_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("evaluated.artifact_sha256 is invalid"))?;
    let readback_sha256 = object
        .get("readback_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("evaluated.readback_sha256 is invalid"))?;
    let correspondence_status = object
        .get("correspondence_status")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128)
        .ok_or_else(|| invalid("evaluated.correspondence_status is invalid"))?;
    Ok(Some(AuthoringMeshV2EvaluatedBinding {
        artifact_id: artifact_id.to_owned(),
        artifact_sha256: artifact_sha256.to_owned(),
        readback_sha256: readback_sha256.to_owned(),
        correspondence_status: correspondence_status.to_owned(),
    }))
}

pub(crate) fn durable_record_for(
    project_id: &str,
    revision: &AuthoringMeshRevision,
    revision_object_sha256: &str,
    request_input_sha256: &str,
    idempotency_key: &str,
) -> Result<AuthoringMeshV2DurableRecord, RuntimeError> {
    let operation = revision.operation.as_ref();
    let mut record = AuthoringMeshV2DurableRecord {
        schema_version: AUTHORING_MESH_V2_DURABLE_RECORD_SCHEMA_VERSION.to_owned(),
        project_id: project_id.to_owned(),
        mesh_id: revision.mesh_id.0.clone(),
        lineage_id: revision.lineage_id.0.clone(),
        revision_id: revision.revision_id.0.clone(),
        parent_revision_ids: revision
            .parent_revision_ids
            .iter()
            .map(|id| id.0.clone())
            .collect(),
        revision_index: revision.revision_index,
        revision_object_sha256: revision_object_sha256.to_owned(),
        revision_sha256: revision.canonical_sha256.clone(),
        operation_id: operation.map(|value| value.operation_id.clone()),
        operation_kind: operation.map(|value| operation_name(&value.kind).to_owned()),
        operation_lineage_sha256: operation.map(|value| value.operation_lineage_sha256.clone()),
        request_input_sha256: request_input_sha256.to_owned(),
        idempotency_key: idempotency_key.to_owned(),
        materialization_status: MATERIALIZATION_STATUS.to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    let mut value = serde_json::to_value(&record)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    value["canonical_sha256"] = Value::String(String::new());
    record.canonical_sha256 = canonical_json_hash(&value);
    Ok(record)
}

fn revision_from_cas_with_limit(
    runtime: &Runtime,
    record: &AuthoringMeshV2DurableRecord,
    max_bytes: u64,
) -> Result<AuthoringMeshRevision, RuntimeError> {
    let object = runtime
        .store
        .get_object(&record.revision_object_sha256)?
        .ok_or_else(|| invalid("revision CAS object is absent from the Store"))?;
    if object.kind != AUTHORING_MESH_V2_REVISION_OBJECT_KIND || object.mime != JSON_MIME {
        return Err(invalid("revision CAS object kind or MIME differs"));
    }
    if object.size_bytes > max_bytes {
        return Err(invalid(
            "revision CAS object exceeds its bounded read limit",
        ));
    }
    let bytes = runtime.cas_read_bounded(&record.revision_object_sha256, max_bytes)?;
    let revision: AuthoringMeshRevision = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("revision CAS JSON is invalid: {error}")))?;
    let kernel = AuthoringMeshV2Revision::from_record(revision.clone())?;
    if record.revision_sha256 != kernel.record().canonical_sha256
        || record.mesh_id != kernel.record().mesh_id.0
        || record.lineage_id != kernel.record().lineage_id.0
        || record.revision_id != kernel.record().revision_id.0
    {
        return Err(invalid("durable row and typed revision differ"));
    }
    let revision_value = serde_json::to_value(&revision)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let bytes_again = canonical_json_bytes(&revision_value)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    if bytes_again != bytes {
        return Err(invalid("revision CAS bytes are not canonical"));
    }
    Ok(revision)
}

fn revision_from_cas(
    runtime: &Runtime,
    record: &AuthoringMeshV2DurableRecord,
) -> Result<AuthoringMeshRevision, RuntimeError> {
    revision_from_cas_with_limit(runtime, record, MAX_CAS_JSON_BYTES)
}

fn result_value(
    runtime: &Runtime,
    record: &AuthoringMeshV2DurableRecord,
    revision: &AuthoringMeshRevision,
    request_input_sha256: &str,
    replayed: bool,
    runtime_write_performed: bool,
    max_bytes: usize,
) -> Result<Value, RuntimeError> {
    // from_record is intentionally repeated on the final path so a prepare
    // result is validated exactly like a post-restart get result.
    AuthoringMeshV2Revision::from_record(revision.clone())?;
    let mut result = serde_json::json!({
        "schema_version": AUTHORING_MESH_V2_DURABLE_RESULT_SCHEMA_VERSION,
        "project_id": record.project_id,
        "mesh_id": record.mesh_id,
        "lineage_id": record.lineage_id,
        "revision_id": record.revision_id,
        "revision_index": record.revision_index,
        "parent_revision_ids": record.parent_revision_ids,
        "revision_sha256": record.revision_sha256,
        "revision_object_sha256": record.revision_object_sha256,
        "operation": revision
            .operation
            .as_ref()
            .map(|value| operation_name(&value.kind))
            .unwrap_or("genesis"),
        "revision": revision,
        "durable_record": record,
        "request_input_sha256": request_input_sha256,
        "idempotency_key": record.idempotency_key,
        "replayed": replayed,
        "restart_hash_verified": true,
        "runtime_write_performed": runtime_write_performed,
        "persistent_user_data_touched": runtime_write_performed,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": "structural_only",
        "limitations": [
            "RUNTIME_SOLE_WRITER",
            "NO_STAGE_ADVANCEMENT",
            "NO_CANDIDATE_CONFIRM",
            "NO_VERSION_CREATED",
            "NO_EXPORT",
            "STRUCTURAL_ONLY_NOT_COMMERCIAL_QUALITY"
        ],
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    if canonical_json_bytes(&result)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?
        .len()
        > max_bytes
    {
        return Err(invalid(
            "AuthoringMesh@2 durable result exceeds max_response_bytes",
        ));
    }
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    if canonical_json_bytes(&result)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?
        .len()
        > max_bytes
    {
        return Err(invalid(
            "AuthoringMesh@2 durable result exceeds max_response_bytes",
        ));
    }
    let _ = runtime;
    Ok(result)
}

/// Build the public result for a foundation-derived revision.  Unlike the
/// historical generic durable result, this intentionally never serializes the
/// revision or durable row: a foundation Part can be large, while the public
/// contract is a bounded hash/count/idempotency receipt.
fn foundation_result_summary(
    record: &AuthoringMeshV2DurableRecord,
    revision: &AuthoringMeshRevision,
    request_input_sha256: &str,
    replayed: bool,
    runtime_write_performed: bool,
) -> Result<Value, RuntimeError> {
    AuthoringMeshV2Revision::from_record(revision.clone())?;
    let original = &revision.original;
    let mut result = serde_json::json!({
        "schema_version": "AuthoringMeshV2FoundationGenesisSummary@1",
        "response_shape": "hash-count-idempotency-replay-restart@1",
        "project_id": record.project_id,
        "mesh_id": record.mesh_id,
        "lineage_id": record.lineage_id,
        "revision_id": record.revision_id,
        "revision_index": record.revision_index,
        "revision_sha256": record.revision_sha256,
        "revision_object_sha256": record.revision_object_sha256,
        "request_input_sha256": request_input_sha256,
        "idempotency_key": record.idempotency_key,
        "vertex_count": original.vertices.len(),
        "edge_count": original.edges.len(),
        "half_edge_count": original.half_edges.len(),
        "corner_count": original.corners.len(),
        "face_count": original.faces.len(),
        "loop_count": original.loops.len(),
        "ring_count": original.rings.len(),
        "replayed": replayed,
        "restart_hash_verified": true,
        "runtime_write_performed": runtime_write_performed,
        "persistent_user_data_touched": runtime_write_performed,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": ""
    });
    let size = |value: &Value| {
        canonical_json_bytes(value)
            .map(|bytes| bytes.len())
            .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
    };
    if size(&result)? >= FOUNDATION_SUMMARY_MAX_RESPONSE_BYTES {
        return Err(invalid(
            "foundation genesis summary must remain strictly below 1 MiB",
        ));
    }
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    if size(&result)? >= FOUNDATION_SUMMARY_MAX_RESPONSE_BYTES {
        return Err(invalid(
            "foundation genesis summary must remain strictly below 1 MiB",
        ));
    }
    Ok(result)
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        PREPARE_FIELDS,
        "AuthoringMeshV2DurablePrepareRequest@1",
    )?;
    check_prepare_policy(object)?;
    let request_input_sha256 = input_hash(request, object)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let mesh_id = identifier(object, "mesh_id")?.to_owned();
    let lineage_id = identifier(object, "lineage_id")?.to_owned();
    let idempotency_key = identifier(object, "idempotency_key")?.to_owned();
    let max_bytes = max_response_bytes(object)?;
    let operation = text(object, "operation")?;
    if !matches!(operation, "genesis" | "split_edge" | "move_vertices") {
        return Err(invalid(
            "operation must be genesis, split_edge, or move_vertices",
        ));
    }

    if let Some(existing) = runtime
        .store
        .get_authoring_mesh_v2_durable_record(&project_id, &idempotency_key)?
    {
        if existing.request_input_sha256 != request_input_sha256
            || existing.mesh_id != mesh_id
            || existing.lineage_id != lineage_id
        {
            return Err(invalid(
                "idempotency_key is already bound to a different AuthoringMesh@2 request",
            ));
        }
        let revision = revision_from_cas(runtime, &existing)?;
        return result_value(
            runtime,
            &existing,
            &revision,
            &request_input_sha256,
            true,
            false,
            max_bytes,
        );
    }

    let parent_revision_id = nullable_text(object, "parent_revision_id")?;
    let operation_id = nullable_text(object, "operation_id")?;
    let edge_id = nullable_text(object, "edge_id")?;
    let split_ratio_milli = nullable_u64(object, "split_ratio_milli")?;
    let vertex_ids = nullable_vertex_ids(object, "vertex_ids")?;
    let delta_m = nullable_deltas(object, "delta_m")?;
    let operation_lineage_sha256 = match object.get("operation_lineage_sha256") {
        Some(Value::Null) => None,
        Some(Value::String(value)) if is_sha256(value) => Some(value.clone()),
        _ => return Err(invalid("operation_lineage_sha256 must be null or SHA-256")),
    };
    let revision = match operation {
        "genesis" => {
            if parent_revision_id.is_some()
                || operation_id.is_some()
                || edge_id.is_some()
                || split_ratio_milli.is_some()
                || vertex_ids.is_some()
                || delta_m.is_some()
                || operation_lineage_sha256.is_some()
            {
                return Err(invalid("genesis cannot carry operation fields"));
            }
            let positions: Vec<[f64; 3]> = serde_json::from_value(
                object
                    .get("positions_m")
                    .cloned()
                    .ok_or_else(|| invalid("positions_m is missing"))?,
            )
            .map_err(|error| invalid(format!("positions_m is invalid: {error}")))?;
            let faces: Vec<Vec<usize>> = serde_json::from_value(
                object
                    .get("faces")
                    .cloned()
                    .ok_or_else(|| invalid("faces is missing"))?,
            )
            .map_err(|error| invalid(format!("faces is invalid: {error}")))?;
            let evaluated = evaluated_binding(
                object
                    .get("evaluated")
                    .ok_or_else(|| invalid("evaluated is missing"))?,
            )?;
            AuthoringMeshV2Revision::genesis(AuthoringMeshV2GenesisInput {
                mesh_id: AuthoringMeshId(mesh_id.clone()),
                lineage_id: AuthoringMeshLineageId(lineage_id.clone()),
                positions_m: positions,
                faces,
                evaluated,
                source_binding: None,
                foundation_source_binding: None,
            })?
            .record()
            .clone()
        }
        "split_edge" => {
            if object.get("positions_m") != Some(&Value::Null)
                || object.get("faces") != Some(&Value::Null)
                || object.get("evaluated") != Some(&Value::Null)
                || vertex_ids.is_some()
                || delta_m.is_some()
            {
                return Err(invalid("split_edge must not carry genesis payloads"));
            }
            let parent_revision_id = parent_revision_id
                .ok_or_else(|| invalid("split_edge parent_revision_id is required"))?;
            let operation_id =
                operation_id.ok_or_else(|| invalid("split_edge operation_id is required"))?;
            let edge_id = edge_id.ok_or_else(|| invalid("split_edge edge_id is required"))?;
            let split_ratio_milli = split_ratio_milli
                .ok_or_else(|| invalid("split_edge split_ratio_milli is required"))?;
            let operation_lineage_sha256 = operation_lineage_sha256
                .ok_or_else(|| invalid("split_edge operation_lineage_sha256 is required"))?;
            let parent = runtime
                .store
                .get_authoring_mesh_v2_durable_record_by_revision(&project_id, &parent_revision_id)?
                .ok_or_else(|| invalid("split_edge parent revision is not durable"))?;
            if parent.mesh_id != mesh_id || parent.lineage_id != lineage_id {
                return Err(invalid("split_edge parent is from another mesh lineage"));
            }
            let parent_revision = revision_from_cas(runtime, &parent)?;
            AuthoringMeshV2Revision::from_record(parent_revision.clone())?
                .split_edge(AuthoringMeshSplitEdgeRequest {
                    operation_id,
                    parent_revision_id: AuthoringMeshRevisionId(parent_revision_id),
                    edge_id: AuthoringMeshEdgeId(edge_id),
                    split_ratio_milli: u32::try_from(split_ratio_milli)
                        .map_err(|_| invalid("split_ratio_milli is too large"))?,
                    operation_lineage_sha256,
                })?
                .child_revision
        }
        "move_vertices" => {
            if object.get("positions_m") != Some(&Value::Null)
                || object.get("faces") != Some(&Value::Null)
                || object.get("evaluated") != Some(&Value::Null)
                || edge_id.is_some()
                || split_ratio_milli.is_some()
            {
                return Err(invalid("move_vertices must not carry unrelated payloads"));
            }
            let parent_revision_id = parent_revision_id
                .ok_or_else(|| invalid("move_vertices parent_revision_id is required"))?;
            let operation_id =
                operation_id.ok_or_else(|| invalid("move_vertices operation_id is required"))?;
            let vertex_ids =
                vertex_ids.ok_or_else(|| invalid("move_vertices vertex_ids is required"))?;
            let delta_m = delta_m.ok_or_else(|| invalid("move_vertices delta_m is required"))?;
            let operation_lineage_sha256 = operation_lineage_sha256
                .ok_or_else(|| invalid("move_vertices operation_lineage_sha256 is required"))?;
            let parent = runtime
                .store
                .get_authoring_mesh_v2_durable_record_by_revision(&project_id, &parent_revision_id)?
                .ok_or_else(|| invalid("move_vertices parent revision is not durable"))?;
            if parent.mesh_id != mesh_id || parent.lineage_id != lineage_id {
                return Err(invalid("move_vertices parent is from another mesh lineage"));
            }
            let parent_revision = revision_from_cas(runtime, &parent)?;
            AuthoringMeshV2Revision::from_record(parent_revision.clone())?
                .move_vertices(AuthoringMeshMoveVerticesRequest {
                    operation_id,
                    parent_revision_id: AuthoringMeshRevisionId(parent_revision_id),
                    vertex_ids,
                    delta_m,
                    operation_lineage_sha256,
                })?
                .child_revision
        }
        _ => unreachable!(),
    };
    if revision.mesh_id.0 != mesh_id || revision.lineage_id.0 != lineage_id {
        return Err(invalid("generated revision identity differs from request"));
    }
    let revision_value = serde_json::to_value(&revision)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let revision_bytes = canonical_json_bytes(&revision_value)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let reservation = runtime.store.begin_cas_reservation();
    let revision_object = runtime.store.put_object_reserved(
        &reservation,
        &revision_bytes,
        None,
        JSON_MIME,
        AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
        &now_string(),
    )?;
    let record = durable_record_for(
        &project_id,
        &revision,
        &revision_object.record.sha256,
        &request_input_sha256,
        &idempotency_key,
    )?;
    let persisted = runtime.store.record_authoring_mesh_v2_revision_with_replay(
        &record,
        &revision,
        &revision_object.record,
    );
    let (stored, replayed) = match persisted {
        Ok(value) => {
            runtime
                .store
                .release_cas_reservation_object(&reservation, &revision_object, false)?;
            value
        }
        Err(error) => {
            let _ =
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, &revision_object, true);
            return Err(error.into());
        }
    };
    let persisted_revision = revision_from_cas(runtime, &stored)?;
    result_value(
        runtime,
        &stored,
        &persisted_revision,
        &request_input_sha256,
        replayed,
        true,
        max_bytes,
    )
}

/// Persist a Runtime-derived, source-bound genesis without exposing raw
/// topology or provenance fields through the public generic durable request.
pub(crate) fn persist_runtime_derived_source_genesis(
    runtime: &Runtime,
    project_id: &str,
    request_input_sha256: &str,
    idempotency_key: &str,
    revision: AuthoringMeshRevision,
) -> Result<Value, RuntimeError> {
    if !is_opaque_id(project_id)
        || !is_opaque_id(idempotency_key)
        || !is_sha256(request_input_sha256)
        || revision.revision_index != 0
        || !revision.parent_revision_ids.is_empty()
        || revision.operation.is_some()
        || revision.source_binding.is_none()
    {
        return Err(invalid("Runtime-derived source genesis binding is invalid"));
    }
    AuthoringMeshV2Revision::from_record(revision.clone())?;
    if let Some(existing) = runtime
        .store
        .get_authoring_mesh_v2_durable_record(project_id, idempotency_key)?
    {
        if existing.request_input_sha256 != request_input_sha256
            || existing.mesh_id != revision.mesh_id.0
            || existing.lineage_id != revision.lineage_id.0
            || existing.revision_sha256 != revision.canonical_sha256
        {
            return Err(invalid(
                "source genesis idempotency_key is bound to another revision",
            ));
        }
        let persisted = revision_from_cas(runtime, &existing)?;
        return result_value(
            runtime,
            &existing,
            &persisted,
            request_input_sha256,
            true,
            false,
            MAX_RESPONSE_BYTES,
        );
    }

    let revision_value = serde_json::to_value(&revision)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let revision_bytes = canonical_json_bytes(&revision_value)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let reservation = runtime.store.begin_cas_reservation();
    let revision_object = runtime.store.put_object_reserved(
        &reservation,
        &revision_bytes,
        None,
        JSON_MIME,
        AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
        &now_string(),
    )?;
    let record = durable_record_for(
        project_id,
        &revision,
        &revision_object.record.sha256,
        request_input_sha256,
        idempotency_key,
    )?;
    let persisted = runtime.store.record_authoring_mesh_v2_revision_with_replay(
        &record,
        &revision,
        &revision_object.record,
    );
    let (stored, replayed) = match persisted {
        Ok(value) => {
            runtime
                .store
                .release_cas_reservation_object(&reservation, &revision_object, false)?;
            value
        }
        Err(error) => {
            let _ =
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, &revision_object, true);
            return Err(error.into());
        }
    };
    let persisted_revision = revision_from_cas(runtime, &stored)?;
    result_value(
        runtime,
        &stored,
        &persisted_revision,
        request_input_sha256,
        replayed,
        true,
        MAX_RESPONSE_BYTES,
    )
}

/// Persist a Runtime-derived foundation revision while keeping the full
/// topology private to CAS.  The existing AuthoringMesh durable API above
/// intentionally remains a full-result API; foundation materialization uses a
/// separate hash-only receipt so a large Part cannot leak through MCP.
fn persist_runtime_derived_foundation_revision(
    runtime: &Runtime,
    project_id: &str,
    request_input_sha256: &str,
    idempotency_key: &str,
    revision: AuthoringMeshRevision,
    genesis: bool,
) -> Result<Value, RuntimeError> {
    let shape_valid = if genesis {
        revision.revision_index == 0
            && revision.parent_revision_ids.is_empty()
            && revision.operation.is_none()
    } else {
        revision.revision_index > 0
            && revision.parent_revision_ids.len() == 1
            && revision.operation.is_some()
    };
    let Some(binding) = revision.foundation_source_binding.as_ref() else {
        return Err(invalid("Runtime-derived foundation binding is missing"));
    };
    if !is_opaque_id(project_id)
        || !is_opaque_id(idempotency_key)
        || !is_sha256(request_input_sha256)
        || !shape_valid
        || revision.source_binding.is_some()
        || binding.project_id != project_id
        || binding.authoring_mesh_id != revision.mesh_id.0
        || binding.authoring_mesh_lineage_id != revision.lineage_id.0
    {
        return Err(invalid(
            "Runtime-derived foundation revision binding is invalid",
        ));
    }
    AuthoringMeshV2Revision::from_record(revision.clone())?;

    let parent = if genesis {
        None
    } else {
        let parent_revision_id = &revision.parent_revision_ids[0].0;
        let parent = runtime
            .store
            .get_authoring_mesh_v2_durable_record_by_revision(project_id, parent_revision_id)?
            .ok_or_else(|| invalid("Runtime-derived foundation child parent is not durable"))?;
        if parent.mesh_id != revision.mesh_id.0
            || parent.lineage_id != revision.lineage_id.0
            || parent.revision_index + 1 != revision.revision_index
        {
            return Err(invalid(
                "Runtime-derived foundation child parent lineage/index differs",
            ));
        }
        let parent_revision =
            revision_from_cas_with_limit(runtime, &parent, MAX_FOUNDATION_CAS_JSON_BYTES)?;
        if parent_revision.foundation_source_binding != revision.foundation_source_binding
            || parent_revision.source_binding.is_some()
            || revision.source_binding.is_some()
        {
            return Err(invalid(
                "foundation child must preserve the parent's foundation provenance",
            ));
        }
        Some(parent)
    };
    let _ = parent;

    if let Some(existing) = runtime
        .store
        .get_authoring_mesh_v2_durable_record(project_id, idempotency_key)?
    {
        if existing.request_input_sha256 != request_input_sha256
            || existing.mesh_id != revision.mesh_id.0
            || existing.lineage_id != revision.lineage_id.0
            || existing.revision_sha256 != revision.canonical_sha256
        {
            return Err(invalid(
                "foundation genesis/child idempotency_key is bound to another revision",
            ));
        }
        let persisted =
            revision_from_cas_with_limit(runtime, &existing, MAX_FOUNDATION_CAS_JSON_BYTES)?;
        return foundation_result_summary(&existing, &persisted, request_input_sha256, true, false);
    }

    let revision_value = serde_json::to_value(&revision)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let revision_bytes = canonical_json_bytes(&revision_value)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    if revision_bytes.is_empty() || revision_bytes.len() as u64 > MAX_FOUNDATION_CAS_JSON_BYTES {
        return Err(invalid(
            "foundation revision exceeds the 64 MiB internal CAS limit",
        ));
    }
    let reservation = runtime.store.begin_cas_reservation();
    let revision_object = runtime.store.put_object_reserved(
        &reservation,
        &revision_bytes,
        None,
        JSON_MIME,
        AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
        &now_string(),
    )?;
    let record = durable_record_for(
        project_id,
        &revision,
        &revision_object.record.sha256,
        request_input_sha256,
        idempotency_key,
    )?;
    let persisted = runtime.store.record_authoring_mesh_v2_revision_with_replay(
        &record,
        &revision,
        &revision_object.record,
    );
    let (stored, replayed) = match persisted {
        Ok(value) => {
            runtime
                .store
                .release_cas_reservation_object(&reservation, &revision_object, false)?;
            value
        }
        Err(error) => {
            let _ =
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, &revision_object, true);
            return Err(error.into());
        }
    };
    let persisted_revision =
        revision_from_cas_with_limit(runtime, &stored, MAX_FOUNDATION_CAS_JSON_BYTES)?;
    foundation_result_summary(
        &stored,
        &persisted_revision,
        request_input_sha256,
        replayed,
        true,
    )
}

/// Persist one foundation-derived genesis revision and return only a compact
/// hash/count/idempotency/replay/restart receipt.
pub(crate) fn persist_runtime_derived_foundation_genesis(
    runtime: &Runtime,
    project_id: &str,
    request_input_sha256: &str,
    idempotency_key: &str,
    revision: AuthoringMeshRevision,
) -> Result<Value, RuntimeError> {
    persist_runtime_derived_foundation_revision(
        runtime,
        project_id,
        request_input_sha256,
        idempotency_key,
        revision,
        true,
    )
}

/// Persist a foundation-derived local child revision.  The kernel already
/// copies the binding; this durable boundary additionally compares it with
/// the parent after restart so provenance cannot be silently replaced.
pub(crate) fn persist_runtime_derived_foundation_child(
    runtime: &Runtime,
    project_id: &str,
    request_input_sha256: &str,
    idempotency_key: &str,
    revision: AuthoringMeshRevision,
) -> Result<Value, RuntimeError> {
    persist_runtime_derived_foundation_revision(
        runtime,
        project_id,
        request_input_sha256,
        idempotency_key,
        revision,
        false,
    )
}

/// Persist a Runtime-derived, source-bound child revision produced by a
/// product-owned bounded authoring kernel. This deliberately does not widen
/// the public generic durable request with operation-specific topology input.
pub(crate) fn persist_runtime_derived_source_child(
    runtime: &Runtime,
    project_id: &str,
    request_input_sha256: &str,
    idempotency_key: &str,
    revision: AuthoringMeshRevision,
) -> Result<Value, RuntimeError> {
    if !is_opaque_id(project_id)
        || !is_opaque_id(idempotency_key)
        || !is_sha256(request_input_sha256)
        || revision.revision_index == 0
        || revision.parent_revision_ids.len() != 1
        || revision.operation.is_none()
        || revision.source_binding.is_none()
    {
        return Err(invalid("Runtime-derived source child binding is invalid"));
    }
    AuthoringMeshV2Revision::from_record(revision.clone())?;
    let parent_revision_id = &revision.parent_revision_ids[0].0;
    let parent = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(project_id, parent_revision_id)?
        .ok_or_else(|| invalid("Runtime-derived source child parent is not durable"))?;
    if parent.mesh_id != revision.mesh_id.0
        || parent.lineage_id != revision.lineage_id.0
        || parent.revision_index + 1 != revision.revision_index
    {
        return Err(invalid(
            "Runtime-derived source child parent lineage/index differs",
        ));
    }
    if let Some(existing) = runtime
        .store
        .get_authoring_mesh_v2_durable_record(project_id, idempotency_key)?
    {
        if existing.request_input_sha256 != request_input_sha256
            || existing.mesh_id != revision.mesh_id.0
            || existing.lineage_id != revision.lineage_id.0
            || existing.revision_sha256 != revision.canonical_sha256
        {
            return Err(invalid(
                "source child idempotency_key is bound to another revision",
            ));
        }
        let persisted = revision_from_cas(runtime, &existing)?;
        return result_value(
            runtime,
            &existing,
            &persisted,
            request_input_sha256,
            true,
            false,
            MAX_RESPONSE_BYTES,
        );
    }

    let revision_value = serde_json::to_value(&revision)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let revision_bytes = canonical_json_bytes(&revision_value)
        .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
    let reservation = runtime.store.begin_cas_reservation();
    let revision_object = runtime.store.put_object_reserved(
        &reservation,
        &revision_bytes,
        None,
        JSON_MIME,
        AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
        &now_string(),
    )?;
    let record = durable_record_for(
        project_id,
        &revision,
        &revision_object.record.sha256,
        request_input_sha256,
        idempotency_key,
    )?;
    let persisted = runtime.store.record_authoring_mesh_v2_revision_with_replay(
        &record,
        &revision,
        &revision_object.record,
    );
    let (stored, replayed) = match persisted {
        Ok(value) => {
            runtime
                .store
                .release_cas_reservation_object(&reservation, &revision_object, false)?;
            value
        }
        Err(error) => {
            let _ =
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, &revision_object, true);
            return Err(error.into());
        }
    };
    let persisted_revision = revision_from_cas(runtime, &stored)?;
    result_value(
        runtime,
        &stored,
        &persisted_revision,
        request_input_sha256,
        replayed,
        true,
        MAX_RESPONSE_BYTES,
    )
}

pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_FIELDS, "AuthoringMeshV2DurableGetRequest@1")?;
    check_get_policy(object)?;
    let request_input_sha256 = input_hash(request, object)?;
    let project_id = identifier(object, "project_id")?;
    let mesh_id = identifier(object, "mesh_id")?;
    let revision_id = identifier(object, "revision_id")?;
    let revision_sha256 = sha(object, "revision_sha256")?;
    let revision_object_sha256 = sha(object, "revision_object_sha256")?;
    let record = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(project_id, revision_id)?
        .ok_or_else(|| invalid("AuthoringMesh@2 revision is not durable"))?;
    if record.mesh_id != mesh_id
        || record.revision_id != revision_id
        || record.revision_sha256 != revision_sha256
        || record.revision_object_sha256 != revision_object_sha256
    {
        return Err(invalid("get request does not match the durable revision"));
    }
    let revision = revision_from_cas(runtime, &record)?;
    result_value(
        runtime,
        &record,
        &revision,
        &request_input_sha256,
        true,
        false,
        MAX_RESPONSE_BYTES,
    )
}
