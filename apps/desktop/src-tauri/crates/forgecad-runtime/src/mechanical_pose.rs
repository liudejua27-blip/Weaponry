//! Candidate-bound, read-only mechanical pose evaluation.
//!
//! This module is a product-owned clean-room evaluator. It intentionally does
//! not expose Blender armatures, skinning, IK, NLA, F-Curves or scripting, and
//! it never persists geometry or writes Runtime state. The explicit geometry
//! preview path may compile a transient derived GLB for hash/readback only.

use super::{
    canonical_json_bytes, canonical_json_hash, compile_geometry_with_runtime_worker,
    hash_geometry_program_with_runtime_worker, is_opaque_id, is_sha256, now_string, sha256_hex,
    strict_glb_inspection, validate_worker_metadata, Runtime, RuntimeError,
};
use forgecad_contracts::MechanicalAnimationClipLinkRecord;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const ERROR: &str = "MECHANICAL_POSE_INVALID";
const MAX_LINKS: usize = 64;
const MAX_DEPTH: usize = 16;
const MAX_CHANNELS: usize = 64;
const MAX_KEYS_PER_CHANNEL: usize = 32;
const MAX_TOTAL_KEYS: usize = 512;
const MAX_SEQUENCE_SAMPLES: usize = 16;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_CLIP_BYTES: usize = 1024 * 1024;
const MAX_VIEWER_INVENTORY_BYTES: usize = 128 * 1024;
const MAX_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;
const EPSILON: f64 = 1.0e-6;
const QUANTIZE_SCALE: f64 = 1.0e12;

#[derive(Clone)]
struct Link {
    link_id: String,
    part_id: String,
    source_node_ids: Vec<String>,
    joint_type: String,
    rest_translation: [f64; 3],
    rest_rotation: [f64; 4],
    axis: Option<[f64; 3]>,
    limit_min: Option<f64>,
    limit_max: Option<f64>,
    value_unit: String,
}

#[derive(Clone, Copy)]
struct Transform {
    translation: [f64; 3],
    rotation: [f64; 4],
}

pub(super) fn evaluate(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    match request.get("schema_version").and_then(Value::as_str) {
        Some("MechanicalPoseEvaluationRequest@1") => evaluate_single(runtime, request),
        Some("MechanicalPoseSequencePreviewRequest@1") => evaluate_sequence(runtime, request),
        _ => invalid(
            "schema_version must be MechanicalPoseEvaluationRequest@1 or MechanicalPoseSequencePreviewRequest@1",
        ),
    }
}

