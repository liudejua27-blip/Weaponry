//! Runtime-owned durable `AuthoringMeshIdentityLineage@1` producer/readback.
//!
//! The @2 prepare/get surface is deliberately closed: callers provide only
//! exact current AuthoringMesh/evidence bindings and an optional persisted
//! parent. Runtime derives authored and representation identities from the
//! durable AuthoringMesh source. It does not accept caller-supplied identity
//! arrays, tombstones or correspondence claims. The Store identity record is
//! the atomic CAS/index owner; its evaluated sidecar fields remain separate
//! from original authored identity preimages.

use super::{
    authoring_mesh_identity, canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256,
    now_string, sha256_hex, Runtime, RuntimeError,
};
use forgecad_store::{
    AuthoringMeshDurableRecord, AuthoringMeshIdentityLineageDurableRecord,
    AuthoringMeshProjectionIndexRecord, AUTHORING_MESH_CANONICAL_OBJECT_KIND,
    AUTHORING_MESH_IDENTITY_LINEAGE_DURABLE_RECORD_SCHEMA_VERSION,
    AUTHORING_MESH_IDENTITY_LINEAGE_OBJECT_KIND,
};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const IDENTITY_SCHEMA: &str = "AuthoringMeshIdentityLineage@1";
const PREPARE_REQUEST_SCHEMA: &str = "AuthoringMeshIdentityLineagePrepareRequest@2";
const GET_REQUEST_SCHEMA: &str = "AuthoringMeshIdentityLineageGetRequest@2";
const PREPARE_RESULT_SCHEMA: &str = "AuthoringMeshIdentityLineagePrepareResult@2";
const GET_RESULT_SCHEMA: &str = "AuthoringMeshIdentityLineageGetResult@2";
const IDENTITY_OBJECT_KIND: &str = AUTHORING_MESH_IDENTITY_LINEAGE_OBJECT_KIND;
const JSON_MIME: &str = "application/json";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_CAS_JSON_BYTES: u64 = 1024 * 1024;
const MAX_ELEMENTS: usize = 32_768;
const MAX_PARENT_IDS: usize = 8;
const MAX_ID_LENGTH: usize = 128;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";
const IDENTITY_POLICY: &str = "runtime-owned-authored-ids-with-monotonic-tombstones@1";
const EVALUATED_IDENTITY_POLICY: &str = "non-bijective-derived-only-no-authoring-source-reversal@1";
const QUALITY_STATUS: &str = "structural_only";
const DURABLE_RECORD_STATUS: &str =
    "runtime-owned-store-authoring-mesh-identity-lineage-durable-record@1";
const LIMITATIONS: &[&str] = &[
    "RUNTIME_SOLE_WRITER",
    "NO_STAGE_ADVANCEMENT",
    "NO_CANDIDATE_CONFIRM",
    "NO_VERSION_CREATED",
    "NO_EXPORT",
    "IDENTITY_LEDGER_RUNTIME_DERIVED",
    "GENERAL_CORRESPONDENCE_BEYOND_TYPED_SPLIT_COLLAPSE_DISSOLVE_NOT_PROVEN",
    "CROSS_VERSION_STABILITY_NOT_PROVEN",
    "STRUCTURAL_ONLY_NOT_COMMERCIAL_QUALITY",
];

const IDENTITY_FIELDS: &[&str] = &[
    "schema_version",
    "lineage_id",
    "project_id",
    "authoring_node_id",
    "part_id",
    "genesis_source_mesh_sha256",
    "current_source_mesh_sha256",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "parent_lineage_object_sha256",
    "parent_lineage_sha256",
    "revision_index",
    "revision_kind",
    "operation_lineage_sha256",
    "source_program_object_sha256",
    "source_program_sha256",
    "source_artifact_object_sha256",
    "source_artifact_sha256",
    "source_artifact_readback_object_sha256",
    "source_artifact_readback_sha256",
    "elements",
    "tombstones",
    "correspondence",
    "identity_policy",
    "evaluated_identity_policy",
    "budgets",
    "writer_policy",
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
    "canonical_mesh_id",
    "canonical_mesh_object_sha256",
    "canonical_mesh_sha256",
    "genesis_source_mesh_sha256",
    "current_source_mesh_sha256",
    "parent_lineage_object_sha256",
    "parent_lineage_sha256",
    "operation_lineage_sha256",
    "expected_lineage_id",
    "expected_lineage_sha256",
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
    "lineage_id",
    "revision_index",
    "candidate_id",
    "candidate_state_sha256",
    "canonical_mesh_id",
    "canonical_mesh_object_sha256",
    "canonical_mesh_sha256",
    "identity_lineage_object_sha256",
    "identity_lineage_sha256",
    "max_response_bytes",
    "writer_policy",
    "runtime_write_performed",
    "persistent_user_data_touched",
    "input_sha256",
];

const PREPARE_RESULT_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "source_candidate_id",
    "source_candidate_state_sha256",
    "base_version_id",
    "authoring_node_id",
    "part_id",
    "lineage_id",
    "genesis_source_mesh_sha256",
    "current_source_mesh_sha256",
    "candidate_id",
    "candidate_state_sha256",
    "canonical_mesh_id",
    "canonical_mesh_object_sha256",
    "canonical_mesh_sha256",
    "parent_lineage_object_sha256",
    "parent_lineage_sha256",
    "revision_index",
    "revision_kind",
    "operation_lineage_sha256",
    "source_program_object_sha256",
    "source_program_sha256",
    "source_artifact_object_sha256",
    "source_artifact_sha256",
    "source_artifact_readback_object_sha256",
    "source_artifact_readback_sha256",
    "evaluated_artifact_object_sha256",
    "evaluated_artifact_sha256",
    "evaluated_artifact_readback_object_sha256",
    "evaluated_artifact_readback_sha256",
    "identity_lineage_object_sha256",
    "identity_lineage_sha256",
    "identity_lineage",
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
    "authoring_node_id",
    "part_id",
    "lineage_id",
    "genesis_source_mesh_sha256",
    "current_source_mesh_sha256",
    "candidate_id",
    "candidate_state_sha256",
    "base_version_id",
    "canonical_mesh_id",
    "canonical_mesh_object_sha256",
    "canonical_mesh_sha256",
    "parent_lineage_object_sha256",
    "parent_lineage_sha256",
    "revision_index",
    "revision_kind",
    "operation_lineage_sha256",
    "source_program_object_sha256",
    "source_program_sha256",
    "source_artifact_object_sha256",
    "source_artifact_sha256",
    "source_artifact_readback_object_sha256",
    "source_artifact_readback_sha256",
    "evaluated_artifact_object_sha256",
    "evaluated_artifact_sha256",
    "evaluated_artifact_readback_object_sha256",
    "evaluated_artifact_readback_sha256",
    "identity_lineage_object_sha256",
    "identity_lineage_sha256",
    "identity_lineage",
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

#[derive(Clone, Debug)]
struct SourceTruth {
    record: AuthoringMeshDurableRecord,
    canonical: Value,
    projection: Value,
    projection_index: AuthoringMeshProjectionIndexRecord,
    source_artifact_id: String,
    current_source_mesh_sha256: String,
    source_lineage_sha256: String,
}

#[derive(Clone, Debug)]
struct ParentTruth {
    object_sha256: String,
    payload: Value,
}

#[derive(Clone, Debug)]
struct TopologyTombstoneProof {
    source_element_id: String,
    element_kind: String,
    retired_revision_index: u64,
    operation_lineage_sha256: String,
    reason: String,
}

#[derive(Clone, Debug)]
struct TopologyCorrespondenceProof {
    kind: String,
    parent_source_element_ids: Vec<String>,
    child_source_element_ids: Vec<String>,
    operation_lineage_sha256: String,
    identity_namespace_status: String,
}

#[derive(Clone, Debug)]
struct TopologyOperationProof {
    operation: String,
    parent_revision: u64,
    child_revision: u64,
    operation_lineage_sha256: String,
    source_vertex_ids: Vec<String>,
    source_edge_ids: Vec<String>,
    source_face_ids: Vec<String>,
    generated_vertex_ids: Vec<String>,
    generated_edge_ids: Vec<String>,
    generated_loop_ids: Vec<String>,
    generated_face_ids: Vec<String>,
    retired_vertex_ids: Vec<String>,
    retired_edge_ids: Vec<String>,
    retired_loop_ids: Vec<String>,
    retired_face_ids: Vec<String>,
    tombstones: Vec<TopologyTombstoneProof>,
    correspondence: TopologyCorrespondenceProof,
    identity_namespace_status: String,
    canonical_sha256: String,
}

const TOPOLOGY_PROOF_SCHEMA: &str = "AuthoringMeshTopologyOperationProof@1";
const TOPOLOGY_IDENTITY_NAMESPACE: &str =
    "source-element-only-not-materialized-to-identity-lineage@1";

fn proof_ids(object: &Map<String, Value>, key: &str) -> Result<Vec<String>, RuntimeError> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("topology proof {key} is missing")))?;
    if values.len() > 64 {
        return Err(invalid(format!("topology proof {key} exceeds its budget")));
    }
    let mut result = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let id = value
            .as_str()
            .filter(|value| is_opaque_id(value) && value.len() <= MAX_ID_LENGTH)
            .ok_or_else(|| invalid(format!("topology proof {key} contains an invalid ID")))?;
        if !seen.insert(id.to_owned()) {
            return Err(invalid(format!(
                "topology proof {key} contains a duplicate ID"
            )));
        }
        result.push(id.to_owned());
    }
    Ok(result)
}

fn proof_id_set(values: &[String]) -> BTreeSet<String> {
    values.iter().cloned().collect()
}

fn proof_sets_equal(left: &[String], right: &[String]) -> bool {
    proof_id_set(left) == proof_id_set(right)
}

fn parse_topology_proof(
    value: &Value,
    expected_operation_hash: &str,
    expected_parent_revision: u64,
) -> Result<TopologyOperationProof, RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "operation",
            "parent_revision",
            "child_revision",
            "operation_lineage_sha256",
            "source_vertex_ids",
            "source_edge_ids",
            "source_face_ids",
            "generated_vertex_ids",
            "generated_edge_ids",
            "generated_loop_ids",
            "generated_face_ids",
            "retired_vertex_ids",
            "retired_edge_ids",
            "retired_loop_ids",
            "retired_face_ids",
            "tombstones",
            "correspondence",
            "identity_namespace_status",
            "canonical_sha256",
        ],
        "topology operation proof",
    )?;
    if text(object, "schema_version")? != TOPOLOGY_PROOF_SCHEMA
        || text(object, "identity_namespace_status")? != TOPOLOGY_IDENTITY_NAMESPACE
    {
        return Err(invalid("topology proof schema or namespace differs"));
    }
    let operation = text(object, "operation")?.to_owned();
    if !matches!(
        operation.as_str(),
        "split_edge" | "collapse_edge" | "dissolve_edge"
    ) {
        return Err(invalid("topology proof operation is unsupported"));
    }
    let parent_revision = object
        .get("parent_revision")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 1_000_000)
        .ok_or_else(|| invalid("topology proof parent revision is invalid"))?;
    let child_revision = object
        .get("child_revision")
        .and_then(Value::as_u64)
        .filter(|value| *value <= 1_000_000)
        .ok_or_else(|| invalid("topology proof child revision is invalid"))?;
    if parent_revision != expected_parent_revision
        || child_revision != parent_revision.saturating_add(1)
    {
        return Err(invalid(
            "topology proof revision does not match the parent identity revision",
        ));
    }
    let operation_lineage_sha256 = sha(object, "operation_lineage_sha256")?.to_owned();
    if operation_lineage_sha256 != expected_operation_hash {
        return Err(invalid(
            "topology proof operation hash differs from the identity request",
        ));
    }
    let canonical_sha256 = sha(object, "canonical_sha256")?.to_owned();
    let mut without_hash = value.clone();
    without_hash["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&without_hash) != canonical_sha256 {
        return Err(invalid("topology proof canonical hash differs"));
    }

    let source_vertex_ids = proof_ids(object, "source_vertex_ids")?;
    let source_edge_ids = proof_ids(object, "source_edge_ids")?;
    let source_face_ids = proof_ids(object, "source_face_ids")?;
    let generated_vertex_ids = proof_ids(object, "generated_vertex_ids")?;
    let generated_edge_ids = proof_ids(object, "generated_edge_ids")?;
    let generated_loop_ids = proof_ids(object, "generated_loop_ids")?;
    let generated_face_ids = proof_ids(object, "generated_face_ids")?;
    let retired_vertex_ids = proof_ids(object, "retired_vertex_ids")?;
    let retired_edge_ids = proof_ids(object, "retired_edge_ids")?;
    let retired_loop_ids = proof_ids(object, "retired_loop_ids")?;
    let retired_face_ids = proof_ids(object, "retired_face_ids")?;

    let mut namespaces = BTreeMap::<String, &'static str>::new();
    for (namespace, ids) in [
        ("source", &source_vertex_ids),
        ("source", &source_edge_ids),
        ("source", &source_face_ids),
        ("generated", &generated_vertex_ids),
        ("generated", &generated_edge_ids),
        ("generated", &generated_loop_ids),
        ("generated", &generated_face_ids),
        ("retired", &retired_vertex_ids),
        ("retired", &retired_edge_ids),
        ("retired", &retired_loop_ids),
        ("retired", &retired_face_ids),
    ] {
        for id in ids {
            if let Some(previous) = namespaces.get(id) {
                if namespace == "generated" || *previous == "generated" {
                    return Err(invalid(
                        "topology proof reuses an ID across generated/source or retired namespaces",
                    ));
                }
                if *previous == "retired" && namespace == "retired" {
                    return Err(invalid("topology proof duplicates a retired ID"));
                }
                if *previous == "retired" && namespace == "source" {
                    // A retired source element is deliberately also listed in
                    // the operation's source set.  The tombstone namespace is
                    // a subset marker, not a second identity namespace.
                    continue;
                }
                if *previous == "source" && namespace == "retired" {
                    namespaces.insert(id.clone(), namespace);
                    continue;
                }
                return Err(invalid("topology proof duplicates a source ID"));
            }
            namespaces.insert(id.clone(), namespace);
        }
    }

    let tombstone_values = object
        .get("tombstones")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("topology proof tombstones are missing"))?;
    if tombstone_values.is_empty() || tombstone_values.len() > 64 {
        return Err(invalid(
            "topology proof tombstones are outside the bounded range",
        ));
    }
    let mut tombstones = Vec::with_capacity(tombstone_values.len());
    let mut tombstone_ids = BTreeSet::new();
    for value in tombstone_values {
        let item = exact_object(
            value,
            &[
                "source_element_id",
                "element_kind",
                "retired_revision_index",
                "operation_lineage_sha256",
                "reason",
            ],
            "topology proof tombstone",
        )?;
        let source_element_id = identifier(item, "source_element_id")?.to_owned();
        let element_kind = text(item, "element_kind")?.to_owned();
        if !matches!(element_kind.as_str(), "vertex" | "edge" | "loop" | "face") {
            return Err(invalid("topology proof tombstone element kind is invalid"));
        }
        let retired_revision_index = item
            .get("retired_revision_index")
            .and_then(Value::as_u64)
            .ok_or_else(|| invalid("topology proof tombstone revision is invalid"))?;
        let tombstone_operation = sha(item, "operation_lineage_sha256")?.to_owned();
        let reason = text(item, "reason")?.to_owned();
        if retired_revision_index != child_revision
            || tombstone_operation != operation_lineage_sha256
            || !matches!(
                reason.as_str(),
                "collapsed" | "dissolved" | "merged" | "replaced"
            )
            || !tombstone_ids.insert((element_kind.clone(), source_element_id.clone()))
        {
            return Err(invalid("topology proof tombstone binding is invalid"));
        }
        let retired = match element_kind.as_str() {
            "vertex" => &retired_vertex_ids,
            "edge" => &retired_edge_ids,
            "loop" => &retired_loop_ids,
            "face" => &retired_face_ids,
            _ => unreachable!(),
        };
        if !retired.contains(&source_element_id) {
            return Err(invalid(
                "topology proof tombstone is not present in its retired set",
            ));
        }
        tombstones.push(TopologyTombstoneProof {
            source_element_id,
            element_kind,
            retired_revision_index,
            operation_lineage_sha256: tombstone_operation,
            reason,
        });
    }
    let expected_tombstone_ids = retired_vertex_ids
        .iter()
        .map(|id| ("vertex".to_owned(), id.clone()))
        .chain(
            retired_edge_ids
                .iter()
                .map(|id| ("edge".to_owned(), id.clone())),
        )
        .chain(
            retired_loop_ids
                .iter()
                .map(|id| ("loop".to_owned(), id.clone())),
        )
        .chain(
            retired_face_ids
                .iter()
                .map(|id| ("face".to_owned(), id.clone())),
        )
        .collect::<BTreeSet<_>>();
    if tombstone_ids != expected_tombstone_ids {
        return Err(invalid(
            "topology proof tombstones do not exactly cover retired source elements",
        ));
    }

    let correspondence_values = object
        .get("correspondence")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("topology proof correspondence is missing"))?;
    if correspondence_values.len() != 1 {
        return Err(invalid(
            "topology proof must carry exactly one closed correspondence",
        ));
    }
    let correspondence_object = exact_object(
        &correspondence_values[0],
        &[
            "kind",
            "parent_source_element_ids",
            "child_source_element_ids",
            "operation_lineage_sha256",
            "identity_namespace_status",
        ],
        "topology proof correspondence",
    )?;
    let kind = text(correspondence_object, "kind")?.to_owned();
    if !matches!(kind.as_str(), "one-to-many" | "many-to-one")
        || text(correspondence_object, "identity_namespace_status")? != TOPOLOGY_IDENTITY_NAMESPACE
    {
        return Err(invalid("topology proof correspondence metadata is invalid"));
    }
    let parent_source_element_ids = proof_ids(correspondence_object, "parent_source_element_ids")?;
    let child_source_element_ids = proof_ids(correspondence_object, "child_source_element_ids")?;
    let correspondence_operation =
        sha(correspondence_object, "operation_lineage_sha256")?.to_owned();
    if correspondence_operation != operation_lineage_sha256 {
        return Err(invalid(
            "topology proof correspondence operation hash differs",
        ));
    }
    let correspondence = TopologyCorrespondenceProof {
        kind,
        parent_source_element_ids,
        child_source_element_ids,
        operation_lineage_sha256: correspondence_operation,
        identity_namespace_status: TOPOLOGY_IDENTITY_NAMESPACE.to_owned(),
    };
    match operation.as_str() {
        "split_edge" => {
            if correspondence.kind != "one-to-many"
                || correspondence.parent_source_element_ids.len() != 1
                || correspondence.child_source_element_ids.len() < 2
                || !proof_sets_equal(&correspondence.parent_source_element_ids, &source_edge_ids)
                || !proof_sets_equal(
                    &correspondence.child_source_element_ids,
                    &generated_edge_ids,
                )
                || !proof_sets_equal(&retired_edge_ids, &source_edge_ids)
            {
                return Err(invalid("split topology proof correspondence is invalid"));
            }
        }
        "collapse_edge" => {
            if correspondence.kind != "many-to-one"
                || correspondence.parent_source_element_ids.len() != 2
                || correspondence.child_source_element_ids.len() != 1
                || !proof_sets_equal(
                    &correspondence.parent_source_element_ids,
                    &source_vertex_ids,
                )
                || !source_vertex_ids.contains(&correspondence.child_source_element_ids[0])
                || retired_vertex_ids.len() != 1
                || !source_vertex_ids
                    .iter()
                    .any(|id| !retired_vertex_ids.contains(id))
                || retired_edge_ids.is_empty()
                || !proof_sets_equal(&retired_edge_ids, &source_edge_ids)
            {
                return Err(invalid("collapse topology proof correspondence is invalid"));
            }
        }
        "dissolve_edge" => {
            if correspondence.kind != "many-to-one"
                || correspondence.parent_source_element_ids.len() < 2
                || correspondence.child_source_element_ids.len() != 1
                || !proof_sets_equal(&correspondence.parent_source_element_ids, &source_face_ids)
                || !generated_face_ids.contains(&correspondence.child_source_element_ids[0])
                || !source_face_ids
                    .iter()
                    .all(|id| retired_face_ids.contains(id))
                || retired_edge_ids.is_empty()
                || !proof_sets_equal(&retired_edge_ids, &source_edge_ids)
            {
                return Err(invalid("dissolve topology proof correspondence is invalid"));
            }
        }
        _ => unreachable!(),
    }

    Ok(TopologyOperationProof {
        operation,
        parent_revision,
        child_revision,
        operation_lineage_sha256,
        source_vertex_ids,
        source_edge_ids,
        source_face_ids,
        generated_vertex_ids,
        generated_edge_ids,
        generated_loop_ids,
        generated_face_ids,
        retired_vertex_ids,
        retired_edge_ids,
        retired_loop_ids,
        retired_face_ids,
        tombstones,
        correspondence,
        identity_namespace_status: TOPOLOGY_IDENTITY_NAMESPACE.to_owned(),
        canonical_sha256,
    })
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "AUTHORING_MESH_IDENTITY_LINEAGE_INVALID: {}",
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
    if !is_opaque_id(value) || value.len() > MAX_ID_LENGTH {
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
        Some(Value::String(value)) if is_opaque_id(value) && value.len() <= MAX_ID_LENGTH => {
            Ok(Some(value.clone()))
        }
        _ => Err(invalid(format!("{key} must be a nullable identifier"))),
    }
}

