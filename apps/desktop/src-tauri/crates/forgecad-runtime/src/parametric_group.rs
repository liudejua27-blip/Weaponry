//! First-party, declarative geometry-group templates.
//!
//! This is a clean-room adaptation of the reusable-group idea, not a Blender
//! node runtime. Definitions are compiled into ForgeCAD, instances expose only
//! closed typed sockets, and the only executable result is an existing
//! `GeometryProgram@2` validated by the fixed Geometry Worker. No template can
//! carry code, paths, URLs, environment access, network access, or nested
//! groups.

use forgecad_contracts::{is_opaque_id, is_sha256};
use forgecad_core::canonical_json_hash;
use serde_json::{json, Map, Value};

pub(crate) const REQUEST_SCHEMA: &str = "ParametricDesignKitRequest@2";
pub(crate) const RESULT_SCHEMA: &str = "ParametricDesignKitProgram@2";

const ROUNDED_BOX: &str = "forgecad.group.rounded-box@1";
const MIRRORED_BOX: &str = "forgecad.group.mirrored-box@1";
const ARRAYED_CYLINDER: &str = "forgecad.group.arrayed-cylinder@1";

const LIMITATIONS: [&str; 7] = [
    "candidate_not_created",
    "runtime_write_not_performed",
    "worker_receives_geometry_program_only",
    "nested_groups_not_supported",
    "fields_attributes_simulation_not_supported",
    "visual_quality_not_evaluated",
    "user_approval_required_before_geometry_prepare",
];

pub(crate) struct Expansion {
    pub(crate) template_id: String,
    pub(crate) template_definition: Value,
    pub(crate) template_sha256: String,
    pub(crate) template_catalog_sha256: String,
    pub(crate) project_id: String,
    pub(crate) representation_plan_sha256: String,
    pub(crate) instance_id: String,
    pub(crate) part_id: String,
    pub(crate) material_zone_id: String,
    pub(crate) parameters: Value,
    pub(crate) parameters_sha256: String,
    pub(crate) instance_sha256: String,
    pub(crate) geometry_program_draft: Value,
    pub(crate) evaluation_order: Value,
    pub(crate) source_map: Value,
}

