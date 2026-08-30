//! Explicit legacy compatibility registry.
//!
//! The current Weaponry surface is the knife profile in `knife_tool_profile`.
//! This module owns the composition of the historical raw MCP manifest so the
//! compatibility surface has one visible registration boundary.  It remains
//! available for replay and migration tests, but callers must opt into the
//! compatibility profile before exposing these names to Codex.

pub(crate) const UNAVAILABLE_ERROR: &str = "WEAPONRY_COMPATIBILITY_PROFILE_UNAVAILABLE: rebuild with --features legacy-compatibility-registry to enable the historical compatibility registry";

pub(crate) fn ensure_enabled() -> Result<(), String> {
    Ok(())
}

use forgecad_runtime::canonical_json_hash;
use serde_json::{json, Value};

/// Build the historical raw manifest used by the explicit compatibility
/// profile and by replay/manifest integrity tests.
pub(crate) fn tools_with_writes(writes_enabled: bool) -> Vec<Value> {
    let mut tools = super::read_only_tools();
    if writes_enabled {
        tools.extend(super::mcp004_write_tools());
        tools.extend(super::mcp005_write_tools());
        tools.extend(super::mcp007_write_tools());
        tools.extend(super::mcp008_write_tools());
        tools.extend(super::mcp009_write_tools());
        tools.extend(super::mcp010c_write_tools());
        tools.extend(super::mcp010f_write_tools());
        tools.extend(super::authoring_mesh_durable_tools::write_tools());
        tools.extend(super::authoring_mesh_transaction_tools::write_tools());
        tools.extend(super::authoring_mesh_v2_durable_tools::write_tools());
        tools.extend(super::production_weapon_form_art_baseline_materializer_tools::write_tools());
        tools.extend(super::production_weapon_form_art_composite_proposal_tools::write_tools());
        tools.extend(super::production_weapon_form_art_composite_evidence_tools::write_tools());
        tools.extend(super::production_weapon_form_art_mesh_proposal_tools::write_tools());
        tools.extend(super::native_high_durable_tools::write_tools());
        tools.extend(super::low_quad_durable_tools::write_tools());
        tools.extend(super::hero_uv_durable_tools::write_tools());
        tools.extend(super::production_camera_lock_registration_lineage_tools::write_tools());
        tools.extend(super::production_weapon_formal_high_tools::write_tools());
        tools.extend(super::production_weapon_high_low_bake_tools::write_tools());
        tools.extend(super::authoring_mesh_identity_lineage_tools::write_tools());
        tools.extend(super::optimization_tools::write_tools());
        tools.extend(super::agentic_orchestrator_tools::write_tools());
        tools.extend(super::agentic_action_tools::write_tools());
        tools.extend(super::cross_view_promotion_tools::write_tools());
        tools.extend(super::agentic_write_tools::write_tools());
        tools.extend(super::weapon_foundation_authoring_materialization_tools::write_tools());
        tools.extend(super::fps_presentation_package_v2_tools::write_tools());
        tools.extend(super::fps_presentation_package_v2_candidate_tools::write_tools());
        tools.extend(super::weapon_foundation_tools::write_tools());
    }
    tools.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));
    tools
}

/// Return the write names declared by the historical registry.  Keeping this
/// list beside the raw manifest composition makes count/hash drift fail in the
/// existing manifest summary checks instead of silently widening the public
/// façade.
pub(crate) fn all_write_tool_names() -> Vec<String> {
    let mut names = super::mcp004_write_tool_names();
    names.extend(super::mcp005_write_tool_names());
    names.extend(super::mcp007_write_tool_names());
    names.extend(super::mcp008_write_tool_names());
    names.extend(super::mcp009_write_tool_names());
    names.extend(super::mcp010c_write_tool_names());
    names.extend(super::mcp010f_write_tool_names());
    names.extend(super::authoring_mesh_durable_write_tool_names());
    names.extend(super::authoring_mesh_transaction_write_tool_names());
    names.extend(super::authoring_mesh_v2_durable_write_tool_names());
    names.extend(super::production_weapon_form_art_baseline_write_tool_names());
    names.extend(super::production_weapon_form_art_composite_proposal_write_tool_names());
    names.extend(super::production_weapon_form_art_composite_evidence_write_tool_names());
    names.extend(super::production_weapon_form_art_mesh_proposal_write_tool_names());
    names.extend(super::native_high_durable_write_tool_names());
    names.extend(super::low_quad_durable_write_tool_names());
    names.extend(super::hero_uv_durable_write_tool_names());
    names.extend(super::production_camera_lock_registration_lineage_write_tool_names());
    names.extend(super::production_weapon_formal_high_write_tool_names());
    names.extend(super::production_weapon_high_low_bake_write_tool_names());
    names.extend(super::authoring_mesh_identity_lineage_write_tool_names());
    names.extend(super::optimization_write_tool_names());
    names.extend(super::agentic_orchestrator_write_tool_names());
    names.extend(super::agentic_action_write_tool_names());
    names.extend(super::cross_view_promotion_write_tool_names());
    names.extend(super::agentic_write_tool_names());
    names.extend(super::weapon_foundation_authoring_materialization_tools::write_tool_names());
    names.extend(super::fps_presentation_package_v2_tools::write_tool_names());
    names.extend(super::fps_presentation_package_v2_candidate_tools::write_tool_names());
    names.extend(super::weapon_foundation_tools::write_tool_names());
    names
}