fn nullable_sha(object: &Map<String, Value>, key: &str) -> Result<Option<String>, RuntimeError> {
    match object.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_sha256(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!("{key} must be a nullable SHA-256"))),
    }
}

fn bool_const(object: &Map<String, Value>, key: &str, expected: bool) -> Result<(), RuntimeError> {
    if object.get(key).and_then(Value::as_bool) != Some(expected) {
        return Err(invalid(format!("{key} differs from the identity contract")));
    }
    Ok(())
}

fn canonical_bytes(value: &Value, context: &str) -> Result<Vec<u8>, RuntimeError> {
    let bytes = canonical_json_bytes(value).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() > MAX_CAS_JSON_BYTES as usize {
        return Err(invalid(format!("{context} exceeds the 1 MiB CAS budget")));
    }
    Ok(bytes)
}

fn verify_payload_hash(value: &Value, context: &str) -> Result<String, RuntimeError> {
    let supplied = value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid(format!("{context}.canonical_sha256 is invalid")))?;
    let mut without_hash = value.clone();
    without_hash["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&without_hash) != supplied {
        return Err(invalid(format!(
            "{context}.canonical_sha256 mismatches payload"
        )));
    }
    Ok(supplied.to_owned())
}

fn input_hash(value: &Value, object: &Map<String, Value>) -> Result<String, RuntimeError> {
    let supplied = sha(object, "input_sha256")?.to_owned();
    let mut without_hash = value.clone();
    without_hash["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&without_hash) != supplied {
        return Err(invalid("input_sha256 does not match the closed request"));
    }
    Ok(supplied)
}

fn read_json_object(
    runtime: &Runtime,
    object_sha256: &str,
    kind: &str,
) -> Result<(Value, Vec<u8>), RuntimeError> {
    let record = runtime
        .store
        .get_object(object_sha256)?
        .ok_or_else(|| invalid(format!("CAS object {object_sha256} is unavailable")))?;
    if record.sha256 != object_sha256 || record.mime != JSON_MIME || record.kind != kind {
        return Err(invalid(format!(
            "CAS object {object_sha256} metadata differs"
        )));
    }
    let bytes = runtime.cas_read_bounded(object_sha256, MAX_CAS_JSON_BYTES)?;
    if sha256_hex(&bytes) != object_sha256 {
        return Err(invalid(format!(
            "CAS object {object_sha256} hash readback differs"
        )));
    }
    let value = serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
    Ok((value, bytes))
}

fn check_identity_payload(value: &Value) -> Result<&Map<String, Value>, RuntimeError> {
    let object = exact_object(value, IDENTITY_FIELDS, IDENTITY_SCHEMA)?;
    if text(object, "schema_version")? != IDENTITY_SCHEMA
        || text(object, "identity_policy")? != IDENTITY_POLICY
        || text(object, "evaluated_identity_policy")? != EVALUATED_IDENTITY_POLICY
        || text(object, "writer_policy")? != WRITER_POLICY
        || text(object, "canonicalization_policy")? != CANONICALIZATION_POLICY
        || text(object, "quality_status")? != QUALITY_STATUS
    {
        return Err(invalid("identity payload policy differs"));
    }
    for key in [
        "lineage_id",
        "project_id",
        "authoring_node_id",
        "part_id",
        "candidate_id",
    ] {
        identifier(object, key)?;
    }
    for key in [
        "genesis_source_mesh_sha256",
        "current_source_mesh_sha256",
        "candidate_state_sha256",
        "operation_lineage_sha256",
        "source_program_object_sha256",
        "source_program_sha256",
        "source_artifact_object_sha256",
        "source_artifact_sha256",
        "source_artifact_readback_object_sha256",
        "source_artifact_readback_sha256",
    ] {
        sha(object, key)?;
    }
    nullable_identifier(object, "base_version_id")?;
    nullable_sha(object, "parent_lineage_object_sha256")?;
    nullable_sha(object, "parent_lineage_sha256")?;
    let revision_index = object
        .get("revision_index")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("revision_index is invalid"))?;
    if revision_index > 1_000_000
        || !matches!(
            text(object, "revision_kind")?,
            "genesis" | "preserving-edit" | "topology-edit"
        )
    {
        return Err(invalid("identity revision metadata is invalid"));
    }
    for (key, expected) in [
        ("runtime_write_performed", true),
        ("persistent_user_data_touched", true),
        ("stage_advanced", false),
        ("candidate_confirmed", false),
        ("version_created", false),
        ("export_performed", false),
    ] {
        bool_const(object, key, expected)?;
    }
    let budgets = object
        .get("budgets")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("identity budgets are missing"))?;
    if budgets.get("max_response_bytes").and_then(Value::as_u64) != Some(1_048_576)
        || budgets.get("max_elements").and_then(Value::as_u64) != Some(32_768)
        || budgets.get("max_tombstones").and_then(Value::as_u64) != Some(32_768)
        || budgets.get("max_parent_ids").and_then(Value::as_u64) != Some(8)
        || budgets.get("max_evaluated_ids").and_then(Value::as_u64) != Some(64)
    {
        return Err(invalid("identity budgets differ"));
    }
    let elements = object
        .get("elements")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("identity elements are missing"))?;
    if elements.is_empty() || elements.len() > MAX_ELEMENTS {
        return Err(invalid("identity elements exceed the bounded range"));
    }
    let mut active = BTreeSet::new();
    for value in elements {
        let element = exact_object(
            value,
            &[
                "identity_id",
                "element_kind",
                "source_element_id",
                "origin",
                "stability_status",
                "parent_identity_ids",
                "operation_lineage_sha256",
                "role",
                "ordinal",
            ],
            "identity element",
        )?;
        let id = identifier(element, "identity_id")?.to_owned();
        if !active.insert(id) {
            return Err(invalid("identity active ID is duplicated"));
        }
        if !matches!(
            text(element, "element_kind")?,
            "vertex" | "edge" | "half-edge" | "corner" | "face" | "loop" | "ring" | "boundary"
        ) || !matches!(
            text(element, "origin")?,
            "authored" | "operation-derived" | "representation-derived"
        ) || !matches!(
            text(element, "stability_status")?,
            "cross-revision-stable" | "same-revision-only"
        ) {
            return Err(invalid("identity element metadata is invalid"));
        }
        match element.get("source_element_id") {
            Some(Value::Null) => {}
            Some(Value::String(value)) if is_opaque_id(value) => {}
            _ => return Err(invalid("identity source element ID is invalid")),
        }
        let parents = element
            .get("parent_identity_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("identity element parents are missing"))?;
        if parents.len() > MAX_PARENT_IDS {
            return Err(invalid("identity element has too many parents"));
        }
        let mut parent_set = BTreeSet::new();
        for parent in parents {
            let parent = parent
                .as_str()
                .filter(|value| is_opaque_id(value))
                .ok_or_else(|| invalid("identity parent ID is invalid"))?;
            if !parent_set.insert(parent) {
                return Err(invalid("identity parent ID is duplicated"));
            }
        }
        sha(element, "operation_lineage_sha256")?;
        identifier(element, "role")?;
        if element
            .get("ordinal")
            .and_then(Value::as_u64)
            .filter(|value| *value <= 32_767)
            .is_none()
        {
            return Err(invalid("identity ordinal is invalid"));
        }
    }
    let tombstones = object
        .get("tombstones")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("identity tombstones are missing"))?;
    if tombstones.len() > MAX_ELEMENTS {
        return Err(invalid("identity tombstones exceed the bounded range"));
    }
    let mut retired = BTreeSet::new();
    for value in tombstones {
        let tombstone = exact_object(
            value,
            &[
                "identity_id",
                "element_kind",
                "retired_revision_index",
                "operation_lineage_sha256",
                "reason",
            ],
            "identity tombstone",
        )?;
        let id = identifier(tombstone, "identity_id")?.to_owned();
        if !retired.insert(id.clone()) || active.contains(&id) {
            return Err(invalid("identity tombstone is duplicated or reused"));
        }
        if tombstone
            .get("retired_revision_index")
            .and_then(Value::as_u64)
            .filter(|value| (1..=1_000_000).contains(value))
            .is_none()
            || !matches!(
                text(tombstone, "element_kind")?,
                "vertex" | "edge" | "half-edge" | "corner" | "face" | "loop" | "ring" | "boundary"
            )
            || !matches!(
                text(tombstone, "reason")?,
                "deleted" | "collapsed" | "dissolved" | "replaced" | "merged"
            )
        {
            return Err(invalid("identity tombstone metadata is invalid"));
        }
        sha(tombstone, "operation_lineage_sha256")?;
    }
    let correspondence = object
        .get("correspondence")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("identity correspondence is missing"))?;
    if correspondence.len() > MAX_ELEMENTS {
        return Err(invalid("identity correspondence exceeds the bounded range"));
    }
    for value in correspondence {
        let item = exact_object(
            value,
            &[
                "kind",
                "parent_identity_ids",
                "child_identity_ids",
                "operation_lineage_sha256",
            ],
            "identity correspondence",
        )?;
        if !matches!(
            text(item, "kind")?,
            "preserved" | "created" | "retired" | "one-to-many" | "many-to-one" | "many-to-many"
        ) {
            return Err(invalid("identity correspondence kind is invalid"));
        }
        for (key, maximum) in [
            ("parent_identity_ids", 8usize),
            ("child_identity_ids", 64usize),
        ] {
            let ids = item
                .get(key)
                .and_then(Value::as_array)
                .ok_or_else(|| invalid(format!("{key} is missing")))?;
            if ids.len() > maximum {
                return Err(invalid(format!("{key} exceeds its budget")));
            }
            let mut seen = BTreeSet::new();
            for id in ids {
                let id = id
                    .as_str()
                    .filter(|value| is_opaque_id(value))
                    .ok_or_else(|| invalid(format!("{key} contains an invalid ID")))?;
                if !seen.insert(id) {
                    return Err(invalid(format!("{key} contains a duplicate ID")));
                }
            }
        }
        sha(item, "operation_lineage_sha256")?;
    }
    verify_payload_hash(value, IDENTITY_SCHEMA)?;
    Ok(object)
}

fn source_entries(
    projection: &Value,
    array_key: &str,
    id_key: &str,
    element_kind: &str,
) -> Result<Vec<(String, String)>, RuntimeError> {
    let values = projection
        .get(array_key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("projection.{array_key} is missing")))?;
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        let object = value
            .as_object()
            .ok_or_else(|| invalid(format!("projection.{array_key} entry is invalid")))?;
        let lineage = object
            .get("lineage")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("projection element lineage is missing"))?;
        let source_id = lineage
            .get("original_element_ids")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("projection original source element ID is invalid"))?;
        let _projection_id = object
            .get(id_key)
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| invalid("projection element ID is invalid"))?;
        result.push((element_kind.to_owned(), source_id.to_owned()));
    }
    result.sort();
    Ok(result)
}

fn source_loop_entries(projection: &Value) -> Result<Vec<(String, String)>, RuntimeError> {
    // The public projection's `loops` array is one face-cycle record, while
    // topology proofs retire/create the authored source loops that back each
    // half-edge.  Reuse the explicit half-edge lineage to keep that namespace
    // closed and avoid guessing a face-to-loop relation.
    source_entries(projection, "half_edges", "half_edge_id", "loop")
}

