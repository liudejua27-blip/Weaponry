//! Closed Runtime bridge for the bounded AuthoringMesh@2 command journal.
//!
//! This module is intentionally independent from the older one-operation
//! durable bridge.  It parses the closed transaction envelope, validates all
//! command references and canonical hashes, runs the pure kernel on a clone,
//! stages every derived revision in CAS, and asks Store to commit the complete
//! chain atomically.  A caller never supplies a revision/hash proof: all
//! revision IDs and CAS hashes below are derived from Runtime output.

use super::{
    authoring_mesh_v2::{
        AuthoringMeshV2Revision, AuthoringMeshV2Transaction, AuthoringMeshV2TransactionCommand,
        AuthoringMeshV2TransactionRef,
    },
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, now_string, Runtime,
    RuntimeError,
};
use forgecad_contracts::{
    AuthoringMeshElementKind, AuthoringMeshElementRef, AuthoringMeshRevision,
    AuthoringMeshTopologyOperationKind,
};
use forgecad_store::{
    AuthoringMeshV2DurableRecord, AuthoringMeshV2TransactionCommit,
    AuthoringMeshV2TransactionDurableRecord, AuthoringMeshV2TransactionPayload,
    AuthoringMeshV2TransactionRevisionInput, CasObject,
    AUTHORING_MESH_V2_DURABLE_RECORD_SCHEMA_VERSION, AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
    AUTHORING_MESH_V2_TRANSACTION_OBJECT_KIND,
    AUTHORING_MESH_V2_TRANSACTION_PAYLOAD_SCHEMA_VERSION,
    AUTHORING_MESH_V2_TRANSACTION_RECORD_SCHEMA_VERSION, AUTHORING_MESH_V2_TRANSACTION_STATUS,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const PREPARE_SCHEMA_VERSION: &str = "AuthoringMeshTransactionPrepareRequest@1";
const GET_SCHEMA_VERSION: &str = "AuthoringMeshTransactionGetRequest@1";
const RESULT_SCHEMA_VERSION: &str = "AuthoringMeshTransactionResult@1";
const MAX_COMMANDS: usize = 32;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_TRANSACTION_BYTES: u64 = 8 * 1024 * 1024;
const JSON_MIME: &str = "application/json";
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";

const PREPARE_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "transaction",
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
    "transaction_id",
    "transaction_sha256",
    "transaction_object_sha256",
    "max_response_bytes",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("AUTHORING_TRANSACTION_INVALID: {}", message.into()))
}

fn contract_error(code: &str, message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("{code}: {}", message.into()))
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
    if actual != expected {
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

fn id<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_opaque_id(value) {
        return Err(invalid(format!("{field} is not an opaque identifier")));
    }
    Ok(value)
}

fn hash<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} is not a SHA-256")));
    }
    Ok(value)
}

fn bool_const(
    object: &Map<String, Value>,
    field: &str,
    expected: bool,
) -> Result<(), RuntimeError> {
    if object.get(field).and_then(Value::as_bool) != Some(expected) {
        return Err(invalid(format!("{field} differs from the closed contract")));
    }
    Ok(())
}

fn max_response_bytes(object: &Map<String, Value>) -> Result<usize, RuntimeError> {
    let bytes = object
        .get("max_response_bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("max_response_bytes must be an integer"))?;
    let bytes = usize::try_from(bytes).map_err(|_| invalid("max_response_bytes is too large"))?;
    if !(1..=MAX_RESPONSE_BYTES).contains(&bytes) {
        return Err(invalid("max_response_bytes is outside the bounded budget"));
    }
    Ok(bytes)
}

fn request_input_hash(value: &Value, object: &Map<String, Value>) -> Result<String, RuntimeError> {
    let supplied = hash(object, "input_sha256")?.to_owned();
    let mut without_hash = value.clone();
    without_hash["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&without_hash) != supplied {
        return Err(invalid("input_sha256 does not match the closed request"));
    }
    Ok(supplied)
}

fn parse_u64(value: &Value, field: &str) -> Result<u64, RuntimeError> {
    value
        .as_u64()
        .ok_or_else(|| invalid(format!("{field} must be a non-negative integer")))
}

fn parse_kind(value: &Value, field: &str) -> Result<AuthoringMeshElementKind, RuntimeError> {
    match value.as_str() {
        Some("vertex") => Ok(AuthoringMeshElementKind::Vertex),
        Some("edge") => Ok(AuthoringMeshElementKind::Edge),
        Some("half_edge") => Ok(AuthoringMeshElementKind::HalfEdge),
        Some("corner") => Ok(AuthoringMeshElementKind::Corner),
        Some("face") => Ok(AuthoringMeshElementKind::Face),
        Some("loop") => Ok(AuthoringMeshElementKind::Loop),
        Some("ring") => Ok(AuthoringMeshElementKind::Ring),
        _ => Err(invalid(format!("{field} is not a closed element kind"))),
    }
}

fn parse_element_ref(value: &Value) -> Result<AuthoringMeshV2TransactionRef, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("element reference must be an object"))?;
    let kind = parse_kind(
        object
            .get("kind")
            .ok_or_else(|| invalid("element reference kind is missing"))?,
        "element reference kind",
    )?;
    let has_id = object.contains_key("id");
    let has_generated = object.contains_key("command_index") || object.contains_key("output_index");
    if has_id == has_generated {
        return Err(invalid(
            "element reference must be exactly stable {kind,id} or generated {kind,command_index,output_index}",
        ));
    }
    if has_id {
        if object.len() != 2 {
            return Err(invalid("stable element reference has unknown fields"));
        }
        let element_id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("stable element reference id is invalid"))?;
        return Ok(AuthoringMeshV2TransactionRef::Stable(
            AuthoringMeshElementRef {
                kind,
                id: element_id.to_owned(),
            },
        ));
    }
    if object.len() != 3 {
        return Err(invalid("generated element reference has unknown fields"));
    }
    let command_index = parse_u64(
        object
            .get("command_index")
            .ok_or_else(|| invalid("generated command_index is missing"))?,
        "generated command_index",
    )?;
    let output_index = parse_u64(
        object
            .get("output_index")
            .ok_or_else(|| invalid("generated output_index is missing"))?,
        "generated output_index",
    )?;
    if command_index >= MAX_COMMANDS as u64 || output_index >= 131_072 {
        return Err(invalid("generated element reference exceeds its bound"));
    }
    Ok(AuthoringMeshV2TransactionRef::Generated {
        command_index: command_index as usize,
        kind,
        output_index: output_index as usize,
    })
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

