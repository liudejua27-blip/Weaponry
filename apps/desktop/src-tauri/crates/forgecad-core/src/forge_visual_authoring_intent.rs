//! Compact Provider authoring input for the visual-program pipeline.
//!
//! A model chooses the visual language through `ArmDesignIntent@1`. Rust then
//! derives every executable operation lineage, Part, Material Zone, Surface
//! Program and Detail binding from the reviewed C111 compiler substrate. The
//! resulting `ForgeVisualProgram@1` remains the only candidate design truth.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    apply_arm_geometry_family, reviewed_c111_draft_visual_program, semantic_sha256,
    ArmDesignIntent, CoreError, CoreResult, ForgeVisualDesignToken, ForgeVisualProgram,
    ForgeVisualProgramStage, VisualDetailBindingKind,
};

pub const FORGE_VISUAL_AUTHORING_INTENT_SCHEMA_VERSION: &str = "ForgeVisualAuthoringIntent@1";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeVisualAuthoringIntent {
    pub schema_version: String,
    pub authoring_id: String,
    pub title: String,
    pub arm_design_intent: ArmDesignIntent,
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("FORGE_VISUAL_AUTHORING_INTENT_INVALID", message)
}

fn require_id(field: &str, value: &str) -> CoreResult<()> {
    let valid = !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(invalid(format!(
            "{field} must be a bounded stable identifier"
        )))
    }
}

fn require_safe_text(field: &str, value: &str, max_chars: usize) -> CoreResult<()> {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    let forbidden = [
        "http://",
        "https://",
        "file://",
        "/users/",
        "bearer ",
        "sk-",
        "```",
        "<script",
        "function(",
        "subprocess",
        "std::process",
    ];
    if trimmed.is_empty()
        || trimmed.chars().count() > max_chars
        || forbidden.iter().any(|token| lower.contains(token))
    {
        return Err(invalid(format!(
            "{field} must be bounded visual text without code, URL, path, or secret material"
        )));
    }
    Ok(())
}

fn material_for_palette(current: &str, palette: &str, zone_id: &str) -> String {
    match palette {
        "white_aluminum" if current == "mat_graphite" => "mat_aluminum".into(),
        // The C111 reviewer fixture keeps the three gripper armor outputs on
        // automotive paint as an exact structural-detail contract.  Every
        // other paint zone is presentation material, so the white-aluminum
        // palette must replace it with the reviewed high-metallic aluminum
        // slot instead of silently retaining the fixed blue paint texture.
        "white_aluminum"
            if current == "mat_automotive_paint" && zone_id != "zone_arm_gripper_paint" =>
        {
            "mat_aluminum".into()
        }
        "monochrome_technical" if current == "mat_emissive_blue" => "mat_graphite".into(),
        "industrial_yellow" if current == "mat_graphite" => "mat_aluminum".into(),
        "warm_copper" if current == "mat_graphite" => "mat_aluminum".into(),
        _ => current.to_owned(),
    }
}