fn parent_source_identity_index(
    parent: Option<&Value>,
) -> Result<(BTreeMap<(String, String), Value>, BTreeSet<String>), RuntimeError> {
    let Some(parent) = parent else {
        return Ok((BTreeMap::new(), BTreeSet::new()));
    };
    let object = check_identity_payload(parent)?;
    let elements = object
        .get("elements")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("identity parent elements are missing"))?;
    let mut by_source = BTreeMap::new();
    let mut active_ids = BTreeSet::new();
    for value in elements {
        let element = value
            .as_object()
            .ok_or_else(|| invalid("identity parent element must be an object"))?;
        let identity_id = identifier(element, "identity_id")?.to_owned();
        if !active_ids.insert(identity_id) {
            return Err(invalid("identity parent active ID is duplicated"));
        }
        let Some(source_element_id) = element
            .get("source_element_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
        else {
            continue;
        };
        let key = (
            text(element, "element_kind")?.to_owned(),
            source_element_id.to_owned(),
        );
        if by_source.insert(key, value.clone()).is_some() {
            return Err(invalid(
                "identity parent source element binding is duplicated",
            ));
        }
    }
    Ok((by_source, active_ids))
}

fn topology_generated_ids<'a>(
    proof: &'a TopologyOperationProof,
    element_kind: &str,
) -> &'a [String] {
    match element_kind {
        "vertex" => &proof.generated_vertex_ids,
        "edge" => &proof.generated_edge_ids,
        "loop" => &proof.generated_loop_ids,
        "face" => &proof.generated_face_ids,
        _ => &[],
    }
}

fn topology_source_ids<'a>(proof: &'a TopologyOperationProof, element_kind: &str) -> &'a [String] {
    match element_kind {
        "vertex" => &proof.source_vertex_ids,
        "edge" => &proof.source_edge_ids,
        "face" => &proof.source_face_ids,
        _ => &[],
    }
}

fn topology_retired_ids<'a>(proof: &'a TopologyOperationProof, element_kind: &str) -> &'a [String] {
    match element_kind {
        "vertex" => &proof.retired_vertex_ids,
        "edge" => &proof.retired_edge_ids,
        "loop" => &proof.retired_loop_ids,
        "face" => &proof.retired_face_ids,
        _ => &[],
    }
}

fn topology_generated_parent_spec<'a>(
    proof: &'a TopologyOperationProof,
    element_kind: &str,
) -> Result<(&'a str, &'a [String], &'static str), RuntimeError> {
    match (proof.operation.as_str(), element_kind) {
        ("split_edge", "vertex") => Ok(("edge", &proof.source_edge_ids, "split-midpoint")),
        ("split_edge", "edge") => Ok(("edge", &proof.source_edge_ids, "split-child-edge")),
        ("split_edge", "loop") => Ok(("face", &proof.source_face_ids, "split-face-loop")),
        ("collapse_edge", "edge") => {
            Ok(("vertex", &proof.source_vertex_ids, "collapse-child-edge"))
        }
        ("collapse_edge", "loop") => Ok(("face", &proof.source_face_ids, "collapse-face-loop")),
        ("dissolve_edge", "face") => Ok(("face", &proof.retired_face_ids, "dissolve-face")),
        ("dissolve_edge", "loop") => Ok(("face", &proof.retired_face_ids, "dissolve-face-loop")),
        _ => Err(invalid(format!(
            "topology proof has no generated identity rule for {element_kind}"
        ))),
    }
}

fn operation_derived_identity(
    proof: &TopologyOperationProof,
    lineage_id: &str,
    element_kind: &str,
    source_element_id: &str,
    generated_ids: &[String],
    parent_source: &BTreeMap<(String, String), Value>,
) -> Result<Value, RuntimeError> {
    let (parent_kind, parent_source_ids, role) =
        topology_generated_parent_spec(proof, element_kind)?;
    if parent_source_ids.is_empty() || parent_source_ids.len() > MAX_PARENT_IDS {
        return Err(invalid(
            "topology generated identity parent set is outside the bounded range",
        ));
    }
    let mut parent_identity_ids = Vec::with_capacity(parent_source_ids.len());
    for parent_source_id in parent_source_ids {
        let parent_value = parent_source
            .get(&(parent_kind.to_owned(), parent_source_id.clone()))
            .ok_or_else(|| {
                invalid(format!(
                    "topology proof parent source {parent_kind}:{parent_source_id} is not active"
                ))
            })?;
        let parent_object = parent_value
            .as_object()
            .ok_or_else(|| invalid("identity parent element must be an object"))?;
        parent_identity_ids.push(identifier(parent_object, "identity_id")?.to_owned());
    }
    parent_identity_ids.sort();
    parent_identity_ids.dedup();
    if parent_identity_ids.is_empty() || parent_identity_ids.len() > MAX_PARENT_IDS {
        return Err(invalid(
            "topology generated identity has no bounded parent identity set",
        ));
    }
    let ordinal = generated_ids
        .iter()
        .position(|value| value == source_element_id)
        .ok_or_else(|| invalid("topology generated identity source is not declared"))?;
    let identity_id = authoring_mesh_identity::generated_identity_id(
        lineage_id,
        element_kind,
        &proof.operation_lineage_sha256,
        &parent_identity_ids,
        role,
        ordinal,
    )?;
    Ok(json!({
        "identity_id":identity_id,
        "element_kind":element_kind,
        "source_element_id":source_element_id,
        "origin":"operation-derived",
        "stability_status":"cross-revision-stable",
        "parent_identity_ids":parent_identity_ids,
        "operation_lineage_sha256":proof.operation_lineage_sha256,
        "role":role,
        "ordinal":ordinal,
    }))
}

fn build_elements(
    source: &SourceTruth,
    lineage_id: &str,
    operation_lineage_sha256: &str,
    parent: Option<&Value>,
    topology: Option<&TopologyOperationProof>,
) -> Result<Vec<Value>, RuntimeError> {
    let entries = [
        source_entries(&source.projection, "vertices", "vertex_id", "vertex")?,
        source_entries(&source.projection, "edges", "edge_id", "edge")?,
        source_entries(&source.projection, "faces", "face_id", "face")?,
        source_loop_entries(&source.projection)?,
        source_entries(
            &source.projection,
            "half_edges",
            "half_edge_id",
            "half-edge",
        )?,
        source_entries(&source.projection, "corners", "corner_id", "corner")?,
        source_entries(&source.projection, "rings", "ring_id", "ring")?,
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    if entries.is_empty() || entries.len() > MAX_ELEMENTS {
        return Err(invalid("identity element budget is exceeded"));
    }
    let (parent_source, parent_active_ids) = parent_source_identity_index(parent)?;
    let primary_entries = entries
        .iter()
        .filter(|(kind, _)| matches!(kind.as_str(), "vertex" | "edge" | "face" | "loop"))
        .cloned()
        .collect::<Vec<_>>();
    let mut current_primary = BTreeSet::new();
    for (kind, source_id) in &primary_entries {
        if !current_primary.insert((kind.clone(), source_id.clone())) {
            return Err(invalid("identity current source element is duplicated"));
        }
    }
    if let Some(proof) = topology {
        for kind in ["vertex", "edge", "face", "loop"] {
            for source_id in topology_source_ids(proof, kind) {
                if !parent_source.contains_key(&(kind.to_owned(), source_id.clone())) {
                    return Err(invalid(format!(
                        "topology proof source {kind}:{source_id} is not active in the parent"
                    )));
                }
            }
            for source_id in topology_retired_ids(proof, kind) {
                if !parent_source.contains_key(&(kind.to_owned(), source_id.clone()))
                    || current_primary.contains(&(kind.to_owned(), source_id.clone()))
                {
                    return Err(invalid(format!(
                        "topology proof retired {kind}:{source_id} is missing or reappeared"
                    )));
                }
            }
            for source_id in topology_generated_ids(proof, kind) {
                if !current_primary.contains(&(kind.to_owned(), source_id.clone()))
                    || parent_source.contains_key(&(kind.to_owned(), source_id.clone()))
                {
                    return Err(invalid(format!(
                        "topology proof generated {kind}:{source_id} is missing or reuses a parent source"
                    )));
                }
            }
        }
    }

    let mut primary_values = BTreeMap::<(String, String), Value>::new();
    for (kind, source_id) in primary_entries {
        let generated_ids = topology.map(|proof| topology_generated_ids(proof, &kind));
        let is_generated = generated_ids
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|id| id == &source_id));
        let value = if is_generated {
            let proof = topology.expect("generated topology element has proof");
            let value = operation_derived_identity(
                proof,
                lineage_id,
                &kind,
                &source_id,
                generated_ids.expect("generated IDs"),
                &parent_source,
            )?;
            if parent_active_ids.contains(
                value
                    .get("identity_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("generated identity ID is missing"))?,
            ) {
                return Err(invalid(
                    "topology generated identity reuses a parent identity",
                ));
            }
            value
        } else if let Some(parent_value) = parent_source.get(&(kind.clone(), source_id.clone())) {
            let mut preserved = parent_value.clone();
            preserved["operation_lineage_sha256"] =
                Value::String(operation_lineage_sha256.to_owned());
            preserved
        } else if topology.is_some() {
            return Err(invalid(format!(
                "typed topology current source {kind}:{source_id} is not parent-preserved or generated"
            )));
        } else {
            let identity_id =
                authoring_mesh_identity::authored_identity_id(lineage_id, &kind, &source_id)?;
            json!({
                "identity_id":identity_id,
                "element_kind":kind,
                "source_element_id":source_id,
                "origin":"authored",
                "stability_status":"cross-revision-stable",
                "parent_identity_ids":[],
                "operation_lineage_sha256":operation_lineage_sha256,
                "role":"authored-source",
                "ordinal":0,
            })
        };
        let key = (kind, source_id);
        if primary_values.insert(key, value).is_some() {
            return Err(invalid("identity primary source element is duplicated"));
        }
    }

    let mut ordinals = BTreeMap::<String, usize>::new();
    let mut result = primary_values.into_values().collect::<Vec<_>>();
    for (kind, source_id) in entries
        .into_iter()
        .filter(|(kind, _)| matches!(kind.as_str(), "half-edge" | "corner" | "ring"))
    {
        let ordinal_key = kind.clone();
        let ordinal = *ordinals.entry(ordinal_key.clone()).or_default();
        *ordinals.entry(ordinal_key).or_default() += 1;
        let (parent_kind, role) = match kind.as_str() {
            "half-edge" | "corner" => ("loop", format!("authoring-{kind}-projection")),
            "ring" => ("edge", "boundary-ring-projection".to_owned()),
            _ => unreachable!(),
        };
        let parent_id = if let Some(parent_value) = result.iter().find(|value| {
            value.get("element_kind").and_then(Value::as_str) == Some(parent_kind)
                && value.get("source_element_id").and_then(Value::as_str)
                    == Some(source_id.as_str())
        }) {
            parent_value
                .get("identity_id")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("representation parent identity ID is missing"))?
                .to_owned()
        } else if parent.is_some() {
            return Err(invalid(format!(
                "representation {kind}:{source_id} has no current {parent_kind} source"
            )));
        } else {
            authoring_mesh_identity::authored_identity_id(lineage_id, parent_kind, &source_id)?
        };
        let operation = canonical_json_hash(&json!({
            "schema_version": IDENTITY_SCHEMA,
            "lineage_id": lineage_id,
            "element_kind": kind,
            "source_element_id": source_id,
            "representation": "authoring-half-edge-projection@1",
        }));
        let identity_id = authoring_mesh_identity::generated_identity_id(
            lineage_id,
            &kind,
            &operation,
            std::slice::from_ref(&parent_id),
            &role,
            ordinal,
        )?;
        result.push(json!({
            "identity_id":identity_id,
            "element_kind":kind,
            "source_element_id":Value::Null,
            "origin":"representation-derived",
            "stability_status":"same-revision-only",
            "parent_identity_ids":[parent_id],
            "operation_lineage_sha256":operation_lineage_sha256,
            "role":"representation-projection",
            "ordinal":ordinal,
        }));
    }
    result.sort_by(|left, right| {
        left.get("identity_id")
            .and_then(Value::as_str)
            .cmp(&right.get("identity_id").and_then(Value::as_str))
    });
    Ok(result)
}

fn read_parent(
    runtime: &Runtime,
    object_sha256: Option<&str>,
    canonical_sha256: Option<&str>,
) -> Result<Option<ParentTruth>, RuntimeError> {
    match (object_sha256, canonical_sha256) {
        (None, None) => Ok(None),
        (Some(object_sha256), Some(canonical_sha256)) => {
            let (payload, bytes) = read_json_object(runtime, object_sha256, IDENTITY_OBJECT_KIND)?;
            if sha256_hex(&bytes) != object_sha256
                || payload.get("canonical_sha256").and_then(Value::as_str) != Some(canonical_sha256)
            {
                return Err(invalid("identity parent object/hash differs"));
            }
            check_identity_payload(&payload)?;
            Ok(Some(ParentTruth {
                object_sha256: object_sha256.to_owned(),
                payload,
            }))
        }
        _ => Err(invalid(
            "parent identity object and semantic hash must be both null or present",
        )),
    }
}

fn load_source(
    runtime: &Runtime,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    base_version_id: Option<&str>,
    authoring_node_id: &str,
    part_id: &str,
    source_program_object_sha256: &str,
    source_program_sha256: &str,
    source_artifact_id: &str,
    source_artifact_object_sha256: &str,
    source_artifact_sha256: &str,
    source_artifact_readback_object_sha256: &str,
    source_artifact_readback_sha256: &str,
    source_lineage_sha256: &str,
    canonical_mesh_id: &str,
    canonical_mesh_object_sha256: &str,
    canonical_mesh_sha256: &str,
    current_source_mesh_sha256: &str,
) -> Result<SourceTruth, RuntimeError> {
    let record = runtime
        .store
        .get_authoring_mesh_durable_record_by_mesh(candidate_id, canonical_mesh_id)?
        .ok_or_else(|| invalid("durable AuthoringMesh source record is unavailable"))?;
    if record.project_id != project_id
        || record.candidate_id != candidate_id
        || record.candidate_state_sha256 != candidate_state_sha256
        || record.base_version_id.as_deref() != base_version_id
        || record.canonical_mesh_id != canonical_mesh_id
        || record.canonical_mesh_object_sha256 != canonical_mesh_object_sha256
        || record.canonical_mesh_sha256 != canonical_mesh_sha256
        || record.authoring_node_id != authoring_node_id
        || record.part_id != part_id
        || record.source_program_object_sha256 != source_program_object_sha256
        || record.source_program_sha256 != source_program_sha256
        || record.source_artifact_object_sha256 != source_artifact_object_sha256
        || record.source_artifact_sha256 != source_artifact_sha256
        || record.source_artifact_readback_object_sha256 != source_artifact_readback_object_sha256
        || record.source_artifact_readback_sha256 != source_artifact_readback_sha256
    {
        return Err(invalid(
            "identity source request does not match durable AuthoringMesh",
        ));
    }
    let candidate = runtime
        .candidate(candidate_id)?
        .ok_or_else(|| invalid("identity source candidate is unavailable"))?;
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .ok_or_else(|| invalid("identity source geometry evidence is unavailable"))?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != record.candidate_state_sha256
        || candidate.base_version_id != record.base_version_id
        || candidate.prepared_object_id.as_deref() != Some(source_artifact_id)
        || candidate.prepared_object_sha256.as_deref() != Some(source_artifact_object_sha256)
        || evidence.project_id != project_id
        || evidence.candidate_id != candidate_id
        || evidence.geometry_program_object_sha256 != source_program_object_sha256
        || evidence.geometry_program_sha256 != source_program_sha256
        || evidence.artifact_object_sha256 != source_artifact_object_sha256
        || evidence.artifact_readback_object_sha256 != source_artifact_readback_object_sha256
    {
        return Err(invalid(
            "identity candidate/program/artifact/readback binding differs",
        ));
    }
    let (canonical, _) = read_json_object(
        runtime,
        canonical_mesh_object_sha256,
        AUTHORING_MESH_CANONICAL_OBJECT_KIND,
    )?;
    if canonical.get("schema_version").and_then(Value::as_str) != Some("AuthoringMeshCanonical@1")
        || canonical.get("canonical_mesh_id").and_then(Value::as_str) != Some(canonical_mesh_id)
        || canonical.get("project_id").and_then(Value::as_str) != Some(project_id)
        || canonical.get("candidate_id").and_then(Value::as_str) != Some(candidate_id)
        || canonical
            .get("candidate_state_sha256")
            .and_then(Value::as_str)
            != Some(candidate_state_sha256)
        || canonical
            .get("source_lineage_sha256")
            .and_then(Value::as_str)
            != Some(source_lineage_sha256)
        || canonical.get("canonical_sha256").and_then(Value::as_str) != Some(canonical_mesh_sha256)
    {
        return Err(invalid("identity canonical source readback differs"));
    }
    let projection_index = runtime
        .store
        .get_authoring_mesh_projection_index(candidate_id, canonical_mesh_id)?
        .ok_or_else(|| invalid("identity source projection index is unavailable"))?;
    if projection_index.project_id != project_id
        || projection_index.candidate_id != candidate_id
        || projection_index.candidate_state_sha256 != candidate_state_sha256
        || projection_index.mesh_id != canonical_mesh_id
        || projection_index.artifact_id != source_artifact_object_sha256
        || projection_index.artifact_sha256 != source_artifact_object_sha256
        || projection_index.program_sha256 != source_program_sha256
        || projection_index.authoring_node_id != authoring_node_id
        || projection_index.part_id != part_id
    {
        return Err(invalid("identity projection source binding differs"));
    }
    let (projection, _) = read_json_object(
        runtime,
        &projection_index.mesh_object_sha256,
        "authoring-mesh",
    )?;
    let projection_lineage = projection
        .get("lineage")
        .and_then(|value| value.get("lineage_sha256"))
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("identity projection lineage is invalid"))?;
    let source_mesh_sha256 = projection
        .get("mesh_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("identity projection source mesh hash is invalid"))?;
    if projection_lineage != source_lineage_sha256
        || source_mesh_sha256 != current_source_mesh_sha256
        || projection.get("mesh_id").and_then(Value::as_str) != Some(canonical_mesh_id)
    {
        return Err(invalid("identity projection source hash/lineage differs"));
    }
    Ok(SourceTruth {
        record,
        canonical,
        projection,
        projection_index,
        source_artifact_id: source_artifact_id.to_owned(),
        current_source_mesh_sha256: current_source_mesh_sha256.to_owned(),
        source_lineage_sha256: source_lineage_sha256.to_owned(),
    })
}

