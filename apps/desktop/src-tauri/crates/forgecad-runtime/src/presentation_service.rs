//! Runtime-owned Presentation service for the default Weaponry knife profile.
//!
//! The checked-in knife profile is the active inventory for this service.  The
//! service is a borrow-only view over [`Runtime`]: every operation delegates to
//! the existing typed Runtime method, so Store/CAS writes retain the Runtime
//! single-writer boundary and compatibility callers observe the same result.

use crate::{Runtime, RuntimeError};
use serde_json::Value;

/// The exact read operation inventory of the `fps_presentation` façade.
///
/// Keep this list in the same order as
/// `packages/forgecad-contracts/profiles/weaponry-knife-p0.json`.  It is the
/// only active Presentation inventory; historical animation and
/// fictional-energy operations remain outside this service.
pub(crate) const PRESENTATION_READ_OPERATIONS: &[&str] = &[
    "fps_presentation_package_v2_candidate_get",
    "fps_presentation_package_v2_get",
    "fps_presentation_package_v2_production_preflight_get",
    "game_weapon_anchor_get",
    "game_weapon_animated_glb_socket_get",
    "game_weapon_animated_glb_socket_transform_projection_get",
    "game_weapon_animated_glb_socket_transform_projection_v2_get",
    "mechanical_animation_clip_get",
    "mechanical_animation_clip_preview_get",
    "mechanical_animation_clip_v2_get",
    "mechanical_animation_clip_v2_preview",
    "mechanical_animation_glb_v2_get",
];

/// The exact write operation inventory of the `fps_presentation` façade.
pub(crate) const PRESENTATION_WRITE_OPERATIONS: &[&str] = &[
    "fps_presentation_package_v2_candidate_prepare",
    "fps_presentation_package_v2_prepare",
    "game_weapon_anchor_prepare",
    "game_weapon_animated_glb_socket_prepare",
    "game_weapon_animated_glb_socket_transform_projection_prepare",
    "game_weapon_animated_glb_socket_transform_projection_v2_prepare",
    "mechanical_animation_clip_prepare",
    "mechanical_animation_clip_v2_prepare",
    "mechanical_animation_glb_v2_prepare",
];

/// Return whether `operation` belongs to the active Presentation service.
pub(crate) fn is_presentation_operation(operation: &str) -> bool {
    PRESENTATION_READ_OPERATIONS.contains(&operation)
        || PRESENTATION_WRITE_OPERATIONS.contains(&operation)
}

