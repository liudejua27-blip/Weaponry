use super::{
    artifact_readback_v2_value, canonical_json_bytes, canonical_json_hash,
    compile_geometry_with_runtime_worker, geometry_candidate_evidence_value,
    hash_geometry_program_with_runtime_worker, is_opaque_id, is_sha256, now_string, sha256_hex,
    strict_glb_inspection, strict_integrity_value, validate_artifact_readback_v2_output,
    validate_geometry_candidate_evidence_output, validate_geometry_quality_report_v2_output,
    validate_worker_metadata, Runtime, RuntimeError,
};
use forgecad_contracts::{
    AuditEventRecord, CandidateRecord, GeometryCandidateEvidenceRecord, JobEventRecord, JobRecord,
    JobSummary,
};
use forgecad_store::CasObject;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

const MAX_GLB_BYTES: u64 = 64 * 1024 * 1024;
const MAX_JSON_BYTES: u64 = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const AUTHORING_TOPOLOGY_POLICY: &str = "forgecad-authoring-topology@1:source-program-v-e-loop-face:single-direct-part-output:elements-1536:faces-512:response-1mib:no-write";
const AUTHORING_TOPOLOGY_POLICY_SHA256: &str =
    "a6fb36a530e49537673b66d65ecb6e4fb4f51ffb3e7d01a0980be71f28cb367d";
const EDIT_POLICY: &str = "forgecad-authoring-mesh-edit-preview@1:translate-vertices-or-single-face-extrude:source-program-bound:worker-double-replay:glb-64mib:response-1mib:no-write";
const EDIT_POLICY_SHA256: &str = "1d050226b13848902f44bddb1b88c240cdfa86759703f804443b03964f8ddaae";

struct AuthoringContext {
    project_id: String,
    candidate_id: String,
    artifact_id: String,
    artifact_readback_sha256: String,
    geometry_candidate_evidence_sha256: String,
    reference_id: Option<String>,
    reference_sha256: Option<String>,
    geometry_program_object_sha256: String,
    program_sha256: String,
    operator_catalog_sha256: String,
    readback_config_sha256: String,
    authoring_node_id: String,
    part_id: String,
    material_zone_id: String,
    solid: bool,
    program: Value,
    node_index: usize,
    parameters: Map<String, Value>,
    source_artifact_bytes: Vec<u8>,
    source_triangle_count: u64,
    source_part_ids: Vec<String>,
    source_material_zone_ids: Vec<String>,
    source_worker_cohort_sha256: Option<String>,
}

fn invalid(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("AUTHORING_TOPOLOGY_INVALID: {}", message.into()))
}

fn exact_object<'a>(
    value: &'a Value,
    keys: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid(format!("{context} must be an object")))?;
    let expected = keys.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid(format!("{context} fields differ")));
    }
    Ok(object)
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("{key} is required")))
}

fn identifier<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    text(object, key).and_then(|value| {
        is_opaque_id(value)
            .then_some(value)
            .ok_or_else(|| invalid(format!("{key} is not an identifier")))
    })
}

fn sha<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, RuntimeError> {
    text(object, key).and_then(|value| {
        is_sha256(value)
            .then_some(value)
            .ok_or_else(|| invalid(format!("{key} is not a SHA-256")))
    })
}

fn value_array<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a Vec<Value>, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invalid(format!("{key} must be an array")))
}

fn validate_durable_geometry_evidence(
    evidence: &GeometryCandidateEvidenceRecord,
) -> Result<(), RuntimeError> {
    let value = serde_json::to_value(evidence)
        .map_err(|error| invalid(format!("durable geometry evidence is invalid: {error}")))?;
    validate_geometry_candidate_evidence_output(&value)
        .map_err(|_| invalid("durable geometry evidence canonical binding differs"))
}

fn load_context(runtime: &Runtime, request: &Value) -> Result<AuthoringContext, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "artifact_id",
            "artifact_readback_sha256",
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "authoring_node_id",
            "part_id",
            "authoring_topology_policy_sha256",
            "max_response_bytes",
        ],
        "AuthoringTopologyRequest@1",
    )?;
    if text(object, "schema_version")? != "AuthoringTopologyRequest@1"
        || sha(object, "authoring_topology_policy_sha256")? != AUTHORING_TOPOLOGY_POLICY_SHA256
        || sha256_hex(AUTHORING_TOPOLOGY_POLICY.as_bytes()) != AUTHORING_TOPOLOGY_POLICY_SHA256
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
    {
        return Err(invalid("request policy or response budget differs"));
    }
    let project_id = identifier(object, "project_id")?.to_owned();
    let candidate_id = identifier(object, "candidate_id")?.to_owned();
    let artifact_id = sha(object, "artifact_id")?.to_owned();
    let artifact_readback_sha256 = sha(object, "artifact_readback_sha256")?.to_owned();
    let program_sha256 = sha(object, "program_sha256")?.to_owned();
    let operator_catalog_sha256 = sha(object, "operator_catalog_sha256")?.to_owned();
    let readback_config_sha256 = sha(object, "readback_config_sha256")?.to_owned();
    let authoring_node_id = identifier(object, "authoring_node_id")?.to_owned();
    let part_id = identifier(object, "part_id")?.to_owned();

    let candidate = runtime
        .candidate(&candidate_id)?
        .ok_or_else(|| invalid("candidate is unavailable"))?;
    if candidate.project_id != project_id
        || candidate.prepared_object_sha256.as_deref() != Some(artifact_id.as_str())
    {
        return Err(invalid("candidate project or artifact binding differs"));
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&candidate_id)?
        .ok_or_else(|| invalid("durable V2 geometry evidence is unavailable"))?;
    validate_durable_geometry_evidence(&evidence)?;
    if evidence.project_id != project_id
        || evidence.artifact_object_sha256 != artifact_id
        || evidence.geometry_program_sha256 != program_sha256
        || evidence.operator_catalog_sha256 != operator_catalog_sha256
        || evidence.readback_config_sha256 != readback_config_sha256
    {
        return Err(invalid("durable geometry evidence binding differs"));
    }
    let artifact_record = runtime
        .store
        .get_object(&artifact_id)?
        .ok_or_else(|| invalid("candidate GLB CAS metadata is unavailable"))?;
    if artifact_record.mime != "model/gltf-binary"
        || !matches!(
            artifact_record.kind.as_str(),
            "geometry-glb" | "appearance-glb"
        )
        || artifact_record.size_bytes == 0
        || artifact_record.size_bytes > MAX_GLB_BYTES
    {
        return Err(invalid("candidate GLB metadata or 64 MiB budget differs"));
    }
    let source_artifact_bytes = runtime.cas_read_bounded(&artifact_id, MAX_GLB_BYTES)?;
    let inspection = strict_glb_inspection(&source_artifact_bytes)?;
    runtime.revalidate_v2_geometry_evidence(&candidate, &inspection, &evidence)?;
    let readback = runtime.artifact_readback_bounded(&artifact_id, &candidate_id, MAX_GLB_BYTES)?;
    if readback.get("canonical_sha256").and_then(Value::as_str)
        != Some(artifact_readback_sha256.as_str())
        || readback.get("program_sha256").and_then(Value::as_str) != Some(program_sha256.as_str())
        || readback
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(operator_catalog_sha256.as_str())
        || readback
            .get("readback_config_sha256")
            .and_then(Value::as_str)
            != Some(readback_config_sha256.as_str())
    {
        return Err(invalid("ArtifactReadback@2 binding differs"));
    }

    let program_record = runtime
        .store
        .get_object(&evidence.geometry_program_object_sha256)?
        .ok_or_else(|| invalid("GeometryProgram CAS metadata is unavailable"))?;
    if program_record.mime != "application/json"
        || program_record.kind != "geometry-program-v2"
        || program_record.size_bytes == 0
        || program_record.size_bytes > MAX_JSON_BYTES
    {
        return Err(invalid("GeometryProgram metadata or 1 MiB budget differs"));
    }
    let program_bytes =
        runtime.cas_read_bounded(&evidence.geometry_program_object_sha256, MAX_JSON_BYTES)?;
    let mut program: Value = serde_json::from_slice(&program_bytes)
        .map_err(|_| invalid("GeometryProgram is not JSON"))?;
    let program_object = program
        .as_object()
        .ok_or_else(|| invalid("GeometryProgram is not an object"))?;
    if program_object.contains_key("canonical_sha256")
        || program_object.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2")
        || program_object.get("project_id").and_then(Value::as_str) != Some(project_id.as_str())
    {
        return Err(invalid("persisted GeometryProgram draft shape differs"));
    }
    let hash = hash_geometry_program_with_runtime_worker(&program).map_err(|error| {
        invalid(format!(
            "persisted GeometryProgram validation failed: {error}"
        ))
    })?;
    if hash.get("canonical_sha256").and_then(Value::as_str) != Some(program_sha256.as_str())
        || hash.get("operator_catalog_sha256").and_then(Value::as_str)
            != Some(operator_catalog_sha256.as_str())
    {
        return Err(invalid("persisted GeometryProgram hash or catalog differs"));
    }
    program
        .as_object_mut()
        .expect("validated GeometryProgram object")
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(program_sha256.clone()),
        );

    let first_replay = compile_geometry_with_runtime_worker(&program, None)
        .map_err(|error| invalid(format!("source Geometry Worker replay failed: {error}")))?;
    let repeat_replay = compile_geometry_with_runtime_worker(&program, None)
        .map_err(|error| invalid(format!("source Geometry Worker repeat failed: {error}")))?;
    if first_replay.glb != source_artifact_bytes
        || repeat_replay.glb != source_artifact_bytes
        || first_replay.glb != repeat_replay.glb
        || first_replay.program_sha256 != program_sha256
        || repeat_replay.program_sha256 != program_sha256
        || first_replay.build_cohort_sha256 != repeat_replay.build_cohort_sha256
    {
        return Err(invalid(
            "source full-GLB replay differs from candidate artifact",
        ));
    }
    validate_worker_metadata(&first_replay, &inspection)?;

    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("GeometryProgram nodes are unavailable"))?;
    let matching_nodes = nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.get("node_id").and_then(Value::as_str) == Some(&authoring_node_id))
        .collect::<Vec<_>>();
    if matching_nodes.len() != 1 {
        return Err(invalid("authoring node is absent or ambiguous"));
    }
    let (node_index, node) = matching_nodes[0];
    if node.get("operator_id").and_then(Value::as_str) != Some("forgecad.geometry.authoring-mesh@1")
        || node
            .get("inputs")
            .and_then(Value::as_array)
            .is_none_or(|inputs| !inputs.is_empty())
    {
        return Err(invalid(
            "authoring node is not a direct source authoring-mesh@1",
        ));
    }
    if nodes.iter().any(|candidate_node| {
        candidate_node
            .get("inputs")
            .and_then(Value::as_array)
            .is_some_and(|inputs| {
                inputs
                    .iter()
                    .any(|input| input.as_str() == Some(authoring_node_id.as_str()))
            })
    }) {
        return Err(invalid(
            "authoring node has downstream consumers and is not an isolated direct Part source",
        ));
    }
    let parameters = node
        .get("parameters")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| invalid("authoring node parameters are unavailable"))?;
    if parameters.get("shape").and_then(Value::as_str) != Some("authoring-mesh")
        || parameters.get("topology_policy").and_then(Value::as_str)
            != Some("triangle-quad-manifold-with-boundary@1")
    {
        return Err(invalid("authoring topology policy differs"));
    }

    let part_outputs = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("GeometryProgram part outputs are unavailable"))?;
    let matching_parts = part_outputs
        .iter()
        .filter(|part| part.get("part_id").and_then(Value::as_str) == Some(&part_id))
        .collect::<Vec<_>>();
    if matching_parts.len() != 1 {
        return Err(invalid("Part output is absent or ambiguous"));
    }
    let part = matching_parts[0];
    let inputs = part
        .get("input_node_ids")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("Part output inputs are invalid"))?;
    if inputs.len() != 1 || inputs[0].as_str() != Some(authoring_node_id.as_str()) {
        return Err(invalid("Part output is not a single direct authoring node"));
    }
    if part_outputs
        .iter()
        .filter(|part| {
            part.get("input_node_ids")
                .and_then(Value::as_array)
                .is_some_and(|ids| {
                    ids.iter()
                        .any(|id| id.as_str() == Some(authoring_node_id.as_str()))
                })
        })
        .count()
        != 1
    {
        return Err(invalid("authoring node feeds more than one Part output"));
    }
    let material_zone_id = part
        .get("material_zone_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("Part material zone is invalid"))?
        .to_owned();
    let solid = part
        .get("solid")
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid("Part solid flag is invalid"))?;
    let bindings = readback
        .get("part_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("ArtifactReadback part bindings are unavailable"))?;
    let exact_binding = bindings
        .iter()
        .filter(|binding| {
            binding.get("part_id").and_then(Value::as_str) == Some(part_id.as_str())
                && binding.get("source_node_id").and_then(Value::as_str)
                    == Some(authoring_node_id.as_str())
                && binding.get("material_zone_id").and_then(Value::as_str)
                    == Some(material_zone_id.as_str())
                && binding.get("solid").and_then(Value::as_bool) == Some(solid)
        })
        .count();
    if exact_binding != 1 {
        return Err(invalid("ArtifactReadback direct Part binding differs"));
    }

    Ok(AuthoringContext {
        project_id,
        candidate_id,
        artifact_id,
        artifact_readback_sha256,
        geometry_candidate_evidence_sha256: evidence.canonical_sha256,
        reference_id: evidence.reference_id,
        reference_sha256: evidence.reference_sha256,
        geometry_program_object_sha256: evidence.geometry_program_object_sha256,
        program_sha256,
        operator_catalog_sha256,
        readback_config_sha256,
        authoring_node_id,
        part_id,
        material_zone_id,
        solid,
        program,
        node_index,
        parameters,
        source_artifact_bytes,
        source_triangle_count: first_replay.triangle_count,
        source_part_ids: first_replay.part_ids,
        source_material_zone_ids: first_replay.material_zone_ids,
        source_worker_cohort_sha256: first_replay.build_cohort_sha256,
    })
}

