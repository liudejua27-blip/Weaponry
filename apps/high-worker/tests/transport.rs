mod support;

use forgecad_worker_protocol::{
    canonical_json_sha256, validate_native_high_glb_materialize_result, validate_response,
    WorkerResponse, NATIVE_HIGH_GLB_MATERIALIZE_OPERATION, NATIVE_HIGH_GLB_REQUEST_SCHEMA_VERSION,
    WORKER_PROTOCOL,
};
use serde_json::{json, Value};
use std::process::Command;

fn run_worker(args: &[&str], input: &[u8]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forgecad-high-worker"))
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .take()
                .expect("worker stdin")
                .write_all(input)
                .expect("write worker request");
            child.wait_with_output()
        })
        .expect("run high worker")
}

#[test]
fn build_identity_retains_dedicated_entry_and_cohort() {
    let output = run_worker(&["--build-identity"], b"");
    assert!(output.status.success());
    let identity: Value = serde_json::from_slice(&output.stdout).expect("identity JSON");
    assert_eq!(
        identity["schema_version"],
        "ForgeCADHighWorkerBuildIdentity@1"
    );
    assert_eq!(identity["component"], "forgecad-high-worker");
    assert_eq!(identity["entry"], "--isolated-once-native-high");
    assert_eq!(
        identity["operation"],
        "forgecad.production.high-mesh-prepare@1"
    );
    assert!(
        identity["build_cohort_sha256"].is_null() || identity["build_cohort_sha256"].is_string()
    );
    assert_eq!(
        identity["authoring_mesh_v2_high_execution_operation"],
        "forgecad.production.authoring-mesh-v2-high-execute@1"
    );
    assert_eq!(
        identity["authoring_mesh_v2_high_execution_request_schema"],
        "AuthoringMeshV2HighExecutionRequest@2"
    );
    assert_eq!(
        identity["authoring_mesh_v2_high_result_schema"],
        "AuthoringMeshV2HighResult@2"
    );
    assert_eq!(
        identity["authoring_mesh_v2_high_artifact_materialize_entry"],
        "--isolated-once-authoring-mesh-v2-high-artifact-materialize"
    );
    assert_eq!(
        identity["authoring_mesh_v2_high_artifact_materialize_operation"],
        "forgecad.production.authoring-mesh-v2-high-artifact-materialize@1"
    );
    assert_eq!(
        identity["authoring_mesh_v2_high_artifact_materialize_request_schema"],
        "AuthoringMeshV2HighArtifactMaterializeRequest@1"
    );
    assert_eq!(
        identity["authoring_mesh_v2_high_artifact_materialize_result_schema"],
        "AuthoringMeshV2HighArtifactMaterializeResult@1"
    );
    assert_eq!(
        identity["authoring_mesh_v2_high_artifact_materialize_glb_kind"],
        "authoring-mesh-v2-high-artifact-glb@1"
    );
    assert_eq!(
        identity["authoring_mesh_v2_high_artifact_materialize_readback_kind"],
        "authoring-mesh-v2-high-artifact-readback@1"
    );
    assert_eq!(
        identity["authoring_mesh_v2_high_algorithm_sha256"],
        forgecad_high_worker::authoring_mesh_v2::algorithm_sha256()
    );
}

#[test]
fn dedicated_transport_returns_generic_response_after_inner_high_validation() {
    let request = br#"{
        "protocol":"forgecad-worker-protocol@1",
        "request_id":"transport-test-1",
        "operation":"forgecad.production.high-mesh-prepare@1",
        "payload":{"schema_version":"HighMeshWorkerRequest@1"}
    }"#;
    let output = run_worker(&["--isolated-once-native-high"], request);
    assert!(!output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).expect("response JSON");
    assert_eq!(response["protocol"], "forgecad-worker-protocol@1");
    assert_eq!(response["request_id"], "transport-test-1");
    assert_eq!(response["ok"], false);
    assert_eq!(response["result"], Value::Null);
    assert_eq!(response["error"]["code"], "HIGH_WORKER_JSON_INVALID");
    assert_eq!(response["error"]["message"], "request JSON invalid");
}

