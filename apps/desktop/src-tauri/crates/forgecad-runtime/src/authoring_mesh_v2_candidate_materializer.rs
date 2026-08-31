//! Runtime-only lowering from an immutable `AuthoringMeshRevision@2` to a
//! reviewable geometry candidate.
//!
//! This module is deliberately not a second authoring writer.  The source
//! revision remains the durable AuthoringMesh truth; this service only reads
//! that revision from Store/CAS, lowers it to the already-closed
//! `forgecad.geometry.authoring-mesh@1` Worker operator, and delegates the
//! candidate/artifact transaction to `prepare_geometry_candidate_exact`.  No
//! caller topology, GeometryProgram, GLB, path, script, or worker selection
//! is accepted.

use super::{
    authoring_mesh_v2, authoring_mesh_v2_durable,
    authoring_mesh_v2_geometry::{
        authoring_mesh_v2_geometry_parameters, authoring_mesh_v2_geometry_projection_sha256,
    },
    canonical_json_bytes, canonical_json_hash, is_opaque_id, is_sha256, sha256_hex, Runtime,
    RuntimeError,
};
use forgecad_contracts::AuthoringMeshRevision;
use forgecad_store::{AuthoringMeshV2DurableRecord, KnifeSourceBindingStoreRecord};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub(crate) const PREPARE_SCHEMA_VERSION: &str = "AuthoringMeshV2CandidateMaterializeRequest@1";
pub(crate) const RESULT_SCHEMA_VERSION: &str = "AuthoringMeshV2CandidateMaterializeResult@1";
pub(crate) const OPERATION: &str = "authoring_mesh_v2_candidate_materialize";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_REVISION_BYTES: u64 = 64 * 1024 * 1024;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-input-sha256@1";
const OPERATOR_ID: &str = "forgecad.geometry.authoring-mesh@1";
const MATERIALIZATION_PLAN_SCHEMA: &str =
    "AuthoringMeshV2CandidateMaterializationRepresentationPlan@1";
const MAX_SOURCE_REVISION_ANCESTRY_HOPS: usize = 64;

const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "operation",
    "project_id",
    "mesh_id",
    "lineage_id",
    "revision_id",
    "revision_index",
    "revision_sha256",
    "revision_object_sha256",
    "source_binding_id",
    "source_binding_sha256",
    "source_binding_object_sha256",
    "base_version_id",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "AUTHORING_MESH_V2_CANDIDATE_MATERIALIZE_INVALID: {}",
        message.into()
    ))
}

fn exact_object<'a>(value: &'a Value) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("request must be an object"))?;
    let expected = REQUEST_FIELDS.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if expected != actual {
        return Err(invalid(
            "request fields differ from the closed Runtime envelope",
        ));
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
        return Err(invalid(format!("{field} must be an opaque Runtime ID")));
    }
    Ok(value)
}

fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    let value = text(object, field)?;
    if !is_sha256(value) {
        return Err(invalid(format!("{field} must be a SHA-256")));
    }
    Ok(value)
}

fn nullable_id(object: &Map<String, Value>, field: &str) -> Result<Option<String>, RuntimeError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_opaque_id(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!(
            "{field} must be null or an opaque Runtime ID"
        ))),
    }
}

fn nullable_sha(object: &Map<String, Value>, field: &str) -> Result<Option<String>, RuntimeError> {
    match object.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if is_sha256(value) => Ok(Some(value.clone())),
        _ => Err(invalid(format!("{field} must be null or a SHA-256"))),
    }
}

fn input_sha256(request: &Value, object: &Map<String, Value>) -> Result<String, RuntimeError> {
    let input_sha256 = sha(object, "input_sha256")?.to_owned();
    let mut preimage = request.clone();
    preimage["input_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != input_sha256 {
        return Err(invalid("input_sha256 does not match the closed request"));
    }
    Ok(input_sha256)
}

#[derive(Debug, Clone)]
struct Request {
    input_sha256: String,
    project_id: String,
    mesh_id: String,
    lineage_id: String,
    revision_id: String,
    revision_index: u64,
    revision_sha256: String,
    revision_object_sha256: String,
    source_binding_id: Option<String>,
    source_binding_sha256: Option<String>,
    source_binding_object_sha256: Option<String>,
    base_version_id: Option<String>,
    idempotency_key: String,
}

