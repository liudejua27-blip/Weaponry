use base64::Engine;
use forgecad_render_core::{
    render_fixed_glb, render_perspective_glb, render_perspective_glb_fit_at_resolution, RenderPass,
};
use forgecad_worker_protocol::{
    build_cohort_sha256, validate_request, WorkerError, WorkerRequest, WorkerResponse,
    MAX_WORKER_REQUEST_BYTES, MAX_WORKER_RESPONSE_BYTES, WORKER_PROTOCOL,
};
use serde_json::{Map, Value};
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
    match render_worker_result(&request) {
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

fn render_worker_result(request: &WorkerRequest) -> Result<Value, String> {
    let payload = request
        .payload
        .as_object()
        .ok_or_else(|| "payload is required".to_owned())?;
    match request.operation.as_str() {
        "render_fixed" => {
            require_closed_payload(payload, &["glb_base64"])?;
            let glb = decode_render_glb(payload)?;
            let passes = render_fixed_glb(&glb).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerResult@1",
                "passes":serialize_passes(&passes)
            }))
        }
        "render_glb" => {
            require_closed_payload(payload, &["glb_base64", "camera"])?;
            let glb = decode_render_glb(payload)?;
            let camera = payload
                .get("camera")
                .ok_or_else(|| "camera is required".to_owned())?;
            let passes = render_perspective_glb(&glb, camera).map_err(|error| error.to_string())?;
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerResult@2",
                "width":512,
                "height":512,
                "renderer_revision":"forgecad-renderer-2",
                "passes":serialize_passes(&passes)
            }))
        }
        "render_glb_fit_batch" => {
            require_closed_payload(payload, &["glb_base64", "cameras", "resolution"])?;
            let glb = decode_render_glb(payload)?;
            let resolution = payload
                .get("resolution")
                .and_then(Value::as_u64)
                .filter(|value| matches!(*value, 128 | 256 | 512))
                .ok_or_else(|| "fit resolution must be 128, 256 or 512".to_owned())?
                as u32;
            let cameras = payload
                .get("cameras")
                .and_then(Value::as_array)
                .filter(|values| !values.is_empty() && values.len() <= 64)
                .ok_or_else(|| "fit cameras are outside the bounded range".to_owned())?;
            let mut renders = Vec::with_capacity(cameras.len());
            for (index, camera) in cameras.iter().enumerate() {
                let passes = render_perspective_glb_fit_at_resolution(&glb, camera, resolution)
                    .map_err(|error| error.to_string())?;
                renders.push(serde_json::json!({
                    "index":index,
                    "passes":serialize_passes(&passes)
                }));
            }
            Ok(serde_json::json!({
                "schema_version":"RenderWorkerFitBatchResult@1",
                "width":resolution,
                "height":resolution,
                "renderer_revision":"forgecad-renderer-2",
                "renders":renders
            }))
        }
        _ => Err("render worker operation is not allowlisted".to_owned()),
    }
}

fn require_closed_payload(payload: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    if payload.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("worker payload contains an unknown field".to_owned());
    }
    Ok(())
}

fn decode_render_glb(payload: &Map<String, Value>) -> Result<Vec<u8>, String> {
    let encoded = payload
        .get("glb_base64")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "glb_base64 is required".to_owned())?;
    let glb = base64::engine::general_purpose::STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| "glb_base64 is invalid".to_owned())?;
    if glb.is_empty() || glb.len() > 64 * 1024 * 1024 {
        return Err("GLB exceeds the bounded render input".to_owned());
    }
    Ok(glb)
}

fn serialize_passes(passes: &[RenderPass]) -> Vec<Value> {
    passes
        .iter()
        .map(|pass| {
            serde_json::json!({
                "pass":pass.pass,
                "mime":"image/png",
                "width":pass.width,
                "height":pass.height,
                "png_base64":base64::engine::general_purpose::STANDARD.encode(&pass.png)
            })
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_worker_rejects_geometry_compile_payload() {
        let request = WorkerRequest {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id: "render-boundary-test-1".to_owned(),
            operation: "render_fixed".to_owned(),
            payload: serde_json::json!({
                "geometry_program": {},
                "appearance_program": {}
            }),
        };
        let error = render_worker_result(&request)
            .expect_err("render boundary must reject compiler input");
        assert!(error.contains("unknown field"));
    }
}