pub(super) fn geometry_preview(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let outer = exact_object(
        request,
        &[
            "schema_version",
            "pose_evaluation_request",
            "preview_policy",
            "input_sha256",
        ],
        "mechanical pose geometry preview request",
    )?;
    if text(outer, "schema_version")? != "MechanicalPoseGeometryPreviewRequest@1"
        || text(outer, "preview_policy")? != "transient-derived-program-worker-readback@1"
    {
        return invalid("mechanical pose geometry preview request policy differs");
    }
    let input_sha256 = sha(outer, "input_sha256")?;
    let mut input_preimage = request.clone();
    input_preimage
        .as_object_mut()
        .expect("preview request is an object")
        .remove("input_sha256");
    if canonical_json_hash(&input_preimage) != input_sha256 {
        return invalid("input_sha256 does not match the closed geometry preview request");
    }
    let pose_request = outer
        .get("pose_evaluation_request")
        .ok_or_else(|| error("pose_evaluation_request is required"))?;
    let pose_result = evaluate_single(runtime, pose_request)?;
    let pose = pose_result
        .as_object()
        .ok_or_else(|| error("pose evaluation result is invalid"))?;

    let rest_frame = pose
        .get("rest_frame")
        .and_then(Value::as_object)
        .ok_or_else(|| error("pose evaluation omitted rest_frame"))?;
    let links = rest_frame
        .get("links")
        .and_then(Value::as_array)
        .ok_or_else(|| error("rest_frame links are invalid"))?
        .iter()
        .map(parse_link)
        .map(|result| result.map(|link| (link.link_id.clone(), link)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let parents = rest_frame
        .get("parent_map")
        .and_then(Value::as_array)
        .ok_or_else(|| error("rest_frame parent_map is invalid"))?
        .iter()
        .map(|value| {
            let entry = exact_object(value, &["child_link_id", "parent_link_id"], "parent_map")?;
            Ok((
                identifier(entry, "child_link_id")?.to_owned(),
                identifier(entry, "parent_link_id")?.to_owned(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, RuntimeError>>()?;
    let order = pose
        .get("evaluation_order")
        .and_then(Value::as_array)
        .ok_or_else(|| error("evaluation_order is invalid"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| error("evaluation_order contains non-text"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rest_nodes = evaluate_nodes(&links, &parents, &order, &BTreeMap::new())?;
    let rest_by_part = transforms_by_part(&rest_nodes)?;
    let posed_by_part = transforms_by_part(
        pose.get("evaluated_nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| error("evaluated_nodes are invalid"))?,
    )?;

    let project_id = text(pose, "project_id")?;
    let candidate_id = text(pose, "candidate_id")?;
    let source_artifact_id = text(pose, "artifact_id")?;
    let source_program_sha256 = text(pose, "program_sha256")?;
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(candidate_id)?
        .ok_or_else(|| error("durable geometry evidence is unavailable"))?;
    if evidence.project_id != project_id
        || evidence.artifact_object_sha256 != source_artifact_id
        || evidence.geometry_program_sha256 != source_program_sha256
        || evidence.operator_catalog_sha256 != text(pose, "operator_catalog_sha256")?
        || evidence.readback_config_sha256 != text(pose, "readback_config_sha256")?
    {
        return invalid("durable geometry evidence differs from the pose cohort");
    }
    let source_glb = runtime.cas_read_bounded(source_artifact_id, MAX_ARTIFACT_BYTES as u64)?;
    let source_inspection = strict_glb_inspection(&source_glb)?;
    if !source_inspection.hard_gate_passed {
        return invalid("source artifact strict readback failed");
    }

    let program_bytes = runtime.cas_read_bounded(
        &evidence.geometry_program_object_sha256,
        MAX_CLIP_BYTES as u64,
    )?;
    if sha256_hex(&program_bytes) != evidence.geometry_program_object_sha256 {
        return invalid("persisted GeometryProgram CAS bytes differ");
    }
    let mut program: Value = serde_json::from_slice(&program_bytes)
        .map_err(|_| error("persisted GeometryProgram draft is not JSON"))?;
    let baseline_object = program
        .as_object()
        .ok_or_else(|| error("persisted GeometryProgram draft is invalid"))?;
    if baseline_object.contains_key("canonical_sha256") {
        return invalid("persisted GeometryProgram draft unexpectedly contains canonical_sha256");
    }
    if baseline_object.get("project_id").and_then(Value::as_str) != Some(project_id)
        || baseline_object
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(text(pose, "operator_catalog_sha256")?)
    {
        return invalid(
            "persisted GeometryProgram project or catalog differs from the pose cohort",
        );
    }
    let baseline_hash_result = hash_geometry_program_with_runtime_worker(&program)
        .map_err(|source| error(format!("baseline GeometryProgram hash failed: {source}")))?;
    if baseline_hash_result
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(source_program_sha256)
    {
        return invalid("fixed Worker baseline GeometryProgram hash differs from the pose cohort");
    }
    let program_object = program
        .as_object_mut()
        .expect("baseline GeometryProgram object validated");

    let node_values = program_object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| error("persisted GeometryProgram nodes are invalid"))?;
    let mut node_inputs = BTreeMap::<String, Vec<String>>::new();
    let mut existing_node_ids = BTreeSet::new();
    for node in node_values {
        let node = node
            .as_object()
            .ok_or_else(|| error("GeometryProgram node is invalid"))?;
        let node_id = identifier(node, "node_id")?.to_owned();
        let inputs = node
            .get("inputs")
            .and_then(Value::as_array)
            .ok_or_else(|| error("GeometryProgram node inputs are invalid"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| error("node input is invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !existing_node_ids.insert(node_id.clone()) {
            return invalid("GeometryProgram node IDs are not unique");
        }
        node_inputs.insert(node_id, inputs);
    }
    let mut source_owner = BTreeMap::new();
    for link in links.values() {
        for source_node_id in &link.source_node_ids {
            if source_owner
                .insert(source_node_id.clone(), link.part_id.clone())
                .is_some()
            {
                return invalid(
                    "a GeometryProgram source node may belong to only one mechanical Part",
                );
            }
        }
    }

    let output_values = program_object
        .get("part_outputs")
        .and_then(Value::as_array)
        .ok_or_else(|| error("persisted GeometryProgram part_outputs are invalid"))?
        .clone();
    let mut seen_parts = BTreeSet::new();
    let mut derived_nodes = Vec::new();
    let mut derived_outputs = Vec::new();
    let mut part_deltas = Vec::new();
    for (part_index, output) in output_values.iter().enumerate() {
        let mut output = output
            .as_object()
            .cloned()
            .ok_or_else(|| error("part_output is invalid"))?;
        let part_id = identifier(&output, "part_id")?.to_owned();
        if !seen_parts.insert(part_id.clone()) {
            return invalid("each Part must have exactly one final part_output");
        }
        let link = links
            .values()
            .find(|link| link.part_id == part_id)
            .ok_or_else(|| error("part_output has no mechanical link"))?;
        let rest = *rest_by_part
            .get(&part_id)
            .ok_or_else(|| error("rest world pose is unavailable"))?;
        let posed = *posed_by_part
            .get(&part_id)
            .ok_or_else(|| error("posed world pose is unavailable"))?;
        let delta = compose(posed, inverse(rest)?)?;
        let euler = quaternion_to_worker_euler(delta.rotation)?;
        ensure_worker_delta_translation_bounded(delta.translation)?;
        let inputs = output
            .get("input_node_ids")
            .and_then(Value::as_array)
            .ok_or_else(|| error("part_output input_node_ids are invalid"))?;
        let mut transformed = Vec::new();
        for (input_index, input) in inputs.iter().enumerate() {
            let input_id = input
                .as_str()
                .ok_or_else(|| error("part_output input is invalid"))?;
            let owners = reachable_source_owners(input_id, &node_inputs, &source_owner)?;
            if owners != BTreeSet::from([part_id.clone()]) {
                return invalid(
                    "part_output is shared across Parts or lacks a pure source binding",
                );
            }
            let node_id = format!("pose-delta-{part_index}-{input_index}");
            if existing_node_ids.contains(&node_id) {
                return invalid("derived pose node ID collides with the source program");
            }
            existing_node_ids.insert(node_id.clone());
            derived_nodes.push(json!({
                "node_id":node_id,
                "operator_id":"forgecad.geometry.transform@2",
                "inputs":[input_id],
                "parameters":{"shape":"transform","translation_m":delta.translation,"rotation_rad":euler,"scale":[1.0,1.0,1.0]}
            }));
            transformed.push(Value::String(node_id));
        }
        output.insert(
            "input_node_ids".to_owned(),
            Value::Array(transformed.clone()),
        );
        derived_outputs.push(Value::Object(output));
        part_deltas.push(json!({
            "part_id":part_id,
            "source_node_ids":link.source_node_ids,
            "rest_world_pose":transform_value(rest),
            "posed_world_pose":transform_value(posed),
            "delta_pose":transform_value(delta),
            "delta_rotation_euler_xyz_rad":euler,
            "transformed_output_node_ids":transformed
        }));
    }
    if seen_parts
        != source_inspection
            .part_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
    {
        return invalid("derived program does not cover every strict artifact Part exactly once");
    }
    part_deltas.sort_by(|left, right| left["part_id"].as_str().cmp(&right["part_id"].as_str()));
    let node_count = {
        let nodes = program_object
            .get_mut("nodes")
            .and_then(Value::as_array_mut)
            .expect("nodes validated");
        nodes.extend(derived_nodes);
        if nodes.len() > 512 {
            return invalid("derived pose program exceeds 512 nodes");
        }
        nodes.len()
    };
    program_object.insert("part_outputs".to_owned(), Value::Array(derived_outputs));
    let budgets = program_object
        .get_mut("budgets")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| error("GeometryProgram budgets are invalid"))?;
    budgets.insert("max_nodes".to_owned(), Value::from(node_count as u64));
    let _ = program_object;

    let hash_result = hash_geometry_program_with_runtime_worker(&program)
        .map_err(|source| error(format!("derived GeometryProgram hash failed: {source}")))?;
    let posed_program_sha256 = hash_result["canonical_sha256"]
        .as_str()
        .ok_or_else(|| error("Worker omitted derived program hash"))?
        .to_owned();
    program
        .as_object_mut()
        .expect("program object validated")
        .insert(
            "canonical_sha256".to_owned(),
            Value::String(posed_program_sha256.clone()),
        );
    let artifact = compile_geometry_with_runtime_worker(&program, None)
        .map_err(|source| error(format!("transient pose geometry compile failed: {source}")))?;
    let repeated_artifact =
        compile_geometry_with_runtime_worker(&program, None).map_err(|source| {
            error(format!(
                "transient pose geometry repeat compile failed: {source}"
            ))
        })?;
    if artifact.glb != repeated_artifact.glb
        || artifact.program_sha256 != repeated_artifact.program_sha256
        || artifact.part_ids != repeated_artifact.part_ids
        || artifact.triangle_count != repeated_artifact.triangle_count
        || artifact.material_zone_ids != repeated_artifact.material_zone_ids
        || artifact.uv_status != repeated_artifact.uv_status
        || artifact.tangent_status != repeated_artifact.tangent_status
        || artifact.build_cohort_sha256 != repeated_artifact.build_cohort_sha256
    {
        return invalid("transient pose Geometry Worker replay differs");
    }
    let inspection = strict_glb_inspection(&artifact.glb)?;
    validate_worker_metadata(&artifact, &inspection)?;
    if !inspection.hard_gate_passed
        || inspection.program_sha256 != posed_program_sha256
        || inspection.part_ids.iter().cloned().collect::<BTreeSet<_>>() != seen_parts
    {
        return invalid("transient pose artifact strict readback differs");
    }
    let application_policy = json!({
        "coordinate_system":"forgecad-rh-y-up-m@1",
        "baseline_domain":"persisted-geometry-program-part-output-world-mesh@1",
        "delta_formula":"posed-world-times-inverse-rest-world@1",
        "rotation_lowering":"quaternion-to-euler-xyz-rz-ry-rx-fail-near-gimbal@1",
        "worker_transform_order":"scale-then-x-y-z-rotation-then-translation@1",
        "rest_frame_provenance":"caller-authored-hash-bound-not-artifact-rig-provenance@1"
    });
    let mut result = json!({
        "schema_version":"MechanicalPoseGeometryPreview@1",
        "project_id":project_id,
        "candidate_id":candidate_id,
        "source_artifact_id":source_artifact_id,
        "source_artifact_readback_sha256":pose["artifact_readback_sha256"],
        "source_program_sha256":source_program_sha256,
        "operator_catalog_sha256":pose["operator_catalog_sha256"],
        "readback_config_sha256":pose["readback_config_sha256"],
        "input_sha256":input_sha256,
        "rest_frame_sha256":pose["rest_frame_sha256"],
        "pose_action_sha256":pose["pose_action_sha256"],
        "sample_time_ticks":pose["sample_time_ticks"],
        "evaluated_pose_sha256":pose["evaluated_pose_sha256"],
        "application_policy_sha256":canonical_json_hash(&application_policy),
        "application_policy":application_policy,
        "part_deltas_sha256":canonical_json_hash(&json!(part_deltas)),
        "part_deltas":part_deltas,
        "posed_geometry_program":program,
        "posed_program_sha256":posed_program_sha256,
        "transient_artifact":{
            "artifact_sha256":sha256_hex(&artifact.glb),
            "size_bytes":artifact.glb.len(),
            "program_sha256":artifact.program_sha256,
            "part_ids":artifact.part_ids,
            "triangle_count":artifact.triangle_count,
            "material_zone_ids":artifact.material_zone_ids,
            "uv_status":artifact.uv_status,
            "tangent_status":artifact.tangent_status,
            "strict_readback":{
                "validator_status":inspection.validator_status,
                "hard_gate_passed":inspection.hard_gate_passed,
                "invalid_index_count":inspection.invalid_index_count,
                "non_finite_count":inspection.non_finite_count,
                "degenerate_triangle_count":inspection.degenerate_triangle_count,
                "metadata_mismatch_count":inspection.metadata_mismatch_count,
                "part_coverage":inspection.part_coverage,
                "source_coverage":inspection.source_coverage,
                "material_zone_coverage":inspection.material_zone_coverage
            },
            "delivery":"hash-and-readback-only-no-cas-object"
        },
        "worker_replay":{
            "first_build_cohort_sha256":artifact.build_cohort_sha256,
            "repeat_build_cohort_sha256":repeated_artifact.build_cohort_sha256,
            "first_artifact_sha256":sha256_hex(&artifact.glb),
            "repeat_artifact_sha256":sha256_hex(&repeated_artifact.glb),
            "byte_exact":true,
            "metadata_exact":true
        },
        "geometry_materialization":"transient-worker-glb-not-persisted",
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "validator_status":"passed",
        "quality_status":"structural_only",
        "limitations":[
            "caller-authored-rest-frame-not-artifact-rig-provenance",
            "rigid-parts-only-no-skinning-or-deformation",
            "single-scalar-dof-per-link",
            "no-ik-constraints-nla-fcurves-or-drivers",
            "euler-lowering-fails-near-gimbal",
            "f32-worker-transform-no-quaternion-bit-exact-claim",
            "transient-hash-and-readback-only-no-cas-object",
            "candidate-version-and-user-data-not-modified",
            "not-blender-armature-animation-or-python-parity",
            "structural-readback-does-not-prove-visual-quality"
        ],
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    if canonical_json_bytes(&result)
        .map_err(|source| error(source.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return invalid("mechanical pose geometry preview exceeds 1 MiB");
    }
    Ok(result)
}

pub(super) fn animation_clip_prepare(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let outer = exact_object(
        request,
        &[
            "schema_version",
            "clip_id",
            "pose_sequence_request",
            "clip_policy",
            "input_sha256",
        ],
        "mechanical animation clip prepare request",
    )?;
    if text(outer, "schema_version")? != "MechanicalAnimationClipPrepareRequest@1"
        || text(outer, "clip_policy")? != "runtime-owned-immutable-cas-rigid-mechanical-action@1"
    {
        return invalid("mechanical animation clip prepare policy differs");
    }
    let clip_id = identifier(outer, "clip_id")?.to_owned();
    let input_sha256 = sha(outer, "input_sha256")?.to_owned();
    let mut request_preimage = request.clone();
    request_preimage
        .as_object_mut()
        .expect("clip request is an object")
        .remove("input_sha256");
    if canonical_json_hash(&request_preimage) != input_sha256 {
        return invalid("input_sha256 does not match the closed clip request");
    }
    let sequence_request = outer
        .get("pose_sequence_request")
        .ok_or_else(|| error("pose_sequence_request is required"))?;
    if sequence_request
        .get("pose_action_draft")
        .is_none_or(Value::is_null)
    {
        return invalid("a durable mechanical animation clip requires a non-null PoseAction");
    }
    let sequence = evaluate_sequence(runtime, sequence_request)?;
    let sequence_object = sequence
        .as_object()
        .ok_or_else(|| error("validated pose sequence is invalid"))?;
    let project_id = text(sequence_object, "project_id")?.to_owned();
    let candidate_id = text(sequence_object, "candidate_id")?.to_owned();
    let artifact_id = text(sequence_object, "artifact_id")?.to_owned();
    let artifact_readback_sha256 = text(sequence_object, "artifact_readback_sha256")?.to_owned();
    let program_sha256 = text(sequence_object, "program_sha256")?.to_owned();
    let operator_catalog_sha256 = text(sequence_object, "operator_catalog_sha256")?.to_owned();
    let readback_config_sha256 = text(sequence_object, "readback_config_sha256")?.to_owned();
    let rest_frame = sequence_object
        .get("rest_frame")
        .cloned()
        .ok_or_else(|| error("validated sequence omitted rest_frame"))?;
    let rest_frame_sha256 = text(sequence_object, "rest_frame_sha256")?.to_owned();
    let pose_action = sequence_object
        .get("pose_action")
        .cloned()
        .filter(|value| !value.is_null())
        .ok_or_else(|| error("validated sequence omitted PoseAction"))?;
    let pose_action_sha256 = text(sequence_object, "pose_action_sha256")?.to_owned();
    let sample_time_ticks = sequence_object
        .get("sample_time_ticks")
        .cloned()
        .ok_or_else(|| error("validated sequence omitted sample schedule"))?;

    let candidate = runtime
        .candidate(&candidate_id)?
        .ok_or_else(|| error("candidate is unavailable"))?;
    if candidate.project_id != project_id
        || candidate.prepared_object_sha256.as_deref() != Some(artifact_id.as_str())
    {
        return invalid("candidate binding differs before clip prepare");
    }
    let evidence = runtime
        .store
        .get_geometry_candidate_evidence(&candidate_id)?
        .ok_or_else(|| error("durable geometry evidence is unavailable"))?;
    if evidence.project_id != project_id
        || evidence.artifact_object_sha256 != artifact_id
        || evidence.geometry_program_sha256 != program_sha256
        || evidence.operator_catalog_sha256 != operator_catalog_sha256
        || evidence.readback_config_sha256 != readback_config_sha256
    {
        return invalid("durable geometry evidence differs from the clip cohort");
    }
    let source_glb = runtime.cas_read_bounded(&artifact_id, MAX_ARTIFACT_BYTES as u64)?;
    let program_bytes = runtime.cas_read_bounded(
        &evidence.geometry_program_object_sha256,
        MAX_CLIP_BYTES as u64,
    )?;
    let mut program: Value = serde_json::from_slice(&program_bytes)
        .map_err(|_| error("persisted GeometryProgram draft is not JSON"))?;
    let program_object = program
        .as_object_mut()
        .ok_or_else(|| error("persisted GeometryProgram draft is invalid"))?;
    if program_object.contains_key("canonical_sha256") {
        return invalid("persisted GeometryProgram draft unexpectedly contains canonical_sha256");
    }
    program_object.insert(
        "canonical_sha256".to_owned(),
        Value::String(program_sha256.clone()),
    );
    let first_source_replay = compile_geometry_with_runtime_worker(&program, None)
        .map_err(|source| error(format!("source geometry replay failed: {source}")))?;
    let repeat_source_replay = compile_geometry_with_runtime_worker(&program, None)
        .map_err(|source| error(format!("source geometry repeat replay failed: {source}")))?;
    if first_source_replay.glb != source_glb
        || repeat_source_replay.glb != source_glb
        || first_source_replay.glb != repeat_source_replay.glb
        || first_source_replay.program_sha256 != program_sha256
        || repeat_source_replay.program_sha256 != program_sha256
    {
        return invalid(
            "source Geometry Worker full-GLB replay differs from the candidate artifact",
        );
    }
    let source_replay_worker_cohort_sha256 = first_source_replay
        .build_cohort_sha256
        .clone()
        .filter(|value| is_sha256(value))
        .ok_or_else(|| error("source Geometry Worker cohort is unavailable"))?;
    if repeat_source_replay.build_cohort_sha256.as_deref()
        != Some(source_replay_worker_cohort_sha256.as_str())
    {
        return invalid("source Geometry Worker replay cohort differs");
    }
    let source_inspection = strict_glb_inspection(&source_glb)?;
    validate_worker_metadata(&first_source_replay, &source_inspection)?;
    validate_worker_metadata(&repeat_source_replay, &source_inspection)?;

    let sampling_policy = json!({
        "schema_version":"MechanicalAnimationSamplingPolicy@1",
        "timebase_hz":1000,
        "interpolation":"scalar-linear-integer-ticks-clamped",
        "unkeyed":"rest",
        "sample_time_ticks":sample_time_ticks,
        "max_samples":MAX_SEQUENCE_SAMPLES,
        "frame_preview_batch_size":1
    });
    let sampling_policy_sha256 = canonical_json_hash(&sampling_policy);
    let mut clip = json!({
        "schema_version":"MechanicalAnimationClip@1",
        "clip_id":clip_id,
        "project_id":project_id,
        "candidate_id":candidate_id,
        "artifact_id":artifact_id,
        "artifact_readback_sha256":artifact_readback_sha256,
        "geometry_candidate_evidence_sha256":evidence.canonical_sha256,
        "program_sha256":program_sha256,
        "operator_catalog_sha256":operator_catalog_sha256,
        "readback_config_sha256":readback_config_sha256,
        "request_sha256":input_sha256,
        "rest_frame":rest_frame,
        "rest_frame_sha256":rest_frame_sha256,
        "pose_action":pose_action,
        "pose_action_sha256":pose_action_sha256,
        "sampling_policy":sampling_policy,
        "sampling_policy_sha256":sampling_policy_sha256,
        "source_replay":{
            "worker_build_cohort_sha256":source_replay_worker_cohort_sha256,
            "first_artifact_sha256":sha256_hex(&first_source_replay.glb),
            "repeat_artifact_sha256":sha256_hex(&repeat_source_replay.glb),
            "byte_exact_with_candidate_artifact":true,
            "strict_readback_passed":true
        },
        "materialization_status":"runtime-owned-immutable-cas-clip",
        "quality_status":"structural_only",
        "limitations":[
            "caller-authored-rest-frame-not-artifact-rig-provenance",
            "rigid-parts-only-no-skinning-or-deformation",
            "single-scalar-dof-per-link",
            "linear-clamped-integer-tick-actions-only",
            "maximum-16-explicit-preview-samples",
            "no-ik-constraints-nla-fcurves-drivers-or-timeline",
            "no-glb-animation-channels-or-durable-frame-meshes",
            "not-blender-armature-animation-or-python-parity",
            "structural-replay-does-not-prove-visual-quality"
        ],
        "canonical_sha256":""
    });
    set_closed_canonical_sha256(&mut clip, "canonical_sha256")?;
    validate_animation_clip(&clip)?;
    let clip_bytes = canonical_json_bytes(&clip).map_err(|source| error(source.to_string()))?;
    if clip_bytes.is_empty() || clip_bytes.len() > MAX_CLIP_BYTES {
        return invalid("canonical mechanical animation clip exceeds 1 MiB");
    }
    let clip_object = runtime.put_object(
        &clip_bytes,
        None,
        "application/json",
        "mechanical-animation-clip",
    )?;
    let link = animation_clip_link_value(
        &clip,
        &clip_object.record.sha256,
        &source_replay_worker_cohort_sha256,
    )?;
    let record = animation_clip_record_from_value(&link, &now_string())?;
    if let Err(commit_error) = runtime.store.record_mechanical_animation_clip_link(&record) {
        if let Err(rollback_error) = runtime
            .store
            .discard_new_temporary_mechanical_animation_clip(&clip_object)
        {
            return Err(error(format!(
                "clip link commit failed ({commit_error}); temporary CAS rollback also failed ({rollback_error})"
            )));
        }
        return Err(commit_error.into());
    }
    load_animation_clip_link(runtime, &record)
}

pub(super) fn animation_clip_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let (project_id, candidate_id, clip_id) = validate_animation_clip_lookup_request(
        request,
        "MechanicalAnimationClipGetRequest@1",
        false,
    )?;
    let record = runtime
        .store
        .get_mechanical_animation_clip_link(&candidate_id, &clip_id)?
        .ok_or_else(|| error("durable mechanical animation clip is unavailable"))?;
    if record.project_id != project_id {
        return invalid("mechanical animation clip belongs to another project");
    }
    load_animation_clip_link(runtime, &record)
}

/// Return a compact, candidate-bound inventory for the read-only Viewer. The
/// complete immutable clip remains behind `animation_clip_get`; this method
/// intentionally exposes no pose evaluation or write operation.
pub(super) fn animation_clip_inventory(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "artifact_id",
            "max_clips",
            "canonical_sha256",
        ],
        "mechanical animation clip inventory request",
    )?;
    if text(object, "schema_version")? != "MechanicalAnimationClipInventoryRequest@1"
        || object.get("max_clips").and_then(Value::as_u64) != Some(16)
    {
        return invalid("mechanical animation clip inventory request policy differs");
    }
    verify_closed_canonical(request, "canonical_sha256")?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let candidate_id = identifier(object, "candidate_id")?.to_owned();
    let artifact_id = sha(object, "artifact_id")?.to_owned();
    runtime.ensure_candidate_artifact_binding(&candidate_id, &artifact_id)?;
    let candidate = runtime
        .candidate(&candidate_id)?
        .ok_or_else(|| error("candidate is unavailable"))?;
    if candidate.project_id != project_id {
        return invalid("mechanical animation clip inventory belongs to another project");
    }
    let records = runtime
        .store
        .list_mechanical_animation_clip_links(&candidate_id)?;
    let mut clips = Vec::with_capacity(records.len());
    for record in records {
        if record.project_id != project_id || record.artifact_id != artifact_id {
            return invalid("mechanical animation clip inventory binding differs");
        }
        clips.push(json!({
            "clip_id":record.clip_id,
            "clip_object_sha256":record.clip_object_sha256,
            "clip_sha256":record.clip_sha256,
            "rest_frame_sha256":record.rest_frame_sha256,
            "pose_action_sha256":record.pose_action_sha256,
            "source_replay_worker_cohort_sha256":record.source_replay_worker_cohort_sha256,
            "materialization_status":record.materialization_status,
            "created_at":record.created_at,
        }));
    }
    let clip_count = clips.len();
    let mut response = json!({
        "schema_version":"ViewerMechanicalAnimationInventory@1",
        "status":"Ready",
        "read_only":true,
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "project_id":project_id,
        "candidate_id":candidate_id,
        "artifact_id":artifact_id,
        "clip_count":clip_count,
        "max_clips":16,
        "clips":clips,
        "quality_status":"structural_only",
        "limitations":[
            "caller-authored-rigid-links-only",
            "no-armature-skinning-ik-nla-fcurves-drivers-or-editing",
            "no-viewer-mesh-motion-without-link-part-transform-evidence",
            "structural-evidence-does-not-prove-visual-quality"
        ],
        "canonical_sha256":""
    });
    set_closed_canonical_sha256(&mut response, "canonical_sha256")?;
    if canonical_json_bytes(&response)
        .map_err(|source| error(source.to_string()))?
        .len()
        > MAX_VIEWER_INVENTORY_BYTES
    {
        return invalid("mechanical animation clip inventory exceeds 128 KiB");
    }
    Ok(response)
}

pub(super) fn animation_clip_preview_get(
    runtime: &Runtime,
    request: &Value,
) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "clip_id",
            "sample_time_ticks",
            "preview_policy",
            "canonical_sha256",
        ],
        "mechanical animation clip preview request",
    )?;
    if text(object, "schema_version")? != "MechanicalAnimationClipPreviewRequest@1"
        || text(object, "preview_policy")? != "single-tick-transient-double-worker-replay@1"
    {
        return invalid("mechanical animation clip preview policy differs");
    }
    verify_closed_canonical(request, "canonical_sha256")?;
    let project_id = identifier(object, "project_id")?.to_owned();
    let candidate_id = identifier(object, "candidate_id")?.to_owned();
    let clip_id = identifier(object, "clip_id")?.to_owned();
    let sample_time_ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .filter(|tick| *tick <= 1_000_000)
        .ok_or_else(|| error("sample_time_ticks is outside the bounded integer range"))?;
    let record = runtime
        .store
        .get_mechanical_animation_clip_link(&candidate_id, &clip_id)?
        .ok_or_else(|| error("durable mechanical animation clip is unavailable"))?;
    if record.project_id != project_id {
        return invalid("mechanical animation clip belongs to another project");
    }
    let link = load_animation_clip_link(runtime, &record)?;
    let clip = link
        .get("clip")
        .and_then(Value::as_object)
        .ok_or_else(|| error("durable mechanical animation clip is invalid"))?;
    let scheduled = clip["sampling_policy"]["sample_time_ticks"]
        .as_array()
        .ok_or_else(|| error("clip sample schedule is invalid"))?;
    if !scheduled
        .iter()
        .any(|value| value.as_u64() == Some(sample_time_ticks))
    {
        return invalid("sample_time_ticks is not present in the immutable clip schedule");
    }
    let rest_frame_draft = rest_frame_draft_from_normalized(&clip["rest_frame"])?;
    let pose_action_draft = action_draft_from_normalized(&clip["pose_action"])?;
    let mut pose_request = json!({
        "schema_version":"MechanicalPoseEvaluationRequest@1",
        "project_id":project_id,
        "artifact_id":record.artifact_id,
        "candidate_id":candidate_id,
        "artifact_readback_sha256":record.artifact_readback_sha256,
        "program_sha256":record.program_sha256,
        "operator_catalog_sha256":record.operator_catalog_sha256,
        "readback_config_sha256":record.readback_config_sha256,
        "rest_frame_draft":rest_frame_draft,
        "pose_action_draft":pose_action_draft,
        "sample_time_ticks":sample_time_ticks,
        "input_sha256":""
    });
    set_input_sha256(&mut pose_request)?;
    let mut preview_request = json!({
        "schema_version":"MechanicalPoseGeometryPreviewRequest@1",
        "pose_evaluation_request":pose_request,
        "preview_policy":"transient-derived-program-worker-readback@1",
        "input_sha256":""
    });
    set_input_sha256(&mut preview_request)?;
    let pose_geometry_preview = geometry_preview(runtime, &preview_request)?;
    let replay = pose_geometry_preview
        .get("worker_replay")
        .and_then(Value::as_object)
        .ok_or_else(|| error("pose geometry preview omitted Worker replay"))?;
    if replay
        .get("first_build_cohort_sha256")
        .and_then(Value::as_str)
        != Some(record.source_replay_worker_cohort_sha256.as_str())
        || replay
            .get("repeat_build_cohort_sha256")
            .and_then(Value::as_str)
            != Some(record.source_replay_worker_cohort_sha256.as_str())
    {
        return invalid("frame preview Geometry Worker cohort differs from the clip");
    }
    let frame_identity = json!({
        "schema_version":"MechanicalAnimationFrameIdentity@1",
        "clip_sha256":record.clip_sha256,
        "sample_time_ticks":sample_time_ticks,
        "evaluated_pose_sha256":pose_geometry_preview["evaluated_pose_sha256"],
        "posed_program_sha256":pose_geometry_preview["posed_program_sha256"],
        "transient_artifact_sha256":pose_geometry_preview["transient_artifact"]["artifact_sha256"],
        "worker_build_cohort_sha256":record.source_replay_worker_cohort_sha256
    });
    let mut result = json!({
        "schema_version":"MechanicalAnimationClipPreview@1",
        "project_id":record.project_id,
        "candidate_id":record.candidate_id,
        "artifact_id":record.artifact_id,
        "clip_id":record.clip_id,
        "clip_object_sha256":record.clip_object_sha256,
        "clip_sha256":record.clip_sha256,
        "rest_frame_sha256":record.rest_frame_sha256,
        "pose_action_sha256":record.pose_action_sha256,
        "sample_time_ticks":sample_time_ticks,
        "frame_sha256":canonical_json_hash(&frame_identity),
        "source_replay_worker_cohort_sha256":record.source_replay_worker_cohort_sha256,
        "pose_geometry_preview":pose_geometry_preview,
        "geometry_materialization":"transient-double-worker-glb-not-persisted",
        "runtime_write_performed":false,
        "persistent_user_data_touched":false,
        "quality_status":"structural_only",
        "limitations":[
            "rigid-parts-only-no-skinning-or-deformation",
            "single-scheduled-tick-per-preview-call",
            "transient-frame-glb-not-persisted",
            "no-ik-constraints-nla-fcurves-drivers-or-timeline",
            "not-blender-armature-animation-or-python-parity",
            "structural-replay-does-not-prove-visual-quality"
        ],
        "canonical_sha256":""
    });
    set_closed_canonical_sha256(&mut result, "canonical_sha256")?;
    if canonical_json_bytes(&result)
        .map_err(|source| error(source.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return invalid("mechanical animation clip preview exceeds 1 MiB");
    }
    Ok(result)
}

fn validate_animation_clip(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "clip_id",
            "project_id",
            "candidate_id",
            "artifact_id",
            "artifact_readback_sha256",
            "geometry_candidate_evidence_sha256",
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "request_sha256",
            "rest_frame",
            "rest_frame_sha256",
            "pose_action",
            "pose_action_sha256",
            "sampling_policy",
            "sampling_policy_sha256",
            "source_replay",
            "materialization_status",
            "quality_status",
            "limitations",
            "canonical_sha256",
        ],
        "mechanical animation clip",
    )?;
    if text(object, "schema_version")? != "MechanicalAnimationClip@1"
        || text(object, "materialization_status")? != "runtime-owned-immutable-cas-clip"
        || text(object, "quality_status")? != "structural_only"
    {
        return invalid("mechanical animation clip policy differs");
    }
    for field in [
        "artifact_id",
        "artifact_readback_sha256",
        "geometry_candidate_evidence_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "request_sha256",
        "rest_frame_sha256",
        "pose_action_sha256",
        "sampling_policy_sha256",
        "canonical_sha256",
    ] {
        sha(object, field)?;
    }
    identifier(object, "clip_id")?;
    identifier(object, "project_id")?;
    identifier(object, "candidate_id")?;
    verify_closed_canonical(value, "canonical_sha256")?;
    let rest_frame = object
        .get("rest_frame")
        .ok_or_else(|| error("clip rest_frame is required"))?;
    if rest_frame.get("canonical_sha256").and_then(Value::as_str)
        != object.get("rest_frame_sha256").and_then(Value::as_str)
    {
        return invalid("clip rest_frame hash binding differs");
    }
    let pose_action = object
        .get("pose_action")
        .ok_or_else(|| error("clip pose_action is required"))?;
    if pose_action.get("canonical_sha256").and_then(Value::as_str)
        != object.get("pose_action_sha256").and_then(Value::as_str)
    {
        return invalid("clip PoseAction hash binding differs");
    }
    let sampling = exact_object(
        object
            .get("sampling_policy")
            .ok_or_else(|| error("clip sampling_policy is required"))?,
        &[
            "schema_version",
            "timebase_hz",
            "interpolation",
            "unkeyed",
            "sample_time_ticks",
            "max_samples",
            "frame_preview_batch_size",
        ],
        "mechanical animation sampling policy",
    )?;
    if text(sampling, "schema_version")? != "MechanicalAnimationSamplingPolicy@1"
        || sampling.get("timebase_hz").and_then(Value::as_u64) != Some(1000)
        || text(sampling, "interpolation")? != "scalar-linear-integer-ticks-clamped"
        || text(sampling, "unkeyed")? != "rest"
        || sampling.get("max_samples").and_then(Value::as_u64) != Some(MAX_SEQUENCE_SAMPLES as u64)
        || sampling
            .get("frame_preview_batch_size")
            .and_then(Value::as_u64)
            != Some(1)
        || canonical_json_hash(&Value::Object(sampling.clone()))
            != text(object, "sampling_policy_sha256")?
    {
        return invalid("mechanical animation sampling policy differs");
    }
    let ticks = sampling
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= MAX_SEQUENCE_SAMPLES)
        .ok_or_else(|| error("clip sample schedule must contain 1..16 ticks"))?;
    let duration = pose_action
        .get("duration_ticks")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("clip PoseAction duration is invalid"))?;
    let mut prior = None;
    for value in ticks {
        let tick = value
            .as_u64()
            .filter(|tick| *tick <= duration)
            .ok_or_else(|| error("clip sample tick exceeds PoseAction duration"))?;
        if prior.is_some_and(|previous| tick <= previous) {
            return invalid("clip sample schedule must be strictly increasing");
        }
        prior = Some(tick);
    }
    let source_replay = exact_object(
        object
            .get("source_replay")
            .ok_or_else(|| error("source_replay is required"))?,
        &[
            "worker_build_cohort_sha256",
            "first_artifact_sha256",
            "repeat_artifact_sha256",
            "byte_exact_with_candidate_artifact",
            "strict_readback_passed",
        ],
        "mechanical animation source replay",
    )?;
    let cohort = sha(source_replay, "worker_build_cohort_sha256")?;
    if sha(source_replay, "first_artifact_sha256")? != text(object, "artifact_id")?
        || sha(source_replay, "repeat_artifact_sha256")? != text(object, "artifact_id")?
        || source_replay
            .get("byte_exact_with_candidate_artifact")
            .and_then(Value::as_bool)
            != Some(true)
        || source_replay
            .get("strict_readback_passed")
            .and_then(Value::as_bool)
            != Some(true)
        || !is_sha256(cohort)
    {
        return invalid("mechanical animation source replay binding differs");
    }
    Ok(())
}

fn animation_clip_link_value(
    clip: &Value,
    clip_object_sha256: &str,
    source_replay_worker_cohort_sha256: &str,
) -> Result<Value, RuntimeError> {
    validate_animation_clip(clip)?;
    let mut link = json!({
        "schema_version":"MechanicalAnimationClipLink@1",
        "project_id":clip["project_id"],
        "candidate_id":clip["candidate_id"],
        "artifact_id":clip["artifact_id"],
        "artifact_readback_sha256":clip["artifact_readback_sha256"],
        "geometry_candidate_evidence_sha256":clip["geometry_candidate_evidence_sha256"],
        "program_sha256":clip["program_sha256"],
        "operator_catalog_sha256":clip["operator_catalog_sha256"],
        "readback_config_sha256":clip["readback_config_sha256"],
        "clip_id":clip["clip_id"],
        "request_sha256":clip["request_sha256"],
        "clip_object_sha256":clip_object_sha256,
        "clip_sha256":clip["canonical_sha256"],
        "rest_frame_sha256":clip["rest_frame_sha256"],
        "pose_action_sha256":clip["pose_action_sha256"],
        "source_replay_worker_cohort_sha256":source_replay_worker_cohort_sha256,
        "materialization_status":"runtime-owned-immutable-cas-clip",
        "clip":clip,
        "canonical_sha256":""
    });
    set_closed_canonical_sha256(&mut link, "canonical_sha256")?;
    Ok(link)
}

fn animation_clip_record_from_value(
    link: &Value,
    created_at: &str,
) -> Result<MechanicalAnimationClipLinkRecord, RuntimeError> {
    let object = link
        .as_object()
        .ok_or_else(|| error("mechanical animation clip link is invalid"))?;
    Ok(MechanicalAnimationClipLinkRecord {
        schema_version: text(object, "schema_version")?.to_owned(),
        project_id: identifier(object, "project_id")?.to_owned(),
        candidate_id: identifier(object, "candidate_id")?.to_owned(),
        artifact_id: sha(object, "artifact_id")?.to_owned(),
        artifact_readback_sha256: sha(object, "artifact_readback_sha256")?.to_owned(),
        geometry_candidate_evidence_sha256: sha(object, "geometry_candidate_evidence_sha256")?
            .to_owned(),
        program_sha256: sha(object, "program_sha256")?.to_owned(),
        operator_catalog_sha256: sha(object, "operator_catalog_sha256")?.to_owned(),
        readback_config_sha256: sha(object, "readback_config_sha256")?.to_owned(),
        clip_id: identifier(object, "clip_id")?.to_owned(),
        request_sha256: sha(object, "request_sha256")?.to_owned(),
        clip_object_sha256: sha(object, "clip_object_sha256")?.to_owned(),
        clip_sha256: sha(object, "clip_sha256")?.to_owned(),
        rest_frame_sha256: sha(object, "rest_frame_sha256")?.to_owned(),
        pose_action_sha256: sha(object, "pose_action_sha256")?.to_owned(),
        source_replay_worker_cohort_sha256: sha(object, "source_replay_worker_cohort_sha256")?
            .to_owned(),
        materialization_status: text(object, "materialization_status")?.to_owned(),
        canonical_sha256: sha(object, "canonical_sha256")?.to_owned(),
        created_at: created_at.to_owned(),
    })
}

pub(crate) fn load_animation_clip_link(
    runtime: &Runtime,
    record: &MechanicalAnimationClipLinkRecord,
) -> Result<Value, RuntimeError> {
    let clip_bytes = runtime.cas_read_bounded(&record.clip_object_sha256, MAX_CLIP_BYTES as u64)?;
    let clip: Value = serde_json::from_slice(&clip_bytes)
        .map_err(|_| error("persisted mechanical animation clip is not JSON"))?;
    validate_animation_clip(&clip)?;
    if canonical_json_bytes(&clip).map_err(|source| error(source.to_string()))? != clip_bytes
        || clip.get("canonical_sha256").and_then(Value::as_str) != Some(record.clip_sha256.as_str())
        || clip.get("project_id").and_then(Value::as_str) != Some(record.project_id.as_str())
        || clip.get("candidate_id").and_then(Value::as_str) != Some(record.candidate_id.as_str())
        || clip.get("artifact_id").and_then(Value::as_str) != Some(record.artifact_id.as_str())
        || clip.get("request_sha256").and_then(Value::as_str)
            != Some(record.request_sha256.as_str())
        || clip.get("artifact_readback_sha256").and_then(Value::as_str)
            != Some(record.artifact_readback_sha256.as_str())
        || clip
            .get("geometry_candidate_evidence_sha256")
            .and_then(Value::as_str)
            != Some(record.geometry_candidate_evidence_sha256.as_str())
        || clip.get("program_sha256").and_then(Value::as_str)
            != Some(record.program_sha256.as_str())
        || clip.get("operator_catalog_sha256").and_then(Value::as_str)
            != Some(record.operator_catalog_sha256.as_str())
        || clip.get("readback_config_sha256").and_then(Value::as_str)
            != Some(record.readback_config_sha256.as_str())
        || clip.get("rest_frame_sha256").and_then(Value::as_str)
            != Some(record.rest_frame_sha256.as_str())
        || clip.get("pose_action_sha256").and_then(Value::as_str)
            != Some(record.pose_action_sha256.as_str())
        || clip["source_replay"]["worker_build_cohort_sha256"].as_str()
            != Some(record.source_replay_worker_cohort_sha256.as_str())
    {
        return invalid("persisted mechanical animation clip differs from its durable link");
    }
    let first_tick = clip["sampling_policy"]["sample_time_ticks"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_u64)
        .ok_or_else(|| error("persisted clip schedule is invalid"))?;
    let rest_frame_draft = rest_frame_draft_from_normalized(&clip["rest_frame"])?;
    let pose_action_draft = action_draft_from_normalized(&clip["pose_action"])?;
    let mut semantic_request = json!({
        "schema_version":"MechanicalPoseEvaluationRequest@1",
        "project_id":record.project_id,
        "artifact_id":record.artifact_id,
        "candidate_id":record.candidate_id,
        "artifact_readback_sha256":record.artifact_readback_sha256,
        "program_sha256":record.program_sha256,
        "operator_catalog_sha256":record.operator_catalog_sha256,
        "readback_config_sha256":record.readback_config_sha256,
        "rest_frame_draft":rest_frame_draft,
        "pose_action_draft":pose_action_draft,
        "sample_time_ticks":first_tick,
        "input_sha256":""
    });
    set_input_sha256(&mut semantic_request)?;
    let semantic_result = evaluate_single(runtime, &semantic_request)?;
    if semantic_result
        .get("rest_frame_sha256")
        .and_then(Value::as_str)
        != Some(record.rest_frame_sha256.as_str())
        || semantic_result
            .get("pose_action_sha256")
            .and_then(Value::as_str)
            != Some(record.pose_action_sha256.as_str())
    {
        return invalid("persisted clip semantic replay differs");
    }
    let link = animation_clip_link_value(
        &clip,
        &record.clip_object_sha256,
        &record.source_replay_worker_cohort_sha256,
    )?;
    if link.get("canonical_sha256").and_then(Value::as_str)
        != Some(record.canonical_sha256.as_str())
    {
        return invalid("persisted mechanical animation clip link hash differs");
    }
    if canonical_json_bytes(&link)
        .map_err(|source| error(source.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return invalid("mechanical animation clip link exceeds 1 MiB");
    }
    Ok(link)
}

fn validate_animation_clip_lookup_request(
    request: &Value,
    schema_version: &str,
    _allow_preview_fields: bool,
) -> Result<(String, String, String), RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "candidate_id",
            "clip_id",
            "canonical_sha256",
        ],
        "mechanical animation clip lookup request",
    )?;
    if text(object, "schema_version")? != schema_version {
        return invalid("mechanical animation clip lookup schema differs");
    }
    verify_closed_canonical(request, "canonical_sha256")?;
    Ok((
        identifier(object, "project_id")?.to_owned(),
        identifier(object, "candidate_id")?.to_owned(),
        identifier(object, "clip_id")?.to_owned(),
    ))
}

fn verify_closed_canonical(value: &Value, field: &str) -> Result<(), RuntimeError> {
    let expected = value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| error(format!("{field} is invalid")))?;
    let mut preimage = value.clone();
    preimage
        .as_object_mut()
        .ok_or_else(|| error("canonical value is not an object"))?
        .remove(field);
    if canonical_json_hash(&preimage) != expected {
        return invalid(&format!("{field} differs from the closed request"));
    }
    Ok(())
}

fn set_closed_canonical_sha256(value: &mut Value, field: &str) -> Result<(), RuntimeError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| error("canonical value is not an object"))?;
    object.remove(field);
    let hash = canonical_json_hash(value);
    value[field] = Value::String(hash);
    Ok(())
}

