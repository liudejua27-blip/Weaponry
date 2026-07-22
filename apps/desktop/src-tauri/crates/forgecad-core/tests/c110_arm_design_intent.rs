use forgecad_core::{lower_arm_design_intent, ArmRecipeLowering};
use serde_json::json;

fn intent(architecture: &str) -> serde_json::Value {
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
fn c110b_lowers_a_valid_intent_to_reviewed_c106_recipe_graph() {
    let lowered: ArmRecipeLowering = lower_arm_design_intent(&intent("serial_chain")).unwrap();
    assert_eq!(lowered.schema_version, "ArmRecipeLowering@1");
    assert_eq!(lowered.status, "lowered");
    assert_eq!(
        lowered.root_recipe_id.as_deref(),
        Some("recipe_c106_arm_service_display")
    );
    assert_eq!(lowered.child_recipe_ids.len(), 6);
    assert_eq!(lowered.surface_tokens, ["panel_seams", "flowline"]);
    assert_eq!(lowered.intent_sha256.len(), 64);
}

#[test]
fn c110g_lowers_parallel_link_to_its_independent_recipe_graph() {
    let lowered = lower_arm_design_intent(&intent("parallel_link")).unwrap();
    assert_eq!(lowered.status, "lowered");
    assert_eq!(
        lowered.root_recipe_id.as_deref(),
        Some("recipe_c110g_parallel_link_root")
    );
    assert_eq!(lowered.child_recipe_ids.len(), 4);
    assert!(lowered
        .child_recipe_ids
        .iter()
        .all(|recipe_id| recipe_id.starts_with("recipe_c110g_")));
}

#[test]
fn c110b_does_not_fallback_for_unreviewed_architecture() {
    let lowered = lower_arm_design_intent(&intent("scara")).unwrap();
    assert_eq!(lowered.status, "unsupported");
    assert!(lowered.root_recipe_id.is_none());
    assert_eq!(
        lowered.unsupported_codes,
        ["ARM_INTENT_ARCHITECTURE_UNSUPPORTED"]
    );
}

#[test]
fn c110b_rejects_arbitrary_geometry_payloads() {
    let mut value = intent("serial_chain");
    value
        .as_object_mut()
        .unwrap()
        .insert("geometry_code".into(), json!("freeform_python"));
    let error = lower_arm_design_intent(&value).unwrap_err();
    assert_eq!(error.code(), "ARM_DESIGN_INTENT_INVALID");
}