/// Lower compact Provider intent into the existing complete visual-program
/// truth. This function does not persist the intent or create an alternate
/// version chain; callers still author a normal `ForgeVisualProgramRevision`.
pub fn lower_forge_visual_authoring_intent(value: &Value) -> CoreResult<ForgeVisualProgram> {
    let intent: ForgeVisualAuthoringIntent =
        serde_json::from_value(value.clone()).map_err(|error| {
            invalid(format!(
                "ForgeVisualAuthoringIntent@1 failed closed: {error}"
            ))
        })?;
    if intent.schema_version != FORGE_VISUAL_AUTHORING_INTENT_SCHEMA_VERSION {
        return Err(invalid(
            "schema_version must be ForgeVisualAuthoringIntent@1",
        ));
    }
    require_id("authoring_id", &intent.authoring_id)?;
    require_safe_text("title", &intent.title, 160)?;
    for keyword in &intent.arm_design_intent.style_keywords {
        require_safe_text("arm_design_intent.style_keywords", keyword, 80)?;
    }

    // This validates every visual enum and explicitly rejects unimplemented
    // architectures before a dense program is materialized.
    let arm_value = serde_json::to_value(&intent.arm_design_intent)
        .map_err(|error| invalid(format!("arm intent serialization failed: {error}")))?;
    let lowering = crate::lower_arm_design_intent(&arm_value)?;
    if lowering.status != "lowered" {
        return Err(invalid(format!(
            "arm architecture is not available: {}",
            lowering.unsupported_codes.join(",")
        )));
    }

    let intent_sha256 = semantic_sha256(&intent)?;
    let mut program = reviewed_c111_draft_visual_program()?;
    program.program_id = format!("visualprog_provider_ir_{}", &intent_sha256[..24]);
    program.title = intent.title;
    program.stage = ForgeVisualProgramStage::Draft;

    apply_arm_geometry_family(
        &arm_value,
        &mut program.geometry_graph,
        &mut program.assembly_graph,
    )?;

    // `lower_forge_visual_program` treats material_graph as authoritative and
    // writes it back into ShapeProgram operations. Keep it synchronized with
    // the palette pass above or the Provider's palette would be silently lost.
    for binding in &mut program.material_graph {
        binding.material_id = material_for_palette(
            &binding.material_id,
            &intent.arm_design_intent.material_palette,
            &binding.material_zone_id,
        );
    }

    let surface_language = intent.arm_design_intent.surface_language.join("_");
    let mut renamed_surfaces = BTreeMap::new();
    for (index, binding) in program.surface_graph.iter_mut().enumerate() {
        let previous = binding.surface_program_id.clone();
        let next = format!("surface_ir_{:02}_{}", index + 1, surface_language);
        binding.surface_program_id = next.clone();
        renamed_surfaces.insert((binding.part_id.clone(), previous), next);
    }
    for detail in &mut program.detail_inventory {
        for binding in &mut detail.bindings {
            if binding.kind == VisualDetailBindingKind::SurfaceProgram {
                if let Some(next) =
                    renamed_surfaces.get(&(binding.part_id.clone(), binding.target_id.clone()))
                {
                    binding.target_id = next.clone();
                }
            }
        }
    }

    program.design_tokens = vec![
        ForgeVisualDesignToken {
            token_id: "provider_authoring_ir_sha256".into(),
            value: intent_sha256,
        },
        ForgeVisualDesignToken {
            token_id: "proportion_profile".into(),
            value: intent.arm_design_intent.proportion_profile,
        },
        ForgeVisualDesignToken {
            token_id: "surface_language".into(),
            value: surface_language,
        },
        ForgeVisualDesignToken {
            token_id: "detail_density".into(),
            value: intent.arm_design_intent.detail_density,
        },
    ];
    program.validate()?;
    Ok(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lower_forge_visual_program, ForgeVisualProgramRevision};
    use serde_json::json;

    fn intent(palette: &str, proportion: &str) -> Value {
        json!({
            "schema_version":"ForgeVisualAuthoringIntent@1",
            "authoring_id":"authoring_arm_deepsea",
            "title":"深海维修机械臂",
            "arm_design_intent":{
                "schema_version":"ArmDesignIntent@1",
                "domain_pack_id":"pack_robotic_arm_concept",
                "architecture":"serial_chain",
                "joint_language":"exposed_ring",
                "link_language":"open_truss",
                "base_language":"industrial_deck",
                "wrist_language":"fork_wrist",
                "end_effector_language":"adaptive_claw",
                "cable_language":"braided_external",
                "surface_language":["panel_seams","flowline","fastener_bands"],
                "material_palette":palette,
                "detail_density":"dense",
                "pose":"extended",
                "proportion_profile":proportion,
                "style_keywords":["deep sea","industrial collectible"],
                "source":"agent_inferred",
                "visual_only":true
            }
        })
    }

    #[test]
    fn pv008_compact_intent_lowers_to_complete_visual_program() {
        let program =
            lower_forge_visual_authoring_intent(&intent("graphite_blue", "long_reach")).unwrap();
        assert!(program.program_id.starts_with("visualprog_provider_ir_"));
        assert_eq!(program.parts.len(), 10);
        let reviewed_output_count = reviewed_c111_draft_visual_program().unwrap().geometry_graph
            ["outputs"]
            .as_array()
            .unwrap()
            .len();
        assert!(reviewed_output_count >= 96);
        assert_eq!(
            program.geometry_graph["outputs"].as_array().unwrap().len(),
            reviewed_output_count,
            "Provider authoring must preserve the complete current reviewed substrate"
        );
        assert_eq!(program.detail_inventory.len(), 27);
        assert!(program
            .surface_graph
            .iter()
            .all(|binding| binding.surface_program_id.contains("panel_seams_flowline")));
        let value = serde_json::to_value(&program).unwrap();
        let revision = ForgeVisualProgramRevision::author(&value).unwrap();
        lower_forge_visual_program(&serde_json::to_value(revision.program).unwrap()).unwrap();
    }

    #[test]
    fn pv008_intent_changes_real_geometry_and_material_lineage() {
        let graphite =
            lower_forge_visual_authoring_intent(&intent("graphite_blue", "compact")).unwrap();
        let aluminum =
            lower_forge_visual_authoring_intent(&intent("white_aluminum", "long_reach")).unwrap();
        assert_ne!(
            semantic_sha256(&graphite.geometry_graph).unwrap(),
            semantic_sha256(&aluminum.geometry_graph).unwrap()
        );
        assert_ne!(graphite.material_graph, aluminum.material_graph);
    }

    #[test]
    fn pv008_white_aluminum_replaces_presentation_paint_but_preserves_c111_gripper_contract() {
        let program =
            lower_forge_visual_authoring_intent(&intent("white_aluminum", "balanced")).unwrap();
        assert!(program.material_graph.iter().all(|binding| {
            binding.material_zone_id == "zone_arm_gripper_paint"
                || binding.material_id != "mat_automotive_paint"
        }));
        assert!(program.material_graph.iter().any(|binding| {
            binding.material_zone_id == "zone_arm_gripper_paint"
                && binding.material_id == "mat_automotive_paint"
        }));
        assert!(program.material_graph.iter().any(|binding| {
            binding.material_zone_id == "zone_arm_link_armor"
                && binding.material_id == "mat_aluminum"
        }));
        lower_forge_visual_program(&serde_json::to_value(program).unwrap()).unwrap();
    }

    #[test]
    fn pv008_rejects_code_or_url_in_creative_text() {
        let mut malicious = intent("graphite_blue", "balanced");
        malicious["arm_design_intent"]["style_keywords"] = json!(["https://bad.invalid/model"]);
        let error = lower_forge_visual_authoring_intent(&malicious).unwrap_err();
        assert_eq!(error.code(), "FORGE_VISUAL_AUTHORING_INTENT_INVALID");
    }
}
