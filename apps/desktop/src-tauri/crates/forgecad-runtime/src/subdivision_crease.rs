//! Bounded clean-room crease-aware subdivision authoring projection.
//!
//! This module deliberately supports only a regular rectangular open quad grid,
//! one product-owned integer-level edge-sharpness rule and one fixed boundary
//! policy.  It does not link Blender or OpenSubdiv, expose arbitrary topology,
//! or persist a candidate.  Actual mesh evaluation remains in the isolated
//! Geometry Worker through `forgecad.geometry.subd-cage@2`.

use super::{
    canonical_json_bytes, canonical_json_hash, hash_geometry_program_with_runtime_worker,
    operator_catalog_sha256, required_value_id, required_value_sha, validate_request_keys,
    RuntimeError,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub(crate) const REQUEST_SCHEMA: &str = "SubdivisionCreaseEvaluationRequest@1";
pub(crate) const RESULT_SCHEMA: &str = "SubdivisionCreaseEvaluationResult@1";
const ERROR: &str = "SUBDIVISION_CREASE_EVALUATION_INVALID";
const MAX_RESULT_BYTES: usize = 1024 * 1024;
const LIMITATIONS: [&str; 10] = [
    "regular_rectangular_open_quad_grid_only",
    "integer_edge_sharpness_levels_1_to_2_only",
    "fractional_and_vertex_creases_unsupported",
    "adaptive_subdivision_unsupported",
    "limit_surface_not_evaluated",
    "face_varying_uv_not_interpolated",
    "root_lineage_requires_separate_subdivision_topology_lineage_preview",
    "compiled_geometry_not_created_by_read_only_projection",
    "candidate_not_created",
    "visual_quality_not_evaluated",
];

pub(crate) fn evaluate(object: &Map<String, Value>) -> Result<Value, RuntimeError> {
    validate_request_keys(
        object,
        &[
            "schema_version",
            "project_id",
            "representation_plan_sha256",
            "part_id",
            "material_zone_id",
            "solid",
            "control_cage",
            "crease_edges",
            "policy",
            "transform",
            "budgets",
            "input_sha256",
        ],
        "subdivision_crease_evaluation",
    )?;
    if object.len() != 12
        || object.get("schema_version").and_then(Value::as_str) != Some(REQUEST_SCHEMA)
    {
        return invalid("required fields or schema version are invalid");
    }
    let project_id = required_value_id(object.get("project_id"), "project_id")?;
    let representation_plan_sha256 = required_value_sha(
        object.get("representation_plan_sha256"),
        "representation_plan_sha256",
    )?;
    let part_id = required_value_id(object.get("part_id"), "part_id")?;
    let material_zone_id = required_value_id(object.get("material_zone_id"), "material_zone_id")?;
    if object.get("solid").and_then(Value::as_bool) != Some(false) {
        return invalid("regular control cages are open surfaces and solid must be false");
    }

    let control_cage = exact_object(
        object.get("control_cage"),
        &["u_points", "v_points", "control_points"],
        "control_cage",
    )?;
    let u_points = bounded_u64(control_cage.get("u_points"), 3, 16, "u_points")?;
    let v_points = bounded_u64(control_cage.get("v_points"), 3, 16, "v_points")?;
    let control_points = control_cage
        .get("control_points")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(format!("{ERROR}: control_points must be an array"))
        })?;
    let expected_control_points = u_points.checked_mul(v_points).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("{ERROR}: control point count overflow"))
    })?;
    if control_points.len() as u64 != expected_control_points {
        return invalid("control_points count must equal u_points * v_points");
    }
    for point in control_points {
        let coordinates = point
            .as_array()
            .filter(|items| items.len() == 3)
            .ok_or_else(|| {
                RuntimeError::InvalidInput(format!("{ERROR}: control point must be vec3"))
            })?;
        if coordinates.iter().any(|coordinate| {
            coordinate
                .as_f64()
                .is_none_or(|number| !number.is_finite() || !(-10.0..=10.0).contains(&number))
        }) {
            return invalid("control point is non-finite or outside the coordinate envelope");
        }
    }

    let policy = exact_object(
        object.get("policy"),
        &[
            "scheme",
            "subdivision_levels",
            "boundary_interpolation",
            "crease_method",
            "sharpness_domain",
            "face_varying_interpolation",
            "limit_surface",
            "adaptive",
        ],
        "policy",
    )?;
    let subdivision_levels =
        bounded_u64(policy.get("subdivision_levels"), 1, 2, "subdivision_levels")?;
    if policy.get("scheme").and_then(Value::as_str)
        != Some("catmull-clark-uniform-regular-quad-grid")
        || policy.get("boundary_interpolation").and_then(Value::as_str) != Some("edge-only")
        || policy.get("crease_method").and_then(Value::as_str)
            != Some("uniform-integer-level-decay@1")
        || policy.get("sharpness_domain").and_then(Value::as_str) != Some("integer-levels-1-to-2")
        || policy
            .get("face_varying_interpolation")
            .and_then(Value::as_str)
            != Some("worker-triangle-chart-postprocess")
        || policy.get("limit_surface").and_then(Value::as_bool) != Some(false)
        || policy.get("adaptive").and_then(Value::as_bool) != Some(false)
    {
        return invalid("unsupported crease subdivision policy");
    }

    let normalized_creases = validate_and_normalize_creases(
        object.get("crease_edges"),
        u_points as usize,
        v_points as usize,
    )?;
    let transform = exact_object(
        object.get("transform"),
        &["position_m", "rotation_rad"],
        "transform",
    )?;
    validate_vec3(transform.get("position_m"), -10.0, 10.0, "position_m")?;
    validate_vec3(
        transform.get("rotation_rad"),
        -std::f64::consts::TAU,
        std::f64::consts::TAU,
        "rotation_rad",
    )?;
    let budgets = exact_object(
        object.get("budgets"),
        &[
            "max_nodes",
            "max_triangles",
            "max_glb_bytes",
            "max_worker_memory_bytes",
            "max_runtime_ms",
        ],
        "budgets",
    )?;
    let max_triangles = bounded_u64(budgets.get("max_triangles"), 1, 250_000, "max_triangles")?;

    let mut normalized_input = Value::Object(object.clone());
    normalized_input
        .as_object_mut()
        .expect("request is an object")
        .remove("input_sha256");
    normalized_input["crease_edges"] = Value::Array(normalized_creases.clone());
    let expected_input_sha256 = canonical_json_hash(&normalized_input);
    let input_sha256 = required_value_sha(object.get("input_sha256"), "input_sha256")?;
    if input_sha256 != expected_input_sha256 {
        return Err(RuntimeError::InvalidInput(format!(
            "SUBDIVISION_CREASE_EVALUATION_INPUT_HASH_MISMATCH: expected={expected_input_sha256} actual={input_sha256}"
        )));
    }

    let scale = 1u64 << subdivision_levels;
    let evaluated_u_points = (u_points - 1)
        .checked_mul(scale)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{ERROR}: topology overflow")))?;
    let evaluated_v_points = (v_points - 1)
        .checked_mul(scale)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{ERROR}: topology overflow")))?;
    let control_quads = (u_points - 1) * (v_points - 1);
    let control_edges = u_points * (v_points - 1) + v_points * (u_points - 1);
    let evaluated_quads = (evaluated_u_points - 1) * (evaluated_v_points - 1);
    let evaluated_triangles = evaluated_quads
        .checked_mul(2)
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{ERROR}: triangle count overflow")))?;
    if evaluated_triangles > max_triangles {
        return Err(RuntimeError::InvalidInput(format!(
            "SUBDIVISION_CREASE_EVALUATION_BUDGET_EXCEEDED: predicted_triangles={evaluated_triangles} max_triangles={max_triangles}"
        )));
    }
    let evaluated_vertices = evaluated_u_points * evaluated_v_points;
    let boundary_edges = 2 * ((evaluated_u_points - 1) + (evaluated_v_points - 1));
    let level_2_crease_application_count = if subdivision_levels == 2 {
        normalized_creases
            .iter()
            .filter(|edge| edge["sharpness_levels"].as_u64() == Some(2))
            .count() as u64
            * 2
    } else {
        0
    };
    let predicted_topology = json!({
        "control_vertex_count":expected_control_points,
        "control_edge_count":control_edges,
        "control_quad_count":control_quads,
        "control_crease_edge_count":normalized_creases.len(),
        "level_1_crease_application_count":normalized_creases.len(),
        "level_2_crease_application_count":level_2_crease_application_count,
        "evaluated_u_points":evaluated_u_points,
        "evaluated_v_points":evaluated_v_points,
        "evaluated_vertex_count":evaluated_vertices,
        "evaluated_quad_count":evaluated_quads,
        "evaluated_triangle_count":evaluated_triangles,
        "boundary_edge_count":boundary_edges
    });

    let draft = json!({
        "schema_version":"GeometryProgram@2",
        "project_id":project_id,
        "representation_plan_sha256":representation_plan_sha256,
        "operator_catalog_sha256":operator_catalog_sha256(),
        "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
        "budgets":budgets,
        "nodes":[{
            "node_id":"subdivision-crease-control-cage",
            "operator_id":"forgecad.geometry.subd-cage@2",
            "inputs":[],
            "parameters":{
                "shape":"subd-cage",
                "control_points":control_points,
                "u_points":u_points,
                "v_points":v_points,
                "subdivision_levels":subdivision_levels,
                "crease_method":"uniform-integer-level-decay@1",
                "crease_edges":normalized_creases,
                "position_m":transform.get("position_m").expect("transform checked"),
                "rotation_rad":transform.get("rotation_rad").expect("transform checked")
            }
        }],
        "part_outputs":[{
            "part_id":part_id,
            "input_node_ids":["subdivision-crease-control-cage"],
            "material_zone_id":material_zone_id,
            "solid":false
        }]
    });
    let hash_result = hash_geometry_program_with_runtime_worker(&draft).map_err(|error| {
        RuntimeError::InvalidInput(format!("SUBDIVISION_CREASE_EVALUATION_REJECTED: {error}"))
    })?;
    if hash_result
        .get("operator_catalog_sha256")
        .and_then(Value::as_str)
        != Some(operator_catalog_sha256().as_str())
    {
        return Err(RuntimeError::InvalidInput(
            "SUBDIVISION_CREASE_EVALUATION_REJECTED: GEOMETRY_WORKER_PROTOCOL catalog cohort mismatch"
                .to_owned(),
        ));
    }
    let program_sha256 = hash_result
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|hash| forgecad_contracts::is_sha256(hash))
        .ok_or_else(|| {
            RuntimeError::InvalidInput(
                "SUBDIVISION_CREASE_EVALUATION_PROGRAM_HASH_INVALID".to_owned(),
            )
        })?
        .to_owned();
    let mut geometry_program = draft;
    geometry_program["canonical_sha256"] = Value::String(program_sha256.clone());
    let crease_edges_sha256 = canonical_json_hash(&Value::Array(normalized_creases));
    let mut result = json!({
        "schema_version":RESULT_SCHEMA,
        "project_id":project_id,
        "representation_plan_sha256":representation_plan_sha256,
        "part_id":part_id,
        "material_zone_id":material_zone_id,
        "solid":false,
        "input_sha256":input_sha256,
        "control_cage_sha256":canonical_json_hash(object.get("control_cage").expect("checked")),
        "crease_edges_sha256":crease_edges_sha256,
        "evaluation_policy_sha256":canonical_json_hash(object.get("policy").expect("checked")),
        "predicted_topology_sha256":canonical_json_hash(&predicted_topology),
        "program_sha256":program_sha256,
        "operator_catalog_sha256":operator_catalog_sha256(),
        "predicted_topology":predicted_topology,
        "crease_policy":{
            "method":"uniform-integer-level-decay@1",
            "sharpness_domain":"integer-levels-1-to-2",
            "decay_per_level":1,
            "boundary_edges":"always-sharp",
            "boundary_vertices":"edge-only-crease-rule-not-corner-pinned",
            "junction_rule":"two-crease-neighbors-six-one-one-eighth-three-plus-corner"
        },
        "attribute_policy":{
            "normals":"worker-regenerated-smooth",
            "uv":"worker-triangle-chart-postprocess",
            "tangents":"worker-mikktspace-0.3.0-postprocess",
            "material_zone":"part-output-only"
        },
        "geometry_program":geometry_program,
        "validator_status":"passed",
        "validator_scope":"typed-policy-program-hash-and-worker-operator-validation",
        "quality_status":"structural_only",
        "limitations":LIMITATIONS,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    validate_result(object, &result)?;
    Ok(result)
}

pub(crate) fn validate_result(
    request: &Map<String, Value>,
    value: &Value,
) -> Result<(), RuntimeError> {
    let object = value
        .as_object()
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{ERROR}: result must be an object")))?;
    let fields = [
        "schema_version",
        "project_id",
        "representation_plan_sha256",
        "part_id",
        "material_zone_id",
        "solid",
        "input_sha256",
        "control_cage_sha256",
        "crease_edges_sha256",
        "evaluation_policy_sha256",
        "predicted_topology_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "predicted_topology",
        "crease_policy",
        "attribute_policy",
        "geometry_program",
        "validator_status",
        "validator_scope",
        "quality_status",
        "limitations",
        "canonical_sha256",
    ];
    validate_request_keys(object, &fields, "subdivision_crease_evaluation_result")?;
    if object.len() != fields.len()
        || object.get("schema_version").and_then(Value::as_str) != Some(RESULT_SCHEMA)
        || object.get("solid").and_then(Value::as_bool) != Some(false)
        || object.get("validator_status").and_then(Value::as_str) != Some("passed")
        || object.get("validator_scope").and_then(Value::as_str)
            != Some("typed-policy-program-hash-and-worker-operator-validation")
        || object.get("quality_status").and_then(Value::as_str) != Some("structural_only")
        || object.get("limitations") != Some(&json!(LIMITATIONS))
    {
        return invalid("result constants or field set drifted");
    }
    for key in [
        "representation_plan_sha256",
        "input_sha256",
        "control_cage_sha256",
        "crease_edges_sha256",
        "evaluation_policy_sha256",
        "predicted_topology_sha256",
        "program_sha256",
        "operator_catalog_sha256",
        "canonical_sha256",
    ] {
        required_value_sha(object.get(key), key)?;
    }
    if object
        .get("operator_catalog_sha256")
        .and_then(Value::as_str)
        != Some(operator_catalog_sha256().as_str())
    {
        return invalid("result operator catalog binding drifted");
    }
    for field in [
        "project_id",
        "representation_plan_sha256",
        "part_id",
        "material_zone_id",
        "solid",
        "input_sha256",
    ] {
        if object.get(field) != request.get(field) {
            return invalid("result identity or request hash binding drifted");
        }
    }
    let program = object
        .get("geometry_program")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(format!("{ERROR}: geometry_program is invalid"))
        })?;
    if program.get("canonical_sha256") != object.get("program_sha256")
        || program.get("operator_catalog_sha256") != object.get("operator_catalog_sha256")
        || program.get("project_id") != object.get("project_id")
        || program.get("representation_plan_sha256") != object.get("representation_plan_sha256")
    {
        return invalid("result program binding drifted");
    }
    let mut program_preimage = Value::Object(program.clone());
    program_preimage
        .as_object_mut()
        .expect("program is an object")
        .remove("canonical_sha256");
    if object.get("program_sha256") != Some(&Value::String(canonical_json_hash(&program_preimage)))
    {
        return invalid("result program canonical hash drifted");
    }
    let node = program
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| nodes.len() == 1)
        .and_then(|nodes| nodes[0].as_object())
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{ERROR}: program node is invalid")))?;
    if node.get("operator_id").and_then(Value::as_str) != Some("forgecad.geometry.subd-cage@2") {
        return invalid("result program does not use the crease-aware operator");
    }
    let parameters = node
        .get("parameters")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(format!("{ERROR}: node parameters are invalid"))
        })?;
    let u_points = bounded_u64(parameters.get("u_points"), 3, 16, "result u_points")?;
    let v_points = bounded_u64(parameters.get("v_points"), 3, 16, "result v_points")?;
    let subdivision_levels = bounded_u64(
        parameters.get("subdivision_levels"),
        1,
        2,
        "result subdivision_levels",
    )?;
    let control_points = parameters.get("control_points").ok_or_else(|| {
        RuntimeError::InvalidInput(format!("{ERROR}: result control points are missing"))
    })?;
    let crease_edges = parameters.get("crease_edges").ok_or_else(|| {
        RuntimeError::InvalidInput(format!("{ERROR}: result crease edges are missing"))
    })?;
    let normalized_creases =
        validate_and_normalize_creases(Some(crease_edges), u_points as usize, v_points as usize)?;
    if crease_edges != &Value::Array(normalized_creases.clone()) {
        return invalid("result program crease edges are not canonical");
    }
    let request_control_cage = request
        .get("control_cage")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(format!("{ERROR}: request control cage is invalid"))
        })?;
    let request_transform = request
        .get("transform")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(format!("{ERROR}: request transform is invalid"))
        })?;
    let request_policy = request
        .get("policy")
        .and_then(Value::as_object)
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{ERROR}: request policy is invalid")))?;
    let request_creases = validate_and_normalize_creases(
        request.get("crease_edges"),
        request_control_cage
            .get("u_points")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        request_control_cage
            .get("v_points")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
    )?;
    let expected_parameters = json!({
        "shape":"subd-cage",
        "control_points":request_control_cage.get("control_points").expect("request checked"),
        "u_points":request_control_cage.get("u_points").expect("request checked"),
        "v_points":request_control_cage.get("v_points").expect("request checked"),
        "subdivision_levels":request_policy.get("subdivision_levels").expect("request checked"),
        "crease_method":"uniform-integer-level-decay@1",
        "crease_edges":request_creases,
        "position_m":request_transform.get("position_m").expect("request checked"),
        "rotation_rad":request_transform.get("rotation_rad").expect("request checked")
    });
    if Value::Object(parameters.clone()) != expected_parameters
        || program.get("budgets") != request.get("budgets")
    {
        return invalid("result GeometryProgram is not bound to the originating request");
    }
    let expected_control_cage = json!({
        "u_points":u_points,
        "v_points":v_points,
        "control_points":control_points
    });
    let expected_policy = json!({
        "scheme":"catmull-clark-uniform-regular-quad-grid",
        "subdivision_levels":subdivision_levels,
        "boundary_interpolation":"edge-only",
        "crease_method":"uniform-integer-level-decay@1",
        "sharpness_domain":"integer-levels-1-to-2",
        "face_varying_interpolation":"worker-triangle-chart-postprocess",
        "limit_surface":false,
        "adaptive":false
    });
    let scale = 1u64 << subdivision_levels;
    let evaluated_u_points = (u_points - 1) * scale + 1;
    let evaluated_v_points = (v_points - 1) * scale + 1;
    let evaluated_quads = (evaluated_u_points - 1) * (evaluated_v_points - 1);
    let expected_topology = json!({
        "control_vertex_count":u_points * v_points,
        "control_edge_count":u_points * (v_points - 1) + v_points * (u_points - 1),
        "control_quad_count":(u_points - 1) * (v_points - 1),
        "control_crease_edge_count":normalized_creases.len(),
        "level_1_crease_application_count":normalized_creases.len(),
        "level_2_crease_application_count":if subdivision_levels == 2 {
            normalized_creases.iter().filter(|edge| edge["sharpness_levels"].as_u64() == Some(2)).count() * 2
        } else { 0 },
        "evaluated_u_points":evaluated_u_points,
        "evaluated_v_points":evaluated_v_points,
        "evaluated_vertex_count":evaluated_u_points * evaluated_v_points,
        "evaluated_quad_count":evaluated_quads,
        "evaluated_triangle_count":evaluated_quads * 2,
        "boundary_edge_count":2 * ((evaluated_u_points - 1) + (evaluated_v_points - 1))
    });
    if object.get("control_cage_sha256")
        != Some(&Value::String(canonical_json_hash(&expected_control_cage)))
        || object.get("control_cage_sha256")
            != Some(&Value::String(canonical_json_hash(
                request.get("control_cage").expect("request checked"),
            )))
        || object.get("crease_edges_sha256")
            != Some(&Value::String(canonical_json_hash(crease_edges)))
        || object.get("crease_edges_sha256")
            != Some(&Value::String(canonical_json_hash(
                &expected_parameters["crease_edges"],
            )))
        || object.get("evaluation_policy_sha256")
            != Some(&Value::String(canonical_json_hash(&expected_policy)))
        || object.get("evaluation_policy_sha256")
            != Some(&Value::String(canonical_json_hash(
                request.get("policy").expect("request checked"),
            )))
        || object.get("predicted_topology") != Some(&expected_topology)
        || object.get("predicted_topology_sha256")
            != Some(&Value::String(canonical_json_hash(&expected_topology)))
    {
        return invalid("result control, crease, policy or topology semantic binding drifted");
    }
    if object.get("crease_policy")
        != Some(&json!({
            "method":"uniform-integer-level-decay@1",
            "sharpness_domain":"integer-levels-1-to-2",
            "decay_per_level":1,
            "boundary_edges":"always-sharp",
            "boundary_vertices":"edge-only-crease-rule-not-corner-pinned",
            "junction_rule":"two-crease-neighbors-six-one-one-eighth-three-plus-corner"
        }))
        || object.get("attribute_policy")
            != Some(&json!({
                "normals":"worker-regenerated-smooth",
                "uv":"worker-triangle-chart-postprocess",
                "tangents":"worker-mikktspace-0.3.0-postprocess",
                "material_zone":"part-output-only"
            }))
    {
        return invalid("result policy constants drifted");
    }
    let part_output = program
        .get("part_outputs")
        .and_then(Value::as_array)
        .filter(|outputs| outputs.len() == 1)
        .and_then(|outputs| outputs[0].as_object())
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{ERROR}: part output is invalid")))?;
    if part_output.get("part_id") != object.get("part_id")
        || part_output.get("material_zone_id") != object.get("material_zone_id")
        || part_output.get("solid").and_then(Value::as_bool) != Some(false)
        || part_output.get("input_node_ids") != Some(&json!(["subdivision-crease-control-cage"]))
    {
        return invalid("result Part or MaterialZone binding drifted");
    }
    let actual_canonical = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .expect("SHA checked");
    let mut preimage = Value::Object(object.clone());
    preimage["canonical_sha256"] = Value::String(String::new());
    if actual_canonical != canonical_json_hash(&preimage) {
        return invalid("result canonical hash drifted");
    }
    let result_bytes = canonical_json_bytes(value).map_err(|_| {
        RuntimeError::InvalidInput(format!("{ERROR}: result canonicalization failed"))
    })?;
    if result_bytes.len() > MAX_RESULT_BYTES {
        return invalid("result exceeds the 1 MiB canonical response budget");
    }
    Ok(())
}

