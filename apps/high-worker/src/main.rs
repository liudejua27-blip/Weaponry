//! Dedicated one-shot transport for the standalone Native High Worker.
//!
//! Runtime launches this binary with the closed Native High entry point and
//! sends the ordinary ForgeCAD `WorkerRequest` envelope. The request payload
//! remains the High worker's private JSON contract; this adapter projects it
//! into the protocol crate's closed Native High envelope so the transport
//! budget, canonical payload hash/size, cohort binding, and result hash are
//! validated by one shared implementation. The result is projected back to
//! the generic `WorkerResponse` shape expected by the existing Runtime
//! sibling launcher.
//!
//! This process has no filesystem, network, subprocess, script, SQLite, CAS,
//! or Runtime-state access. It may perform the bounded in-memory GLB lowering
//! exposed by its second closed entry. It reads one request, writes one
//! response, and exits.

use forgecad_high_worker::{
    authoring_mesh_v2 as authoring_mesh_v2_high, evaluator as high_evaluator,
    lower_high_mesh_artifact, run_json, HighMeshArtifact, ARTIFACT_SCHEMA_VERSION, OPERATION,
    REQUEST_SCHEMA_VERSION,
};
use forgecad_worker_protocol::{
    build_cohort_sha256, canonical_json_bytes, canonical_json_sha256,
    validate_authoring_mesh_v2_high_artifact_materialize_request,
    validate_authoring_mesh_v2_high_artifact_materialize_result,
    validate_native_high_glb_materialize_payload, validate_native_high_glb_materialize_result,
    validate_native_high_request, validate_native_high_response, validate_request,
    NativeHighWorkerBudget, NativeHighWorkerError, NativeHighWorkerRequestEnvelope,
    NativeHighWorkerResponseEnvelope, WorkerError, WorkerRequest, WorkerResponse,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_ENTRY,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_GLB_KIND,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_MIME,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_OPERATION,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_READBACK_KIND,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_REQUEST_SCHEMA_VERSION,
    AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_RESULT_SCHEMA_VERSION,
    AUTHORING_MESH_V2_HIGH_ENTRY, AUTHORING_MESH_V2_HIGH_OPERATION,
    AUTHORING_MESH_V2_HIGH_REQUEST_SCHEMA_VERSION, AUTHORING_MESH_V2_HIGH_RESULT_SCHEMA_VERSION,
    MAX_WORKER_RESPONSE_BYTES, NATIVE_HIGH_EVALUATOR_ENTRY, NATIVE_HIGH_EVALUATOR_OPERATION,
    NATIVE_HIGH_EVALUATOR_REQUEST_SCHEMA_VERSION, NATIVE_HIGH_EVALUATOR_RESULT_SCHEMA_VERSION,
    NATIVE_HIGH_GLB_MATERIALIZE_ENTRY, NATIVE_HIGH_GLB_MATERIALIZE_OPERATION,
    NATIVE_HIGH_GLB_REQUEST_SCHEMA_VERSION, NATIVE_HIGH_GLB_RESULT_SCHEMA_VERSION,
    NATIVE_HIGH_MAX_MEMORY_BYTES, NATIVE_HIGH_MAX_PAYLOAD_BYTES, NATIVE_HIGH_MAX_REQUEST_BYTES,
    NATIVE_HIGH_MAX_RESULT_BYTES, NATIVE_HIGH_MAX_RUNTIME_MS,
    NATIVE_HIGH_REQUEST_ENVELOPE_SCHEMA_VERSION, NATIVE_HIGH_RESPONSE_ENVELOPE_SCHEMA_VERSION,
    NATIVE_HIGH_RESULT_SCHEMA_VERSION, NATIVE_HIGH_WORKER_ENTRY, NATIVE_HIGH_WORKER_OPERATION,
    WORKER_PROTOCOL,
};
use serde_json::Value;
use std::io::{self, Read, Write};

