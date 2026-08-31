//! Real-candidate bridge from one product-owned GeometryProgram source node
//! to an immutable `AuthoringMeshRevision@2` genesis.
//!
//! The caller supplies only exact durable identities/hashes. Runtime loads the
//! candidate-owned program, derives the editable local topology, generates a
//! deterministic lineage, and delegates persistence to the existing V2
//! durable writer. No caller GeometryProgram or mesh buffer is accepted.

use super::{
    authoring_mesh_v2::{
        AuthoringMeshV2EvaluatedBinding, AuthoringMeshV2GenesisInput, AuthoringMeshV2Revision,
    },
    authoring_mesh_v2_durable::persist_runtime_derived_source_genesis,
    authoring_mesh_v2_geometry::{
        authoring_mesh_source_genesis, primitive_box_source_genesis,
        profile_extrude_source_genesis, AuthoringMeshV2SourceGenesis,
    },
    canonical_json_hash, is_opaque_id, is_sha256, Runtime, RuntimeError,
};
use forgecad_contracts::{AuthoringMeshId, AuthoringMeshLineageId, AuthoringMeshV2SourceBinding};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const MAX_JSON_BYTES: u64 = 1024 * 1024;
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const WRITER_POLICY: &str = "forgecad-runtime-only-state-writer@1";
const CANONICALIZATION_POLICY: &str = "canonical-json-sha256-excluding-canonical-sha256@1";

