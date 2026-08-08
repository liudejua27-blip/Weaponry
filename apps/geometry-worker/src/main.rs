use forgecad_worker_protocol::{WorkerRequest, WorkerResponse};
use std::io::{self, BufRead, Write};

fn main() {
    let mut stdout = io::BufWriter::new(io::stdout());
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let response = match serde_json::from_str::<WorkerRequest>(&line) {
            Ok(request) => WorkerResponse::unavailable(request.request_id, "geometry"),
            Err(error) => WorkerResponse {
                protocol: forgecad_worker_protocol::WORKER_PROTOCOL.to_owned(),
                request_id: "unknown".to_owned(),
                ok: false,
                result: None,
                error: Some(forgecad_worker_protocol::WorkerError {
                    code: "PARSE_ERROR".to_owned(),
                    message: error.to_string(),
                }),
            },
        };
        serde_json::to_writer(&mut stdout, &response).expect("worker response serializes");
        stdout.write_all(b"\n").expect("worker response writes");
        stdout.flush().expect("worker response flushes");
    }
}