fn set_input_sha256(value: &mut Value) -> Result<(), RuntimeError> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| error("hashed request is not an object"))?;
    object.remove("input_sha256");
    let hash = canonical_json_hash(value);
    value["input_sha256"] = Value::String(hash);
    Ok(())
}

fn rest_frame_draft_from_normalized(value: &Value) -> Result<Value, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| error("normalized RestFrame is invalid"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("MechanicalRestFrame@1") {
        return invalid("normalized RestFrame schema differs");
    }
    Ok(json!({
        "schema_version":"MechanicalRestFrameDraft@1",
        "rest_frame_id":object["rest_frame_id"],
        "coordinate_system":object["coordinate_system"],
        "transform_convention":object["transform_convention"],
        "root_link_id":object["root_link_id"],
        "links":object["links"],
        "parent_map":object["parent_map"]
    }))
}

fn action_draft_from_normalized(value: &Value) -> Result<Value, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| error("normalized PoseAction is invalid"))?;
    if object.get("schema_version").and_then(Value::as_str) != Some("MechanicalPoseAction@1") {
        return invalid("normalized PoseAction schema differs");
    }
    Ok(json!({
        "schema_version":"MechanicalPoseActionDraft@1",
        "action_id":object["action_id"],
        "timebase_hz":object["timebase_hz"],
        "duration_ticks":object["duration_ticks"],
        "interpolation":object["interpolation"],
        "extrapolation":object["extrapolation"],
        "unkeyed_policy":object["unkeyed_policy"],
        "channels":object["channels"]
    }))
}