fn parse_command(
    value: &Value,
    expected_index: usize,
) -> Result<AuthoringMeshV2TransactionCommand, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("transaction command must be an object"))?;
    let command_index = parse_u64(
        object
            .get("command_index")
            .ok_or_else(|| invalid("command_index is missing"))?,
        "command_index",
    )?;
    if command_index != expected_index as u64 {
        return Err(invalid(
            "command_index must be contiguous and equal to array index",
        ));
    }
    let operation = object
        .get("operation")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("operation is missing"))?;
    let operation_id = object
        .get("operation_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("operation_id is invalid"))?
        .to_owned();
    let lineage = object
        .get("operation_lineage_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("operation_lineage_sha256 is invalid"))?
        .to_owned();
    match operation {
        "split_edge" => {
            if object.len() != 6 {
                return Err(invalid("split_edge command fields are not closed"));
            }
            let edge = parse_element_ref(
                object
                    .get("edge")
                    .ok_or_else(|| invalid("split_edge edge is missing"))?,
            )?;
            let ratio = parse_u64(
                object
                    .get("split_ratio_milli")
                    .ok_or_else(|| invalid("split_ratio_milli is missing"))?,
                "split_ratio_milli",
            )?;
            if !(1..=999).contains(&ratio) {
                return Err(invalid("split_ratio_milli is outside [1,999]"));
            }
            Ok(AuthoringMeshV2TransactionCommand::SplitEdge {
                operation_id,
                edge,
                split_ratio_milli: ratio as u32,
                operation_lineage_sha256: lineage,
            })
        }
        "move_vertices" => {
            if object.len() != 6 {
                return Err(invalid("move_vertices command fields are not closed"));
            }
            let values = object
                .get("vertices")
                .and_then(Value::as_array)
                .filter(|values| (1..=32).contains(&values.len()))
                .ok_or_else(|| invalid("vertices must contain 1..32 references"))?;
            let vertices = values
                .iter()
                .map(parse_element_ref)
                .collect::<Result<Vec<_>, _>>()?;
            let deltas = serde_json::from_value::<Vec<[f64; 3]>>(
                object
                    .get("delta_m")
                    .ok_or_else(|| invalid("delta_m is missing"))?
                    .clone(),
            )
            .map_err(|error| invalid(format!("delta_m is invalid: {error}")))?;
            if deltas.len() != vertices.len()
                || deltas.iter().any(|delta| {
                    delta
                        .iter()
                        .any(|value| !value.is_finite() || value.abs() > 1.0)
                })
            {
                return Err(invalid(
                    "delta_m must parallel vertices and stay inside [-1,1]m",
                ));
            }
            Ok(AuthoringMeshV2TransactionCommand::MoveVertices {
                operation_id,
                vertices,
                delta_m: deltas,
                operation_lineage_sha256: lineage,
            })
        }
        "face_extrude" => {
            if object.len() != 6 {
                return Err(invalid("face_extrude command fields are not closed"));
            }
            let face = parse_element_ref(
                object
                    .get("face")
                    .ok_or_else(|| invalid("face_extrude face is missing"))?,
            )?;
            let distance = object
                .get("distance_m")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite() && value.abs() >= 1.0e-7 && value.abs() <= 10.0)
                .ok_or_else(|| invalid("distance_m is outside the finite bounded range"))?;
            Ok(AuthoringMeshV2TransactionCommand::FaceExtrude {
                operation_id,
                face,
                distance_m: distance,
                operation_lineage_sha256: lineage,
            })
        }
        _ => Err(invalid(
            "operation is not available in AuthoringMeshTransaction@1",
        )),
    }
}

fn parse_transaction(
    value: &Value,
) -> Result<
    (
        String,
        String,
        String,
        String,
        u64,
        String,
        Vec<AuthoringMeshV2TransactionCommand>,
    ),
    RuntimeError,
> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "transaction_id",
            "mesh_id",
            "lineage_id",
            "base_revision_id",
            "base_revision_index",
            "base_revision_sha256",
            "commands",
            "budgets",
            "execution_policy",
            "canonicalization_policy",
            "canonical_sha256",
        ],
        "transaction",
    )?;
    if text(object, "schema_version")? != AUTHORING_MESH_V2_TRANSACTION_PAYLOAD_SCHEMA_VERSION
        || text(object, "canonicalization_policy")? != CANONICALIZATION_POLICY
    {
        return Err(invalid(
            "transaction schema or canonicalization policy is invalid",
        ));
    }
    let mut canonical_input = value.clone();
    canonical_input["canonical_sha256"] = Value::String(String::new());
    let supplied_canonical = hash(object, "canonical_sha256")?;
    if canonical_json_hash(&canonical_input) != supplied_canonical {
        return Err(invalid(
            "transaction canonical_sha256 is not Runtime-verifiable",
        ));
    }
    let transaction_id = id(object, "transaction_id")?.to_owned();
    let mesh_id = id(object, "mesh_id")?.to_owned();
    let lineage_id = id(object, "lineage_id")?.to_owned();
    let base_revision_id = id(object, "base_revision_id")?.to_owned();
    let base_revision_index = parse_u64(
        object
            .get("base_revision_index")
            .ok_or_else(|| invalid("base_revision_index is missing"))?,
        "base_revision_index",
    )?;
    if base_revision_index > 1_000_000 {
        return Err(invalid("base_revision_index exceeds its bound"));
    }
    let base_revision_sha256 = hash(object, "base_revision_sha256")?.to_owned();

    let budgets = exact_object(
        object
            .get("budgets")
            .ok_or_else(|| invalid("budgets is missing"))?,
        &[
            "max_commands",
            "max_move_vertices_per_command",
            "max_face_degree",
            "max_vertex_delta_m",
            "max_face_extrude_distance_m",
            "overflow_policy",
        ],
        "transaction.budgets",
    )?;
    if budgets.get("max_commands").and_then(Value::as_u64) != Some(32)
        || budgets
            .get("max_move_vertices_per_command")
            .and_then(Value::as_u64)
            != Some(32)
        || budgets.get("max_face_degree").and_then(Value::as_u64) != Some(32)
        || budgets.get("max_vertex_delta_m").and_then(Value::as_f64) != Some(1.0)
        || budgets
            .get("max_face_extrude_distance_m")
            .and_then(Value::as_f64)
            != Some(10.0)
        || budgets.get("overflow_policy").and_then(Value::as_str)
            != Some("reject-entire-transaction@1")
    {
        return Err(invalid("transaction budgets differ from Runtime bounds"));
    }
    let policy = exact_object(
        object
            .get("execution_policy")
            .ok_or_else(|| invalid("execution_policy is missing"))?,
        &[
            "writer_policy",
            "source_of_truth",
            "reference_policy",
            "atomicity_policy",
            "replay_policy",
            "evaluation_policy",
            "identity_policy",
        ],
        "transaction.execution_policy",
    )?;
    let expected_policy = [
        ("writer_policy", WRITER_POLICY),
        ("source_of_truth", "original-authoring-mesh@2"),
        (
            "reference_policy",
            "stable-or-earlier-generated-element-by-kind@1",
        ),
        (
            "atomicity_policy",
            "clone-before-first-command-no-partial-result@1",
        ),
        (
            "replay_policy",
            "same-input-same-base-deterministic-revision-chain@1",
        ),
        (
            "evaluation_policy",
            "authored-edit-invalidates-evaluated-sidecar@2",
        ),
        (
            "identity_policy",
            "runtime-derived-lineage-operation-parent-stable-no-reuse@2",
        ),
    ];
    if expected_policy
        .iter()
        .any(|(field, expected)| policy.get(*field).and_then(Value::as_str) != Some(*expected))
    {
        return Err(invalid(
            "transaction execution policy differs from Runtime policy",
        ));
    }
    let commands = object
        .get("commands")
        .and_then(Value::as_array)
        .filter(|commands| (1..=MAX_COMMANDS).contains(&commands.len()))
        .ok_or_else(|| invalid("commands must contain 1..32 entries"))?
        .iter()
        .enumerate()
        .map(|(index, command)| parse_command(command, index))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((
        transaction_id,
        mesh_id,
        lineage_id,
        base_revision_id,
        base_revision_index,
        base_revision_sha256,
        commands,
    ))
}

fn revision_from_cas(
    runtime: &Runtime,
    record: &AuthoringMeshV2DurableRecord,
) -> Result<AuthoringMeshRevision, RuntimeError> {
    let object = runtime
        .store
        .get_object(&record.revision_object_sha256)?
        .ok_or_else(|| invalid("base revision CAS object is unavailable"))?;
    if object.kind != AUTHORING_MESH_V2_REVISION_OBJECT_KIND || object.mime != JSON_MIME {
        return Err(invalid("base revision CAS metadata is invalid"));
    }
    let bytes = runtime.cas_read_bounded(
        &record.revision_object_sha256,
        MAX_TRANSACTION_BYTES.min(64 * 1024 * 1024),
    )?;
    let revision: AuthoringMeshRevision = serde_json::from_slice(&bytes)
        .map_err(|error| invalid(format!("base revision CAS JSON is invalid: {error}")))?;
    AuthoringMeshV2Revision::from_record(revision.clone())?;
    let canonical = canonical_json_bytes(
        &serde_json::to_value(&revision)
            .map_err(|error| invalid(format!("base revision serialization failed: {error}")))?,
    )
    .map_err(|error| invalid(format!("base revision canonicalization failed: {error}")))?;
    if canonical != bytes
        || record.revision_sha256 != revision.canonical_sha256
        || record.mesh_id != revision.mesh_id.0
        || record.lineage_id != revision.lineage_id.0
        || record.revision_id != revision.revision_id.0
        || record.revision_index != revision.revision_index
    {
        return Err(invalid("base revision durable row and CAS payload differ"));
    }
    Ok(revision)
}