pub(crate) fn expand(
    object: &Map<String, Value>,
    operator_catalog_sha256: &str,
) -> Result<Expansion, String> {
    exact_keys(
        object,
        &[
            "schema_version",
            "project_id",
            "representation_plan_sha256",
            "template_id",
            "instance_id",
            "part_id",
            "material_zone_id",
            "parameters",
            "input_sha256",
        ],
        "request",
    )?;
    if string(object, "schema_version")? != REQUEST_SCHEMA {
        return Err("PARAMETRIC_GROUP_INVALID: unsupported schema_version".to_owned());
    }
    let project_id = identifier(object, "project_id")?.to_owned();
    let representation_plan_sha256 = sha256(object, "representation_plan_sha256")?.to_owned();
    let template_id = string(object, "template_id")?.to_owned();
    let instance_id = identifier(object, "instance_id")?.to_owned();
    let part_id = identifier(object, "part_id")?.to_owned();
    let material_zone_id = identifier(object, "material_zone_id")?.to_owned();
    let parameters = object
        .get("parameters")
        .and_then(Value::as_object)
        .ok_or_else(|| "PARAMETRIC_GROUP_INVALID: parameters must be an object".to_owned())?;
    validate_parameters(&template_id, parameters)?;
    let input_sha256 = sha256(object, "input_sha256")?;
    let mut input_binding = Value::Object(object.clone());
    input_binding
        .as_object_mut()
        .expect("request clone is an object")
        .remove("input_sha256");
    let expected_input_sha256 = canonical_json_hash(&input_binding);
    if input_sha256 != expected_input_sha256 {
        return Err(format!(
            "PARAMETRIC_GROUP_INPUT_HASH_MISMATCH: expected={expected_input_sha256} actual={input_sha256}"
        ));
    }
    if !is_sha256(operator_catalog_sha256) {
        return Err("PARAMETRIC_GROUP_INVALID: operator catalog hash is invalid".to_owned());
    }

    let template_definition = template_definition(&template_id)?;
    let template_sha256 = template_definition
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .expect("template definition has a canonical hash")
        .to_owned();
    let template_catalog_sha256 = template_catalog_sha256();
    let parameters = Value::Object(parameters.clone());
    let parameters_sha256 = canonical_json_hash(&parameters);
    let instance_sha256 = canonical_json_hash(&json!({
        "schema_version":"ForgeCADGeometryGroupInstanceIdentity@1",
        "template_sha256":template_sha256,
        "template_catalog_sha256":template_catalog_sha256,
        "operator_catalog_sha256":operator_catalog_sha256,
        "project_id":project_id,
        "representation_plan_sha256":representation_plan_sha256,
        "instance_id":instance_id,
        "part_id":part_id,
        "material_zone_id":material_zone_id,
        "solid":true,
        "parameters_sha256":parameters_sha256
    }));
    let (nodes, evaluation_order, source_map) =
        lower_nodes(&template_id, parameters.as_object().unwrap())?;
    let final_node_id = evaluation_order
        .as_array()
        .and_then(|items| items.last())
        .and_then(Value::as_str)
        .expect("all templates have an output node");
    let geometry_program_draft = json!({
        "schema_version":"GeometryProgram@2",
        "project_id":project_id,
        "representation_plan_sha256":representation_plan_sha256,
        "operator_catalog_sha256":operator_catalog_sha256,
        "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
        "budgets":{"max_nodes":3,"max_triangles":100000,"max_glb_bytes":16777216,"max_worker_memory_bytes":134217728,"max_runtime_ms":5000},
        "nodes":nodes,
        "part_outputs":[{"part_id":part_id,"input_node_ids":[final_node_id],"material_zone_id":material_zone_id,"solid":true}]
    });

    Ok(Expansion {
        template_id,
        template_definition,
        template_sha256,
        template_catalog_sha256,
        project_id,
        representation_plan_sha256,
        instance_id,
        part_id,
        material_zone_id,
        parameters,
        parameters_sha256,
        instance_sha256,
        geometry_program_draft,
        evaluation_order,
        source_map,
    })
}

pub(crate) fn finalize(
    expansion: Expansion,
    program_sha256: String,
    geometry_program: Value,
    operator_catalog_sha256: &str,
) -> Result<Value, String> {
    if !is_sha256(&program_sha256) || !is_sha256(operator_catalog_sha256) {
        return Err("PARAMETRIC_GROUP_GEOMETRY_HASH_INVALID".to_owned());
    }
    let mut result = json!({
        "schema_version":RESULT_SCHEMA,
        "template_id":expansion.template_id,
        "template_definition":expansion.template_definition,
        "template_sha256":expansion.template_sha256,
        "template_catalog_sha256":expansion.template_catalog_sha256,
        "operator_catalog_sha256":operator_catalog_sha256,
        "project_id":expansion.project_id,
        "representation_plan_sha256":expansion.representation_plan_sha256,
        "instance_id":expansion.instance_id,
        "part_id":expansion.part_id,
        "material_zone_id":expansion.material_zone_id,
        "solid":true,
        "parameters":expansion.parameters,
        "parameters_sha256":expansion.parameters_sha256,
        "instance_sha256":expansion.instance_sha256,
        "program_sha256":program_sha256,
        "geometry_program":geometry_program,
        "evaluation_order":expansion.evaluation_order,
        "source_map":expansion.source_map,
        "validator_status":"passed",
        "quality_status":"structural_only",
        "runtime_write_performed":false,
        "candidate_created":false,
        "limitations":LIMITATIONS,
        "canonical_sha256":""
    });
    result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
    validate_result(&result, operator_catalog_sha256)?;
    Ok(result)
}

