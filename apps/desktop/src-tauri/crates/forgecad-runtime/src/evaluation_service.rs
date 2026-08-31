//! Runtime-owned Evaluation service for the default Weaponry knife profile.
//!
//! The public knife profile groups observation, quality/evidence review, and
//! jobs under the Evaluation domain.  This module is the one typed entry
//! point for those operations.  It deliberately calls the existing
//! Runtime-owned implementations directly; it never re-enters
//! `Runtime::dispatch_ipc`, so the default router and the legacy compatibility
//! bridge share the same implementation and error boundary.

use crate::{Runtime, RuntimeError};
use serde_json::Value;

/// Physical Evaluation implementation extracted from the Runtime root.
/// Keeping this as a child of the existing Evaluation service avoids growing
/// the Runtime root-module count while preserving the public Runtime methods.
#[path = "evaluation_reference_comparison.rs"]
pub(crate) mod reference_comparison;

/// The exact read operation inventory of the default knife profile's three
/// Evaluation façades: `observe` (10), `quality_review` (25), and `job` (8).
/// Keep this list derived from the checked-in profile's active operation set,
/// not from the partial capability map used for migration exceptions.
pub(crate) const EVALUATION_READ_OPERATIONS: &[&str] = &[
    // observe (10)
    "artifact_readback_get",
    "authoring_mesh_durable_get",
    "authoring_mesh_get",
    "authoring_mesh_transaction_get",
    "authoring_mesh_v2_durable_get",
    "candidate_get",
    "production_stage_transition_get",
    "scene_observe_get",
    "selection_get",
    "snapshot_get",
    // quality_review (17, including KnifePassState)
    "candidate_material_surface_quality_get",
    "candidate_topology_quality_get",
    "critic_report_get",
    "production_weapon_form_quality_get",
    "production_weapon_form_quality_v2_get",
    "quality_get",
    "render_evidence_integrity_get",
    "render_evidence_replay_get",
    "render_pass_get",
    "silhouette_candidate_compare",
    "silhouette_evaluation_objective_prepare",
    "silhouette_fit_prepare",
    "silhouette_part_error_get",
    "silhouette_rig_hash",
    "silhouette_target_get",
    "visual_evidence_bundle_get",
    // knife pass state (2)
    "knife_pass_state_get",
    // job (4)
    "job_events_read",
    "job_get",
    "job_result_get",
    "optimization_job_get",
];

/// The exact write operation inventory of the default knife profile's three
/// Evaluation façades: `quality_review` (8, including KnifePassState) and `job` (4).
pub(crate) const EVALUATION_WRITE_OPERATIONS: &[&str] = &[
    // quality_review (8, including KnifePassState)
    "candidate_material_surface_quality_prepare",
    "candidate_topology_quality_prepare",
    "human_visual_review_submit",
    "production_weapon_form_quality_prepare",
    "production_weapon_form_quality_v2_prepare",
    "high_artifact_reference_compare_prepare",
    "reference_compare_prepare",
    "visual_review_submit",
    "knife_pass_state_prepare",
    // job (4)
    "job_cancel",
    "optimization_job_prepare",
    "optimization_job_resume",
    "primary_form_repair_job_prepare",
];

/// Return whether an operation is owned by the active Evaluation service.
pub(crate) fn is_evaluation_operation(operation: &str) -> bool {
    EVALUATION_READ_OPERATIONS.contains(&operation)
        || EVALUATION_WRITE_OPERATIONS.contains(&operation)
}