fn topology_counts(
    parameters: &Map<String, Value>,
    triangle_count: u64,
) -> Result<Value, RuntimeError> {
    Ok(json!({
        "vertex_count":value_array(parameters, "vertices")?.len(),
        "edge_count":value_array(parameters, "edges")?.len(),
        "loop_count":value_array(parameters, "loops")?.len(),
        "face_count":value_array(parameters, "faces")?.len(),
        "triangle_count":triangle_count,
    }))
}

fn topology_hash(parameters: &Map<String, Value>) -> String {
    canonical_json_hash(&json!({
        "topology_policy":parameters.get("topology_policy"),
        "vertices":parameters.get("vertices"),
        "edges":parameters.get("edges"),
        "loops":parameters.get("loops"),
        "faces":parameters.get("faces"),
        "position_m":parameters.get("position_m"),
        "rotation_rad":parameters.get("rotation_rad"),
    }))
}

fn topology_value(context: &AuthoringContext) -> Result<Value, RuntimeError> {
    let counts = topology_counts(&context.parameters, context.source_triangle_count)?;
    let topology_sha256 = topology_hash(&context.parameters);
    let mut value = json!({
        "schema_version":"AuthoringTopology@1",
        "scope":"single-direct-authoring-mesh-part",
        "complete":true,
        "project_id":context.project_id,
        "candidate_id":context.candidate_id,
        "artifact_id":context.artifact_id,
        "artifact_readback_sha256":context.artifact_readback_sha256,
        "geometry_candidate_evidence_sha256":context.geometry_candidate_evidence_sha256,
        "geometry_program_object_sha256":context.geometry_program_object_sha256,
        "program_sha256":context.program_sha256,
        "operator_catalog_sha256":context.operator_catalog_sha256,
        "readback_config_sha256":context.readback_config_sha256,
        "authoring_node_id":context.authoring_node_id,
        "part_id":context.part_id,
        "material_zone_id":context.material_zone_id,
        "solid":context.solid,
        "topology_policy":"triangle-quad-manifold-with-boundary@1",
        "authoring_topology_policy_sha256":AUTHORING_TOPOLOGY_POLICY_SHA256,
        "topology_space":"source-authoring-node-local@1",
        "id_scope":"geometry-program-node-bound",
        "cross_version_stable":false,
        "node_transform":{
            "position_m":context.parameters.get("position_m"),
            "rotation_rad":context.parameters.get("rotation_rad"),
        },
        "counts":{
            "vertex_count":counts["vertex_count"],
            "edge_count":counts["edge_count"],
            "loop_count":counts["loop_count"],
            "face_count":counts["face_count"],
        },
        "vertices":context.parameters.get("vertices"),
        "edges":context.parameters.get("edges"),
        "loops":context.parameters.get("loops"),
        "faces":context.parameters.get("faces"),
        "authoring_mesh_sha256":canonical_json_hash(&Value::Object(context.parameters.clone())),
        "topology_sha256":topology_sha256,
        "max_response_bytes":MAX_RESPONSE_BYTES,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "quality_status":"structural_only",
        "limitations":[
            "SOURCE_AUTHORING_MESH_ONLY",
            "SINGLE_DIRECT_PART_OUTPUT_ONLY",
            "NO_SELECTION_HISTORY_OR_CROSS_VERSION_ID_PROMISE",
            "NO_BLENDER_BMESH_PYTHON_OR_PLUGIN_RUNTIME",
            "STRUCTURAL_TOPOLOGY_DOES_NOT_PROVE_VISUAL_QUALITY"
        ],
        "canonical_sha256":"",
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    if canonical_json_bytes(&value)
        .map_err(|error| invalid(error.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid("AuthoringTopology response exceeds 1 MiB"));
    }
    Ok(value)
}

pub(super) fn get(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    topology_value(&load_context(runtime, request)?)
}

fn edit_identifier(prefix: &str, input_sha256: &str, kind: &str, ordinal: usize) -> String {
    let hash = canonical_json_hash(&json!({
        "input_sha256":input_sha256,
        "kind":kind,
        "ordinal":ordinal,
    }));
    format!("{prefix}-{}", &hash[..32])
}

fn face_loop_identifier(input_sha256: &str, face_ordinal: usize, loop_ordinal: usize) -> String {
    let face_hash = canonical_json_hash(&json!({
        "input_sha256":input_sha256,
        "kind":"face-loop-cycle",
        "face_ordinal":face_ordinal,
    }));
    // The ordinal suffix deliberately makes loop 00 the lexical minimum.
    // This preserves the Worker contract's rotation-canonical face cycle.
    format!("xl-{}-{loop_ordinal:02}", &face_hash[..24])
}

fn element_id(value: &Value) -> Result<&str, RuntimeError> {
    value
        .get("element_id")
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| invalid("topology element ID is invalid"))
}

fn vec3(value: &Value, context: &str) -> Result<[f64; 3], RuntimeError> {
    let values = value
        .as_array()
        .filter(|values| values.len() == 3)
        .ok_or_else(|| invalid(format!("{context} must be a vec3")))?;
    let result = [
        values[0].as_f64().unwrap_or(f64::NAN),
        values[1].as_f64().unwrap_or(f64::NAN),
        values[2].as_f64().unwrap_or(f64::NAN),
    ];
    if result.iter().any(|value| !value.is_finite()) {
        return Err(invalid(format!("{context} must be finite")));
    }
    Ok(result)
}

fn sort_elements(values: &mut [Value]) {
    values.sort_by(|left, right| {
        left.get("element_id")
            .and_then(Value::as_str)
            .cmp(&right.get("element_id").and_then(Value::as_str))
    });
}

fn edge_forward(vertex_ids: &[String; 2], from: &str, to: &str) -> Result<bool, RuntimeError> {
    if vertex_ids[0] == from && vertex_ids[1] == to {
        Ok(true)
    } else if vertex_ids[1] == from && vertex_ids[0] == to {
        Ok(false)
    } else {
        Err(invalid("face edge endpoints differ from winding"))
    }
}

fn apply_translate(
    parameters: &mut Map<String, Value>,
    edit: &Map<String, Value>,
) -> Result<Value, RuntimeError> {
    let ids = edit
        .get("vertex_ids")
        .and_then(Value::as_array)
        .filter(|values| (1..=64).contains(&values.len()))
        .ok_or_else(|| invalid("translate vertex_ids must contain 1..64 IDs"))?;
    let selected = ids
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| is_opaque_id(value))
                .map(str::to_owned)
                .ok_or_else(|| invalid("translate vertex ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if selected.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invalid(
            "translate vertex_ids must be unique and lexically sorted",
        ));
    }
    let delta = vec3(
        edit.get("delta_m")
            .ok_or_else(|| invalid("translate delta_m is required"))?,
        "translate delta_m",
    )?;
    if delta.iter().any(|value| value.abs() > 1.0)
        || delta.iter().all(|value| value.abs() <= f64::EPSILON)
    {
        return Err(invalid(
            "translate delta must be non-zero and inside [-1,1]m",
        ));
    }
    let selected_set = selected.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let vertices = parameters
        .get_mut("vertices")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("authoring vertices are unavailable"))?;
    let mut found = BTreeSet::new();
    for vertex in vertices {
        let id = element_id(vertex)?.to_owned();
        if selected_set.contains(id.as_str()) {
            let position = vec3(
                vertex
                    .get("position_m")
                    .ok_or_else(|| invalid("vertex position is unavailable"))?,
                "vertex position",
            )?;
            let moved = [
                position[0] + delta[0],
                position[1] + delta[1],
                position[2] + delta[2],
            ];
            if moved
                .iter()
                .any(|value| !value.is_finite() || value.abs() > 10.0)
            {
                return Err(invalid(
                    "translated vertex leaves the 10m coordinate envelope",
                ));
            }
            vertex["position_m"] = json!(moved);
            found.insert(id);
        }
    }
    if found.len() != selected.len() {
        return Err(invalid("translate references an unknown vertex"));
    }
    Ok(json!({
        "source_vertex_ids":selected,
        "source_face_ids":[],
        "generated_vertex_ids":[],
        "generated_edge_ids":[],
        "generated_loop_ids":[],
        "generated_face_ids":[],
    }))
}