fn evaluate_single(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "artifact_id",
            "candidate_id",
            "artifact_readback_sha256",
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "rest_frame_draft",
            "pose_action_draft",
            "sample_time_ticks",
            "input_sha256",
        ],
        "request",
    )?;
    if text(object, "schema_version")? != "MechanicalPoseEvaluationRequest@1" {
        return invalid("schema_version must be MechanicalPoseEvaluationRequest@1");
    }
    let project_id = identifier(object, "project_id")?;
    let artifact_id = sha(object, "artifact_id")?;
    let candidate_id = identifier(object, "candidate_id")?;
    let artifact_readback_sha256 = sha(object, "artifact_readback_sha256")?;
    let program_sha256 = sha(object, "program_sha256")?;
    let operator_catalog_sha256 = sha(object, "operator_catalog_sha256")?;
    let readback_config_sha256 = sha(object, "readback_config_sha256")?;
    let input_sha256 = sha(object, "input_sha256")?;
    let mut input_preimage = Value::Object(object.clone());
    input_preimage
        .as_object_mut()
        .expect("request is an object")
        .remove("input_sha256");
    if canonical_json_hash(&input_preimage) != input_sha256 {
        return invalid("input_sha256 does not match the closed request");
    }
    let sample_time_ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("sample_time_ticks must be a non-negative integer"))?;

    runtime.ensure_candidate_artifact_binding(candidate_id, artifact_id)?;
    let candidate = runtime
        .candidate(candidate_id)?
        .ok_or_else(|| error("candidate is unavailable"))?;
    if candidate.project_id != project_id {
        return invalid("candidate belongs to another project");
    }
    let readback =
        runtime.artifact_readback_bounded(artifact_id, candidate_id, MAX_ARTIFACT_BYTES as u64)?;
    if readback.get("schema_version").and_then(Value::as_str) != Some("ArtifactReadback@2")
        || readback.get("validator_status").and_then(Value::as_str) != Some("passed")
        || readback.get("hard_gate_passed").and_then(Value::as_bool) != Some(true)
    {
        return invalid("strict ArtifactReadback@2 must pass before pose evaluation");
    }
    for (field, expected) in [
        ("canonical_sha256", artifact_readback_sha256),
        ("program_sha256", program_sha256),
        ("operator_catalog_sha256", operator_catalog_sha256),
        ("readback_config_sha256", readback_config_sha256),
    ] {
        if readback.get(field).and_then(Value::as_str) != Some(expected) {
            return invalid(&format!("{field} binding differs from ArtifactReadback@2"));
        }
    }
    let glb = runtime.cas_read_bounded(artifact_id, MAX_ARTIFACT_BYTES as u64)?;
    let inspection = strict_glb_inspection(&glb)?;
    if !inspection.hard_gate_passed
        || inspection.program_sha256 != program_sha256
        || inspection.operator_catalog_sha256.as_deref() != Some(operator_catalog_sha256)
        || inspection.readback_config_sha256 != readback_config_sha256
    {
        return invalid("Runtime GLB inspection differs from the requested pose cohort");
    }
    if inspection.part_ids.is_empty() || inspection.part_ids.len() > MAX_LINKS {
        return invalid("artifact Part count must be 1..64 for mechanical pose v1");
    }

    let rest_draft = exact_object(
        object
            .get("rest_frame_draft")
            .ok_or_else(|| error("rest_frame_draft is required"))?,
        &[
            "schema_version",
            "rest_frame_id",
            "coordinate_system",
            "transform_convention",
            "root_link_id",
            "links",
            "parent_map",
        ],
        "rest_frame_draft",
    )?;
    if text(rest_draft, "schema_version")? != "MechanicalRestFrameDraft@1"
        || text(rest_draft, "coordinate_system")? != "forgecad-rh-y-up-m@1"
        || text(rest_draft, "transform_convention")? != "column-vector-trs-quaternion@1"
    {
        return invalid("rest-frame coordinate or transform policy is unsupported");
    }
    let rest_frame_id = identifier(rest_draft, "rest_frame_id")?;
    let root_link_id = identifier(rest_draft, "root_link_id")?;
    let link_values = rest_draft
        .get("links")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= MAX_LINKS)
        .ok_or_else(|| error("links must contain 1..64 items"))?;
    let mut links = BTreeMap::<String, Link>::new();
    let mut part_ids = BTreeSet::new();
    for value in link_values {
        let link = parse_link(value)?;
        if links.insert(link.link_id.clone(), link.clone()).is_some() {
            return invalid("link_id values must be unique");
        }
        if !part_ids.insert(link.part_id.clone()) {
            return invalid("each artifact Part must map to exactly one link");
        }
        let expected_sources = inspection
            .part_bindings
            .iter()
            .filter(|binding| binding.part_id == link.part_id)
            .map(|binding| binding.source_node_id.clone())
            .collect::<BTreeSet<_>>();
        let actual_sources = link
            .source_node_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if expected_sources.is_empty() || expected_sources != actual_sources {
            return invalid("link source_node_ids differ from strict GLB lineage");
        }
    }
    let artifact_parts = inspection.part_ids.iter().cloned().collect::<BTreeSet<_>>();
    if part_ids != artifact_parts {
        return invalid("rest frame must cover every artifact Part exactly once");
    }
    if !links.contains_key(root_link_id) {
        return invalid("root_link_id is absent from links");
    }

    let parent_values = rest_draft
        .get("parent_map")
        .and_then(Value::as_array)
        .filter(|items| items.len() + 1 == links.len())
        .ok_or_else(|| error("parent_map must contain exactly links-1 entries"))?;
    let mut parents = BTreeMap::<String, String>::new();
    for value in parent_values {
        let entry = exact_object(value, &["child_link_id", "parent_link_id"], "parent_map")?;
        let child = identifier(entry, "child_link_id")?;
        let parent = identifier(entry, "parent_link_id")?;
        if child == parent || child == root_link_id {
            return invalid("parent_map contains a self edge or parents the root");
        }
        if !links.contains_key(child) || !links.contains_key(parent) {
            return invalid("parent_map references an unknown link");
        }
        if parents
            .insert(child.to_owned(), parent.to_owned())
            .is_some()
        {
            return invalid("each non-root link must have exactly one parent");
        }
    }
    if links
        .keys()
        .any(|link_id| link_id != root_link_id && !parents.contains_key(link_id))
    {
        return invalid("parent_map does not cover every non-root link");
    }
    let evaluation_order = evaluation_order(root_link_id, &links, &parents)?;

    let normalized_links = links.values().map(link_value).collect::<Vec<_>>();
    let normalized_parent_map = parents
        .iter()
        .map(|(child, parent)| json!({"child_link_id":child,"parent_link_id":parent}))
        .collect::<Vec<_>>();
    let parent_map_sha256 = canonical_json_hash(&json!(normalized_parent_map));
    let mut rest_frame = json!({
        "schema_version":"MechanicalRestFrame@1",
        "rest_frame_id":rest_frame_id,
        "project_id":project_id,
        "artifact_id":artifact_id,
        "candidate_id":candidate_id,
        "program_sha256":program_sha256,
        "coordinate_system":"forgecad-rh-y-up-m@1",
        "transform_convention":"column-vector-trs-quaternion@1",
        "root_link_id":root_link_id,
        "links":normalized_links,
        "parent_map":normalized_parent_map,
        "evaluation_order":evaluation_order,
        "parent_map_sha256":parent_map_sha256,
        "canonical_sha256":""
    });
    rest_frame["canonical_sha256"] = Value::String(canonical_json_hash(&rest_frame));
    let rest_frame_sha256 = rest_frame["canonical_sha256"]
        .as_str()
        .expect("rest-frame hash")
        .to_owned();

    let (pose_action, pose_action_sha256, channel_values) = parse_action(
        object.get("pose_action_draft").unwrap_or(&Value::Null),
        project_id,
        candidate_id,
        program_sha256,
        &rest_frame_sha256,
        sample_time_ticks,
        &links,
    )?;
    let evaluation_policy = json!({
        "schema_version":"MechanicalPoseEvaluationPolicy@1",
        "math":"binary64-quantized-1e-12",
        "local_composition":"rest-local-times-joint-delta",
        "world_composition":"parent-world-times-local",
        "rotation":"normalized-quaternion-xyzw-canonical-sign",
        "interpolation":"scalar-linear-integer-ticks-clamped",
        "unkeyed":"rest",
        "max_links":MAX_LINKS,
        "max_depth":MAX_DEPTH,
        "max_channels":MAX_CHANNELS,
        "max_total_keys":MAX_TOTAL_KEYS,
        "geometry_materialization":"not-materialized"
    });
    let evaluation_policy_sha256 = canonical_json_hash(&evaluation_policy);

    let evaluated_nodes = evaluate_nodes(&links, &parents, &evaluation_order, &channel_values)?;
    let evaluated_pose_sha256 = canonical_json_hash(&json!({
        "schema_version":"MechanicalEvaluatedPoseIdentity@1",
        "project_id":project_id,
        "artifact_id":artifact_id,
        "candidate_id":candidate_id,
        "program_sha256":program_sha256,
        "rest_frame_sha256":rest_frame_sha256,
        "pose_action_sha256":pose_action_sha256,
        "sample_time_ticks":sample_time_ticks,
        "evaluation_policy_sha256":evaluation_policy_sha256,
        "evaluated_nodes":evaluated_nodes
    }));
    let mut result = json!({
        "schema_version":"MechanicalPoseEvaluationResult@1",
        "project_id":project_id,
        "artifact_id":artifact_id,
        "candidate_id":candidate_id,
        "artifact_readback_sha256":artifact_readback_sha256,
        "program_sha256":program_sha256,
        "operator_catalog_sha256":operator_catalog_sha256,
        "readback_config_sha256":readback_config_sha256,
        "input_sha256":input_sha256,
        "rest_frame":rest_frame,
        "rest_frame_sha256":rest_frame_sha256,
        "parent_map_sha256":parent_map_sha256,
        "pose_action":pose_action,
        "pose_action_sha256":pose_action_sha256,
        "sample_time_ticks":sample_time_ticks,
        "evaluation_policy":evaluation_policy,
        "evaluation_policy_sha256":evaluation_policy_sha256,
        "evaluation_order":evaluation_order,
        "evaluated_nodes":evaluated_nodes,
        "evaluated_pose_sha256":evaluated_pose_sha256,
        "geometry_materialization":"not-materialized",
        "worker_evaluation":"not-run-runtime-read-only-projection",
        "validator_status":"passed",
        "quality_status":"structural_only",
        "limitations":[
            "mechanical-rigid-links-only",
            "single-scalar-dof-per-link",
            "no-skinning-or-mesh-deformation",
            "no-ik-constraints-nla-fcurves-or-drivers",
            "no-geometry-materialization",
            "no-cross-platform-bit-exact-claim",
            "candidate-not-created",
            "visual-quality-not-evaluated"
        ],
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    validate_result(&result)?;
    if canonical_json_bytes(&result)
        .map_err(|source| error(source.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return invalid("canonical response exceeds 1 MiB");
    }
    Ok(result)
}

fn evaluate_sequence(runtime: &Runtime, request: &Value) -> Result<Value, RuntimeError> {
    let object = exact_object(
        request,
        &[
            "schema_version",
            "project_id",
            "artifact_id",
            "candidate_id",
            "artifact_readback_sha256",
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "rest_frame_draft",
            "pose_action_draft",
            "sample_time_ticks",
            "input_sha256",
        ],
        "sequence request",
    )?;
    if text(object, "schema_version")? != "MechanicalPoseSequencePreviewRequest@1" {
        return invalid("schema_version must be MechanicalPoseSequencePreviewRequest@1");
    }
    let input_sha256 = sha(object, "input_sha256")?;
    let mut input_preimage = Value::Object(object.clone());
    input_preimage
        .as_object_mut()
        .expect("sequence request is an object")
        .remove("input_sha256");
    if canonical_json_hash(&input_preimage) != input_sha256 {
        return invalid("input_sha256 does not match the closed sequence request");
    }
    let tick_values = object
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= MAX_SEQUENCE_SAMPLES)
        .ok_or_else(|| error("sample_time_ticks must contain 1..16 items"))?;
    let mut ticks = Vec::with_capacity(tick_values.len());
    let mut previous = None;
    for value in tick_values {
        let tick = value
            .as_u64()
            .filter(|tick| *tick <= 1_000_000)
            .ok_or_else(|| error("sample_time_ticks must contain bounded integers"))?;
        if previous.is_some_and(|prior| tick <= prior) {
            return invalid("sample_time_ticks must be strictly increasing and unique");
        }
        previous = Some(tick);
        ticks.push(tick);
    }
    if object.get("pose_action_draft").is_some_and(Value::is_null) && ticks.as_slice() != [0] {
        return invalid("a null pose_action_draft only permits sample_time_ticks [0]");
    }

    let mut common: Option<Value> = None;
    let mut samples = Vec::with_capacity(ticks.len());
    for tick in &ticks {
        let single = sequence_single_request(object, *tick);
        let result = evaluate_single(runtime, &single)?;
        let shared = sequence_common_from_single(&result)?;
        if let Some(existing) = &common {
            if existing != &shared {
                return invalid("sequence samples do not share one pose cohort");
            }
        } else {
            common = Some(shared);
        }
        samples.push(json!({
            "sample_time_ticks":tick,
            "evaluated_nodes":result["evaluated_nodes"],
            "evaluated_pose_sha256":result["evaluated_pose_sha256"]
        }));
    }
    let common = common.expect("non-empty bounded sequence");
    let sequence_identity = json!({
        "schema_version":"MechanicalPoseSequenceIdentity@1",
        "project_id":common["project_id"],
        "artifact_id":common["artifact_id"],
        "candidate_id":common["candidate_id"],
        "artifact_readback_sha256":common["artifact_readback_sha256"],
        "program_sha256":common["program_sha256"],
        "operator_catalog_sha256":common["operator_catalog_sha256"],
        "readback_config_sha256":common["readback_config_sha256"],
        "input_sha256":input_sha256,
        "rest_frame_sha256":common["rest_frame_sha256"],
        "pose_action_sha256":common["pose_action_sha256"],
        "evaluation_policy_sha256":common["evaluation_policy_sha256"],
        "sample_time_ticks":ticks,
        "samples":samples
    });
    let sequence_sha256 = canonical_json_hash(&sequence_identity);
    let common_object = common
        .as_object()
        .expect("sequence common projection is an object");
    let mut result = json!({
        "schema_version":"MechanicalPoseSequencePreview@1",
        "project_id":common_object["project_id"],
        "artifact_id":common_object["artifact_id"],
        "candidate_id":common_object["candidate_id"],
        "artifact_readback_sha256":common_object["artifact_readback_sha256"],
        "program_sha256":common_object["program_sha256"],
        "operator_catalog_sha256":common_object["operator_catalog_sha256"],
        "readback_config_sha256":common_object["readback_config_sha256"],
        "input_sha256":input_sha256,
        "rest_frame":common_object["rest_frame"],
        "rest_frame_sha256":common_object["rest_frame_sha256"],
        "parent_map_sha256":common_object["parent_map_sha256"],
        "pose_action":common_object["pose_action"],
        "pose_action_sha256":common_object["pose_action_sha256"],
        "sample_time_ticks":sequence_identity["sample_time_ticks"],
        "evaluation_policy":common_object["evaluation_policy"],
        "evaluation_policy_sha256":common_object["evaluation_policy_sha256"],
        "evaluation_order":common_object["evaluation_order"],
        "samples":sequence_identity["samples"],
        "sequence_sha256":sequence_sha256,
        "geometry_materialization":"not-materialized",
        "worker_evaluation":"not-run-runtime-read-only-projection",
        "validator_status":"passed",
        "quality_status":"structural_only",
        "limitations":[
            "mechanical-rigid-links-only",
            "sequence-preview-only",
            "maximum-16-ordered-samples",
            "single-scalar-dof-per-link",
            "no-skinning-or-mesh-deformation",
            "no-ik-constraints-nla-fcurves-or-drivers",
            "no-animation-asset-or-timeline",
            "no-geometry-materialization",
            "no-cross-platform-bit-exact-claim",
            "candidate-not-created",
            "visual-quality-not-evaluated"
        ],
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    validate_sequence_result(runtime, &result, request)?;
    if canonical_json_bytes(&result)
        .map_err(|source| error(source.to_string()))?
        .len()
        > MAX_RESPONSE_BYTES
    {
        return invalid("canonical sequence response exceeds 1 MiB");
    }
    Ok(result)
}

fn sequence_single_request(sequence_request: &Map<String, Value>, tick: u64) -> Value {
    let mut single = Value::Object(sequence_request.clone());
    single["schema_version"] = Value::String("MechanicalPoseEvaluationRequest@1".to_owned());
    single["sample_time_ticks"] = Value::from(tick);
    let mut single_preimage = single.clone();
    single_preimage
        .as_object_mut()
        .expect("single sequence sample request is an object")
        .remove("input_sha256");
    single["input_sha256"] = Value::String(canonical_json_hash(&single_preimage));
    single
}

fn sequence_common_from_single(value: &Value) -> Result<Value, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| error("single pose result is not an object"))?;
    let mut common = Map::new();
    for field in [
        "project_id",
        "artifact_id",
        "candidate_id",
        "artifact_readback_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "rest_frame",
        "rest_frame_sha256",
        "parent_map_sha256",
        "pose_action",
        "pose_action_sha256",
        "evaluation_policy",
        "evaluation_policy_sha256",
        "evaluation_order",
    ] {
        common.insert(
            field.to_owned(),
            object
                .get(field)
                .cloned()
                .ok_or_else(|| error(format!("single pose result lacks {field}")))?,
        );
    }
    Ok(Value::Object(common))
}

fn parse_link(value: &Value) -> Result<Link, RuntimeError> {
    let object = exact_object(
        value,
        &[
            "link_id",
            "part_id",
            "source_node_ids",
            "joint_type",
            "rest_translation_m",
            "rest_rotation_quat_xyzw",
            "axis_local",
            "limit_min",
            "limit_max",
            "value_unit",
        ],
        "link",
    )?;
    let link_id = identifier(object, "link_id")?.to_owned();
    let part_id = identifier(object, "part_id")?.to_owned();
    let sources = object
        .get("source_node_ids")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= 16)
        .ok_or_else(|| error("source_node_ids must contain 1..16 items"))?;
    let mut source_node_ids = sources
        .iter()
        .map(|source| {
            source
                .as_str()
                .filter(|source| is_opaque_id(source))
                .map(str::to_owned)
                .ok_or_else(|| error("source_node_ids contains an invalid identifier"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    source_node_ids.sort();
    source_node_ids.dedup();
    if source_node_ids.len() != sources.len() {
        return invalid("source_node_ids must be unique");
    }
    let joint_type = text(object, "joint_type")?.to_owned();
    let rest_translation = vec3(
        object.get("rest_translation_m"),
        -10.0,
        10.0,
        "rest_translation_m",
    )?;
    let rest_rotation = canonical_quaternion(
        object.get("rest_rotation_quat_xyzw"),
        "rest_rotation_quat_xyzw",
    )?;
    let value_unit = text(object, "value_unit")?.to_owned();
    let (axis, limit_min, limit_max) = match joint_type.as_str() {
        "fixed" => {
            if !object.get("axis_local").is_some_and(Value::is_null)
                || !object.get("limit_min").is_some_and(Value::is_null)
                || !object.get("limit_max").is_some_and(Value::is_null)
                || value_unit != "none"
            {
                return invalid("fixed links require null axis/limits and unit none");
            }
            (None, None, None)
        }
        "revolute" | "prismatic" => {
            let axis = vec3(object.get("axis_local"), -1.0, 1.0, "axis_local")?;
            let length = norm3(axis);
            if (length - 1.0).abs() > EPSILON {
                return invalid("joint axis must be unit length");
            }
            let min = finite_number(object.get("limit_min"), "limit_min")?;
            let max = finite_number(object.get("limit_max"), "limit_max")?;
            let expected_unit = if joint_type == "revolute" {
                "radian"
            } else {
                "meter"
            };
            let bound = if joint_type == "revolute" {
                std::f64::consts::PI
            } else {
                1.0
            };
            if value_unit != expected_unit || min > max || min < -bound || max > bound {
                return invalid("joint limits or value_unit are invalid");
            }
            (Some(axis.map(q)), Some(q(min)), Some(q(max)))
        }
        _ => return invalid("joint_type must be fixed, revolute or prismatic"),
    };
    Ok(Link {
        link_id,
        part_id,
        source_node_ids,
        joint_type,
        rest_translation: rest_translation.map(q),
        rest_rotation,
        axis,
        limit_min,
        limit_max,
        value_unit,
    })
}

fn parse_action(
    value: &Value,
    project_id: &str,
    candidate_id: &str,
    program_sha256: &str,
    rest_frame_sha256: &str,
    sample_time_ticks: u64,
    links: &BTreeMap<String, Link>,
) -> Result<(Value, Value, BTreeMap<String, f64>), RuntimeError> {
    if value.is_null() {
        if sample_time_ticks != 0 {
            return invalid("sample_time_ticks must be zero when pose_action_draft is null");
        }
        return Ok((Value::Null, Value::Null, BTreeMap::new()));
    }
    let object = exact_object(
        value,
        &[
            "schema_version",
            "action_id",
            "timebase_hz",
            "duration_ticks",
            "interpolation",
            "extrapolation",
            "unkeyed_policy",
            "channels",
        ],
        "pose_action_draft",
    )?;
    if text(object, "schema_version")? != "MechanicalPoseActionDraft@1"
        || object.get("timebase_hz").and_then(Value::as_u64) != Some(1000)
        || text(object, "interpolation")? != "linear@1"
        || text(object, "extrapolation")? != "clamp@1"
        || text(object, "unkeyed_policy")? != "rest@1"
    {
        return invalid("pose action policy is unsupported");
    }
    let action_id = identifier(object, "action_id")?;
    let duration_ticks = object
        .get("duration_ticks")
        .and_then(Value::as_u64)
        .filter(|value| (1..=1_000_000).contains(value))
        .ok_or_else(|| error("duration_ticks must be 1..1000000"))?;
    if sample_time_ticks > duration_ticks {
        return invalid("sample_time_ticks exceeds duration_ticks");
    }
    let channels = object
        .get("channels")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= MAX_CHANNELS)
        .ok_or_else(|| error("channels must contain 1..64 items"))?;
    let mut normalized_channels = BTreeMap::<String, Value>::new();
    let mut values = BTreeMap::new();
    let mut total_keys = 0usize;
    for value in channels {
        let channel = exact_object(value, &["link_id", "value_unit", "keys"], "channel")?;
        let link_id = identifier(channel, "link_id")?;
        let link = links
            .get(link_id)
            .ok_or_else(|| error("channel references an unknown link"))?;
        if link.joint_type == "fixed" || text(channel, "value_unit")? != link.value_unit {
            return invalid("channel unit differs from the link joint or targets a fixed link");
        }
        let keys = channel
            .get("keys")
            .and_then(Value::as_array)
            .filter(|items| !items.is_empty() && items.len() <= MAX_KEYS_PER_CHANNEL)
            .ok_or_else(|| error("channel keys must contain 1..32 items"))?;
        total_keys += keys.len();
        if total_keys > MAX_TOTAL_KEYS {
            return invalid("total key count exceeds 512");
        }
        let mut normalized_keys = Vec::with_capacity(keys.len());
        let mut parsed_keys = Vec::with_capacity(keys.len());
        let mut previous_tick = None;
        for key in keys {
            let key = exact_object(key, &["time_ticks", "value"], "key")?;
            let tick = key
                .get("time_ticks")
                .and_then(Value::as_u64)
                .filter(|tick| *tick <= duration_ticks)
                .ok_or_else(|| error("key time_ticks exceeds duration"))?;
            if previous_tick.is_some_and(|previous| tick <= previous) {
                return invalid("key time_ticks must be strictly increasing");
            }
            previous_tick = Some(tick);
            let number = finite_number(key.get("value"), "key.value")?;
            if number < link.limit_min.expect("non-fixed link")
                || number > link.limit_max.expect("non-fixed link")
            {
                return invalid("key value exceeds joint limits");
            }
            normalized_keys.push(json!({"time_ticks":tick,"value":q(number)}));
            parsed_keys.push((tick, q(number)));
        }
        let sampled = sample_keys(&parsed_keys, sample_time_ticks);
        if normalized_channels
            .insert(
                link_id.to_owned(),
                json!({"link_id":link_id,"value_unit":link.value_unit,"keys":normalized_keys}),
            )
            .is_some()
        {
            return invalid("each link may have at most one channel");
        }
        values.insert(link_id.to_owned(), sampled);
    }
    let mut action = json!({
        "schema_version":"MechanicalPoseAction@1",
        "action_id":action_id,
        "project_id":project_id,
        "candidate_id":candidate_id,
        "rest_frame_sha256":rest_frame_sha256,
        "program_sha256":program_sha256,
        "timebase_hz":1000,
        "duration_ticks":duration_ticks,
        "interpolation":"linear@1",
        "extrapolation":"clamp@1",
        "unkeyed_policy":"rest@1",
        "channels":normalized_channels.into_values().collect::<Vec<_>>(),
        "canonical_sha256":""
    });
    action["canonical_sha256"] = Value::String(canonical_json_hash(&action));
    let hash = action["canonical_sha256"].clone();
    Ok((action, hash, values))
}

fn sample_keys(keys: &[(u64, f64)], tick: u64) -> f64 {
    if tick <= keys[0].0 {
        return keys[0].1;
    }
    if tick >= keys[keys.len() - 1].0 {
        return keys[keys.len() - 1].1;
    }
    for pair in keys.windows(2) {
        if (pair[0].0..=pair[1].0).contains(&tick) {
            let alpha = (tick - pair[0].0) as f64 / (pair[1].0 - pair[0].0) as f64;
            return q(pair[0].1 + alpha * (pair[1].1 - pair[0].1));
        }
    }
    0.0
}

fn evaluation_order(
    root: &str,
    links: &BTreeMap<String, Link>,
    parents: &BTreeMap<String, String>,
) -> Result<Vec<String>, RuntimeError> {
    let mut children = BTreeMap::<String, Vec<String>>::new();
    for (child, parent) in parents {
        children
            .entry(parent.clone())
            .or_default()
            .push(child.clone());
    }
    for values in children.values_mut() {
        values.sort();
    }
    let mut order = Vec::with_capacity(links.len());
    let mut stack = vec![(root.to_owned(), 0usize)];
    while let Some((link_id, depth)) = stack.pop() {
        if depth > MAX_DEPTH || order.iter().any(|existing| existing == &link_id) {
            return invalid("parent map contains a cycle or exceeds depth 16");
        }
        order.push(link_id.clone());
        if let Some(values) = children.get(&link_id) {
            for child in values.iter().rev() {
                stack.push((child.clone(), depth + 1));
            }
        }
    }
    if order.len() != links.len() {
        return invalid("parent map is disconnected or cyclic");
    }
    Ok(order)
}

fn evaluate_nodes(
    links: &BTreeMap<String, Link>,
    parents: &BTreeMap<String, String>,
    evaluation_order: &[String],
    channel_values: &BTreeMap<String, f64>,
) -> Result<Vec<Value>, RuntimeError> {
    let mut world = BTreeMap::<String, Transform>::new();
    let mut nodes = Vec::with_capacity(links.len());
    for link_id in evaluation_order {
        let link = links
            .get(link_id)
            .ok_or_else(|| error("evaluation order references an unknown link"))?;
        let joint_value = channel_values.get(link_id).copied().unwrap_or(0.0);
        let delta = joint_delta(link, joint_value)?;
        let rest = Transform {
            translation: link.rest_translation,
            rotation: link.rest_rotation,
        };
        let local = compose(rest, delta)?;
        let parent_link_id = parents.get(link_id);
        let world_pose = match parent_link_id {
            Some(parent_id) => compose(
                *world
                    .get(parent_id)
                    .ok_or_else(|| error("parent pose is unavailable"))?,
                local,
            )?,
            None => local,
        };
        world.insert(link_id.clone(), world_pose);
        nodes.push(json!({
            "link_id":link.link_id,
            "part_id":link.part_id,
            "parent_link_id":parent_link_id,
            "joint_type":link.joint_type,
            "joint_value":q(joint_value),
            "value_unit":link.value_unit,
            "local_pose":transform_value(local),
            "world_pose":transform_value(world_pose)
        }));
    }
    Ok(nodes)
}

fn joint_delta(link: &Link, value: f64) -> Result<Transform, RuntimeError> {
    match link.joint_type.as_str() {
        "fixed" => Ok(identity()),
        "revolute" => Ok(Transform {
            translation: [0.0; 3],
            rotation: axis_angle(link.axis.expect("revolute axis"), value)?,
        }),
        "prismatic" => Ok(Transform {
            translation: link
                .axis
                .expect("prismatic axis")
                .map(|axis| q(axis * value)),
            rotation: [0.0, 0.0, 0.0, 1.0],
        }),
        _ => invalid("unknown joint type"),
    }
}

fn compose(parent: Transform, local: Transform) -> Result<Transform, RuntimeError> {
    let rotated = rotate(parent.rotation, local.translation)?;
    Ok(Transform {
        translation: [
            q(parent.translation[0] + rotated[0]),
            q(parent.translation[1] + rotated[1]),
            q(parent.translation[2] + rotated[2]),
        ],
        rotation: canonical_quaternion_array(mul_quat(parent.rotation, local.rotation))?,
    })
}

fn identity() -> Transform {
    Transform {
        translation: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
    }
}

fn inverse(value: Transform) -> Result<Transform, RuntimeError> {
    let inverse_rotation = canonical_quaternion_array([
        -value.rotation[0],
        -value.rotation[1],
        -value.rotation[2],
        value.rotation[3],
    ])?;
    let inverse_translation = rotate(
        inverse_rotation,
        [
            -value.translation[0],
            -value.translation[1],
            -value.translation[2],
        ],
    )?;
    Ok(Transform {
        translation: inverse_translation,
        rotation: inverse_rotation,
    })
}

fn quaternion_to_worker_euler(rotation: [f64; 4]) -> Result<[f64; 3], RuntimeError> {
    let [x, y, z, w] = rotation;
    let r20 = 2.0 * (x * z - w * y);
    let pitch = (-r20.clamp(-1.0, 1.0)).asin();
    if pitch.cos().abs() <= 1.0e-6 {
        return invalid("pose delta is too close to an Euler XYZ gimbal singularity");
    }
    let roll = (2.0 * (y * z + w * x)).atan2(1.0 - 2.0 * (x * x + y * y));
    let yaw = (2.0 * (x * y + w * z)).atan2(1.0 - 2.0 * (y * y + z * z));
    let result = [q(roll), q(pitch), q(yaw)];
    if result.iter().any(|value| !value.is_finite()) {
        return invalid("pose delta Euler lowering produced a non-finite value");
    }
    Ok(result)
}

fn ensure_worker_delta_translation_bounded(translation: [f64; 3]) -> Result<(), RuntimeError> {
    if translation.iter().any(|value| value.abs() > 10.0) {
        return invalid("pose delta translation exceeds transform@2 coordinate bounds");
    }
    Ok(())
}

fn transforms_by_part(nodes: &[Value]) -> Result<BTreeMap<String, Transform>, RuntimeError> {
    nodes
        .iter()
        .map(|node| {
            let object = node
                .as_object()
                .ok_or_else(|| error("evaluated node is invalid"))?;
            let part_id = identifier(object, "part_id")?.to_owned();
            let world = object
                .get("world_pose")
                .and_then(Value::as_object)
                .ok_or_else(|| error("evaluated node world_pose is invalid"))?;
            let translation = vec3(world.get("translation_m"), -640.0, 640.0, "translation_m")?;
            let rotation =
                canonical_quaternion(world.get("rotation_quat_xyzw"), "rotation_quat_xyzw")?;
            Ok((
                part_id,
                Transform {
                    translation,
                    rotation,
                },
            ))
        })
        .collect()
}

fn reachable_source_owners(
    root: &str,
    node_inputs: &BTreeMap<String, Vec<String>>,
    source_owner: &BTreeMap<String, String>,
) -> Result<BTreeSet<String>, RuntimeError> {
    let mut stack = vec![root.to_owned()];
    let mut visited = BTreeSet::new();
    let mut owners = BTreeSet::new();
    while let Some(node_id) = stack.pop() {
        if !visited.insert(node_id.clone()) {
            continue;
        }
        if let Some(owner) = source_owner.get(&node_id) {
            owners.insert(owner.clone());
        }
        let inputs = node_inputs
            .get(&node_id)
            .ok_or_else(|| error("part_output references an unknown GeometryProgram node"))?;
        stack.extend(inputs.iter().cloned());
    }
    Ok(owners)
}

fn axis_angle(axis: [f64; 3], angle: f64) -> Result<[f64; 4], RuntimeError> {
    let half = angle * 0.5;
    let (sin, cos) = half.sin_cos();
    canonical_quaternion_array([axis[0] * sin, axis[1] * sin, axis[2] * sin, cos])
}

fn mul_quat(left: [f64; 4], right: [f64; 4]) -> [f64; 4] {
    let [lx, ly, lz, lw] = left;
    let [rx, ry, rz, rw] = right;
    [
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
        lw * rw - lx * rx - ly * ry - lz * rz,
    ]
}

fn rotate(rotation: [f64; 4], vector: [f64; 3]) -> Result<[f64; 3], RuntimeError> {
    let vector_quat = [vector[0], vector[1], vector[2], 0.0];
    let conjugate = [-rotation[0], -rotation[1], -rotation[2], rotation[3]];
    let result = mul_quat(mul_quat(rotation, vector_quat), conjugate);
    if result.iter().any(|value| !value.is_finite()) {
        return invalid("pose transform produced a non-finite vector");
    }
    Ok([q(result[0]), q(result[1]), q(result[2])])
}

fn canonical_quaternion(value: Option<&Value>, field: &str) -> Result<[f64; 4], RuntimeError> {
    let array = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 4)
        .ok_or_else(|| error(format!("{field} must contain four numbers")))?;
    canonical_quaternion_array([
        finite_number(array.first(), field)?,
        finite_number(array.get(1), field)?,
        finite_number(array.get(2), field)?,
        finite_number(array.get(3), field)?,
    ])
}

fn canonical_quaternion_array(mut value: [f64; 4]) -> Result<[f64; 4], RuntimeError> {
    let length = value
        .iter()
        .map(|component| component * component)
        .sum::<f64>()
        .sqrt();
    if !length.is_finite() || length <= EPSILON {
        return invalid("quaternion length is invalid");
    }
    for component in &mut value {
        *component /= length;
    }
    let sign_negative = value[3] < 0.0
        || (value[3].abs() <= EPSILON
            && value[..3]
                .iter()
                .find(|component| component.abs() > EPSILON)
                .is_some_and(|component| *component < 0.0));
    if sign_negative {
        for component in &mut value {
            *component = -*component;
        }
    }
    Ok(value.map(q))
}

fn transform_value(value: Transform) -> Value {
    json!({
        "translation_m":value.translation,
        "rotation_quat_xyzw":value.rotation,
        "scale":[1.0,1.0,1.0]
    })
}

fn link_value(link: &Link) -> Value {
    json!({
        "link_id":link.link_id,
        "part_id":link.part_id,
        "source_node_ids":link.source_node_ids,
        "joint_type":link.joint_type,
        "rest_translation_m":link.rest_translation,
        "rest_rotation_quat_xyzw":link.rest_rotation,
        "axis_local":link.axis,
        "limit_min":link.limit_min,
        "limit_max":link.limit_max,
        "value_unit":link.value_unit
    })
}

fn exact_object<'a>(
    value: &'a Value,
    fields: &[&str],
    scope: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| error(format!("{scope} must be an object")))?;
    if object.len() != fields.len() || object.keys().any(|key| !fields.contains(&key.as_str())) {
        return invalid(&format!("{scope} field set is not closed"));
    }
    Ok(object)
}