fn validate_and_normalize_creases(
    value: Option<&Value>,
    u_points: usize,
    v_points: usize,
) -> Result<Vec<Value>, RuntimeError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty() && items.len() <= 128)
        .ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "{ERROR}: crease_edges must contain 1..=128 entries"
            ))
        })?;
    let vertex_count = u_points * v_points;
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        let edge = exact_object(
            Some(value),
            &["vertex_a", "vertex_b", "sharpness_levels"],
            "crease_edge",
        )?;
        let vertex_a = bounded_u64(
            edge.get("vertex_a"),
            0,
            (vertex_count - 1) as u64,
            "vertex_a",
        )? as usize;
        let vertex_b = bounded_u64(
            edge.get("vertex_b"),
            0,
            (vertex_count - 1) as u64,
            "vertex_b",
        )? as usize;
        let sharpness_levels = bounded_u64(edge.get("sharpness_levels"), 1, 2, "sharpness_levels")?;
        if vertex_a >= vertex_b {
            return invalid("crease endpoints must be strictly ascending");
        }
        let a_row = vertex_a / u_points;
        let a_column = vertex_a % u_points;
        let b_row = vertex_b / u_points;
        let b_column = vertex_b % u_points;
        if !((a_row == b_row && b_column == a_column + 1)
            || (a_column == b_column && b_row == a_row + 1))
        {
            return invalid("crease endpoints must identify one control-grid edge");
        }
        let boundary = (a_row == b_row && (a_row == 0 || a_row + 1 == v_points))
            || (a_column == b_column && (a_column == 0 || a_column + 1 == u_points));
        if boundary {
            return invalid("explicit boundary creases are redundant and rejected");
        }
        if !seen.insert((vertex_a, vertex_b)) {
            return invalid("crease edges must be unique");
        }
        normalized.push(json!({
            "vertex_a":vertex_a,
            "vertex_b":vertex_b,
            "sharpness_levels":sharpness_levels
        }));
    }
    normalized.sort_by_key(|edge| {
        (
            edge["vertex_a"].as_u64().unwrap_or_default(),
            edge["vertex_b"].as_u64().unwrap_or_default(),
        )
    });
    Ok(normalized)
}