#[test]
fn dedicated_transport_returns_high_artifact_and_replays_byte_exactly() {
    let request = support::worker_request();
    let request_bytes = serde_json::to_vec(&request).expect("request JSON");
    let first = run_worker(&["--isolated-once-native-high"], &request_bytes);
    let second = run_worker(&["--isolated-once-native-high"], &request_bytes);
    assert!(
        first.status.success(),
        "first stdout: {:?}; stderr: {:?}",
        String::from_utf8_lossy(&first.stdout),
        first.stderr
    );
    assert!(
        second.status.success(),
        "second stdout: {:?}; stderr: {:?}",
        String::from_utf8_lossy(&second.stdout),
        second.stderr
    );
    assert_eq!(
        first.stdout, second.stdout,
        "replay stdout must be byte-exact"
    );

    let response: WorkerResponse = serde_json::from_slice(&first.stdout).expect("response JSON");
    validate_response(&response, "native-high-transport-positive-1")
        .expect("generic WorkerResponse validation");
    assert_eq!(response.protocol, WORKER_PROTOCOL);
    assert!(response.ok);
    assert!(response.error.is_none());

    let artifact = response.result.as_ref().expect("successful artifact");
    assert_eq!(artifact["schema_version"], "HighMeshArtifact@1");
    assert_eq!(
        artifact["operation"],
        "forgecad.production.high-mesh-prepare@1"
    );
    assert_eq!(artifact["replay_count"], 2);
    assert_eq!(artifact["replay_byte_exact"], true);
    assert_eq!(artifact["non_destructive"], true);
    assert_eq!(artifact["hard_gate_passed"], false);
    assert_eq!(artifact["runtime_write_performed"], false);
    assert_eq!(artifact["base_triangle_count"], 12);
    assert_eq!(artifact["detail_triangle_count"], 20);
    assert_eq!(artifact["triangle_count"], 32);

    let base = &artifact["base_parts"][0];
    assert_eq!(base["kind"], "authoring_base");
    assert_eq!(base["part_id"], "receiver");
    let expected_positions = request["payload"]["source_authoring_mesh"]["canonical_mesh"]
        ["vertices"]
        .as_array()
        .expect("canonical vertices")
        .iter()
        .map(|vertex| vertex["position_m"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        base["geometry"]["positions_m"],
        Value::Array(expected_positions)
    );
    assert_eq!(
        base["geometry"]["indices"],
        json!([
            [0, 3, 2],
            [0, 2, 1],
            [0, 1, 5],
            [0, 5, 4],
            [0, 4, 7],
            [0, 7, 3],
            [3, 7, 6],
            [3, 6, 2],
            [1, 2, 6],
            [1, 6, 5],
            [4, 5, 6],
            [4, 6, 7]
        ])
    );
    let base_lineage = base["source_element_lineage"]
        .as_array()
        .expect("base lineage");
    for stable_id in [
        "vertex:v000",
        "vertex:v007",
        "edge:e-v001-v005",
        "loop:loop-f-top",
        "face:f-top",
        "part:receiver",
        "node:receiver-source",
    ] {
        assert!(
            base_lineage.iter().any(|value| value == stable_id),
            "missing stable base lineage {stable_id}"
        );
    }
    assert_eq!(artifact["detail_primitives"].as_array().unwrap().len(), 3);
    assert_eq!(artifact["detail_lineage"].as_array().unwrap().len(), 3);
}

#[test]
fn entrypoint_is_closed() {
    let output = run_worker(&["--isolated-once"], b"{}");
    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty());
}

#[test]
fn v2_high_artifact_materialize_entry_is_closed_and_bounded() {
    let output = run_worker(
        &["--isolated-once-authoring-mesh-v2-high-artifact-materialize"],
        br#"{}"#,
    );
    assert!(!output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).expect("response JSON");
    assert_eq!(response["ok"], false);
    assert_eq!(response["error"]["code"], "WORKER_PROTOCOL");
}