fn result_value(
    record: &AuthoringMeshV2TransactionDurableRecord,
    revisions: &[AuthoringMeshV2TransactionRevisionInput],
    request_kind: &str,
    replayed: bool,
    runtime_write_performed: bool,
    idempotency_key: Option<&str>,
    max_bytes: usize,
) -> Result<Value, RuntimeError> {
    let revision_chain = revisions
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let operation = input
                .revision
                .operation
                .as_ref()
                .map(|operation| operation_name(&operation.kind))
                .unwrap_or("genesis");
            json!({
                "command_index": index,
                "operation_id": input.record.operation_id,
                "operation": operation,
                "parent_revision_id": input.revision.parent_revision_ids.first().map(|id| id.0.clone()).unwrap_or_default(),
                "revision_id": input.revision.revision_id,
                "revision_index": input.revision.revision_index,
                "revision_sha256": input.record.revision_sha256,
                "revision_object_sha256": input.record.revision_object_sha256,
                "readback_sha256": input.record.revision_sha256,
            })
        })
        .collect::<Vec<_>>();
    let steps = revisions
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let operation = input
                .revision
                .operation
                .as_ref()
                .map(|operation| operation_name(&operation.kind))
                .unwrap_or("genesis");
            let step = input
                .revision
                .operation
                .as_ref()
                .map(|operation| {
                    json!({
                        "command_index": index,
                        "operation_id": operation.operation_id,
                        "operation": operation_name(&operation.kind),
                        "parent_revision_id": operation.parent_revision_id,
                        "child_revision_id": input.revision.revision_id,
                        "child_revision_sha256": input.record.revision_sha256,
                        "child_revision_object_sha256": input.record.revision_object_sha256,
                        "changed_elements": operation.source_elements,
                        "generated_elements": operation.generated_elements,
                        "retired_elements": operation.retired_elements,
                        "readback_sha256": input.record.revision_sha256,
                    })
                })
                .unwrap_or_else(|| {
                    json!({
                        "command_index": index,
                        "operation_id": "genesis",
                        "operation": operation,
                        "parent_revision_id": record.base_revision_id,
                        "child_revision_id": input.revision.revision_id,
                        "child_revision_sha256": input.record.revision_sha256,
                        "child_revision_object_sha256": input.record.revision_object_sha256,
                        "changed_elements": [],
                        "generated_elements": [],
                        "retired_elements": [],
                        "readback_sha256": input.record.revision_sha256,
                    })
                });
            step
        })
        .collect::<Vec<_>>();
    let mut result = json!({
        "schema_version": RESULT_SCHEMA_VERSION,
        "request_kind": request_kind,
        "status": if request_kind == "get" { "found" } else if replayed { "replayed" } else { "prepared" },
        "project_id": record.project_id,
        "transaction_id": record.transaction_id,
        "transaction_sha256": record.transaction_sha256,
        "transaction_object_sha256": record.transaction_object_sha256,
        "mesh_id": record.mesh_id,
        "lineage_id": record.lineage_id,
        "base_revision_id": record.base_revision_id,
        "base_revision_index": record.base_revision_index,
        "base_revision_sha256": record.base_revision_sha256,
        "final_revision_id": record.final_revision_id,
        "final_revision_index": record.final_revision_index,
        "final_revision_sha256": record.final_revision_sha256,
        "final_revision_object_sha256": record.final_revision_object_sha256,
        "idempotency_key": idempotency_key,
        "replayed": replayed,
        "revision_chain": revision_chain,
        "steps": steps,
        "readback": {
            "status": "passed",
            "revision_sha256": record.final_revision_sha256,
            "revision_object_sha256": record.final_revision_object_sha256,
            "readback_sha256": record.final_revision_sha256,
            "topology_validation_status": "passed",
            "deterministic_replay": true,
            "byte_exact_revision_replay": true,
            "restart_hash_verified": false,
            "partial_result_exposed": false,
        },
        "atomicity_status": "committed",
        "source_revision_unchanged": true,
        "revision_chain_persisted": true,
        "partial_result_exposed": false,
        "store_commit_status": if runtime_write_performed { "committed" } else { "not-touched" },
        "cas_commit_status": if runtime_write_performed { "committed" } else { "not-touched" },
        "runtime_write_performed": runtime_write_performed,
        "persistent_user_data_touched": runtime_write_performed,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": "structural_only",
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": "",
    });
    let bytes = canonical_json_bytes(&result).map_err(|error| {
        invalid(format!(
            "transaction result canonicalization failed: {error}"
        ))
    })?;
    if bytes.len() > max_bytes {
        return Err(invalid("transaction result exceeds max_response_bytes"));
    }
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    let final_bytes = canonical_json_bytes(&result)
        .map_err(|error| invalid(format!("transaction result serialization failed: {error}")))?;
    if final_bytes.len() > max_bytes {
        return Err(invalid("transaction result exceeds max_response_bytes"));
    }
    Ok(result)
}

fn transaction_payload(value: &Value) -> Result<AuthoringMeshV2TransactionPayload, RuntimeError> {
    serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("transaction payload is not closed: {error}")))
}

fn canonical_record_hash(
    record: &AuthoringMeshV2TransactionDurableRecord,
) -> Result<String, RuntimeError> {
    let mut value = serde_json::to_value(record)
        .map_err(|error| invalid(format!("transaction record serialization failed: {error}")))?;
    value["canonical_sha256"] = Value::String(String::new());
    Ok(canonical_json_hash(&value))
}

fn revision_record_for(
    project_id: &str,
    revision: &AuthoringMeshRevision,
    object: &CasObject,
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
        revision_object_sha256: object.record.sha256.clone(),
        revision_sha256: revision.canonical_sha256.clone(),
        operation_id: operation.map(|value| value.operation_id.clone()),
        operation_kind: operation.map(|value| operation_name(&value.kind).to_owned()),
        operation_lineage_sha256: operation.map(|value| value.operation_lineage_sha256.clone()),
        request_input_sha256: request_input_sha256.to_owned(),
        idempotency_key: idempotency_key.to_owned(),
        materialization_status: "runtime-owned-store-authoring-mesh-v2-durable-record@1".to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    };
    record.canonical_sha256 = {
        let mut value = serde_json::to_value(&record)
            .map_err(|error| invalid(format!("revision record serialization failed: {error}")))?;
        value["canonical_sha256"] = Value::String(String::new());
        canonical_json_hash(&value)
    };
    Ok(record)
}

fn load_chain(
    runtime: &Runtime,
    record: &AuthoringMeshV2TransactionDurableRecord,
) -> Result<Vec<AuthoringMeshV2TransactionRevisionInput>, RuntimeError> {
    record
        .revision_object_sha256s
        .iter()
        .enumerate()
        .map(|(index, object_sha256)| {
            let key = format!("{}-revision-{index}", record.idempotency_key);
            let durable = runtime
                .store
                .get_authoring_mesh_v2_durable_record(&record.project_id, &key)?
                .ok_or_else(|| invalid("transaction child revision durable row is missing"))?;
            let object = runtime
                .store
                .get_object(object_sha256)?
                .ok_or_else(|| invalid("transaction child revision CAS object is missing"))?;
            let bytes = runtime.cas_read_bounded(object_sha256, MAX_TRANSACTION_BYTES)?;
            let revision: AuthoringMeshRevision =
                serde_json::from_slice(&bytes).map_err(|error| {
                    invalid(format!(
                        "transaction child revision JSON is invalid: {error}"
                    ))
                })?;
            AuthoringMeshV2Revision::from_record(revision.clone())?;
            if durable.revision_object_sha256 != *object_sha256
                || durable.revision_sha256 != revision.canonical_sha256
                || durable.revision_id != revision.revision_id.0
            {
                return Err(invalid(
                    "transaction child revision readback binding differs",
                ));
            }
            Ok(AuthoringMeshV2TransactionRevisionInput {
                record: durable,
                revision,
                object,
            })
        })
        .collect()
}

