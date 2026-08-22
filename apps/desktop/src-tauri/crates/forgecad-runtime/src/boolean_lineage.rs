use super::{
    canonical_json_bytes, canonical_json_hash, exact_object, geometry_worker, is_opaque_id,
    is_sha256, operator_catalog_sha256, verify_output_canonical_hash, Runtime, RuntimeError,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

const MAX_LINEAGE_RUNS: u64 = 4096;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const LIMITATIONS: [&str; 4] = [
    "EVALUATED_FACE_ID_NOT_ORIGINAL_AUTHORING_FACE_ID",
    "FACE_IDS_NOT_STABLE_ACROSS_PROGRAM_CHANGE",
    "LINEAGE_NOT_PERSISTED_IN_CURRENT_GLB",
    "STRUCTURAL_LINEAGE_DOES_NOT_PROVE_VISUAL_QUALITY",
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedBooleanSemantics {
    operation: String,
    left_node_id: String,
    right_node_id: String,
    left_source_node_ids: Vec<String>,
    right_source_node_ids: Vec<String>,
}

impl Runtime {
    /// Evaluate one exact Boolean node in the fixed Geometry Worker and return
    /// a bounded read-only projection of operand and evaluated-face runs.
    /// The face IDs are Manifold evaluated planar-face identities, not stable
    /// authoring-face IDs and not current GLB lineage.
    pub fn boolean_operand_lineage_preview(&self, request: Value) -> Result<Value, RuntimeError> {
        let object = exact_object(
            &request,
            &[
                "schema_version",
                "geometry_program",
                "boolean_node_id",
                "max_lineage_runs",
                "canonical_sha256",
            ],
            "BooleanOperandLineageRequest@1",
        )?;
        if object.get("schema_version").and_then(Value::as_str)
            != Some("BooleanOperandLineageRequest@1")
        {
            return Err(lineage_error("request schema_version differs"));
        }
        verify_output_canonical_hash(&request, "BooleanOperandLineageRequest@1")?;
        let program = object
            .get("geometry_program")
            .filter(|value| value.is_object())
            .ok_or_else(|| lineage_error("geometry_program must be an object"))?;
        verify_geometry_program_canonical_hash(program)?;
        let program_sha256 = program
            .get("canonical_sha256")
            .and_then(Value::as_str)
            .filter(|value| is_sha256(value))
            .ok_or_else(|| lineage_error("geometry_program canonical hash is invalid"))?;
        let boolean_node_id = required_id(object, "boolean_node_id")?;
        let expected_semantics = expected_boolean_semantics(program, &boolean_node_id)?;
        let max_lineage_runs = object
            .get("max_lineage_runs")
            .and_then(Value::as_u64)
            .filter(|value| (1..=MAX_LINEAGE_RUNS).contains(value))
            .ok_or_else(|| lineage_error("max_lineage_runs is outside 1..4096"))?;

        let result = execute_worker(program, &boolean_node_id, max_lineage_runs)
            .map_err(|error| lineage_error(&error.to_string()))?;
        validate_result(
            &result,
            program_sha256,
            &boolean_node_id,
            max_lineage_runs,
            &expected_semantics,
        )?;
        let bytes = canonical_json_bytes(&result)
            .map_err(|error| lineage_error(&format!("result serialization failed: {error}")))?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err(lineage_error("result exceeds 1 MiB"));
        }
        Ok(result)
    }
}

fn execute_worker(
    program: &Value,
    boolean_node_id: &str,
    max_lineage_runs: u64,
) -> Result<Value, geometry_worker::GeometryWorkerError> {
    let result =
        geometry_worker::boolean_operand_lineage(program, boolean_node_id, max_lineage_runs);
    #[cfg(any(test, feature = "test-geometry-worker-fallback"))]
    if matches!(
        result,
        Err(geometry_worker::GeometryWorkerError::Unavailable)
    ) {
        return geometry_worker::boolean_operand_lineage_test_fallback(
            program,
            boolean_node_id,
            max_lineage_runs,
        );
    }
    result
}

fn validate_result(
    value: &Value,
    expected_program_sha256: &str,
    expected_boolean_node_id: &str,
    max_lineage_runs: u64,
    expected_semantics: &ExpectedBooleanSemantics,
) -> Result<(), RuntimeError> {
    let object = exact_object(
        value,
        &[
            "schema_version",
            "program_sha256",
            "operator_catalog_sha256",
            "boolean_node_id",
            "operation",
            "operands",
            "output_triangle_count",
            "lineage_run_count",
            "lineage_runs",
            "lineage_sha256",
            "lineage_kind",
            "materialization_status",
            "runtime_write_performed",
            "limitations",
            "canonical_sha256",
        ],
        "BooleanOperandLineage@1",
    )?;
    if object.get("schema_version").and_then(Value::as_str) != Some("BooleanOperandLineage@1")
        || object.get("program_sha256").and_then(Value::as_str) != Some(expected_program_sha256)
        || object
            .get("operator_catalog_sha256")
            .and_then(Value::as_str)
            != Some(operator_catalog_sha256().as_str())
        || object.get("boolean_node_id").and_then(Value::as_str) != Some(expected_boolean_node_id)
        || object.get("operation").and_then(Value::as_str)
            != Some(expected_semantics.operation.as_str())
        || object.get("lineage_kind").and_then(Value::as_str)
            != Some("evaluated-face-with-operand-run")
        || object.get("materialization_status").and_then(Value::as_str)
            != Some("preview-only-not-persisted-in-glb")
        || object.get("runtime_write_performed") != Some(&Value::Bool(false))
        || object.get("limitations") != Some(&json!(LIMITATIONS))
    {
        return Err(lineage_error("result constants or scope differ"));
    }
    let operands = object
        .get("operands")
        .and_then(Value::as_array)
        .filter(|values| values.len() == 2)
        .ok_or_else(|| lineage_error("operand inventory differs"))?;
    let mut operand_nodes = Map::<String, Value>::new();
    let mut operand_triangle_sum = 0u64;
    for (index, operand) in operands.iter().enumerate() {
        let operand = exact_object(
            operand,
            &[
                "operand",
                "node_id",
                "lineage_source_node_ids",
                "output_triangle_count",
            ],
            "BooleanOperandLineage@1.operands",
        )?;
        let side = if index == 0 { "left" } else { "right" };
        let (expected_node_id, expected_sources) = if index == 0 {
            (
                expected_semantics.left_node_id.as_str(),
                &expected_semantics.left_source_node_ids,
            )
        } else {
            (
                expected_semantics.right_node_id.as_str(),
                &expected_semantics.right_source_node_ids,
            )
        };
        if operand.get("operand").and_then(Value::as_str) != Some(side) {
            return Err(lineage_error("operand order differs"));
        }
        let node_id = required_id(operand, "node_id")?;
        if node_id != expected_node_id {
            return Err(lineage_error("operand node binding differs from program"));
        }
        let sources = operand
            .get("lineage_source_node_ids")
            .and_then(Value::as_array)
            .filter(|values| !values.is_empty() && values.len() <= 64)
            .ok_or_else(|| lineage_error("operand source lineage differs"))?;
        if sources.iter().any(|value| {
            !value
                .as_str()
                .is_some_and(|candidate| is_opaque_id(candidate))
        }) {
            return Err(lineage_error("operand source lineage ID is invalid"));
        }
        let source_ids = sources
            .iter()
            .map(|value| value.as_str().expect("validated string").to_owned())
            .collect::<Vec<_>>();
        if source_ids.iter().enumerate().any(|(index, source)| {
            source_ids[..index]
                .iter()
                .any(|existing| existing == source)
        }) || &source_ids != expected_sources
        {
            return Err(lineage_error(
                "operand source lineage differs from program evaluation",
            ));
        }
        let count = operand
            .get("output_triangle_count")
            .and_then(Value::as_u64)
            .filter(|count| *count <= 250_000)
            .ok_or_else(|| lineage_error("operand triangle count is invalid"))?;
        operand_triangle_sum = operand_triangle_sum
            .checked_add(count)
            .ok_or_else(|| lineage_error("operand triangle count overflow"))?;
        operand_nodes.insert(side.to_owned(), Value::String(node_id));
    }
    let output_triangle_count = object
        .get("output_triangle_count")
        .and_then(Value::as_u64)
        .filter(|count| (1..=250_000).contains(count))
        .ok_or_else(|| lineage_error("output triangle count is invalid"))?;
    let runs = object
        .get("lineage_runs")
        .and_then(Value::as_array)
        .filter(|runs| !runs.is_empty() && runs.len() as u64 <= max_lineage_runs)
        .ok_or_else(|| lineage_error("lineage runs exceed the declared bound"))?;
    if object.get("lineage_run_count").and_then(Value::as_u64) != Some(runs.len() as u64)
        || operand_triangle_sum != output_triangle_count
    {
        return Err(lineage_error("lineage or operand counts differ"));
    }
    let mut next_triangle = 0u64;
    for run in runs {
        let run = exact_object(
            run,
            &[
                "output_triangle_start",
                "output_triangle_count",
                "operand",
                "operand_node_id",
                "evaluated_face_id",
            ],
            "BooleanOperandLineage@1.lineage_runs",
        )?;
        let side = run
            .get("operand")
            .and_then(Value::as_str)
            .filter(|side| matches!(*side, "left" | "right"))
            .ok_or_else(|| lineage_error("lineage operand is invalid"))?;
        let count = run
            .get("output_triangle_count")
            .and_then(Value::as_u64)
            .filter(|count| (1..=250_000).contains(count))
            .ok_or_else(|| lineage_error("lineage run count is invalid"))?;
        if run.get("output_triangle_start").and_then(Value::as_u64) != Some(next_triangle)
            || next_triangle > 249_999
            || run.get("operand_node_id") != operand_nodes.get(side)
            || run
                .get("evaluated_face_id")
                .and_then(Value::as_u64)
                .is_none_or(|face_id| face_id > u32::MAX as u64)
        {
            return Err(lineage_error(
                "lineage run continuity or operand binding differs",
            ));
        }
        next_triangle = next_triangle
            .checked_add(count)
            .ok_or_else(|| lineage_error("lineage run count overflow"))?;
    }
    if next_triangle != output_triangle_count
        || object.get("lineage_sha256").and_then(Value::as_str)
            != Some(canonical_json_hash(&Value::Array(runs.clone())).as_str())
    {
        return Err(lineage_error("lineage coverage or hash differs"));
    }
    verify_output_canonical_hash(value, "BooleanOperandLineage@1")
}

fn expected_boolean_semantics(
    program: &Value,
    boolean_node_id: &str,
) -> Result<ExpectedBooleanSemantics, RuntimeError> {
    if program.get("schema_version").and_then(Value::as_str) != Some("GeometryProgram@2") {
        return Err(lineage_error("geometry_program schema_version differs"));
    }
    let nodes = program
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|nodes| !nodes.is_empty() && nodes.len() <= 4096)
        .ok_or_else(|| lineage_error("geometry_program nodes are invalid"))?;
    let mut lineages = BTreeMap::<String, Vec<String>>::new();
    let mut expected = None;
    for node in nodes {
        let node = exact_object(
            node,
            &["node_id", "operator_id", "inputs", "parameters"],
            "GeometryProgram@2.nodes",
        )?;
        let node_id = required_id(node, "node_id")?;
        if lineages.contains_key(&node_id) {
            return Err(lineage_error("geometry_program node_id is duplicated"));
        }
        let operator_id = node
            .get("operator_id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty() && value.len() <= 128 && value.is_ascii())
            .ok_or_else(|| lineage_error("geometry_program operator_id is invalid"))?;
        let inputs = node
            .get("inputs")
            .and_then(Value::as_array)
            .filter(|inputs| inputs.len() <= 64)
            .ok_or_else(|| lineage_error("geometry_program node inputs are invalid"))?;
        let input_ids = inputs
            .iter()
            .map(|input| {
                input
                    .as_str()
                    .filter(|value| is_opaque_id(value))
                    .map(str::to_owned)
                    .ok_or_else(|| lineage_error("geometry_program input ID is invalid"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut lineage = Vec::<String>::new();
        if input_ids.is_empty() {
            lineage.push(node_id.clone());
        } else {
            for input_id in &input_ids {
                let input_lineage = lineages.get(input_id).ok_or_else(|| {
                    lineage_error("geometry_program input is not a prior evaluated node")
                })?;
                for source in input_lineage {
                    if !lineage.iter().any(|existing| existing == source) {
                        lineage.push(source.clone());
                    }
                }
            }
        }
        if node_id == boolean_node_id {
            if operator_id != "forgecad.geometry.boolean@1" || input_ids.len() != 2 {
                return Err(lineage_error(
                    "target node is not an exact Boolean operator",
                ));
            }
            let parameters = exact_object(
                node.get("parameters")
                    .ok_or_else(|| lineage_error("Boolean parameters are missing"))?,
                &["shape"],
                "GeometryProgram@2.nodes.boolean.parameters",
            )?;
            let operation = parameters
                .get("shape")
                .and_then(Value::as_str)
                .filter(|value| matches!(*value, "union" | "difference" | "intersection"))
                .ok_or_else(|| lineage_error("Boolean operation is invalid"))?
                .to_owned();
            expected = Some(ExpectedBooleanSemantics {
                operation,
                left_node_id: input_ids[0].clone(),
                right_node_id: input_ids[1].clone(),
                left_source_node_ids: lineages[&input_ids[0]].clone(),
                right_source_node_ids: lineages[&input_ids[1]].clone(),
            });
        }
        lineages.insert(node_id, lineage);
    }
    expected.ok_or_else(|| lineage_error("Boolean target node is missing"))
}

fn verify_geometry_program_canonical_hash(program: &Value) -> Result<(), RuntimeError> {
    let object = program
        .as_object()
        .ok_or_else(|| lineage_error("geometry_program must be an object"))?;
    let actual = object
        .get("canonical_sha256")
        .and_then(Value::as_str)
        .filter(|value| is_sha256(value))
        .ok_or_else(|| lineage_error("geometry_program canonical hash is invalid"))?;
    let mut without_hash = object.clone();
    without_hash.remove("canonical_sha256");
    if canonical_json_hash(&Value::Object(without_hash)) != actual {
        return Err(lineage_error(
            "geometry_program canonical hash does not bind the payload",
        ));
    }
    Ok(())
}

fn required_id(object: &Map<String, Value>, key: &str) -> Result<String, RuntimeError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| is_opaque_id(value))
        .map(str::to_owned)
        .ok_or_else(|| lineage_error(&format!("{key} is invalid")))
}

fn lineage_error(detail: &str) -> RuntimeError {
    RuntimeError::InvalidInput(format!("BOOLEAN_OPERAND_LINEAGE_INVALID: {detail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boolean_program(project_id: &str, operation: &str) -> Value {
        let mut program = json!({
            "schema_version":"GeometryProgram@2",
            "project_id":project_id,
            "representation_plan_sha256":"6".repeat(64),
            "operator_catalog_sha256":operator_catalog_sha256(),
            "units":{"length":"meter","angle":"radian","coordinate_system":"right-handed-y-up"},
            "budgets":{"max_nodes":8,"max_triangles":10000,"max_glb_bytes":67108864,"max_worker_memory_bytes":536870912,"max_runtime_ms":10000},
            "nodes":[
                {"node_id":"left","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[-0.25,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"right","operator_id":"forgecad.geometry.primitive@2","inputs":[],"parameters":{"shape":"box","size_m":[1.0,1.0,1.0],"position_m":[0.25,0.0,0.0],"rotation_rad":[0.0,0.0,0.0]}},
                {"node_id":"boolean","operator_id":"forgecad.geometry.boolean@1","inputs":["left","right"],"parameters":{"shape":operation}}
            ],
            "part_outputs":[{"part_id":"boolean-part","input_node_ids":["boolean"],"material_zone_id":"zone-mechanical","solid":true}]
        });
        program["canonical_sha256"] = Value::String(canonical_json_hash(&program));
        program
    }

    fn request(program: Value, node_id: &str, max_runs: u64) -> Value {
        let mut request = json!({
            "schema_version":"BooleanOperandLineageRequest@1",
            "geometry_program":program,
            "boolean_node_id":node_id,
            "max_lineage_runs":max_runs,
            "canonical_sha256":""
        });
        request["canonical_sha256"] = Value::String(canonical_json_hash(&request));
        request
    }

    #[test]
    fn runtime_boolean_lineage_is_deterministic_and_does_not_write() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("Boolean lineage preview", json!({"profile":"mvp"}))
            .expect("project");
        let request = request(
            boolean_program(&project.project_id, "union"),
            "boolean",
            4096,
        );
        let before = json!({
            "projects":runtime.projects().expect("projects"),
            "candidates":runtime.candidates(&project.project_id).expect("candidates"),
            "versions":runtime.versions(Some(&project.project_id)).expect("versions"),
            "cas_objects":runtime.store.cas().list_objects().expect("CAS inventory")
        });
        let first = runtime
            .boolean_operand_lineage_preview(request.clone())
            .expect("lineage preview");
        let second = runtime
            .boolean_operand_lineage_preview(request)
            .expect("deterministic repeat");
        assert_eq!(first, second);
        assert_eq!(first["schema_version"], "BooleanOperandLineage@1");
        assert_eq!(first["lineage_kind"], "evaluated-face-with-operand-run");
        assert_eq!(first["runtime_write_performed"], false);
        assert_eq!(
            first["limitations"][0],
            "EVALUATED_FACE_ID_NOT_ORIGINAL_AUTHORING_FACE_ID"
        );
        let after = json!({
            "projects":runtime.projects().expect("projects"),
            "candidates":runtime.candidates(&project.project_id).expect("candidates"),
            "versions":runtime.versions(Some(&project.project_id)).expect("versions"),
            "cas_objects":runtime.store.cas().list_objects().expect("CAS inventory")
        });
        assert_eq!(
            before, after,
            "lineage preview must not persist Runtime state"
        );
    }

    #[test]
    fn runtime_boolean_lineage_rejects_scope_and_hash_drift() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let project = runtime
            .create_project("Boolean lineage negatives", json!({"profile":"mvp"}))
            .expect("project");
        let program = boolean_program(&project.project_id, "difference");
        assert!(runtime
            .boolean_operand_lineage_preview(request(program.clone(), "left", 4096))
            .is_err());
        assert!(runtime
            .boolean_operand_lineage_preview(request(program.clone(), "boolean", 0))
            .is_err());
        let mut unknown = request(program.clone(), "boolean", 4096);
        unknown["unexpected"] = Value::Bool(true);
        assert!(runtime.boolean_operand_lineage_preview(unknown).is_err());
        let mut tampered = request(program, "boolean", 4096);
        tampered["boolean_node_id"] = Value::String("right".to_owned());
        assert!(runtime.boolean_operand_lineage_preview(tampered).is_err());
    }

    #[test]
    fn runtime_boolean_lineage_rejects_hash_consistent_result_limit_drift() {
        let program = boolean_program("project-output-limits", "union");
        let program_sha256 = program["canonical_sha256"].as_str().unwrap().to_owned();
        let mut result =
            geometry_worker::boolean_operand_lineage_test_fallback(&program, "boolean", 4096)
                .expect("worker lineage");
        let expected = expected_boolean_semantics(&program, "boolean").expect("semantics");
        result["lineage_runs"][0]["evaluated_face_id"] = Value::from(u32::MAX as u64 + 1);
        result["lineage_sha256"] = Value::String(canonical_json_hash(&result["lineage_runs"]));
        result["canonical_sha256"] = Value::String(String::new());
        result["canonical_sha256"] = Value::String(canonical_json_hash(&result));
        assert!(validate_result(&result, &program_sha256, "boolean", 4096, &expected).is_err());
    }

    fn rehash_result(result: &mut Value) {
        result["canonical_sha256"] = Value::String(String::new());
        result["canonical_sha256"] = Value::String(canonical_json_hash(result));
    }

    #[test]
    fn runtime_boolean_lineage_rejects_hash_consistent_worker_semantic_drift() {
        let program = boolean_program("project-semantic-drift", "difference");
        let program_sha256 = program["canonical_sha256"].as_str().unwrap().to_owned();
        let expected = expected_boolean_semantics(&program, "boolean").expect("semantics");
        let valid =
            geometry_worker::boolean_operand_lineage_test_fallback(&program, "boolean", 4096)
                .expect("worker lineage");

        let mut wrong_operation = valid.clone();
        wrong_operation["operation"] = Value::String("union".to_owned());
        rehash_result(&mut wrong_operation);
        assert!(validate_result(
            &wrong_operation,
            &program_sha256,
            "boolean",
            4096,
            &expected
        )
        .is_err());

        let mut swapped_operands = valid.clone();
        let left_node = swapped_operands["operands"][0]["node_id"].clone();
        let left_sources = swapped_operands["operands"][0]["lineage_source_node_ids"].clone();
        swapped_operands["operands"][0]["node_id"] =
            swapped_operands["operands"][1]["node_id"].clone();
        swapped_operands["operands"][0]["lineage_source_node_ids"] =
            swapped_operands["operands"][1]["lineage_source_node_ids"].clone();
        swapped_operands["operands"][1]["node_id"] = left_node;
        swapped_operands["operands"][1]["lineage_source_node_ids"] = left_sources;
        let swapped_left_node = swapped_operands["operands"][0]["node_id"].clone();
        let swapped_right_node = swapped_operands["operands"][1]["node_id"].clone();
        for run in swapped_operands["lineage_runs"]
            .as_array_mut()
            .expect("runs")
        {
            let side = run["operand"].as_str().expect("side");
            run["operand_node_id"] = if side == "left" {
                swapped_left_node.clone()
            } else {
                swapped_right_node.clone()
            };
        }
        swapped_operands["lineage_sha256"] =
            Value::String(canonical_json_hash(&swapped_operands["lineage_runs"]));
        rehash_result(&mut swapped_operands);
        assert!(validate_result(
            &swapped_operands,
            &program_sha256,
            "boolean",
            4096,
            &expected
        )
        .is_err());

        let mut forged_sources = valid;
        forged_sources["operands"][0]["lineage_source_node_ids"] = json!(["left", "left"]);
        rehash_result(&mut forged_sources);
        assert!(
            validate_result(&forged_sources, &program_sha256, "boolean", 4096, &expected).is_err()
        );
    }
}
