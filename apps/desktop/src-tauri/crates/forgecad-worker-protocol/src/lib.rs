use serde::{Deserialize, Serialize};

pub const WORKER_PROTOCOL: &str = "forgecad-worker-protocol@1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRequest {
    pub protocol: String,
    pub request_id: String,
    pub operation: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub protocol: String,
    pub request_id: String,
    pub ok: bool,
    pub result: Option<serde_json::Value>,
    pub error: Option<WorkerError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerError {
    pub code: String,
    pub message: String,
}

impl WorkerResponse {
    pub fn unavailable(request_id: String, worker: &str) -> Self {
        Self {
            protocol: WORKER_PROTOCOL.to_owned(),
            request_id,
            ok: false,
            result: None,
            error: Some(WorkerError {
                code: "CAPABILITY_UNAVAILABLE".to_owned(),
                message: format!("{worker} worker is not enabled in MCP001"),
            }),
        }
    }
}
