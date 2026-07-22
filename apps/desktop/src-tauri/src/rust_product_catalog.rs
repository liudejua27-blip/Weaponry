//! Code-owned, visual-only product catalogs used after the K003 cutover.
//!
//! These values intentionally describe appearance and concept-domain routing
//! only. They do not contain manufacturing materials, dimensions, performance
//! claims, Provider configuration, paths, URLs, or executable content.

use serde_json::{json, Value};

pub fn domain_packs() -> Value {
    json!([
        {
            "schema_version": "DomainPackManifest@1",
            "pack_id": "pack_future_weapon_prop",
            "domain": "future_weapon_prop",
            "display_name": "未来武器概念道具",
            "description": "虚构游戏资产、影视道具与非功能性未来道具的完整外观概念。",
            "non_functional_only": true,
            "templates": ["compact_prop", "long_profile_prop", "heavy_support_prop", "energy_visual_prop"],
            "connector_types": ["surface_mount", "rail_mount", "panel_mount", "tool_mount"],
            "joint_types": ["fixed", "hinge", "slider"],
            "material_preset_ids": ["mat_graphite", "mat_signal_red", "mat_dark_glass"],
            "quality_profile_id": "quality_concept_default",
            "export_profile_id": "export_concept_default"
        },
        {
            "schema_version": "DomainPackManifest@1",
            "pack_id": "pack_vehicle_concept",
            "domain": "vehicle_concept",
            "display_name": "汽车与地面载具",
            "description": "未来汽车、探索车、竞速车与科幻运输工具的完整外观概念。",
            "non_functional_only": true,
            "templates": ["urban_scout", "exploration_vehicle", "low_racer", "heavy_transport"],
            "connector_types": ["surface_mount", "wheel_mount", "panel_mount", "payload_mount"],
            "joint_types": ["fixed", "hinge", "continuous"],
            "material_preset_ids": ["mat_graphite", "mat_automotive_paint", "mat_rubber"],
            "quality_profile_id": "quality_concept_default",
            "export_profile_id": "export_concept_default"
        },
        {
            "schema_version": "DomainPackManifest@1",
            "pack_id": "pack_aircraft_concept",
            "domain": "aircraft_concept",
            "display_name": "飞机与航空器",
            "description": "未来飞机、垂直起降器与无人航空器的完整外观概念。",
            "non_functional_only": true,
            "templates": ["fast_single_seat", "wide_body_transport", "vertical_takeoff", "uncrewed_scout"],
            "connector_types": ["surface_mount", "wing_mount", "nacelle_mount", "payload_mount"],
            "joint_types": ["fixed", "hinge", "slider"],
            "material_preset_ids": ["mat_graphite", "mat_composite", "mat_dark_glass"],
            "quality_profile_id": "quality_concept_default",
            "export_profile_id": "export_concept_default"
        },
        {
            "schema_version": "DomainPackManifest@1",
            "pack_id": "pack_robotic_arm_concept",
            "domain": "robotic_arm_concept",
            "display_name": "机械臂与机器人机构",
            "description": "机械臂、服务机器人上肢与科幻操作机构的概念资产。",
            "non_functional_only": true,
            "templates": ["precision_light", "heavy_handler", "long_reach_maintenance", "dual_tool_service"],
            "connector_types": ["surface_mount", "axial_mount", "tool_mount", "panel_mount"],
            "joint_types": ["fixed", "hinge", "slider", "ball", "continuous"],
            "material_preset_ids": ["mat_graphite", "mat_aluminum", "mat_rubber"],
            "quality_profile_id": "quality_concept_default",
            "export_profile_id": "export_concept_default"
        }
    ])
}