fn identifier<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .ok_or_else(|| error(format!("{field} is not a valid identifier")))
}

fn sha<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| error(format!("{field} is not a SHA-256")))
}

fn text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, RuntimeError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| error(format!("{field} must be a string")))
}

fn finite_number(value: Option<&Value>, field: &str) -> Result<f64, RuntimeError> {
    value
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| error(format!("{field} must be finite")))
}

fn vec3(value: Option<&Value>, min: f64, max: f64, field: &str) -> Result<[f64; 3], RuntimeError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|values| values.len() == 3)
        .ok_or_else(|| error(format!("{field} must contain three numbers")))?;
    let result = [
        finite_number(values.first(), field)?,
        finite_number(values.get(1), field)?,
        finite_number(values.get(2), field)?,
    ];
    if result.iter().any(|value| !(min..=max).contains(value)) {
        return invalid(&format!("{field} is outside its bounded range"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worker_euler_quaternion(euler: [f64; 3]) -> [f64; 4] {
        let x = axis_angle([1.0, 0.0, 0.0], euler[0]).expect("x rotation");
        let y = axis_angle([0.0, 1.0, 0.0], euler[1]).expect("y rotation");
        let z = axis_angle([0.0, 0.0, 1.0], euler[2]).expect("z rotation");
        canonical_quaternion_array(mul_quat(z, mul_quat(y, x)))
            .expect("worker X then Y then Z rotation")
    }

    #[test]
    fn quaternion_lowering_matches_worker_xyz_rotation_and_rejects_gimbal() {
        let expected_euler = [0.37, -0.41, 0.82];
        let quaternion = worker_euler_quaternion(expected_euler);
        let lowered = quaternion_to_worker_euler(quaternion).expect("non-singular lowering");
        for (actual, expected) in lowered.iter().zip(expected_euler) {
            assert!((actual - expected).abs() <= 1.0e-10);
        }
        let sample = [0.23, -0.61, 0.47];
        let quaternion_result = rotate(quaternion, sample).expect("quaternion rotation");
        let worker_result =
            rotate(worker_euler_quaternion(lowered), sample).expect("worker Euler rotation");
        for (actual, expected) in worker_result.iter().zip(quaternion_result) {
            assert!((actual - expected).abs() <= 1.0e-10);
        }

        let gimbal = worker_euler_quaternion([0.1, std::f64::consts::FRAC_PI_2, -0.2]);
        assert!(quaternion_to_worker_euler(gimbal).is_err());
    }

    #[test]
    fn world_pose_parsing_allows_hierarchy_accumulation_but_remains_bounded() {
        let nodes = json!([{
            "part_id":"deep-part",
            "world_pose":{
                "translation_m":[120.0,-80.0,32.0],
                "rotation_quat_xyzw":[0.0,0.0,0.0,1.0],
                "scale":[1.0,1.0,1.0]
            }
        }]);
        let parsed = transforms_by_part(nodes.as_array().unwrap()).expect("bounded world pose");
        assert_eq!(parsed["deep-part"].translation, [120.0, -80.0, 32.0]);

        let mut overflow = nodes;
        overflow[0]["world_pose"]["translation_m"][0] = json!(641.0);
        assert!(transforms_by_part(overflow.as_array().unwrap()).is_err());
    }

    #[test]
    fn worker_delta_translation_is_bounded_independently_from_world_pose() {
        ensure_worker_delta_translation_bounded([10.0, -10.0, 0.0]).expect("worker-boundary delta");
        assert!(ensure_worker_delta_translation_bounded([10.000_001, 0.0, 0.0]).is_err());
    }
}

fn norm3(value: [f64; 3]) -> f64 {
    (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt()
}

fn q(value: f64) -> f64 {
    let rounded = (value * QUANTIZE_SCALE).round() / QUANTIZE_SCALE;
    if rounded == -0.0 {
        0.0
    } else {
        rounded
    }
}

pub(super) fn validate_result(value: &Value) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "artifact_id",
            "candidate_id",
            "artifact_readback_sha256",
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "input_sha256",
            "rest_frame",
            "rest_frame_sha256",
            "parent_map_sha256",
            "pose_action",
            "pose_action_sha256",
            "sample_time_ticks",
            "evaluation_policy",
            "evaluation_policy_sha256",
            "evaluation_order",
            "evaluated_nodes",
            "evaluated_pose_sha256",
            "geometry_materialization",
            "worker_evaluation",
            "validator_status",
            "quality_status",
            "limitations",
            "canonical_sha256",
        ],
        "mechanical pose result",
    )?;
    if text(object, "schema_version")? != "MechanicalPoseEvaluationResult@1"
        || text(object, "geometry_materialization")? != "not-materialized"
        || text(object, "worker_evaluation")? != "not-run-runtime-read-only-projection"
        || text(object, "validator_status")? != "passed"
        || text(object, "quality_status")? != "structural_only"
    {
        return invalid("mechanical pose result policy fields are invalid");
    }
    identifier(object, "project_id")?;
    identifier(object, "candidate_id")?;
    for field in [
        "artifact_id",
        "artifact_readback_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "input_sha256",
        "rest_frame_sha256",
        "parent_map_sha256",
        "evaluation_policy_sha256",
        "evaluated_pose_sha256",
        "canonical_sha256",
    ] {
        sha(object, field)?;
    }
    let limitations = object
        .get("limitations")
        .and_then(Value::as_array)
        .ok_or_else(|| error("limitations must be an array"))?;
    if limitations
        != json!([
            "mechanical-rigid-links-only",
            "single-scalar-dof-per-link",
            "no-skinning-or-mesh-deformation",
            "no-ik-constraints-nla-fcurves-or-drivers",
            "no-geometry-materialization",
            "no-cross-platform-bit-exact-claim",
            "candidate-not-created",
            "visual-quality-not-evaluated"
        ])
        .as_array()
        .expect("limitations fixture")
    {
        return invalid("mechanical pose result limitations drifted");
    }

    let rest = exact_object(
        object
            .get("rest_frame")
            .ok_or_else(|| error("rest_frame is required"))?,
        &[
            "schema_version",
            "rest_frame_id",
            "project_id",
            "artifact_id",
            "candidate_id",
            "program_sha256",
            "coordinate_system",
            "transform_convention",
            "root_link_id",
            "links",
            "parent_map",
            "evaluation_order",
            "parent_map_sha256",
            "canonical_sha256",
        ],
        "rest_frame result",
    )?;
    if text(rest, "schema_version")? != "MechanicalRestFrame@1"
        || text(rest, "coordinate_system")? != "forgecad-rh-y-up-m@1"
        || text(rest, "transform_convention")? != "column-vector-trs-quaternion@1"
        || rest.get("project_id") != object.get("project_id")
        || rest.get("artifact_id") != object.get("artifact_id")
        || rest.get("candidate_id") != object.get("candidate_id")
        || rest.get("program_sha256") != object.get("program_sha256")
        || rest.get("parent_map_sha256") != object.get("parent_map_sha256")
        || rest.get("evaluation_order") != object.get("evaluation_order")
    {
        return invalid("rest frame result binding drifted");
    }
    validate_embedded_canonical_hash(rest, "rest_frame")?;
    if rest.get("canonical_sha256") != object.get("rest_frame_sha256") {
        return invalid("rest_frame_sha256 differs from the embedded rest frame");
    }
    let parent_map = rest
        .get("parent_map")
        .ok_or_else(|| error("rest frame parent_map is required"))?;
    if canonical_json_hash(parent_map) != text(object, "parent_map_sha256")? {
        return invalid("parent_map_sha256 differs from the embedded parent map");
    }

    let link_values = rest
        .get("links")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= MAX_LINKS)
        .ok_or_else(|| error("rest frame links must contain 1..64 items"))?;
    let mut links = BTreeMap::<String, Link>::new();
    let mut part_ids = BTreeSet::new();
    for value in link_values {
        let link = parse_link(value)?;
        if link_value(&link) != *value {
            return invalid("rest frame link is not in canonical normalized form");
        }
        if links.insert(link.link_id.clone(), link.clone()).is_some()
            || !part_ids.insert(link.part_id.clone())
        {
            return invalid("rest frame link_id and part_id values must be unique");
        }
    }
    let normalized_links = links.values().map(link_value).collect::<Vec<_>>();
    if normalized_links != *link_values {
        return invalid("rest frame links must use canonical link_id ordering");
    }
    let root_link_id = identifier(rest, "root_link_id")?;
    if !links.contains_key(root_link_id) {
        return invalid("rest frame root_link_id is absent from links");
    }
    let parent_values = parent_map
        .as_array()
        .filter(|items| items.len() + 1 == links.len())
        .ok_or_else(|| error("rest frame parent_map must contain links-1 entries"))?;
    let mut parents = BTreeMap::<String, String>::new();
    for value in parent_values {
        let entry = exact_object(value, &["child_link_id", "parent_link_id"], "parent_map")?;
        let child = identifier(entry, "child_link_id")?;
        let parent = identifier(entry, "parent_link_id")?;
        if child == parent
            || child == root_link_id
            || !links.contains_key(child)
            || !links.contains_key(parent)
            || parents
                .insert(child.to_owned(), parent.to_owned())
                .is_some()
        {
            return invalid("rest frame parent_map is not a unique closed hierarchy");
        }
    }
    let normalized_parent_map = parents
        .iter()
        .map(|(child, parent)| json!({"child_link_id":child,"parent_link_id":parent}))
        .collect::<Vec<_>>();
    if normalized_parent_map != *parent_values {
        return invalid("rest frame parent_map must use canonical child ordering");
    }
    let derived_order = evaluation_order(root_link_id, &links, &parents)?;
    if rest.get("evaluation_order") != Some(&json!(derived_order))
        || object.get("evaluation_order") != rest.get("evaluation_order")
    {
        return invalid("evaluation_order differs from the rest-frame hierarchy");
    }

    let sample_time_ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_u64)
        .ok_or_else(|| error("sample_time_ticks must be a non-negative integer"))?;

    let action = object
        .get("pose_action")
        .ok_or_else(|| error("pose_action is required"))?;
    let channel_values = match (action.is_null(), object.get("pose_action_sha256")) {
        (true, Some(hash)) if hash.is_null() => {
            if sample_time_ticks != 0 {
                return invalid("sample_time_ticks must be zero without a pose action");
            }
            BTreeMap::new()
        }
        (false, Some(hash)) => {
            let action_object = exact_object(
                action,
                &[
                    "schema_version",
                    "action_id",
                    "project_id",
                    "candidate_id",
                    "rest_frame_sha256",
                    "program_sha256",
                    "timebase_hz",
                    "duration_ticks",
                    "interpolation",
                    "extrapolation",
                    "unkeyed_policy",
                    "channels",
                    "canonical_sha256",
                ],
                "pose_action result",
            )?;
            validate_embedded_canonical_hash(action_object, "pose_action")?;
            if text(action_object, "schema_version")? != "MechanicalPoseAction@1"
                || text(action_object, "interpolation")? != "linear@1"
                || text(action_object, "extrapolation")? != "clamp@1"
                || text(action_object, "unkeyed_policy")? != "rest@1"
                || action_object.get("canonical_sha256") != Some(hash)
                || action_object.get("project_id") != object.get("project_id")
                || action_object.get("candidate_id") != object.get("candidate_id")
                || action_object.get("program_sha256") != object.get("program_sha256")
                || action_object.get("rest_frame_sha256") != object.get("rest_frame_sha256")
            {
                return invalid("pose action result binding drifted");
            }
            let action_draft = json!({
                "schema_version":"MechanicalPoseActionDraft@1",
                "action_id":action_object["action_id"],
                "timebase_hz":action_object["timebase_hz"],
                "duration_ticks":action_object["duration_ticks"],
                "interpolation":action_object["interpolation"],
                "extrapolation":action_object["extrapolation"],
                "unkeyed_policy":action_object["unkeyed_policy"],
                "channels":action_object["channels"]
            });
            let (derived_action, derived_hash, values) = parse_action(
                &action_draft,
                text(object, "project_id")?,
                text(object, "candidate_id")?,
                text(object, "program_sha256")?,
                text(object, "rest_frame_sha256")?,
                sample_time_ticks,
                &links,
            )?;
            if derived_action != *action || derived_hash != *hash {
                return invalid("pose action differs from normalized bounded evaluation input");
            }
            values
        }
        _ => return invalid("pose_action and pose_action_sha256 nullability differs"),
    };

    let policy = exact_object(
        object
            .get("evaluation_policy")
            .ok_or_else(|| error("evaluation_policy is required"))?,
        &[
            "schema_version",
            "math",
            "local_composition",
            "world_composition",
            "rotation",
            "interpolation",
            "unkeyed",
            "max_links",
            "max_depth",
            "max_channels",
            "max_total_keys",
            "geometry_materialization",
        ],
        "evaluation_policy",
    )?;
    if policy
        != json!({
            "schema_version":"MechanicalPoseEvaluationPolicy@1",
            "math":"binary64-quantized-1e-12",
            "local_composition":"rest-local-times-joint-delta",
            "world_composition":"parent-world-times-local",
            "rotation":"normalized-quaternion-xyzw-canonical-sign",
            "interpolation":"scalar-linear-integer-ticks-clamped",
            "unkeyed":"rest",
            "max_links":64,
            "max_depth":16,
            "max_channels":64,
            "max_total_keys":512,
            "geometry_materialization":"not-materialized"
        })
        .as_object()
        .expect("policy fixture")
    {
        return invalid("evaluation policy constants drifted");
    }
    if canonical_json_hash(object.get("evaluation_policy").expect("policy exists"))
        != text(object, "evaluation_policy_sha256")?
    {
        return invalid("evaluation_policy_sha256 differs from the embedded policy");
    }

    let order = object
        .get("evaluation_order")
        .and_then(Value::as_array)
        .filter(|items| items.len() == links.len())
        .ok_or_else(|| error("evaluation_order must cover every rest-frame link"))?;
    let nodes = object
        .get("evaluated_nodes")
        .and_then(Value::as_array)
        .filter(|items| items.len() == order.len())
        .ok_or_else(|| error("evaluated_nodes must match evaluation_order"))?;
    let mut seen = BTreeSet::new();
    for (expected_link, node) in order.iter().zip(nodes) {
        let expected_link = expected_link
            .as_str()
            .filter(|value| is_opaque_id(value))
            .ok_or_else(|| error("evaluation_order contains an invalid link"))?;
        if !seen.insert(expected_link) {
            return invalid("evaluation_order contains a duplicate link");
        }
        let node = exact_object(
            node,
            &[
                "link_id",
                "part_id",
                "parent_link_id",
                "joint_type",
                "joint_value",
                "value_unit",
                "local_pose",
                "world_pose",
            ],
            "evaluated node",
        )?;
        if identifier(node, "link_id")? != expected_link {
            return invalid("evaluated node order differs from evaluation_order");
        }
        identifier(node, "part_id")?;
        if node
            .get("parent_link_id")
            .is_some_and(|parent| !parent.is_null())
        {
            identifier(node, "parent_link_id")?;
        }
        finite_number(node.get("joint_value"), "joint_value")?;
        validate_pose(node.get("local_pose"), "local_pose")?;
        validate_pose(node.get("world_pose"), "world_pose")?;
    }
    let expected_nodes = evaluate_nodes(&links, &parents, &derived_order, &channel_values)?;
    if expected_nodes != *nodes {
        return invalid("evaluated nodes differ from the recomputed bounded hierarchy");
    }
    let identity = json!({
        "schema_version":"MechanicalEvaluatedPoseIdentity@1",
        "project_id":object["project_id"],
        "artifact_id":object["artifact_id"],
        "candidate_id":object["candidate_id"],
        "program_sha256":object["program_sha256"],
        "rest_frame_sha256":object["rest_frame_sha256"],
        "pose_action_sha256":object["pose_action_sha256"],
        "sample_time_ticks":object["sample_time_ticks"],
        "evaluation_policy_sha256":object["evaluation_policy_sha256"],
        "evaluated_nodes":object["evaluated_nodes"]
    });
    if canonical_json_hash(&identity) != text(object, "evaluated_pose_sha256")? {
        return invalid("evaluated_pose_sha256 differs from the evaluated pose identity");
    }
    validate_embedded_canonical_hash(object, "mechanical pose result")
}

