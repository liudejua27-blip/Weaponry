//! Stateless MCP result/error adaptation for the default and compatibility
//! binaries.
//!
//! This module intentionally owns no `Runtime`, `Store`, SQLite connection,
//! CAS handle, session state, filesystem access, or dispatch table.  It only
//! transforms already-produced JSON values at the MCP wire boundary.  The
//! `structuredContent` field is always the exact value supplied by Runtime;
//! summary text is an additional bounded presentation and never a second
//! source of truth.
//!
//! The two binaries currently have subtly different historical contracts:
//!
//! * `DefaultKnife` applies a 1 MiB response budget to every tools/call
//!   operation (8 MiB for the two Hero UV operations), retries both
//!   `RUNTIME_UNAVAILABLE` and `RUNTIME_BUSY`, and uses the short next-action
//!   message.
//! * `Compatibility` applies its budget only to the checked-in bounded
//!   operation allowlist, keeps the same 1 MiB/8 MiB limits, retries only
//!   `RUNTIME_UNAVAILABLE`, and preserves the historical longer next-action
//!   message.  The allowlist below is intentionally explicit so a newly added
//!   legacy operation cannot accidentally acquire a wire-budget contract.
//!
//! The next physical migration can declare `mod result_adapter;` in both
//! binaries and replace only their local helpers first.  Default `main.rs`
//! can pass the two façade summary providers to [`summary_text_with`].  The
//! compatibility binary must pass its existing family-summary chain and keep
//! the `render_pass_get` image branch separate until its no-budget historical
//! behavior has an explicit replay decision.  The legacy dispatch blocks are
//! not moved by this module: `compat_main.rs`'s profile dispatch and backend
//! adapters still depend on `Backend`, `Session`, binding state, Runtime
//! canonicalization, and the feature-gated registry.  They are the next
//! bounded extraction after this adapter is wired and replay-tested.

use serde_json::{json, Value};

/// The normal MCP tools/call response budget.
pub const MCP_RESPONSE_MAX_BYTES: usize = 1024 * 1024;

/// Hero UV responses retain the historical larger read-model budget.
pub const HERO_UV_MCP_RESPONSE_MAX_BYTES: usize = 8 * 1024 * 1024;

/// A stateless summary callback supplied by the binary that owns the
/// operation-specific summary contract.
pub type SummaryProvider = fn(&str, &Value) -> Option<String>;

/// Selects the wire and RuntimeError compatibility contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterProfile {
    /// The public Knife-only MCP binary.
    DefaultKnife,
    /// The explicit `forgecad-mcp-compat` replay binary.
    Compatibility,
}

impl AdapterProfile {
    /// Return the response limit for an operation, or `None` when the
    /// compatibility binary intentionally leaves that historical operation
    /// unbounded at this adapter layer.
    pub fn response_limit(self, operation: &str) -> Option<usize> {
        match self {
            Self::DefaultKnife => Some(response_limit_for_operation(operation)),
            Self::Compatibility => {
                if is_compatibility_bounded_operation(operation) {
                    Some(response_limit_for_operation(operation))
                } else {
                    None
                }
            }
        }
    }

    /// Whether this profile treats `RUNTIME_BUSY` as retryable.  This
    /// difference is preserved until compatibility replay has an explicit
    /// versioned contract migration.
    pub const fn retryable_on_busy(self) -> bool {
        matches!(self, Self::DefaultKnife)
    }

    /// The historical next action for a non-retryable RuntimeError.
    pub const fn non_retryable_next_action(self) -> &'static str {
        match self {
            Self::DefaultKnife => "Read capabilities_get and correct the request.",
            Self::Compatibility => {
                "Read capabilities_get and correct the request or wait for the required MCP task."
            }
        }
    }
}

fn response_limit_for_operation(operation: &str) -> usize {
    if is_hero_uv_operation(operation) {
        HERO_UV_MCP_RESPONSE_MAX_BYTES
    } else {
        MCP_RESPONSE_MAX_BYTES
    }
}

fn is_hero_uv_operation(operation: &str) -> bool {
    matches!(operation, "hero_uv_durable_get" | "hero_uv_durable_prepare")
}