pub fn material_presets() -> Value {
    let all_domains = json!([
        "future_weapon_prop",
        "vehicle_concept",
        "aircraft_concept",
        "robotic_arm_concept"
    ]);
    let preset = |material_id: &str,
                  display_name: &str,
                  category: &str,
                  pbr: Value,
                  allowed_domains: Value| {
        json!({
            "schema_version": "MaterialPreset@1",
            "material_id": material_id,
            "display_name": display_name,
            "category": category,
            "pbr": pbr,
            "visual_only": true,
            "allowed_domains": allowed_domains,
            "provenance": "forgecad_builtin",
            "visual_tags": [category],
            "source": "forgecad_builtin",
            "license": "not_applicable",
            "version": "1",
            "thumbnail_fallback": "parameter",
            "texture_summary": []
        })
    };
    Value::Array(vec![
        preset(
            "mat_graphite",
            "石墨深灰",
            "metal",
            json!({"base_color":"#26313b","metallic":0.78,"roughness":0.34,"opacity":1}),
            all_domains.clone(),
        ),
        preset(
            "mat_aluminum",
            "拉丝铝",
            "metal",
            json!({"base_color":"#8a9aa8","metallic":0.88,"roughness":0.28,"opacity":1}),
            all_domains.clone(),
        ),
        preset(
            "mat_automotive_paint",
            "亮面汽车漆",
            "coating",
            json!({"base_color":"#3d78b8","metallic":0.38,"roughness":0.2,"opacity":1}),
            json!(["vehicle_concept", "future_weapon_prop"]),
        ),
        preset(
            "mat_rubber",
            "橡胶外观",
            "rubber",
            json!({"base_color":"#15191d","metallic":0.02,"roughness":0.78,"opacity":1}),
            json!([
                "vehicle_concept",
                "robotic_arm_concept",
                "future_weapon_prop"
            ]),
        ),
        preset(
            "mat_composite",
            "哑光复合材料",
            "composite",
            json!({"base_color":"#344451","metallic":0.22,"roughness":0.58,"opacity":1}),
            json!([
                "aircraft_concept",
                "robotic_arm_concept",
                "future_weapon_prop"
            ]),
        ),
        preset(
            "mat_dark_glass",
            "深色玻璃",
            "glass",
            json!({"base_color":"#172a3d","metallic":0.12,"roughness":0.12,"opacity":0.58}),
            json!(["aircraft_concept", "vehicle_concept", "future_weapon_prop"]),
        ),
        preset(
            "mat_signal_red",
            "信号红涂层",
            "coating",
            json!({"base_color":"#c4493d","metallic":0.42,"roughness":0.3,"opacity":1}),
            all_domains.clone(),
        ),
        preset(
            "mat_painted_steel",
            "喷涂钢板",
            "metal",
            json!({"base_color":"#4d5b68","metallic":0.72,"roughness":0.48,"opacity":1,"clearcoat":0.08}),
            all_domains.clone(),
        ),
        preset(
            "mat_abs_matte",
            "哑光工程塑料",
            "polymer",
            json!({"base_color":"#39434d","metallic":0.02,"roughness":0.7,"opacity":1}),
            all_domains.clone(),
        ),
        preset(
            "mat_rubber_tire",
            "轮胎橡胶",
            "rubber",
            json!({"base_color":"#101214","metallic":0.0,"roughness":0.9,"opacity":1}),
            json!(["vehicle_concept", "robotic_arm_concept"]),
        ),
        preset(
            "mat_carbon_composite",
            "碳纤复合外观",
            "composite",
            json!({"base_color":"#1c252d","metallic":0.16,"roughness":0.42,"opacity":1}),
            all_domains.clone(),
        ),
        preset(
            "mat_clear_glass",
            "透明玻璃",
            "glass",
            json!({"base_color":"#c8e5f2","metallic":0.0,"roughness":0.08,"opacity":0.32,"transmission":0.72,"ior":1.5}),
            json!(["vehicle_concept", "aircraft_concept", "future_weapon_prop"]),
        ),
        preset(
            "mat_powder_coat",
            "粉末涂层",
            "coating",
            json!({"base_color":"#6d7480","metallic":0.28,"roughness":0.62,"opacity":1}),
            all_domains,
        ),
    ])
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn visual_catalogs_are_complete_unique_and_non_engineering() {
        let packs = domain_packs().as_array().unwrap().clone();
        let materials = material_presets().as_array().unwrap().clone();
        assert_eq!(packs.len(), 4);
        assert_eq!(materials.len(), 13);
        let material_ids = materials
            .iter()
            .map(|item| item["material_id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(material_ids.len(), materials.len());
        for pack in packs {
            assert_eq!(pack["non_functional_only"], true);
            for id in pack["material_preset_ids"].as_array().unwrap() {
                assert!(material_ids.contains(id.as_str().unwrap()));
            }
        }
        let encoded = serde_json::to_string(&materials)
            .unwrap()
            .to_ascii_lowercase();
        for forbidden in [
            "density",
            "tensile",
            "tolerance",
            "supplier",
            "manufacturing",
            "api_key",
            "http://",
            "https://",
        ] {
            assert!(!encoded.contains(forbidden));
        }
    }
}