fn apply_extrude(
    parameters: &mut Map<String, Value>,
    edit: &Map<String, Value>,
    input_sha256: &str,
) -> Result<Value, RuntimeError> {
    let face_id = identifier(edit, "face_id")?.to_owned();
    let distance = edit
        .get("distance_m")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && (0.000001..=1.0).contains(value))
        .ok_or_else(|| invalid("extrude distance_m must be inside 0.000001..1m"))?;
    let mut vertices = value_array(parameters, "vertices")?.clone();
    let mut edges = value_array(parameters, "edges")?.clone();
    let mut loops = value_array(parameters, "loops")?.clone();
    let mut faces = value_array(parameters, "faces")?.clone();

    let source_face = faces
        .iter()
        .find(|face| element_id(face).ok() == Some(face_id.as_str()))
        .cloned()
        .ok_or_else(|| invalid("extrude face is unavailable"))?;
    let source_loop_ids = source_face
        .get("loop_ids")
        .and_then(Value::as_array)
        .filter(|ids| (3..=4).contains(&ids.len()))
        .ok_or_else(|| invalid("extrude source face loops are invalid"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|value| is_opaque_id(value))
                .map(str::to_owned)
                .ok_or_else(|| invalid("extrude source loop ID is invalid"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let loop_map = loops
        .iter()
        .map(|value| Ok((element_id(value)?.to_owned(), value.clone())))
        .collect::<Result<BTreeMap<_, _>, RuntimeError>>()?;
    let edge_map = edges
        .iter()
        .map(|value| {
            let endpoints = value
                .get("vertex_ids")
                .and_then(Value::as_array)
                .filter(|items| items.len() == 2)
                .ok_or_else(|| invalid("edge endpoints are invalid"))?;
            Ok((
                element_id(value)?.to_owned(),
                [
                    endpoints[0].as_str().unwrap_or_default().to_owned(),
                    endpoints[1].as_str().unwrap_or_default().to_owned(),
                ],
            ))
        })
        .collect::<Result<BTreeMap<_, _>, RuntimeError>>()?;
    let position_map = vertices
        .iter()
        .map(|value| {
            Ok((
                element_id(value)?.to_owned(),
                vec3(
                    value
                        .get("position_m")
                        .ok_or_else(|| invalid("vertex position is unavailable"))?,
                    "vertex position",
                )?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, RuntimeError>>()?;

    let mut old_vertex_ids = Vec::new();
    let mut old_edge_ids = Vec::new();
    for (ordinal, loop_id) in source_loop_ids.iter().enumerate() {
        let loop_value = loop_map
            .get(loop_id)
            .ok_or_else(|| invalid("extrude source loop is unavailable"))?;
        if loop_value.get("face_id").and_then(Value::as_str) != Some(face_id.as_str())
            || loop_value.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
        {
            return Err(invalid("extrude source loop ownership or order differs"));
        }
        old_vertex_ids.push(
            loop_value
                .get("vertex_id")
                .and_then(Value::as_str)
                .filter(|value| is_opaque_id(value))
                .ok_or_else(|| invalid("extrude source vertex ID is invalid"))?
                .to_owned(),
        );
        old_edge_ids.push(
            loop_value
                .get("edge_id")
                .and_then(Value::as_str)
                .filter(|value| is_opaque_id(value))
                .ok_or_else(|| invalid("extrude source edge ID is invalid"))?
                .to_owned(),
        );
    }
    let edge_incidence = loops.iter().try_fold(
        BTreeMap::<String, usize>::new(),
        |mut counts, loop_value| -> Result<_, RuntimeError> {
            let edge_id = loop_value
                .get("edge_id")
                .and_then(Value::as_str)
                .filter(|value| is_opaque_id(value))
                .ok_or_else(|| invalid("source loop edge ID is invalid"))?;
            *counts.entry(edge_id.to_owned()).or_default() += 1;
            Ok(counts)
        },
    )?;
    if old_edge_ids
        .iter()
        .any(|edge_id| edge_incidence.get(edge_id) != Some(&1))
    {
        return Err(invalid(
            "single_face_extrude@1 accepts boundary faces only; interior face extrusion is unavailable",
        ));
    }
    let p0 = position_map[&old_vertex_ids[0]];
    let p1 = position_map[&old_vertex_ids[1]];
    let p2 = position_map[&old_vertex_ids[2]];
    let a = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let b = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let cross = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let length = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    if !length.is_finite() || length <= 1.0e-8 {
        return Err(invalid("extrude source face normal is degenerate"));
    }
    let normal = [cross[0] / length, cross[1] / length, cross[2] / length];
    if old_vertex_ids.len() == 4 {
        let p3 = position_map[&old_vertex_ids[3]];
        let from_plane = [p3[0] - p0[0], p3[1] - p0[1], p3[2] - p0[2]];
        let plane_distance =
            from_plane[0] * normal[0] + from_plane[1] * normal[1] + from_plane[2] * normal[2];
        if plane_distance.abs() > 1.0e-6 {
            return Err(invalid("single_face_extrude@1 requires a planar quad"));
        }
        let points = [p0, p1, p2, p3];
        for ordinal in 0..4 {
            let current = points[ordinal];
            let next = points[(ordinal + 1) % 4];
            let after = points[(ordinal + 2) % 4];
            let left = [
                next[0] - current[0],
                next[1] - current[1],
                next[2] - current[2],
            ];
            let right = [after[0] - next[0], after[1] - next[1], after[2] - next[2]];
            let turn = [
                left[1] * right[2] - left[2] * right[1],
                left[2] * right[0] - left[0] * right[2],
                left[0] * right[1] - left[1] * right[0],
            ];
            let signed = turn[0] * normal[0] + turn[1] * normal[1] + turn[2] * normal[2];
            if !signed.is_finite() || signed <= 1.0e-8 {
                return Err(invalid(
                    "single_face_extrude@1 requires a convex authored quad",
                ));
            }
        }
    }

    let mut new_vertex_ids = Vec::new();
    for (ordinal, old_id) in old_vertex_ids.iter().enumerate() {
        let new_id = edit_identifier("xv", input_sha256, "vertex", ordinal);
        let position = position_map[old_id];
        let moved = [
            position[0] + normal[0] * distance,
            position[1] + normal[1] * distance,
            position[2] + normal[2] * distance,
        ];
        if moved
            .iter()
            .any(|value| !value.is_finite() || value.abs() > 10.0)
        {
            return Err(invalid(
                "extruded vertex leaves the 10m coordinate envelope",
            ));
        }
        vertices.push(json!({"element_id":new_id,"position_m":moved}));
        new_vertex_ids.push(new_id);
    }

    let count = old_vertex_ids.len();
    let mut top_edge_ids = Vec::new();
    let mut vertical_edge_ids = Vec::new();
    let mut generated_edges = Vec::new();
    for ordinal in 0..count {
        let next = (ordinal + 1) % count;
        let top_id = edit_identifier("xe", input_sha256, "top-edge", ordinal);
        let mut top_endpoints = [
            new_vertex_ids[ordinal].clone(),
            new_vertex_ids[next].clone(),
        ];
        top_endpoints.sort();
        edges.push(json!({"element_id":top_id,"vertex_ids":top_endpoints}));
        generated_edges.push(top_id.clone());
        top_edge_ids.push(top_id);
        let vertical_id = edit_identifier("xe", input_sha256, "vertical-edge", ordinal);
        let mut vertical_endpoints = [
            old_vertex_ids[ordinal].clone(),
            new_vertex_ids[ordinal].clone(),
        ];
        vertical_endpoints.sort();
        edges.push(json!({"element_id":vertical_id,"vertex_ids":vertical_endpoints}));
        generated_edges.push(vertical_id.clone());
        vertical_edge_ids.push(vertical_id);
    }
    let mut all_edge_map = edge_map;
    for edge in &edges {
        let id = element_id(edge)?.to_owned();
        let endpoints = edge["vertex_ids"]
            .as_array()
            .ok_or_else(|| invalid("generated edge endpoints are invalid"))?;
        all_edge_map.insert(
            id,
            [
                endpoints[0].as_str().unwrap_or_default().to_owned(),
                endpoints[1].as_str().unwrap_or_default().to_owned(),
            ],
        );
    }

    faces.retain(|face| element_id(face).ok() != Some(face_id.as_str()));
    let source_loop_set = source_loop_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    loops.retain(|item| {
        element_id(item)
            .ok()
            .is_some_and(|id| !source_loop_set.contains(id))
    });
    let top_face_id = edit_identifier("xf", input_sha256, "top-face", 0);
    let mut generated_loop_ids = Vec::new();
    let mut generated_face_ids = vec![top_face_id.clone()];

    let mut append_face = |new_face_id: &str,
                           vertex_winding: &[String],
                           edge_winding: &[String],
                           face_ordinal: usize|
     -> Result<(), RuntimeError> {
        let mut face_loop_ids = Vec::new();
        for ordinal in 0..vertex_winding.len() {
            let loop_id = face_loop_identifier(input_sha256, face_ordinal, ordinal);
            let endpoints = all_edge_map
                .get(&edge_winding[ordinal])
                .ok_or_else(|| invalid("generated face edge is unavailable"))?;
            let next = &vertex_winding[(ordinal + 1) % vertex_winding.len()];
            let forward = edge_forward(endpoints, &vertex_winding[ordinal], next)?;
            loops.push(json!({
                "element_id":loop_id,
                "face_id":new_face_id,
                "ordinal":ordinal,
                "vertex_id":vertex_winding[ordinal],
                "edge_id":edge_winding[ordinal],
                "edge_forward":forward,
            }));
            generated_loop_ids.push(loop_id.clone());
            face_loop_ids.push(loop_id);
        }
        faces.push(json!({"element_id":new_face_id,"loop_ids":face_loop_ids}));
        Ok(())
    };
    append_face(&top_face_id, &new_vertex_ids, &top_edge_ids, 0)?;
    for ordinal in 0..count {
        let next = (ordinal + 1) % count;
        let side_face_id = edit_identifier("xf", input_sha256, "side-face", ordinal);
        let winding = vec![
            old_vertex_ids[ordinal].clone(),
            old_vertex_ids[next].clone(),
            new_vertex_ids[next].clone(),
            new_vertex_ids[ordinal].clone(),
        ];
        let edge_winding = vec![
            old_edge_ids[ordinal].clone(),
            vertical_edge_ids[next].clone(),
            top_edge_ids[ordinal].clone(),
            vertical_edge_ids[ordinal].clone(),
        ];
        append_face(&side_face_id, &winding, &edge_winding, ordinal + 1)?;
        generated_face_ids.push(side_face_id);
    }

    sort_elements(&mut vertices);
    sort_elements(&mut edges);
    sort_elements(&mut loops);
    sort_elements(&mut faces);
    parameters.insert("vertices".to_owned(), Value::Array(vertices));
    parameters.insert("edges".to_owned(), Value::Array(edges));
    parameters.insert("loops".to_owned(), Value::Array(loops));
    parameters.insert("faces".to_owned(), Value::Array(faces));
    generated_edges.sort();
    generated_loop_ids.sort();
    generated_face_ids.sort();
    Ok(json!({
        "source_vertex_ids":old_vertex_ids,
        "source_face_ids":[face_id],
        "generated_vertex_ids":new_vertex_ids,
        "generated_edge_ids":generated_edges,
        "generated_loop_ids":generated_loop_ids,
        "generated_face_ids":generated_face_ids,
    }))
}

fn replay_value(
    artifact_sha256: &str,
    artifact_size_bytes: usize,
    program_sha256: &str,
    worker_build_cohort_sha256: &Option<String>,
    triangle_count: u64,
    part_ids: &[String],
) -> Value {
    json!({
        "artifact_sha256":artifact_sha256,
        "artifact_size_bytes":artifact_size_bytes,
        "program_sha256":program_sha256,
        "worker_build_cohort_sha256":worker_build_cohort_sha256,
        "triangle_count":triangle_count,
        "part_ids":part_ids,
        "byte_exact_repeat":true,
        "strict_readback_passed":true,
    })
}

struct EditComputation {
    preview: Value,
    derived_program: Value,
    derived_glb: Vec<u8>,
    derived_worker_cohort_sha256: Option<String>,
}

fn compute_edit(runtime: &Runtime, request: &Value) -> Result<EditComputation, RuntimeError> {
    let outer = exact_object(
        request,
        &[
            "schema_version",
            "topology_request",
            "base_topology_sha256",
            "edit",
            "edit_policy_sha256",
            "input_sha256",
        ],
        "AuthoringMeshEditPreviewRequest@1",
    )?;
    if text(outer, "schema_version")? != "AuthoringMeshEditPreviewRequest@1"
        || sha(outer, "edit_policy_sha256")? != EDIT_POLICY_SHA256
        || sha256_hex(EDIT_POLICY.as_bytes()) != EDIT_POLICY_SHA256
    {
        return Err(invalid("edit schema or policy differs"));
    }
    let input_sha256 = sha(outer, "input_sha256")?.to_owned();
    let base_topology_sha256 = sha(outer, "base_topology_sha256")?.to_owned();
    let mut preimage = request.clone();
    preimage
        .as_object_mut()
        .expect("validated edit preview object")
        .remove("input_sha256");
    if canonical_json_hash(&preimage) != input_sha256 {
        return Err(invalid(
            "input_sha256 differs from the closed preview request",
        ));
    }
    let topology_request = outer
        .get("topology_request")
        .ok_or_else(|| invalid("topology_request is required"))?;
    let context = load_context(runtime, topology_request)?;
    let source_topology = topology_value(&context)?;
    let source_topology_sha256 = source_topology["topology_sha256"]
        .as_str()
        .ok_or_else(|| invalid("source topology hash is unavailable"))?
        .to_owned();
    if base_topology_sha256 != source_topology_sha256 {
        return Err(invalid(
            "base_topology_sha256 is stale or belongs to another authoring topology",
        ));
    }
    let edit = exact_object(
        outer
            .get("edit")
            .ok_or_else(|| invalid("edit is required"))?,
        match outer["edit"].get("operation").and_then(Value::as_str) {
            Some("translate_vertices") => &["operation", "vertex_ids", "delta_m"],
            Some("single_face_extrude") => &["operation", "face_id", "distance_m"],
            _ => return Err(invalid("edit operation is unsupported")),
        },
        "authoring edit",
    )?;
    let operation = text(edit, "operation")?.to_owned();
    let mut derived_parameters = context.parameters.clone();
    let edited_element_ids = match operation.as_str() {
        "translate_vertices" => apply_translate(&mut derived_parameters, edit)?,
        "single_face_extrude" => apply_extrude(&mut derived_parameters, edit, &input_sha256)?,
        _ => unreachable!(),
    };
    let source_counts = topology_counts(&context.parameters, context.source_triangle_count)?;
    let derived_topology_sha256 = topology_hash(&derived_parameters);
    let mut derived_program = context.program.clone();
    derived_program
        .as_object_mut()
        .expect("validated GeometryProgram")
        .remove("canonical_sha256");
    derived_program["nodes"][context.node_index]["parameters"] =
        Value::Object(derived_parameters.clone());
    let hash = hash_geometry_program_with_runtime_worker(&derived_program).map_err(|error| {
        invalid(format!(
            "derived GeometryProgram validation failed: {error}"
        ))
    })?;
    let derived_program_sha256 = hash
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("derived GeometryProgram hash is unavailable"))?
        .to_owned();
    if derived_program_sha256 == context.program_sha256
        || hash.get("operator_catalog_sha256").and_then(Value::as_str)
            != Some(context.operator_catalog_sha256.as_str())
    {
        return Err(invalid(
            "derived GeometryProgram hash or catalog did not change as expected",
        ));
    }
    derived_program["canonical_sha256"] = Value::String(derived_program_sha256.clone());
    let first = compile_geometry_with_runtime_worker(&derived_program, None)
        .map_err(|error| invalid(format!("derived Geometry Worker preview failed: {error}")))?;
    let repeat = compile_geometry_with_runtime_worker(&derived_program, None)
        .map_err(|error| invalid(format!("derived Geometry Worker repeat failed: {error}")))?;
    if first.glb != repeat.glb
        || first.program_sha256 != derived_program_sha256
        || repeat.program_sha256 != derived_program_sha256
        || first.build_cohort_sha256 != repeat.build_cohort_sha256
    {
        return Err(invalid("derived Worker replay is not byte exact"));
    }
    let derived_inspection = strict_glb_inspection(&first.glb)?;
    validate_worker_metadata(&first, &derived_inspection)?;
    if !derived_inspection.hard_gate_passed
        || first.glb.len() > MAX_GLB_BYTES as usize
        || first.part_ids != context.source_part_ids
        || first.material_zone_ids != context.source_material_zone_ids
    {
        return Err(invalid(
            "derived strict GLB readback or 64 MiB budget failed",
        ));
    }
    let after_counts = topology_counts(&derived_parameters, first.triangle_count)?;
    let edit_lineage_sha256 = canonical_json_hash(&json!({
        "source_program_sha256":context.program_sha256,
        "derived_program_sha256":derived_program_sha256,
        "source_topology_sha256":source_topology_sha256,
        "derived_topology_sha256":derived_topology_sha256,
        "operation":operation,
        "edited_element_ids":edited_element_ids,
    }));
    let mut value = json!({
        "schema_version":"AuthoringMeshEditPreview@1",
        "project_id":context.project_id,
        "candidate_id":context.candidate_id,
        "source_artifact_id":context.artifact_id,
        "artifact_readback_sha256":context.artifact_readback_sha256,
        "geometry_candidate_evidence_sha256":context.geometry_candidate_evidence_sha256,
        "source_program_sha256":context.program_sha256,
        "derived_program_sha256":derived_program_sha256,
        "operator_catalog_sha256":context.operator_catalog_sha256,
        "readback_config_sha256":context.readback_config_sha256,
        "authoring_node_id":context.authoring_node_id,
        "part_id":context.part_id,
        "source_topology_sha256":source_topology_sha256,
        "derived_topology_sha256":derived_topology_sha256,
        "edit_lineage_sha256":edit_lineage_sha256,
        "edit_policy_sha256":EDIT_POLICY_SHA256,
        "input_sha256":input_sha256,
        "operation":operation,
        "edited_element_ids":edited_element_ids,
        "counts":{"before":source_counts,"after":after_counts},
        "source_replay":replay_value(
            &context.artifact_id,
            context.source_artifact_bytes.len(),
            &context.program_sha256,
            &context.source_worker_cohort_sha256,
            context.source_triangle_count,
            &context.source_part_ids,
        ),
        "derived_replay":replay_value(
            &sha256_hex(&first.glb),
            first.glb.len(),
            &derived_program_sha256,
            &first.build_cohort_sha256,
            first.triangle_count,
            &first.part_ids,
        ),
        "geometry_materialization":"transient-worker-glb-not-persisted",
        "max_response_bytes":MAX_RESPONSE_BYTES,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "validator_status":"passed",
        "quality_status":"structural_only",
        "limitations":[
            "PREVIEW_ONLY_NO_CANDIDATE_OR_VERSION",
            "TRANSLATE_VERTICES_OR_SINGLE_FACE_EXTRUDE_ONLY",
            "GENERATED_IDS_STABLE_ONLY_FOR_EXACT_SOURCE_AND_EDIT",
            "NO_SELECTION_HISTORY_UNDO_OR_PERSISTENT_MESH_EDIT",
            "NO_BLENDER_BMESH_PYTHON_OR_PLUGIN_RUNTIME",
            "STRUCTURAL_READBACK_DOES_NOT_PROVE_VISUAL_QUALITY"
        ],
        "canonical_sha256":"",
    });
    value["canonical_sha256"] = Value::String(canonical_json_hash(&value));
    if canonical_json_bytes(&value)
        .map_err(|error| invalid(error.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid("edit preview response exceeds 1 MiB"));
    }
    Ok(EditComputation {
        preview: value,
        derived_program,
        derived_glb: first.glb,
        derived_worker_cohort_sha256: first.build_cohort_sha256,
    })
}

pub(super) fn preview(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    Ok(compute_edit(runtime, request)?.preview)
}

fn rollback_new_materialization(
    runtime: &Runtime,
    objects: &[CasObject],
) -> Result<(), RuntimeError> {
    for object in objects.iter().rev().filter(|object| object.created_new) {
        runtime
            .store
            .discard_new_temporary_authoring_mesh_edit_object(object)?;
    }
    Ok(())
}

fn validate_prepare_output(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "source_candidate_id",
            "source_candidate_canonical_sha256",
            "base_version_id",
            "new_candidate_id",
            "candidate",
            "job",
            "input_sha256",
            "preview_input_sha256",
            "expected_preview_canonical_sha256",
            "preview_canonical_sha256",
            "operation",
            "edit_policy_sha256",
            "edited_element_ids",
            "edit_lineage_sha256",
            "source_topology_sha256",
            "derived_topology_sha256",
            "source_program_sha256",
            "derived_program_sha256",
            "source_artifact_sha256",
            "derived_artifact_sha256",
            "source_artifact_readback_sha256",
            "derived_artifact_readback_sha256",
            "source_geometry_candidate_evidence_sha256",
            "derived_geometry_candidate_evidence_sha256",
            "source_worker_build_cohort_sha256",
            "derived_worker_build_cohort_sha256",
            "geometry_materialization",
            "materialization_status",
            "runtime_write_performed",
            "persistent_user_data_touched",
            "version_status",
            "confirm_status",
            "export_status",
            "max_response_bytes",
            "validator_status",
            "quality_status",
            "limitations",
            "canonical_sha256",
        ],
        "AuthoringMeshEditPrepare@1",
    )?;
    if text(object, "schema_version")? != "AuthoringMeshEditPrepare@1"
        || text(object, "geometry_materialization")? != "runtime-owned-cas-staged-candidate"
        || text(object, "materialization_status")? != "runtime-owned-staged-candidate"
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(true)
        || object
            .get("persistent_user_data_touched")
            .and_then(Value::as_bool)
            != Some(true)
        || text(object, "version_status")? != "no-version-created"
        || text(object, "confirm_status")? != "approval-required"
        || text(object, "export_status")? != "locked-until-confirm"
        || object.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
        || text(object, "validator_status")? != "passed"
        || text(object, "quality_status")? != "structural_only"
    {
        return Err(invalid("prepare output constants differ"));
    }
    for key in [
        "source_candidate_canonical_sha256",
        "input_sha256",
        "preview_input_sha256",
        "expected_preview_canonical_sha256",
        "preview_canonical_sha256",
        "edit_policy_sha256",
        "edit_lineage_sha256",
        "source_topology_sha256",
        "derived_topology_sha256",
        "source_program_sha256",
        "derived_program_sha256",
        "source_artifact_sha256",
        "derived_artifact_sha256",
        "source_artifact_readback_sha256",
        "derived_artifact_readback_sha256",
        "source_geometry_candidate_evidence_sha256",
        "derived_geometry_candidate_evidence_sha256",
        "source_worker_build_cohort_sha256",
        "derived_worker_build_cohort_sha256",
        "canonical_sha256",
    ] {
        sha(object, key)?;
    }
    for key in ["project_id", "source_candidate_id", "new_candidate_id"] {
        identifier(object, key)?;
    }
    if object.get("candidate").and_then(Value::as_object).is_none()
        || object.get("job").and_then(Value::as_object).is_none()
        || object["candidate"].get("candidate_id") != object.get("new_candidate_id")
        || object["candidate"].get("project_id") != object.get("project_id")
        || object["candidate"].get("state").and_then(Value::as_str) != Some("reviewable")
        || object["candidate"]
            .get("quality_hard_gate_passed")
            .and_then(Value::as_bool)
            != Some(true)
        || object["job"].get("project_id") != object.get("project_id")
        || object["job"].get("kind").and_then(Value::as_str) != Some("authoring_mesh_edit_prepare")
        || object["job"].get("status").and_then(Value::as_str) != Some("succeeded")
        || object.get("expected_preview_canonical_sha256") != object.get("preview_canonical_sha256")
        || object.get("edit_policy_sha256").and_then(Value::as_str) != Some(EDIT_POLICY_SHA256)
    {
        return Err(invalid("prepare output bindings differ"));
    }
    let limitations = object
        .get("limitations")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("prepare limitations are missing"))?;
    let expected_limitations = [
        "STAGED_CANDIDATE_REQUIRES_USER_APPROVAL_BEFORE_CONFIRM",
        "NO_VERSION_CREATED_UNTIL_CONFIRM",
        "NO_EXPORT_BEFORE_CONFIRM",
        "TRANSLATE_VERTICES_OR_SINGLE_FACE_EXTRUDE_ONLY",
        "GENERATED_IDS_STABLE_ONLY_FOR_EXACT_SOURCE_AND_EDIT",
        "NO_CROSS_VERSION_STABLE_ELEMENT_ID_PROMISE",
        "NO_SELECTION_HISTORY_OR_UNDO_BLOB_IN_GEOMETRY_TRUTH",
        "NO_BLENDER_BMESH_PYTHON_OR_PLUGIN_RUNTIME",
        "STRUCTURAL_READBACK_DOES_NOT_PROVE_VISUAL_QUALITY",
    ];
    if limitations.len() != expected_limitations.len()
        || limitations
            .iter()
            .zip(expected_limitations)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(invalid("prepare limitations differ"));
    }
    let expected_hash = sha(object, "canonical_sha256")?.to_owned();
    let mut preimage = value.clone();
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != expected_hash {
        return Err(invalid("prepare canonical hash differs"));
    }
    if canonical_json_bytes(value)
        .map_err(|error| invalid(error.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return Err(invalid("prepare response exceeds 1 MiB"));
    }
    Ok(())
}

pub(super) fn prepare(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let outer = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "source_candidate_id",
            "base_version_id",
            "preview_request",
            "expected_preview_canonical_sha256",
            "idempotency_key",
            "max_response_bytes",
            "input_sha256",
        ],
        "AuthoringMeshEditPrepareRequest@1",
    )?;
    if text(outer, "schema_version")? != "AuthoringMeshEditPrepareRequest@1"
        || outer.get("max_response_bytes").and_then(Value::as_u64)
            != Some(MAX_RESPONSE_BYTES as u64)
    {
        return Err(invalid("prepare request schema or response budget differs"));
    }
    let project_id = identifier(outer, "project_id")?.to_owned();
    let source_candidate_id = identifier(outer, "source_candidate_id")?.to_owned();
    let base_version_id = match outer.get("base_version_id") {
        Some(Value::Null) => None,
        Some(Value::String(value)) if is_opaque_id(value) => Some(value.clone()),
        _ => return Err(invalid("base_version_id must be an identifier or null")),
    };
    let idempotency_key = identifier(outer, "idempotency_key")?.to_owned();
    let expected_preview_canonical_sha256 =
        sha(outer, "expected_preview_canonical_sha256")?.to_owned();
    let input_sha256 = sha(outer, "input_sha256")?.to_owned();
    let mut request_preimage = request.clone();
    request_preimage
        .as_object_mut()
        .expect("validated prepare request")
        .remove("input_sha256");
    if canonical_json_hash(&request_preimage) != input_sha256 {
        return Err(invalid("prepare input_sha256 differs"));
    }
    let preview_request = outer
        .get("preview_request")
        .ok_or_else(|| invalid("preview_request is required"))?;
    if preview_request
        .get("topology_request")
        .and_then(|value| value.get("project_id"))
        .and_then(Value::as_str)
        != Some(project_id.as_str())
        || preview_request
            .get("topology_request")
            .and_then(|value| value.get("candidate_id"))
            .and_then(Value::as_str)
            != Some(source_candidate_id.as_str())
    {
        return Err(invalid(
            "prepare project/source candidate differ from preview topology request",
        ));
    }
    let computation = compute_edit(runtime, preview_request)?;
    let preview_canonical_sha256 = computation.preview["canonical_sha256"]
        .as_str()
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("preview canonical hash is unavailable"))?
        .to_owned();
    if preview_canonical_sha256 != expected_preview_canonical_sha256 {
        return Err(invalid("expected preview canonical hash differs"));
    }
    let source_candidate = runtime
        .candidate(&source_candidate_id)?
        .ok_or_else(|| invalid("source candidate is unavailable"))?;
    if source_candidate.project_id != project_id
        || !source_candidate.quality_hard_gate_passed
        || !matches!(source_candidate.state.as_str(), "reviewable" | "confirmed")
    {
        return Err(invalid(
            "source candidate is outside scope or not structurally reviewable",
        ));
    }
    let current_head_version_id = runtime
        .store
        .latest_version_for_project(&project_id)?
        .map(|version| version.version_id);
    if current_head_version_id != base_version_id {
        return Err(invalid(
            "prepare base_version_id is not the current project head",
        ));
    }
    let source_worker_build_cohort_sha256 = computation.preview["source_replay"]
        ["worker_build_cohort_sha256"]
        .as_str()
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("source Worker cohort is unavailable"))?
        .to_owned();
    let derived_worker_build_cohort_sha256 = computation
        .derived_worker_cohort_sha256
        .as_deref()
        .filter(|value| is_sha256(value))
        .ok_or_else(|| invalid("derived Worker cohort is unavailable"))?
        .to_owned();

    let context = load_context(
        runtime,
        preview_request
            .get("topology_request")
            .ok_or_else(|| invalid("topology_request is unavailable"))?,
    )?;
    let mut created_objects = Vec::<CasObject>::new();
    let materialized = (|| -> Result<Value, RuntimeError> {
        let inspection = strict_glb_inspection(&computation.derived_glb)?;
        if !inspection.hard_gate_passed
            || inspection.program_sha256
                != computation.preview["derived_program_sha256"]
                    .as_str()
                    .unwrap_or_default()
            || inspection.operator_catalog_sha256.as_deref()
                != Some(context.operator_catalog_sha256.as_str())
            || inspection.readback_config_sha256 != context.readback_config_sha256
        {
            return Err(invalid("derived persisted GLB binding differs"));
        }
        let glb_object = runtime.put_object(
            &computation.derived_glb,
            Some(&sha256_hex(&computation.derived_glb)),
            "model/gltf-binary",
            "geometry-glb",
        )?;
        created_objects.push(glb_object.clone());
        let mut program_draft = computation.derived_program.clone();
        program_draft
            .as_object_mut()
            .ok_or_else(|| invalid("derived GeometryProgram is not an object"))?
            .remove("canonical_sha256");
        let program_bytes =
            canonical_json_bytes(&program_draft).map_err(|error| invalid(error.to_string()))?;
        if program_bytes.len() > MAX_JSON_BYTES as usize {
            return Err(invalid("derived GeometryProgram exceeds 1 MiB"));
        }
        let program_object = runtime.put_object(
            &program_bytes,
            Some(&inspection.program_sha256),
            "application/json",
            "geometry-program-v2",
        )?;
        created_objects.push(program_object.clone());

        let timestamp = now_string();
        let candidate_id = format!("candidate-{}", Uuid::new_v4().simple());
        let quality_report_id = format!("quality-geometry-{}", Uuid::new_v4().simple());
        let readback = artifact_readback_v2_value(
            &glb_object.record.sha256,
            &candidate_id,
            &inspection,
            glb_object.record.size_bytes,
        );
        validate_artifact_readback_v2_output(&readback)?;
        let readback_bytes =
            canonical_json_bytes(&readback).map_err(|error| invalid(error.to_string()))?;
        let readback_object = runtime.put_object(
            &readback_bytes,
            None,
            "application/json",
            "geometry-artifact-readback-v2",
        )?;
        created_objects.push(readback_object.clone());
        let mut quality_report = json!({
            "schema_version":"GeometryQualityReport@2",
            "scope":"mcp010b-strict-glb-bin-accessor-hard-gates",
            "quality_report_id":quality_report_id,
            "candidate_id":candidate_id,
            "artifact_sha256":glb_object.record.sha256,
            "program_sha256":inspection.program_sha256,
            "operator_catalog_sha256":inspection.operator_catalog_sha256,
            "readback_config_sha256":inspection.readback_config_sha256,
            "artifact_readback_object_sha256":readback_object.record.sha256,
            "integrity":strict_integrity_value(&inspection),
            "hard_gate_passed":true,
            "canonical_sha256":""
        });
        quality_report["canonical_sha256"] = Value::String(canonical_json_hash(&quality_report));
        validate_geometry_quality_report_v2_output(&quality_report)?;
        let quality_bytes =
            canonical_json_bytes(&quality_report).map_err(|error| invalid(error.to_string()))?;
        let quality_object = runtime.put_object(
            &quality_bytes,
            None,
            "application/json",
            "geometry-quality-report",
        )?;
        created_objects.push(quality_object.clone());

        let prepared_object_id = format!("geometry-object-{}", &glb_object.record.sha256[..32]);
        let mut candidate = CandidateRecord {
            schema_version: "Candidate@1".to_owned(),
            candidate_id: candidate_id.clone(),
            project_id: project_id.clone(),
            base_version_id: base_version_id.clone(),
            source_version_id: None,
            prepared_object_id: Some(prepared_object_id),
            prepared_object_sha256: Some(glb_object.record.sha256.clone()),
            state: "reviewable".to_owned(),
            request_sha256: input_sha256.clone(),
            manifest_hash: Some(glb_object.record.sha256.clone()),
            quality_report_id: Some(quality_report_id.clone()),
            quality_hard_gate_passed: true,
            canonical_sha256: String::new(),
            error_code: None,
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };
        candidate.canonical_sha256 = canonical_json_hash(
            &serde_json::to_value(&candidate)
                .map_err(|error| invalid(format!("candidate serialization failed: {error}")))?,
        );
        let evidence_value = geometry_candidate_evidence_value(
            &candidate,
            context.reference_id.as_deref(),
            context.reference_sha256.as_deref(),
            &inspection,
            &program_object.record.sha256,
            &glb_object.record.sha256,
            &readback_object.record.sha256,
            &quality_object.record.sha256,
            &quality_report_id,
        );
        validate_geometry_candidate_evidence_output(&evidence_value)?;
        let evidence: GeometryCandidateEvidenceRecord = serde_json::from_value(evidence_value)
            .map_err(|error| invalid(format!("geometry evidence serialization failed: {error}")))?;
        let job_id = format!("job-{}", Uuid::new_v4().simple());
        let job = JobRecord {
            schema_version: "RuntimeJob@1".to_owned(),
            job_id: job_id.clone(),
            project_id: project_id.clone(),
            kind: "authoring_mesh_edit_prepare".to_owned(),
            status: "succeeded".to_owned(),
            progress: 100,
            request_sha256: input_sha256.clone(),
            checkpoint_sha256: None,
            error_code: None,
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
        };
        let job_summary = JobSummary {
            job_id: job.job_id.clone(),
            project_id: job.project_id.clone(),
            kind: job.kind.clone(),
            status: job.status.clone(),
            progress: job.progress,
            error_code: job.error_code.clone(),
            created_at: job.created_at.clone(),
            updated_at: job.updated_at.clone(),
        };
        let event = JobEventRecord {
            schema_version: "RuntimeJobEvent@1".to_owned(),
            job_id,
            sequence: 1,
            kind: "authoring_mesh_edit_prepared".to_owned(),
            payload: json!({
                "source_candidate_id":source_candidate_id,
                "candidate_id":candidate_id,
                "artifact_sha256":glb_object.record.sha256,
                "preview_canonical_sha256":preview_canonical_sha256,
                "edit_lineage_sha256":computation.preview["edit_lineage_sha256"],
            }),
            created_at: timestamp.clone(),
        };
        let audit = AuditEventRecord {
            schema_version: "AuditEvent@1".to_owned(),
            audit_id: format!("audit-{}", Uuid::new_v4().simple()),
            project_id: Some(project_id.clone()),
            kind: "authoring_mesh_edit_prepared".to_owned(),
            object_id: Some(candidate_id.clone()),
            request_sha256: Some(input_sha256.clone()),
            payload: event.payload.clone(),
            created_at: timestamp,
        };
        let mut result_object = json!({
            "schema_version":"AuthoringMeshEditPrepare@1",
            "project_id":project_id.clone(),
            "source_candidate_id":source_candidate_id.clone(),
            "source_candidate_canonical_sha256":source_candidate.canonical_sha256.clone(),
            "base_version_id":base_version_id.clone(),
            "new_candidate_id":candidate_id.clone(),
            "candidate":serde_json::to_value(&candidate).map_err(|error| invalid(format!("candidate serialization failed: {error}")))?,
            "job":serde_json::to_value(&job_summary).map_err(|error| invalid(format!("job summary serialization failed: {error}")))?,
            "input_sha256":input_sha256.clone(),
            "preview_input_sha256":computation.preview["input_sha256"],
            "expected_preview_canonical_sha256":expected_preview_canonical_sha256.clone(),
            "preview_canonical_sha256":preview_canonical_sha256.clone(),
            "operation":computation.preview["operation"],
            "edit_policy_sha256":EDIT_POLICY_SHA256,
            "edited_element_ids":computation.preview["edited_element_ids"],
            "edit_lineage_sha256":computation.preview["edit_lineage_sha256"],
            "source_topology_sha256":computation.preview["source_topology_sha256"],
            "derived_topology_sha256":computation.preview["derived_topology_sha256"],
            "source_program_sha256":computation.preview["source_program_sha256"],
            "derived_program_sha256":computation.preview["derived_program_sha256"]
        })
        .as_object()
        .expect("prepare result head")
        .clone();
        let result_tail = json!({
            "source_artifact_sha256":computation.preview["source_artifact_id"],
            "derived_artifact_sha256":glb_object.record.sha256.clone(),
            "source_artifact_readback_sha256":computation.preview["artifact_readback_sha256"],
            "derived_artifact_readback_sha256":readback["canonical_sha256"],
            "source_geometry_candidate_evidence_sha256":computation.preview["geometry_candidate_evidence_sha256"],
            "derived_geometry_candidate_evidence_sha256":evidence.canonical_sha256.clone(),
            "source_worker_build_cohort_sha256":source_worker_build_cohort_sha256.clone(),
            "derived_worker_build_cohort_sha256":derived_worker_build_cohort_sha256.clone(),
            "geometry_materialization":"runtime-owned-cas-staged-candidate",
            "materialization_status":"runtime-owned-staged-candidate",
            "runtime_write_performed":true,
            "persistent_user_data_touched":true,
            "version_status":"no-version-created",
            "confirm_status":"approval-required",
            "export_status":"locked-until-confirm",
            "max_response_bytes":MAX_RESPONSE_BYTES,
            "validator_status":"passed",
            "quality_status":"structural_only",
            "limitations":[
                "STAGED_CANDIDATE_REQUIRES_USER_APPROVAL_BEFORE_CONFIRM",
                "NO_VERSION_CREATED_UNTIL_CONFIRM",
                "NO_EXPORT_BEFORE_CONFIRM",
                "TRANSLATE_VERTICES_OR_SINGLE_FACE_EXTRUDE_ONLY",
                "GENERATED_IDS_STABLE_ONLY_FOR_EXACT_SOURCE_AND_EDIT",
                "NO_CROSS_VERSION_STABLE_ELEMENT_ID_PROMISE",
                "NO_SELECTION_HISTORY_OR_UNDO_BLOB_IN_GEOMETRY_TRUTH",
                "NO_BLENDER_BMESH_PYTHON_OR_PLUGIN_RUNTIME",
                "STRUCTURAL_READBACK_DOES_NOT_PROVE_VISUAL_QUALITY"
            ],
            "canonical_sha256":""
        });
        result_object.extend(
            result_tail
                .as_object()
                .expect("prepare result tail")
                .clone(),
        );
        let mut result = Value::Object(result_object);
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        validate_prepare_output(&result)?;
        let response_json = serde_json::to_string(&result)
            .map_err(|error| invalid(format!("prepare response serialization failed: {error}")))?;
        let stored_response = runtime.store.commit_authoring_mesh_edit_prepare(
            &source_candidate_id,
            current_head_version_id.as_deref(),
            base_version_id.as_deref(),
            &idempotency_key,
            &input_sha256,
            &candidate,
            &job,
            &event,
            &audit,
            &evidence,
            &response_json,
            true,
            &now_string(),
        )?;
        if stored_response != response_json {
            rollback_new_materialization(runtime, &created_objects)?;
        }
        let stored: Value = serde_json::from_str(&stored_response)
            .map_err(|error| invalid(format!("stored prepare response is invalid: {error}")))?;
        validate_prepare_output(&stored)?;
        Ok(stored)
    })();
    match materialized {
        Ok(value) => Ok(value),
        Err(error) => match rollback_new_materialization(runtime, &created_objects) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(invalid(format!(
                "prepare failed ({error}); temporary CAS rollback also failed ({rollback_error})"
            ))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_self_hash(value: &Value) {
        let actual = value["canonical_sha256"]
            .as_str()
            .expect("canonical hash")
            .to_owned();
        let mut preimage = value.clone();
        preimage["canonical_sha256"] = Value::String(String::new());
        assert_eq!(actual, canonical_json_hash(&preimage));
    }

    fn prepare_authoring_candidate(runtime: &Runtime) -> (Value, Value) {
        let project = runtime
            .create_project("authoring topology", json!({"profile":"mvp"}))
            .expect("project");
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project.project_id,
            "representation_plan_sha256":"b".repeat(64),
            "operator_catalog_sha256":super::super::operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{
                "max_nodes":1,
                "max_triangles":32,
                "max_glb_bytes":67108864,
                "max_worker_memory_bytes":536870912,
                "max_runtime_ms":10000
            },
            "nodes":[{
                "node_id":"authored-panel",
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
                "part_id":"authored-panel",
                "input_node_ids":["authored-panel"],
                "material_zone_id":"zone-authored-shell",
                "solid":false
            }]
        });
        let hash = hash_geometry_program_with_runtime_worker(&program).expect("program hash");
        program["canonical_sha256"] = hash["canonical_sha256"].clone();
        let prepared = runtime
            .prepare_geometry_candidate(
                project.project_id.as_str(),
                None,
                json!({"typed":"geometry","geometry_program":program}),
            )
            .expect("geometry prepare");
        let candidate_id = prepared["candidate"]["candidate_id"]
            .as_str()
            .expect("candidate ID");
        let evidence = runtime
            .store
            .get_geometry_candidate_evidence(candidate_id)
            .expect("evidence query")
            .expect("evidence");
        let request = json!({
            "schema_version":"AuthoringTopologyRequest@1",
            "project_id":project.project_id,
            "candidate_id":candidate_id,
            "artifact_id":prepared["artifact"]["artifact_id"],
            "artifact_readback_sha256":prepared["artifact"]["canonical_sha256"],
            "program_sha256":evidence.geometry_program_sha256,
            "operator_catalog_sha256":evidence.operator_catalog_sha256,
            "readback_config_sha256":evidence.readback_config_sha256,
            "authoring_node_id":"authored-panel",
            "part_id":"authored-panel",
            "authoring_topology_policy_sha256":AUTHORING_TOPOLOGY_POLICY_SHA256,
            "max_response_bytes":1048576
        });
        (prepared, request)
    }

    fn preview_request(topology_request: &Value, base_topology_sha256: &str, edit: Value) -> Value {
        let mut request = json!({
            "schema_version":"AuthoringMeshEditPreviewRequest@1",
            "topology_request":topology_request,
            "base_topology_sha256":base_topology_sha256,
            "edit":edit,
            "edit_policy_sha256":EDIT_POLICY_SHA256
        });
        let input_sha256 = canonical_json_hash(&request);
        request["input_sha256"] = Value::String(input_sha256);
        request
    }

    fn prepare_request(
        topology_request: &Value,
        preview_request: Value,
        preview_canonical_sha256: &str,
        idempotency_key: &str,
    ) -> Value {
        let mut request = json!({
            "schema_version":"AuthoringMeshEditPrepareRequest@1",
            "project_id":topology_request["project_id"],
            "source_candidate_id":topology_request["candidate_id"],
            "base_version_id":null,
            "preview_request":preview_request,
            "expected_preview_canonical_sha256":preview_canonical_sha256,
            "idempotency_key":idempotency_key,
            "max_response_bytes":1048576
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    #[test]
    fn authoring_topology_and_two_edit_previews_are_exact_bound_deterministic_and_read_only() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let (_prepared, request) = prepare_authoring_candidate(&runtime);
        let before = json!({
            "project":runtime.project(request["project_id"].as_str().unwrap()).unwrap(),
            "candidates":runtime.candidates(request["project_id"].as_str().unwrap()).unwrap(),
            "versions":runtime.versions(Some(request["project_id"].as_str().unwrap())).unwrap(),
            "cas":runtime.store.cas().list_objects().unwrap(),
        });

        let topology = get(&runtime, &request).expect("authoring topology");
        let base_topology_sha256 = topology["topology_sha256"].as_str().unwrap();
        assert_eq!(topology["schema_version"], "AuthoringTopology@1");
        assert_eq!(topology["counts"]["vertex_count"], 4);
        assert_eq!(topology["counts"]["edge_count"], 4);
        assert_eq!(topology["counts"]["loop_count"], 4);
        assert_eq!(topology["counts"]["face_count"], 1);
        assert_eq!(topology["vertices"][0]["element_id"], "v0");
        assert_eq!(topology["faces"][0]["element_id"], "f0");
        assert_self_hash(&topology);
        assert!(canonical_json_bytes(&topology).unwrap().len() < MAX_RESPONSE_BYTES);

        let translate = preview_request(
            &request,
            base_topology_sha256,
            json!({
                "operation":"translate_vertices",
                "vertex_ids":["v2","v3"],
                "delta_m":[0.0,0.0,0.25]
            }),
        );
        let translated = preview(&runtime, &translate).expect("translate preview");
        let translated_repeat = preview(&runtime, &translate).expect("translate repeat");
        assert_eq!(translated, translated_repeat);
        assert_eq!(translated["operation"], "translate_vertices");
        assert_eq!(
            translated["counts"]["before"],
            translated["counts"]["after"]
        );
        assert_ne!(
            translated["source_program_sha256"],
            translated["derived_program_sha256"]
        );
        assert_eq!(translated["runtime_write_performed"], false);
        assert_self_hash(&translated);

        let extrude = preview_request(
            &request,
            base_topology_sha256,
            json!({"operation":"single_face_extrude","face_id":"f0","distance_m":0.25}),
        );
        let extruded = preview(&runtime, &extrude).expect("single face extrude preview");
        let extruded_repeat = preview(&runtime, &extrude).expect("extrude repeat");
        assert_eq!(extruded, extruded_repeat);
        assert_eq!(extruded["operation"], "single_face_extrude");
        assert_eq!(extruded["counts"]["before"]["triangle_count"], 2);
        assert_eq!(extruded["counts"]["after"]["vertex_count"], 8);
        assert_eq!(extruded["counts"]["after"]["edge_count"], 12);
        assert_eq!(extruded["counts"]["after"]["loop_count"], 20);
        assert_eq!(extruded["counts"]["after"]["face_count"], 5);
        assert_eq!(extruded["counts"]["after"]["triangle_count"], 10);
        assert_eq!(
            extruded["edited_element_ids"]["generated_vertex_ids"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            extruded["edited_element_ids"]["generated_edge_ids"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert_eq!(
            extruded["edited_element_ids"]["generated_loop_ids"]
                .as_array()
                .unwrap()
                .len(),
            20
        );
        assert_eq!(
            extruded["edited_element_ids"]["generated_face_ids"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        assert_self_hash(&extruded);
        assert!(canonical_json_bytes(&extruded).unwrap().len() < MAX_RESPONSE_BYTES);

        let context = load_context(&runtime, &request).expect("authoring context");
        let mut tampered_evidence = runtime
            .store
            .get_geometry_candidate_evidence(request["candidate_id"].as_str().unwrap())
            .expect("evidence query")
            .expect("evidence");
        tampered_evidence.canonical_sha256 = "f".repeat(64);
        assert!(validate_durable_geometry_evidence(&tampered_evidence).is_err());

        let mut interior = context.parameters.clone();
        interior
            .get_mut("loops")
            .and_then(Value::as_array_mut)
            .expect("loops")
            .push(json!({
                "element_id":"neighbor-loop",
                "face_id":"neighbor-face",
                "ordinal":0,
                "vertex_id":"v0",
                "edge_id":"e01",
                "edge_forward":true
            }));
        let interior_error = apply_extrude(
            &mut interior,
            json!({"face_id":"f0","distance_m":0.1})
                .as_object()
                .unwrap(),
            "a",
        )
        .expect_err("interior edge must be rejected");
        assert!(interior_error.to_string().contains("boundary faces only"));

        let mut non_planar = context.parameters.clone();
        non_planar["vertices"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|vertex| vertex["element_id"] == "v3")
            .unwrap()["position_m"] = json!([-1.0, 1.0, 0.25]);
        let non_planar_error = apply_extrude(
            &mut non_planar,
            json!({"face_id":"f0","distance_m":0.1})
                .as_object()
                .unwrap(),
            "b",
        )
        .expect_err("non-planar quad must be rejected");
        assert!(non_planar_error.to_string().contains("planar quad"));

        let mut concave = context.parameters.clone();
        concave["vertices"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|vertex| vertex["element_id"] == "v2")
            .unwrap()["position_m"] = json!([0.0, 0.0, 0.0]);
        let concave_error = apply_extrude(
            &mut concave,
            json!({"face_id":"f0","distance_m":0.1})
                .as_object()
                .unwrap(),
            "c",
        )
        .expect_err("concave quad must be rejected");
        assert!(concave_error.to_string().contains("convex authored quad"));

        let mut unsorted = context.parameters.clone();
        let unsorted_error = apply_translate(
            &mut unsorted,
            json!({"vertex_ids":["v3","v2"],"delta_m":[0.0,0.0,0.1]})
                .as_object()
                .unwrap(),
        )
        .expect_err("unsorted IDs must be rejected");
        assert!(unsorted_error.to_string().contains("lexically sorted"));

        let after = json!({
            "project":runtime.project(request["project_id"].as_str().unwrap()).unwrap(),
            "candidates":runtime.candidates(request["project_id"].as_str().unwrap()).unwrap(),
            "versions":runtime.versions(Some(request["project_id"].as_str().unwrap())).unwrap(),
            "cas":runtime.store.cas().list_objects().unwrap(),
        });
        assert_eq!(after, before);

        let mut stale = request.clone();
        stale["program_sha256"] = json!("f".repeat(64));
        assert!(get(&runtime, &stale).is_err());
        let mut cross_project = request.clone();
        cross_project["project_id"] = json!("another-project");
        assert!(get(&runtime, &cross_project).is_err());
        let unknown_vertex = preview_request(
            &request,
            base_topology_sha256,
            json!({"operation":"translate_vertices","vertex_ids":["missing"],"delta_m":[0.0,0.0,0.1]}),
        );
        assert!(preview(&runtime, &unknown_vertex).is_err());
        let zero_delta = preview_request(
            &request,
            base_topology_sha256,
            json!({"operation":"translate_vertices","vertex_ids":["v0"],"delta_m":[0.0,0.0,0.0]}),
        );
        assert!(preview(&runtime, &zero_delta).is_err());
        let unknown_face = preview_request(
            &request,
            base_topology_sha256,
            json!({"operation":"single_face_extrude","face_id":"missing","distance_m":0.1}),
        );
        assert!(preview(&runtime, &unknown_face).is_err());
        let mut unknown_field = request;
        unknown_field["python"] = json!("print('no')");
        assert!(get(&runtime, &unknown_field).is_err());
    }

    #[test]
    fn authoring_mesh_edit_prepare_stages_exact_candidate_atomically_and_replays_idempotently() {
        if forgecad_contracts::build_cohort_sha256().is_none() {
            return;
        }
        let runtime = Runtime::ephemeral().expect("runtime");
        let (source_prepared, topology_request) = prepare_authoring_candidate(&runtime);
        let project_id = topology_request["project_id"].as_str().unwrap();
        let source_candidate_id = topology_request["candidate_id"].as_str().unwrap();
        let source_candidate_before = runtime
            .candidate(source_candidate_id)
            .expect("source candidate query")
            .expect("source candidate");
        let topology = get(&runtime, &topology_request).expect("topology");
        let edit_preview_request = preview_request(
            &topology_request,
            topology["topology_sha256"].as_str().unwrap(),
            json!({"operation":"single_face_extrude","face_id":"f0","distance_m":0.25}),
        );
        let edit_preview = preview(&runtime, &edit_preview_request).expect("preview");
        let request = prepare_request(
            &topology_request,
            edit_preview_request.clone(),
            edit_preview["canonical_sha256"].as_str().unwrap(),
            "authoring-edit-prepare-once",
        );
        let candidates_before = runtime.candidates(project_id).unwrap().len();
        let versions_before = runtime.versions(Some(project_id)).unwrap().len();

        let prepared = prepare(&runtime, &request).expect("authoring edit prepare");
        assert_eq!(prepared["schema_version"], "AuthoringMeshEditPrepare@1");
        assert_eq!(prepared["source_candidate_id"], source_candidate_id);
        assert_ne!(prepared["new_candidate_id"], source_candidate_id);
        assert_eq!(prepared["candidate"]["state"], "reviewable");
        assert_eq!(prepared["candidate"]["quality_hard_gate_passed"], true);
        assert_eq!(prepared["runtime_write_performed"], true);
        assert_eq!(prepared["version_status"], "no-version-created");
        assert_eq!(prepared["confirm_status"], "approval-required");
        assert_eq!(prepared["export_status"], "locked-until-confirm");
        assert_eq!(
            prepared["preview_canonical_sha256"],
            edit_preview["canonical_sha256"]
        );
        assert_eq!(
            prepared["derived_program_sha256"],
            edit_preview["derived_program_sha256"]
        );
        assert_eq!(
            prepared["source_artifact_sha256"],
            source_prepared["artifact"]["artifact_id"]
        );
        assert_self_hash(&prepared);
        assert!(canonical_json_bytes(&prepared).unwrap().len() <= MAX_RESPONSE_BYTES);

        let new_candidate_id = prepared["new_candidate_id"].as_str().unwrap();
        let staged = runtime
            .candidate(new_candidate_id)
            .expect("staged candidate query")
            .expect("staged candidate");
        assert_eq!(staged.state, "reviewable");
        assert!(staged.quality_hard_gate_passed);
        assert_eq!(
            staged.prepared_object_sha256.as_deref(),
            prepared["derived_artifact_sha256"].as_str()
        );
        let evidence = runtime
            .store
            .get_geometry_candidate_evidence(new_candidate_id)
            .expect("staged evidence query")
            .expect("staged evidence");
        assert_eq!(
            evidence.canonical_sha256,
            prepared["derived_geometry_candidate_evidence_sha256"]
        );
        let readback = runtime
            .artifact_readback(
                prepared["derived_artifact_sha256"].as_str().unwrap(),
                new_candidate_id,
            )
            .expect("derived readback");
        assert_eq!(
            readback["canonical_sha256"],
            prepared["derived_artifact_readback_sha256"]
        );
        assert_eq!(
            runtime.candidates(project_id).unwrap().len(),
            candidates_before + 1
        );
        assert_eq!(
            runtime.versions(Some(project_id)).unwrap().len(),
            versions_before
        );
        assert_eq!(
            runtime
                .candidate(source_candidate_id)
                .unwrap()
                .unwrap()
                .canonical_sha256,
            source_candidate_before.canonical_sha256
        );

        let cas_after_first = runtime.store.cas().list_objects().unwrap();
        let replay = prepare(&runtime, &request).expect("exact idempotent replay");
        assert_eq!(replay, prepared);
        assert_eq!(
            runtime.candidates(project_id).unwrap().len(),
            candidates_before + 1
        );
        assert_eq!(runtime.store.cas().list_objects().unwrap(), cas_after_first);

        let alternate_preview_request = preview_request(
            &topology_request,
            topology["topology_sha256"].as_str().unwrap(),
            json!({"operation":"single_face_extrude","face_id":"f0","distance_m":0.30}),
        );
        let alternate_preview =
            preview(&runtime, &alternate_preview_request).expect("alternate preview");
        let conflicting = prepare_request(
            &topology_request,
            alternate_preview_request,
            alternate_preview["canonical_sha256"].as_str().unwrap(),
            "authoring-edit-prepare-once",
        );
        let conflict_error = prepare(&runtime, &conflicting).expect_err("key reuse must fail");
        assert!(conflict_error
            .to_string()
            .contains("IDEMPOTENCY_KEY_REUSED"));
        assert_eq!(
            runtime.candidates(project_id).unwrap().len(),
            candidates_before + 1
        );
        assert_eq!(runtime.store.cas().list_objects().unwrap(), cas_after_first);

        let mut stale = request.clone();
        stale["base_version_id"] = json!("version-stale");
        stale.as_object_mut().unwrap().remove("input_sha256");
        stale["input_sha256"] = Value::String(canonical_json_hash(&stale));
        assert!(prepare(&runtime, &stale)
            .expect_err("stale head")
            .to_string()
            .contains("current project head"));
        let mut unknown = request;
        unknown["python"] = json!("bmesh.ops.extrude_face_region");
        assert!(prepare(&runtime, &unknown).is_err());
        assert_eq!(
            runtime.candidates(project_id).unwrap().len(),
            candidates_before + 1
        );
        assert_eq!(
            runtime.versions(Some(project_id)).unwrap().len(),
            versions_before
        );
        assert_eq!(runtime.store.cas().list_objects().unwrap(), cas_after_first);
    }
}