fn make_payload(
    source: &SourceTruth,
    lineage_id: &str,
    genesis_source_mesh_sha256: &str,
    parent: Option<&ParentTruth>,
    revision_index: u64,
    operation_lineage_sha256: &str,
    topology: Option<&TopologyOperationProof>,
) -> Result<Value, RuntimeError> {
    if revision_index == 0 && parent.is_some() {
        return Err(invalid("genesis identity revision has a parent"));
    }
    if revision_index > 0 {
        let parent = parent.ok_or_else(|| invalid("non-genesis identity parent is missing"))?;
        let object = check_identity_payload(&parent.payload)?;
        if text(object, "lineage_id")? != lineage_id
            || text(object, "project_id")? != source.record.project_id
            || text(object, "authoring_node_id")? != source.record.authoring_node_id
            || text(object, "part_id")? != source.record.part_id
            || sha(object, "genesis_source_mesh_sha256")? != genesis_source_mesh_sha256
            || object.get("revision_index").and_then(Value::as_u64) != Some(revision_index - 1)
        {
            return Err(invalid("identity parent lineage differs"));
        }
    }
    let elements = build_elements(
        source,
        lineage_id,
        operation_lineage_sha256,
        parent.map(|value| &value.payload),
        topology,
    )?;
    let (revision_kind, tombstones, correspondence) = lineage_delta(
        parent.map(|value| &value.payload),
        &elements,
        revision_index,
        operation_lineage_sha256,
        topology,
    )?;
    let mut payload = json!({
        "schema_version": IDENTITY_SCHEMA,
        "lineage_id": lineage_id,
        "project_id": source.record.project_id,
        "authoring_node_id": source.record.authoring_node_id,
        "part_id": source.record.part_id,
        "genesis_source_mesh_sha256": genesis_source_mesh_sha256,
        "current_source_mesh_sha256": source.current_source_mesh_sha256,
        "candidate_id": source.record.candidate_id,
        "candidate_state_sha256": source.record.candidate_state_sha256,
        "base_version_id": source.record.base_version_id,
        "parent_lineage_object_sha256": parent.map(|value| value.object_sha256.clone()),
        "parent_lineage_sha256": parent.map(|value| value.payload["canonical_sha256"].clone()),
        "revision_index": revision_index,
        "revision_kind": revision_kind,
        "operation_lineage_sha256": operation_lineage_sha256,
        "source_program_object_sha256": source.record.source_program_object_sha256,
        "source_program_sha256": source.record.source_program_sha256,
        "source_artifact_object_sha256": source.record.source_artifact_object_sha256,
        "source_artifact_sha256": source.record.source_artifact_sha256,
        "source_artifact_readback_object_sha256": source.record.source_artifact_readback_object_sha256,
        "source_artifact_readback_sha256": source.record.source_artifact_readback_sha256,
        "elements": elements,
        "tombstones": tombstones,
        "correspondence": correspondence,
        "identity_policy": IDENTITY_POLICY,
        "evaluated_identity_policy": EVALUATED_IDENTITY_POLICY,
        "budgets": {
            "max_response_bytes": 1_048_576,
            "max_elements": 32_768,
            "max_tombstones": 32_768,
            "max_parent_ids": 8,
            "max_evaluated_ids": 64,
        },
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "runtime_write_performed": true,
        "persistent_user_data_touched": true,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": QUALITY_STATUS,
        "canonical_sha256": "",
    });
    exact_object(&payload, IDENTITY_FIELDS, IDENTITY_SCHEMA)?;
    payload["canonical_sha256"] = Value::String(canonical_json_hash(&payload));
    check_identity_payload(&payload)?;
    Ok(payload)
}

/// Derive the bounded authoring identity delta from the previous immutable
/// ledger and the current Runtime-owned source projection.  The caller cannot
/// provide this data: preserving/created/retired entries and monotonic
/// tombstones are all derived from the two identity sets here.
///
/// Untyped revisions emit only one-to-one relations.  Typed topology
/// correspondence is accepted only from the Runtime-owned proof recovered
/// from the exact staged candidate; it is never inferred from mesh bytes.
fn lineage_delta(
    parent: Option<&Value>,
    elements: &[Value],
    revision_index: u64,
    operation_lineage_sha256: &str,
    topology: Option<&TopologyOperationProof>,
) -> Result<(String, Value, Value), RuntimeError> {
    let Some(parent) = parent else {
        if topology.is_some() {
            return Err(invalid(
                "typed topology identity materialization requires a parent revision",
            ));
        }
        return Ok((
            "genesis".to_owned(),
            Value::Array(Vec::new()),
            Value::Array(Vec::new()),
        ));
    };
    let parent_object = check_identity_payload(parent)?;
    let parent_elements = parent_object
        .get("elements")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("identity parent elements are missing"))?;
    let parent_tombstones = parent_object
        .get("tombstones")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("identity parent tombstones are missing"))?;

    let (parent_source, _) = parent_source_identity_index(Some(parent))?;

    let identity_entry = |value: &Value| -> Result<(String, String), RuntimeError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("identity delta element must be an object"))?;
        Ok((
            identifier(object, "identity_id")?.to_owned(),
            text(object, "element_kind")?.to_owned(),
        ))
    };

    let mut previous = BTreeMap::<String, String>::new();
    for value in parent_elements {
        let (identity_id, element_kind) = identity_entry(value)?;
        if previous.insert(identity_id, element_kind).is_some() {
            return Err(invalid("identity parent active ID is duplicated"));
        }
    }
    let mut retired = BTreeSet::new();
    for value in parent_tombstones {
        let object = value
            .as_object()
            .ok_or_else(|| invalid("identity parent tombstone must be an object"))?;
        let identity_id = identifier(object, "identity_id")?;
        if !retired.insert(identity_id.to_owned()) || previous.contains_key(identity_id) {
            return Err(invalid("identity parent tombstone is duplicated or reused"));
        }
    }

    let mut current = BTreeMap::<String, String>::new();
    let mut current_source = BTreeMap::<(String, String), String>::new();
    for value in elements {
        let (identity_id, element_kind) = identity_entry(value)?;
        if retired.contains(&identity_id) {
            return Err(invalid(
                "a historical tombstoned identity cannot reappear in an active revision",
            ));
        }
        if current.insert(identity_id, element_kind).is_some() {
            return Err(invalid("identity current active ID is duplicated"));
        }
        let element = value
            .as_object()
            .ok_or_else(|| invalid("identity delta element must be an object"))?;
        if let Some(source_element_id) = element
            .get("source_element_id")
            .and_then(Value::as_str)
            .filter(|value| is_opaque_id(value))
        {
            let key = (
                text(element, "element_kind")?.to_owned(),
                source_element_id.to_owned(),
            );
            if current_source
                .insert(key, identifier(element, "identity_id")?.to_owned())
                .is_some()
            {
                return Err(invalid("identity current source binding is duplicated"));
            }
        }
    }

    let mut tombstones = parent_tombstones.to_vec();
    let mut correspondence = Vec::new();
    let mut typed_parent_identity_ids = BTreeSet::new();
    let mut typed_retired_identity_ids = BTreeSet::new();
    let mut typed_main_relation_parent_ids = BTreeSet::new();
    let mut typed_child_identity_ids = BTreeSet::new();
    let mut typed_tombstone_identity_ids = BTreeSet::new();
    let mut typed_correspondence = None;
    if let Some(proof) = topology {
        if proof.parent_revision + 1 != revision_index
            || proof.child_revision != revision_index
            || proof.operation_lineage_sha256 != operation_lineage_sha256
        {
            return Err(invalid(
                "typed topology proof does not match identity revision metadata",
            ));
        }
        let source_identity = |index: &BTreeMap<(String, String), Value>,
                               kind: &str,
                               source_id: &str|
         -> Result<String, RuntimeError> {
            let value = index
                .get(&(kind.to_owned(), source_id.to_owned()))
                .ok_or_else(|| {
                    invalid(format!(
                        "typed topology source {kind}:{source_id} is not active in the parent"
                    ))
                })?;
            let object = value
                .as_object()
                .ok_or_else(|| invalid("typed topology parent identity is invalid"))?;
            Ok(identifier(object, "identity_id")?.to_owned())
        };
        let current_identity = |kind: &str, source_id: &str| -> Result<String, RuntimeError> {
            current_source
                .get(&(kind.to_owned(), source_id.to_owned()))
                .cloned()
                .ok_or_else(|| {
                    invalid(format!(
                        "typed topology child {kind}:{source_id} is not active in the current source"
                    ))
                })
        };
        for kind in ["vertex", "edge", "loop", "face"] {
            for source_id in topology_retired_ids(proof, kind) {
                let identity_id = source_identity(&parent_source, kind, source_id)?;
                if !typed_parent_identity_ids.insert(identity_id.clone())
                    || !typed_retired_identity_ids.insert(identity_id.clone())
                    || !typed_tombstone_identity_ids.insert(identity_id.clone())
                {
                    return Err(invalid("typed topology identity retirement is duplicated"));
                }
            }
        }
        let (relation_parent_kind, relation_child_kind) = match proof.operation.as_str() {
            "split_edge" => ("edge", "edge"),
            "collapse_edge" => ("vertex", "vertex"),
            "dissolve_edge" => ("face", "face"),
            _ => return Err(invalid("typed topology operation is unsupported")),
        };
        let parent_identity_ids = proof
            .correspondence
            .parent_source_element_ids
            .iter()
            .map(|source_id| source_identity(&parent_source, relation_parent_kind, source_id))
            .collect::<Result<Vec<_>, _>>()?;
        let child_identity_ids = proof
            .correspondence
            .child_source_element_ids
            .iter()
            .map(|source_id| current_identity(relation_child_kind, source_id))
            .collect::<Result<Vec<_>, _>>()?;
        for identity_id in &parent_identity_ids {
            typed_parent_identity_ids.insert(identity_id.clone());
        }
        typed_main_relation_parent_ids.extend(parent_identity_ids.iter().cloned());
        for identity_id in &child_identity_ids {
            typed_child_identity_ids.insert(identity_id.clone());
        }
        typed_correspondence = Some(json!({
            "kind":proof.correspondence.kind,
            "parent_identity_ids":parent_identity_ids,
            "child_identity_ids":child_identity_ids,
            "operation_lineage_sha256":operation_lineage_sha256,
        }));
        for item in &proof.tombstones {
            let identity_id =
                source_identity(&parent_source, &item.element_kind, &item.source_element_id)?;
            if !typed_tombstone_identity_ids.contains(&identity_id) {
                return Err(invalid(
                    "typed topology tombstone is not in the retired identity set",
                ));
            }
            tombstones.push(json!({
                "identity_id":identity_id,
                "element_kind":item.element_kind,
                "retired_revision_index":item.retired_revision_index,
                "operation_lineage_sha256":item.operation_lineage_sha256,
                "reason":item.reason,
            }));
        }
    }
    for (identity_id, element_kind) in &previous {
        if topology.is_some() && typed_parent_identity_ids.contains(identity_id) {
            if typed_retired_identity_ids.contains(identity_id)
                && !typed_main_relation_parent_ids.contains(identity_id)
            {
                correspondence.push(json!({
                    "kind": "retired",
                    "parent_identity_ids": [identity_id],
                    "child_identity_ids": [],
                    "operation_lineage_sha256": operation_lineage_sha256,
                }));
            }
            continue;
        }
        if let Some(current_kind) = current.get(identity_id) {
            if current_kind == element_kind {
                correspondence.push(json!({
                    "kind": "preserved",
                    "parent_identity_ids": [identity_id],
                    "child_identity_ids": [identity_id],
                    "operation_lineage_sha256": operation_lineage_sha256,
                }));
            } else {
                return Err(invalid(
                    "an active identity changed element kind without a typed topology operation",
                ));
            }
        } else {
            tombstones.push(json!({
                "identity_id": identity_id,
                "element_kind": element_kind,
                "retired_revision_index": revision_index,
                "operation_lineage_sha256": operation_lineage_sha256,
                "reason": "deleted",
            }));
            correspondence.push(json!({
                "kind": "retired",
                "parent_identity_ids": [identity_id],
                "child_identity_ids": [],
                "operation_lineage_sha256": operation_lineage_sha256,
            }));
        }
    }
    for identity_id in current.keys() {
        if !previous.contains_key(identity_id) && !typed_child_identity_ids.contains(identity_id) {
            correspondence.push(json!({
                "kind": "created",
                "parent_identity_ids": [],
                "child_identity_ids": [identity_id],
                "operation_lineage_sha256": operation_lineage_sha256,
            }));
        }
    }
    if let Some(value) = typed_correspondence {
        correspondence.push(value);
    }
    // Parent tombstones are immutable history.  Re-sort both derived arrays
    // before the payload hash so repeated Runtime/restart replays have one
    // canonical order independent of source projection iteration order.
    tombstones.sort_by(|left, right| {
        left.get("identity_id")
            .and_then(Value::as_str)
            .cmp(&right.get("identity_id").and_then(Value::as_str))
            .then_with(|| {
                left.get("retired_revision_index")
                    .and_then(Value::as_u64)
                    .cmp(&right.get("retired_revision_index").and_then(Value::as_u64))
            })
    });
    correspondence.sort_by(|left, right| {
        let left_parent = left
            .get("parent_identity_ids")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .unwrap_or("");
        let right_parent = right
            .get("parent_identity_ids")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .unwrap_or("");
        let left_child = left
            .get("child_identity_ids")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .unwrap_or("");
        let right_child = right
            .get("child_identity_ids")
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .unwrap_or("");
        left_parent
            .cmp(right_parent)
            .then_with(|| left_child.cmp(right_child))
            .then_with(|| {
                left.get("kind")
                    .and_then(Value::as_str)
                    .cmp(&right.get("kind").and_then(Value::as_str))
            })
    });

    let revision_kind = if previous == current {
        "preserving-edit"
    } else {
        "topology-edit"
    };
    Ok((
        revision_kind.to_owned(),
        Value::Array(tombstones),
        Value::Array(correspondence),
    ))
}