fn exact_object<'a>(
    value: Option<&'a Value>,
    fields: &[&str],
    context: &str,
) -> Result<&'a Map<String, Value>, RuntimeError> {
    let object = value.and_then(Value::as_object).ok_or_else(|| {
        RuntimeError::InvalidInput(format!("{ERROR}: {context} must be an object"))
    })?;
    validate_request_keys(object, fields, context)?;
    if object.len() != fields.len() || !fields.iter().all(|field| object.contains_key(*field)) {
        return invalid(&format!("{context} fields are incomplete"));
    }
    Ok(object)
}

fn bounded_u64(
    value: Option<&Value>,
    minimum: u64,
    maximum: u64,
    context: &str,
) -> Result<u64, RuntimeError> {
    value
        .and_then(Value::as_u64)
        .filter(|number| (minimum..=maximum).contains(number))
        .ok_or_else(|| {
            RuntimeError::InvalidInput(format!(
                "{ERROR}: {context} must be an integer in {minimum}..={maximum}"
            ))
        })
}

fn validate_vec3(
    value: Option<&Value>,
    minimum: f64,
    maximum: f64,
    context: &str,
) -> Result<(), RuntimeError> {
    let values = value
        .and_then(Value::as_array)
        .filter(|items| items.len() == 3)
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{ERROR}: {context} must be vec3")))?;
    if values.iter().any(|value| {
        value
            .as_f64()
            .is_none_or(|number| !number.is_finite() || !(minimum..=maximum).contains(&number))
    }) {
        return invalid(&format!("{context} is non-finite or outside bounds"));
    }
    Ok(())
}