/// Runtime-owned proof of the source program splice.  This is deliberately
/// kept separate from the public request: a caller may select an exact
/// SourceBinding, but may not supply the source program or any replacement
/// topology.  The proof is derived after reloading the candidate-owned CAS
/// program and is used both for the representation plan and the result
/// readback checks.
#[derive(Debug, Clone)]
struct SourceMaterialization {
    source_program: Value,
    source_reference_id: String,
    source_reference_sha256: String,
    source_candidate_id: String,
    source_candidate_state_sha256: String,
    source_artifact_sha256: String,
    source_artifact_readback_sha256: String,
    source_program_sha256: String,
    source_program_object_sha256: String,
    source_binding_id: String,
    source_binding_sha256: String,
    source_binding_object_sha256: String,
    source_node_id: String,
    source_part_id: String,
    source_material_zone_id: String,
    source_solid: bool,
    source_position_m: [f64; 3],
    source_rotation_rad: [f64; 3],
    source_part_output_sha256: String,
    source_part_bindings: Vec<Value>,
    preserved_part_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct MaterializationProof {
    materialization_mode: String,
    source: Option<SourceMaterialization>,
    replacement_node_id: String,
    preserved_part_ids: Vec<String>,
}

fn parse_request(value: &Value) -> Result<Request, RuntimeError> {
    let object = exact_object(value)?;
    if text(object, "schema_version")? != PREPARE_SCHEMA_VERSION
        || text(object, "operation")? != OPERATION
        || text(object, "writer_policy")? != WRITER_POLICY
        || text(object, "canonicalization_policy")? != CANONICALIZATION_POLICY
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
    {
        return Err(invalid("request policy or response budget differs"));
    }

    let source_binding_id = nullable_id(object, "source_binding_id")?;
    let source_binding_sha256 = nullable_sha(object, "source_binding_sha256")?;
    let source_binding_object_sha256 = nullable_sha(object, "source_binding_object_sha256")?;
    let source_binding_fields_present = [
        source_binding_id.is_some(),
        source_binding_sha256.is_some(),
        source_binding_object_sha256.is_some(),
    ];
    if source_binding_fields_present
        .iter()
        .any(|present| *present != source_binding_fields_present[0])
    {
        return Err(invalid(
            "source binding identity must be all-null or all-present",
        ));
    }

    Ok(Request {
        input_sha256: input_sha256(value, object)?,
        project_id: id(object, "project_id")?.to_owned(),
        mesh_id: id(object, "mesh_id")?.to_owned(),
        lineage_id: id(object, "lineage_id")?.to_owned(),
        revision_id: id(object, "revision_id")?.to_owned(),
        revision_index: object
            .get("revision_index")
            .and_then(Value::as_u64)
            .filter(|value| *value <= 1_000_000)
            .ok_or_else(|| invalid("revision_index must be <= 1,000,000"))?,
        revision_sha256: sha(object, "revision_sha256")?.to_owned(),
        revision_object_sha256: sha(object, "revision_object_sha256")?.to_owned(),
        source_binding_id,
        source_binding_sha256,
        source_binding_object_sha256,
        base_version_id: nullable_id(object, "base_version_id")?,
        idempotency_key: id(object, "idempotency_key")?.to_owned(),
    })
}

fn validate_source_binding(
    runtime: &Runtime,
    request: &Request,
    revision: &AuthoringMeshRevision,
) -> Result<Option<KnifeSourceBindingStoreRecord>, RuntimeError> {
    let Some(source_binding_id) = request.source_binding_id.as_deref() else {
        if request.source_binding_sha256.is_some() || request.source_binding_object_sha256.is_some()
        {
            return Err(invalid("source binding selector is incomplete"));
        }
        return Ok(None);
    };
    let source_binding_sha256 = request
        .source_binding_sha256
        .as_deref()
        .ok_or_else(|| invalid("source binding semantic hash is missing"))?;
    let source_binding_object_sha256 = request
        .source_binding_object_sha256
        .as_deref()
        .ok_or_else(|| invalid("source binding object hash is missing"))?;
    let record = runtime
        .store
        .get_knife_source_binding(
            &request.project_id,
            source_binding_id,
            source_binding_sha256,
        )?
        .ok_or_else(|| invalid("exact source binding is not durable"))?;
    if record.source_binding_object_sha256 != source_binding_object_sha256
        || record.project_id != request.project_id
        || record.authoring_mesh_id != request.mesh_id
        || record.authoring_mesh_lineage_id != request.lineage_id
    {
        return Err(invalid(
            "source binding does not match the requested mesh lineage",
        ));
    }
    validate_source_revision_ancestry(runtime, request, revision, &record)?;
    Ok(Some(record))
}

fn embedded_source_binding_value(revision: &AuthoringMeshRevision) -> Result<Value, RuntimeError> {
    let binding = revision.source_binding.as_ref().ok_or_else(|| {
        invalid("source-bound materialization requires an embedded AuthoringMesh source binding")
    })?;
    authoring_mesh_v2::validate_source_binding(binding)?;
    serde_json::to_value(binding)
        .map_err(|error| invalid(format!("cannot encode embedded source binding: {error}")))
}

/// Prove that the selected revision is either the immutable SourceBinding
/// anchor itself or a bounded, single-parent descendant of it.  The binding
/// remains the source root; a real correction is expressed by a later
/// AuthoringMesh revision, never by changing only a Candidate id.
fn validate_source_revision_ancestry(
    runtime: &Runtime,
    request: &Request,
    revision: &AuthoringMeshRevision,
    source: &KnifeSourceBindingStoreRecord,
) -> Result<(), RuntimeError> {
    let anchor_record = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(
            &request.project_id,
            &source.authoring_mesh_revision_id,
        )?
        .ok_or_else(|| invalid("SourceBinding anchor revision is not durable"))?;
    if anchor_record.mesh_id != source.authoring_mesh_id
        || anchor_record.lineage_id != source.authoring_mesh_lineage_id
        || anchor_record.revision_id != source.authoring_mesh_revision_id
        || anchor_record.revision_index != source.authoring_mesh_revision_index
        || anchor_record.revision_sha256 != source.authoring_mesh_revision_sha256
        || anchor_record.revision_object_sha256 != source.authoring_mesh_revision_object_sha256
    {
        return Err(invalid(
            "SourceBinding anchor revision identity differs from durable Store/CAS",
        ));
    }
    let anchor_revision = authoring_mesh_v2_durable::revision_from_cas(runtime, &anchor_record)?;
    let anchor_binding = embedded_source_binding_value(&anchor_revision)?;
    let binding = anchor_revision
        .source_binding
        .as_ref()
        .ok_or_else(|| invalid("SourceBinding anchor has no embedded source binding"))?;
    if binding.project_id != request.project_id
        || binding.candidate_id != source.source_candidate_id
        || binding.candidate_state_sha256 != source.source_candidate_state_sha256
    {
        return Err(invalid(
            "SourceBinding anchor embedded source identity differs from its durable record",
        ));
    }

    if revision.mesh_id.0 != source.authoring_mesh_id
        || revision.lineage_id.0 != source.authoring_mesh_lineage_id
        || revision.revision_index < source.authoring_mesh_revision_index
    {
        return Err(invalid(
            "selected revision is outside the SourceBinding mesh lineage",
        ));
    }

    let mut current_record = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(
            &request.project_id,
            &request.revision_id,
        )?
        .ok_or_else(|| invalid("selected SourceBinding descendant is not durable"))?;
    let mut current_revision = revision.clone();
    for _ in 0..=MAX_SOURCE_REVISION_ANCESTRY_HOPS {
        if embedded_source_binding_value(&current_revision)? != anchor_binding {
            return Err(invalid(
                "AuthoringMesh descendant changed its embedded SourceBinding",
            ));
        }
        if current_record.revision_id == anchor_record.revision_id {
            if current_record != anchor_record
                || current_revision.canonical_sha256 != source.authoring_mesh_revision_sha256
            {
                return Err(invalid(
                    "SourceBinding ancestry terminated at a drifted anchor revision",
                ));
            }
            return Ok(());
        }
        if current_record.revision_index <= anchor_record.revision_index
            || current_record.parent_revision_ids.len() != 1
            || current_revision.parent_revision_ids.len() != 1
        {
            return Err(invalid(
                "SourceBinding descendant must have a bounded single-parent ancestry",
            ));
        }
        let parent_id = current_record.parent_revision_ids[0].clone();
        if current_revision.parent_revision_ids[0].0 != parent_id {
            return Err(invalid(
                "durable and CAS AuthoringMesh parent identities differ",
            ));
        }
        let parent_record = runtime
            .store
            .get_authoring_mesh_v2_durable_record_by_revision(&request.project_id, &parent_id)?
            .ok_or_else(|| invalid("SourceBinding descendant parent is not durable"))?;
        if parent_record.mesh_id != source.authoring_mesh_id
            || parent_record.lineage_id != source.authoring_mesh_lineage_id
            || parent_record.revision_index.checked_add(1) != Some(current_record.revision_index)
        {
            return Err(invalid(
                "SourceBinding descendant parent lineage or revision index differs",
            ));
        }
        current_revision = authoring_mesh_v2_durable::revision_from_cas(runtime, &parent_record)?;
        current_record = parent_record;
    }
    Err(invalid(
        "SourceBinding descendant ancestry exceeds the bounded correction budget",
    ))
}

fn load_source_materialization(
    runtime: &Runtime,
    request: &Request,
    revision: &AuthoringMeshRevision,
    record: &KnifeSourceBindingStoreRecord,
) -> Result<SourceMaterialization, RuntimeError> {
    let binding = revision.source_binding.as_ref().ok_or_else(|| {
        invalid("source-bound materialization requires an embedded AuthoringMesh source binding")
    })?;
    authoring_mesh_v2::validate_source_binding(binding)?;
    if binding.project_id != request.project_id
        || binding.candidate_id != record.source_candidate_id
        || binding.candidate_state_sha256 != record.source_candidate_state_sha256
        || binding.artifact_sha256.is_empty()
    {
        return Err(invalid(
            "embedded AuthoringMesh source binding does not match the durable SourceBinding",
        ));
    }

    let candidate = runtime
        .candidate(&record.source_candidate_id)?
        .ok_or_else(|| invalid("source candidate is not durable"))?;
    if candidate.project_id != request.project_id
        || candidate.canonical_sha256 != record.source_candidate_state_sha256
        || candidate.state != "reviewable"
        || !candidate.quality_hard_gate_passed
    {
        return Err(invalid(
            "source candidate state/hash is not the exact reviewable source",
        ));
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&record.source_candidate_id)?
        .ok_or_else(|| invalid("source candidate GeometryCandidateEvidence is not durable"))?;
    if evidence.project_id != request.project_id
        || evidence.candidate_id != record.source_candidate_id
        || evidence.reference_id.as_deref() != Some(record.reference_id.as_str())
        || evidence.reference_sha256.as_deref() != Some(record.reference_object_sha256.as_str())
        || candidate.prepared_object_sha256.as_deref()
            != Some(evidence.artifact_object_sha256.as_str())
        || binding.artifact_sha256 != evidence.artifact_object_sha256
        || binding.geometry_program_sha256 != evidence.geometry_program_sha256
    {
        return Err(invalid(
            "source candidate artifact/evidence/source binding hashes differ",
        ));
    }

    let program_object = runtime
        .store
        .get_object(&evidence.geometry_program_object_sha256)?
        .ok_or_else(|| invalid("source GeometryProgram CAS metadata is missing"))?;
    if program_object.sha256 != evidence.geometry_program_object_sha256
        || program_object.mime != "application/json"
        || program_object.kind != "geometry-program-v2"
        || program_object.size_bytes == 0
        || program_object.size_bytes > 64 * 1024 * 1024
    {
        return Err(invalid("source GeometryProgram CAS metadata is invalid"));
    }
    let program_bytes =
        runtime.cas_read_bounded(&evidence.geometry_program_object_sha256, 64 * 1024 * 1024)?;
    if program_bytes.len() as u64 != program_object.size_bytes
        || sha256_hex(&program_bytes) != program_object.sha256
    {
        return Err(invalid(
            "source GeometryProgram CAS bytes are not hash-bound",
        ));
    }
    let source_program: Value = serde_json::from_slice(&program_bytes)
        .map_err(|error| invalid(format!("source GeometryProgram JSON is invalid: {error}")))?;
    if !source_program.is_object()
        || source_program.get("canonical_sha256").is_some()
        || canonical_json_bytes(&source_program).map_err(|error| invalid(error.to_string()))?
            != program_bytes
        || canonical_json_hash(&source_program) != evidence.geometry_program_sha256
        || source_program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || source_program.get("project_id").and_then(Value::as_str)
            != Some(request.project_id.as_str())
        || source_program
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(super::operator_catalog_sha256().as_str())
    {
        return Err(invalid(
            "source GeometryProgram draft or semantic hash is not the active Runtime truth",
        ));
    }

    let artifact_object = runtime
        .store
        .get_object(&evidence.artifact_object_sha256)?
        .ok_or_else(|| invalid("source candidate artifact CAS metadata is missing"))?;
    if artifact_object.sha256 != evidence.artifact_object_sha256
        || artifact_object.mime != "model/gltf-binary"
        || artifact_object.kind != "geometry-glb"
        || artifact_object.size_bytes == 0
        || artifact_object.size_bytes > 64 * 1024 * 1024
    {
        return Err(invalid("source candidate artifact CAS metadata is invalid"));
    }
    let artifact_bytes =
        runtime.cas_read_bounded(&evidence.artifact_object_sha256, 64 * 1024 * 1024)?;
    if artifact_bytes.len() as u64 != artifact_object.size_bytes
        || sha256_hex(&artifact_bytes) != artifact_object.sha256
    {
        return Err(invalid(
            "source candidate artifact CAS bytes are not hash-bound",
        ));
    }

    let readback_object = runtime
        .store
        .get_object(&evidence.artifact_readback_object_sha256)?
        .ok_or_else(|| invalid("source ArtifactReadback CAS metadata is missing"))?;
    if readback_object.mime != "application/json"
        || readback_object.kind != "geometry-artifact-readback-v2"
        || readback_object.size_bytes == 0
        || readback_object.size_bytes > 8 * 1024 * 1024
    {
        return Err(invalid("source ArtifactReadback CAS metadata is invalid"));
    }
    let readback_bytes =
        runtime.cas_read_bounded(&evidence.artifact_readback_object_sha256, 8 * 1024 * 1024)?;
    if readback_bytes.len() as u64 != readback_object.size_bytes
        || sha256_hex(&readback_bytes) != readback_object.sha256
    {
        return Err(invalid(
            "source ArtifactReadback CAS bytes are not hash-bound",
        ));
    }
    let readback: Value = serde_json::from_slice(&readback_bytes)
        .map_err(|error| invalid(format!("source ArtifactReadback JSON is invalid: {error}")))?;
    let readback_canonical = readback
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("source ArtifactReadback canonical hash is invalid"))?;
    let mut readback_preimage = readback.clone();
    readback_preimage["canonical_sha256"] = Value::String(String::new());
    if readback.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
        || readback.get("candidate_id").and_then(Value::as_str)
            != Some(record.source_candidate_id.as_str())
        || readback.get("object_sha256").and_then(Value::as_str)
            != Some(evidence.artifact_object_sha256.as_str())
        || readback.get("program_sha256").and_then(Value::as_str)
            != Some(evidence.geometry_program_sha256.as_str())
        || canonical_json_hash(&readback_preimage) != readback_canonical
        || binding.artifact_readback_sha256 != readback_canonical
    {
        return Err(invalid(
            "source ArtifactReadback is not bound to the exact candidate artifact/program",
        ));
    }
    let source_part_bindings = readback
        .get("part_bindings")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| invalid("source ArtifactReadback part_bindings are missing"))?;

    let nodes = source_program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source GeometryProgram nodes are missing"))?;
    let matching_nodes = nodes
        .iter()
        .filter(|node| {
            node.get("node_id").and_then(Value::as_str) == Some(binding.source_node_id.as_str())
        })
        .collect::<Vec<_>>();
    if matching_nodes.len() != 1 {
        return Err(invalid("bound source node is missing or duplicated"));
    }
    let source_node = matching_nodes[0];
    if source_node.get("operator_id").and_then(Value::as_str)
        != Some(binding.source_operator_id.as_str())
        || source_node
            .get("inputs")
            .and_then(Value::as_array)
            .is_none_or(|inputs| !inputs.is_empty())
    {
        return Err(invalid("bound source node is not a direct source node"));
    }
    let source_parameters = source_node
        .get("parameters")
        .ok_or_else(|| invalid("bound source node parameters are missing"))?;
    if canonical_json_hash(source_parameters) != binding.source_parameters_sha256 {
        return Err(invalid("bound source node parameters hash drifted"));
    }
    if nodes.iter().any(|node| {
        node.get("inputs")
            .and_then(Value::as_array)
            .is_some_and(|inputs| {
                inputs
                    .iter()
                    .any(|input| input.as_str() == Some(binding.source_node_id.as_str()))
            })
    }) {
        return Err(invalid(
            "bound source node has a downstream consumer and cannot be replaced atomically",
        ));
    }

    let outputs = source_program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("source GeometryProgram part_outputs are missing"))?;
    let matching_parts = outputs
        .iter()
        .filter(|part| {
            part.get("part_id").and_then(Value::as_str) == Some(binding.part_id.as_str())
        })
        .collect::<Vec<_>>();
    if matching_parts.len() != 1 {
        return Err(invalid("bound source part is missing or duplicated"));
    }
    let source_part = matching_parts[0];
    let input_node_ids = source_part
        .get("input_node_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("bound source part input_node_ids are missing"))?;
    if input_node_ids
        .iter()
        .filter(|value| value.as_str() == Some(binding.source_node_id.as_str()))
        .count()
        != 1
        || source_part.get("material_zone_id").and_then(Value::as_str)
            != Some(binding.material_zone_id.as_str())
        || source_part.get("solid").and_then(Value::as_bool) != Some(binding.solid)
        || canonical_json_hash(source_part) != binding.part_output_sha256
    {
        return Err(invalid(
            "bound source part output identity or semantics drifted",
        ));
    }
    if source_part_bindings.iter().any(|value| {
        value.get("part_id").and_then(Value::as_str) == Some(binding.part_id.as_str())
            && value.get("source_node_id").and_then(Value::as_str)
                == Some(binding.source_node_id.as_str())
            && (value.get("material_zone_id").and_then(Value::as_str)
                != Some(binding.material_zone_id.as_str())
                || value.get("solid").and_then(Value::as_bool) != Some(binding.solid))
    }) {
        return Err(invalid(
            "source ArtifactReadback bound part semantics drifted",
        ));
    }
    let target_binding_count = source_part_bindings
        .iter()
        .filter(|value| {
            value.get("part_id").and_then(Value::as_str) == Some(binding.part_id.as_str())
                && value.get("source_node_id").and_then(Value::as_str)
                    == Some(binding.source_node_id.as_str())
        })
        .count();
    if target_binding_count != 1 {
        return Err(invalid(
            "source ArtifactReadback bound part is missing or duplicated",
        ));
    }

    let mut preserved_part_ids = Vec::new();
    let mut seen_part_ids = BTreeSet::new();
    for part in outputs {
        let part_id = part
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("source part output id is missing"))?;
        if !seen_part_ids.insert(part_id.to_owned()) {
            return Err(invalid("source GeometryProgram part IDs are duplicated"));
        }
        if part_id != binding.part_id {
            preserved_part_ids.push(part_id.to_owned());
        }
    }
    // ArtifactReadback binds emitted source nodes, not just Part outputs. A
    // single semantic Part may therefore have multiple bindings when its
    // `input_node_ids` contains multiple independent source nodes (for
    // example two grip fasteners). Validate the exact (Part, node) relation
    // instead of incorrectly requiring one binding per Part.
    let mut expected_binding_keys = BTreeSet::new();
    for part in outputs {
        let part_id = part
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("source part output id is missing"))?;
        let material_zone_id = part
            .get("material_zone_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("source part output material zone is missing"))?;
        let solid = part
            .get("solid")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("source part output solid flag is missing"))?;
        let input_node_ids = part
            .get("input_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("source part output input_node_ids are missing"))?;
        if input_node_ids.is_empty() {
            return Err(invalid("source part output has no input nodes"));
        }
        for node_id in input_node_ids {
            let node_id = node_id
                .as_str()
                .ok_or_else(|| invalid("source part output node id is invalid"))?;
            if !expected_binding_keys.insert((part_id.to_owned(), node_id.to_owned())) {
                return Err(invalid(
                    "source GeometryProgram Part/node bindings are duplicated",
                ));
            }
            let matching = source_part_bindings
                .iter()
                .filter(|value| {
                    value.get("part_id").and_then(Value::as_str) == Some(part_id)
                        && value.get("source_node_id").and_then(Value::as_str) == Some(node_id)
                })
                .collect::<Vec<_>>();
            if matching.len() != 1
                || matching[0].get("material_zone_id").and_then(Value::as_str)
                    != Some(material_zone_id)
                || matching[0].get("solid").and_then(Value::as_bool) != Some(solid)
            {
                return Err(invalid(
                    "source ArtifactReadback Part/node semantics are not exact",
                ));
            }
        }
    }
    let actual_binding_keys = source_part_bindings
        .iter()
        .map(|value| {
            let part_id = value
                .get("part_id")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("source ArtifactReadback Part id is missing"))?;
            let node_id = value
                .get("source_node_id")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("source ArtifactReadback source node id is missing"))?;
            Ok((part_id.to_owned(), node_id.to_owned()))
        })
        .collect::<Result<BTreeSet<_>, RuntimeError>>()?;
    if actual_binding_keys.len() != source_part_bindings.len()
        || actual_binding_keys != expected_binding_keys
    {
        return Err(invalid(
            "source ArtifactReadback Part/node set is not exact",
        ));
    }
    preserved_part_ids.sort();

    Ok(SourceMaterialization {
        source_program,
        source_reference_id: record.reference_id.clone(),
        source_reference_sha256: record.reference_object_sha256.clone(),
        source_candidate_id: record.source_candidate_id.clone(),
        source_candidate_state_sha256: record.source_candidate_state_sha256.clone(),
        source_artifact_sha256: evidence.artifact_object_sha256,
        source_artifact_readback_sha256: readback_canonical.to_owned(),
        source_program_sha256: evidence.geometry_program_sha256,
        source_program_object_sha256: evidence.geometry_program_object_sha256,
        source_binding_id: record.source_binding_id.clone(),
        source_binding_sha256: record.source_binding_sha256.clone(),
        source_binding_object_sha256: record.source_binding_object_sha256.clone(),
        source_node_id: binding.source_node_id.clone(),
        source_part_id: binding.part_id.clone(),
        source_material_zone_id: binding.material_zone_id.clone(),
        source_solid: binding.solid,
        source_position_m: binding.position_m,
        source_rotation_rad: binding.rotation_rad,
        source_part_output_sha256: binding.part_output_sha256.clone(),
        source_part_bindings,
        preserved_part_ids,
    })
}