fn store_record(
    source: &SourceTruth,
    payload: &Value,
    lineage_id: &str,
    genesis_source_mesh_sha256: &str,
    revision_index: u64,
    revision_kind: &str,
    operation_lineage_sha256: &str,
    parent: Option<&ParentTruth>,
    identity_object_sha256: &str,
    request_input_sha256: &str,
    idempotency_key: &str,
) -> Result<AuthoringMeshIdentityLineageDurableRecord, RuntimeError> {
    let identity_lineage_sha256 = payload
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("identity payload canonical hash is missing"))?;
    Ok(AuthoringMeshIdentityLineageDurableRecord {
        schema_version: AUTHORING_MESH_IDENTITY_LINEAGE_DURABLE_RECORD_SCHEMA_VERSION.to_owned(),
        project_id: source.record.project_id.clone(),
        authoring_node_id: source.record.authoring_node_id.clone(),
        part_id: source.record.part_id.clone(),
        lineage_id: lineage_id.to_owned(),
        genesis_source_mesh_sha256: genesis_source_mesh_sha256.to_owned(),
        current_source_mesh_sha256: source.current_source_mesh_sha256.clone(),
        candidate_id: source.record.candidate_id.clone(),
        candidate_state_sha256: source.record.candidate_state_sha256.clone(),
        base_version_id: source.record.base_version_id.clone(),
        canonical_mesh_id: source.record.canonical_mesh_id.clone(),
        canonical_mesh_object_sha256: source.record.canonical_mesh_object_sha256.clone(),
        canonical_mesh_sha256: source.record.canonical_mesh_sha256.clone(),
        parent_lineage_object_sha256: parent.map(|value| value.object_sha256.clone()),
        parent_lineage_sha256: parent.map(|value| {
            value.payload["canonical_sha256"]
                .as_str()
                .unwrap()
                .to_owned()
        }),
        revision_index,
        revision_kind: revision_kind.to_owned(),
        operation_lineage_sha256: operation_lineage_sha256.to_owned(),
        source_program_object_sha256: source.record.source_program_object_sha256.clone(),
        source_program_sha256: source.record.source_program_sha256.clone(),
        source_artifact_object_sha256: source.record.source_artifact_object_sha256.clone(),
        source_artifact_sha256: source.record.source_artifact_sha256.clone(),
        source_artifact_readback_object_sha256: source
            .record
            .source_artifact_readback_object_sha256
            .clone(),
        source_artifact_readback_sha256: source.record.source_artifact_readback_sha256.clone(),
        operator_catalog_sha256: source.record.operator_catalog_sha256.clone(),
        readback_config_sha256: source.record.readback_config_sha256.clone(),
        evaluated_artifact_object_sha256: source.record.artifact_object_sha256.clone(),
        evaluated_artifact_sha256: source.record.artifact_sha256.clone(),
        evaluated_artifact_readback_object_sha256: source
            .record
            .artifact_readback_object_sha256
            .clone(),
        evaluated_artifact_readback_sha256: source.record.artifact_readback_sha256.clone(),
        identity_lineage_object_sha256: identity_object_sha256.to_owned(),
        identity_lineage_sha256: identity_lineage_sha256.to_owned(),
        request_input_sha256: request_input_sha256.to_owned(),
        idempotency_key: idempotency_key.to_owned(),
        materialization_status: DURABLE_RECORD_STATUS.to_owned(),
        canonical_sha256: String::new(),
        created_at: now_string(),
    })
}

fn result_value(
    schema: &str,
    source: &SourceTruth,
    identity: &AuthoringMeshIdentityLineageDurableRecord,
    payload: Value,
    request_input_sha256: &str,
    replayed: bool,
    prepare: bool,
) -> Result<Value, RuntimeError> {
    let mut result = json!({
        "schema_version": schema,
        "project_id": identity.project_id,
        "source_candidate_id": if prepare { Value::String(identity.candidate_id.clone()) } else { Value::Null },
        "source_candidate_state_sha256": if prepare { Value::String(identity.candidate_state_sha256.clone()) } else { Value::Null },
        "base_version_id": identity.base_version_id,
        "authoring_node_id": identity.authoring_node_id,
        "part_id": identity.part_id,
        "lineage_id": identity.lineage_id,
        "genesis_source_mesh_sha256": identity.genesis_source_mesh_sha256,
        "current_source_mesh_sha256": identity.current_source_mesh_sha256,
        "candidate_id": identity.candidate_id,
        "candidate_state_sha256": identity.candidate_state_sha256,
        "canonical_mesh_id": identity.canonical_mesh_id,
        "canonical_mesh_object_sha256": identity.canonical_mesh_object_sha256,
        "canonical_mesh_sha256": identity.canonical_mesh_sha256,
        "parent_lineage_object_sha256": identity.parent_lineage_object_sha256,
        "parent_lineage_sha256": identity.parent_lineage_sha256,
        "revision_index": identity.revision_index,
        "revision_kind": identity.revision_kind,
        "operation_lineage_sha256": identity.operation_lineage_sha256,
        "source_program_object_sha256": identity.source_program_object_sha256,
        "source_program_sha256": identity.source_program_sha256,
        "source_artifact_object_sha256": identity.source_artifact_object_sha256,
        "source_artifact_sha256": identity.source_artifact_sha256,
        "source_artifact_readback_object_sha256": identity.source_artifact_readback_object_sha256,
        "source_artifact_readback_sha256": identity.source_artifact_readback_sha256,
        "evaluated_artifact_object_sha256": identity.evaluated_artifact_object_sha256,
        "evaluated_artifact_sha256": identity.evaluated_artifact_sha256,
        "evaluated_artifact_readback_object_sha256": identity.evaluated_artifact_readback_object_sha256,
        "evaluated_artifact_readback_sha256": identity.evaluated_artifact_readback_sha256,
        "identity_lineage_object_sha256": identity.identity_lineage_object_sha256,
        "identity_lineage_sha256": identity.identity_lineage_sha256,
        "identity_lineage": payload,
        "request_input_sha256": request_input_sha256,
        "idempotency_key": identity.idempotency_key,
        "replayed": replayed,
        "restart_hash_verified": true,
        "runtime_write_performed": prepare,
        "persistent_user_data_touched": prepare,
        "stage_advanced": false,
        "candidate_confirmed": false,
        "version_created": false,
        "export_performed": false,
        "quality_status": QUALITY_STATUS,
        "limitations": LIMITATIONS,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "canonical_sha256": "",
    });
    if !prepare {
        result
            .as_object_mut()
            .unwrap()
            .remove("source_candidate_id");
        result
            .as_object_mut()
            .unwrap()
            .remove("source_candidate_state_sha256");
    }
    let fields = if prepare {
        PREPARE_RESULT_FIELDS
    } else {
        GET_RESULT_FIELDS
    };
    exact_object(&result, fields, schema)?;
    if canonical_bytes(&result, schema)?.len() > MAX_RESPONSE_BYTES {
        return Err(invalid(format!("{schema} exceeds the response budget")));
    }
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    verify_payload_hash(&result, schema)?;
    let _ = source;
    Ok(result)
}

fn verify_request_source_fields(
    object: &Map<String, Value>,
    source: &SourceTruth,
    project_id: &str,
    candidate_id: &str,
    candidate_state_sha256: &str,
    base_version_id: Option<&str>,
    authoring_node_id: &str,
    part_id: &str,
    source_program_object_sha256: &str,
    source_program_sha256: &str,
    source_artifact_id: &str,
    source_artifact_object_sha256: &str,
    source_artifact_sha256: &str,
    source_artifact_readback_object_sha256: &str,
    source_artifact_readback_sha256: &str,
    source_lineage_sha256: &str,
    canonical_mesh_id: &str,
    canonical_mesh_object_sha256: &str,
    canonical_mesh_sha256: &str,
    current_source_mesh_sha256: &str,
) -> Result<(), RuntimeError> {
    if source.record.project_id != project_id
        || source.record.candidate_id != candidate_id
        || source.record.candidate_state_sha256 != candidate_state_sha256
        || source.record.base_version_id.as_deref() != base_version_id
        || source.record.authoring_node_id != authoring_node_id
        || source.record.part_id != part_id
        || source.record.source_program_object_sha256 != source_program_object_sha256
        || source.record.source_program_sha256 != source_program_sha256
        || source.source_artifact_id != source_artifact_id
        || source.record.source_artifact_object_sha256 != source_artifact_object_sha256
        || source.record.source_artifact_sha256 != source_artifact_sha256
        || source.record.source_artifact_readback_object_sha256
            != source_artifact_readback_object_sha256
        || source.record.source_artifact_readback_sha256 != source_artifact_readback_sha256
        || source.source_lineage_sha256 != source_lineage_sha256
        || source.record.canonical_mesh_id != canonical_mesh_id
        || source.record.canonical_mesh_object_sha256 != canonical_mesh_object_sha256
        || source.record.canonical_mesh_sha256 != canonical_mesh_sha256
        || source.current_source_mesh_sha256 != current_source_mesh_sha256
    {
        return Err(invalid("identity request/source binding differs"));
    }
    let _ = object;
    Ok(())
}

/// Recover the exact Runtime-owned typed edit proof for the target candidate.
/// The identity request carries only the existing operation hash; all proof
/// arrays and tombstones come from the Store row bound to the candidate's
/// request hash and are revalidated here before materialization.
fn topology_proof_for_candidate(
    runtime: &Runtime,
    project_id: &str,
    candidate_id: &str,
    parent: Option<&ParentTruth>,
    operation_lineage_sha256: &str,
    revision_index: u64,
) -> Result<Option<TopologyOperationProof>, RuntimeError> {
    let Some(response_json) = runtime
        .store
        .get_authoring_mesh_edit_topology_response_for_candidate(project_id, candidate_id)?
    else {
        return Ok(None);
    };
    let response: Value = serde_json::from_str(&response_json)
        .map_err(|error| invalid(format!("stored topology edit response is invalid: {error}")))?;
    let response_object = response
        .as_object()
        .ok_or_else(|| invalid("stored topology edit response must be an object"))?;
    if response_object.get("project_id").and_then(Value::as_str) != Some(project_id)
        || response_object
            .get("new_candidate_id")
            .and_then(Value::as_str)
            != Some(candidate_id)
    {
        return Err(invalid(
            "stored topology edit response target candidate binding differs",
        ));
    }
    let parent = parent.ok_or_else(|| {
        invalid("typed topology candidate cannot materialize an identity genesis revision")
    })?;
    let parent_candidate_id = parent
        .payload
        .get("candidate_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("identity parent candidate binding is invalid"))?;
    if response_object
        .get("source_candidate_id")
        .and_then(Value::as_str)
        != Some(parent_candidate_id)
    {
        return Err(invalid(
            "stored topology edit response source candidate differs from identity parent",
        ));
    }
    let edited = response_object
        .get("edited_element_ids")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("stored topology edit response has no edited-element object"))?;
    let proof_value = edited
        .get("typed_operation_proof")
        .ok_or_else(|| invalid("stored topology edit response has no typed operation proof"))?;
    let expected_parent_revision = revision_index
        .checked_sub(1)
        .ok_or_else(|| invalid("typed topology identity revision has no parent index"))?;
    let proof = parse_topology_proof(
        proof_value,
        operation_lineage_sha256,
        expected_parent_revision,
    )?;
    if response_object.get("operation").and_then(Value::as_str) != Some(proof.operation.as_str()) {
        return Err(invalid(
            "stored topology edit operation differs from its proof",
        ));
    }
    Ok(Some(proof))
}

pub(super) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, PREPARE_REQUEST_FIELDS, PREPARE_REQUEST_SCHEMA)?;
    if text(object, "schema_version")? != PREPARE_REQUEST_SCHEMA
        || text(object, "writer_policy")? != WRITER_POLICY
        || text(object, "canonicalization_policy")? != CANONICALIZATION_POLICY
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
    {
        return Err(invalid("identity prepare request policy differs"));
    }
    bool_const(object, "runtime_write_performed", false)?;
    let request_input_sha256 = input_hash(request, object)?;
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
    let canonical_mesh_id = identifier(object, "canonical_mesh_id")?.to_owned();
    let canonical_mesh_object_sha256 = sha(object, "canonical_mesh_object_sha256")?.to_owned();
    let canonical_mesh_sha256 = sha(object, "canonical_mesh_sha256")?.to_owned();
    let genesis_source_mesh_sha256 = sha(object, "genesis_source_mesh_sha256")?.to_owned();
    let current_source_mesh_sha256 = sha(object, "current_source_mesh_sha256")?.to_owned();
    let parent_object_sha256 = nullable_sha(object, "parent_lineage_object_sha256")?;
    let parent_canonical_sha256 = nullable_sha(object, "parent_lineage_sha256")?;
    let operation_lineage_sha256 = sha(object, "operation_lineage_sha256")?.to_owned();
    let expected_lineage_id = nullable_identifier(object, "expected_lineage_id")?;
    let expected_lineage_sha256 = nullable_sha(object, "expected_lineage_sha256")?;
    let idempotency_key = identifier(object, "idempotency_key")?.to_owned();
    let source = load_source(
        runtime,
        &project_id,
        &candidate_id,
        &candidate_state_sha256,
        base_version_id.as_deref(),
        &authoring_node_id,
        &part_id,
        &source_program_object_sha256,
        &source_program_sha256,
        &source_artifact_id,
        &source_artifact_object_sha256,
        &source_artifact_sha256,
        &source_artifact_readback_object_sha256,
        &source_artifact_readback_sha256,
        &source_lineage_sha256,
        &canonical_mesh_id,
        &canonical_mesh_object_sha256,
        &canonical_mesh_sha256,
        &current_source_mesh_sha256,
    )?;
    if genesis_source_mesh_sha256 != current_source_mesh_sha256 && parent_object_sha256.is_none() {
        return Err(invalid(
            "genesis source mesh differs without a parent revision",
        ));
    }
    let lineage_id = authoring_mesh_identity::lineage_id(
        &project_id,
        &authoring_node_id,
        &part_id,
        &genesis_source_mesh_sha256,
    )?;
    if expected_lineage_id
        .as_deref()
        .is_some_and(|value| value != lineage_id)
    {
        return Err(invalid(
            "expected_lineage_id differs from Runtime derivation",
        ));
    }
    // The public @2 request intentionally has no revision_index: Store's
    // next-revision binding is derived from the supplied parent.  A parent
    // must be the immediately preceding revision; Store repeats this check in
    // its transaction.  We derive the index here for the payload/record.
    let parent = read_parent(
        runtime,
        parent_object_sha256.as_deref(),
        parent_canonical_sha256.as_deref(),
    )?;
    let revision_index = parent
        .as_ref()
        .and_then(|value| value.payload.get("revision_index"))
        .and_then(Value::as_u64)
        .map(|value| value + 1)
        .unwrap_or(0);
    let topology = topology_proof_for_candidate(
        runtime,
        &project_id,
        &candidate_id,
        parent.as_ref(),
        &operation_lineage_sha256,
        revision_index,
    )?;
    let payload = make_payload(
        &source,
        &lineage_id,
        &genesis_source_mesh_sha256,
        parent.as_ref(),
        revision_index,
        &operation_lineage_sha256,
        topology.as_ref(),
    )?;
    let revision_kind = payload
        .get("revision_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("identity revision kind is missing"))?;
    let identity_lineage_sha256 = payload["canonical_sha256"]
        .as_str()
        .ok_or_else(|| invalid("identity payload hash is missing"))?
        .to_owned();
    if expected_lineage_sha256
        .as_deref()
        .is_some_and(|value| value != identity_lineage_sha256)
    {
        return Err(invalid(
            "expected_lineage_sha256 differs from Runtime payload",
        ));
    }
    let payload_bytes = canonical_bytes(&payload, IDENTITY_SCHEMA)?;
    let identity_object_sha256 = sha256_hex(&payload_bytes);
    let object_record = runtime.put_object(
        &payload_bytes,
        Some(&identity_object_sha256),
        JSON_MIME,
        IDENTITY_OBJECT_KIND,
    )?;
    if object_record.record.sha256 != identity_object_sha256 {
        return Err(invalid("identity CAS object hash differs after put"));
    }
    let record = store_record(
        &source,
        &payload,
        &lineage_id,
        &genesis_source_mesh_sha256,
        revision_index,
        revision_kind,
        &operation_lineage_sha256,
        parent.as_ref(),
        &identity_object_sha256,
        &request_input_sha256,
        &idempotency_key,
    )?;
    let (stored, replayed) = runtime
        .store
        .persist_authoring_mesh_identity_lineage_with_replay(&record, &object_record.record)?;
    if stored.identity_lineage_object_sha256 != identity_object_sha256
        || stored.identity_lineage_sha256 != identity_lineage_sha256
        || stored.canonical_mesh_object_sha256 != canonical_mesh_object_sha256
        || stored.canonical_mesh_sha256 != canonical_mesh_sha256
    {
        return Err(invalid("identity Store record readback differs"));
    }
    let (readback, readback_bytes) =
        read_json_object(runtime, &identity_object_sha256, IDENTITY_OBJECT_KIND)?;
    if readback != payload || sha256_hex(&readback_bytes) != identity_object_sha256 {
        return Err(invalid("identity CAS readback differs"));
    }
    result_value(
        PREPARE_RESULT_SCHEMA,
        &source,
        &stored,
        payload,
        &request_input_sha256,
        replayed,
        true,
    )
}