/// Historical compatibility operation names that were already subject to the
/// read-model MCP wire budget.  This is a public inventory for the future
/// compatibility handler migration and for focused drift tests; it is not a
/// new manifest or a Runtime capability registry.
pub const COMPATIBILITY_BOUNDED_OPERATION_NAMES: &[&str] = &[
    "geometry_prepare",
    "topology_snapshot_get",
    "authoring_topology_get",
    "authoring_mesh_get",
    "authoring_mesh_durable_get",
    "authoring_mesh_durable_prepare",
    "authoring_mesh_v2_durable_get",
    "authoring_mesh_v2_durable_prepare",
    "authoring_mesh_transaction_get",
    "authoring_mesh_transaction_prepare",
    "production_weapon_authoring_mesh_v2_source_prepare",
    "production_weapon_form_art_composite_proposal_get",
    "production_weapon_form_art_composite_proposal_prepare",
    "production_weapon_form_art_composite_evidence_get",
    "production_weapon_form_art_composite_evidence_prepare",
    "production_weapon_form_art_repair_plan_get",
    "production_weapon_form_art_failure_diagnostic_get",
    "production_weapon_form_art_visibility_calibration_get",
    "production_weapon_form_art_target_occlusion_attribution_get",
    "production_weapon_form_art_aperture_repair_plan_get",
    "production_weapon_form_art_mesh_proposal_get",
    "production_weapon_form_art_mesh_proposal_prepare",
    "production_weapon_owner_reviewed_void_calibration_get",
    "native_high_durable_get",
    "native_high_durable_prepare",
    "low_quad_draft_durable_get",
    "low_quad_draft_durable_prepare",
    "hero_uv_durable_get",
    "hero_uv_durable_prepare",
    "authoring_mesh_identity_lineage_get",
    "authoring_mesh_identity_lineage_prepare",
    "authoring_mesh_edit_preview",
    "authoring_mesh_edit_prepare",
    "mechanical_pose_evaluate",
    "mechanical_pose_geometry_preview",
    "mechanical_animation_clip_prepare",
    "mechanical_animation_clip_get",
    "mechanical_animation_clip_preview_get",
    "mechanical_animation_clip_v2_prepare",
    "mechanical_animation_clip_v2_get",
    "mechanical_animation_clip_v2_preview",
    "mechanical_animation_glb_v2_prepare",
    "mechanical_animation_glb_v2_get",
    "mechanical_animation_glb_prepare",
    "game_asset_delivery_prepare",
    "game_asset_delivery_get",
    "game_asset_lod_derive",
    "appearance_source_lineage_prepare",
    "appearance_source_lineage_get",
    "candidate_material_surface_quality_prepare",
    "candidate_material_surface_quality_get",
    "candidate_animation_vfx_quality_prepare",
    "candidate_animation_vfx_quality_get",
    "candidate_animation_vfx_quality_v2_prepare",
    "candidate_animation_vfx_quality_v2_get",
    "production_stage_transition_v2_prepare",
    "production_stage_transition_v2_get",
    "production_stage_transition_v3_prepare",
    "production_stage_transition_v3_get",
    "production_camera_lock_prepare",
    "production_camera_lock_get",
    "production_camera_lock_registration_lineage_prepare",
    "production_camera_lock_registration_lineage_get",
    "production_camera_lock_registration_lineage_preflight_get",
    "production_camera_lock_registration_lineage_preflight_projection_get",
    "production_weapon_form_art_baseline_preflight_get",
    "production_weapon_form_art_baseline_prepare",
    "production_weapon_form_art_baseline_get",
    "production_weapon_form_evidence_prepare",
    "production_weapon_form_evidence_get",
    "production_weapon_form_art_evidence_prepare",
    "production_weapon_form_art_evidence_get",
    "production_weapon_art_decision_proposal_get",
    "production_weapon_assembly_parameter_sink_get",
    "production_weapon_form_quality_v2_prepare",
    "production_weapon_form_quality_v2_get",
    "production_weapon_form_quality_v2_preflight_get",
    "production_weapon_formal_high_get",
    "production_weapon_formal_high_prepare",
    "production_weapon_high_low_bake_get",
    "production_weapon_high_low_bake_prepare",
    "production_weapon_high_low_bake_preflight_get",
    "game_weapon_anchor_prepare",
    "game_weapon_anchor_get",
    "game_weapon_glb_socket_prepare",
    "game_weapon_glb_socket_get",
    "game_weapon_animated_glb_socket_prepare",
    "game_weapon_animated_glb_socket_get",
    "game_weapon_animated_glb_socket_v2_prepare",
    "game_weapon_animated_glb_socket_v2_get",
    "fictional_energy_vfx_animated_socket_attachment_prepare",
    "fictional_energy_vfx_animated_socket_attachment_get",
    "fictional_energy_vfx_animated_socket_attachment_v2_prepare",
    "fictional_energy_vfx_animated_socket_attachment_v2_get",
    "fictional_energy_vfx_animated_socket_attachment_v3_prepare",
    "fictional_energy_vfx_animated_socket_attachment_v3_get",
    "game_weapon_animated_glb_socket_transform_projection_prepare",
    "game_weapon_animated_glb_socket_transform_projection_get",
    "game_weapon_animated_glb_socket_transform_projection_v2_prepare",
    "game_weapon_animated_glb_socket_transform_projection_v2_get",
    "fictional_energy_vfx_animated_socket_particles_sequence_prepare",
    "fictional_energy_vfx_animated_socket_particles_sequence_get",
    "fictional_energy_vfx_animated_socket_particles_sequence_v2_prepare",
    "fictional_energy_vfx_animated_socket_particles_sequence_v2_get",
    "fictional_energy_vfx_animated_socket_trails_sequence_prepare",
    "fictional_energy_vfx_animated_socket_trails_sequence_get",
    "fictional_energy_vfx_animated_socket_trails_sequence_v2_prepare",
    "fictional_energy_vfx_animated_socket_trails_sequence_v2_get",
    "fictional_energy_vfx_animated_socket_trails_bloom_sequence_prepare",
    "fictional_energy_vfx_animated_socket_trails_bloom_sequence_get",
    "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_prepare",
    "fictional_energy_vfx_animated_socket_trails_bloom_sequence_v2_get",
    "fictional_energy_vfx_prepare",
    "fictional_energy_vfx_get",
    "fictional_energy_vfx_frame_sample",
    "fictional_energy_vfx_appearance_frame_sample",
    "fictional_energy_vfx_rendered_frame_prepare",
    "fictional_energy_vfx_rendered_frame_get",
    "fictional_energy_vfx_rendered_sequence_prepare",
    "fictional_energy_vfx_rendered_sequence_get",
    "fictional_energy_vfx_hdr_bloom_prepare",
    "fictional_energy_vfx_hdr_bloom_get",
    "fictional_energy_vfx_particles_prepare",
    "fictional_energy_vfx_particles_get",
    "fictional_energy_vfx_trails_prepare",
    "fictional_energy_vfx_trails_get",
    "fictional_energy_vfx_trails_bloom_prepare",
    "fictional_energy_vfx_trails_bloom_get",
    "render_evidence_integrity_get",
    "render_evidence_replay_get",
    "boolean_operand_lineage_preview",
    "subdivision_topology_lineage_preview",
    "subdivision_artifact_lineage_get",
    "subdivision_artifact_lineage_sidecar_get",
    "subdivision_artifact_lineage_prepare",
    "geometry_program_hash",
    "knife_curve_evaluated_mesh_get",
    "knife_curve_evaluated_mesh_prepare",
    "knife_curve_modifier_graph_get",
    "knife_curve_modifier_graph_prepare",
];