const INVALID_REQUEST_ID: &str = "invalid-request";

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--build-identity"] {
        let identity = serde_json::json!({
            "schema_version":"ForgeCADHighWorkerBuildIdentity@1",
            "component":"forgecad-high-worker",
            "build_cohort_sha256":build_cohort_sha256(),
            "entry":NATIVE_HIGH_WORKER_ENTRY,
            "request_schema":REQUEST_SCHEMA_VERSION,
            "artifact_schema":ARTIFACT_SCHEMA_VERSION,
            "operation":OPERATION,
            "glb_operation":NATIVE_HIGH_GLB_MATERIALIZE_OPERATION,
            "glb_entry":NATIVE_HIGH_GLB_MATERIALIZE_ENTRY,
            "glb_request_schema":NATIVE_HIGH_GLB_REQUEST_SCHEMA_VERSION,
            "glb_result_schema":NATIVE_HIGH_GLB_RESULT_SCHEMA_VERSION,
            "evaluator_entry":NATIVE_HIGH_EVALUATOR_ENTRY,
            "evaluator_operation":NATIVE_HIGH_EVALUATOR_OPERATION,
            "evaluator_request_schema":NATIVE_HIGH_EVALUATOR_REQUEST_SCHEMA_VERSION,
            "evaluator_result_schema":NATIVE_HIGH_EVALUATOR_RESULT_SCHEMA_VERSION,
            "authoring_mesh_v2_high_entry":AUTHORING_MESH_V2_HIGH_ENTRY,
            "authoring_mesh_v2_high_execution_operation":AUTHORING_MESH_V2_HIGH_OPERATION,
            "authoring_mesh_v2_high_execution_request_schema":AUTHORING_MESH_V2_HIGH_REQUEST_SCHEMA_VERSION,
            "authoring_mesh_v2_high_result_operation":authoring_mesh_v2_high::OPERATION,
            "authoring_mesh_v2_high_result_schema":AUTHORING_MESH_V2_HIGH_RESULT_SCHEMA_VERSION,
            "authoring_mesh_v2_high_algorithm_sha256":authoring_mesh_v2_high::algorithm_sha256(),
            "authoring_mesh_v2_high_artifact_materialize_entry":AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_ENTRY,
            "authoring_mesh_v2_high_artifact_materialize_operation":AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_OPERATION,
            "authoring_mesh_v2_high_artifact_materialize_request_schema":AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_REQUEST_SCHEMA_VERSION,
            "authoring_mesh_v2_high_artifact_materialize_result_schema":AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_RESULT_SCHEMA_VERSION,
            "authoring_mesh_v2_high_artifact_materialize_glb_kind":AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_GLB_KIND,
            "authoring_mesh_v2_high_artifact_materialize_readback_kind":AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_READBACK_KIND
        });
        println!(
            "{}",
            serde_json::to_string(&identity).expect("identity serialization")
        );
        return;
    }
    if args == [NATIVE_HIGH_WORKER_ENTRY] {
        std::process::exit(run_isolated_once(NATIVE_HIGH_WORKER_ENTRY));
    }
    if args == [NATIVE_HIGH_GLB_MATERIALIZE_ENTRY] {
        std::process::exit(run_isolated_once(NATIVE_HIGH_GLB_MATERIALIZE_ENTRY));
    }
    if args == [NATIVE_HIGH_EVALUATOR_ENTRY] {
        std::process::exit(run_high_evaluator_once());
    }
    if args == [AUTHORING_MESH_V2_HIGH_ENTRY] {
        std::process::exit(run_authoring_mesh_v2_high_once());
    }
    if args == [AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_ENTRY] {
        std::process::exit(run_authoring_mesh_v2_high_artifact_materialize_once());
    }
    eprintln!(
        "forgecad-high-worker: expected --build-identity, {}, {}, {}, {}, or {}",
        NATIVE_HIGH_WORKER_ENTRY,
        NATIVE_HIGH_GLB_MATERIALIZE_ENTRY,
        NATIVE_HIGH_EVALUATOR_ENTRY,
        AUTHORING_MESH_V2_HIGH_ENTRY,
        AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_ENTRY
    );
    std::process::exit(64);
}