pub(super) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request, GET_REQUEST_FIELDS, GET_REQUEST_SCHEMA)?;
    if text(object, "schema_version")? != GET_REQUEST_SCHEMA
        || text(object, "writer_policy")? != WRITER_POLICY
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
    {
        return Err(invalid("identity get request policy differs"));
    }
    bool_const(object, "runtime_write_performed", false)?;
    bool_const(object, "persistent_user_data_touched", false)?;
    let request_input_sha256 = input_hash(request, object)?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let lineage_id = identifier(object, "lineage_id")?.to_owned();
    let revision_index = object
        .get("revision_index")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("revision_index is invalid"))?;
    let candidate_id = identifier(object, "candidate_id")?.to_owned();
    let candidate_state_sha256 = sha(object, "candidate_state_sha256")?.to_owned();
    let canonical_mesh_id = identifier(object, "canonical_mesh_id")?.to_owned();
    let canonical_mesh_object_sha256 = sha(object, "canonical_mesh_object_sha256")?.to_owned();
    let canonical_mesh_sha256 = sha(object, "canonical_mesh_sha256")?.to_owned();
    let identity_object_sha256 = sha(object, "identity_lineage_object_sha256")?.to_owned();
    let identity_lineage_sha256 = sha(object, "identity_lineage_sha256")?.to_owned();
    let record = runtime
        .store
        .get_authoring_mesh_identity_lineage_by_lineage(&project_id, &lineage_id, revision_index)?
        .ok_or_else(|| invalid("identity Store record is unavailable"))?;
    if record.lineage_id != lineage_id
        || record.candidate_id != candidate_id
        || record.candidate_state_sha256 != candidate_state_sha256
        || record.canonical_mesh_id != canonical_mesh_id
        || record.canonical_mesh_object_sha256 != canonical_mesh_object_sha256
        || record.canonical_mesh_sha256 != canonical_mesh_sha256
        || record.identity_lineage_object_sha256 != identity_object_sha256
        || record.identity_lineage_sha256 != identity_lineage_sha256
    {
        return Err(invalid("identity get request does not match Store record"));
    }
    let source_artifact_id = runtime
        .candidate(&record.candidate_id)?
        .and_then(|candidate| candidate.prepared_object_id)
        .ok_or_else(|| invalid("identity source artifact ID is unavailable"))?;
    let source_lineage_sha256 =
        source_lineage_from_canonical(runtime, &record.canonical_mesh_object_sha256)?;
    let source = load_source(
        runtime,
        &record.project_id,
        &record.candidate_id,
        &record.candidate_state_sha256,
        record.base_version_id.as_deref(),
        &record.authoring_node_id,
        &record.part_id,
        &record.source_program_object_sha256,
        &record.source_program_sha256,
        &source_artifact_id,
        &record.source_artifact_object_sha256,
        &record.source_artifact_sha256,
        &record.source_artifact_readback_object_sha256,
        &record.source_artifact_readback_sha256,
        &source_lineage_sha256,
        &record.canonical_mesh_id,
        &record.canonical_mesh_object_sha256,
        &record.canonical_mesh_sha256,
        &record.current_source_mesh_sha256,
    )?;
    let (payload, bytes) =
        read_json_object(runtime, &identity_object_sha256, IDENTITY_OBJECT_KIND)?;
    if sha256_hex(&bytes) != identity_object_sha256
        || payload.get("canonical_sha256").and_then(Value::as_str)
            != Some(identity_lineage_sha256.as_str())
    {
        return Err(invalid("identity get CAS hash differs"));
    }
    check_identity_payload(&payload)?;
    result_value(
        GET_RESULT_SCHEMA,
        &source,
        &record,
        payload,
        &request_input_sha256,
        true,
        false,
    )
}