/// Return whether a compatibility operation had an existing bounded wire
/// contract.
pub fn is_compatibility_bounded_operation(operation: &str) -> bool {
    COMPATIBILITY_BOUNDED_OPERATION_NAMES.contains(&operation)
}

/// Convert an operation result into an MCP `tools/call` success response.
/// `structuredContent` is moved unchanged; providers only produce the text
/// item and cannot alter the Runtime result.
pub fn tool_success(
    id: Value,
    operation: &str,
    value: Value,
    summary_providers: &[SummaryProvider],
) -> Option<Value> {
    tool_success_for(
        Some(id),
        operation,
        value,
        summary_providers,
        AdapterProfile::DefaultKnife,
    )
}

/// Profile-aware success response used by both default and compatibility
/// wiring.  `None` preserves JSON-RPC notification behavior.
pub fn tool_success_for(
    id: Option<Value>,
    operation: &str,
    value: Value,
    summary_providers: &[SummaryProvider],
    profile: AdapterProfile,
) -> Option<Value> {
    let id = id?;
    let summary = summary_text_with(operation, &value, summary_providers);
    Some(apply_response_budget_for(
        operation,
        json!({
            "jsonrpc":"2.0",
            "id":id,
            "result":{
                "content":[{"type":"text","text":summary}],
                "structuredContent":value
            }
        }),
        profile,
    ))
}