/// Read exactly one bounded request. Reading until EOF preserves the existing
/// sibling-worker lifecycle: a second JSON request cannot be smuggled into a
/// long-lived process.
fn run_isolated_once(entry: &str) -> i32 {
    let request_bytes = match read_bounded_stdin() {
        Ok(bytes) => bytes,
        Err(message) => {
            let response = error_response(INVALID_REQUEST_ID, "WORKER_PROTOCOL", message);
            return finish_response(response);
        }
    };

    let request = match serde_json::from_slice::<WorkerRequest>(&request_bytes) {
        Ok(request) => request,
        Err(_) => {
            let response = error_response(
                INVALID_REQUEST_ID,
                "WORKER_PROTOCOL",
                "native high worker request is not valid strict JSON",
            );
            return finish_response(response);
        }
    };
    let response = if entry == NATIVE_HIGH_GLB_MATERIALIZE_ENTRY {
        handle_glb_materialize_request(request)
    } else {
        handle_request(request)
    };
    finish_response(response)
}

fn finish_response(response: WorkerResponse) -> i32 {
    let ok = response.ok;
    if !emit_response(response) {
        1
    } else if ok {
        0
    } else {
        1
    }
}

fn handle_request(request: WorkerRequest) -> WorkerResponse {
    let request_id = safe_request_id(&request.request_id);
    if request.operation != NATIVE_HIGH_WORKER_OPERATION {
        return error_response(
            &request_id,
            "HIGH_WORKER_OPERATION_NOT_ALLOWED",
            "native high worker accepts only the closed high-mesh operation",
        );
    }
    if validate_request(&request).is_err() {
        return error_response(
            &request_id,
            "WORKER_PROTOCOL",
            "native high worker request failed the generic protocol gate",
        );
    }

    let native_request = native_high_request(&request);
    if validate_native_high_request(&native_request).is_err() {
        return error_response(
            &request_id,
            "WORKER_PROTOCOL",
            "native high worker request failed the closed transport gate",
        );
    }

    match run_native_high(&native_request) {
        Ok(result) => {
            let response = NativeHighWorkerResponseEnvelope {
                schema_version: NATIVE_HIGH_RESPONSE_ENVELOPE_SCHEMA_VERSION.to_owned(),
                protocol: WORKER_PROTOCOL.to_owned(),
                request_id: native_request.request_id.clone(),
                operation: native_request.operation.clone(),
                build_cohort_sha256: native_request.build_cohort_sha256.clone(),
                ok: true,
                result: Some(result.clone()),
                result_sha256: Some(canonical_json_sha256(&result)),
                result_bytes: Some(canonical_json_bytes(&result).len() as u64),
                error: None,
            };
            if validate_native_high_response(&response, &native_request).is_err() {
                return error_response(
                    &request_id,
                    "WORKER_PROTOCOL",
                    "native high worker result failed the closed response gate",
                );
            }
            WorkerResponse {
                protocol: WORKER_PROTOCOL.to_owned(),
                request_id,
                build_cohort_sha256: build_cohort_sha256(),
                ok: true,
                result: Some(result),
                error: None,
            }
        }
        Err(error) => {
            let (code, message) = map_high_error(&error.to_string());
            let native_error = NativeHighWorkerError {
                code: code.to_owned(),
                message: message.to_owned(),
            };
            let response = NativeHighWorkerResponseEnvelope {
                schema_version: NATIVE_HIGH_RESPONSE_ENVELOPE_SCHEMA_VERSION.to_owned(),
                protocol: WORKER_PROTOCOL.to_owned(),
                request_id: native_request.request_id.clone(),
                operation: native_request.operation.clone(),
                build_cohort_sha256: native_request.build_cohort_sha256.clone(),
                ok: false,
                result: None,
                result_sha256: None,
                result_bytes: None,
                error: Some(native_error),
            };
            if validate_native_high_response(&response, &native_request).is_err() {
                return error_response(
                    &request_id,
                    "WORKER_PROTOCOL",
                    "native high worker error failed the closed response gate",
                );
            }
            let error = response.error.expect("validated Native High error");
            WorkerResponse {
                protocol: WORKER_PROTOCOL.to_owned(),
                request_id,
                build_cohort_sha256: build_cohort_sha256(),
                ok: false,
                result: None,
                error: Some(WorkerError {
                    code: error.code,
                    message: error.message,
                }),
            }
        }
    }
}

