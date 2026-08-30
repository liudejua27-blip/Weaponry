//! Explicit compatibility adapter for in-process Runtime dispatch.
//!
//! This module is intentionally limited to the historical compatibility
//! dispatch function. It owns no session, manifest, schema, result, Runtime,
//! Store, or CAS state; all behavior remains the existing Runtime method
//! mapping and error conversion.

use super::*;

pub(crate) fn dispatch_in_process(
    runtime: &Runtime,
    name: &str,
    arguments: &Value,
) -> Result<Value, String> {
    match name {
        "knife_curve_evaluated_mesh_prepare" => runtime
            .knife_curve_evaluated_mesh_prepare(arguments)
            .map_err(|error| error.to_string()),
        "knife_curve_evaluated_mesh_get" => runtime
            .knife_curve_evaluated_mesh_get(arguments)
            .map_err(|error| error.to_string()),
        "knife_curve_modifier_graph_prepare" => runtime
            .knife_curve_modifier_graph_prepare(arguments)
            .map_err(|error| error.to_string()),
        "knife_curve_modifier_graph_get" => runtime
            .knife_curve_modifier_graph_get(arguments)
            .map_err(|error| error.to_string()),
        "design_stage_run_prepare" => runtime
            .design_stage_run_prepare(arguments.clone())
            .map_err(|error| error.to_string()),
        "design_composition_prepare" => runtime
            .design_composition_prepare(arguments.clone())
            .map_err(|error| error.to_string()),
        "cross_view_promotion_confirm" => runtime
            .cross_view_promotion_confirm(arguments.clone())
            .map_err(|error| error.to_string()),
        "optimization_job_get" | "optimization_job_prepare" | "optimization_job_resume" => {
            match name {
                "optimization_job_get" => runtime
                    .optimization_job_get(arguments.clone())
                    .map_err(|error| error.to_string()),
                "optimization_job_prepare" => runtime
                    .optimization_job_prepare(arguments.clone())
                    .map_err(|error| error.to_string()),
                "optimization_job_resume" => runtime
                    .optimization_job_resume(arguments.clone())
                    .map_err(|error| error.to_string()),
                _ => unreachable!("OptimizationJob dispatch arm is exhaustive"),
            }
        }
        "design_action_run_get"
        | "design_action_run_prepare"
        | "design_action_optimization_proposal_prepare"
        | "repair_intent_run_prepare"
        | "repair_apply_prepare"
        | "repair_apply_confirm" => match name {
            "design_action_run_get" => runtime
                .design_action_run_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "design_action_run_prepare" => runtime
                .design_action_run_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "design_action_optimization_proposal_prepare" => runtime
                .design_action_optimization_proposal_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "repair_intent_run_prepare" => runtime
                .repair_intent_run_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "repair_apply_prepare" => runtime
                .repair_apply_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "repair_apply_confirm" => runtime
                .repair_apply_confirm(arguments.clone())
                .map_err(|error| error.to_string()),
            _ => unreachable!("DesignActionRun dispatch arm is exhaustive"),
        },
        "session_create_or_resume"
        | "session_get"
        | "checkpoint_prepare"
        | "checkpoint_get"
        | "checkpoint_restore_prepare"
        | "production_stage_transition_prepare"
        | "production_stage_transition_get"
        | "production_stage_transition_v2_prepare"
        | "production_stage_transition_v2_get"
        | "production_stage_transition_v3_prepare"
        | "production_stage_transition_v3_get"
        | "production_weapon_form_art_baseline_preflight_get"
        | "production_weapon_form_art_baseline_prepare"
        | "production_weapon_form_art_baseline_get"
        | "production_weapon_form_art_evidence_prepare"
        | "production_weapon_form_art_evidence_get"
        | "production_weapon_form_art_mesh_proposal_prepare"
        | "production_weapon_form_art_mesh_proposal_get"
        | "production_weapon_form_art_composite_proposal_prepare"
        | "production_weapon_form_art_composite_proposal_get"
        | "production_weapon_form_art_composite_evidence_prepare"
        | "production_weapon_form_art_composite_evidence_get"
        | "production_weapon_form_art_repair_plan_get"
        | "production_weapon_form_art_failure_diagnostic_get"
        | "production_weapon_form_art_visibility_calibration_get"
        | "production_weapon_form_art_target_occlusion_attribution_get"
        | "production_weapon_form_art_aperture_repair_plan_get"
        | "production_weapon_owner_reviewed_void_calibration_get"
        | "production_weapon_art_decision_proposal_get"
        | "production_weapon_assembly_parameter_sink_get"
        | "production_weapon_form_evidence_prepare"
        | "production_weapon_form_evidence_get"
        | "production_weapon_form_quality_prepare"
        | "production_weapon_form_quality_get"
        | "production_weapon_form_quality_v2_prepare"
        | "production_weapon_form_quality_v2_get"
        | "production_weapon_form_quality_v2_preflight_get"
        | "production_weapon_formal_high_prepare"
        | "production_weapon_formal_high_get"
        | "production_weapon_high_low_bake_prepare"
        | "production_weapon_high_low_bake_get"
        | "production_weapon_high_low_bake_preflight_get"
        | "production_weapon_retopology_cage_source_prepare"
        | "production_weapon_retopology_cage_source_get"
        | "candidate_topology_quality_prepare"
        | "candidate_topology_quality_get"
        | "candidate_material_surface_quality_prepare"
        | "candidate_material_surface_quality_get"
        | "candidate_animation_vfx_quality_prepare"
        | "candidate_animation_vfx_quality_get"
        | "candidate_animation_vfx_quality_v2_prepare"
        | "candidate_animation_vfx_quality_v2_get"
        | "mechanical_animation_clip_v2_prepare"
        | "mechanical_animation_clip_v2_get"
        | "mechanical_animation_clip_v2_preview"
        | "mechanical_animation_glb_v2_prepare"
        | "mechanical_animation_glb_v2_get"
        | "game_weapon_animated_glb_socket_v2_prepare"
        | "game_weapon_animated_glb_socket_v2_get"
        | "fictional_energy_vfx_animated_socket_attachment_prepare"
        | "fictional_energy_vfx_animated_socket_attachment_get"
        | "fictional_energy_vfx_animated_socket_attachment_v2_prepare"
        | "fictional_energy_vfx_animated_socket_attachment_v2_get"
        | "fictional_energy_vfx_animated_socket_attachment_v3_prepare"
        | "fictional_energy_vfx_animated_socket_attachment_v3_get"
        | "game_weapon_animated_glb_socket_transform_projection_prepare"
        | "game_weapon_animated_glb_socket_transform_projection_get"
        | "game_weapon_animated_glb_socket_transform_projection_v2_prepare"
        | "game_weapon_animated_glb_socket_transform_projection_v2_get"
        | "fictional_energy_vfx_animated_socket_particles_sequence_prepare"
        | "fictional_energy_vfx_animated_socket_particles_sequence_get"
        | "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare"
        | "fictional_energy_vfx_animated_socket_particles_sequence_v2_get"
        | "fictional_energy_vfx_animated_socket_trails_sequence_prepare"
        | "fictional_energy_vfx_animated_socket_trails_sequence_get"
        | "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare"
        | "fictional_energy_vfx_animated_socket_trails_sequence_v2_get"
        | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare"
        | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get"
        | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare"
        | "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get" => match name {
            "session_create_or_resume" => runtime
                .session_create_or_resume(arguments.clone())
                .map_err(|error| error.to_string()),
            "session_get" => runtime
                .session_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "checkpoint_prepare" => runtime
                .checkpoint_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "checkpoint_get" => runtime
                .checkpoint_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "checkpoint_restore_prepare" => runtime
                .checkpoint_restore_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_stage_transition_prepare" => runtime
                .production_stage_transition_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_stage_transition_get" => runtime
                .production_stage_transition_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_stage_transition_v2_prepare" => runtime
                .production_stage_transition_v2_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_stage_transition_v2_get" => runtime
                .production_stage_transition_v2_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_stage_transition_v3_prepare" => runtime
                .production_stage_transition_v3_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_stage_transition_v3_get" => runtime
                .production_stage_transition_v3_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_baseline_preflight_get" => runtime
                .production_weapon_form_art_baseline_preflight_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_baseline_prepare" => runtime
                .production_weapon_form_art_baseline_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_baseline_get" => runtime
                .production_weapon_form_art_baseline_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_evidence_prepare" => runtime
                .production_weapon_form_art_evidence_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_evidence_get" => runtime
                .production_weapon_form_art_evidence_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_mesh_proposal_get" => runtime
                .production_weapon_form_art_mesh_proposal_get(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_composite_proposal_prepare" => runtime
                .production_weapon_form_art_composite_proposal_prepare(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_composite_proposal_get" => runtime
                .production_weapon_form_art_composite_proposal_get(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_composite_evidence_prepare" => runtime
                .production_weapon_form_art_composite_evidence_prepare(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_composite_evidence_get" => runtime
                .production_weapon_form_art_composite_evidence_get(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_repair_plan_get" => runtime
                .production_weapon_form_art_repair_plan_get(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_failure_diagnostic_get" => runtime
                .production_weapon_form_art_failure_diagnostic_get(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_visibility_calibration_get" => runtime
                .production_weapon_form_art_visibility_calibration_get(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_target_occlusion_attribution_get" => runtime
                .production_weapon_form_art_target_occlusion_attribution_get(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_aperture_repair_plan_get" => runtime
                .production_weapon_form_art_aperture_repair_plan_get(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_owner_reviewed_void_calibration_get" => runtime
                .production_weapon_owner_reviewed_void_calibration_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_art_mesh_proposal_prepare" => runtime
                .production_weapon_form_art_mesh_proposal_prepare(arguments)
                .map_err(|error| error.to_string()),
            "production_weapon_art_decision_proposal_get" => runtime
                .production_weapon_art_decision_proposal_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_assembly_parameter_sink_get" => runtime
                .production_weapon_assembly_parameter_sink_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_evidence_prepare" => runtime
                .production_weapon_form_evidence_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_evidence_get" => runtime
                .production_weapon_form_evidence_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_quality_prepare" => runtime
                .production_weapon_form_quality_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_quality_get" => runtime
                .production_weapon_form_quality_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_quality_v2_prepare" => runtime
                .production_weapon_form_quality_v2_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_quality_v2_get" => runtime
                .production_weapon_form_quality_v2_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_form_quality_v2_preflight_get" => runtime
                .production_weapon_form_quality_v2_preflight_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_formal_high_prepare" => runtime
                .production_weapon_formal_high_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_formal_high_get" => runtime
                .production_weapon_formal_high_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_high_low_bake_prepare" => runtime
                .production_weapon_high_low_bake_prepare(arguments.clone())
                .map_err(|error| {
                    preserve_production_weapon_high_low_bake_error(&error.to_string())
                }),
            "production_weapon_high_low_bake_get" => runtime
                .production_weapon_high_low_bake_get(arguments.clone())
                .map_err(|error| {
                    preserve_production_weapon_high_low_bake_error(&error.to_string())
                }),
            "production_weapon_high_low_bake_preflight_get" => runtime
                .production_weapon_high_low_bake_preflight_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_retopology_cage_source_prepare" => runtime
                .production_weapon_retopology_cage_source_bundle_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_weapon_retopology_cage_source_get" => runtime
                .production_weapon_retopology_cage_source_bundle_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_camera_lock_prepare" => runtime
                .production_camera_lock_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "production_camera_lock_get" => runtime
                .production_camera_lock_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_topology_quality_prepare" => runtime
                .candidate_topology_quality_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_topology_quality_get" => runtime
                .candidate_topology_quality_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_material_surface_quality_prepare" => runtime
                .candidate_material_surface_quality_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_material_surface_quality_get" => runtime
                .candidate_material_surface_quality_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_animation_vfx_quality_prepare" => runtime
                .candidate_animation_vfx_quality_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_animation_vfx_quality_get" => runtime
                .candidate_animation_vfx_quality_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_animation_vfx_quality_v2_prepare" => runtime
                .candidate_animation_vfx_quality_v2_prepare(arguments.clone())
                .map_err(|error| error.to_string()),
            "candidate_animation_vfx_quality_v2_get" => runtime
                .candidate_animation_vfx_quality_v2_get(arguments.clone())
                .map_err(|error| error.to_string()),
            "mechanical_animation_clip_v2_prepare" => runtime
                .mechanical_animation_clip_v2_prepare(arguments)
                .map_err(|error| error.to_string()),
            "mechanical_animation_clip_v2_get" => runtime
                .mechanical_animation_clip_v2_get(arguments)
                .map_err(|error| error.to_string()),
            "mechanical_animation_clip_v2_preview" => runtime
                .mechanical_animation_clip_v2_preview_get(arguments)
                .map_err(|error| error.to_string()),
            "mechanical_animation_glb_v2_prepare" => runtime
                .mechanical_animation_glb_v2_prepare(arguments)
                .map_err(|error| error.to_string()),
            "mechanical_animation_glb_v2_get" => runtime
                .mechanical_animation_glb_v2_get(arguments)
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_v2_prepare" => runtime
                .game_weapon_animated_glb_socket_v2_prepare(arguments)
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_v2_get" => runtime
                .game_weapon_animated_glb_socket_v2_get(arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_prepare" => runtime
                .fictional_energy_vfx_animated_socket_attachment_prepare(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_get" => runtime
                .fictional_energy_vfx_animated_socket_attachment_get(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_v2_prepare" => runtime
                .fictional_energy_vfx_animated_socket_attachment_v2_prepare(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_v2_get" => runtime
                .fictional_energy_vfx_animated_socket_attachment_v2_get(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_v3_prepare" => runtime
                .fictional_energy_vfx_animated_socket_attachment_v3_prepare(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_attachment_v3_get" => runtime
                .fictional_energy_vfx_animated_socket_attachment_v3_get(&arguments.clone())
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_transform_projection_prepare" => runtime
                .game_weapon_animated_glb_socket_transform_projection_prepare(&arguments.clone())
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_transform_projection_get" => runtime
                .game_weapon_animated_glb_socket_transform_projection_get(&arguments.clone())
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_transform_projection_v2_prepare" => runtime
                .game_weapon_animated_glb_socket_transform_projection_v2_prepare(&arguments.clone())
                .map_err(|error| error.to_string()),
            "game_weapon_animated_glb_socket_transform_projection_v2_get" => runtime
                .game_weapon_animated_glb_socket_transform_projection_v2_get(&arguments.clone())
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_particles_sequence_prepare" => runtime
                .fictional_energy_vfx_animated_socket_particles_sequence_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_particles_sequence_get" => runtime
                .fictional_energy_vfx_animated_socket_particles_sequence_get(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare" => runtime
                .fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_particles_sequence_v2_get" => runtime
                .fictional_energy_vfx_animated_socket_particles_sequence_v2_get(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_sequence_prepare" => runtime
                .fictional_energy_vfx_animated_socket_trails_sequence_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_sequence_get" => runtime
                .fictional_energy_vfx_animated_socket_trails_sequence_get(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare" => runtime
                .fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_sequence_v2_get" => runtime
                .fictional_energy_vfx_animated_socket_trails_sequence_v2_get(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare" => runtime
                .fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get" => runtime
                .fictional_energy_vfx_animated_socket_trails_bloom_sequence_get(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare" => runtime
                .fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare(&arguments)
                .map_err(|error| error.to_string()),
            "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get" => runtime
                .fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get(&arguments)
                .map_err(|error| error.to_string()),
            _ => unreachable!("agentic write tool dispatch arm is exhaustive"),
        },
        "capabilities_get" => {
            serde_json::to_value(runtime.capabilities()).map_err(|error| error.to_string())
        }
        "operator_catalog_get" => Ok(runtime.active_operator_catalog()),
        "material_pack_get" => runtime
            .material_pack_get(arguments)
            .map_err(|error| error.to_string()),
        "agentic_scene_observe" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .agentic_scene_observe(
                    project_id,
                    arguments.get("candidate_id").and_then(Value::as_str),
                )
                .map_err(|error| error.to_string())
        }
        "agentic_stage_plan" => {
            let project_id = required_id(arguments, "project_id")?;
            let observation_sha256 = required_sha256(arguments, "observation_sha256")?;
            runtime
                .agentic_stage_plan_bound(
                    project_id,
                    arguments.get("candidate_id").and_then(Value::as_str),
                    observation_sha256,
                )
                .map_err(|error| error.to_string())
        }
        "agentic_critic_projection" => {
            let project_id = required_id(arguments, "project_id")?;
            let observation_sha256 = required_sha256(arguments, "observation_sha256")?;
            runtime
                .agentic_critic_projection_bound(
                    project_id,
                    arguments.get("candidate_id").and_then(Value::as_str),
                    observation_sha256,
                )
                .map_err(|error| error.to_string())
        }
        "agentic_visual_evidence_bundle" => {
            let project_id = required_id(arguments, "project_id")?;
            let candidate_id = required_id(arguments, "candidate_id")?;
            let observation_sha256 = required_sha256(arguments, "observation_sha256")?;
            runtime
                .agentic_visual_evidence_bundle_bound(project_id, candidate_id, observation_sha256)
                .map_err(|error| error.to_string())
        }
        "visual_surface_get" => runtime
            .visual_surface_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "geometry_program_hash" => runtime
            .geometry_program_hash(arguments)
            .map_err(|error| error.to_string()),
        "silhouette_rig_hash" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_rig_hash(project_id, arguments)
                .map_err(|error| error.to_string())
        }
        "silhouette_fit_intent_hash" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_fit_intent_hash(project_id, arguments)
                .map_err(|error| error.to_string())
        }
        "silhouette_target_get" => {
            let target_sha256 = required_sha256(arguments, "target_sha256")?;
            runtime
                .silhouette_target_get(target_sha256)
                .map_err(|error| error.to_string())
        }
        "boundary_error_get" => {
            let candidate_id = required_id(arguments, "candidate_id")?;
            let target_sha256 = required_sha256(arguments, "target_sha256")?;
            runtime
                .boundary_error(
                    candidate_id,
                    target_sha256,
                    arguments.get("max_segments").and_then(Value::as_u64),
                )
                .map_err(|error| error.to_string())
        }
        "silhouette_part_error_get" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_part_error(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "render_pass_get" => {
            let render_set_hash = arguments
                .get("render_set_hash")
                .and_then(Value::as_str)
                .ok_or_else(|| "render_set_hash is required".to_owned())?;
            let pass = arguments
                .get("pass")
                .and_then(Value::as_str)
                .ok_or_else(|| "pass is required".to_owned())?;
            runtime
                .render_pass_get(render_set_hash, pass)
                .map_err(|error| error.to_string())
        }
        "render_evidence_integrity_get" => runtime
            .render_evidence_integrity_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "render_evidence_replay_get" => runtime
            .render_evidence_replay_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "boolean_operand_lineage_preview" => runtime
            .boolean_operand_lineage_preview(arguments.clone())
            .map_err(|error| error.to_string()),
        "subdivision_topology_lineage_preview" => runtime
            .subdivision_topology_lineage_preview(arguments.clone())
            .map_err(|error| error.to_string()),
        "subdivision_artifact_lineage_get" => runtime
            .subdivision_artifact_lineage_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "subdivision_artifact_lineage_sidecar_get" => runtime
            .subdivision_artifact_lineage_sidecar_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "subdivision_artifact_lineage_prepare" => runtime
            .subdivision_artifact_lineage_prepare(arguments.clone())
            .map_err(|error| error.to_string()),
        "mechanical_animation_clip_prepare" => runtime
            .mechanical_animation_clip_prepare(arguments)
            .map_err(|error| error.to_string()),
        "mechanical_animation_clip_get" => runtime
            .mechanical_animation_clip_get(arguments)
            .map_err(|error| error.to_string()),
        "mechanical_animation_clip_preview_get" => runtime
            .mechanical_animation_clip_preview_get(arguments)
            .map_err(|error| error.to_string()),
        "mechanical_animation_glb_prepare" => runtime
            .mechanical_animation_glb_prepare(arguments)
            .map_err(|error| error.to_string()),
        "game_asset_delivery_prepare" => runtime
            .game_asset_delivery_prepare(arguments)
            .map_err(|error| error.to_string()),
        "game_asset_delivery_get" => runtime
            .game_asset_delivery_get(arguments)
            .map_err(|error| error.to_string()),
        "game_asset_lod_derive" => runtime
            .game_asset_lod_derive(arguments)
            .map_err(|error| error.to_string()),
        "appearance_source_lineage_prepare" => runtime
            .appearance_source_lineage_prepare(arguments)
            .map_err(|error| error.to_string()),
        "appearance_source_lineage_get" => runtime
            .appearance_source_lineage_get(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_anchor_prepare" => runtime
            .game_weapon_anchor_prepare(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_anchor_get" => runtime
            .game_weapon_anchor_get(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_glb_socket_prepare" => runtime
            .game_weapon_glb_socket_prepare(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_glb_socket_get" => runtime
            .game_weapon_glb_socket_get(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_animated_glb_socket_prepare" => runtime
            .game_weapon_animated_glb_socket_prepare(arguments)
            .map_err(|error| error.to_string()),
        "game_weapon_animated_glb_socket_get" => runtime
            .game_weapon_animated_glb_socket_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_prepare" => runtime
            .fictional_energy_vfx_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_get" => runtime
            .fictional_energy_vfx_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_frame_sample" => runtime
            .fictional_energy_vfx_frame_sample(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_appearance_frame_sample" => runtime
            .fictional_energy_vfx_appearance_frame_sample(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_rendered_frame_prepare" => runtime
            .fictional_energy_vfx_rendered_frame_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_rendered_frame_get" => runtime
            .fictional_energy_vfx_rendered_frame_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_rendered_sequence_prepare" => runtime
            .fictional_energy_vfx_rendered_sequence_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_rendered_sequence_get" => runtime
            .fictional_energy_vfx_rendered_sequence_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_hdr_bloom_prepare" => runtime
            .fictional_energy_vfx_hdr_bloom_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_hdr_bloom_get" => runtime
            .fictional_energy_vfx_hdr_bloom_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_particles_prepare" => runtime
            .fictional_energy_vfx_particles_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_particles_get" => runtime
            .fictional_energy_vfx_particles_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_trails_prepare" => runtime
            .fictional_energy_vfx_trails_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_trails_get" => runtime
            .fictional_energy_vfx_trails_get(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_trails_bloom_prepare" => runtime
            .fictional_energy_vfx_trails_bloom_prepare(arguments)
            .map_err(|error| error.to_string()),
        "fictional_energy_vfx_trails_bloom_get" => runtime
            .fictional_energy_vfx_trails_bloom_get(arguments)
            .map_err(|error| error.to_string()),
        "project_list" => {
            serde_json::to_value(runtime.projects().map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "project_get" => {
            let id = required_id(arguments, "project_id")?;
            serde_json::to_value(runtime.project(id).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "reference_import" => {
            let request: forgecad_runtime::ReferenceImportRequest =
                serde_json::from_value(arguments.clone()).map_err(|error| error.to_string())?;
            serde_json::to_value(
                runtime
                    .import_reference(&request)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "reference_get" => {
            let id = required_id(arguments, "reference_id")?;
            let reference = runtime
                .reference(id)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "NOT_FOUND: reference not found".to_owned())?;
            serde_json::to_value(forgecad_runtime::ReferenceGetResult {
                schema_version: "ReferenceGetResult@1".to_owned(),
                reference,
            })
            .map_err(|error| error.to_string())
        }
        "geometry_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            let request = arguments
                .get("request")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if let Some(idempotency_value) = arguments.get("idempotency_key") {
                let idempotency_key = idempotency_value
                    .as_str()
                    .ok_or_else(|| "idempotency_key must be a non-null identifier".to_owned())?;
                let base_version_id = match arguments.get("base_version_id") {
                    Some(Value::Null) => None,
                    Some(Value::String(value)) => Some(value.as_str()),
                    Some(_) => {
                        return Err("base_version_id must be an identifier or null".to_owned())
                    }
                    None => {
                        return Err("HEAD_BINDING_REQUIRED: exact geometry prepare requires an explicit base_version_id field".to_owned())
                    }
                };
                runtime
                    .prepare_geometry_candidate_exact(
                        project_id,
                        base_version_id,
                        idempotency_key,
                        request,
                    )
                    .map_err(|error| error.to_string())
            } else {
                let base_version_id = arguments.get("base_version_id").and_then(Value::as_str);
                runtime
                    .prepare_geometry_candidate(project_id, base_version_id, request)
                    .map_err(|error| error.to_string())
            }
        }
        "reference_compare_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .prepare_reference_comparison(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "reference_mask_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .prepare_reference_mask(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "reference_mask_refine_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .refine_reference_mask(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "camera_fit_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .prepare_camera_fit(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "silhouette_fit_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            let arguments = canonicalize_silhouette_fit_wire(arguments)?;
            runtime
                .silhouette_fit_prepare(project_id, arguments)
                .map_err(|error| error.to_string())
        }
        "part_contour_fit_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .part_contour_fit_prepare(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "silhouette_candidate_compare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_candidate_compare(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "silhouette_evaluation_objective_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_evaluation_objective_prepare(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "silhouette_objective_compare" => {
            let project_id = required_id(arguments, "project_id")?;
            runtime
                .silhouette_objective_compare(project_id, arguments.clone())
                .map_err(|error| error.to_string())
        }
        "visual_review_submit" => runtime
            .submit_visual_review(arguments.clone())
            .map_err(|error| error.to_string()),
        "human_visual_review_submit" => runtime
            .submit_human_visual_review(arguments.clone())
            .map_err(|error| error.to_string()),
        "appearance_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            let base_version_id = arguments.get("base_version_id").and_then(Value::as_str);
            let request = arguments
                .get("request")
                .cloned()
                .unwrap_or_else(|| json!({}));
            runtime
                .prepare_appearance_candidate(project_id, base_version_id, request)
                .map_err(|error| error.to_string())
        }
        "change_prepare" => {
            let project_id = required_id(arguments, "project_id")?;
            let base_version_id = arguments.get("base_version_id").and_then(Value::as_str);
            let request = arguments
                .get("request")
                .cloned()
                .unwrap_or_else(|| json!({}));
            runtime
                .prepare_change_candidate(project_id, base_version_id, request)
                .map_err(|error| error.to_string())
        }
        "artifact_readback_get" => {
            let artifact_id = required_id(arguments, "artifact_id")?;
            let candidate_id = required_id(arguments, "candidate_id")?;
            runtime
                .artifact_readback(artifact_id, candidate_id)
                .map_err(|error| error.to_string())
        }
        "topology_snapshot_get" => {
            let project_id = required_id(arguments, "project_id")?;
            let artifact_id = required_sha256(arguments, "artifact_id")?;
            let candidate_id = required_id(arguments, "candidate_id")?;
            let part_id = required_id(arguments, "part_id")?;
            let artifact_readback_sha256 = required_sha256(arguments, "artifact_readback_sha256")?;
            let program_sha256 = required_sha256(arguments, "program_sha256")?;
            let operator_catalog_sha256 = required_sha256(arguments, "operator_catalog_sha256")?;
            let readback_config_sha256 = required_sha256(arguments, "readback_config_sha256")?;
            let snapshot_policy_sha256 = required_sha256(arguments, "snapshot_policy_sha256")?;
            let max_face_count = arguments
                .get("max_face_count")
                .and_then(Value::as_u64)
                .ok_or_else(|| "max_face_count is required".to_owned())?;
            runtime
                .topology_snapshot(
                    project_id,
                    artifact_id,
                    candidate_id,
                    part_id,
                    artifact_readback_sha256,
                    program_sha256,
                    operator_catalog_sha256,
                    readback_config_sha256,
                    snapshot_policy_sha256,
                    max_face_count,
                )
                .map_err(|error| error.to_string())
        }
        "authoring_topology_get" => runtime
            .authoring_topology(arguments)
            .map_err(|error| preserve_authoring_topology_error(&error.to_string())),
        "authoring_mesh_get" => runtime
            .authoring_mesh(arguments)
            .map_err(|error| error.to_string()),
        "authoring_mesh_durable_get" => runtime
            .authoring_mesh_durable_get(arguments)
            .map_err(|error| error.to_string()),
        "authoring_mesh_durable_prepare" => runtime
            .authoring_mesh_durable_prepare(arguments)
            .map_err(|error| error.to_string()),
        "authoring_mesh_v2_durable_get" => runtime
            .authoring_mesh_v2_durable_get(arguments)
            .map_err(|error| error.to_string()),
        "authoring_mesh_v2_durable_prepare" => runtime
            .authoring_mesh_v2_durable_prepare(arguments)
            .map_err(|error| error.to_string()),
        "authoring_mesh_transaction_get" => runtime
            .authoring_mesh_transaction_get(arguments)
            .map_err(|error| error.to_string()),
        "authoring_mesh_transaction_prepare" => runtime
            .authoring_mesh_transaction_prepare(arguments)
            .map_err(|error| error.to_string()),
        "production_weapon_authoring_mesh_v2_source_prepare" => runtime
            .production_weapon_authoring_mesh_v2_source_prepare(arguments)
            .map_err(|error| error.to_string()),
        "native_high_durable_get" => runtime
            .native_high_durable_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "native_high_durable_prepare" => runtime
            .native_high_durable_prepare(arguments.clone())
            .map_err(|error| error.to_string()),
        "low_quad_draft_durable_get" => runtime
            .low_quad_draft_durable_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "low_quad_draft_durable_prepare" => runtime
            .low_quad_draft_durable_prepare(arguments.clone())
            .map_err(|error| error.to_string()),
        // The Hero UV durable core is source-only in this cohort. Keep the
        // exact Runtime method names on the transport, but fail closed until
        // the Store/Runtime adapter is implemented in its own task.
        "hero_uv_durable_get" | "hero_uv_durable_prepare" => {
            Err(hero_uv_durable_tools::unavailable_error(name))
        }
        "production_camera_lock_registration_lineage_prepare" => runtime
            .production_camera_lock_registration_lineage_prepare(arguments.clone())
            .map_err(|error| error.to_string()),
        "production_camera_lock_registration_lineage_get" => runtime
            .production_camera_lock_registration_lineage_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "production_camera_lock_registration_lineage_preflight_get" => runtime
            .production_camera_lock_registration_lineage_preflight_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "production_camera_lock_registration_lineage_preflight_projection_get" => runtime
            .production_camera_lock_registration_lineage_preflight_projection_get(arguments.clone())
            .map_err(|error| error.to_string()),
        "authoring_mesh_identity_lineage_get" => runtime
            .authoring_mesh_identity_lineage_get(arguments)
            .map_err(|error| preserve_identity_lineage_error(&error.to_string())),
        "authoring_mesh_identity_lineage_prepare" => runtime
            .authoring_mesh_identity_lineage_prepare(arguments)
            .map_err(|error| preserve_identity_lineage_error(&error.to_string())),
        "authoring_mesh_edit_preview" => runtime
            .authoring_mesh_edit_preview(arguments)
            .map_err(|error| preserve_authoring_topology_error(&error.to_string())),
        // Keep the Runtime result byte-for-byte as structuredContent. This
        // edit surface exposes only the Runtime's existing source-element
        // topology proof; durable IdentityLineage is a separate tool surface.
        "authoring_mesh_edit_prepare" => runtime
            .authoring_mesh_edit_prepare(arguments)
            .map_err(|error| preserve_authoring_topology_error(&error.to_string())),
        "mechanical_pose_evaluate" => runtime
            .mechanical_pose_evaluate(arguments)
            .map_err(|error| error.to_string()),
        "mechanical_pose_geometry_preview" => runtime
            .mechanical_pose_geometry_preview(arguments)
            .map_err(|error| error.to_string()),
        "quality_get" => {
            let candidate_id = required_id(arguments, "candidate_id")?;
            let reference_id = arguments.get("reference_id").and_then(Value::as_str);
            runtime
                .quality(candidate_id, reference_id)
                .map_err(|error| error.to_string())
        }
        "version_diff" => {
            let version_id = required_id(arguments, "version_id")?;
            let compare_to_version_id = required_id(arguments, "compare_to_version_id")?;
            runtime
                .version_diff(version_id, compare_to_version_id)
                .map_err(|error| error.to_string())
        }
        "skill_list" => serde_json::to_value(json!({
            "schema_version":"SkillListResult@1",
            "skills":runtime.skills().map_err(|error| error.to_string())?
        }))
        .map_err(|error| error.to_string()),
        "skill_get" => {
            let skill_id = required_id(arguments, "skill_id")?;
            let version = required_id(arguments, "version")?;
            runtime
                .skill_result(skill_id, version)
                .map_err(|error| error.to_owned())
        }
        "snapshot_get" => {
            let id = required_id(arguments, "snapshot_id")?;
            serde_json::to_value(runtime.snapshot(id).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "selection_get" => {
            serde_json::to_value(runtime.selection()).map_err(|error| error.to_string())
        }
        "candidate_get" => {
            let id = required_id(arguments, "candidate_id")?;
            serde_json::to_value(runtime.candidate(id).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "job_get" => {
            let id = required_id(arguments, "job_id")?;
            serde_json::to_value(runtime.job(id).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())
        }
        "job_result_get" => {
            let id = required_id(arguments, "job_id")?;
            runtime.job_result(id).map_err(|error| error.to_string())
        }
        "job_events_read" => {
            let id = required_id(arguments, "job_id")?;
            let after_sequence = arguments
                .get("after_sequence")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            serde_json::to_value(
                runtime
                    .job_events(id, after_sequence)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "version_list" => {
            let project_id = arguments.get("project_id").and_then(Value::as_str);
            serde_json::to_value(
                runtime
                    .versions(project_id)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        "resources_list" => serde_json::to_value(
            runtime
                .resource_descriptors()
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string()),
        "resource_read" => {
            let uri = arguments
                .get("uri")
                .and_then(Value::as_str)
                .ok_or_else(|| "uri is required".to_owned())?;
            serde_json::to_value(
                runtime
                    .read_resource(uri)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())
        }
        _ => Err(format!(
            "CAPABILITY_UNAVAILABLE: unsupported Runtime read method {name}"
        )),
    }
}
