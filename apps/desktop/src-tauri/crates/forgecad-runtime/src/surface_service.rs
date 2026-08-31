//! Runtime-owned Surface service.
//!
//! Surface is the durable High/Low/UV/Cage/Bake/PBR aggregate in the
//! Weaponry runtime.  This module owns the operation dispatch seam for that
//! aggregate and the two Appearance source-lineage entry points that used to
//! live in `lib.rs`.  The service is only a borrow-only view over `Runtime`;
//! all Store, CAS, validation, replay, and idempotency behavior remains in the
//! existing typed implementations.

use crate::{appearance_source_lineage, Runtime, RuntimeError};
use serde_json::{json, Value};

/// Surface operations that may still arrive through the compatibility IPC
/// entry point.  The typed Weaponry router reaches this same implementation
/// directly through `SurfaceService`; this list only preserves old callers.
const SURFACE_OPERATION_NAMES: &[&str] = &[
    "appearance_prepare",
    "appearance_source_lineage_prepare",
    "appearance_source_lineage_get",
    "authoring_mesh_v2_high_artifact_prepare",
    "authoring_mesh_v2_high_artifact_get",
    "hero_uv_durable_prepare",
    "hero_uv_durable_get",
    "production_knife_uv_bake_v2_prepare",
    "production_knife_uv_bake_v2_get",
    "low_quad_draft_durable_prepare",
    "low_quad_draft_durable_get",
    "production_weapon_form_quality_v2_preflight_get",
    "production_weapon_formal_high_prepare",
    "production_weapon_formal_high_get",
    "production_weapon_high_low_bake_prepare",
    "production_weapon_high_low_bake_get",
    "production_weapon_high_low_bake_preflight_get",
    "production_weapon_retopology_cage_source_bundle_prepare",
    "production_weapon_retopology_cage_source_bundle_get",
    "production_weapon_retopology_cage_source_prepare",
    "production_weapon_retopology_cage_source_get",
];

/// Return whether `operation` belongs to the Surface aggregate's legacy IPC
/// compatibility set.
pub(crate) fn is_surface_operation(operation: &str) -> bool {
    SURFACE_OPERATION_NAMES.contains(&operation)
}

/// Dispatch one Surface operation through its existing typed Runtime method.
///
/// This function deliberately does not call `Runtime::dispatch_ipc`, avoiding
/// a generic dispatcher cycle for the migrated Surface aggregate.  The
/// methods below continue to be the Runtime-owned write boundary and retain
/// their existing Store/CAS/hash/replay semantics byte-for-byte.
pub(crate) fn invoke(
    runtime: &Runtime,
    operation: &str,
    payload: &Value,
) -> Result<Value, RuntimeError> {
    match operation {
        "appearance_prepare" => {
            let project_id = payload
                .get("project_id")
                .and_then(Value::as_str)
                .ok_or_else(|| RuntimeError::InvalidInput("project_id is required".to_owned()))?;
            let base_version_id = payload.get("base_version_id").and_then(Value::as_str);
            let request = payload.get("request").cloned().unwrap_or_else(|| json!({}));
            Ok(runtime.prepare_appearance_candidate(project_id, base_version_id, request)?)
        }
        "appearance_source_lineage_prepare" => {
            Ok(runtime.appearance_source_lineage_prepare(payload)?)
        }
        "appearance_source_lineage_get" => Ok(runtime.appearance_source_lineage_get(payload)?),
        "authoring_mesh_v2_high_artifact_prepare" => {
            Ok(runtime.authoring_mesh_v2_high_artifact_prepare(payload)?)
        }
        "authoring_mesh_v2_high_artifact_get" => {
            Ok(runtime.authoring_mesh_v2_high_artifact_get(payload)?)
        }
        "hero_uv_durable_prepare" => Ok(runtime.hero_uv_durable_prepare(payload.clone())?),
        "hero_uv_durable_get" => Ok(runtime.hero_uv_durable_get(payload.clone())?),
        "production_knife_uv_bake_v2_prepare" => {
            Ok(runtime.production_knife_uv_bake_v2_prepare(payload.clone())?)
        }
        "production_knife_uv_bake_v2_get" => {
            Ok(runtime.production_knife_uv_bake_v2_get(payload.clone())?)
        }
        "low_quad_draft_durable_prepare" => {
            Ok(runtime.low_quad_draft_durable_prepare(payload.clone())?)
        }
        "low_quad_draft_durable_get" => Ok(runtime.low_quad_draft_durable_get(payload.clone())?),
        "production_weapon_form_quality_v2_preflight_get" => {
            Ok(runtime.production_weapon_form_quality_v2_preflight_get(payload.clone())?)
        }
        "production_weapon_formal_high_prepare" => {
            Ok(runtime.production_weapon_formal_high_prepare(payload.clone())?)
        }
        "production_weapon_formal_high_get" => {
            Ok(runtime.production_weapon_formal_high_get(payload.clone())?)
        }
        "production_weapon_high_low_bake_prepare" => {
            Ok(runtime.production_weapon_high_low_bake_prepare(payload.clone())?)
        }
        "production_weapon_high_low_bake_get" => {
            Ok(runtime.production_weapon_high_low_bake_get(payload.clone())?)
        }
        "production_weapon_high_low_bake_preflight_get" => {
            Ok(runtime.production_weapon_high_low_bake_preflight_get(payload.clone())?)
        }
        "production_weapon_retopology_cage_source_bundle_prepare" => {
            Ok(runtime.production_weapon_retopology_cage_source_bundle_prepare(payload.clone())?)
        }
        "production_weapon_retopology_cage_source_bundle_get" => {
            Ok(runtime.production_weapon_retopology_cage_source_bundle_get(payload.clone())?)
        }
        "production_weapon_retopology_cage_source_prepare" => {
            Ok(runtime.production_weapon_retopology_cage_source_prepare(payload.clone())?)
        }
        "production_weapon_retopology_cage_source_get" => {
            Ok(runtime.production_weapon_retopology_cage_source_get(payload.clone())?)
        }
        _ => Err(RuntimeError::InvalidInput(format!(
            "RUNTIME_SURFACE_OPERATION_UNKNOWN: operation {operation} is not owned by Surface"
        ))),
    }
}