fn handle_glb_materialize_request(request: WorkerRequest) -> WorkerResponse {
    let request_id = safe_request_id(&request.request_id);
    if request.operation != NATIVE_HIGH_GLB_MATERIALIZE_OPERATION {
        return error_response(
            &request_id,
            "HIGH_WORKER_OPERATION_NOT_ALLOWED",
            "native high GLB operation is not allowed",
        );
    }
    if validate_request(&request).is_err()
        || validate_native_high_glb_materialize_payload(&request.payload).is_err()
    {
        return error_response(
            &request_id,
            "HIGH_GLB_REQUEST_INVALID",
            "native high GLB request failed the closed transport gate",
        );
    }
    let artifact_value = request
        .payload
        .get("artifact")
        .cloned()
        .expect("validated Native High GLB artifact");
    let artifact = match serde_json::from_value::<HighMeshArtifact>(artifact_value) {
        Ok(artifact) => artifact,
        Err(_) => {
            return error_response(
                &request_id,
                "HIGH_GLB_REQUEST_INVALID",
                "native high GLB artifact schema is invalid",
            )
        }
    };
    let lowered = match lower_high_mesh_artifact(&artifact) {
        Ok(lowered) => lowered,
        Err(_) => {
            return error_response(
                &request_id,
                "HIGH_GLB_READBACK_REJECTED",
                "native high GLB lowering or readback failed",
            )
        }
    };
    let strict_readback = match serde_json::to_value(&lowered.readback) {
        Ok(value) => value,
        Err(_) => {
            return error_response(
                &request_id,
                "HIGH_GLB_READBACK_REJECTED",
                "native high GLB readback serialization failed",
            )
        }
    };
    let mut result = serde_json::json!({
        "schema_version": NATIVE_HIGH_GLB_RESULT_SCHEMA_VERSION,
        "glb_base64": base64_encode(&lowered.glb),
        "glb_sha256": lowered.glb_sha256,
        "strict_readback": strict_readback,
        "runtime_write_performed": false,
        "canonical_sha256": ""
    });
    result["canonical_sha256"] = Value::String(canonical_json_sha256(&result));
    if validate_native_high_glb_materialize_result(&result).is_err() {
        return error_response(
            &request_id,
            "HIGH_GLB_READBACK_REJECTED",
            "native high GLB result failed the closed response gate",
        );
    }
    WorkerResponse {
        protocol: WORKER_PROTOCOL.to_owned(),
        request_id,
        build_cohort_sha256: build_cohort_sha256(),
        ok: true,
        result: Some(result),
        error: None,
    }
}

/// Dedicated one-shot evaluator transport.  It intentionally uses the
/// generic Worker envelope, but its nested request/result remain owned by the
/// High crate and are validated again by `evaluator::run_json`.  There is no
/// Runtime/MCP dispatch or persistence path behind this entry yet.
fn run_high_evaluator_once() -> i32 {
    let request_bytes = match read_bounded_stdin() {
        Ok(bytes) => bytes,
        Err(message) => {
            return finish_response(error_response(
                INVALID_REQUEST_ID,
                "WORKER_PROTOCOL",
                message,
            ))
        }
    };
    let request = match serde_json::from_slice::<WorkerRequest>(&request_bytes) {
        Ok(request) => request,
        Err(_) => {
            return finish_response(error_response(
                INVALID_REQUEST_ID,
                "WORKER_PROTOCOL",
                "high evaluator request is not valid strict JSON",
            ))
        }
    };
    let request_id = safe_request_id(&request.request_id);
    if request.operation != NATIVE_HIGH_EVALUATOR_OPERATION
        || validate_request(&request).is_err()
        || request
            .payload
            .get("schema_version")
            .and_then(Value::as_str)
            != Some(NATIVE_HIGH_EVALUATOR_REQUEST_SCHEMA_VERSION)
    {
        return finish_response(error_response(
            &request_id,
            "HIGH_EVALUATOR_REQUEST_INVALID",
            "high evaluator request failed the closed transport gate",
        ));
    }
    let payload_bytes = canonical_json_bytes(&request.payload);
    let output = match high_evaluator::run_json(&payload_bytes) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(result)
                if result.get("schema_version").and_then(Value::as_str)
                    == Some(NATIVE_HIGH_EVALUATOR_RESULT_SCHEMA_VERSION) =>
            {
                WorkerResponse {
                    protocol: WORKER_PROTOCOL.to_owned(),
                    request_id,
                    build_cohort_sha256: build_cohort_sha256(),
                    ok: true,
                    result: Some(result),
                    error: None,
                }
            }
            _ => error_response(
                &request_id,
                "HIGH_EVALUATOR_RESULT_INVALID",
                "high evaluator result failed the closed response gate",
            ),
        },
        Err(error) => {
            let code = if error.0.contains("MODULE_UNAVAILABLE")
                || error.0.contains("NOT_VENDORED_OR_LINKED")
            {
                "CAPABILITY_UNAVAILABLE"
            } else if error.0.contains("REPLAY_NON_DETERMINISTIC") {
                "WORKER_DETERMINISM_MISMATCH"
            } else if error.0.contains("HASH") || error.0.contains("CANONICAL") {
                "WORKER_HASH_MISMATCH"
            } else {
                "HIGH_EVALUATOR_FAILED"
            };
            let message = if error.0.contains('/')
                || error.0.contains('\\')
                || error.0.chars().any(char::is_control)
                || !error.0.is_ascii()
            {
                "high evaluator rejected request".to_owned()
            } else {
                error.0.chars().take(192).collect()
            };
            error_response(&request_id, code, message)
        }
    };
    finish_response(output)
}