const REQUEST_FIELDS: &[&str] = &[
    "schema_version",
    "project_id",
    "candidate_id",
    "candidate_state_sha256",
    "geometry_program_sha256",
    "artifact_sha256",
    "artifact_readback_sha256",
    "part_id",
    "source_node_id",
    "idempotency_key",
    "max_response_bytes",
    "runtime_write_performed",
    "writer_policy",
    "canonicalization_policy",
    "input_sha256",
];

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!(
        "PRODUCTION_WEAPON_AUTHORING_MESH_V2_SOURCE_INVALID: {}",
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
        return Err(invalid("request fields differ from the closed contract"));
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
        return Err(invalid(format!("{field} must be an opaque ID")));
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

fn source_genesis(
    node: &Value,
    expected_node_id: &str,
) -> Result<AuthoringMeshV2SourceGenesis, RuntimeError> {
    match node.get("operator_id").and_then(Value::as_str) {
        Some("forgecad.geometry.primitive@2") => {
            primitive_box_source_genesis(node, expected_node_id)
        }
        Some("forgecad.geometry.profile-extrude@1") => {
            profile_extrude_source_genesis(node, expected_node_id)
        }
        Some("forgecad.geometry.authoring-mesh@1") => {
            authoring_mesh_source_genesis(node, expected_node_id)
        }
        Some(operator_id) => Err(invalid(format!(
            "source operator {operator_id} is not enabled for AuthoringMeshV2 genesis"
        ))),
        None => Err(invalid("source operator is missing")),
    }
}

pub(crate) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(request)?;
    if text(object, "schema_version")? != "ProductionWeaponAuthoringMeshV2SourcePrepareRequest@1"
        || text(object, "writer_policy")? != WRITER_POLICY
        || text(object, "canonicalization_policy")? != CANONICALIZATION_POLICY
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("max_response_bytes").and_then(Value::as_u64) != Some(MAX_RESPONSE_BYTES)
    {
        return Err(invalid("request policy or response budget differs"));
    }
    let mut preimage = request.clone();
    preimage["input_sha256"] = Value::String(String::new());
    let request_input_sha256 = sha(object, "input_sha256")?;
    if canonical_json_hash(&preimage) != request_input_sha256 {
        return Err(invalid("input_sha256 differs from the closed request"));
    }

    let project_id = id(object, "project_id")?;
    let candidate_id = id(object, "candidate_id")?;
    let part_id = id(object, "part_id")?;
    let source_node_id = id(object, "source_node_id")?;
    let candidate_state_sha256 = sha(object, "candidate_state_sha256")?;
    let geometry_program_sha256 = sha(object, "geometry_program_sha256")?;
    let artifact_sha256 = sha(object, "artifact_sha256")?;
    let artifact_readback_sha256 = sha(object, "artifact_readback_sha256")?;
    let idempotency_key = id(object, "idempotency_key")?;

    let candidate = runtime
        .candidate(candidate_id)?
        .ok_or_else(|| invalid("candidate is unavailable"))?;
    if candidate.project_id != project_id
        || candidate.canonical_sha256 != candidate_state_sha256
        || candidate.prepared_object_sha256.as_deref() != Some(artifact_sha256)
    {
        return Err(invalid("candidate binding differs"));
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .ok_or_else(|| invalid("candidate geometry evidence is unavailable"))?;
    if evidence.project_id != project_id
        || evidence.geometry_program_sha256 != geometry_program_sha256
        || evidence.artifact_object_sha256 != artifact_sha256
    {
        return Err(invalid("geometry evidence binding differs"));
    }
    let program_bytes =
        runtime.cas_read_bounded(&evidence.geometry_program_object_sha256, MAX_JSON_BYTES)?;
    let program: Value = serde_json::from_slice(&program_bytes)
        .map_err(|error| invalid(format!("GeometryProgram CAS JSON is invalid: {error}")))?;
    if program
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .is_some_and(|value| value != geometry_program_sha256)
        || evidence.geometry_program_object_sha256 != geometry_program_sha256
        || program.get("project_id").and_then(Value::as_str) != Some(project_id)
    {
        return Err(invalid("GeometryProgram canonical/project binding differs"));
    }
    let readback_bytes =
        runtime.cas_read_bounded(&evidence.artifact_readback_object_sha256, MAX_JSON_BYTES)?;
    let readback: Value = serde_json::from_slice(&readback_bytes)
        .map_err(|error| invalid(format!("ArtifactReadback CAS JSON is invalid: {error}")))?;
    if readback.get("canonical_sha256").and_then(Value::as_str) != Some(artifact_readback_sha256)
        || readback.get("program_sha256").and_then(Value::as_str) != Some(geometry_program_sha256)
    {
        return Err(invalid("ArtifactReadback binding differs"));
    }

    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("GeometryProgram nodes are unavailable"))?;
    let matching_nodes = nodes
        .iter()
        .filter(|node| node.get("node_id").and_then(Value::as_str) == Some(source_node_id))
        .collect::<Vec<_>>();
    if matching_nodes.len() != 1 {
        return Err(invalid("source node is absent or ambiguous"));
    }
    let outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("GeometryProgram PartOutputs are unavailable"))?;
    let matching_parts = outputs
        .iter()
        .filter(|output| output.get("part_id").and_then(Value::as_str) == Some(part_id))
        .collect::<Vec<_>>();
    if matching_parts.len() != 1
        || !matching_parts[0]
            .get("input_node_ids")
            .and_then(Value::as_array)
            .is_some_and(|ids| {
                ids.iter()
                    .filter(|value| value.as_str() == Some(source_node_id))
                    .count()
                    == 1
            })
        || outputs
            .iter()
            .filter(|output| {
                output
                    .get("input_node_ids")
                    .and_then(Value::as_array)
                    .is_some_and(|ids| {
                        ids.iter()
                            .any(|value| value.as_str() == Some(source_node_id))
                    })
            })
            .count()
            != 1
    {
        return Err(invalid(
            "source node is not uniquely owned by the requested Part",
        ));
    }
    let material_zone_id = matching_parts[0]
        .get("material_zone_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("Part material zone is invalid"))?;
    let solid = matching_parts[0]
        .get("solid")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("Part solid flag is invalid"))?;

    let source = source_genesis(matching_nodes[0], source_node_id)?;
    let artifact_id = candidate
        .prepared_object_id
        .clone()
        .unwrap_or_else(|| artifact_sha256.to_owned());
    let part_output_sha256 = canonical_json_hash(matching_parts[0]);
    let stable_identity_sha256 = canonical_json_hash(&json!({
        "schema_version":"AuthoringMeshV2StableSourceIdentity@1",
        "project_id":project_id,
        "part_id":part_id,
        "source_node_id":source_node_id,
        "source_operator_id":source.source_operator_id,
        "material_zone_id":material_zone_id,
        "identity_policy":"project-part-source-node-material-operator-stable@1"
    }));
    let mesh_id = format!("amv2-mesh-{}", &stable_identity_sha256[..48]);
    let lineage_id = format!("amv2-lineage-{}", &stable_identity_sha256[..48]);
    let mut source_binding = AuthoringMeshV2SourceBinding {
        schema_version: "AuthoringMeshV2SourceBinding@1".to_owned(),
        project_id: project_id.to_owned(),
        candidate_id: candidate_id.to_owned(),
        candidate_state_sha256: candidate_state_sha256.to_owned(),
        artifact_id: artifact_id.clone(),
        artifact_sha256: artifact_sha256.to_owned(),
        artifact_readback_sha256: artifact_readback_sha256.to_owned(),
        geometry_program_sha256: geometry_program_sha256.to_owned(),
        source_node_id: source_node_id.to_owned(),
        part_id: part_id.to_owned(),
        material_zone_id: material_zone_id.to_owned(),
        solid,
        source_operator_id: source.source_operator_id.clone(),
        source_parameters_sha256: source.source_parameters_sha256.clone(),
        part_output_sha256,
        position_m: source.position_m,
        rotation_rad: source.rotation_rad,
        canonical_sha256: String::new(),
    };
    source_binding.canonical_sha256 = canonical_json_hash(
        &serde_json::to_value(&source_binding)
            .map_err(|error| invalid(format!("source binding serialization failed: {error}")))?,
    );
    let source_binding_sha256 = source_binding.canonical_sha256.clone();
    let inner_idempotency_key = format!(
        "amv2-source-{}-{}",
        &source_binding_sha256[..20],
        &canonical_json_hash(&json!({"idempotency_key":idempotency_key}))[..20]
    );
    let revision = AuthoringMeshV2Revision::genesis(AuthoringMeshV2GenesisInput {
        mesh_id: AuthoringMeshId(mesh_id.clone()),
        lineage_id: AuthoringMeshLineageId(lineage_id.clone()),
        positions_m: source.positions_m.clone(),
        faces: source.faces.clone(),
        evaluated: Some(AuthoringMeshV2EvaluatedBinding {
            artifact_id,
            artifact_sha256: artifact_sha256.to_owned(),
            readback_sha256: artifact_readback_sha256.to_owned(),
            correspondence_status: format!("source-bound-{}@1", source_binding_sha256),
        }),
        source_binding: Some(source_binding),
        foundation_source_binding: None,
    })?;
    let durable = persist_runtime_derived_source_genesis(
        runtime,
        project_id,
        request_input_sha256,
        &inner_idempotency_key,
        revision.record().clone(),
    )?;
    if durable.get("mesh_id").and_then(Value::as_str) != Some(mesh_id.as_str())
        || durable.get("lineage_id").and_then(Value::as_str) != Some(lineage_id.as_str())
    {
        return Err(invalid(
            "durable AuthoringMesh identity differs from source binding",
        ));
    }
    let mut result = json!({
        "schema_version":"ProductionWeaponAuthoringMeshV2SourcePrepareResult@1",
        "project_id":project_id,
        "candidate_id":candidate_id,
        "candidate_state_sha256":candidate_state_sha256,
        "geometry_program_sha256":geometry_program_sha256,
        "artifact_sha256":artifact_sha256,
        "artifact_readback_sha256":artifact_readback_sha256,
        "part_id":part_id,
        "source_node_id":source.source_node_id,
        "source_operator_id":source.source_operator_id,
        "source_parameters_sha256":source.source_parameters_sha256,
        "source_position_m":source.position_m,
        "source_rotation_rad":source.rotation_rad,
        "material_zone_id":material_zone_id,
        "solid":solid,
        "source_binding_sha256":source_binding_sha256,
        "mesh_id":mesh_id,
        "lineage_id":lineage_id,
        "revision_id":durable["revision_id"],
        "revision_sha256":durable["revision_sha256"],
        "revision_object_sha256":durable["revision_object_sha256"],
        "authoring_mesh_v2":durable,
        "request_input_sha256":request_input_sha256,
        "idempotency_key":idempotency_key,
        "runtime_write_performed":true,
        "persistent_user_data_touched":true,
        "stage_advanced":false,
        "candidate_confirmed":false,
        "version_created":false,
        "export_performed":false,
        "quality_status":"structural_source_bound_not_visually_evaluated",
        "limitations":[
            "REAL_CANDIDATE_SOURCE_BOUND",
            "PROFILE_EXTRUDE_OR_PRIMITIVE_SOURCE",
            "NO_ART_EDIT_APPLIED",
            "NO_STAGE_ADVANCEMENT",
            "NO_VISUAL_QUALITY_CLAIM"
        ],
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_dispatch_accepts_profile_extrude_and_rejects_unregistered_operator() {
        let node = json!({
            "node_id":"dragonfang-blade-body",
            "operator_id":"forgecad.geometry.profile-extrude@1",
            "inputs":[],
            "parameters":{
                "shape":"profile-extrude",
                "profile":[[-1.0,-0.25],[1.0,-0.25],[0.4,0.4],[-0.2,0.2],[-1.0,0.4]],
                "depth_m":0.12,
                "position_m":[0.0,0.0,0.0],
                "rotation_rad":[0.0,0.0,0.0]
            }
        });
        let source = source_genesis(&node, "dragonfang-blade-body").expect("profile source");
        assert_eq!(source.source_node_id, "dragonfang-blade-body");
        assert_eq!(
            source.source_operator_id,
            "forgecad.geometry.profile-extrude@1"
        );
        assert_eq!(source.positions_m.len(), 10);

        let mut unsupported = node;
        unsupported["operator_id"] = json!("forgecad.geometry.boolean@1");
        let error = source_genesis(&unsupported, "dragonfang-blade-body")
            .expect_err("unregistered source operator must fail closed");
        assert!(error.to_string().contains("not enabled"));
    }
}
