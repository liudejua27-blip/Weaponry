//! Rust-owned validation and lowering for `ArmDesignIntent@1`.
//!
//! The intent is deliberately a visual vocabulary, not a geometry program.
//! C110B/C110G lower only reviewed robotic-arm recipe families.  A value
//! outside those reviewed families is returned as an explicit unsupported
//! result; it is never silently replaced with a default arm.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{semantic_sha256, CoreError, CoreResult};

pub const ARM_DESIGN_INTENT_SCHEMA_VERSION: &str = "ArmDesignIntent@1";
pub const ARM_RECIPE_LOWERING_SCHEMA_VERSION: &str = "ArmRecipeLowering@1";

const C106_CHILD_RECIPES: [&str; 6] = [
    "recipe_c106_arm_turntable",
    "recipe_c106_arm_joint_housing",
    "recipe_c106_arm_link_armor",
    "recipe_c106_arm_cable_harness",
    "recipe_c106_arm_gripper",
    "recipe_c106_arm_surface_trim",
];

const ARCHITECTURES: [&str; 6] = [
    "serial_chain",
    "parallel_link",
    "scara",
    "gantry",
    "delta",
    "cantilever",
];
const JOINT_LANGUAGES: [&str; 5] = [
    "armored_bearing",
    "exposed_ring",
    "gimbal_shell",
    "capsule_joint",
    "bellows_joint",
];
const LINK_LANGUAGES: [&str; 5] = [
    "closed_shell",
    "twin_rail",
    "open_truss",
    "tapered_loft",
    "tube_frame",
];
const BASE_LANGUAGES: [&str; 5] = [
    "round_turntable",
    "hex_platform",
    "floating_pedestal",
    "industrial_deck",
    "compact_puck",
];
const WRIST_LANGUAGES: [&str; 4] = [
    "layered_wrist",
    "gimbal_wrist",
    "cylindrical_wrist",
    "fork_wrist",
];
const END_EFFECTORS: [&str; 5] = [
    "parallel_gripper",
    "adaptive_claw",
    "precision_tool",
    "sensor_probe",
    "soft_pad_gripper",
];
const CABLE_LANGUAGES: [&str; 4] = [
    "internal_routing",
    "braided_external",
    "armored_harness",
    "minimal_cable",
];
const SURFACE_LANGUAGES: [&str; 6] = [
    "panel_seams",
    "flowline",
    "chevron_relief",
    "hex_microgrid",
    "engraved_ribs",
    "fastener_bands",
];
const MATERIAL_PALETTES: [&str; 5] = [
    "graphite_blue",
    "white_aluminum",
    "industrial_yellow",
    "warm_copper",
    "monochrome_technical",
];
const DETAIL_DENSITIES: [&str; 3] = ["light", "medium", "dense"];
const POSES: [&str; 5] = ["neutral", "grounded", "elevated", "extended", "folded"];
const PROPORTIONS: [&str; 5] = ["compact", "balanced", "long_reach", "heavy_base", "slender"];
const SOURCES: [&str; 3] = ["user_brief", "reference_evidence", "agent_inferred"];

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArmDesignIntent {
    pub schema_version: String,
    pub domain_pack_id: String,
    pub architecture: String,
    pub joint_language: String,
    pub link_language: String,
    pub base_language: String,
    pub wrist_language: String,
    pub end_effector_language: String,
    pub cable_language: String,
    pub surface_language: Vec<String>,
    pub material_palette: String,
    pub detail_density: String,
    pub pose: String,
    pub proportion_profile: String,
    pub style_keywords: Vec<String>,
    pub source: String,
    pub visual_only: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArmRecipeLowering {
    pub schema_version: String,
    pub status: String,
    pub root_recipe_id: Option<String>,
    pub child_recipe_ids: Vec<String>,
    pub surface_tokens: Vec<String>,
    pub unsupported_codes: Vec<String>,
    pub intent_sha256: String,
}

fn invalid(message: impl Into<String>) -> CoreError {
    CoreError::invalid_data("ARM_DESIGN_INTENT_INVALID", message)
}

fn require_allowed(field: &str, value: &str, allowed: &[&str]) -> CoreResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(invalid(format!("{field} contains an unreviewed value")))
    }
}

fn validate_intent(intent: &ArmDesignIntent) -> CoreResult<()> {
    if intent.schema_version != ARM_DESIGN_INTENT_SCHEMA_VERSION {
        return Err(invalid("schema_version must be ArmDesignIntent@1"));
    }
    if intent.domain_pack_id != "pack_robotic_arm_concept" {
        return Err(invalid("domain_pack_id must be pack_robotic_arm_concept"));
    }
    require_allowed("architecture", &intent.architecture, &ARCHITECTURES)?;
    require_allowed("joint_language", &intent.joint_language, &JOINT_LANGUAGES)?;
    require_allowed("link_language", &intent.link_language, &LINK_LANGUAGES)?;
    require_allowed("base_language", &intent.base_language, &BASE_LANGUAGES)?;
    require_allowed("wrist_language", &intent.wrist_language, &WRIST_LANGUAGES)?;
    require_allowed(
        "end_effector_language",
        &intent.end_effector_language,
        &END_EFFECTORS,
    )?;
    require_allowed("cable_language", &intent.cable_language, &CABLE_LANGUAGES)?;
    require_allowed(
        "material_palette",
        &intent.material_palette,
        &MATERIAL_PALETTES,
    )?;
    require_allowed("detail_density", &intent.detail_density, &DETAIL_DENSITIES)?;
    require_allowed("pose", &intent.pose, &POSES)?;
    require_allowed(
        "proportion_profile",
        &intent.proportion_profile,
        &PROPORTIONS,
    )?;
    require_allowed("source", &intent.source, &SOURCES)?;
    if !intent.visual_only {
        return Err(invalid("visual_only must be true"));
    }
    if intent.surface_language.is_empty() || intent.surface_language.len() > 6 {
        return Err(invalid("surface_language must contain 1 to 6 tokens"));
    }
    let mut surface_tokens = BTreeSet::new();
    for token in &intent.surface_language {
        require_allowed("surface_language", token, &SURFACE_LANGUAGES)?;
        if !surface_tokens.insert(token) {
            return Err(invalid("surface_language tokens must be unique"));
        }
    }
    if intent.style_keywords.len() > 12
        || intent.style_keywords.iter().any(|keyword| {
            let trimmed = keyword.trim();
            trimmed.is_empty() || trimmed.chars().count() > 80
        })
    {
        return Err(invalid(
            "style_keywords must contain at most 12 bounded strings",
        ));
    }
    Ok(())
}

