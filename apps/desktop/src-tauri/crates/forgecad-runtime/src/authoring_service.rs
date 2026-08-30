//! Runtime-owned Authoring service seam.
//!
//! This module is intentionally small while the historical Runtime entrypoint
//! is being physically extracted.  It owns the first set of AuthoringMesh and
//! knife curve operations that already have dedicated typed implementations.
//! Operations that have not moved yet are forwarded to the existing Runtime
//! dispatch for compatibility; the router has already checked their domain
//! ownership before this function is reached.

use super::{Runtime, RuntimeError};
use serde_json::Value;

/// Invoke an Authoring operation after the typed domain router has validated
/// its owner.  A moved operation calls its dedicated Runtime implementation,
/// preserving the existing prepare/Store/CAS transaction boundary.
pub(crate) fn invoke(
    runtime: &Runtime,
    operation: &str,
    payload: &Value,
) -> Result<Value, RuntimeError> {
    match operation {
        "authoring_mesh_transaction_prepare" => runtime.authoring_mesh_transaction_prepare(payload),
        "authoring_mesh_v2_durable_prepare" => runtime.authoring_mesh_v2_durable_prepare(payload),
        "knife_curve_modifier_graph_prepare" => runtime.knife_curve_modifier_graph_prepare(payload),
        "knife_curve_modifier_graph_get" => runtime.knife_curve_modifier_graph_get(payload),
        "knife_curve_evaluated_mesh_prepare" => runtime.knife_curve_evaluated_mesh_prepare(payload),
        "knife_curve_evaluated_mesh_get" => runtime.knife_curve_evaluated_mesh_get(payload),
        "weapon_foundation_asset_prepare" => runtime.weapon_foundation_asset_prepare(payload),
        "weapon_foundation_asset_get" => runtime.weapon_foundation_asset_get(payload),
        "weapon_foundation_authoring_materialization_prepare" => {
            runtime.weapon_foundation_authoring_materialization_prepare(payload)
        }
        "weapon_foundation_authoring_materialization_get" => {
            runtime.weapon_foundation_authoring_materialization_get(payload)
        }
        _ => runtime.dispatch_ipc(operation, payload),
    }
}