fn verify_deterministic_replay(
    base_revision: &AuthoringMeshRevision,
    commands: &[AuthoringMeshV2TransactionCommand],
    expected: &[AuthoringMeshRevision],
) -> Result<(), RuntimeError> {
    let replay = AuthoringMeshV2Revision::from_record(base_revision.clone())?.apply_transaction(
        AuthoringMeshV2Transaction {
            commands: commands.to_vec(),
        },
    )?;
    if replay.revision_chain.len() != expected.len()
        || replay
            .revision_chain
            .iter()
            .zip(expected)
            .any(|(actual, expected)| {
                actual.revision_id != expected.revision_id
                    || actual.revision_index != expected.revision_index
                    || actual.canonical_sha256 != expected.canonical_sha256
            })
    {
        return Err(contract_error(
            "AUTHORING_TRANSACTION_READBACK_FAILED",
            "deterministic replay produced a different revision chain",
        ));
    }
    Ok(())
}

fn cleanup_staged(
    runtime: &Runtime,
    reservation: &forgecad_store::CasReservation,
    staged: &[CasObject],
) {
    for object in staged {
        let _ = runtime
            .store
            .release_cas_reservation_object(reservation, object, true);
    }
}

/// Runtime-owned prepare for one multi-operation AuthoringMesh transaction.
pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, PREPARE_FIELDS, "prepare request")?;
    if text(object, "schema_version")? != PREPARE_SCHEMA_VERSION
        || text(object, "writer_policy")? != WRITER_POLICY
        || text(object, "canonicalization_policy")? != CANONICALIZATION_POLICY
    {
        return Err(invalid("prepare policy differs from the closed contract"));
    }
    bool_const(object, "runtime_write_performed", false)?;
    let max_bytes = max_response_bytes(object)?;
    let project_id = id(object, "project_id")?.to_owned();
    let request_input_sha256 = request_input_hash(request, object)?;
    let transaction_value = object
        .get("transaction")
        .ok_or_else(|| invalid("transaction is missing"))?;
    let transaction_payload = transaction_payload(transaction_value)?;
    let (
        transaction_id,
        mesh_id,
        lineage_id,
        base_revision_id,
        base_revision_index,
        base_revision_sha256,
        commands,
    ) = parse_transaction(transaction_value)?;
    if transaction_payload.transaction_id != transaction_id
        || transaction_payload.mesh_id != mesh_id
        || transaction_payload.lineage_id != lineage_id
        || transaction_payload.base_revision_id != base_revision_id
        || transaction_payload.base_revision_index != base_revision_index
        || transaction_payload.base_revision_sha256 != base_revision_sha256
    {
        return Err(invalid(
            "transaction payload identity was not canonicalized consistently",
        ));
    }
    let idempotency_key = id(object, "idempotency_key")?.to_owned();
    let commands_for_replay = commands.clone();

    if let Some(existing) = runtime
        .store
        .authoring_repository()
        .get_authoring_mesh_transaction(&project_id, &idempotency_key)?
    {
        if existing.transaction_id != transaction_id
            || existing.transaction_sha256 != transaction_payload.canonical_sha256
            || existing.mesh_id != mesh_id
            || existing.lineage_id != lineage_id
            || existing.base_revision_id != base_revision_id
            || existing.base_revision_index != base_revision_index
            || existing.base_revision_sha256 != base_revision_sha256
            || existing.request_input_sha256 != request_input_sha256
        {
            return Err(contract_error(
                "AUTHORING_TRANSACTION_IDEMPOTENCY_CONFLICT",
                "idempotency key is already bound to another transaction",
            ));
        }
        let chain = load_chain(runtime, &existing)?;
        let base_record = runtime
            .store
            .get_authoring_mesh_v2_durable_record_by_revision(&project_id, &base_revision_id)?
            .ok_or_else(|| invalid("existing transaction base revision is unavailable"))?;
        let base_revision = revision_from_cas(runtime, &base_record)?;
        let expected_revisions = chain
            .iter()
            .map(|input| input.revision.clone())
            .collect::<Vec<_>>();
        verify_deterministic_replay(&base_revision, &commands_for_replay, &expected_revisions)?;
        return result_value(
            &existing,
            &chain,
            "prepare",
            true,
            false,
            Some(&existing.idempotency_key),
            max_bytes,
        );
    }

    let base_record = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(&project_id, &base_revision_id)?
        .ok_or_else(|| {
            contract_error(
                "AUTHORING_TRANSACTION_BASE_REVISION_MISMATCH",
                "base revision is unavailable",
            )
        })?;
    if base_record.mesh_id != mesh_id
        || base_record.lineage_id != lineage_id
        || base_record.revision_index != base_revision_index
        || base_record.revision_sha256 != base_revision_sha256
    {
        return Err(contract_error(
            "AUTHORING_TRANSACTION_BASE_REVISION_MISMATCH",
            "base revision id/index/sha256 does not match durable Store/CAS truth",
        ));
    }
    let base_revision = revision_from_cas(runtime, &base_record)?;
    let pure_result = AuthoringMeshV2Revision::from_record(base_revision.clone())?
        .apply_transaction(AuthoringMeshV2Transaction { commands })?;
    // Re-execute the typed command journal before staging any CAS object. A
    // successful receipt may therefore truthfully report deterministic replay
    // rather than relying on a hash-only assertion.
    verify_deterministic_replay(
        &base_revision,
        &commands_for_replay,
        &pure_result.revision_chain,
    )?;

    let reservation = runtime.store.begin_cas_reservation();
    let mut staged = Vec::new();
    let outcome = (|| -> Result<Value, RuntimeError> {
        let mut revisions = Vec::with_capacity(pure_result.revision_chain.len());
        for (index, revision) in pure_result.revision_chain.iter().enumerate() {
            let bytes = canonical_json_bytes(
                &serde_json::to_value(revision)
                    .map_err(|error| invalid(format!("revision serialization failed: {error}")))?,
            )
            .map_err(|error| invalid(format!("revision canonicalization failed: {error}")))?;
            let object = runtime.store.put_object_reserved(
                &reservation,
                &bytes,
                None,
                JSON_MIME,
                AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
                &now_string(),
            )?;
            staged.push(object.clone());
            let child_key = format!("{idempotency_key}-revision-{index}");
            let record = revision_record_for(
                &project_id,
                revision,
                &object,
                &request_input_sha256,
                &child_key,
            )?;
            revisions.push(AuthoringMeshV2TransactionRevisionInput {
                record,
                revision: revision.clone(),
                object: object.record.clone(),
            });
        }
        let transaction_bytes = canonical_json_bytes(transaction_value)
            .map_err(|error| invalid(format!("transaction canonicalization failed: {error}")))?;
        let transaction_object = runtime.store.put_object_reserved(
            &reservation,
            &transaction_bytes,
            None,
            JSON_MIME,
            AUTHORING_MESH_V2_TRANSACTION_OBJECT_KIND,
            &now_string(),
        )?;
        staged.push(transaction_object.clone());
        let final_revision = revisions
            .last()
            .ok_or_else(|| invalid("transaction produced no revisions"))?;
        let mut record = AuthoringMeshV2TransactionDurableRecord {
            schema_version: AUTHORING_MESH_V2_TRANSACTION_RECORD_SCHEMA_VERSION.to_owned(),
            project_id: project_id.clone(),
            transaction_id: transaction_id.clone(),
            mesh_id: mesh_id.clone(),
            lineage_id: lineage_id.clone(),
            base_revision_id: base_revision_id.clone(),
            base_revision_index,
            base_revision_sha256: base_revision_sha256.clone(),
            final_revision_id: final_revision.record.revision_id.clone(),
            final_revision_index: final_revision.record.revision_index,
            final_revision_sha256: final_revision.record.revision_sha256.clone(),
            final_revision_object_sha256: final_revision.record.revision_object_sha256.clone(),
            transaction_sha256: transaction_payload.canonical_sha256.clone(),
            transaction_object_sha256: transaction_object.record.sha256.clone(),
            revision_ids: revisions
                .iter()
                .map(|input| input.record.revision_id.clone())
                .collect(),
            revision_sha256s: revisions
                .iter()
                .map(|input| input.record.revision_sha256.clone())
                .collect(),
            revision_object_sha256s: revisions
                .iter()
                .map(|input| input.record.revision_object_sha256.clone())
                .collect(),
            operation_ids: revisions
                .iter()
                .map(|input| {
                    input
                        .record
                        .operation_id
                        .clone()
                        .unwrap_or_else(|| "genesis".to_owned())
                })
                .collect(),
            request_input_sha256: request_input_sha256.clone(),
            idempotency_key: idempotency_key.clone(),
            materialization_status: AUTHORING_MESH_V2_TRANSACTION_STATUS.to_owned(),
            canonical_sha256: String::new(),
            created_at: now_string(),
        };
        record.canonical_sha256 = canonical_record_hash(&record)?;
        let commit = AuthoringMeshV2TransactionCommit {
            record: record.clone(),
            payload: transaction_payload.clone(),
            transaction_object: transaction_object.record.clone(),
            revisions,
        };
        let (stored, replayed) = runtime
            .store
            .authoring_repository()
            .record_authoring_mesh_transaction_with_replay(&commit)?;
        let chain = load_chain(runtime, &stored)?;
        result_value(
            &stored,
            &chain,
            "prepare",
            replayed,
            !replayed,
            Some(&stored.idempotency_key),
            max_bytes,
        )
    })();
    match outcome {
        Ok(value) => {
            for object in &staged {
                runtime
                    .store
                    .release_cas_reservation_object(&reservation, object, false)?;
            }
            Ok(value)
        }
        Err(error) => {
            cleanup_staged(runtime, &reservation, &staged);
            Err(error)
        }
    }
}

