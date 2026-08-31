//! Runtime-owned Delivery service for the default Weaponry knife profile.
//!
//! Delivery contains the game-asset preparation path and the approval
//! lifecycle.  The service is a borrow-only view over `Runtime`; the existing
//! typed methods remain responsible for Store/CAS transactions, validation,
//! idempotency, and immutable version/export semantics.

use crate::{Runtime, RuntimeError};
use forgecad_contracts::{
    CandidateConfirmRequest, CandidateRejectRequest, ExportConfirmRequest, ExportPrepareRequest,
};
use serde_json::Value;

/// The exact active read inventory of the Delivery domain.
pub(crate) const DELIVERY_READ_OPERATIONS: &[&str] = &[
    // delivery façade (3)
    "game_asset_delivery_get",
    "game_asset_lod_derive",
    "game_weapon_glb_socket_get",
    // approval façade (1)
    "version_diff",
];

/// The exact active write inventory of the Delivery domain.
pub(crate) const DELIVERY_WRITE_OPERATIONS: &[&str] = &[
    // delivery façade (3)
    "export_prepare",
    "game_asset_delivery_prepare",
    "game_weapon_glb_socket_prepare",
    // approval façade (4)
    "candidate_confirm",
    "candidate_reject",
    "cross_view_promotion_confirm",
    "export_confirm",
];

/// Return whether `operation` belongs to one of the two active Delivery
/// façades (`delivery` or `approval`).  This set is also used by the legacy
/// IPC bridge, so compatibility callers reach this same typed implementation.
pub(crate) fn is_delivery_operation(operation: &str) -> bool {
    DELIVERY_READ_OPERATIONS.contains(&operation) || DELIVERY_WRITE_OPERATIONS.contains(&operation)
}