pub(crate) fn validate_result(
    value: &Value,
    expected_operator_catalog_sha256: &str,
) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| "PARAMETRIC_GROUP_RESULT_INVALID: result must be an object".to_owned())?;
    exact_keys(
        object,
        &[
            "schema_version",
            "template_id",
            "template_definition",
            "template_sha256",
            "template_catalog_sha256",
            "operator_catalog_sha256",
            "project_id",
            "representation_plan_sha256",
            "instance_id",
            "part_id",
            "material_zone_id",
            "solid",
            "parameters",
            "parameters_sha256",
            "instance_sha256",
            "program_sha256",
            "geometry_program",
            "evaluation_order",
            "source_map",
            "validator_status",
            "quality_status",
            "runtime_write_performed",
            "candidate_created",
            "limitations",
            "canonical_sha256",
        ],
        "result",
    )?;
    if string(object, "schema_version")? != RESULT_SCHEMA
        || string(object, "validator_status")? != "passed"
        || string(object, "quality_status")? != "structural_only"
        || object.get("solid").and_then(Value::as_bool) != Some(true)
        || object
            .get("runtime_write_performed")
            .and_then(Value::as_bool)
            != Some(false)
        || object.get("candidate_created").and_then(Value::as_bool) != Some(false)
    {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: result constants drifted".to_owned());
    }
    let template_id = string(object, "template_id")?;
    let expected_definition = template_definition(template_id)?;
    if object.get("template_definition") != Some(&expected_definition) {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: template definition drifted".to_owned());
    }
    let template_sha256 = sha256(object, "template_sha256")?;
    if expected_definition
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(template_sha256)
        || sha256(object, "template_catalog_sha256")? != template_catalog_sha256()
    {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: template hash drifted".to_owned());
    }
    let operator_catalog_sha256 = sha256(object, "operator_catalog_sha256")?;
    if operator_catalog_sha256 != expected_operator_catalog_sha256 {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: operator catalog cohort drifted".to_owned());
    }
    let project_id = identifier(object, "project_id")?;
    let representation_plan_sha256 = sha256(object, "representation_plan_sha256")?;
    let instance_id = identifier(object, "instance_id")?;
    let part_id = identifier(object, "part_id")?;
    let material_zone_id = identifier(object, "material_zone_id")?;
    let parameters = object
        .get("parameters")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "PARAMETRIC_GROUP_RESULT_INVALID: parameters must be an object".to_owned()
        })?;
    validate_parameters(template_id, parameters)?;
    let parameters_value = Value::Object(parameters.clone());
    let parameters_sha256 = sha256(object, "parameters_sha256")?;
    if canonical_json_hash(&parameters_value) != parameters_sha256 {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: parameter hash drifted".to_owned());
    }
    let expected_instance_sha256 = canonical_json_hash(&json!({
        "schema_version":"ForgeCADGeometryGroupInstanceIdentity@1",
        "template_sha256":template_sha256,
        "template_catalog_sha256":template_catalog_sha256(),
        "operator_catalog_sha256":operator_catalog_sha256,
        "project_id":project_id,
        "representation_plan_sha256":representation_plan_sha256,
        "instance_id":instance_id,
        "part_id":part_id,
        "material_zone_id":material_zone_id,
        "solid":true,
        "parameters_sha256":parameters_sha256
    }));
    if sha256(object, "instance_sha256")? != expected_instance_sha256 {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: instance hash drifted".to_owned());
    }
    let (nodes, evaluation_order, source_map) = lower_nodes(template_id, parameters)?;
    if object.get("evaluation_order") != Some(&evaluation_order)
        || object.get("source_map") != Some(&source_map)
    {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: evaluation/source map drifted".to_owned());
    }
    let program_sha256 = sha256(object, "program_sha256")?;
    let geometry_program = object
        .get("geometry_program")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            "PARAMETRIC_GROUP_RESULT_INVALID: geometry_program must be an object".to_owned()
        })?;
    exact_keys(
        geometry_program,
        &[
            "schema_version",
            "project_id",
            "representation_plan_sha256",
            "operator_catalog_sha256",
            "units",
            "budgets",
            "nodes",
            "part_outputs",
            "canonical_sha256",
        ],
        "geometry_program",
    )?;
    if geometry_program
        .get("schema_version")
        .and_then(Value::as_str)
        != Some("GeometryProgram@2")
        || geometry_program.get("project_id").and_then(Value::as_str) != Some(project_id)
        || geometry_program
            .get("representation_plan_sha256")
            .and_then(Value::as_str)
            != Some(representation_plan_sha256)
        || geometry_program
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(operator_catalog_sha256)
        || geometry_program.get("nodes") != Some(&nodes)
        || geometry_program
            .get("canonical_sha256")
            .and_then(Value::as_str)
            != Some(program_sha256)
    {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: geometry program binding drifted".to_owned());
    }
    let final_node_id = evaluation_order
        .as_array()
        .and_then(|items| items.last())
        .and_then(Value::as_str)
        .expect("template output exists");
    let expected_outputs = json!([{
        "part_id":part_id,
        "input_node_ids":[final_node_id],
        "material_zone_id":material_zone_id,
        "solid":true
    }]);
    if geometry_program.get("part_outputs") != Some(&expected_outputs)
        || geometry_program.get("units")
            != Some(
                &json!({"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"}),
            )
        || geometry_program.get("budgets")
            != Some(
                &json!({"max_nodes":3,"max_triangles":100000,"max_glb_bytes":16777216,"max_worker_memory_bytes":134217728,"max_runtime_ms":5000}),
            )
    {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: output or budget binding drifted".to_owned());
    }
    let mut program_preimage = Value::Object(geometry_program.clone());
    program_preimage
        .as_object_mut()
        .expect("program is object")
        .remove("canonical_sha256");
    if canonical_json_hash(&program_preimage) != program_sha256
        && canonical_json_hash(&normalize_numbers(&program_preimage)) != program_sha256
    {
        return Err(
            "PARAMETRIC_GROUP_RESULT_INVALID: program hash does not bind program".to_owned(),
        );
    }
    if object.get("limitations") != Some(&json!(LIMITATIONS)) {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: limitations drifted".to_owned());
    }
    let canonical_sha256 = sha256(object, "canonical_sha256")?;
    let mut canonical_preimage = value.clone();
    canonical_preimage["canonical_sha256"] = Value::String(String::new());
    if canonical_json_hash(&canonical_preimage) != canonical_sha256 {
        return Err("PARAMETRIC_GROUP_RESULT_INVALID: canonical hash drifted".to_owned());
    }
    Ok(())
}