/// Read one transaction receipt.  This path performs no CAS/SQLite writes;
/// Store/CAS metadata, all child rows and every topology payload are verified
/// before the compact result is returned.
pub(crate) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_FIELDS, "get request")?;
    if text(object, "schema_version")? != GET_SCHEMA_VERSION
        || text(object, "writer_policy")? != WRITER_POLICY
        || text(object, "canonicalization_policy")? != CANONICALIZATION_POLICY
    {
        return Err(invalid("get policy differs from the closed contract"));
    }
    bool_const(object, "runtime_write_performed", false)?;
    bool_const(object, "persistent_user_data_touched", false)?;
    let max_bytes = max_response_bytes(object)?;
    let project_id = id(object, "project_id")?;
    let transaction_id = id(object, "transaction_id")?;
    let transaction_sha256 = hash(object, "transaction_sha256")?;
    let transaction_object_sha256 = hash(object, "transaction_object_sha256")?;
    let _request_input_sha256 = request_input_hash(request, object)?;
    let record = runtime
        .store
        .authoring_repository()
        .get_authoring_mesh_transaction_by_id(project_id, transaction_id)?
        .ok_or_else(|| {
            contract_error(
                "AUTHORING_TRANSACTION_NOT_FOUND",
                "transaction is not durably materialized",
            )
        })?;
    if record.transaction_sha256 != transaction_sha256
        || record.transaction_object_sha256 != transaction_object_sha256
    {
        return Err(contract_error(
            "AUTHORING_TRANSACTION_CORRUPT",
            "get hash expectations differ from Store/CAS truth",
        ));
    }
    let transaction_bytes =
        runtime.cas_read_bounded(transaction_object_sha256, MAX_TRANSACTION_BYTES)?;
    let transaction_value: Value = serde_json::from_slice(&transaction_bytes)
        .map_err(|error| invalid(format!("transaction CAS JSON is invalid: {error}")))?;
    let payload = transaction_payload(&transaction_value)?;
    if payload.canonical_sha256 != record.transaction_sha256
        || canonical_json_bytes(&transaction_value).map_err(|error| invalid(error.to_string()))?
            != transaction_bytes
    {
        return Err(contract_error(
            "AUTHORING_TRANSACTION_CORRUPT",
            "transaction CAS readback failed",
        ));
    }
    let chain = load_chain(runtime, &record)?;
    let (
        _,
        mesh_id,
        lineage_id,
        base_revision_id,
        base_revision_index,
        base_revision_sha256,
        commands,
    ) = parse_transaction(&transaction_value)?;
    if mesh_id != record.mesh_id
        || lineage_id != record.lineage_id
        || base_revision_id != record.base_revision_id
        || base_revision_index != record.base_revision_index
        || base_revision_sha256 != record.base_revision_sha256
    {
        return Err(contract_error(
            "AUTHORING_TRANSACTION_CORRUPT",
            "transaction journal identity differs from durable receipt",
        ));
    }
    let base_record = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(
            &record.project_id,
            &record.base_revision_id,
        )?
        .ok_or_else(|| invalid("transaction base revision is unavailable during get"))?;
    let base_revision = revision_from_cas(runtime, &base_record)?;
    let expected_revisions = chain
        .iter()
        .map(|input| input.revision.clone())
        .collect::<Vec<_>>();
    verify_deterministic_replay(&base_revision, &commands, &expected_revisions)?;
    result_value(&record, &chain, "get", false, false, None, max_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authoring_mesh_v2::AuthoringMeshV2GenesisInput;
    use std::fs;
    use uuid::Uuid;

    fn seed_runtime() -> (Runtime, String, AuthoringMeshRevision) {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("transaction test", json!({"scope":"test"}))
            .expect("project");
        let genesis = AuthoringMeshV2Revision::genesis(AuthoringMeshV2GenesisInput {
            mesh_id: "mesh-tx".into(),
            lineage_id: "lineage-tx".into(),
            positions_m: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            faces: vec![vec![0, 1, 2, 3]],
            evaluated: None,
            source_binding: None,
            foundation_source_binding: None,
        })
        .expect("genesis")
        .record()
        .clone();
        let bytes = canonical_json_bytes(&serde_json::to_value(&genesis).expect("genesis json"))
            .expect("genesis bytes");
        let reservation = runtime.store.begin_cas_reservation();
        let object = runtime
            .store
            .put_object_reserved(
                &reservation,
                &bytes,
                None,
                JSON_MIME,
                AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
                "1",
            )
            .expect("genesis object");
        let record = revision_record_for(
            &project.project_id,
            &genesis,
            &object,
            &"a".repeat(64),
            "genesis-seed",
        )
        .expect("genesis record");
        runtime
            .store
            .record_authoring_mesh_v2_revision_with_replay(&record, &genesis, &object.record)
            .expect("persist genesis");
        runtime
            .store
            .release_cas_reservation_object(&reservation, &object, false)
            .expect("release genesis");
        (runtime, project.project_id, genesis)
    }

    fn transaction_for(
        genesis: &AuthoringMeshRevision,
        transaction_id: &str,
        delta: [f64; 3],
        second_invalid: bool,
    ) -> Value {
        let vertex_id = genesis.original.vertices[0].vertex_id.0.clone();
        let mut commands = vec![json!({
            "command_index": 0,
            "operation": "move_vertices",
            "operation_id": format!("op-{transaction_id}-0"),
            "vertices": [{"kind":"vertex", "id": vertex_id}],
            "delta_m": [delta],
            "operation_lineage_sha256": "b".repeat(64),
        })];
        if second_invalid {
            commands.push(json!({
                "command_index": 1,
                "operation": "move_vertices",
                "operation_id": format!("op-{transaction_id}-1"),
                "vertices": [{"kind":"vertex", "id": "missing-vertex"}],
                "delta_m": [[0.0, 0.0, 0.1]],
                "operation_lineage_sha256": "c".repeat(64),
            }));
        }
        let mut transaction = json!({
            "schema_version": AUTHORING_MESH_V2_TRANSACTION_PAYLOAD_SCHEMA_VERSION,
            "transaction_id": transaction_id,
            "mesh_id": genesis.mesh_id,
            "lineage_id": genesis.lineage_id,
            "base_revision_id": genesis.revision_id,
            "base_revision_index": genesis.revision_index,
            "base_revision_sha256": genesis.canonical_sha256,
            "commands": commands,
            "budgets": {
                "max_commands": 32,
                "max_move_vertices_per_command": 32,
                "max_face_degree": 32,
                "max_vertex_delta_m": 1.0,
                "max_face_extrude_distance_m": 10.0,
                "overflow_policy": "reject-entire-transaction@1"
            },
            "execution_policy": {
                "writer_policy": WRITER_POLICY,
                "source_of_truth": "original-authoring-mesh@2",
                "reference_policy": "stable-or-earlier-generated-element-by-kind@1",
                "atomicity_policy": "clone-before-first-command-no-partial-result@1",
                "replay_policy": "same-input-same-base-deterministic-revision-chain@1",
                "evaluation_policy": "authored-edit-invalidates-evaluated-sidecar@2",
                "identity_policy": "runtime-derived-lineage-operation-parent-stable-no-reuse@2"
            },
            "canonicalization_policy": CANONICALIZATION_POLICY,
            "canonical_sha256": ""
        });
        transaction["canonical_sha256"] = Value::String(canonical_json_hash(&transaction));
        transaction
    }

    fn request_for(project_id: &str, transaction: Value, key: &str) -> Value {
        let mut request = json!({
            "schema_version": PREPARE_SCHEMA_VERSION,
            "project_id": project_id,
            "transaction": transaction,
            "idempotency_key": key,
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": CANONICALIZATION_POLICY,
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn assert_result_shape(result: &Value, request_kind: &str) {
        let expected = [
            "schema_version",
            "request_kind",
            "status",
            "project_id",
            "transaction_id",
            "transaction_sha256",
            "transaction_object_sha256",
            "mesh_id",
            "lineage_id",
            "base_revision_id",
            "base_revision_index",
            "base_revision_sha256",
            "final_revision_id",
            "final_revision_index",
            "final_revision_sha256",
            "final_revision_object_sha256",
            "revision_chain",
            "steps",
            "readback",
            "replayed",
            "idempotency_key",
            "atomicity_status",
            "source_revision_unchanged",
            "revision_chain_persisted",
            "partial_result_exposed",
            "store_commit_status",
            "cas_commit_status",
            "runtime_write_performed",
            "persistent_user_data_touched",
            "stage_advanced",
            "candidate_confirmed",
            "version_created",
            "export_performed",
            "quality_status",
            "canonicalization_policy",
            "canonical_sha256",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let actual = result
            .as_object()
            .expect("result object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "result schema drift");
        assert_eq!(result["request_kind"], request_kind);
        assert!(result["canonical_sha256"].as_str().is_some_and(is_sha256));
        let readback_keys = [
            "status",
            "revision_sha256",
            "revision_object_sha256",
            "readback_sha256",
            "topology_validation_status",
            "deterministic_replay",
            "byte_exact_revision_replay",
            "restart_hash_verified",
            "partial_result_exposed",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(
            result["readback"]
                .as_object()
                .expect("readback object")
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>(),
            readback_keys
        );
    }

    #[test]
    fn prepare_replay_and_same_key_conflict_are_durable() {
        let (runtime, project_id, genesis) = seed_runtime();
        let transaction = transaction_for(&genesis, "tx-replay", [0.1, 0.0, 0.0], false);
        let request = request_for(&project_id, transaction.clone(), "tx-key");
        let first = runtime
            .authoring_mesh_transaction_prepare(&request)
            .expect("first prepare");
        assert_result_shape(&first, "prepare");
        assert_eq!(first["status"], "prepared");
        let replay = runtime
            .authoring_mesh_transaction_prepare(&request)
            .expect("replay");
        assert_result_shape(&replay, "prepare");
        assert_eq!(replay["status"], "replayed");
        assert_eq!(first["transaction_sha256"], replay["transaction_sha256"]);
        let conflict_transaction = transaction_for(&genesis, "tx-replay", [0.2, 0.0, 0.0], false);
        let conflict = request_for(&project_id, conflict_transaction, "tx-key");
        let error = runtime
            .authoring_mesh_transaction_prepare(&conflict)
            .expect_err("same key conflict");
        assert!(error
            .to_string()
            .contains("AUTHORING_TRANSACTION_IDEMPOTENCY_CONFLICT"));
    }

    #[test]
    fn invalid_later_command_leaves_zero_transaction_and_child_rows() {
        let (runtime, project_id, genesis) = seed_runtime();
        let request = request_for(
            &project_id,
            transaction_for(&genesis, "tx-invalid-late", [0.1, 0.0, 0.0], true),
            "tx-invalid-key",
        );
        let error = runtime
            .authoring_mesh_transaction_prepare(&request)
            .expect_err("late invalid command");
        assert!(!error.to_string().is_empty());
        assert!(runtime
            .store
            .authoring_repository()
            .get_authoring_mesh_transaction(&project_id, "tx-invalid-key")
            .expect("transaction lookup")
            .is_none());
        assert!(runtime
            .store
            .get_authoring_mesh_v2_durable_record(&project_id, "tx-invalid-key-revision-0")
            .expect("child lookup")
            .is_none());
    }

    #[test]
    fn drop_and_reopen_get_revalidates_chain() {
        let root = std::env::temp_dir().join(format!("forgecad-authoring-tx-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("test root");
        let db = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let (runtime, project_id, genesis) = {
            let runtime = Runtime::from_store(
                forgecad_store::Store::open_with_cas(&db, &cas).expect("store"),
            )
            .expect("runtime");
            let project = runtime
                .create_project("transaction restart", json!({"scope":"test"}))
                .expect("project");
            let genesis = AuthoringMeshV2Revision::genesis(AuthoringMeshV2GenesisInput {
                mesh_id: "mesh-restart".into(),
                lineage_id: "lineage-restart".into(),
                positions_m: vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [1.0, 1.0, 0.0],
                    [0.0, 1.0, 0.0],
                ],
                faces: vec![vec![0, 1, 2, 3]],
                evaluated: None,
                source_binding: None,
                foundation_source_binding: None,
            })
            .expect("genesis")
            .record()
            .clone();
            let bytes = canonical_json_bytes(&serde_json::to_value(&genesis).expect("json"))
                .expect("bytes");
            let reservation = runtime.store.begin_cas_reservation();
            let object = runtime
                .store
                .put_object_reserved(
                    &reservation,
                    &bytes,
                    None,
                    JSON_MIME,
                    AUTHORING_MESH_V2_REVISION_OBJECT_KIND,
                    "1",
                )
                .expect("object");
            let record = revision_record_for(
                &project.project_id,
                &genesis,
                &object,
                &"a".repeat(64),
                "genesis-seed",
            )
            .expect("record");
            runtime
                .store
                .record_authoring_mesh_v2_revision_with_replay(&record, &genesis, &object.record)
                .expect("persist");
            runtime
                .store
                .release_cas_reservation_object(&reservation, &object, false)
                .expect("release");
            (runtime, project.project_id, genesis)
        };
        let request = request_for(
            &project_id,
            transaction_for(&genesis, "tx-restart", [0.1, 0.0, 0.0], false),
            "tx-restart-key",
        );
        let first = runtime
            .authoring_mesh_transaction_prepare(&request)
            .expect("prepare");
        let record = runtime
            .store
            .authoring_repository()
            .get_authoring_mesh_transaction(&project_id, "tx-restart-key")
            .expect("record")
            .expect("record exists");
        drop(runtime);
        let reopened = Runtime::from_store(
            forgecad_store::Store::open_with_cas(&db, &cas).expect("reopen store"),
        )
        .expect("reopen runtime");
        let mut get = json!({
            "schema_version": GET_SCHEMA_VERSION,
            "project_id": project_id,
            "transaction_id": record.transaction_id,
            "transaction_sha256": record.transaction_sha256,
            "transaction_object_sha256": record.transaction_object_sha256,
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": false,
            "persistent_user_data_touched": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": CANONICALIZATION_POLICY,
            "input_sha256": ""
        });
        get["input_sha256"] = Value::String(canonical_json_hash(&get));
        let readback = reopened.authoring_mesh_transaction_get(&get).expect("get");
        assert_result_shape(&readback, "get");
        assert_eq!(readback["status"], "found");
        assert_eq!(
            first["final_revision_sha256"],
            readback["final_revision_sha256"]
        );
        let _ = fs::remove_dir_all(root);
    }
}
