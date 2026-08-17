use serde_json::Value;

/// One immutable target/camera pair consumed by the joint optimizer.
#[derive(Debug, Clone)]
pub(crate) struct OptimizationViewContext {
    pub view_id: String,
    pub kind: String,
    pub target_sha256: String,
    pub target: Value,
    pub target_mask: Vec<bool>,
    pub camera: Value,
    pub camera_hash: String,
    pub weight: f64,
    pub primary: bool,
}