fn template_definition(template_id: &str) -> Result<Value, String> {
    let (sockets, stages) = match template_id {
        ROUNDED_BOX => (
            json!([
                socket("size_m", "vec3", "meter"),
                socket("position_m", "vec3", "meter"),
                socket("rotation_rad", "vec3", "radian"),
                socket("bevel_width_m", "scalar", "meter"),
                socket("bevel_segments", "int", "none"),
                socket("bevel_profile", "scalar", "ratio"),
                socket("crease_angle_rad", "scalar", "radian")
            ]),
            json!([
                stage("source", "forgecad.geometry.primitive@2", &[]),
                stage("bevel", "forgecad.geometry.bevel@1", &["source"]),
                stage("normal", "forgecad.geometry.normal-policy@1", &["bevel"])
            ]),
        ),
        MIRRORED_BOX => (
            json!([
                socket("size_m", "vec3", "meter"),
                socket("position_m", "vec3", "meter"),
                socket("rotation_rad", "vec3", "radian"),
                socket("mirror_axis", "enum", "none"),
                socket("mirror_offset_m", "scalar", "meter"),
                socket("crease_angle_rad", "scalar", "radian")
            ]),
            json!([
                stage("source", "forgecad.geometry.primitive@2", &[]),
                stage("mirror", "forgecad.geometry.mirror@1", &["source"]),
                stage("normal", "forgecad.geometry.normal-policy@1", &["mirror"])
            ]),
        ),
        ARRAYED_CYLINDER => (
            json!([
                socket("radius_m", "scalar", "meter"),
                socket("height_m", "scalar", "meter"),
                socket("radial_segments", "int", "none"),
                socket("position_m", "vec3", "meter"),
                socket("rotation_rad", "vec3", "radian"),
                socket("array_count", "int", "none"),
                socket("array_offset_m", "vec3", "meter"),
                socket("crease_angle_rad", "scalar", "radian")
            ]),
            json!([
                stage("source", "forgecad.geometry.primitive@2", &[]),
                stage("array", "forgecad.geometry.array@1", &["source"]),
                stage("normal", "forgecad.geometry.normal-policy@1", &["array"])
            ]),
        ),
        _ => {
            return Err(format!(
                "PARAMETRIC_GROUP_TEMPLATE_UNAVAILABLE: {template_id}"
            ))
        }
    };
    let lowering_sha256 = canonical_json_hash(&lowering_definition(template_id)?);
    let mut definition = json!({
        "schema_version":"ForgeCADGeometryGroupTemplate@1",
        "template_id":template_id,
        "interface":{"field_mode":"single-value-only","inputs":sockets,"outputs":[{"socket_id":"geometry","socket_type":"geometry"}]},
        "stages":stages,
        "lowering_sha256":lowering_sha256,
        "nested_group_depth":0,
        "max_nodes":3,
        "execution":"runtime-lowered-to-geometry-program-v2",
        "canonical_sha256":""
    });
    definition["canonical_sha256"] = Value::String(canonical_json_hash(&definition));
    Ok(definition)
}