impl Runtime {
    /// Persist one immutable candidate-bound Appearance source lineage
    /// sidecar for a three-LOD weapon cohort.
    pub fn appearance_source_lineage_prepare(
        &self,
        request: &Value,
    ) -> Result<Value, RuntimeError> {
        appearance_source_lineage::prepare(self, request)
    }

    /// Read and re-verify one durable Appearance source lineage sidecar after
    /// Runtime restart. Missing/tampered source objects fail closed.
    pub fn appearance_source_lineage_get(&self, request: &Value) -> Result<Value, RuntimeError> {
        appearance_source_lineage::get(self, request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_compatibility_set_covers_the_complete_surface_aggregate() {
        assert_eq!(SURFACE_OPERATION_NAMES.len(), 21);
        for operation in [
            "appearance_prepare",
            "appearance_source_lineage_prepare",
            "appearance_source_lineage_get",
            "authoring_mesh_v2_high_artifact_prepare",
            "authoring_mesh_v2_high_artifact_get",
            "hero_uv_durable_prepare",
            "hero_uv_durable_get",
            "production_knife_uv_bake_v2_prepare",
            "production_knife_uv_bake_v2_get",
            "low_quad_draft_durable_prepare",
            "low_quad_draft_durable_get",
            "production_weapon_form_quality_v2_preflight_get",
            "production_weapon_formal_high_prepare",
            "production_weapon_formal_high_get",
            "production_weapon_high_low_bake_prepare",
            "production_weapon_high_low_bake_get",
            "production_weapon_high_low_bake_preflight_get",
            "production_weapon_retopology_cage_source_bundle_prepare",
            "production_weapon_retopology_cage_source_bundle_get",
            "production_weapon_retopology_cage_source_prepare",
            "production_weapon_retopology_cage_source_get",
        ] {
            assert!(is_surface_operation(operation), "missing {operation}");
        }
        assert!(!is_surface_operation("candidate_confirm"));
    }

    #[test]
    fn appearance_prepare_keeps_legacy_input_error_through_surface_service() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = invoke(&runtime, "appearance_prepare", &Value::Null)
            .expect_err("invalid appearance request");
        assert_eq!(
            error.to_string(),
            "invalid runtime input: project_id is required"
        );
    }

    #[test]
    fn legacy_ipc_surface_bridge_uses_the_same_typed_service() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = runtime
            .dispatch_ipc("appearance_prepare", &Value::Null)
            .expect_err("invalid appearance request");
        assert_eq!(
            error.to_string(),
            "invalid runtime input: project_id is required"
        );
    }
}