fn invalid<T>(message: &str) -> Result<T, RuntimeError> {
    Err(RuntimeError::InvalidInput(format!("{ERROR}: {message}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile_geometry_program, integrity, Runtime};

    fn request(project_id: &str, crease_edges: Vec<Value>) -> Value {
        let points = vec![
            json!([-1.0, -1.0, 0.0]),
            json!([0.0, -1.0, 0.0]),
            json!([1.0, -1.0, 0.0]),
            json!([-1.0, 0.0, 0.0]),
            json!([0.0, 0.0, 1.0]),
            json!([1.0, 0.0, 0.0]),
            json!([-1.0, 1.0, 0.0]),
            json!([0.0, 1.0, 0.0]),
            json!([1.0, 1.0, 0.0]),
        ];
        let mut request = json!({
            "schema_version":REQUEST_SCHEMA,
            "project_id":project_id,
            "representation_plan_sha256":"7".repeat(64),
            "part_id":"subd-crease-shell",
            "material_zone_id":"zone-shell",
            "solid":false,
            "control_cage":{"u_points":3,"v_points":3,"control_points":points},
            "crease_edges":crease_edges,
            "policy":{
                "scheme":"catmull-clark-uniform-regular-quad-grid",
                "subdivision_levels":2,
                "boundary_interpolation":"edge-only",
                "crease_method":"uniform-integer-level-decay@1",
                "sharpness_domain":"integer-levels-1-to-2",
                "face_varying_interpolation":"worker-triangle-chart-postprocess",
                "limit_surface":false,
                "adaptive":false
            },
            "transform":{"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]},
            "budgets":{"max_nodes":1,"max_triangles":128,"max_glb_bytes":16777216,"max_worker_memory_bytes":134217728,"max_runtime_ms":5000},
            "input_sha256":""
        });
        rebind(&mut request);
        request
    }

    fn rebind(request: &mut Value) {
        let mut binding = request.clone();
        binding
            .as_object_mut()
            .expect("request object")
            .remove("input_sha256");
        let creases = binding["crease_edges"]
            .as_array_mut()
            .expect("crease array");
        creases.sort_by_key(|edge| {
            (
                edge["vertex_a"].as_u64().unwrap_or_default(),
                edge["vertex_b"].as_u64().unwrap_or_default(),
            )
        });
        request["input_sha256"] = Value::String(canonical_json_hash(&binding));
    }

    #[test]
    fn crease_evaluation_is_deterministic_compilable_reorder_stable_and_read_only() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("Subdivision crease evaluation", json!({"profile":"mvp"}))
            .expect("project");
        let before = json!({
            "candidates":runtime.candidates(&project.project_id).expect("candidates"),
            "versions":runtime.versions(Some(&project.project_id)).expect("versions"),
            "cas":runtime.store.cas().list_objects().expect("CAS inventory")
        });
        let first_request = request(
            &project.project_id,
            vec![
                json!({"vertex_a":4,"vertex_b":5,"sharpness_levels":2}),
                json!({"vertex_a":3,"vertex_b":4,"sharpness_levels":1}),
            ],
        );
        let first = runtime
            .geometry_program_hash(&first_request)
            .expect("crease evaluation");
        let repeat = runtime
            .geometry_program_hash(&first_request)
            .expect("crease evaluation repeat");
        assert_eq!(first, repeat);
        assert_eq!(first["schema_version"], RESULT_SCHEMA);
        assert_eq!(first["predicted_topology"]["evaluated_triangle_count"], 128);
        assert_eq!(first["predicted_topology"]["evaluated_vertex_count"], 81);
        assert_eq!(
            first["predicted_topology"]["level_1_crease_application_count"],
            2
        );
        assert_eq!(
            first["predicted_topology"]["level_2_crease_application_count"],
            2
        );
        assert_eq!(first["quality_status"], "structural_only");
        validate_result(first_request.as_object().expect("request object"), &first)
            .expect("result semantics");

        let reordered = request(
            &project.project_id,
            vec![
                json!({"vertex_a":3,"vertex_b":4,"sharpness_levels":1}),
                json!({"vertex_a":4,"vertex_b":5,"sharpness_levels":2}),
            ],
        );
        assert_eq!(first_request["input_sha256"], reordered["input_sha256"]);
        assert_eq!(
            first,
            runtime
                .geometry_program_hash(&reordered)
                .expect("reordered canonical evaluation")
        );

        let artifact = compile_geometry_program(&first["geometry_program"])
            .expect("crease program must actually compile");
        assert_eq!(artifact.triangle_count, 128);
        let readback = integrity::inspect_glb(&artifact.glb).expect("strict GLB readback");
        assert!(readback.hard_gate_passed, "{:?}", readback.failure_codes);
        assert_eq!(readback.triangle_count, 128);

        let after = json!({
            "candidates":runtime.candidates(&project.project_id).expect("candidates"),
            "versions":runtime.versions(Some(&project.project_id)).expect("versions"),
            "cas":runtime.store.cas().list_objects().expect("CAS inventory")
        });
        assert_eq!(before, after, "crease evaluation must remain read-only");
    }

    #[test]
    fn crease_evaluation_rejects_policy_edge_budget_hash_and_result_forgery() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("Subdivision crease negatives", json!({"profile":"mvp"}))
            .expect("project");
        let valid = request(
            &project.project_id,
            vec![json!({"vertex_a":3,"vertex_b":4,"sharpness_levels":1})],
        );
        let result = runtime.geometry_program_hash(&valid).expect("valid result");

        for mut invalid in [
            {
                let mut value = valid.clone();
                value["crease_edges"] = json!([{"vertex_a":0,"vertex_b":1,"sharpness_levels":1}]);
                value
            },
            {
                let mut value = valid.clone();
                value["crease_edges"] = json!([{"vertex_a":1,"vertex_b":7,"sharpness_levels":1}]);
                value
            },
            {
                let mut value = valid.clone();
                value["crease_edges"] = json!([
                    {"vertex_a":3,"vertex_b":4,"sharpness_levels":1},
                    {"vertex_a":3,"vertex_b":4,"sharpness_levels":2}
                ]);
                value
            },
            {
                let mut value = valid.clone();
                value["crease_edges"][0]["sharpness_levels"] = json!(1.5);
                value
            },
            {
                let mut value = valid.clone();
                value["policy"]["adaptive"] = json!(true);
                value
            },
            {
                let mut value = valid.clone();
                value["policy"]["boundary_interpolation"] = json!("edge-and-corner");
                value
            },
            {
                let mut value = valid.clone();
                value["budgets"]["max_triangles"] = json!(127);
                value
            },
            {
                let mut value = valid.clone();
                value["policy"]["script"] = json!("bpy");
                value
            },
        ] {
            rebind(&mut invalid);
            assert!(runtime.geometry_program_hash(&invalid).is_err());
        }

        let mut drift = valid.clone();
        drift["crease_edges"][0]["sharpness_levels"] = json!(2);
        assert!(runtime
            .geometry_program_hash(&drift)
            .expect_err("stale normalized input hash")
            .to_string()
            .contains("INPUT_HASH_MISMATCH"));

        let mut forged = result;
        forged["predicted_topology"]["level_2_crease_application_count"] = json!(99);
        forged["predicted_topology_sha256"] =
            Value::String(canonical_json_hash(&forged["predicted_topology"]));
        forged["canonical_sha256"] = json!("");
        forged["canonical_sha256"] = Value::String(canonical_json_hash(&forged));
        assert!(validate_result(valid.as_object().expect("request object"), &forged).is_err());
    }

    #[test]
    fn crease_program_runs_through_runtime_prepare_and_strict_readback() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project(
                "Subdivision crease Runtime prepare",
                json!({"profile":"mvp"}),
            )
            .expect("project");
        let request = request(
            &project.project_id,
            vec![
                json!({"vertex_a":3,"vertex_b":4,"sharpness_levels":1}),
                json!({"vertex_a":4,"vertex_b":5,"sharpness_levels":2}),
            ],
        );
        let projection = runtime
            .geometry_program_hash(&request)
            .expect("crease authoring projection");
        let prepared = runtime
            .prepare_geometry_candidate(
                &project.project_id,
                None,
                json!({
                    "typed":"geometry",
                    "geometry_program":projection["geometry_program"].clone()
                }),
            )
            .expect("real crease geometry prepare");

        assert_eq!(prepared["schema_version"], "GeometryPrepareResult@2");
        assert_eq!(prepared["artifact"]["triangle_count"], 128);
        assert_eq!(prepared["artifact"]["hard_gate_passed"], true);
        assert_eq!(
            prepared["artifact"]["program_sha256"],
            projection["program_sha256"]
        );
        assert_eq!(
            prepared["artifact"]["operator_catalog_sha256"],
            projection["operator_catalog_sha256"]
        );
        assert_eq!(
            runtime
                .candidates(&project.project_id)
                .expect("prepared candidate")
                .len(),
            1
        );
        assert!(runtime
            .versions(Some(&project.project_id))
            .expect("versions")
            .is_empty());
    }
}