/// Return whether a raw operation is a compatibility write operation.
pub(crate) fn is_write_tool(name: &str) -> bool {
    is_mcp004_write_tool(name)
        || is_mcp005_write_tool(name)
        || is_mcp007_write_tool(name)
        || is_mcp008_write_tool(name)
        || is_mcp009_write_tool(name)
        || is_mcp010c_write_tool(name)
        || is_mcp010f_write_tool(name)
        || super::authoring_mesh_durable_tools::is_write_tool(name)
        || super::authoring_mesh_transaction_tools::is_write_tool(name)
        || super::authoring_mesh_v2_durable_tools::is_write_tool(name)
        || super::production_weapon_form_art_baseline_materializer_tools::is_write_tool(name)
        || super::production_weapon_form_art_composite_proposal_tools::is_write_tool(name)
        || super::production_weapon_form_art_composite_evidence_tools::is_write_tool(name)
        || super::production_weapon_form_art_mesh_proposal_tools::is_write_tool(name)
        || super::native_high_durable_tools::is_write_tool(name)
        || super::low_quad_durable_tools::is_write_tool(name)
        || super::hero_uv_durable_tools::is_write_tool(name)
        || super::production_camera_lock_registration_lineage_tools::is_write_tool(name)
        || super::production_weapon_formal_high_tools::is_write_tool(name)
        || super::production_weapon_high_low_bake_tools::is_write_tool(name)
        || super::authoring_mesh_identity_lineage_tools::is_write_tool(name)
        || super::optimization_tools::is_write_tool(name)
        || super::agentic_orchestrator_tools::is_write_tool(name)
        || super::agentic_action_tools::is_write_tool(name)
        || super::cross_view_promotion_tools::is_write_tool(name)
        || super::agentic_write_tools::is_write_tool(name)
        || super::weapon_foundation_authoring_materialization_tools::is_write_tool(name)
        || super::fps_presentation_package_v2_tools::is_write_tool(name)
        || super::fps_presentation_package_v2_candidate_tools::is_write_tool(name)
        || super::weapon_foundation_tools::is_write_tool(name)
}

/// Hash the raw compatibility manifest.  The default knife profile hashes its
/// own 11-façade projection and never uses this as its public manifest hash.
pub(crate) fn manifest_hash(writes_enabled: bool) -> String {
    canonical_json_hash(&json!({"tools":tools_with_writes(writes_enabled)}))
}

pub(crate) fn contains_tool(name: &str, writes_enabled: bool) -> bool {
    tools_with_writes(writes_enabled)
        .iter()
        .any(|tool| tool.get("name").and_then(Value::as_str) == Some(name))
}

fn is_mcp004_write_tool(name: &str) -> bool {
    super::mcp004_write_tool_names()
        .iter()
        .any(|tool| tool == name)
}

fn is_mcp005_write_tool(name: &str) -> bool {
    super::mcp005_write_tool_names()
        .iter()
        .any(|tool| tool == name)
}

fn is_mcp007_write_tool(name: &str) -> bool {
    super::mcp007_write_tool_names()
        .iter()
        .any(|tool| tool == name)
}

fn is_mcp008_write_tool(name: &str) -> bool {
    super::mcp008_write_tool_names()
        .iter()
        .any(|tool| tool == name)
}

fn is_mcp009_write_tool(name: &str) -> bool {
    super::mcp009_write_tool_names()
        .iter()
        .any(|tool| tool == name)
}

fn is_mcp010c_write_tool(name: &str) -> bool {
    super::mcp010c_write_tool_names()
        .iter()
        .any(|tool| tool == name)
}

fn is_mcp010f_write_tool(name: &str) -> bool {
    super::mcp010f_write_tool_names()
        .iter()
        .any(|tool| tool == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_registry_keeps_the_raw_replay_surface_explicit() {
        let read_only = tools_with_writes(false);
        let enabled = tools_with_writes(true);
        assert_eq!(read_only.len(), 131);
        assert_eq!(enabled.len(), 226);
        assert_eq!(
            enabled.len(),
            read_only.len() + all_write_tool_names().len()
        );
        assert!(contains_tool("runtime_status", false));
        assert!(contains_tool("project_create", true));
        assert!(!contains_tool("project_create", false));
        assert!(is_write_tool("project_create"));
        assert!(!is_write_tool("runtime_status"));
        assert_eq!(manifest_hash(true), manifest_hash(true));
    }
}