/// One-shot transport for the existing direct AuthoringMesh V2 High bridge.
/// Runtime remains the only writer: this process validates and evaluates one
/// immutable revision, returns one closed result, and has no CAS/SQLite access.
fn run_authoring_mesh_v2_high_once() -> i32 {
    let request_bytes = match read_bounded_stdin() {
        Ok(bytes) => bytes,
        Err(message) => {
            return finish_response(error_response(
                INVALID_REQUEST_ID,
                "WORKER_PROTOCOL",
                message,
            ))
        }
    };
    let request = match serde_json::from_slice::<WorkerRequest>(&request_bytes) {
        Ok(request) => request,
        Err(_) => {
            return finish_response(error_response(
                INVALID_REQUEST_ID,
                "WORKER_PROTOCOL",
                "AuthoringMesh V2 High request is not valid strict JSON",
            ))
        }
    };
    let request_id = safe_request_id(&request.request_id);
    if request.operation != AUTHORING_MESH_V2_HIGH_OPERATION
        || validate_request(&request).is_err()
        || request
            .payload
            .get("schema_version")
            .and_then(Value::as_str)
            != Some(AUTHORING_MESH_V2_HIGH_REQUEST_SCHEMA_VERSION)
    {
        return finish_response(error_response(
            &request_id,
            "AUTHORING_MESH_V2_HIGH_REQUEST_INVALID",
            "AuthoringMesh V2 High request failed the closed transport gate",
        ));
    }

    let payload_bytes = canonical_json_bytes(&request.payload);
    let response = match authoring_mesh_v2_high::run_execution_json(&payload_bytes) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(result)
                if result.get("schema_version").and_then(Value::as_str)
                    == Some(AUTHORING_MESH_V2_HIGH_RESULT_SCHEMA_VERSION) =>
            {
                WorkerResponse {
                    protocol: WORKER_PROTOCOL.to_owned(),
                    request_id,
                    build_cohort_sha256: build_cohort_sha256(),
                    ok: true,
                    result: Some(result),
                    error: None,
                }
            }
            _ => error_response(
                &request_id,
                "AUTHORING_MESH_V2_HIGH_RESULT_INVALID",
                "AuthoringMesh V2 High result failed the closed response gate",
            ),
        },
        Err(error) => {
            let code = if error.0.contains("REPLAY_NON_DETERMINISTIC") {
                "WORKER_DETERMINISM_MISMATCH"
            } else if error.0.contains("HASH") || error.0.contains("CANONICAL") {
                "WORKER_HASH_MISMATCH"
            } else if error.0.contains("BUDGET") {
                "HIGH_WORKER_BUDGET_INVALID"
            } else {
                "AUTHORING_MESH_V2_HIGH_FAILED"
            };
            let message = if error.0.is_ascii()
                && !error.0.contains('/')
                && !error.0.contains('\\')
                && !error.0.chars().any(char::is_control)
            {
                error.0.chars().take(192).collect()
            } else {
                "AuthoringMesh V2 High evaluator rejected the request".to_owned()
            };
            error_response(&request_id, code, message)
        }
    };
    finish_response(response)
}

