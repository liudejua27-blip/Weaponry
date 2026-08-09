use forgecad_worker_protocol::{WorkerRequest, WorkerResponse, WORKER_PROTOCOL};
use std::io::{self, BufRead, Write};

fn main() {
    if std::env::args().skip(1).eq(["--build-identity"]) {
        println!(
            "{}",
            serde_json::json!({
                "schema_version": "ForgeCADDevBuildIdentity@1",
                "component": "forgecad-geometry-worker",
                "build_cohort_sha256": option_env!("FORGECAD_BUILD_COHORT_SHA256")
            })
        );
        return;
    }
    let mut stdout = io::BufWriter::new(io::stdout());
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let response = match serde_json::from_str::<WorkerRequest>(&line) {
            Ok(request) => match forgecad_geometry_worker::worker_result(&serde_json::to_value(&request).expect("request serializes")) {
                Ok(result) => WorkerResponse { protocol: WORKER_PROTOCOL.to_owned(), request_id: request.request_id, ok: true, result: Some(result), error: None },
                Err(error) => WorkerResponse { protocol: WORKER_PROTOCOL.to_owned(), request_id: request.request_id, ok: false, result: None, error: Some(forgecad_worker_protocol::WorkerError { code: "GEOMETRY_REJECTED".to_owned(), message: error.to_string() }) },
            },
            Err(error) => WorkerResponse { protocol: WORKER_PROTOCOL.to_owned(), request_id: "unknown".to_owned(), ok: false, result: None, error: Some(forgecad_worker_protocol::WorkerError { code: "PARSE_ERROR".to_owned(), message: error.to_string() }) },
        };
        serde_json::to_writer(&mut stdout, &response).expect("worker response serializes");
        stdout.write_all(b"\n").expect("worker response writes");
        stdout.flush().expect("worker response flushes");
    }
}