/// Invoke one active Delivery operation through its typed Runtime method.
///
/// The match is intentionally exhaustive over the active inventory.  An
/// operation cannot silently fall through to the historical generic
/// dispatcher, while the Runtime methods retain their original result and
/// approval semantics.
pub(crate) fn invoke(
    runtime: &Runtime,
    operation: &str,
    payload: &Value,
) -> Result<Value, RuntimeError> {
    match operation {
        // delivery façade
        "game_asset_delivery_get" => runtime.game_asset_delivery_get(payload),
        "game_asset_lod_derive" => runtime.game_asset_lod_derive(payload),
        "game_weapon_glb_socket_get" => runtime.game_weapon_glb_socket_get(payload),
        "export_prepare" => {
            let request: ExportPrepareRequest = serde_json::from_value(payload.clone())
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
            serde_json::to_value(runtime.prepare_export(&request)?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
        }
        // Keep the established three-authored-LOD contract and its durable
        // GameAssetDeliveryLink replay path byte-for-byte compatible. The
        // Dragonfang High/Low projection remains an internal read-only seam
        // until it has its own Store aggregate and replay contract; it must
        // not be selected by this public prepare operation.
        "game_asset_delivery_prepare" => runtime.game_asset_delivery_prepare(payload),
        "game_weapon_glb_socket_prepare" => runtime.game_weapon_glb_socket_prepare(payload),

        // approval façade
        "version_diff" => {
            let version_id = required_str(payload, "version_id")?;
            let compare_to_version_id = required_str(payload, "compare_to_version_id")?;
            runtime.version_diff(version_id, compare_to_version_id)
        }
        "candidate_confirm" => {
            let request: CandidateConfirmRequest = serde_json::from_value(payload.clone())
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
            serde_json::to_value(runtime.confirm_candidate(&request)?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
        }
        "candidate_reject" => {
            let request: CandidateRejectRequest = serde_json::from_value(payload.clone())
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
            serde_json::to_value(runtime.reject_candidate(&request)?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
        }
        "cross_view_promotion_confirm" => runtime.cross_view_promotion_confirm(payload.clone()),
        "export_confirm" => {
            let request: ExportConfirmRequest = serde_json::from_value(payload.clone())
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))?;
            serde_json::to_value(runtime.confirm_export(&request)?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
        }
        _ => Err(RuntimeError::InvalidInput(format!(
            "RUNTIME_DELIVERY_OPERATION_UNKNOWN: operation {operation} is not owned by Delivery"
        ))),
    }
}

fn required_str<'a>(payload: &'a Value, field: &str) -> Result<&'a str, RuntimeError> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{field} is required")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_services::{RuntimeServiceDomain, RuntimeServiceOperationAccess};
    use std::collections::BTreeSet;

    #[test]
    fn inventory_is_exactly_four_reads_and_seven_writes() {
        assert_eq!(DELIVERY_READ_OPERATIONS.len(), 4);
        assert_eq!(DELIVERY_WRITE_OPERATIONS.len(), 7);

        let mut operations = BTreeSet::new();
        operations.extend(DELIVERY_READ_OPERATIONS.iter().copied());
        operations.extend(DELIVERY_WRITE_OPERATIONS.iter().copied());
        assert_eq!(operations.len(), 11);

        for operation in DELIVERY_READ_OPERATIONS {
            assert!(is_delivery_operation(operation));
        }
        for operation in DELIVERY_WRITE_OPERATIONS {
            assert!(is_delivery_operation(operation));
        }
        assert!(!is_delivery_operation("game_weapon_anchor_get"));
        assert!(!is_delivery_operation("fictional_energy_vfx_get"));
    }

    #[test]
    fn delivery_and_approval_facades_have_the_exact_active_split() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let boundary = runtime.delivery_service().boundary();
        assert_eq!(boundary.domain, RuntimeServiceDomain::Delivery);
        assert_eq!(boundary.facade_names, &["delivery", "approval"]);

        for operation in [
            "game_asset_delivery_get",
            "game_asset_lod_derive",
            "game_weapon_glb_socket_get",
            "export_prepare",
            "game_asset_delivery_prepare",
            "game_weapon_glb_socket_prepare",
        ] {
            assert!(
                boundary.facade_names.contains(&"delivery")
                    && (DELIVERY_READ_OPERATIONS.contains(&operation)
                        || DELIVERY_WRITE_OPERATIONS.contains(&operation)),
                "delivery façade operation missing: {operation}"
            );
        }
        for operation in [
            "version_diff",
            "candidate_confirm",
            "candidate_reject",
            "cross_view_promotion_confirm",
            "export_confirm",
        ] {
            assert!(
                boundary.facade_names.contains(&"approval")
                    && (DELIVERY_READ_OPERATIONS.contains(&operation)
                        || DELIVERY_WRITE_OPERATIONS.contains(&operation)),
                "approval façade operation missing: {operation}"
            );
        }
        for operation in DELIVERY_READ_OPERATIONS {
            assert_eq!(
                boundary.supports(operation),
                Some(RuntimeServiceOperationAccess::Read),
                "read route changed: {operation}"
            );
        }
        for operation in DELIVERY_WRITE_OPERATIONS {
            assert_eq!(
                boundary.supports(operation),
                Some(RuntimeServiceOperationAccess::Write),
                "write route changed: {operation}"
            );
        }
    }

    #[test]
    fn cross_domain_delivery_invocation_fails_closed_before_service() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = runtime
            .invoke_weaponry_operation(
                RuntimeServiceDomain::Presentation,
                "game_asset_delivery_get",
                &Value::Null,
            )
            .expect_err("Delivery operation must reject a Presentation envelope");
        assert!(error
            .to_string()
            .contains("RUNTIME_OPERATION_DOMAIN_MISMATCH"));

        let error = runtime
            .delivery_service()
            .invoke("candidate_topology_quality_get", &Value::Null)
            .expect_err("Delivery service must reject Evaluation operation");
        assert!(error
            .to_string()
            .contains("RUNTIME_SERVICE_OPERATION_OUT_OF_BOUND"));
    }

    #[test]
    fn direct_delivery_route_preserves_access_and_owner() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let router = runtime.weaponry_operation_router();
        for operation in DELIVERY_READ_OPERATIONS {
            let route = router.route(operation).expect("Delivery read route");
            assert_eq!(route.domain, RuntimeServiceDomain::Delivery);
            assert_eq!(
                route.access,
                crate::runtime_operation_router::RuntimeOperationAccess::Read
            );
        }
        for operation in DELIVERY_WRITE_OPERATIONS {
            let route = router.route(operation).expect("Delivery write route");
            assert_eq!(route.domain, RuntimeServiceDomain::Delivery);
            assert_eq!(
                route.access,
                crate::runtime_operation_router::RuntimeOperationAccess::Write,
                "write route changed: {operation}"
            );
        }

        let result = runtime
            .invoke_weaponry_operation(RuntimeServiceDomain::Delivery, "version_diff", &Value::Null)
            .expect_err("direct Delivery route should preserve typed validation");
        assert_eq!(
            result.to_string(),
            "invalid runtime input: version_id is required"
        );
    }
}
