from __future__ import annotations

from typing import Any, Iterable, Mapping, Sequence

from forgecad_agent.application.agent_models import (
    DomainSemanticProportionRecipe,
    MechanicalStyleToken,
)


_ALL_DOMAINS = [
    "pack_future_weapon_prop",
    "pack_vehicle_concept",
    "pack_aircraft_concept",
    "pack_robotic_arm_concept",
]


STYLE_TOKENS = (
    MechanicalStyleToken(
        token_id="style_compact_rounded",
        display_name="紧凑圆润",
        description="缩短主要视觉跨度，保持轻量、亲和的概念外观。",
        proportion_profile="compact",
        edge_language="soft",
        surface_tension="relaxed",
        detail_density="low",
        symmetry="bilateral",
        material_palette="clean_coating",
        lighting_profile="soft_studio",
        allowed_domains=_ALL_DOMAINS,
    ),
    MechanicalStyleToken(
        token_id="style_aerodynamic_sleek",
        display_name="修长流线",
        description="延展主方向比例，形成更连贯的速度感轮廓。",
        proportion_profile="elongated",
        edge_language="controlled",
        surface_tension="taut",
        detail_density="low",
        symmetry="bilateral",
        material_palette="technical_composite",
        lighting_profile="concept_contrast",
        allowed_domains=_ALL_DOMAINS,
    ),
    MechanicalStyleToken(
        token_id="style_industrial_substantial",
        display_name="厚重稳定",
        description="增加承载视觉量感，仅表达外观，不代表结构或性能。",
        proportion_profile="substantial",
        edge_language="crisp",
        surface_tension="neutral",
        detail_density="medium",
        symmetry="assembly_driven",
        material_palette="mixed_industrial",
        lighting_profile="cad_neutral",
        allowed_domains=_ALL_DOMAINS,
    ),
    MechanicalStyleToken(
        token_id="style_clean_balanced",
        display_name="简洁协调",
        description="收敛次要部件比例，让主次关系更清楚。",
        proportion_profile="balanced",
        edge_language="controlled",
        surface_tension="neutral",
        detail_density="low",
        symmetry="assembly_driven",
        material_palette="dark_metal",
        lighting_profile="cad_neutral",
        allowed_domains=_ALL_DOMAINS,
    ),
)


def _recipe(recipe_id: str, domain: str, token: str, name: str, description: str, phrases: list[str], selector: str, path: str, delta: int) -> DomainSemanticProportionRecipe:
    return DomainSemanticProportionRecipe(
        recipe_id=recipe_id,
        domain_pack_id=domain,
        style_token_id=token,
        display_name=name,
        description=description,
        intent_phrases=phrases,
        adjustments=[{"role_selector": selector, "path": path, "step_delta": delta}],
    )