fn load_revision(
    runtime: &Runtime,
    request: &Request,
) -> Result<(AuthoringMeshV2DurableRecord, AuthoringMeshRevision), RuntimeError> {
    let record = runtime
        .store
        .get_authoring_mesh_v2_durable_record_by_revision(
            &request.project_id,
            &request.revision_id,
        )?
        .ok_or_else(|| invalid("exact AuthoringMesh@2 revision is not durable"))?;
    if record.mesh_id != request.mesh_id
        || record.lineage_id != request.lineage_id
        || record.revision_id != request.revision_id
        || record.revision_index != request.revision_index
        || record.revision_sha256 != request.revision_sha256
        || record.revision_object_sha256 != request.revision_object_sha256
    {
        return Err(invalid("durable revision identity differs from request"));
    }
    let revision = authoring_mesh_v2_durable::revision_from_cas(runtime, &record)?;
    if revision.schema_version != "AuthoringMeshRevision@2"
        || revision.mesh_id.0 != request.mesh_id
        || revision.lineage_id.0 != request.lineage_id
        || revision.revision_id.0 != request.revision_id
        || revision.revision_index != request.revision_index
        || revision.canonical_sha256 != request.revision_sha256
    {
        return Err(invalid("CAS revision identity differs from request"));
    }
    if runtime
        .store
        .get_object(&request.revision_object_sha256)?
        .is_none()
    {
        return Err(invalid("revision object is not present in CAS metadata"));
    }
    Ok((record, revision))
}