/// Invoke one active Presentation operation through its typed Runtime method.
///
/// The default domain router and the legacy IPC bridge both call this exact
/// function.  The exhaustive local match intentionally has no generic route:
/// an operation cannot silently leave the Presentation boundary.
pub(crate) fn invoke(
    runtime: &Runtime,
    operation: &str,
    payload: &Value,
) -> Result<Value, RuntimeError> {
    match operation {
        "fps_presentation_package_v2_candidate_get" => {
            runtime.fps_presentation_package_v2_candidate_get(payload)
        }
        "fps_presentation_package_v2_get" => runtime.fps_presentation_package_v2_get(payload),
        "fps_presentation_package_v2_production_preflight_get" => {
            runtime.fps_presentation_package_v2_production_preflight_get(payload)
        }
        "game_weapon_anchor_get" => runtime.game_weapon_anchor_get(payload),
        "game_weapon_animated_glb_socket_get" => {
            runtime.game_weapon_animated_glb_socket_get(payload)
        }
        "game_weapon_animated_glb_socket_transform_projection_get" => {
            runtime.game_weapon_animated_glb_socket_transform_projection_get(payload)
        }
        "game_weapon_animated_glb_socket_transform_projection_v2_get" => {
            runtime.game_weapon_animated_glb_socket_transform_projection_v2_get(payload)
        }
        "mechanical_animation_clip_get" => runtime.mechanical_animation_clip_get(payload),
        "mechanical_animation_clip_preview_get" => {
            runtime.mechanical_animation_clip_preview_get(payload)
        }
        "mechanical_animation_clip_v2_get" => runtime.mechanical_animation_clip_v2_get(payload),
        "mechanical_animation_clip_v2_preview" => {
            runtime.mechanical_animation_clip_v2_preview_get(payload)
        }
        "mechanical_animation_glb_v2_get" => runtime.mechanical_animation_glb_v2_get(payload),

        "fps_presentation_package_v2_candidate_prepare" => {
            runtime.fps_presentation_package_v2_candidate_prepare(payload)
        }
        "fps_presentation_package_v2_prepare" => {
            runtime.fps_presentation_package_v2_prepare(payload)
        }
        "game_weapon_anchor_prepare" => runtime.game_weapon_anchor_prepare(payload),
        "game_weapon_animated_glb_socket_prepare" => {
            runtime.game_weapon_animated_glb_socket_prepare(payload)
        }
        "game_weapon_animated_glb_socket_transform_projection_prepare" => {
            runtime.game_weapon_animated_glb_socket_transform_projection_prepare(payload)
        }
        "game_weapon_animated_glb_socket_transform_projection_v2_prepare" => {
            runtime.game_weapon_animated_glb_socket_transform_projection_v2_prepare(payload)
        }
        "mechanical_animation_clip_prepare" => runtime.mechanical_animation_clip_prepare(payload),
        "mechanical_animation_clip_v2_prepare" => {
            runtime.mechanical_animation_clip_v2_prepare(payload)
        }
        "mechanical_animation_glb_v2_prepare" => {
            runtime.mechanical_animation_glb_v2_prepare(payload)
        }
        _ => Err(RuntimeError::InvalidInput(format!(
            "RUNTIME_PRESENTATION_OPERATION_UNKNOWN: operation {operation} is not owned by Presentation"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_services::RuntimeServiceDomain;
    use serde_json::json;
    use std::collections::BTreeSet;

    #[test]
    fn inventory_matches_the_exact_fps_presentation_profile() {
        assert_eq!(PRESENTATION_READ_OPERATIONS.len(), 12);
        assert_eq!(PRESENTATION_WRITE_OPERATIONS.len(), 9);

        let mut operations = BTreeSet::new();
        operations.extend(PRESENTATION_READ_OPERATIONS.iter().copied());
        operations.extend(PRESENTATION_WRITE_OPERATIONS.iter().copied());
        assert_eq!(operations.len(), 21);
        assert!(operations.contains("fps_presentation_package_v2_get"));
        assert!(
            operations.contains("game_weapon_animated_glb_socket_transform_projection_v2_prepare")
        );
        assert!(operations.contains("mechanical_animation_glb_v2_get"));
        assert!(!is_presentation_operation(
            "mechanical_animation_glb_prepare"
        ));
        assert!(!is_presentation_operation("fictional_energy_vfx_get"));
    }

    #[test]
    fn direct_service_rejects_out_of_domain_before_runtime_dispatch() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = invoke(&runtime, "candidate_confirm", &Value::Null)
            .expect_err("Delivery operation must not enter Presentation");
        assert!(error
            .to_string()
            .contains("RUNTIME_PRESENTATION_OPERATION_UNKNOWN"));
        assert_eq!(
            runtime.presentation_service().boundary().domain,
            RuntimeServiceDomain::Presentation
        );
    }

    #[test]
    fn compatibility_bridge_reuses_presentation_typed_service() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let direct = invoke(&runtime, "fps_presentation_package_v2_get", &Value::Null)
            .expect_err("invalid presentation package request");
        let bridged = runtime
            .dispatch_ipc("fps_presentation_package_v2_get", &Value::Null)
            .expect_err("legacy presentation package request");
        assert_eq!(direct.to_string(), bridged.to_string());
        assert!(direct
            .to_string()
            .contains("FPS_PRESENTATION_PACKAGE_V2_REJECTED"));
    }

    #[test]
    fn write_and_read_routes_keep_their_profile_access_class() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let boundary = runtime.presentation_service().boundary();
        for operation in PRESENTATION_READ_OPERATIONS {
            assert_eq!(
                boundary.supports(operation),
                Some(crate::runtime_services::RuntimeServiceOperationAccess::Read),
                "read route changed: {operation}"
            );
        }
        for operation in PRESENTATION_WRITE_OPERATIONS {
            assert_eq!(
                boundary.supports(operation),
                Some(crate::runtime_services::RuntimeServiceOperationAccess::Write),
                "write route changed: {operation}"
            );
        }
        assert_eq!(boundary.supports("mechanical_animation_glb_prepare"), None);
    }

    #[test]
    fn invalid_domain_envelope_fails_before_presentation_service_or_store() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = runtime
            .invoke_weaponry_operation(
                RuntimeServiceDomain::Delivery,
                "fps_presentation_package_v2_get",
                &json!({"package_id":"package-1"}),
            )
            .expect_err("cross-domain Presentation operation must fail closed");
        assert!(error
            .to_string()
            .contains("RUNTIME_OPERATION_DOMAIN_MISMATCH"));
    }
}