/// Validate a JSON intent and lower it to the reviewed C106 recipe family.
pub fn lower_arm_design_intent(value: &Value) -> CoreResult<ArmRecipeLowering> {
    let intent: ArmDesignIntent = serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("ArmDesignIntent@1 failed closed: {error}")))?;
    validate_intent(&intent)?;
    let intent_sha256 = semantic_sha256(&intent)?;

    // C106 contains the serial-chain family. C110G adds a bounded parallel-link
    // layout family with its own reviewed Recipe/Connector lineage. Other
    // topologies remain explicit unsupported results until their own reviewed
    // fixtures exist.
    if !matches!(
        intent.architecture.as_str(),
        "serial_chain" | "parallel_link"
    ) {
        return Ok(ArmRecipeLowering {
            schema_version: ARM_RECIPE_LOWERING_SCHEMA_VERSION.into(),
            status: "unsupported".into(),
            root_recipe_id: None,
            child_recipe_ids: Vec::new(),
            surface_tokens: intent.surface_language,
            unsupported_codes: vec!["ARM_INTENT_ARCHITECTURE_UNSUPPORTED".into()],
            intent_sha256,
        });
    }

    let root_recipe_id = if intent.architecture == "parallel_link" {
        "recipe_c110g_parallel_link_root"
    } else {
        match intent.proportion_profile.as_str() {
            "compact" => "recipe_c106_arm_desktop_assistant",
            "long_reach" | "slender" => "recipe_c106_arm_service_display",
            "balanced" | "heavy_base" => "recipe_c106_arm_gallery_industrial",
            _ => unreachable!("validated proportion profile"),
        }
    };
    let child_recipe_ids = if intent.architecture == "parallel_link" {
        vec![
            "recipe_c110g_parallel_rail".into(),
            "recipe_c110g_parallel_carriage".into(),
            "recipe_c110g_parallel_link".into(),
            "recipe_c110g_parallel_end_effector".into(),
        ]
    } else {
        C106_CHILD_RECIPES.iter().map(|id| (*id).into()).collect()
    };
    Ok(ArmRecipeLowering {
        schema_version: ARM_RECIPE_LOWERING_SCHEMA_VERSION.into(),
        status: "lowered".into(),
        root_recipe_id: Some(root_recipe_id.into()),
        child_recipe_ids,
        surface_tokens: intent.surface_language,
        unsupported_codes: Vec::new(),
        intent_sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn intent(architecture: &str) -> Value {
        json!({
            "schema_version": "ArmDesignIntent@1",
            "domain_pack_id": "pack_robotic_arm_concept",
            "architecture": architecture,
            "joint_language": "armored_bearing",
            "link_language": "closed_shell",
            "base_language": "round_turntable",
            "wrist_language": "layered_wrist",
            "end_effector_language": "parallel_gripper",
            "cable_language": "armored_harness",
            "surface_language": ["panel_seams", "flowline"],
            "material_palette": "graphite_blue",
            "detail_density": "dense",
            "pose": "grounded",
            "proportion_profile": "long_reach",
            "style_keywords": ["precision", "industrial"],
            "source": "user_brief",
            "visual_only": true
        })
    }

    #[test]
    fn lowers_serial_chain_to_reviewed_service_recipe() {
        let lowered = lower_arm_design_intent(&intent("serial_chain")).unwrap();
        assert_eq!(lowered.status, "lowered");
        assert_eq!(
            lowered.root_recipe_id.as_deref(),
            Some("recipe_c106_arm_service_display")
        );
        assert_eq!(lowered.child_recipe_ids.len(), 6);
        assert_eq!(lowered.intent_sha256.len(), 64);
    }

    #[test]
    fn lowers_parallel_link_to_reviewed_family() {
        let lowered = lower_arm_design_intent(&intent("parallel_link")).unwrap();
        assert_eq!(lowered.status, "lowered");
        assert_eq!(
            lowered.root_recipe_id.as_deref(),
            Some("recipe_c110g_parallel_link_root")
        );
        assert_eq!(lowered.child_recipe_ids.len(), 4);
        assert!(lowered.unsupported_codes.is_empty());
    }

    #[test]
    fn surfaces_unreviewed_architecture_without_fallback() {
        let lowered = lower_arm_design_intent(&intent("scara")).unwrap();
        assert_eq!(lowered.status, "unsupported");
        assert!(lowered.root_recipe_id.is_none());
        assert_eq!(
            lowered.unsupported_codes,
            vec!["ARM_INTENT_ARCHITECTURE_UNSUPPORTED"]
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let mut value = intent("serial_chain");
        value
            .as_object_mut()
            .unwrap()
            .insert("geometry_code".into(), json!("shell"));
        let error = lower_arm_design_intent(&value).unwrap_err();
        assert_eq!(error.code(), "ARM_DESIGN_INTENT_INVALID");
    }
}