fn source_lineage_from_canonical(
    runtime: &Runtime,
    canonical_mesh_object_sha256: &str,
) -> Result<String, RuntimeError> {
    let (canonical, _) = read_json_object(
        runtime,
        canonical_mesh_object_sha256,
        AUTHORING_MESH_CANONICAL_OBJECT_KIND,
    )?;
    canonical
        .get("source_lineage_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .map(str::to_owned)
        .ok_or_else(|| invalid("identity source canonical lineage is unavailable"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgecad_contracts::{CandidateRecord, GeometryCandidateEvidenceRecord};
    use serde_json::json;
    use std::fs;
    use uuid::Uuid;

    const AUTHORING_MESH_POLICY_SHA256: &str =
        "aa72cadabba90ddb43dd0014cfa434ab9b13f4e072b09258072f37334c72e709";
    const CANDIDATE_PROGRAM_SCHEMA: &str = "GeometryProgram@2";

    #[derive(Clone, Debug)]
    struct SourceFixture {
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
        source_mesh_sha256: String,
        durable: Value,
    }

    fn seal_input_hash(request: &mut Value) {
        request["input_sha256"] = Value::String(String::new());
        request["input_sha256"] = Value::String(canonical_json_hash(request));
    }

    fn authoring_program(project_id: &str, variant: u8) -> Value {
        let renamed = variant == 2;
        let vertex_3_id = if renamed { "v4" } else { "v3" };
        let edge_03_id = if renamed { "e04" } else { "e03" };
        let edge_03_vertex = if renamed { "v4" } else { "v3" };
        let position_x = match variant {
            1 => -0.75,
            3 => -0.50,
            _ => -1.0,
        };
        let mut program = json!({
            "schema_version": CANDIDATE_PROGRAM_SCHEMA,
            "project_id": project_id,
            "representation_plan_sha256": "b".repeat(64),
            "operator_catalog_sha256": crate::operator_catalog_sha256(),
            "units": {"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets": {
                "max_nodes":1,
                "max_triangles":32,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"authoring-identity-panel",
                "operator_id":"forgecad.geometry.authoring-mesh@1",
                "inputs":[],
                "parameters":{
                    "shape":"authoring-mesh",
                    "topology_policy":"triangle-quad-manifold-with-boundary@1",
                    "vertices":[
                        {"element_id":"v0","position_m":[-1.0,-1.0,0.0]},
                        {"element_id":"v1","position_m":[1.0,-1.0,0.0]},
                        {"element_id":"v2","position_m":[1.0,1.0,0.0]},
                        {"element_id":vertex_3_id,"position_m":[position_x,1.0,0.0]}
                    ],
                    "edges":[
                        {"element_id":"e01","vertex_ids":["v0","v1"]},
                        {"element_id":edge_03_id,"vertex_ids":["v0",edge_03_vertex]},
                        {"element_id":"e12","vertex_ids":["v1","v2"]},
                        {"element_id":"e23","vertex_ids":["v2",vertex_3_id]}
                    ],
                    "loops":[
                        {"element_id":"l0","face_id":"f0","ordinal":0,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
                        {"element_id":"l1","face_id":"f0","ordinal":1,"vertex_id":"v1","edge_id":"e12","edge_forward":true},
                        {"element_id":"l2","face_id":"f0","ordinal":2,"vertex_id":"v2","edge_id":"e23","edge_forward":true},
                        {"element_id":"l3","face_id":"f0","ordinal":3,"vertex_id":vertex_3_id,"edge_id":edge_03_id,"edge_forward":false}
                    ],
                    "faces":[{"element_id":"f0","loop_ids":["l0","l1","l2","l3"]}],
                    "position_m":[0.0,0.0,0.0],
                    "rotation_rad":[0.0,0.0,0.0]
                }
            }],
            "part_outputs":[{
                "part_id":"authoring-identity-panel",
                "input_node_ids":["authoring-identity-panel"],
                "material_zone_id":"zone-authoring-shell",
                "solid":false
            }]
        });
        let hash = crate::hash_geometry_program_with_runtime_worker(&program)
            .expect("authoring identity GeometryProgram hash");
        program["canonical_sha256"] = hash["canonical_sha256"].clone();
        program
    }

    fn authoring_two_triangle_program(project_id: &str) -> Value {
        let mut program = authoring_program(project_id, 0);
        program["nodes"][0]["parameters"] = json!({
            "shape":"authoring-mesh",
            "topology_policy":"triangle-quad-manifold-with-boundary@1",
            "vertices":[
                {"element_id":"v0","position_m":[-1.0,-1.0,0.0]},
                {"element_id":"v1","position_m":[1.0,-1.0,0.0]},
                {"element_id":"v2","position_m":[0.0,1.0,0.0]},
                {"element_id":"v3","position_m":[1.0,1.0,0.0]}
            ],
            "edges":[
                {"element_id":"e01","vertex_ids":["v0","v1"]},
                {"element_id":"e02","vertex_ids":["v0","v2"]},
                {"element_id":"e12","vertex_ids":["v1","v2"]},
                {"element_id":"e13","vertex_ids":["v1","v3"]},
                {"element_id":"e23","vertex_ids":["v2","v3"]}
            ],
            "loops":[
                {"element_id":"l0","face_id":"f0","ordinal":0,"vertex_id":"v2","edge_id":"e02","edge_forward":false},
                {"element_id":"l1","face_id":"f0","ordinal":1,"vertex_id":"v0","edge_id":"e01","edge_forward":true},
                {"element_id":"l2","face_id":"f0","ordinal":2,"vertex_id":"v1","edge_id":"e12","edge_forward":true},
                {"element_id":"l3","face_id":"f1","ordinal":0,"vertex_id":"v1","edge_id":"e13","edge_forward":true},
                {"element_id":"l4","face_id":"f1","ordinal":1,"vertex_id":"v3","edge_id":"e23","edge_forward":false},
                {"element_id":"l5","face_id":"f1","ordinal":2,"vertex_id":"v2","edge_id":"e12","edge_forward":false}
            ],
            "faces":[
                {"element_id":"f0","loop_ids":["l0","l1","l2"]},
                {"element_id":"f1","loop_ids":["l3","l4","l5"]}
            ],
            "position_m":[0.0,0.0,0.0],
            "rotation_rad":[0.0,0.0,0.0]
        });
        // `authoring_program` returns a hash-bound program.  The worker draft
        // hash is defined over the hash-free GeometryProgram@2 preimage, so
        // remove the inherited field before hashing this mutated fixture.
        program
            .as_object_mut()
            .expect("authoring two-triangle program object")
            .remove("canonical_sha256");
        let hash = crate::hash_geometry_program_with_runtime_worker(&program)
            .expect("authoring two-triangle GeometryProgram hash");
        program["canonical_sha256"] = hash["canonical_sha256"].clone();
        program
    }

    fn canonical_mesh_hash(
        projection: &Value,
        candidate: &CandidateRecord,
        evidence: &GeometryCandidateEvidenceRecord,
        source_artifact_readback_sha256: &str,
        authoring_node_id: &str,
        part_id: &str,
    ) -> String {
        let lineage = projection["lineage"]["lineage_sha256"]
            .as_str()
            .expect("projection lineage hash");
        let mesh_sha256 = projection["mesh_sha256"].as_str().expect("mesh hash");
        let mut value = json!({
            "schema_version":"AuthoringMeshCanonical@1",
            "canonical_mesh_id":projection["mesh_id"],
            "project_id":candidate.project_id,
            "candidate_id":candidate.candidate_id,
            "candidate_state_sha256":candidate.canonical_sha256,
            "base_version_id":candidate.base_version_id,
            "authoring_node_id":authoring_node_id,
            "part_id":part_id,
            "source_program_object_sha256":evidence.geometry_program_object_sha256,
            "source_program_sha256":evidence.geometry_program_sha256,
            "source_artifact_object_sha256":candidate.prepared_object_sha256,
            "source_artifact_sha256":candidate.prepared_object_sha256,
            "source_artifact_readback_object_sha256":evidence.artifact_readback_object_sha256,
            "source_artifact_readback_sha256":source_artifact_readback_sha256,
            "source_lineage_sha256":lineage,
            "representation":"runtime-owned-original-half-edge@1",
            "storage_policy":"runtime-owned-sqlite-cas-canonical-authoring-mesh@1",
            "writer_policy":WRITER_POLICY,
            "original_identity":{
                "identity_id":projection["original_identity"]["identity_id"],
                "namespace":"original",
                "identity_kind":"runtime-owned-original-authoring@1",
                "element_id_policy":"lineage-scoped-opaque-not-cross-version-stable@1",
                "topology_sha256":mesh_sha256,
                "source_lineage_sha256":lineage,
                "stability_scope":"same-canonical-mesh-lineage-only@1"
            },
            "evaluated_identity":{
                "identity_id":projection["evaluated_identity"]["identity_id"],
                "namespace":"evaluated",
                "identity_kind":"runtime-derived-evaluated-artifact-readback@1",
                "element_id_policy":"artifact-local-no-authoring-bijection@1",
                "correspondence_policy":"non-bijective-derived-only@1",
                "artifact_object_sha256":candidate.prepared_object_sha256,
                "artifact_readback_sha256":source_artifact_readback_sha256,
                "source_lineage_sha256":lineage,
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
            "canonicalization_policy":CANONICALIZATION_POLICY,
            "runtime_write_performed":true,
            "persistent_user_data_touched":true,
            "stage_advanced":false,
            "candidate_confirmed":false,
            "version_created":false,
            "export_performed":false,
            "quality_status":QUALITY_STATUS,
            "canonical_sha256":""
        });
        value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
        value["canonical_sha256"]
            .as_str()
            .expect("canonical hash")
            .to_owned()
    }

    fn prepare_source(
        runtime: &Runtime,
        project_id: &str,
        variant: u8,
        key: &str,
    ) -> SourceFixture {
        prepare_source_program(
            runtime,
            project_id,
            authoring_program(project_id, variant),
            key,
        )
    }

    fn prepare_source_program(
        runtime: &Runtime,
        project_id: &str,
        program: Value,
        key: &str,
    ) -> SourceFixture {
        let authoring_node_id = "authoring-identity-panel".to_owned();
        let part_id = authoring_node_id.clone();
        let prepared = runtime
            .prepare_geometry_candidate(
                project_id,
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("source GeometryProgram candidate");
        let candidate_id = prepared["candidate"]["candidate_id"]
            .as_str()
            .expect("source candidate id")
            .to_owned();
        prepare_existing_source(runtime, &candidate_id, &authoring_node_id, &part_id, key)
    }

    fn stage_typed_operation(
        runtime: &Runtime,
        source: &SourceFixture,
        mut edit: Value,
        stage_key: &str,
        target_key: &str,
    ) -> (Value, SourceFixture, String, Value) {
        let evidence = runtime
            .store
            .get_geometry_candidate_evidence(&source.candidate_id)
            .expect("source evidence query")
            .expect("source evidence");
        let topology_request = json!({
            "schema_version":"AuthoringTopologyRequest@1",
            "project_id":source.project_id,
            "candidate_id":source.candidate_id,
            "artifact_id":source.source_artifact_object_sha256,
            "artifact_readback_sha256":source.source_artifact_readback_sha256,
            "program_sha256":source.source_program_sha256,
            "operator_catalog_sha256":evidence.operator_catalog_sha256,
            "readback_config_sha256":evidence.readback_config_sha256,
            "authoring_node_id":source.authoring_node_id,
            "part_id":source.part_id,
            "authoring_topology_policy_sha256":"a6fb36a530e49537673b66d65ecb6e4fb4f51ffb3e7d01a0980be71f28cb367d",
            "max_response_bytes":MAX_RESPONSE_BYTES
        });
        let topology =
            crate::authoring_topology::get(runtime, &topology_request).expect("source topology");
        let operation_lineage_sha256 = canonical_json_hash(&edit);
        edit["operation_lineage_sha256"] = Value::String(operation_lineage_sha256.clone());
        let mut preview_request = json!({
            "schema_version":"AuthoringMeshEditPreviewRequest@1",
            "topology_request":topology_request,
            "base_topology_sha256":topology["topology_sha256"],
            "edit":edit,
            "edit_policy_sha256":"fc76c6dffef2a41c05ff0a65ff160c8fce5eb37d312a3ef7f78043ef92539144"
        });
        preview_request["input_sha256"] = Value::String(canonical_json_hash(&preview_request));
        let preview = runtime
            .authoring_mesh_edit_preview(&preview_request)
            .expect("typed edit preview");
        let mut prepare_request = json!({
            "schema_version":"AuthoringMeshEditPrepareRequest@1",
            "project_id":source.project_id,
            "source_candidate_id":source.candidate_id,
            "base_version_id":source.base_version_id,
            "preview_request":preview_request,
            "expected_preview_canonical_sha256":preview["canonical_sha256"],
            "idempotency_key":stage_key,
            "max_response_bytes":MAX_RESPONSE_BYTES
        });
        prepare_request["input_sha256"] = Value::String(canonical_json_hash(&prepare_request));
        let staged = runtime
            .authoring_mesh_edit_prepare(&prepare_request)
            .expect("typed edit prepare");
        assert_eq!(staged["operation"], edit["operation"]);
        assert_eq!(staged["candidate"]["state"], "reviewable");
        assert_eq!(staged["version_status"], "no-version-created");
        assert_eq!(staged["confirm_status"], "approval-required");
        assert_eq!(staged["export_status"], "locked-until-confirm");
        let target_candidate_id = staged["new_candidate_id"]
            .as_str()
            .expect("typed edit target candidate")
            .to_owned();
        let target_source = prepare_existing_source(
            runtime,
            &target_candidate_id,
            &source.authoring_node_id,
            &source.part_id,
            target_key,
        );
        (preview, target_source, operation_lineage_sha256, staged)
    }

    fn prepare_existing_source(
        runtime: &Runtime,
        candidate_id: &str,
        authoring_node_id: &str,
        part_id: &str,
        key: &str,
    ) -> SourceFixture {
        let candidate = runtime
            .candidate(candidate_id)
            .expect("candidate query")
            .expect("source candidate");
        let evidence = runtime
            .store
            .get_geometry_candidate_evidence(candidate_id)
            .expect("source evidence query")
            .expect("source evidence");
        let source_artifact_id = candidate
            .prepared_object_id
            .clone()
            .expect("source artifact id");
        let source_artifact_object_sha256 = candidate
            .prepared_object_sha256
            .clone()
            .expect("source artifact object hash");
        let source_artifact_readback = runtime
            .artifact_readback(&source_artifact_object_sha256, candidate_id)
            .expect("source artifact readback");
        let source_artifact_readback_sha256 = source_artifact_readback["canonical_sha256"]
            .as_str()
            .expect("source ArtifactReadback hash")
            .to_owned();
        let projection_request = json!({
            "schema_version":"AuthoringMeshRequest@1",
            "project_id":candidate.project_id,
            "candidate_id":candidate_id,
            "artifact_id":source_artifact_object_sha256,
            "artifact_readback_sha256":source_artifact_readback_sha256,
            "program_sha256":evidence.geometry_program_sha256,
            "operator_catalog_sha256":evidence.operator_catalog_sha256,
            "readback_config_sha256":evidence.readback_config_sha256,
            "authoring_node_id":authoring_node_id,
            "part_id":part_id,
            "authoring_mesh_policy_sha256":AUTHORING_MESH_POLICY_SHA256,
            "max_response_bytes":MAX_RESPONSE_BYTES
        });
        let projection = runtime
            .authoring_mesh(&projection_request)
            .expect("source AuthoringMesh projection");
        let source_lineage_sha256 = projection["lineage"]["lineage_sha256"]
            .as_str()
            .expect("source lineage hash")
            .to_owned();
        let source_mesh_sha256 = projection["mesh_sha256"]
            .as_str()
            .expect("source mesh hash")
            .to_owned();
        let expected_canonical_mesh_sha256 = canonical_mesh_hash(
            &projection,
            &candidate,
            &evidence,
            &source_artifact_readback_sha256,
            &authoring_node_id,
            &part_id,
        );
        let mut durable_request = json!({
            "schema_version":"AuthoringMeshPrepareRequest@1",
            "project_id":candidate.project_id,
            "source_candidate_id":candidate_id,
            "source_candidate_state_sha256":candidate.canonical_sha256,
            "base_version_id":candidate.base_version_id,
            "authoring_node_id":authoring_node_id,
            "part_id":part_id,
            "source_program_object_sha256":evidence.geometry_program_object_sha256,
            "source_program_sha256":evidence.geometry_program_sha256,
            "source_artifact_id":source_artifact_id,
            "source_artifact_object_sha256":source_artifact_object_sha256,
            "source_artifact_sha256":source_artifact_object_sha256,
            "source_artifact_readback_object_sha256":evidence.artifact_readback_object_sha256,
            "source_artifact_readback_sha256":source_artifact_readback_sha256,
            "source_lineage_sha256":source_lineage_sha256,
            "expected_canonical_mesh_sha256":expected_canonical_mesh_sha256,
            "idempotency_key":key,
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "runtime_write_performed":false,
            "writer_policy":WRITER_POLICY,
            "canonicalization_policy":CANONICALIZATION_POLICY,
            "input_sha256":""
        });
        seal_input_hash(&mut durable_request);
        let durable = runtime
            .authoring_mesh_durable_prepare(&durable_request)
            .expect("durable AuthoringMesh source");
        SourceFixture {
            project_id: candidate.project_id.clone(),
            candidate_id: candidate_id.to_owned(),
            candidate_state_sha256: candidate.canonical_sha256,
            base_version_id: candidate.base_version_id,
            authoring_node_id: authoring_node_id.to_owned(),
            part_id: part_id.to_owned(),
            source_program_object_sha256: evidence.geometry_program_object_sha256,
            source_program_sha256: evidence.geometry_program_sha256,
            source_artifact_id,
            source_artifact_object_sha256: source_artifact_object_sha256.clone(),
            source_artifact_sha256: source_artifact_object_sha256,
            source_artifact_readback_object_sha256: evidence.artifact_readback_object_sha256,
            source_artifact_readback_sha256,
            source_lineage_sha256,
            source_mesh_sha256,
            durable,
        }
    }

    fn identity_request(
        source: &SourceFixture,
        genesis_source_mesh_sha256: &str,
        parent: Option<&Value>,
        operation_lineage_sha256: &str,
        expected_lineage_id: Option<&str>,
        idempotency_key: &str,
    ) -> Value {
        let mut request = json!({
            "schema_version":PREPARE_REQUEST_SCHEMA,
            "project_id":source.project_id,
            "source_candidate_id":source.candidate_id,
            "source_candidate_state_sha256":source.candidate_state_sha256,
            "base_version_id":source.base_version_id,
            "authoring_node_id":source.authoring_node_id,
            "part_id":source.part_id,
            "source_program_object_sha256":source.source_program_object_sha256,
            "source_program_sha256":source.source_program_sha256,
            "source_artifact_id":source.source_artifact_id,
            "source_artifact_object_sha256":source.source_artifact_object_sha256,
            "source_artifact_sha256":source.source_artifact_sha256,
            "source_artifact_readback_object_sha256":source.source_artifact_readback_object_sha256,
            "source_artifact_readback_sha256":source.source_artifact_readback_sha256,
            "source_lineage_sha256":source.source_lineage_sha256,
            "canonical_mesh_id":source.durable["canonical_mesh_id"],
            "canonical_mesh_object_sha256":source.durable["canonical_mesh_object_sha256"],
            "canonical_mesh_sha256":source.durable["canonical_mesh_sha256"],
            "genesis_source_mesh_sha256":genesis_source_mesh_sha256,
            "current_source_mesh_sha256":source.source_mesh_sha256,
            "parent_lineage_object_sha256":parent.map(|value| value["identity_lineage_object_sha256"].clone()).unwrap_or(Value::Null),
            "parent_lineage_sha256":parent.map(|value| value["identity_lineage_sha256"].clone()).unwrap_or(Value::Null),
            "operation_lineage_sha256":operation_lineage_sha256,
            "expected_lineage_id":expected_lineage_id,
            "expected_lineage_sha256":null,
            "idempotency_key":idempotency_key,
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "runtime_write_performed":false,
            "writer_policy":WRITER_POLICY,
            "canonicalization_policy":CANONICALIZATION_POLICY,
            "input_sha256":""
        });
        seal_input_hash(&mut request);
        request
    }

    fn identity_get_request(result: &Value) -> Value {
        let mut request = json!({
            "schema_version":GET_REQUEST_SCHEMA,
            "project_id":result["project_id"],
            "lineage_id":result["lineage_id"],
            "revision_index":result["identity_lineage"]["revision_index"],
            "candidate_id":result["candidate_id"],
            "candidate_state_sha256":result["candidate_state_sha256"],
            "canonical_mesh_id":result["canonical_mesh_id"],
            "canonical_mesh_object_sha256":result["canonical_mesh_object_sha256"],
            "canonical_mesh_sha256":result["canonical_mesh_sha256"],
            "identity_lineage_object_sha256":result["identity_lineage_object_sha256"],
            "identity_lineage_sha256":result["identity_lineage_sha256"],
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "writer_policy":WRITER_POLICY,
            "runtime_write_performed":false,
            "persistent_user_data_touched":false,
            "input_sha256":""
        });
        seal_input_hash(&mut request);
        request
    }

    #[test]
    fn identity_lineage_runtime_prepare_get_reopen_delta_and_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "forgecad-authoring-mesh-identity-runtime-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("restart fixture root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let project_id;
        let genesis_source_mesh_sha256;
        let first;
        let first_request;
        let second;
        let second_request;
        let third;
        let third_request;
        let fourth_source;
        let fourth_request;
        {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("initial Runtime");
            project_id = runtime
                .create_project("AuthoringMesh Identity Runtime", json!({"profile":"test"}))
                .expect("project")
                .project_id;
            let source0 = prepare_source(&runtime, &project_id, 0, "identity-source-0");
            genesis_source_mesh_sha256 = source0.source_mesh_sha256.clone();
            first_request = identity_request(
                &source0,
                &genesis_source_mesh_sha256,
                None,
                &"a".repeat(64),
                None,
                "identity-revision-0",
            );
            first = runtime
                .authoring_mesh_identity_lineage_prepare(&first_request)
                .expect("identity genesis prepare");
            assert_eq!(first["replayed"], false);
            assert_eq!(first["identity_lineage"]["revision_kind"], "genesis");
            assert!(first["identity_lineage"]["tombstones"]
                .as_array()
                .expect("genesis tombstones")
                .is_empty());
            assert!(first["identity_lineage"]["correspondence"]
                .as_array()
                .expect("genesis correspondence")
                .is_empty());
            let first_get_request = identity_get_request(&first);
            let first_get = runtime
                .authoring_mesh_identity_lineage_get(&first_get_request)
                .expect("identity genesis get");
            assert_eq!(first_get["identity_lineage"], first["identity_lineage"]);
            assert_eq!(first_get["restart_hash_verified"], true);
            assert_eq!(first_get["runtime_write_performed"], false);
            assert_eq!(first_get["persistent_user_data_touched"], false);
            let cas_after_first = runtime.store.cas().list_objects().expect("CAS inventory");
            let replay = runtime
                .authoring_mesh_identity_lineage_prepare(&first_request)
                .expect("identity genesis replay");
            assert_eq!(replay["replayed"], true);
            assert_eq!(replay["identity_lineage"], first["identity_lineage"]);
            assert_eq!(
                runtime
                    .store
                    .cas()
                    .list_objects()
                    .expect("CAS replay inventory"),
                cas_after_first
            );

            let mut conflict_request = first_request.clone();
            conflict_request["operation_lineage_sha256"] = Value::String("b".repeat(64));
            seal_input_hash(&mut conflict_request);
            let conflict = runtime
                .authoring_mesh_identity_lineage_prepare(&conflict_request)
                .expect_err("same idempotency key must conflict");
            assert!(conflict
                .to_string()
                .contains("AUTHORING_MESH_IDENTITY_LINEAGE_CONFLICT"));

            let source1 = prepare_source(&runtime, &project_id, 1, "identity-source-1");
            second_request = identity_request(
                &source1,
                &genesis_source_mesh_sha256,
                Some(&first),
                &"a".repeat(64),
                first["lineage_id"].as_str(),
                "identity-revision-1",
            );
            second = runtime
                .authoring_mesh_identity_lineage_prepare(&second_request)
                .expect("identity preserving edit prepare");
            assert_eq!(second["identity_lineage"]["revision_index"], 1);
            assert_eq!(
                second["identity_lineage"]["revision_kind"],
                "preserving-edit"
            );
            let second_correspondence = second["identity_lineage"]["correspondence"]
                .as_array()
                .expect("second correspondence");
            assert!(!second_correspondence.is_empty());
            assert!(second_correspondence
                .iter()
                .all(|value| value["kind"] == "preserved"));
            assert!(second["identity_lineage"]["tombstones"]
                .as_array()
                .expect("second tombstones")
                .is_empty());

            let source2 = prepare_source(&runtime, &project_id, 2, "identity-source-2");
            third_request = identity_request(
                &source2,
                &genesis_source_mesh_sha256,
                Some(&second),
                &"d".repeat(64),
                first["lineage_id"].as_str(),
                "identity-revision-2",
            );
            third = runtime
                .authoring_mesh_identity_lineage_prepare(&third_request)
                .expect("identity topology edit prepare");
            assert_eq!(third["identity_lineage"]["revision_index"], 2);
            assert_eq!(third["identity_lineage"]["revision_kind"], "topology-edit");
            let third_tombstones = third["identity_lineage"]["tombstones"]
                .as_array()
                .expect("third tombstones");
            assert!(!third_tombstones.is_empty());
            assert!(third_tombstones
                .iter()
                .any(|value| value["retired_revision_index"] == 2));
            assert!(third["identity_lineage"]["correspondence"]
                .as_array()
                .expect("third correspondence")
                .iter()
                .any(|value| value["kind"] == "retired"));

            fourth_source = prepare_source(&runtime, &project_id, 3, "identity-source-3");
            fourth_request = identity_request(
                &fourth_source,
                &genesis_source_mesh_sha256,
                Some(&third),
                &"e".repeat(64),
                first["lineage_id"].as_str(),
                "identity-revision-3",
            );
            let reuse = runtime
                .authoring_mesh_identity_lineage_prepare(&fourth_request)
                .expect_err("historical tombstone reuse must fail closed");
            assert!(reuse
                .to_string()
                .contains("historical tombstoned identity cannot reappear"));
            assert!(runtime
                .store
                .get_authoring_mesh_identity_lineage(&project_id, "identity-revision-3")
                .expect("failed reuse row lookup")
                .is_none());
            assert_eq!(
                runtime.versions(Some(&project_id)).expect("versions").len(),
                0
            );
            drop(runtime);
        }

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopened Runtime");
        let second_restart = reopened
            .authoring_mesh_identity_lineage_get(&identity_get_request(&second))
            .expect("identity preserving edit restart get");
        assert_eq!(
            second_restart["identity_lineage"],
            second["identity_lineage"]
        );
        assert_eq!(second_restart["restart_hash_verified"], true);
        let third_restart = reopened
            .authoring_mesh_identity_lineage_get(&identity_get_request(&third))
            .expect("identity topology edit restart get");
        assert_eq!(third_restart["identity_lineage"], third["identity_lineage"]);
        assert_eq!(
            third_restart["identity_lineage"]["revision_kind"],
            "topology-edit"
        );
        let mut wrong_binding = identity_get_request(&third);
        wrong_binding["canonical_mesh_object_sha256"] = Value::String("f".repeat(64));
        seal_input_hash(&mut wrong_binding);
        assert!(reopened
            .authoring_mesh_identity_lineage_get(&wrong_binding)
            .expect_err("wrong canonical object binding must fail closed")
            .to_string()
            .contains("does not match Store record"));
        let object_sha256 = third["identity_lineage_object_sha256"]
            .as_str()
            .expect("identity object hash");
        let path = reopened
            .store
            .cas()
            .root()
            .join("objects")
            .join(&object_sha256[..2])
            .join(object_sha256);
        fs::write(path, b"{\"tampered\":true}").expect("tamper identity CAS");
        assert!(reopened
            .authoring_mesh_identity_lineage_get(&identity_get_request(&third))
            .expect_err("tampered identity CAS must fail closed")
            .to_string()
            .contains("CAS"));
        drop(reopened);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn typed_split_candidate_materializes_durable_identity_children_and_reopens() {
        if forgecad_contracts::build_cohort_sha256().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "forgecad-authoring-mesh-identity-split-restart-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("split restart fixture root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let split_identity;
        {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("initial Runtime");
            let project_id = runtime
                .create_project(
                    "AuthoringMesh typed split identity",
                    json!({"profile":"test"}),
                )
                .expect("project")
                .project_id;
            let source = prepare_source_program(
                &runtime,
                &project_id,
                authoring_two_triangle_program(&project_id),
                "split-source-durable",
            );
            let genesis_source_mesh_sha256 = source.source_mesh_sha256.clone();
            let genesis_request = identity_request(
                &source,
                &genesis_source_mesh_sha256,
                None,
                &"a".repeat(64),
                None,
                "split-identity-genesis",
            );
            let genesis = runtime
                .authoring_mesh_identity_lineage_prepare(&genesis_request)
                .expect("identity genesis");

            let topology_request = json!({
                "schema_version":"AuthoringTopologyRequest@1",
                "project_id":source.project_id,
                "candidate_id":source.candidate_id,
                "artifact_id":source.source_artifact_object_sha256,
                "artifact_readback_sha256":source.source_artifact_readback_sha256,
                "program_sha256":source.source_program_sha256,
                "operator_catalog_sha256":runtime.store
                    .get_geometry_candidate_evidence(&source.candidate_id)
                    .expect("source evidence query")
                    .expect("source evidence")
                    .operator_catalog_sha256,
                "readback_config_sha256":runtime.store
                    .get_geometry_candidate_evidence(&source.candidate_id)
                    .expect("source evidence query")
                    .expect("source evidence")
                    .readback_config_sha256,
                "authoring_node_id":source.authoring_node_id,
                "part_id":source.part_id,
                "authoring_topology_policy_sha256":"a6fb36a530e49537673b66d65ecb6e4fb4f51ffb3e7d01a0980be71f28cb367d",
                "max_response_bytes":MAX_RESPONSE_BYTES
            });
            let topology = crate::authoring_topology::get(&runtime, &topology_request)
                .expect("source topology");
            let mut edit = json!({
                "operation":"split_edge",
                "edge_id":"e01",
                "parent_revision":0
            });
            let operation_lineage_sha256 = canonical_json_hash(&edit);
            edit["operation_lineage_sha256"] = Value::String(operation_lineage_sha256.clone());
            let mut preview_request = json!({
                "schema_version":"AuthoringMeshEditPreviewRequest@1",
                "topology_request":topology_request,
                "base_topology_sha256":topology["topology_sha256"],
                "edit":edit,
                "edit_policy_sha256":"fc76c6dffef2a41c05ff0a65ff160c8fce5eb37d312a3ef7f78043ef92539144"
            });
            preview_request["input_sha256"] = Value::String(canonical_json_hash(&preview_request));
            let preview = runtime
                .authoring_mesh_edit_preview(&preview_request)
                .expect("split preview");
            let proof = preview["edited_element_ids"]["typed_operation_proof"].clone();
            let mut prepare_request = json!({
                "schema_version":"AuthoringMeshEditPrepareRequest@1",
                "project_id":source.project_id,
                "source_candidate_id":source.candidate_id,
                "base_version_id":source.base_version_id,
                "preview_request":preview_request,
                "expected_preview_canonical_sha256":preview["canonical_sha256"],
                "idempotency_key":"split-edit-stage-once",
                "max_response_bytes":MAX_RESPONSE_BYTES
            });
            prepare_request["input_sha256"] = Value::String(canonical_json_hash(&prepare_request));
            let staged = runtime
                .authoring_mesh_edit_prepare(&prepare_request)
                .expect("stage split candidate");
            assert_eq!(staged["operation"], "split_edge");
            assert_eq!(staged["version_status"], "no-version-created");
            assert_eq!(staged["confirm_status"], "approval-required");
            assert_eq!(staged["export_status"], "locked-until-confirm");
            let split_candidate_id = staged["new_candidate_id"]
                .as_str()
                .expect("split candidate id");
            let split_source = prepare_existing_source(
                &runtime,
                split_candidate_id,
                &source.authoring_node_id,
                &source.part_id,
                "split-target-durable",
            );
            let split_request = identity_request(
                &split_source,
                &genesis_source_mesh_sha256,
                Some(&genesis),
                &operation_lineage_sha256,
                genesis["lineage_id"].as_str(),
                "split-identity-revision-1",
            );
            split_identity = runtime
                .authoring_mesh_identity_lineage_prepare(&split_request)
                .expect("materialize split identity lineage");
            let lineage = &split_identity["identity_lineage"];
            assert_eq!(lineage["revision_index"], 1);
            assert_eq!(lineage["revision_kind"], "topology-edit");
            let relation = lineage["correspondence"]
                .as_array()
                .expect("split correspondence")
                .iter()
                .find(|value| value["kind"] == "one-to-many")
                .expect("split one-to-many identity relation");
            assert_eq!(relation["parent_identity_ids"].as_array().unwrap().len(), 1);
            assert_eq!(relation["child_identity_ids"].as_array().unwrap().len(), 2);
            let generated_edges = proof["generated_edge_ids"]
                .as_array()
                .expect("generated edge source IDs");
            for source_id in generated_edges {
                assert!(lineage["elements"]
                    .as_array()
                    .expect("identity elements")
                    .iter()
                    .any(|element| element["element_kind"] == "edge"
                        && element["source_element_id"] == *source_id
                        && element["origin"] == "operation-derived"
                        && relation["child_identity_ids"]
                            .as_array()
                            .unwrap()
                            .contains(&element["identity_id"])));
            }
            assert!(lineage["tombstones"]
                .as_array()
                .expect("split tombstones")
                .iter()
                .any(|value| value["element_kind"] == "edge"
                    && value["operation_lineage_sha256"] == operation_lineage_sha256));
            let relation_parents = lineage["correspondence"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|value| value["parent_identity_ids"].as_array().unwrap())
                .filter_map(Value::as_str)
                .collect::<Vec<_>>();
            assert_eq!(
                relation_parents.iter().collect::<BTreeSet<_>>().len(),
                relation_parents.len(),
                "one parent identity must not be consumed by duplicate relations"
            );
            assert!(runtime
                .versions(Some(&project_id))
                .expect("versions")
                .is_empty());
        }

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopened Runtime");
        let restarted = reopened
            .authoring_mesh_identity_lineage_get(&identity_get_request(&split_identity))
            .expect("split identity restart get");
        assert_eq!(
            restarted["identity_lineage"],
            split_identity["identity_lineage"]
        );
        assert_eq!(restarted["restart_hash_verified"], true);
        assert_eq!(restarted["runtime_write_performed"], false);
        drop(reopened);
        fs::remove_dir_all(root).expect("split restart fixture cleanup");
    }

    #[test]
    fn typed_collapse_candidate_materializes_durable_identity_children_and_reopens() {
        if forgecad_contracts::build_cohort_sha256().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "forgecad-authoring-mesh-identity-collapse-restart-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("collapse restart fixture root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let collapse_identity;
        {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("initial Runtime");
            let project_id = runtime
                .create_project(
                    "AuthoringMesh typed collapse identity",
                    json!({"profile":"test"}),
                )
                .expect("project")
                .project_id;
            let source = prepare_source(&runtime, &project_id, 0, "collapse-source-durable");
            let genesis_source_mesh_sha256 = source.source_mesh_sha256.clone();
            let genesis_request = identity_request(
                &source,
                &genesis_source_mesh_sha256,
                None,
                &"a".repeat(64),
                None,
                "collapse-identity-genesis",
            );
            let genesis = runtime
                .authoring_mesh_identity_lineage_prepare(&genesis_request)
                .expect("identity genesis");

            let (preview, collapse_source, operation_lineage_sha256, staged) =
                stage_typed_operation(
                    &runtime,
                    &source,
                    json!({
                        "operation":"collapse_edge",
                        "edge_id":"e01",
                        "survivor_vertex_id":"v0",
                        "parent_revision":0
                    }),
                    "collapse-edit-stage-once",
                    "collapse-target-durable",
                );
            assert_eq!(preview["operation"], "collapse_edge");
            let proof = &preview["edited_element_ids"]["typed_operation_proof"];
            assert_eq!(proof["operation"], "collapse_edge");
            assert_eq!(proof["correspondence"][0]["kind"], "many-to-one");
            assert_eq!(staged["operation"], "collapse_edge");
            assert_eq!(staged["version_status"], "no-version-created");
            assert_eq!(staged["confirm_status"], "approval-required");
            assert_eq!(staged["export_status"], "locked-until-confirm");

            collapse_identity = runtime
                .authoring_mesh_identity_lineage_prepare(&identity_request(
                    &collapse_source,
                    &genesis_source_mesh_sha256,
                    Some(&genesis),
                    &operation_lineage_sha256,
                    genesis["lineage_id"].as_str(),
                    "collapse-identity-revision-1",
                ))
                .expect("materialize collapse identity lineage");
            let lineage = &collapse_identity["identity_lineage"];
            assert_eq!(lineage["revision_index"], 1);
            assert_eq!(lineage["revision_kind"], "topology-edit");
            let relation = lineage["correspondence"]
                .as_array()
                .expect("collapse correspondence")
                .iter()
                .find(|value| {
                    value["kind"] == "many-to-one"
                        && value["operation_lineage_sha256"] == operation_lineage_sha256
                })
                .expect("collapse many-to-one identity relation");
            assert_eq!(relation["parent_identity_ids"].as_array().unwrap().len(), 2);
            assert_eq!(relation["child_identity_ids"].as_array().unwrap().len(), 1);
            let tombstones = lineage["tombstones"]
                .as_array()
                .expect("collapse tombstones");
            assert!(tombstones.iter().any(|value| {
                value["element_kind"] == "vertex"
                    && value["reason"] == "collapsed"
                    && value["retired_revision_index"] == 1
                    && value["operation_lineage_sha256"] == operation_lineage_sha256
            }));
            assert!(tombstones.iter().any(|value| {
                value["element_kind"] == "edge"
                    && value["reason"] == "collapsed"
                    && value["operation_lineage_sha256"] == operation_lineage_sha256
            }));

            let same_runtime = runtime
                .authoring_mesh_identity_lineage_get(&identity_get_request(&collapse_identity))
                .expect("collapse identity same-runtime get");
            assert_eq!(
                same_runtime["identity_lineage"],
                collapse_identity["identity_lineage"]
            );
            assert_eq!(same_runtime["runtime_write_performed"], false);
            assert_eq!(same_runtime["persistent_user_data_touched"], false);
            assert!(runtime
                .versions(Some(&project_id))
                .expect("versions")
                .is_empty());
        }

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopened Runtime");
        let restarted = reopened
            .authoring_mesh_identity_lineage_get(&identity_get_request(&collapse_identity))
            .expect("collapse identity restart get");
        assert_eq!(
            restarted["identity_lineage"],
            collapse_identity["identity_lineage"]
        );
        assert_eq!(restarted["restart_hash_verified"], true);
        assert_eq!(restarted["runtime_write_performed"], false);
        drop(reopened);
        fs::remove_dir_all(root).expect("collapse restart fixture cleanup");
    }

    #[test]
    fn typed_dissolve_candidate_materializes_durable_identity_children_and_reopens() {
        if forgecad_contracts::build_cohort_sha256().is_none() {
            return;
        }
        let root = std::env::temp_dir().join(format!(
            "forgecad-authoring-mesh-identity-dissolve-restart-{}",
            Uuid::new_v4()
        ));
        fs::create_dir_all(&root).expect("dissolve restart fixture root");
        let database = root.join("runtime.sqlite");
        let cas = root.join("cas");
        let dissolve_identity;
        {
            let runtime = Runtime::open_with_cas(&database, &cas).expect("initial Runtime");
            let project_id = runtime
                .create_project(
                    "AuthoringMesh typed dissolve identity",
                    json!({"profile":"test"}),
                )
                .expect("project")
                .project_id;
            let source = prepare_source_program(
                &runtime,
                &project_id,
                authoring_two_triangle_program(&project_id),
                "dissolve-source-durable",
            );
            let genesis_source_mesh_sha256 = source.source_mesh_sha256.clone();
            let genesis_request = identity_request(
                &source,
                &genesis_source_mesh_sha256,
                None,
                &"a".repeat(64),
                None,
                "dissolve-identity-genesis",
            );
            let genesis = runtime
                .authoring_mesh_identity_lineage_prepare(&genesis_request)
                .expect("identity genesis");

            let (preview, dissolve_source, operation_lineage_sha256, staged) =
                stage_typed_operation(
                    &runtime,
                    &source,
                    json!({
                        "operation":"dissolve_edge",
                        "edge_id":"e12",
                        "parent_revision":0
                    }),
                    "dissolve-edit-stage-once",
                    "dissolve-target-durable",
                );
            assert_eq!(preview["operation"], "dissolve_edge");
            let proof = &preview["edited_element_ids"]["typed_operation_proof"];
            assert_eq!(proof["operation"], "dissolve_edge");
            assert_eq!(proof["correspondence"][0]["kind"], "many-to-one");
            assert_eq!(proof["retired_face_ids"].as_array().unwrap().len(), 2);
            assert_eq!(proof["generated_face_ids"].as_array().unwrap().len(), 1);
            assert_eq!(staged["operation"], "dissolve_edge");
            assert_eq!(staged["version_status"], "no-version-created");
            assert_eq!(staged["confirm_status"], "approval-required");
            assert_eq!(staged["export_status"], "locked-until-confirm");

            dissolve_identity = runtime
                .authoring_mesh_identity_lineage_prepare(&identity_request(
                    &dissolve_source,
                    &genesis_source_mesh_sha256,
                    Some(&genesis),
                    &operation_lineage_sha256,
                    genesis["lineage_id"].as_str(),
                    "dissolve-identity-revision-1",
                ))
                .expect("materialize dissolve identity lineage");
            let lineage = &dissolve_identity["identity_lineage"];
            assert_eq!(lineage["revision_index"], 1);
            assert_eq!(lineage["revision_kind"], "topology-edit");
            let relation = lineage["correspondence"]
                .as_array()
                .expect("dissolve correspondence")
                .iter()
                .find(|value| {
                    value["kind"] == "many-to-one"
                        && value["operation_lineage_sha256"] == operation_lineage_sha256
                })
                .expect("dissolve many-to-one identity relation");
            assert_eq!(relation["parent_identity_ids"].as_array().unwrap().len(), 2);
            assert_eq!(relation["child_identity_ids"].as_array().unwrap().len(), 1);
            let tombstones = lineage["tombstones"]
                .as_array()
                .expect("dissolve tombstones");
            assert_eq!(
                tombstones
                    .iter()
                    .filter(|value| {
                        value["element_kind"] == "face"
                            && value["reason"] == "merged"
                            && value["retired_revision_index"] == 1
                            && value["operation_lineage_sha256"] == operation_lineage_sha256
                    })
                    .count(),
                2
            );
            assert!(tombstones.iter().any(|value| {
                value["element_kind"] == "edge"
                    && value["reason"] == "dissolved"
                    && value["operation_lineage_sha256"] == operation_lineage_sha256
            }));

            let same_runtime = runtime
                .authoring_mesh_identity_lineage_get(&identity_get_request(&dissolve_identity))
                .expect("dissolve identity same-runtime get");
            assert_eq!(
                same_runtime["identity_lineage"],
                dissolve_identity["identity_lineage"]
            );
            assert_eq!(same_runtime["runtime_write_performed"], false);
            assert_eq!(same_runtime["persistent_user_data_touched"], false);
            assert!(runtime
                .versions(Some(&project_id))
                .expect("versions")
                .is_empty());
        }

        let reopened = Runtime::open_with_cas(&database, &cas).expect("reopened Runtime");
        let restarted = reopened
            .authoring_mesh_identity_lineage_get(&identity_get_request(&dissolve_identity))
            .expect("dissolve identity restart get");
        assert_eq!(
            restarted["identity_lineage"],
            dissolve_identity["identity_lineage"]
        );
        assert_eq!(restarted["restart_hash_verified"], true);
        assert_eq!(restarted["runtime_write_performed"], false);
        drop(reopened);
        fs::remove_dir_all(root).expect("dissolve restart fixture cleanup");
    }
}