/// Render a compatibility `render_pass_get` result with its historical image
/// content shape.  This function deliberately does not apply the generic
/// read-model budget because the current compatibility branch did not apply
/// one to this special image path.  A future contract change must make that
/// decision explicit and add replay evidence before changing it.
pub fn render_pass_result_for(id: Value, value: Value, profile: AdapterProfile) -> Value {
    let mut metadata = value;
    let png_base64 = metadata
        .as_object_mut()
        .and_then(|object| object.remove("png_base64"))
        .and_then(|value| value.as_str().map(str::to_owned));
    let Some(png_base64) = png_base64 else {
        let error = runtime_error_value_for(profile, "RENDER_PASS_INVALID: PNG payload is missing");
        return json!({
            "jsonrpc":"2.0",
            "id":id,
            "result":{
                "isError":true,
                "content":[{"type":"text","text":"Runtime returned an invalid render pass"}],
                "structuredContent":error
            }
        });
    };
    json!({
        "jsonrpc":"2.0",
        "id":id,
        "result":{
            "content":[
                {"type":"image","data":png_base64,"mimeType":"image/png"},
                {"type":"text","text":serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_owned())}
            ],
            "structuredContent":metadata
        }
    })
}

/// Build a default-profile RuntimeError@1 value.
pub fn runtime_error_value(error: &str) -> Value {
    runtime_error_value_for(AdapterProfile::DefaultKnife, error)
}

/// Build a profile-aware RuntimeError@1 value without exposing raw paths,
/// prompts, or multiline Runtime internals.
pub fn runtime_error_value_for(profile: AdapterProfile, error: &str) -> Value {
    let (code, message) = error
        .split_once(':')
        .unwrap_or(("RUNTIME_REQUEST_FAILED", error));
    let code = code.trim();
    let retryable =
        code == "RUNTIME_UNAVAILABLE" || (code == "RUNTIME_BUSY" && profile.retryable_on_busy());
    json!({
        "schema_version":"RuntimeError@1",
        "code":code,
        "message":safe_error(message.trim()),
        "retryable":retryable,
        "next_action":if retryable {"Call runtime_status/doctor and retry after Runtime reaches Ready."} else {profile.non_retryable_next_action()},
        "evidence_ids":[]
    })
}

/// Build the standard JSON-RPC error envelope.  A missing request id remains
/// a notification and therefore produces no response.
pub fn error_response(
    id: Option<Value>,
    code: i64,
    message: &str,
    data: Option<Value>,
) -> Option<Value> {
    let id = id?;
    let mut error = json!({"code":code,"message":message});
    if let Some(data) = data {
        error["data"] = data;
    }
    Some(json!({"jsonrpc":"2.0","id":id,"error":error}))
}

/// Build the MCP result-level error used by `tools/call` failures.
pub fn tool_error(id: Value, error: &str) -> Option<Value> {
    tool_error_for(Some(id), error, AdapterProfile::DefaultKnife)
}

/// Profile-aware MCP result-level error.  This is intentionally a result
/// error, not a JSON-RPC transport error, matching both current binaries.
pub fn tool_error_for(id: Option<Value>, error: &str, profile: AdapterProfile) -> Option<Value> {
    let id = id?;
    let value = runtime_error_value_for(profile, error);
    Some(json!({
        "jsonrpc":"2.0",
        "id":id,
        "result":{
            "isError":true,
            "content":[{"type":"text","text":serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_owned())}],
            "structuredContent":value
        }
    }))
}

/// Apply the default Knife response budget.
pub fn apply_mcp_response_budget(operation: &str, response: Value) -> Value {
    apply_response_budget_for(operation, response, AdapterProfile::DefaultKnife)
}

/// Apply the compatibility binary's historical bounded-operation budget.
pub fn apply_read_model_mcp_wire_budget(operation: &str, response: Value) -> Value {
    apply_response_budget_for(operation, response, AdapterProfile::Compatibility)
}