fn build_geometry_program(
    request: &Request,
    revision: &AuthoringMeshRevision,
    source: Option<&SourceMaterialization>,
) -> Result<(Value, String, MaterializationProof), RuntimeError> {
    let (position_m, rotation_rad) = source
        .map(|value| (value.source_position_m, value.source_rotation_rad))
        .unwrap_or(([0.0; 3], [0.0; 3]));
    let parameters = authoring_mesh_v2_geometry_parameters(revision, position_m, rotation_rad)?;
    let projection_sha256 = authoring_mesh_v2_geometry_projection_sha256(revision, &parameters);
    let materialization_mode = if source.is_some() {
        "source_binding_part_replacement"
    } else {
        "standalone_revision"
    };
    // Keep the replacement node identity independent from the representation
    // plan.  The plan records the replacement node, so deriving either hash
    // from the other would create a self-referential identity.
    let replacement_identity = json!({
        "schema_version":"AuthoringMeshV2CandidateReplacementIdentity@1",
        "project_id":request.project_id,
        "mesh_id":request.mesh_id,
        "lineage_id":request.lineage_id,
        "materialization_mode":materialization_mode,
        "revision_id":request.revision_id,
        "revision_sha256":request.revision_sha256,
        "revision_object_sha256":request.revision_object_sha256,
        "projection_sha256":projection_sha256,
        "source_binding_id":source.map(|value| value.source_binding_id.clone()),
        "source_binding_sha256":source.map(|value| value.source_binding_sha256.clone()),
        "source_node_id":source.map(|value| value.source_node_id.clone()),
        "source_part_id":source.map(|value| value.source_part_id.clone()),
    });
    let replacement_node_id = format!(
        "authoring-mesh-v2-{}",
        &canonical_json_hash(&replacement_identity)[..32]
    );
    let source_field =
        |selector: fn(&SourceMaterialization) -> Value| source.map(selector).unwrap_or(Value::Null);
    let plan = json!({
        "schema_version": MATERIALIZATION_PLAN_SCHEMA,
        "project_id": request.project_id,
        "mesh_id": request.mesh_id,
        "lineage_id": request.lineage_id,
        "materialization_mode": materialization_mode,
        "revision_id": request.revision_id,
        "revision_index": request.revision_index,
        "revision_sha256": request.revision_sha256,
        "revision_object_sha256": request.revision_object_sha256,
        "replacement_revision_id": request.revision_id,
        "replacement_revision_sha256": request.revision_sha256,
        "replacement_revision_object_sha256": request.revision_object_sha256,
        "replacement_projection_sha256": projection_sha256,
        "replacement_node_id": replacement_node_id,
        "source_candidate_id": source_field(|value| json!(value.source_candidate_id)),
        "source_candidate_state_sha256": source_field(|value| json!(value.source_candidate_state_sha256)),
        "source_artifact_sha256": source_field(|value| json!(value.source_artifact_sha256)),
        "source_artifact_readback_sha256": source_field(|value| json!(value.source_artifact_readback_sha256)),
        "source_program_sha256": source_field(|value| json!(value.source_program_sha256)),
        "source_program_object_sha256": source_field(|value| json!(value.source_program_object_sha256)),
        "source_binding_id": source_field(|value| json!(value.source_binding_id)),
        "source_binding_sha256": source_field(|value| json!(value.source_binding_sha256)),
        "source_binding_object_sha256": source_field(|value| json!(value.source_binding_object_sha256)),
        "source_node_id": source_field(|value| json!(value.source_node_id)),
        "source_part_id": source_field(|value| json!(value.source_part_id)),
        "source_material_zone_id": source_field(|value| json!(value.source_material_zone_id)),
        "source_solid": source.map(|value| json!(value.source_solid)).unwrap_or(Value::Null),
        "source_part_output_sha256": source_field(|value| json!(value.source_part_output_sha256)),
    });
    let representation_plan_sha256 = canonical_json_hash(&plan);
    let replacement_node = json!({
        "node_id":replacement_node_id,
        "operator_id":OPERATOR_ID,
        "inputs":[],
        "parameters":parameters,
    });
    let (mut program, preserved_part_ids) = if let Some(source) = source {
        if source.source_program_object_sha256 != source.source_program_sha256
            || canonical_json_hash(&source.source_program) != source.source_program_sha256
        {
            return Err(invalid(
                "source GeometryProgram semantic/object hash drifted before Worker execution",
            ));
        }
        let mut program = source.source_program.clone();
        let nodes = program
            .get_mut("nodes")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("source GeometryProgram nodes are missing"))?;
        let node_indices = nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                (node.get("node_id").and_then(Value::as_str)
                    == Some(source.source_node_id.as_str()))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        if node_indices.len() != 1 {
            return Err(invalid("source replacement node is missing or duplicated"));
        }
        if nodes.iter().any(|node| {
            node.get("node_id").and_then(Value::as_str) == Some(replacement_node_id.as_str())
        }) {
            return Err(invalid(
                "Runtime-derived replacement node collides with the source program",
            ));
        }
        nodes[node_indices[0]] = replacement_node;

        let outputs = program
            .get_mut("part_outputs")
            .and_then(Value::as_array_mut)
            .ok_or_else(|| invalid("source GeometryProgram part_outputs are missing"))?;
        let output_indices = outputs
            .iter()
            .enumerate()
            .filter_map(|(index, part)| {
                (part.get("part_id").and_then(Value::as_str)
                    == Some(source.source_part_id.as_str()))
                .then_some(index)
            })
            .collect::<Vec<_>>();
        if output_indices.len() != 1 {
            return Err(invalid("source replacement part is missing or duplicated"));
        }
        let source_output = &outputs[output_indices[0]];
        let source_input_node_ids = source_output
            .get("input_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("source Part output input_node_ids are missing"))?;
        if source_input_node_ids
            .iter()
            .filter(|value| value.as_str() == Some(source.source_node_id.as_str()))
            .count()
            != 1
            || source_output
                .get("material_zone_id")
                .and_then(Value::as_str)
                != Some(source.source_material_zone_id.as_str())
            || source_output.get("solid").and_then(Value::as_bool) != Some(source.source_solid)
            || canonical_json_hash(source_output) != source.source_part_output_sha256
        {
            return Err(invalid(
                "source Part output semantic/hash drifted before Worker execution",
            ));
        }
        let mut replacement_input_node_ids = source_input_node_ids.clone();
        for node_id in &mut replacement_input_node_ids {
            if node_id.as_str() == Some(source.source_node_id.as_str()) {
                *node_id = Value::String(replacement_node_id.clone());
            }
        }
        outputs[output_indices[0]]["input_node_ids"] = Value::Array(replacement_input_node_ids);
        (program, source.preserved_part_ids.clone())
    } else {
        let face_count = parameters
            .get("faces")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("Worker projection omitted faces"))?
            .len();
        let triangle_budget = u64::try_from(face_count)
            .ok()
            .and_then(|count| count.checked_mul(2))
            .filter(|value| *value > 0)
            .ok_or_else(|| invalid("Worker projection face budget is invalid"))?;
        (
            json!({
                "schema_version": "GeometryProgram@2",
                "project_id": request.project_id,
                "representation_plan_sha256": representation_plan_sha256,
                "operator_catalog_sha256": super::operator_catalog_sha256(),
                "units": {"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
                "budgets": {"max_nodes":1,"max_triangles":triangle_budget,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
                "nodes": [replacement_node],
                "part_outputs": [{"part_id":format!("authoring-part-{}", revision.revision_id.0),"input_node_ids":[replacement_node_id],"material_zone_id":"weaponry-authoring-mesh","solid":false}],
            }),
            Vec::new(),
        )
    };
    program["representation_plan_sha256"] = Value::String(representation_plan_sha256.clone());
    // Worker receives a hash-bearing transient envelope, then persists the
    // canonical GeometryProgram draft without this field in CAS.
    let program_sha256 = canonical_json_hash(&program);
    program["canonical_sha256"] = Value::String(program_sha256.clone());
    Ok((
        program,
        representation_plan_sha256,
        MaterializationProof {
            materialization_mode: materialization_mode.to_owned(),
            source: source.cloned(),
            replacement_node_id,
            preserved_part_ids,
        },
    ))
}

fn geometry_request_sha256(
    project_id: &str,
    base_version_id: Option<&str>,
    request: &Value,
) -> Result<String, RuntimeError> {
    let envelope = serde_json::json!({
        "project_id": project_id,
        "tool": "geometry_prepare",
        "base_version_id_present": true,
        "base_version_id": base_version_id,
        "request": request,
    });
    let bytes = canonical_json_bytes(&envelope).map_err(|error| invalid(error.to_string()))?;
    Ok(sha256_hex(&bytes))
}

/// Verify the one semantic promise that distinguishes source-bound
/// materialization from a standalone projection: every source Part remains
/// present, and only the selected Part changes its source node.  The Worker
/// readback is the authoritative derived representation; this check therefore
/// runs after the real Worker transaction and before exposing the result.
fn validate_materialized_artifact(
    proof: &MaterializationProof,
    geometry_result: &Value,
) -> Result<(), RuntimeError> {
    let Some(source) = proof.source.as_ref() else {
        return Ok(());
    };
    let artifact = geometry_result
        .get("artifact")
        .ok_or_else(|| invalid("Geometry prepare omitted ArtifactReadback"))?;
    let derived = artifact
        .get("part_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("derived ArtifactReadback part_bindings are missing"))?;
    if derived.len() != source.source_part_bindings.len() || derived.is_empty() {
        return Err(invalid(
            "derived ArtifactReadback Part set differs from the source candidate",
        ));
    }

    let mut source_by_key = std::collections::BTreeMap::<(String, String), &Value>::new();
    for binding in &source.source_part_bindings {
        let part_id = binding
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("source ArtifactReadback Part id is missing"))?;
        let source_node_id = binding
            .get("source_node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("source ArtifactReadback source_node_id is missing"))?;
        if source_by_key
            .insert((part_id.to_owned(), source_node_id.to_owned()), binding)
            .is_some()
        {
            return Err(invalid(
                "source ArtifactReadback Part/node set is duplicated",
            ));
        }
    }
    let mut derived_by_key = std::collections::BTreeMap::<(String, String), &Value>::new();
    for binding in derived {
        let part_id = binding
            .get("part_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("derived ArtifactReadback Part id is missing"))?;
        let source_node_id = binding
            .get("source_node_id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("derived ArtifactReadback source_node_id is missing"))?;
        if derived_by_key
            .insert((part_id.to_owned(), source_node_id.to_owned()), binding)
            .is_some()
        {
            return Err(invalid(
                "derived ArtifactReadback Part/node set is duplicated",
            ));
        }
    }
    let expected_derived_keys = source_by_key
        .keys()
        .map(|(part_id, node_id)| {
            if part_id == &source.source_part_id && node_id == &source.source_node_id {
                (part_id.clone(), proof.replacement_node_id.clone())
            } else {
                (part_id.clone(), node_id.clone())
            }
        })
        .collect::<BTreeSet<_>>();
    if expected_derived_keys.len() != source_by_key.len()
        || derived_by_key.keys().cloned().collect::<BTreeSet<_>>() != expected_derived_keys
    {
        return Err(invalid(
            "derived ArtifactReadback is missing or adding a source Part/node binding",
        ));
    }

    for ((part_id, source_node_id), source_binding) in source_by_key {
        let is_replaced =
            part_id == source.source_part_id && source_node_id == source.source_node_id;
        let expected_source_node_id = if is_replaced {
            proof.replacement_node_id.as_str()
        } else {
            source_node_id.as_str()
        };
        let derived_binding = derived_by_key
            .get(&(part_id.clone(), expected_source_node_id.to_owned()))
            .copied()
            .ok_or_else(|| invalid("derived ArtifactReadback source Part is missing"))?;
        if derived_binding
            .get("source_node_id")
            .and_then(Value::as_str)
            != Some(expected_source_node_id)
            || derived_binding.get("material_zone_id") != source_binding.get("material_zone_id")
            || derived_binding.get("solid") != source_binding.get("solid")
        {
            return Err(invalid(format!(
                "derived Part {part_id} changed source/material semantics unexpectedly"
            )));
        }
        let triangle_count = derived_binding
            .get("triangle_count")
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .ok_or_else(|| invalid("derived ArtifactReadback triangle_count is invalid"))?;
        if !is_replaced
            && derived_binding.get("triangle_count") != source_binding.get("triangle_count")
        {
            return Err(invalid(format!(
                "untouched source Part {part_id} changed triangle_count"
            )));
        }
        let _ = triangle_count;
    }
    Ok(())
}

fn result_value(
    request: &Request,
    durable: &AuthoringMeshV2DurableRecord,
    source_binding: Option<&KnifeSourceBindingStoreRecord>,
    proof: &MaterializationProof,
    representation_plan_sha256: &str,
    geometry_idempotency_key: &str,
    geometry_result: Value,
    replayed: bool,
) -> Result<Value, RuntimeError> {
    let candidate = geometry_result
        .get("candidate")
        .cloned()
        .ok_or_else(|| invalid("Geometry prepare omitted candidate"))?;
    let artifact = geometry_result
        .get("artifact")
        .cloned()
        .ok_or_else(|| invalid("Geometry prepare omitted ArtifactReadback"))?;
    let job = geometry_result
        .get("job")
        .cloned()
        .ok_or_else(|| invalid("Geometry prepare omitted Job"))?;
    let source_field = |selector: fn(&SourceMaterialization) -> Value| {
        proof.source.as_ref().map(selector).unwrap_or(Value::Null)
    };
    let mut result = serde_json::json!({
        "schema_version": RESULT_SCHEMA_VERSION,
        "operation": OPERATION,
        "request_kind": "prepare",
        "status": if replayed { "replayed" } else { "prepared" },
        "project_id": request.project_id,
        "mesh_id": request.mesh_id,
        "lineage_id": request.lineage_id,
        "revision_id": request.revision_id,
        "revision_index": durable.revision_index,
        "revision_sha256": durable.revision_sha256,
        "revision_object_sha256": durable.revision_object_sha256,
        "source_binding_id": source_binding.map(|value| value.source_binding_id.clone()),
        "source_binding_sha256": source_binding.map(|value| value.source_binding_sha256.clone()),
        "source_binding_object_sha256": source_binding.map(|value| value.source_binding_object_sha256.clone()),
        "representation_plan_sha256": representation_plan_sha256,
        "materialization_mode": proof.materialization_mode,
        "source_candidate_id": source_field(|value| json!(value.source_candidate_id)),
        "source_candidate_state_sha256": source_field(|value| json!(value.source_candidate_state_sha256)),
        "source_artifact_sha256": source_field(|value| json!(value.source_artifact_sha256)),
        "source_artifact_readback_sha256": source_field(|value| json!(value.source_artifact_readback_sha256)),
        "source_program_sha256": source_field(|value| json!(value.source_program_sha256)),
        "source_program_object_sha256": source_field(|value| json!(value.source_program_object_sha256)),
        "source_node_id": source_field(|value| json!(value.source_node_id)),
        "source_part_id": source_field(|value| json!(value.source_part_id)),
        "source_material_zone_id": source_field(|value| json!(value.source_material_zone_id)),
        "source_solid": proof
            .source
            .as_ref()
            .map(|value| json!(value.source_solid))
            .unwrap_or(Value::Null),
        "source_part_output_sha256": source_field(|value| json!(value.source_part_output_sha256)),
        "replacement_node_id": proof.replacement_node_id,
        "preserved_part_ids": proof.preserved_part_ids,
        "geometry_idempotency_key": geometry_idempotency_key,
        "candidate": candidate,
        "artifact": artifact,
        "job": job,
        "replayed": replayed,
        // A prepare call does not drop and reopen the Runtime.  Keep this
        // false so a later restart probe, rather than the producer itself,
        // is the only source of a positive restart-hash claim.
        "restart_hash_verified": false,
        "runtime_write_performed": !replayed,
        "persistent_user_data_touched": !replayed,
        "quality_status": "structural_only",
        "visual_status": "NOT_RUN",
        "human_status": "NOT_RUN",
        "engine_status": "NOT_RUN",
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "request_input_sha256": request.input_sha256,
        "idempotency_key": if replayed { Value::Null } else { Value::String(request.idempotency_key.clone()) },
        "canonical_sha256": "",
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    let bytes = canonical_json_bytes(&result).map_err(|error| invalid(error.to_string()))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(invalid("materialization response exceeds 1 MiB"));
    }
    Ok(result)
}

pub(crate) fn prepare(runtime: &Runtime, value: &Value) -> Result<Value, RuntimeError> {
    let request = parse_request(value)?;
    let (durable, revision) = load_revision(runtime, &request)?;
    let source_binding = validate_source_binding(runtime, &request, &revision)?;
    let source = source_binding
        .as_ref()
        .map(|record| load_source_materialization(runtime, &request, &revision, record))
        .transpose()?;
    let (program, representation_plan_sha256, proof) =
        build_geometry_program(&request, &revision, source.as_ref())?;
    let mut geometry_request = serde_json::json!({
        "typed": "geometry",
        "geometry_program": program,
    });
    if let Some(source) = source.as_ref() {
        geometry_request["reference_id"] = Value::String(source.source_reference_id.clone());
    }
    let geometry_idempotency_key = format!(
        "authoring-v2-{}",
        &sha256_hex(request.idempotency_key.as_bytes())[..48]
    );
    let request_sha256 = geometry_request_sha256(
        &request.project_id,
        request.base_version_id.as_deref(),
        &geometry_request,
    )?;
    let replayed = runtime
        .store
        .geometry_prepare_idempotency_response(
            &request.project_id,
            "geometry_prepare",
            &geometry_idempotency_key,
            &request_sha256,
        )?
        .is_some();
    let geometry_result = runtime.prepare_geometry_candidate_exact_bounded(
        &request.project_id,
        request.base_version_id.as_deref(),
        &geometry_idempotency_key,
        geometry_request,
        MAX_REVISION_BYTES as usize,
    )?;
    validate_materialized_artifact(&proof, &geometry_result)?;
    let candidate_id = geometry_result
        .get("candidate")
        .and_then(|value| value.get("candidate_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("Geometry prepare candidate identity is missing"))?;
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .ok_or_else(|| invalid("Geometry candidate evidence is not durable"))?;
    if evidence.project_id != request.project_id
        || evidence.geometry_program_sha256
            != geometry_result["artifact"]["program_sha256"]
                .as_str()
                .unwrap_or_default()
        || source.as_ref().is_some_and(|source| {
            evidence.reference_id.as_deref() != Some(source.source_reference_id.as_str())
                || evidence.reference_sha256.as_deref()
                    != Some(source.source_reference_sha256.as_str())
        })
    {
        return Err(invalid(
            "candidate evidence does not bind the generated program",
        ));
    }
    let program_bytes =
        runtime.cas_read_bounded(&evidence.geometry_program_object_sha256, MAX_REVISION_BYTES)?;
    let stored_program: Value = serde_json::from_slice(&program_bytes)
        .map_err(|error| invalid(format!("stored GeometryProgram is invalid: {error}")))?;
    if !stored_program.is_object()
        || stored_program.get("canonical_sha256").is_some()
        || canonical_json_bytes(&stored_program).map_err(|error| invalid(error.to_string()))?
            != program_bytes
        || canonical_json_hash(&stored_program) != evidence.geometry_program_sha256
        || stored_program
            .get("representation_plan_sha256")
            .and_then(Value::as_str)
            != Some(representation_plan_sha256.as_str())
    {
        return Err(invalid(
            "stored GeometryProgram canonical draft or Runtime source binding plan drifted",
        ));
    }
    result_value(
        &request,
        &durable,
        source_binding.as_ref(),
        &proof,
        &representation_plan_sha256,
        &geometry_idempotency_key,
        geometry_result,
        replayed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build_cohort_sha256, canonical_json_hash};
    use forgecad_contracts::AUTHORING_MESH_V2_DURABLE_PREPARE_REQUEST_SCHEMA_VERSION;
    use serde_json::json;

    const MESH: &str = "materializer-mesh";
    const LINEAGE: &str = "materializer-lineage";

    fn durable_genesis_request(
        project_id: &str,
        idempotency_key: &str,
        positions: Value,
        faces: Value,
    ) -> Value {
        let mut request = json!({
            "schema_version": AUTHORING_MESH_V2_DURABLE_PREPARE_REQUEST_SCHEMA_VERSION,
            "project_id": project_id,
            "operation": "genesis",
            "mesh_id": MESH,
            "lineage_id": LINEAGE,
            "parent_revision_id": null,
            "operation_id": null,
            "edge_id": null,
            "split_ratio_milli": null,
            "vertex_ids": null,
            "delta_m": null,
            "operation_lineage_sha256": null,
            "positions_m": positions,
            "faces": faces,
            "evaluated": null,
            "idempotency_key": idempotency_key,
            "max_response_bytes": 1048576,
            "runtime_write_performed": false,
            "writer_policy": "forgecad-runtime-only-state-writer@1",
            "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn materializer_request(
        project_id: &str,
        revision: &Value,
        idempotency_key: &str,
        source_binding: (Value, Value, Value),
    ) -> Value {
        let mut request = json!({
            "schema_version": PREPARE_SCHEMA_VERSION,
            "operation": OPERATION,
            "project_id": project_id,
            "mesh_id": MESH,
            "lineage_id": LINEAGE,
            "revision_id": revision["revision_id"],
            "revision_index": revision["revision_index"],
            "revision_sha256": revision["revision_sha256"],
            "revision_object_sha256": revision["revision_object_sha256"],
            "source_binding_id": source_binding.0,
            "source_binding_sha256": source_binding.1,
            "source_binding_object_sha256": source_binding.2,
            "base_version_id": null,
            "idempotency_key": idempotency_key,
            "max_response_bytes": 1048576,
            "runtime_write_performed": false,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": CANONICALIZATION_POLICY,
            "input_sha256": ""
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    fn setup(positions: Value, faces: Value) -> (crate::Runtime, Value, String) {
        let runtime = crate::Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("AuthoringMesh materializer", json!({"profile":"knife"}))
            .expect("project");
        let durable = runtime
            .authoring_mesh_v2_durable_prepare(&durable_genesis_request(
                &project.project_id,
                "durable-genesis",
                positions,
                faces,
            ))
            .expect("durable genesis");
        (runtime, durable, project.project_id)
    }

    #[test]
    fn source_bound_program_replaces_only_selected_part_and_preserves_other_parts() {
        let (runtime, revision_value, project_id) = setup(
            json!([
                [-0.5, -0.5, -0.5],
                [0.5, -0.5, -0.5],
                [0.5, 0.5, -0.5],
                [-0.5, 0.5, -0.5],
                [-0.5, -0.5, 0.5],
                [0.5, -0.5, 0.5],
                [0.5, 0.5, 0.5],
                [-0.5, 0.5, 0.5]
            ]),
            json!([
                [0, 3, 2, 1],
                [4, 5, 6, 7],
                [0, 1, 5, 4],
                [3, 7, 6, 2],
                [0, 4, 7, 3],
                [1, 2, 6, 5]
            ]),
        );
        let request_value = materializer_request(
            &project_id,
            &revision_value,
            "materializer-source-splice",
            (Value::Null, Value::Null, Value::Null),
        );
        let request = parse_request(&request_value).expect("closed request");
        let (_, revision) = load_revision(&runtime, &request).expect("durable revision");
        let source_program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project_id,
            "representation_plan_sha256":"a".repeat(64),
            "operator_catalog_sha256":crate::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":4,"max_triangles":250000,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[
                {"node_id":"guard-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"primitive":"box","size_m":[0.2,0.2,0.2],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"guard-detail-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"primitive":"box","size_m":[0.1,0.1,0.1],"position_m":[0.2,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"blade-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"primitive":"box","size_m":[1.0,0.1,0.1],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"blade-detail-node","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"primitive":"box","size_m":[0.2,0.05,0.05],"position_m":[0.4,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}}
            ],
            "part_outputs":[
                {"part_id":"guard","input_node_ids":["guard-node","guard-detail-node"],"material_zone_id":"guard-metal","solid":true},
                {"part_id":"blade","input_node_ids":["blade-node","blade-detail-node"],"material_zone_id":"blade-metal","solid":true}
            ]
        });
        let source = SourceMaterialization {
            source_program: source_program.clone(),
            source_reference_id: "source-reference".to_owned(),
            source_reference_sha256: "9".repeat(64),
            source_candidate_id: "source-candidate".to_owned(),
            source_candidate_state_sha256: "b".repeat(64),
            source_artifact_sha256: "c".repeat(64),
            source_artifact_readback_sha256: "d".repeat(64),
            source_program_sha256: canonical_json_hash(&source_program),
            source_program_object_sha256: canonical_json_hash(&source_program),
            source_binding_id: "source-binding".to_owned(),
            source_binding_sha256: "f".repeat(64),
            source_binding_object_sha256: "0".repeat(64),
            source_node_id: "blade-node".to_owned(),
            source_part_id: "blade".to_owned(),
            source_material_zone_id: "blade-metal".to_owned(),
            source_solid: true,
            source_position_m: [0.0, 0.0, 0.0],
            source_rotation_rad: [0.0, 0.0, 0.0],
            source_part_output_sha256: canonical_json_hash(&source_program["part_outputs"][1]),
            source_part_bindings: vec![
                json!({"part_id":"guard","source_node_id":"guard-node","material_zone_id":"guard-metal","solid":true,"triangle_count":12}),
                json!({"part_id":"guard","source_node_id":"guard-detail-node","material_zone_id":"guard-metal","solid":true,"triangle_count":12}),
                json!({"part_id":"blade","source_node_id":"blade-node","material_zone_id":"blade-metal","solid":true,"triangle_count":12}),
                json!({"part_id":"blade","source_node_id":"blade-detail-node","material_zone_id":"blade-metal","solid":true,"triangle_count":12}),
            ],
            preserved_part_ids: vec!["guard".to_owned()],
        };
        let (program, plan_sha256, proof) =
            build_geometry_program(&request, &revision, Some(&source)).expect("source splice");
        assert_eq!(program["representation_plan_sha256"], plan_sha256);
        assert!(program
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .is_some());
        let nodes = program["nodes"].as_array().expect("nodes");
        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes[0], source_program["nodes"][0]);
        assert_eq!(nodes[1], source_program["nodes"][1]);
        assert_eq!(nodes[2]["node_id"], proof.replacement_node_id);
        assert_eq!(nodes[2]["operator_id"], OPERATOR_ID);
        assert_eq!(nodes[3], source_program["nodes"][3]);
        assert!(nodes.iter().all(|node| node["node_id"] != "blade-node"));
        let outputs = program["part_outputs"].as_array().expect("outputs");
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0], source_program["part_outputs"][0]);
        assert_eq!(outputs[1]["part_id"], "blade");
        assert_eq!(outputs[1]["material_zone_id"], "blade-metal");
        assert_eq!(outputs[1]["solid"], true);
        assert_eq!(
            outputs[1]["input_node_ids"],
            json!([proof.replacement_node_id, "blade-detail-node"])
        );

        let mut tampered_source = source.clone();
        tampered_source.source_program["part_outputs"][0]["solid"] = json!(false);
        let error = build_geometry_program(&request, &revision, Some(&tampered_source))
            .expect_err("source program drift must fail before Worker execution");
        assert!(error
            .to_string()
            .contains("semantic/object hash drifted before Worker execution"));
        let mut tampered_output = source.clone();
        tampered_output.source_part_output_sha256 = "1".repeat(64);
        let error = build_geometry_program(&request, &revision, Some(&tampered_output))
            .expect_err("source Part output drift must fail before Worker execution");
        assert!(error
            .to_string()
            .contains("Part output semantic/hash drifted before Worker execution"));

        let derived = json!({
            "artifact":{"part_bindings":[
                {"part_id":"guard","source_node_id":"guard-node","material_zone_id":"guard-metal","solid":true,"triangle_count":12},
                {"part_id":"guard","source_node_id":"guard-detail-node","material_zone_id":"guard-metal","solid":true,"triangle_count":12},
                {"part_id":"blade","source_node_id":proof.replacement_node_id,"material_zone_id":"blade-metal","solid":true,"triangle_count":24},
                {"part_id":"blade","source_node_id":"blade-detail-node","material_zone_id":"blade-metal","solid":true,"triangle_count":12}
            ]}
        });
        validate_materialized_artifact(&proof, &derived).expect("part preservation");
        let changed_untouched = json!({
            "artifact":{"part_bindings":[
                {"part_id":"guard","source_node_id":"changed-guard","material_zone_id":"guard-metal","solid":true,"triangle_count":12},
                {"part_id":"guard","source_node_id":"guard-detail-node","material_zone_id":"guard-metal","solid":true,"triangle_count":12},
                {"part_id":"blade","source_node_id":proof.replacement_node_id,"material_zone_id":"blade-metal","solid":true,"triangle_count":24},
                {"part_id":"blade","source_node_id":"blade-detail-node","material_zone_id":"blade-metal","solid":true,"triangle_count":12}
            ]}
        });
        assert!(validate_materialized_artifact(&proof, &changed_untouched).is_err());
        let changed_same_part_non_target = json!({
            "artifact":{"part_bindings":[
                {"part_id":"guard","source_node_id":"guard-node","material_zone_id":"guard-metal","solid":true,"triangle_count":12},
                {"part_id":"guard","source_node_id":"guard-detail-node","material_zone_id":"guard-metal","solid":true,"triangle_count":12},
                {"part_id":"blade","source_node_id":proof.replacement_node_id,"material_zone_id":"blade-metal","solid":true,"triangle_count":24},
                {"part_id":"blade","source_node_id":"blade-detail-node","material_zone_id":"blade-metal","solid":true,"triangle_count":13}
            ]}
        });
        assert!(validate_materialized_artifact(&proof, &changed_same_part_non_target).is_err());
    }

    #[test]
    fn materializer_rejects_closed_source_binding_selector_drift_before_candidate_write() {
        let (runtime, revision, project_id) = setup(
            json!([
                [-0.5, -0.5, -0.5],
                [0.5, -0.5, -0.5],
                [0.5, 0.5, -0.5],
                [-0.5, 0.5, -0.5],
                [-0.5, -0.5, 0.5],
                [0.5, -0.5, 0.5],
                [0.5, 0.5, 0.5],
                [-0.5, 0.5, 0.5]
            ]),
            json!([
                [0, 3, 2, 1],
                [4, 5, 6, 7],
                [0, 1, 5, 4],
                [3, 7, 6, 2],
                [0, 4, 7, 3],
                [1, 2, 6, 5]
            ]),
        );
        let request = materializer_request(
            &project_id,
            &revision,
            "materializer-source-drift",
            (
                Value::String("source-binding-missing".into()),
                Value::String("a".repeat(64)),
                Value::String("b".repeat(64)),
            ),
        );
        let error = runtime
            .authoring_mesh_v2_candidate_materialize(&request)
            .expect_err("missing source binding must fail closed");
        assert!(error
            .to_string()
            .contains("exact source binding is not durable"));
        assert!(runtime
            .candidates(&project_id)
            .expect("candidates")
            .is_empty());
    }

    #[test]
    fn materializer_rejects_revision_hash_drift_before_candidate_write() {
        let (runtime, mut revision, project_id) = setup(
            json!([
                [-0.5, -0.5, -0.5],
                [0.5, -0.5, -0.5],
                [0.5, 0.5, -0.5],
                [-0.5, 0.5, -0.5],
                [-0.5, -0.5, 0.5],
                [0.5, -0.5, 0.5],
                [0.5, 0.5, 0.5],
                [-0.5, 0.5, 0.5]
            ]),
            json!([
                [0, 3, 2, 1],
                [4, 5, 6, 7],
                [0, 1, 5, 4],
                [3, 7, 6, 2],
                [0, 4, 7, 3],
                [1, 2, 6, 5]
            ]),
        );
        revision["revision_sha256"] = Value::String("a".repeat(64));
        let request = materializer_request(
            &project_id,
            &revision,
            "materializer-revision-drift",
            (Value::Null, Value::Null, Value::Null),
        );
        let error = runtime
            .authoring_mesh_v2_candidate_materialize(&request)
            .expect_err("revision hash drift must fail closed");
        assert!(error
            .to_string()
            .contains("durable revision identity differs from request"));
        assert!(runtime
            .candidates(&project_id)
            .expect("candidates")
            .is_empty());
    }

    #[test]
    fn materializer_rejects_n_gon_without_candidate_or_worker_fallback() {
        let (runtime, revision, project_id) = setup(
            json!([
                [1.0, 0.0, -0.5],
                [0.309016994, 0.951056516, -0.5],
                [-0.809016994, 0.587785252, -0.5],
                [-0.809016994, -0.587785252, -0.5],
                [0.309016994, -0.951056516, -0.5],
                [1.0, 0.0, 0.5],
                [0.309016994, 0.951056516, 0.5],
                [-0.809016994, 0.587785252, 0.5],
                [-0.809016994, -0.587785252, 0.5],
                [0.309016994, -0.951056516, 0.5]
            ]),
            json!([
                [0, 4, 3, 2, 1],
                [5, 6, 7, 8, 9],
                [0, 1, 6, 5],
                [1, 2, 7, 6],
                [2, 3, 8, 7],
                [3, 4, 9, 8],
                [4, 0, 5, 9]
            ]),
        );
        let request = materializer_request(
            &project_id,
            &revision,
            "materializer-ngon",
            (Value::Null, Value::Null, Value::Null),
        );
        let error = runtime
            .authoring_mesh_v2_candidate_materialize(&request)
            .expect_err("n-gon must fail closed");
        assert!(error.to_string().contains("Worker-compatible"));
        assert!(runtime
            .candidates(&project_id)
            .expect("candidates")
            .is_empty());
    }

    #[test]
    fn materializer_exact_prepare_replay_is_single_candidate_when_worker_cohort_is_available() {
        if build_cohort_sha256().is_none() {
            eprintln!("materializer live test requires FORGECAD_BUILD_COHORT_SHA256");
            return;
        }
        let (runtime, revision, project_id) = setup(
            json!([
                [-0.5, -0.5, -0.5],
                [0.5, -0.5, -0.5],
                [0.5, 0.5, -0.5],
                [-0.5, 0.5, -0.5],
                [-0.5, -0.5, 0.5],
                [0.5, -0.5, 0.5],
                [0.5, 0.5, 0.5],
                [-0.5, 0.5, 0.5]
            ]),
            json!([
                [0, 3, 2, 1],
                [4, 5, 6, 7],
                [0, 1, 5, 4],
                [3, 7, 6, 2],
                [0, 4, 7, 3],
                [1, 2, 6, 5]
            ]),
        );
        let request = materializer_request(
            &project_id,
            &revision,
            "materializer-replay",
            (Value::Null, Value::Null, Value::Null),
        );
        let first = runtime
            .authoring_mesh_v2_candidate_materialize(&request)
            .expect("first materialization");
        assert_eq!(first["operation"], OPERATION);
        assert_eq!(first["request_kind"], "prepare");
        assert_eq!(first["status"], "prepared");
        assert_eq!(first["candidate"]["state"], "reviewable");
        assert_eq!(first["runtime_write_performed"], true);
        let replay = runtime
            .authoring_mesh_v2_candidate_materialize(&request)
            .expect("replay materialization");
        assert_eq!(replay["operation"], OPERATION);
        assert_eq!(replay["request_kind"], "prepare");
        assert_eq!(replay["status"], "replayed");
        assert_eq!(replay["replayed"], true);
        assert_eq!(replay["runtime_write_performed"], false);
        assert_eq!(
            runtime.candidates(&project_id).expect("candidates").len(),
            1
        );
    }
}