fn template_catalog_sha256() -> String {
    let definitions = [ROUNDED_BOX, MIRRORED_BOX, ARRAYED_CYLINDER]
        .iter()
        .map(|template_id| template_definition(template_id).expect("built-in template"))
        .collect::<Vec<_>>();
    canonical_json_hash(&json!({
        "schema_version":"ForgeCADGeometryGroupTemplateCatalog@1",
        "templates":definitions
    }))
}

fn lower_nodes(
    template_id: &str,
    parameters: &Map<String, Value>,
) -> Result<(Value, Value, Value), String> {
    validate_parameters(template_id, parameters)?;
    let lowering = lowering_definition(template_id)?;
    let stages = lowering
        .as_array()
        .expect("built-in lowering definition is an array");
    let mut nodes = Vec::with_capacity(stages.len());
    let mut source_map = Vec::with_capacity(stages.len());
    for stage in stages {
        let stage = stage
            .as_object()
            .expect("built-in lowering stage is an object");
        let bindings = stage["parameter_bindings"]
            .as_object()
            .expect("built-in lowering bindings are an object");
        let mut resolved = Map::new();
        for (target_parameter, binding) in bindings {
            let binding = binding
                .as_object()
                .expect("built-in lowering binding is an object");
            let value = if let Some(socket_id) = binding.get("socket").and_then(Value::as_str) {
                parameters
                    .get(socket_id)
                    .expect("built-in lowering socket exists")
                    .clone()
            } else {
                binding
                    .get("constant")
                    .expect("built-in lowering binding has a constant")
                    .clone()
            };
            resolved.insert(target_parameter.clone(), value);
        }
        let node_id = stage["node_id"].clone();
        let operator_id = stage["operator_id"].clone();
        let inputs = stage["inputs"].clone();
        nodes.push(json!({
            "node_id":node_id,
            "operator_id":operator_id,
            "inputs":inputs,
            "parameters":resolved
        }));
        source_map.push(json!({
            "stage_id":stage["stage_id"],
            "node_id":stage["node_id"],
            "operator_id":stage["operator_id"],
            "input_node_ids":stage["inputs"],
            "parameter_ids":stage["parameter_ids"]
        }));
    }
    let nodes = Value::Array(nodes);
    let evaluation_order = Value::Array(
        nodes
            .as_array()
            .expect("lowered nodes are an array")
            .iter()
            .map(|node| node["node_id"].clone())
            .collect(),
    );
    let source_map = Value::Array(source_map);
    Ok((nodes, evaluation_order, source_map))
}