/// Apply a profile-aware response budget.  Oversized responses become a small
/// deterministic RuntimeError result and never reach the transport.
pub fn apply_response_budget_for(
    operation: &str,
    response: Value,
    profile: AdapterProfile,
) -> Value {
    let Some(max_bytes) = profile.response_limit(operation) else {
        return response;
    };
    let within_budget = serde_json::to_vec(&response)
        .map(|bytes| bytes.len() <= max_bytes)
        .unwrap_or(false);
    if within_budget {
        return response;
    }
    let message = match profile {
        AdapterProfile::DefaultKnife => {
            "MCP_READ_MODEL_RESPONSE_BUDGET_EXCEEDED: serialized tools/call response exceeds the operation wire budget"
        }
        AdapterProfile::Compatibility if is_hero_uv_operation(operation) => {
            "MCP_READ_MODEL_RESPONSE_BUDGET_EXCEEDED: serialized Hero UV tools/call response exceeds 8 MiB"
        }
        AdapterProfile::Compatibility => {
            "MCP_READ_MODEL_RESPONSE_BUDGET_EXCEEDED: serialized tools/call response exceeds 1 MiB"
        }
    };
    let error = runtime_error_value_for(profile, message);
    json!({
        "jsonrpc":"2.0",
        "id":response.get("id").cloned().unwrap_or(Value::Null),
        "result":{
            "isError":true,
            "content":[{"type":"text","text":serde_json::to_string(&error).unwrap_or_else(|_| "{}".to_owned())}],
            "structuredContent":error
        }
    })
}

/// Try operation-specific summary providers in order, then fall back to the
/// exact structured value.  Providers must only produce presentation text;
/// no provider receives mutable access to the Runtime result.
pub fn summary_text_with(
    operation: &str,
    value: &Value,
    summary_providers: &[SummaryProvider],
) -> String {
    for provider in summary_providers {
        if let Some(summary) = provider(operation, value) {
            return summary;
        }
    }
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_owned())
}

/// Generic summary fallback when a binary has no operation-specific provider.
pub fn summary_text(operation: &str, value: &Value) -> String {
    summary_text_with(operation, value, &[])
}

/// Error families that the compatibility dispatch currently normalizes before
/// building RuntimeError@1.  Keeping these pure string-boundary transforms
/// here lets the later handler extraction preserve the typed code while
/// dropping request details and local paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorFamily {
    IdentityLineage,
    AuthoringTopology,
    ProductionWeaponHighLowBake,
}

/// Preserve the typed family code while replacing the potentially sensitive
/// Runtime detail with the historical bounded message.
pub fn normalize_typed_error(error: &str, family: ErrorFamily) -> String {
    match family {
        ErrorFamily::IdentityLineage => normalize_error_family(
            error,
            "AUTHORING_MESH_IDENTITY_LINEAGE_",
            "AUTHORING_MESH_IDENTITY_LINEAGE_",
            "Runtime identity lineage request rejected",
        ),
        ErrorFamily::AuthoringTopology => {
            let Some(start) = error
                .find("AUTHORING_TOPOLOGY_")
                .or_else(|| error.find("AUTHORING_MESH_EDIT_"))
            else {
                return error.to_owned();
            };
            let code = error[start..]
                .split(':')
                .next()
                .map(str::trim)
                .filter(|value| {
                    value.starts_with("AUTHORING_TOPOLOGY_")
                        || value.starts_with("AUTHORING_MESH_EDIT_")
                })
                .unwrap_or("AUTHORING_TOPOLOGY_INVALID");
            format!("{code}: Runtime topology edit request rejected")
        }
        ErrorFamily::ProductionWeaponHighLowBake => normalize_error_family(
            error,
            "PRODUCTION_WEAPON_HIGH_LOW_BAKE_",
            "PRODUCTION_WEAPON_HIGH_LOW_BAKE_RUNTIME_REJECTED",
            "Runtime formal High/Low/Cage bake request rejected",
        ),
    }
}

fn normalize_error_family(error: &str, prefix: &str, fallback_code: &str, message: &str) -> String {
    let Some(start) = error.find(prefix) else {
        return error.to_owned();
    };
    let code = error[start..]
        .split(':')
        .next()
        .map(str::trim)
        .filter(|value| value.starts_with(prefix))
        .unwrap_or(fallback_code);
    format!("{code}: {message}")
}

/// Remove line breaks and cap an error detail before it reaches MCP output.
pub fn safe_error(error: &str) -> String {
    error
        .chars()
        .filter(|character| *character != '\n' && *character != '\r')
        .take(512)
        .collect()
}
