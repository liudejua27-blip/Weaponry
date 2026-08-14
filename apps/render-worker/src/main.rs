use forgecad_worker_protocol::{
    build_cohort_sha256, validate_request, WorkerError, WorkerRequest, WorkerResponse,
    MAX_WORKER_REQUEST_BYTES, MAX_WORKER_RESPONSE_BYTES, WORKER_PROTOCOL,
};
use std::io::{self, Read, Write};

const RENDER_OPERATIONS: &[&str] = &["render_fixed", "render_glb", "render_glb_fit_batch"];

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args == ["--build-identity"] {
        println!(
            "{}",
            serde_json::json!({
                "schema_version": "ForgeCADDevBuildIdentity@1",
                "component": "forgecad-render-worker",
                "build_cohort_sha256": build_cohort_sha256()
            })
        );
        return;
    }
    if args != ["--isolated-once"] {
        eprintln!("usage: forgecad-render-worker --isolated-once");
        std::process::exit(2);
    }
    std::process::exit(run_isolated_once());
}

/// Render is deliberately a one-request child. Runtime closes stdin after
/// writing one bounded request; reading to EOF makes a second JSONL request
/// impossible to sneak into the same process and keeps this lifecycle aligned
/// with the Geometry Worker isolation contract.
fn run_isolated_once() -> i32 {
    let request_bytes = match read_bounded_stdin() {
        Ok(bytes) => bytes,
        Err(message) => {
            let mut stdout = io::BufWriter::new(io::stdout());
            let _ = emit(
                &mut stdout,
                error_response("invalid-request", "WORKER_PROTOCOL", message),
            );
            return 1;
        }
    };
    let response = match serde_json::from_slice::<WorkerRequest>(&request_bytes) {
        Ok(request) => handle_request(request),
        Err(error) => WorkerResponse {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "unknown".to_owned(),
            build_cohort_sha256: build_cohort_sha256(),
            ok: false,
            result: None,
            error: Some(forgecad_worker_protocol::WorkerError {
                code: "PARSE_ERROR".to_owned(),
                message: error.to_string(),
            }),
        },
    };
    let ok = response.ok;
    let mut stdout = io::BufWriter::new(io::stdout());
    if !emit(&mut stdout, response) {
        return 1;
    }
    if ok { 0 } else { 1 }
}

fn read_bounded_stdin() -> Result<Vec<u8>, String> {
    let mut input = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut stdin = io::stdin().lock();
    loop {
        let read = stdin
            .read(&mut buffer)
            .map_err(|error| format!("cannot read render request: {error}"))?;
        if read == 0 {
            break;
        }
        if input.len().saturating_add(read) > MAX_WORKER_REQUEST_BYTES {
            return Err("request exceeds the bounded render input".to_owned());
        }
        input.extend_from_slice(&buffer[..read]);
    }
    if input.is_empty() {
        return Err("render request is empty".to_owned());
    }
    Ok(input)
}

fn handle_request(request: WorkerRequest) -> WorkerResponse {
    let request_id = request.request_id.clone();
    if let Err(message) = validate_request(&request) {
        return error_response(&request_id, "WORKER_PROTOCOL", message);
    }
    if !RENDER_OPERATIONS.contains(&request.operation.as_str()) {
        return error_response(
            &request_id,
            "RENDER_WORKER_OPERATION_NOT_ALLOWED",
            "render worker accepts only render operations",
        );
    }
    match forgecad_geometry_worker::render_worker_result(
        &serde_json::to_value(&request).expect("strict request serializes"),
    ) {
        Ok(result) => WorkerResponse {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id,
            build_cohort_sha256: build_cohort_sha256(),
            ok: true,
            result: Some(result),
            error: None,
        },
        Err(error) => error_response(&request_id, "RENDER_REJECTED", error.to_string()),
    }
}

fn error_response(request_id: &str, code: &str, message: impl Into<String>) -> WorkerResponse {
    WorkerResponse {
        protocol: WORKER_PROTOCOL.to_owned(),
        request_id: request_id.to_owned(),
        build_cohort_sha256: build_cohort_sha256(),
        ok: false,
        result: None,
        error: Some(WorkerError {
            code: code.to_owned(),
            message: message.into(),
        }),
    }
}

fn emit(stdout: &mut impl Write, response: WorkerResponse) -> bool {
    let bytes = serde_json::to_vec(&response).expect("worker response serializes");
    if bytes.len() > MAX_WORKER_RESPONSE_BYTES {
        let fallback = error_response(
            &response.request_id,
            "WORKER_RESPONSE_TOO_LARGE",
            "render response exceeds the bounded worker response",
        );
        let fallback_bytes = match serde_json::to_vec(&fallback) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        if stdout.write_all(&fallback_bytes).is_err() {
            return false;
        }
    } else {
        if stdout.write_all(&bytes).is_err() {
            return false;
        }
    }
    stdout.write_all(b"\n").is_ok() && stdout.flush().is_ok()
}