fn lowering_definition(template_id: &str) -> Result<Value, String> {
    let socket = |socket_id: &str| json!({"socket":socket_id});
    let constant = |value: Value| json!({"constant":value});
    match template_id {
        ROUNDED_BOX => Ok(json!([
            {"stage_id":"source","node_id":"group-source","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameter_ids":["size_m","position_m","rotation_rad"],"parameter_bindings":{"shape":constant(json!("box")),"size_m":socket("size_m"),"position_m":socket("position_m"),"rotation_rad":socket("rotation_rad")}},
            {"stage_id":"bevel","node_id":"group-bevel","operator_id":"forgecad.geometry.bevel@1","inputs":["group-source"],"parameter_ids":["bevel_width_m","bevel_segments","bevel_profile"],"parameter_bindings":{"shape":constant(json!("bevel")),"width_m":socket("bevel_width_m"),"segments":socket("bevel_segments"),"profile":socket("bevel_profile"),"edge_scope":constant(json!("all-source-box-edges")),"clamp_overlap":constant(json!(false))}},
            {"stage_id":"normal","node_id":"group-normal","operator_id":"forgecad.geometry.normal-policy@1","inputs":["group-bevel"],"parameter_ids":["crease_angle_rad"],"parameter_bindings":{"shape":constant(json!("normal-policy")),"weighting":constant(json!("face-area-x-corner-angle")),"crease_angle_rad":socket("crease_angle_rad"),"keep_sharp":constant(json!(true)),"output_domain":constant(json!("corner"))}}
        ])),
        MIRRORED_BOX => Ok(json!([
            {"stage_id":"source","node_id":"group-source","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameter_ids":["size_m","position_m","rotation_rad"],"parameter_bindings":{"shape":constant(json!("box")),"size_m":socket("size_m"),"position_m":socket("position_m"),"rotation_rad":socket("rotation_rad")}},
            {"stage_id":"mirror","node_id":"group-mirror","operator_id":"forgecad.geometry.mirror@1","inputs":["group-source"],"parameter_ids":["mirror_axis","mirror_offset_m"],"parameter_bindings":{"shape":constant(json!("mirror")),"axis":socket("mirror_axis"),"offset_m":socket("mirror_offset_m")}},
            {"stage_id":"normal","node_id":"group-normal","operator_id":"forgecad.geometry.normal-policy@1","inputs":["group-mirror"],"parameter_ids":["crease_angle_rad"],"parameter_bindings":{"shape":constant(json!("normal-policy")),"weighting":constant(json!("face-area-x-corner-angle")),"crease_angle_rad":socket("crease_angle_rad"),"keep_sharp":constant(json!(true)),"output_domain":constant(json!("corner"))}}
        ])),
        ARRAYED_CYLINDER => Ok(json!([
            {"stage_id":"source","node_id":"group-source","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameter_ids":["radius_m","height_m","radial_segments","position_m","rotation_rad"],"parameter_bindings":{"shape":constant(json!("cylinder")),"radius_m":socket("radius_m"),"height_m":socket("height_m"),"radial_segments":socket("radial_segments"),"position_m":socket("position_m"),"rotation_rad":socket("rotation_rad")}},
            {"stage_id":"array","node_id":"group-array","operator_id":"forgecad.geometry.array@1","inputs":["group-source"],"parameter_ids":["array_count","array_offset_m"],"parameter_bindings":{"shape":constant(json!("array")),"count":socket("array_count"),"offset_m":socket("array_offset_m")}},
            {"stage_id":"normal","node_id":"group-normal","operator_id":"forgecad.geometry.normal-policy@1","inputs":["group-array"],"parameter_ids":["crease_angle_rad"],"parameter_bindings":{"shape":constant(json!("normal-policy")),"weighting":constant(json!("face-area-x-corner-angle")),"crease_angle_rad":socket("crease_angle_rad"),"keep_sharp":constant(json!(true)),"output_domain":constant(json!("corner"))}}
        ])),
        _ => Err(format!(
            "PARAMETRIC_GROUP_TEMPLATE_UNAVAILABLE: {template_id}"
        )),
    }
}