/// One-shot second step for the direct V2 High bridge.  This operation takes
/// the already evaluated/hash-bound `AuthoringMeshV2HighResult@2` as input,
/// lowers only its evaluated parts into the embedded GLB container, and
/// returns a strict V2-specific readback.  It has no Runtime/CAS access.
fn run_authoring_mesh_v2_high_artifact_materialize_once() -> i32 {
    let request_bytes = match read_bounded_stdin() {
        Ok(bytes) => bytes,
        Err(message) => {
            return finish_response(error_response(
                INVALID_REQUEST_ID,
                "WORKER_PROTOCOL",
                message,
            ))
        }
    };
    let request = match serde_json::from_slice::<WorkerRequest>(&request_bytes) {
        Ok(request) => request,
        Err(_) => {
            return finish_response(error_response(
                INVALID_REQUEST_ID,
                "WORKER_PROTOCOL",
                "AuthoringMesh V2 High artifact request is not valid strict JSON",
            ))
        }
    };
    let request_id = safe_request_id(&request.request_id);
    if request.operation != AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_OPERATION
        || validate_request(&request).is_err()
        || validate_authoring_mesh_v2_high_artifact_materialize_request(&request.payload).is_err()
    {
        return finish_response(error_response(
            &request_id,
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_REQUEST_INVALID",
            "AuthoringMesh V2 High artifact request failed the closed transport gate",
        ));
    }
    let high_result_value = request
        .payload
        .get("high_result")
        .cloned()
        .expect("validated nested High result");
    let expected_high_hash = request
        .payload
        .get("high_result_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if high_result_value
        .get("canonical_sha256")
        .and_then(Value::as_str)
        != Some(expected_high_hash)
    {
        return finish_response(error_response(
            &request_id,
            "AUTHORING_MESH_V2_HIGH_ARTIFACT_REQUEST_INVALID",
            "AuthoringMesh V2 High result semantic hash is invalid",
        ));
    }
    let cohort = build_cohort_sha256();
    let source_cohort = match request
        .payload
        .get("source_high_worker_build_cohort_sha256")
        .and_then(|value| {
            if value.is_null() {
                Some(None)
            } else {
                value.as_str().map(|cohort| Some(cohort.to_owned()))
            }
        }) {
        Some(source_cohort) => source_cohort,
        None => {
            return finish_response(error_response(
                &request_id,
                "AUTHORING_MESH_V2_HIGH_ARTIFACT_REQUEST_INVALID",
                "AuthoringMesh V2 High source cohort is invalid",
            ))
        }
    };
    if source_cohort != cohort {
        return finish_response(error_response(
            &request_id,
            "WORKER_COHORT_MISMATCH",
            "AuthoringMesh V2 High source cohort differs from materializer cohort",
        ));
    }
    let lowered = match forgecad_high_worker::glb::lower_authoring_mesh_v2_high_result_wire(
        &high_result_value,
        cohort.as_deref(),
    ) {
        Ok(lowered) => lowered,
        Err(error) => {
            let error_text = error.0;
            let code = error_text.rsplit(':').next().unwrap_or(error_text.as_str());
            let code = if code.starts_with("AUTHORING_MESH_V2_HIGH_")
                || code.starts_with("HIGH_GLB_")
                || code.starts_with("RESULT_")
            {
                code
            } else {
                "HIGH_GLB_READBACK_REJECTED"
            };
            return finish_response(error_response(
                &request_id,
                code,
                "AuthoringMesh V2 High artifact lowering or readback failed",
            ));
        }
    };
    let strict_readback = match serde_json::to_value(&lowered.readback) {
        Ok(value) => value,
        Err(_) => {
            return finish_response(error_response(
                &request_id,
                "HIGH_GLB_READBACK_REJECTED",
                "AuthoringMesh V2 High artifact readback serialization failed",
            ))
        }
    };
    let mut result = serde_json::json!({
        "schema_version": AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_RESULT_SCHEMA_VERSION,
        "operation": AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_OPERATION,
        "request_kind": "artifact_materialize",
        "status": "materialized",
        "high_result": high_result_value,
        "high_result_sha256": expected_high_hash,
        "source_high_worker_build_cohort_sha256": source_cohort,
        "artifact_id": lowered.artifact_id,
        "artifact_sha256": lowered.artifact_sha256,
        "glb_base64": base64_encode(&lowered.glb),
        "glb_sha256": lowered.glb_sha256,
        "strict_readback": strict_readback,
        "glb_mime": AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_MIME,
        "glb_kind": AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_GLB_KIND,
        "readback_kind": AUTHORING_MESH_V2_HIGH_ARTIFACT_MATERIALIZE_READBACK_KIND,
        "runtime_write_performed": false,
        "canonical_sha256": ""
    });
    result["canonical_sha256"] = Value::String(canonical_json_sha256(&result));
    if validate_authoring_mesh_v2_high_artifact_materialize_result(&result).is_err() {
        return finish_response(error_response(
            &request_id,
            "HIGH_GLB_READBACK_REJECTED",
            "AuthoringMesh V2 High artifact result failed the closed response gate",
        ));
    }
    finish_response(WorkerResponse {
        protocol: WORKER_PROTOCOL.to_owned(),
        request_id,
        build_cohort_sha256: cohort,
        ok: true,
        result: Some(result),
        error: None,
    })
}

fn native_high_request(request: &WorkerRequest) -> NativeHighWorkerRequestEnvelope {
    let payload_bytes = canonical_json_bytes(&request.payload);
    NativeHighWorkerRequestEnvelope {
        schema_version: NATIVE_HIGH_REQUEST_ENVELOPE_SCHEMA_VERSION.to_owned(),
        protocol: request.protocol.clone(),
        request_id: request.request_id.clone(),
        operation: request.operation.clone(),
        build_cohort_sha256: build_cohort_sha256(),
        payload_sha256: canonical_json_sha256(&request.payload),
        payload_bytes: payload_bytes.len() as u64,
        payload: request.payload.clone(),
        budget: NativeHighWorkerBudget {
            max_runtime_ms: NATIVE_HIGH_MAX_RUNTIME_MS,
            max_memory_bytes: NATIVE_HIGH_MAX_MEMORY_BYTES,
            max_input_bytes: NATIVE_HIGH_MAX_PAYLOAD_BYTES as u64,
            max_output_bytes: NATIVE_HIGH_MAX_RESULT_BYTES as u64,
        },
        timeout_ms: NATIVE_HIGH_MAX_RUNTIME_MS,
    }
}

fn run_native_high(
    request: &NativeHighWorkerRequestEnvelope,
) -> Result<Value, forgecad_high_worker::HighWorkerError> {
    // The protocol gate has already checked canonical bytes and the fixed
    // input budget. Passing those bytes to High keeps the payload hash-bound
    // representation identical at both transport and worker boundaries.
    let payload_bytes = canonical_json_bytes(&request.payload);
    let result_bytes = run_json(&payload_bytes)?;
    let result: Value = serde_json::from_slice(&result_bytes)
        .map_err(forgecad_high_worker::HighWorkerError::from)?;
    if result.get("schema_version").and_then(Value::as_str)
        != Some(NATIVE_HIGH_RESULT_SCHEMA_VERSION)
    {
        return Err(forgecad_high_worker::HighWorkerError(
            "HIGH_WORKER_RESULT_SCHEMA_MISMATCH".to_owned(),
        ));
    }
    Ok(result)
}

fn map_high_error(error: &str) -> (&'static str, String) {
    if error.contains("REQUEST_TOO_LARGE") {
        (
            "HIGH_WORKER_REQUEST_TOO_LARGE",
            "request too large".to_owned(),
        )
    } else if error.contains("REQUEST_CANONICAL_MISMATCH") {
        (
            "HIGH_WORKER_REQUEST_CANONICAL_MISMATCH",
            "request canonical hash mismatch".to_owned(),
        )
    } else if error.contains("OPERATION_NOT_ALLOWED") {
        (
            "HIGH_WORKER_OPERATION_NOT_ALLOWED",
            "operation not allowed".to_owned(),
        )
    } else if error.contains("SCHEMA_MISMATCH") {
        (
            "HIGH_WORKER_REQUEST_SCHEMA_MISMATCH",
            "request schema mismatch".to_owned(),
        )
    } else if error.contains("JSON_INVALID") {
        (
            "HIGH_WORKER_JSON_INVALID",
            "request JSON invalid".to_owned(),
        )
    } else if error.contains("REPLAY_NON_DETERMINISTIC") {
        (
            "WORKER_DETERMINISM_MISMATCH",
            "worker determinism mismatch".to_owned(),
        )
    } else if error.contains("HASH") || error.contains("CANONICAL") {
        ("WORKER_HASH_MISMATCH", "worker hash mismatch".to_owned())
    } else if error.contains("BUDGET") {
        (
            "HIGH_WORKER_BUDGET_INVALID",
            "worker budget invalid".to_owned(),
        )
    } else {
        // High validation diagnostics are fixed ASCII tokens plus bounded typed
        // identifiers. Preserve that closed reason across the process boundary
        // so Runtime can distinguish contract drift from a generic crash.
        let message = if error.contains('/')
            || error.contains('\\')
            || error.chars().any(char::is_control)
            || !error.is_ascii()
        {
            "native high worker rejected request".to_owned()
        } else {
            error.chars().take(192).collect()
        };
        ("HIGH_WORKER_FAILED", message)
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0] as u32;
        let second = chunk.get(1).copied().unwrap_or(0) as u32;
        let third = chunk.get(2).copied().unwrap_or(0) as u32;
        let value = (first << 16) | (second << 8) | third;
        output.push(TABLE[((value >> 18) & 63) as usize] as char);
        output.push(TABLE[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn error_response(request_id: &str, code: &str, message: impl Into<String>) -> WorkerResponse {
    WorkerResponse {
        protocol: WORKER_PROTOCOL.to_owned(),
        request_id: safe_request_id(request_id),
        build_cohort_sha256: build_cohort_sha256(),
        ok: false,
        result: None,
        error: Some(WorkerError {
            code: code.to_owned(),
            message: message.into().chars().take(512).collect(),
        }),
    }
}

fn safe_request_id(request_id: &str) -> String {
    if !request_id.is_empty()
        && request_id.len() <= 128
        && request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        request_id.to_owned()
    } else {
        INVALID_REQUEST_ID.to_owned()
    }
}

fn emit_response(response: WorkerResponse) -> bool {
    let bytes = match serde_json::to_value(&response) {
        Ok(value) => {
            let bytes = canonical_json_bytes(&value);
            if bytes.len().saturating_add(1) <= MAX_WORKER_RESPONSE_BYTES {
                bytes
            } else {
                let fallback = error_response(
                    INVALID_REQUEST_ID,
                    "WORKER_PROTOCOL",
                    "worker response exceeded the bounded protocol limit",
                );
                let Ok(fallback) = serde_json::to_value(fallback) else {
                    return false;
                };
                canonical_json_bytes(&fallback)
            }
        }
        Err(_) => return false,
    };
    let mut stdout = io::BufWriter::new(io::stdout());
    stdout.write_all(&bytes).is_ok() && stdout.write_all(b"\n").is_ok() && stdout.flush().is_ok()
}

fn read_bounded_stdin() -> Result<Vec<u8>, String> {
    let mut input = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut stdin = io::stdin().lock();
    loop {
        let read = stdin
            .read(&mut buffer)
            .map_err(|_| "native high worker request could not be read".to_owned())?;
        if read == 0 {
            break;
        }
        if input.len().saturating_add(read) > NATIVE_HIGH_MAX_REQUEST_BYTES {
            return Err(
                "native high worker request exceeded the bounded protocol limit".to_owned(),
            );
        }
        input.extend_from_slice(&buffer[..read]);
    }
    if input.is_empty() {
        return Err("native high worker request is empty".to_owned());
    }
    Ok(input)
}