pub(super) fn validate_sequence_result(
    runtime: &Runtime,
    value: &Value,
    expected_request: &Value,
) -> Result<(), RuntimeError> {
    let expected = exact_object(
        expected_request,
        &[
            "schema_version",
            "project_id",
            "artifact_id",
            "candidate_id",
            "artifact_readback_sha256",
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "rest_frame_draft",
            "pose_action_draft",
            "sample_time_ticks",
            "input_sha256",
        ],
        "expected sequence request",
    )?;
    if text(expected, "schema_version")? != "MechanicalPoseSequencePreviewRequest@1" {
        return invalid("expected request is not a mechanical pose sequence preview request");
    }
    let mut expected_preimage = Value::Object(expected.clone());
    expected_preimage
        .as_object_mut()
        .expect("expected sequence request is an object")
        .remove("input_sha256");
    if canonical_json_hash(&expected_preimage) != sha(expected, "input_sha256")? {
        return invalid("expected sequence request input_sha256 drifted");
    }
    let object = exact_object(
        value,
        &[
            "schema_version",
            "project_id",
            "artifact_id",
            "candidate_id",
            "artifact_readback_sha256",
            "program_sha256",
            "operator_catalog_sha256",
            "readback_config_sha256",
            "input_sha256",
            "rest_frame",
            "rest_frame_sha256",
            "parent_map_sha256",
            "pose_action",
            "pose_action_sha256",
            "sample_time_ticks",
            "evaluation_policy",
            "evaluation_policy_sha256",
            "evaluation_order",
            "samples",
            "sequence_sha256",
            "geometry_materialization",
            "worker_evaluation",
            "validator_status",
            "quality_status",
            "limitations",
            "canonical_sha256",
        ],
        "mechanical pose sequence result",
    )?;
    if text(object, "schema_version")? != "MechanicalPoseSequencePreview@1"
        || text(object, "geometry_materialization")? != "not-materialized"
        || text(object, "worker_evaluation")? != "not-run-runtime-read-only-projection"
        || text(object, "validator_status")? != "passed"
        || text(object, "quality_status")? != "structural_only"
    {
        return invalid("mechanical pose sequence result policy fields are invalid");
    }
    identifier(object, "project_id")?;
    identifier(object, "candidate_id")?;
    for field in [
        "artifact_id",
        "artifact_readback_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "input_sha256",
        "rest_frame_sha256",
        "parent_map_sha256",
        "evaluation_policy_sha256",
        "sequence_sha256",
        "canonical_sha256",
    ] {
        sha(object, field)?;
    }
    for field in [
        "project_id",
        "artifact_id",
        "candidate_id",
        "artifact_readback_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "readback_config_sha256",
        "input_sha256",
        "sample_time_ticks",
    ] {
        if object.get(field) != expected.get(field) {
            return invalid(&format!(
                "mechanical pose sequence result differs from expected request field {field}"
            ));
        }
    }
    if object.get("limitations")
        != Some(&json!([
            "mechanical-rigid-links-only",
            "sequence-preview-only",
            "maximum-16-ordered-samples",
            "single-scalar-dof-per-link",
            "no-skinning-or-mesh-deformation",
            "no-ik-constraints-nla-fcurves-or-drivers",
            "no-animation-asset-or-timeline",
            "no-geometry-materialization",
            "no-cross-platform-bit-exact-claim",
            "candidate-not-created",
            "visual-quality-not-evaluated"
        ]))
    {
        return invalid("mechanical pose sequence limitations drifted");
    }
    let ticks = object
        .get("sample_time_ticks")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= MAX_SEQUENCE_SAMPLES)
        .ok_or_else(|| error("sequence sample_time_ticks must contain 1..16 items"))?;
    let samples = object
        .get("samples")
        .and_then(Value::as_array)
        .filter(|items| items.len() == ticks.len())
        .ok_or_else(|| error("sequence samples must match sample_time_ticks"))?;
    let mut previous = None;
    for (tick_value, sample_value) in ticks.iter().zip(samples) {
        let tick = tick_value
            .as_u64()
            .filter(|tick| *tick <= 1_000_000)
            .ok_or_else(|| error("sequence tick is invalid"))?;
        if previous.is_some_and(|prior| tick <= prior) {
            return invalid("sequence ticks must be strictly increasing and unique");
        }
        previous = Some(tick);
        let sample = exact_object(
            sample_value,
            &[
                "sample_time_ticks",
                "evaluated_nodes",
                "evaluated_pose_sha256",
            ],
            "mechanical pose sequence sample",
        )?;
        if sample.get("sample_time_ticks") != Some(tick_value) {
            return invalid("sequence sample tick differs from the ordered tick list");
        }
        sha(sample, "evaluated_pose_sha256")?;
        let expected_single_request = sequence_single_request(expected, tick);
        let expected_single = evaluate_single(runtime, &expected_single_request)?;
        for field in [
            "rest_frame",
            "rest_frame_sha256",
            "parent_map_sha256",
            "pose_action",
            "pose_action_sha256",
            "evaluation_policy",
            "evaluation_policy_sha256",
            "evaluation_order",
        ] {
            if object.get(field) != expected_single.get(field) {
                return invalid(&format!(
                    "mechanical pose sequence normalized result differs from closed request field {field}"
                ));
            }
        }
        if sample.get("evaluated_nodes") != expected_single.get("evaluated_nodes")
            || sample.get("evaluated_pose_sha256") != expected_single.get("evaluated_pose_sha256")
        {
            return invalid(
                "mechanical pose sequence sample differs from closed request evaluation",
            );
        }
        let mut synthetic = json!({
            "schema_version":"MechanicalPoseEvaluationResult@1",
            "project_id":object["project_id"],
            "artifact_id":object["artifact_id"],
            "candidate_id":object["candidate_id"],
            "artifact_readback_sha256":object["artifact_readback_sha256"],
            "program_sha256":object["program_sha256"],
            "operator_catalog_sha256":object["operator_catalog_sha256"],
            "readback_config_sha256":object["readback_config_sha256"],
            "input_sha256":object["input_sha256"],
            "rest_frame":object["rest_frame"],
            "rest_frame_sha256":object["rest_frame_sha256"],
            "parent_map_sha256":object["parent_map_sha256"],
            "pose_action":object["pose_action"],
            "pose_action_sha256":object["pose_action_sha256"],
            "sample_time_ticks":sample["sample_time_ticks"],
            "evaluation_policy":object["evaluation_policy"],
            "evaluation_policy_sha256":object["evaluation_policy_sha256"],
            "evaluation_order":object["evaluation_order"],
            "evaluated_nodes":sample["evaluated_nodes"],
            "evaluated_pose_sha256":sample["evaluated_pose_sha256"],
            "geometry_materialization":"not-materialized",
            "worker_evaluation":"not-run-runtime-read-only-projection",
            "validator_status":"passed",
            "quality_status":"structural_only",
            "limitations":[
                "mechanical-rigid-links-only",
                "single-scalar-dof-per-link",
                "no-skinning-or-mesh-deformation",
                "no-ik-constraints-nla-fcurves-or-drivers",
                "no-geometry-materialization",
                "no-cross-platform-bit-exact-claim",
                "candidate-not-created",
                "visual-quality-not-evaluated"
            ],
            "canonical_sha256":""
        });
        synthetic["canonical_sha256"] = Value::String(canonical_json_hash(&synthetic));
        validate_result(&synthetic)?;
    }
    let identity = json!({
        "schema_version":"MechanicalPoseSequenceIdentity@1",
        "project_id":object["project_id"],
        "artifact_id":object["artifact_id"],
        "candidate_id":object["candidate_id"],
        "artifact_readback_sha256":object["artifact_readback_sha256"],
        "program_sha256":object["program_sha256"],
        "operator_catalog_sha256":object["operator_catalog_sha256"],
        "readback_config_sha256":object["readback_config_sha256"],
        "input_sha256":object["input_sha256"],
        "rest_frame_sha256":object["rest_frame_sha256"],
        "pose_action_sha256":object["pose_action_sha256"],
        "evaluation_policy_sha256":object["evaluation_policy_sha256"],
        "sample_time_ticks":object["sample_time_ticks"],
        "samples":object["samples"]
    });
    if canonical_json_hash(&identity) != text(object, "sequence_sha256")? {
        return invalid("sequence_sha256 differs from the ordered pose sequence identity");
    }
    validate_embedded_canonical_hash(object, "mechanical pose sequence result")
}