fn validate_parameters(template_id: &str, parameters: &Map<String, Value>) -> Result<(), String> {
    match template_id {
        ROUNDED_BOX => {
            exact_keys(
                parameters,
                &[
                    "size_m",
                    "position_m",
                    "rotation_rad",
                    "bevel_width_m",
                    "bevel_segments",
                    "bevel_profile",
                    "crease_angle_rad",
                ],
                "rounded-box parameters",
            )?;
            positive_vec3(parameters, "size_m", 10.0)?;
            coordinate_vec3(parameters, "position_m", 10.0)?;
            coordinate_vec3(parameters, "rotation_rad", std::f64::consts::TAU)?;
            let size = parameters["size_m"].as_array().unwrap();
            let width = number(parameters, "bevel_width_m", 0.0, 5.0, false)?;
            integer(parameters, "bevel_segments", 1, 4)?;
            number(parameters, "bevel_profile", 0.25, 0.75, true)?;
            number(
                parameters,
                "crease_angle_rad",
                0.0,
                std::f64::consts::PI,
                true,
            )?;
            let min_edge = size[0]
                .as_f64()
                .unwrap()
                .min(size[1].as_f64().unwrap())
                .min(size[2].as_f64().unwrap());
            if width * 2.0 >= min_edge {
                return Err(
                    "PARAMETRIC_GROUP_RELATIONSHIP_INVALID: bevel overlaps the source box"
                        .to_owned(),
                );
            }
        }
        MIRRORED_BOX => {
            exact_keys(
                parameters,
                &[
                    "size_m",
                    "position_m",
                    "rotation_rad",
                    "mirror_axis",
                    "mirror_offset_m",
                    "crease_angle_rad",
                ],
                "mirrored-box parameters",
            )?;
            positive_vec3(parameters, "size_m", 10.0)?;
            coordinate_vec3(parameters, "position_m", 10.0)?;
            coordinate_vec3(parameters, "rotation_rad", std::f64::consts::TAU)?;
            if !matches!(
                parameters.get("mirror_axis").and_then(Value::as_str),
                Some("x" | "y" | "z")
            ) {
                return Err("PARAMETRIC_GROUP_PARAMETER_INVALID: mirror_axis".to_owned());
            }
            number(parameters, "mirror_offset_m", -10.0, 10.0, true)?;
            number(
                parameters,
                "crease_angle_rad",
                0.0,
                std::f64::consts::PI,
                true,
            )?;
        }
        ARRAYED_CYLINDER => {
            exact_keys(
                parameters,
                &[
                    "radius_m",
                    "height_m",
                    "radial_segments",
                    "position_m",
                    "rotation_rad",
                    "array_count",
                    "array_offset_m",
                    "crease_angle_rad",
                ],
                "arrayed-cylinder parameters",
            )?;
            number(parameters, "radius_m", 0.0, 5.0, false)?;
            number(parameters, "height_m", 0.0, 10.0, false)?;
            integer(parameters, "radial_segments", 8, 64)?;
            coordinate_vec3(parameters, "position_m", 10.0)?;
            coordinate_vec3(parameters, "rotation_rad", std::f64::consts::TAU)?;
            integer(parameters, "array_count", 1, 32)?;
            coordinate_vec3(parameters, "array_offset_m", 10.0)?;
            number(
                parameters,
                "crease_angle_rad",
                0.0,
                std::f64::consts::PI,
                true,
            )?;
        }
        _ => {
            return Err(format!(
                "PARAMETRIC_GROUP_TEMPLATE_UNAVAILABLE: {template_id}"
            ))
        }
    }
    Ok(())
}

fn socket(socket_id: &str, socket_type: &str, unit: &str) -> Value {
    json!({"socket_id":socket_id,"direction":"input","socket_type":socket_type,"cardinality":"single","unit":unit,"field_mode":"single-value"})
}

fn stage(stage_id: &str, operator_id: &str, inputs: &[&str]) -> Value {
    json!({"stage_id":stage_id,"operator_id":operator_id,"inputs":inputs})
}

fn exact_keys(object: &Map<String, Value>, keys: &[&str], context: &str) -> Result<(), String> {
    if object.len() != keys.len()
        || keys.iter().any(|key| !object.contains_key(*key))
        || object.keys().any(|key| !keys.contains(&key.as_str()))
    {
        return Err(format!(
            "PARAMETRIC_GROUP_INVALID: {context} has an unexpected field set"
        ));
    }
    Ok(())
}

fn string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("PARAMETRIC_GROUP_INVALID: {key} must be a string"))
}

fn identifier<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = string(object, key)?;
    if !is_opaque_id(value) {
        return Err(format!(
            "PARAMETRIC_GROUP_INVALID: {key} must be an identifier"
        ));
    }
    Ok(value)
}

fn sha256<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    let value = string(object, key)?;
    if !is_sha256(value) {
        return Err(format!("PARAMETRIC_GROUP_INVALID: {key} must be a SHA-256"));
    }
    Ok(value)
}

