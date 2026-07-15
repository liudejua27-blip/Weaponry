from __future__ import annotations

from .agent_models import AgentMaterialPreset


_ALL_DOMAINS = [
    "future_weapon_prop",
    "vehicle_concept",
    "aircraft_concept",
    "robotic_arm_concept",
]


def list_material_presets() -> list[AgentMaterialPreset]:
    """Return the small, visual-only P0 material catalog.

    These presets describe appearance only. They intentionally do not contain
    density, strength, temperature, supplier, or manufacturing claims.
    """
    return [
        AgentMaterialPreset(
            material_id="mat_graphite",
            display_name="石墨深灰",
            category="metal",
            pbr={"base_color": "#26313b", "metallic": 0.78, "roughness": 0.34, "opacity": 1},
            allowed_domains=_ALL_DOMAINS,
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_aluminum",
            display_name="拉丝铝",
            category="metal",
            pbr={"base_color": "#8a9aa8", "metallic": 0.88, "roughness": 0.28, "opacity": 1},
            allowed_domains=_ALL_DOMAINS,
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_automotive_paint",
            display_name="亮面汽车漆",
            category="coating",
            pbr={"base_color": "#3d78b8", "metallic": 0.38, "roughness": 0.2, "opacity": 1},
            allowed_domains=["vehicle_concept", "future_weapon_prop"],
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_rubber",
            display_name="橡胶外观",
            category="rubber",
            pbr={"base_color": "#15191d", "metallic": 0.02, "roughness": 0.78, "opacity": 1},
            allowed_domains=["vehicle_concept", "robotic_arm_concept", "future_weapon_prop"],
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_composite",
            display_name="哑光复合材料",
            category="composite",
            pbr={"base_color": "#344451", "metallic": 0.22, "roughness": 0.58, "opacity": 1},
            allowed_domains=["aircraft_concept", "robotic_arm_concept", "future_weapon_prop"],
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_dark_glass",
            display_name="深色玻璃",
            category="glass",
            pbr={"base_color": "#172a3d", "metallic": 0.12, "roughness": 0.12, "opacity": 0.58},
            allowed_domains=["aircraft_concept", "vehicle_concept", "future_weapon_prop"],
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_signal_red",
            display_name="信号红涂层",
            category="coating",
            pbr={"base_color": "#c4493d", "metallic": 0.42, "roughness": 0.3, "opacity": 1},
            allowed_domains=_ALL_DOMAINS,
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_painted_steel",
            display_name="喷涂钢板",
            category="metal",
            pbr={"base_color": "#4d5b68", "metallic": 0.72, "roughness": 0.48, "opacity": 1, "clearcoat": 0.08},
            allowed_domains=_ALL_DOMAINS,
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_abs_matte",
            display_name="哑光工程塑料",
            category="polymer",
            pbr={"base_color": "#39434d", "metallic": 0.02, "roughness": 0.7, "opacity": 1},
            allowed_domains=_ALL_DOMAINS,
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_rubber_tire",
            display_name="轮胎橡胶",
            category="rubber",
            pbr={"base_color": "#101214", "metallic": 0.0, "roughness": 0.9, "opacity": 1},
            allowed_domains=["vehicle_concept", "robotic_arm_concept"],
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_carbon_composite",
            display_name="碳纤复合外观",
            category="composite",
            pbr={"base_color": "#1c252d", "metallic": 0.16, "roughness": 0.42, "opacity": 1},
            allowed_domains=["vehicle_concept", "aircraft_concept", "robotic_arm_concept", "future_weapon_prop"],
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_clear_glass",
            display_name="透明玻璃",
            category="glass",
            pbr={"base_color": "#c8e5f2", "metallic": 0.0, "roughness": 0.08, "opacity": 0.32, "transmission": 0.72, "ior": 1.5},
            allowed_domains=["vehicle_concept", "aircraft_concept", "future_weapon_prop"],
            provenance="forgecad_builtin",
        ),
        AgentMaterialPreset(
            material_id="mat_powder_coat",
            display_name="粉末涂层",
            category="coating",
            pbr={"base_color": "#6d7480", "metallic": 0.28, "roughness": 0.62, "opacity": 1},
            allowed_domains=_ALL_DOMAINS,
            provenance="forgecad_builtin",
        ),
    ]


def material_preset_map() -> dict[str, AgentMaterialPreset]:
    return {preset.material_id: preset for preset in list_material_presets()}