fn validate_pose(value: Option<&Value>, scope: &str) -> Result<(), RuntimeError> {
    let pose = exact_object(
        value.ok_or_else(|| error(format!("{scope} is required")))?,
        &["translation_m", "rotation_quat_xyzw", "scale"],
        scope,
    )?;
    vec3(pose.get("translation_m"), -640.0, 640.0, "translation_m")?;
    canonical_quaternion(pose.get("rotation_quat_xyzw"), "rotation_quat_xyzw")?;
    if pose.get("scale") != Some(&json!([1.0, 1.0, 1.0])) {
        return invalid("mechanical pose scale must remain unit");
    }
    Ok(())
}

fn validate_embedded_canonical_hash(
    object: &Map<String, Value>,
    scope: &str,
) -> Result<(), RuntimeError> {
    let expected = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| error(format!("{scope} canonical_sha256 is invalid")))?;
    let mut preimage = Value::Object(object.clone());
    preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&preimage) != expected {
        return invalid(&format!("{scope} canonical_sha256 drifted"));
    }
    Ok(())
}

fn error(message: impl Into<String>) -> RuntimeError {
    RuntimeError::InvalidInput(format!("{ERROR}: {}", message.into()))
}

fn invalid<T>(message: &str) -> Result<T, RuntimeError> {
    Err(error(message))
}