SEMANTIC_PROPORTION_RECIPES = (
    _recipe("proportion_prop_compact", "pack_future_weapon_prop", "style_compact_rounded", "主体更紧凑", "收短展示道具主体的视觉长度。", ["更紧凑", "短一些", "圆润一点"], "primary_form", "transform.scale.x", -1),
    _recipe("proportion_prop_sleek", "pack_future_weapon_prop", "style_aerodynamic_sleek", "主体更修长", "延展展示道具主体的视觉长度。", ["更修长", "流线一些", "拉长"], "primary_form", "transform.scale.x", 1),
    _recipe("proportion_prop_substantial", "pack_future_weapon_prop", "style_industrial_substantial", "主体更厚重", "增加主体的视觉高度，不表达功能能力。", ["更厚重", "更有量感"], "primary_form", "transform.scale.y", 1),
    _recipe("proportion_prop_clean", "pack_future_weapon_prop", "style_clean_balanced", "辅助体更简洁", "收窄次要外壳，突出主体层级。", ["更简洁", "减少杂乱"], "secondary_form", "transform.scale.z", -1),
    _recipe("proportion_vehicle_compact", "pack_vehicle_concept", "style_compact_rounded", "车身更紧凑", "收短车身主壳体的视觉长度。", ["车身紧凑", "短一些"], "primary_form", "transform.scale.x", -1),
    _recipe("proportion_vehicle_sleek", "pack_vehicle_concept", "style_aerodynamic_sleek", "车身更修长", "延展车身主壳体的视觉长度。", ["更修长", "更流线"], "primary_form", "transform.scale.x", 1),
    _recipe("proportion_vehicle_substantial", "pack_vehicle_concept", "style_industrial_substantial", "车身更厚重", "增加车身主壳体的视觉高度。", ["更厚重", "更稳重"], "primary_form", "transform.scale.y", 1),
    _recipe("proportion_vehicle_clean", "pack_vehicle_concept", "style_clean_balanced", "座舱更简洁", "收窄座舱视觉比例，强化车身主次。", ["更简洁", "座舱收敛"], "cabin_form", "transform.scale.z", -1),
    _recipe("proportion_aircraft_compact", "pack_aircraft_concept", "style_compact_rounded", "机身更紧凑", "收短机身的视觉长度。", ["机身紧凑", "短一些"], "primary_form", "transform.scale.x", -1),
    _recipe("proportion_aircraft_sleek", "pack_aircraft_concept", "style_aerodynamic_sleek", "机身更修长", "延展机身的视觉长度。", ["更修长", "更流线"], "primary_form", "transform.scale.x", 1),
    _recipe("proportion_aircraft_substantial", "pack_aircraft_concept", "style_industrial_substantial", "机身更厚重", "增加机身的视觉高度，不代表适航或结构性能。", ["更厚重", "更有量感"], "primary_form", "transform.scale.y", 1),
    _recipe("proportion_aircraft_clean", "pack_aircraft_concept", "style_clean_balanced", "座舱盖更简洁", "收窄座舱盖视觉比例。", ["更简洁", "座舱盖收敛"], "cabin_form", "transform.scale.z", -1),
    _recipe("proportion_arm_compact", "pack_robotic_arm_concept", "style_compact_rounded", "上臂更紧凑", "收短上臂连杆的视觉跨度。", ["更紧凑", "连杆短一些"], "upper_link_form", "transform.scale.y", -1),
    _recipe("proportion_arm_sleek", "pack_robotic_arm_concept", "style_aerodynamic_sleek", "上臂更修长", "延展上臂连杆的视觉跨度。", ["更修长", "连杆长一些"], "upper_link_form", "transform.scale.y", 1),
    _recipe("proportion_arm_substantial", "pack_robotic_arm_concept", "style_industrial_substantial", "底座更厚重", "增加底座的视觉宽度，不代表负载能力。", ["底座厚重", "更稳重"], "base_form", "transform.scale.x", 1),
    _recipe("proportion_arm_clean", "pack_robotic_arm_concept", "style_clean_balanced", "末端更简洁", "收窄末端部件的视觉比例。", ["更简洁", "末端收敛"], "end_effector_form", "transform.scale.z", -1),
)


def style_token_map() -> dict[str, MechanicalStyleToken]:
    return {item.token_id: item for item in STYLE_TOKENS}


def recipes_for_domain(domain_pack_id: str) -> Iterable[DomainSemanticProportionRecipe]:
    return (item for item in SEMANTIC_PROPORTION_RECIPES if item.domain_pack_id == domain_pack_id)


def part_id_for_role_selector(parts: Sequence[Any], assembly_graph: Mapping[str, Any], selector: str) -> str | None:
    """Map one semantic slot to a real current part without inventing geometry."""
    if not parts:
        return None
    part_ids = [str(item.part_id if hasattr(item, "part_id") else item["part_id"]) for item in parts]
    roles = [str(item.role if hasattr(item, "role") else item["role"]) for item in parts]
    root_id = assembly_graph.get("root_part_id")
    if selector in {"primary_form", "base_form"}:
        return str(root_id) if root_id in part_ids else part_ids[0]
    if selector == "secondary_form":
        return part_ids[1] if len(part_ids) > 1 else None
    keywords = {
        "cabin_form": ("cabin", "cockpit", "canopy"),
        "upper_link_form": ("upper", "link_1", "boom_a", "desktop_link", "rail_link", "welding_arm"),
        "end_effector_form": ("tool", "gripper", "claw", "probe", "camera", "sensor"),
    }.get(selector, ())
    matches = [part_ids[index] for index, role in enumerate(roles) if any(word in role for word in keywords)]
    if matches:
        return matches[-1] if selector == "end_effector_form" else matches[0]
    if selector == "upper_link_form":
        return part_ids[min(2, len(part_ids) - 1)]
    if selector == "end_effector_form":
        return part_ids[-1]
    return part_ids[1] if len(part_ids) > 1 else None