/// Invoke one active Evaluation operation through its typed Runtime method.
///
/// The operation has already passed the Contract/profile route when called by
/// the default router.  The compatibility bridge also uses this function for
/// the exact same active operation set.  The local match remains exhaustive
/// so an inventory entry cannot silently fall back to the legacy dispatcher.
pub(crate) fn invoke(
    runtime: &Runtime,
    operation: &str,
    payload: &Value,
) -> Result<Value, RuntimeError> {
    match operation {
        // observe
        "artifact_readback_get" => {
            let artifact_id = required_str(payload, "artifact_id")?;
            let candidate_id = required_str(payload, "candidate_id")?;
            runtime.artifact_readback(artifact_id, candidate_id)
        }
        "authoring_mesh_durable_get" => runtime.authoring_mesh_durable_get(payload),
        "authoring_mesh_get" => runtime.authoring_mesh(payload),
        "authoring_mesh_transaction_get" => runtime.authoring_mesh_transaction_get(payload),
        "authoring_mesh_v2_durable_get" => runtime.authoring_mesh_v2_durable_get(payload),
        "candidate_get" => {
            let candidate_id = required_str(payload, "candidate_id")?;
            serde_json::to_value(runtime.candidate(candidate_id)?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
        }
        "production_stage_transition_get" => {
            runtime.production_stage_transition_get(payload.clone())
        }
        "scene_observe_get" => {
            let project_id = required_str(payload, "project_id")?;
            runtime.agentic_scene_observe(
                project_id,
                payload.get("candidate_id").and_then(Value::as_str),
            )
        }
        "selection_get" => serde_json::to_value(runtime.selection())
            .map_err(|error| RuntimeError::InvalidInput(error.to_string())),
        "snapshot_get" => {
            let snapshot_id = required_str(payload, "snapshot_id")?;
            serde_json::to_value(runtime.snapshot(snapshot_id)?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
        }

        // quality_review
        "candidate_material_surface_quality_prepare" => {
            runtime.candidate_material_surface_quality_prepare(payload.clone())
        }
        "candidate_material_surface_quality_get" => {
            runtime.candidate_material_surface_quality_get(payload.clone())
        }
        "critic_report_get" => {
            let project_id = required_str(payload, "project_id")?;
            let observation_sha256 = required_str(payload, "observation_sha256")?;
            runtime.agentic_critic_projection_bound(
                project_id,
                payload.get("candidate_id").and_then(Value::as_str),
                observation_sha256,
            )
        }
        "production_weapon_form_quality_prepare" => {
            runtime.production_weapon_form_quality_prepare(payload.clone())
        }
        "production_weapon_form_quality_get" => {
            runtime.production_weapon_form_quality_get(payload.clone())
        }
        "production_weapon_form_quality_v2_prepare" => {
            runtime.production_weapon_form_quality_v2_prepare(payload.clone())
        }
        "production_weapon_form_quality_v2_get" => {
            runtime.production_weapon_form_quality_v2_get(payload.clone())
        }
        "quality_get" => {
            let candidate_id = required_str(payload, "candidate_id")?;
            runtime.quality(
                candidate_id,
                payload.get("reference_id").and_then(Value::as_str),
            )
        }
        "render_evidence_integrity_get" => runtime.render_evidence_integrity_get(payload.clone()),
        "render_evidence_replay_get" => runtime.render_evidence_replay_get(payload.clone()),
        "render_pass_get" => {
            let render_set_hash = required_str(payload, "render_set_hash")?;
            let pass = required_str(payload, "pass")?;
            runtime.render_pass_get(render_set_hash, pass)
        }
        "silhouette_candidate_compare" => {
            let project_id = required_str(payload, "project_id")?;
            runtime.silhouette_candidate_compare(project_id, payload.clone())
        }
        "silhouette_evaluation_objective_prepare" => {
            let project_id = required_str(payload, "project_id")?;
            runtime.silhouette_evaluation_objective_prepare(project_id, payload.clone())
        }
        "silhouette_fit_prepare" => {
            let project_id = required_str(payload, "project_id")?;
            runtime.silhouette_fit_prepare(project_id, payload.clone())
        }
        "silhouette_part_error_get" => {
            let project_id = required_str(payload, "project_id")?;
            runtime.silhouette_part_error(project_id, payload.clone())
        }
        "silhouette_rig_hash" => {
            let project_id = required_str(payload, "project_id")?;
            runtime.silhouette_rig_hash(project_id, payload)
        }
        "silhouette_target_get" => {
            let target_sha256 = required_str(payload, "target_sha256")?;
            runtime.silhouette_target_get(target_sha256)
        }
        "visual_evidence_bundle_get" => {
            let project_id = required_str(payload, "project_id")?;
            let candidate_id = required_str(payload, "candidate_id")?;
            let observation_sha256 = required_str(payload, "observation_sha256")?;
            runtime.agentic_visual_evidence_bundle_bound(
                project_id,
                candidate_id,
                observation_sha256,
            )
        }
        "candidate_topology_quality_prepare" => {
            runtime.candidate_topology_quality_prepare(payload.clone())
        }
        "candidate_topology_quality_get" => runtime.candidate_topology_quality_get(payload.clone()),
        "reference_compare_prepare" => {
            let project_id = required_str(payload, "project_id")?;
            runtime.prepare_reference_comparison(project_id, payload.clone())
        }
        "high_artifact_reference_compare_prepare" => {
            let project_id = required_str(payload, "project_id")?;
            runtime.prepare_high_artifact_reference_comparison(project_id, payload.clone())
        }
        "visual_review_submit" => runtime.submit_visual_review(payload.clone()),
        "human_visual_review_submit" => runtime.submit_human_visual_review(payload.clone()),
        "knife_pass_state_prepare" => runtime.knife_pass_state_prepare(payload),
        "knife_pass_state_get" => runtime.knife_pass_state_get(payload),

        // job
        "job_events_read" => {
            let job_id = required_str(payload, "job_id")?;
            let after_sequence = payload
                .get("after_sequence")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            serde_json::to_value(runtime.job_events(job_id, after_sequence)?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
        }
        "job_get" => {
            let job_id = required_str(payload, "job_id")?;
            serde_json::to_value(runtime.job(job_id)?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
        }
        "job_result_get" => {
            let job_id = required_str(payload, "job_id")?;
            runtime.job_result(job_id)
        }
        "optimization_job_get" => runtime.optimization_job_get(payload.clone()),
        "job_cancel" => {
            let job_id = required_str(payload, "job_id")?;
            serde_json::to_value(runtime.cancel_job(job_id)?)
                .map_err(|error| RuntimeError::InvalidInput(error.to_string()))
        }
        "optimization_job_prepare" => runtime.optimization_job_prepare(payload.clone()),
        "optimization_job_resume" => runtime.optimization_job_resume(payload.clone()),
        "primary_form_repair_job_prepare" => {
            let project_id = required_str(payload, "project_id")?;
            let base_version_id = payload.get("base_version_id").and_then(Value::as_str);
            runtime.primary_form_repair_job_prepare(project_id, base_version_id, payload.clone())
        }
        _ => Err(RuntimeError::InvalidInput(format!(
            "RUNTIME_EVALUATION_OPERATION_UNKNOWN: operation {operation} is not owned by Evaluation"
        ))),
    }
}

fn required_str<'payload>(
    payload: &'payload Value,
    key: &str,
) -> Result<&'payload str, RuntimeError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RuntimeError::InvalidInput(format!("{key} is required")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_services::RuntimeServiceDomain;
    use serde_json::json;
    use std::collections::BTreeSet;

    #[test]
    fn inventory_matches_all_three_default_evaluation_facades() {
        assert_eq!(EVALUATION_READ_OPERATIONS.len(), 31);
        assert_eq!(EVALUATION_WRITE_OPERATIONS.len(), 13);
        let mut operations = BTreeSet::new();
        operations.extend(EVALUATION_READ_OPERATIONS.iter().copied());
        operations.extend(EVALUATION_WRITE_OPERATIONS.iter().copied());
        assert_eq!(operations.len(), 43);
        assert!(operations.contains("scene_observe_get"));
        assert!(operations.contains("critic_report_get"));
        assert!(operations.contains("visual_evidence_bundle_get"));
        assert!(operations.contains("optimization_job_prepare"));
        assert!(!is_evaluation_operation("appearance_prepare"));
    }

    #[test]
    fn direct_service_rejects_out_of_domain_before_runtime_dispatch() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = invoke(&runtime, "candidate_confirm", &Value::Null)
            .expect_err("Delivery operation must not enter Evaluation");
        assert!(error
            .to_string()
            .contains("RUNTIME_EVALUATION_OPERATION_UNKNOWN"));
        assert_eq!(
            runtime.evaluation_service().boundary().domain,
            RuntimeServiceDomain::Evaluation
        );
    }

    #[test]
    fn compatibility_bridge_reuses_evaluation_typed_service() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let direct =
            invoke(&runtime, "quality_get", &Value::Null).expect_err("invalid quality request");
        let bridged = runtime
            .dispatch_ipc("quality_get", &Value::Null)
            .expect_err("legacy quality request");
        assert_eq!(direct.to_string(), bridged.to_string());
        assert_eq!(
            direct.to_string(),
            "invalid runtime input: candidate_id is required"
        );
    }

    #[test]
    fn invalid_domain_envelope_fails_before_evaluation_service_or_store() {
        let runtime = Runtime::ephemeral().expect("runtime");
        let error = runtime
            .invoke_weaponry_operation(
                RuntimeServiceDomain::Delivery,
                "quality_get",
                &json!({"candidate_id":"candidate-1"}),
            )
            .expect_err("cross-domain Evaluation operation must fail closed");
        assert!(error
            .to_string()
            .contains("RUNTIME_OPERATION_DOMAIN_MISMATCH"));
    }
}