fn glb_materialize_request() -> Value {
    let high_request = support::worker_request();
    let high_output = run_worker(
        &["--isolated-once-native-high"],
        &serde_json::to_vec(&high_request).unwrap(),
    );
    assert!(high_output.status.success());
    let high_response: Value = serde_json::from_slice(&high_output.stdout).expect("high response");
    let artifact = high_response["result"].clone();
    let mut payload = json!({
        "schema_version": NATIVE_HIGH_GLB_REQUEST_SCHEMA_VERSION,
        "artifact": artifact,
        "input_canonical_sha256": "",
        "canonical_sha256": ""
    });
    payload["input_canonical_sha256"] = payload["artifact"]["canonical_sha256"].clone();
    payload["canonical_sha256"] = Value::String(canonical_json_sha256(&payload));
    json!({
        "protocol": WORKER_PROTOCOL,
        "request_id": "native-high-glb-materialize-1",
        "operation": NATIVE_HIGH_GLB_MATERIALIZE_OPERATION,
        "payload": payload
    })
}

#[test]
fn native_high_glb_materialization_is_two_process_byte_exact_and_strict() {
    let request = glb_materialize_request();
    let request_bytes = serde_json::to_vec(&request).expect("materialize request");
    let first = run_worker(&["--isolated-once-native-high-glb"], &request_bytes);
    let second = run_worker(&["--isolated-once-native-high-glb"], &request_bytes);
    assert!(first.status.success(), "first: {:?}", first.stderr);
    assert!(second.status.success(), "second: {:?}", second.stderr);
    assert_eq!(
        first.stdout, second.stdout,
        "two independent GLB replays differ"
    );
    let response: WorkerResponse = serde_json::from_slice(&first.stdout).expect("GLB response");
    validate_response(&response, "native-high-glb-materialize-1").expect("generic response");
    let result = response.result.expect("GLB result");
    validate_native_high_glb_materialize_result(&result).expect("strict GLB result");
    assert_eq!(result["runtime_write_performed"], false);
    assert_eq!(result["strict_readback"]["triangle_count"], 32);
    assert_eq!(result["strict_readback"]["base_triangle_count"], 12);
    assert_eq!(result["strict_readback"]["detail_triangle_count"], 20);
    assert_eq!(result["strict_readback"]["part_ids"], json!(["receiver"]));
}

#[test]
fn native_high_glb_materialization_rejects_unknown_hash_and_closed_entry_drift() {
    let request = glb_materialize_request();
    let mut unknown = request.clone();
    unknown["payload"]["unexpected"] = Value::Bool(true);
    let output = run_worker(
        &["--isolated-once-native-high-glb"],
        &serde_json::to_vec(&unknown).unwrap(),
    );
    assert!(!output.status.success());

    let mut bad_hash = request.clone();
    bad_hash["payload"]["input_canonical_sha256"] = Value::String("0".repeat(64));
    let output = run_worker(
        &["--isolated-once-native-high-glb"],
        &serde_json::to_vec(&bad_hash).unwrap(),
    );
    assert!(!output.status.success());

    let output = run_worker(&["--isolated-once"], &serde_json::to_vec(&request).unwrap());
    assert_eq!(output.status.code(), Some(64));
}

#[test]
fn native_high_glb_materialization_rejects_tampered_base64_result() {
    let request = glb_materialize_request();
    let output = run_worker(
        &["--isolated-once-native-high-glb"],
        &serde_json::to_vec(&request).unwrap(),
    );
    assert!(output.status.success());
    let response: Value = serde_json::from_slice(&output.stdout).expect("GLB response");
    let mut result = response["result"].clone();
    let encoded = result["glb_base64"].as_str().unwrap();
    let replacement = if encoded.as_bytes()[0] == b'A' {
        'B'
    } else {
        'A'
    };
    let mut tampered = encoded.to_owned();
    tampered.replace_range(0..1, &replacement.to_string());
    result["glb_base64"] = Value::String(tampered);
    assert!(validate_native_high_glb_materialize_result(&result).is_err());
}