fn number(
    object: &Map<String, Value>,
    key: &str,
    minimum: f64,
    maximum: f64,
    inclusive_minimum: bool,
) -> Result<f64, String> {
    let value = object
        .get(key)
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("PARAMETRIC_GROUP_PARAMETER_INVALID: {key}"))?;
    let lower_ok = if inclusive_minimum {
        value >= minimum
    } else {
        value > minimum
    };
    if !lower_ok || value > maximum {
        return Err(format!("PARAMETRIC_GROUP_PARAMETER_OUT_OF_BOUNDS: {key}"));
    }
    Ok(value)
}

fn integer(
    object: &Map<String, Value>,
    key: &str,
    minimum: u64,
    maximum: u64,
) -> Result<u64, String> {
    let value = object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("PARAMETRIC_GROUP_PARAMETER_INVALID: {key}"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("PARAMETRIC_GROUP_PARAMETER_OUT_OF_BOUNDS: {key}"));
    }
    Ok(value)
}

fn coordinate_vec3(object: &Map<String, Value>, key: &str, bound: f64) -> Result<(), String> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .filter(|items| items.len() == 3)
        .ok_or_else(|| format!("PARAMETRIC_GROUP_PARAMETER_INVALID: {key}"))?;
    if values.iter().any(|value| {
        value
            .as_f64()
            .is_none_or(|value| !value.is_finite() || value < -bound || value > bound)
    }) {
        return Err(format!("PARAMETRIC_GROUP_PARAMETER_OUT_OF_BOUNDS: {key}"));
    }
    Ok(())
}

fn positive_vec3(object: &Map<String, Value>, key: &str, maximum: f64) -> Result<(), String> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .filter(|items| items.len() == 3)
        .ok_or_else(|| format!("PARAMETRIC_GROUP_PARAMETER_INVALID: {key}"))?;
    if values.iter().any(|value| {
        value
            .as_f64()
            .is_none_or(|value| !value.is_finite() || value <= 0.0 || value > maximum)
    }) {
        return Err(format!("PARAMETRIC_GROUP_PARAMETER_OUT_OF_BOUNDS: {key}"));
    }
    Ok(())
}

fn normalize_numbers(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(normalize_numbers).collect()),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, value)| (key.clone(), normalize_numbers(value)))
                .collect(),
        ),
        Value::Number(number) => number
            .as_f64()
            .filter(|value| {
                value.is_finite()
                    && value.fract() == 0.0
                    && *value >= i64::MIN as f64
                    && *value <= i64::MAX as f64
            })
            .map(|value| Value::from(value as i64))
            .unwrap_or_else(|| value.clone()),
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(template_id: &str, parameters: Value) -> Value {
        let mut request = json!({
            "schema_version":REQUEST_SCHEMA,
            "project_id":"project-group",
            "representation_plan_sha256":"a".repeat(64),
            "template_id":template_id,
            "instance_id":"instance-1",
            "part_id":"part-1",
            "material_zone_id":"zone-1",
            "parameters":parameters,
            "input_sha256":""
        });
        let mut preimage = request.clone();
        preimage.as_object_mut().unwrap().remove("input_sha256");
        request["input_sha256"] = Value::String(canonical_json_hash(&preimage));
        request
    }

    #[test]
    fn templates_are_hash_bound_and_reject_dynamic_fields() {
        let value = request(
            ROUNDED_BOX,
            json!({
                "size_m":[1.0,0.8,0.4],"position_m":[0.0,0.0,0.0],"rotation_rad":[0.0,0.0,0.0],
                "bevel_width_m":0.04,"bevel_segments":2,"bevel_profile":0.5,"crease_angle_rad":1.0
            }),
        );
        let expansion = expand(value.as_object().unwrap(), &"b".repeat(64)).expect("expansion");
        assert_eq!(
            expansion.geometry_program_draft["nodes"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        let mut script = value.clone();
        script["parameters"]["script"] = json!("python.exec");
        assert!(expand(script.as_object().unwrap(), &"b".repeat(64)).is_err());
        let mut unknown = value;
        unknown["template_id"] = json!("https://example.com/plugin.py");
        assert!(expand(unknown.as_object().unwrap(), &"b".repeat(64)).is_err());
    }
}
