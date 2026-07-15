from __future__ import annotations

from typing import List, Literal

from pydantic import Field

from .concept_models import StrictApiModel


DomainPackId = Literal[
    "pack_future_weapon_prop",
    "pack_vehicle_concept",
    "pack_aircraft_concept",
    "pack_robotic_arm_concept",
]
DomainName = Literal[
    "future_weapon_prop",
    "vehicle_concept",
    "aircraft_concept",
    "robotic_arm_concept",
]


class DomainPackManifest(StrictApiModel):
    schema_version: Literal["DomainPackManifest@1"] = "DomainPackManifest@1"
    pack_id: DomainPackId
    domain: DomainName
    display_name: str = Field(min_length=1, max_length=80)
    description: str = Field(min_length=1, max_length=500)
    non_functional_only: Literal[True] = True
    templates: List[str] = Field(min_length=1)
    connector_types: List[str] = Field(default_factory=list)
    joint_types: List[Literal["fixed", "hinge", "slider", "ball", "continuous"]] = Field(default_factory=list)
    material_preset_ids: List[str] = Field(min_length=1)
    quality_profile_id: str
    export_profile_id: str


DOMAIN_PACKS: List[DomainPackManifest] = [
    DomainPackManifest(
        pack_id="pack_future_weapon_prop",
        domain="future_weapon_prop",
        display_name="未来武器概念道具",
        description="虚构游戏资产、影视道具与非功能性未来道具的完整外观概念。",
        templates=["compact_prop", "long_profile_prop", "heavy_support_prop", "energy_visual_prop"],
        connector_types=["surface_mount", "rail_mount", "panel_mount", "tool_mount"],
        joint_types=["fixed", "hinge", "slider"],
        material_preset_ids=["mat_graphite", "mat_signal_red", "mat_dark_glass"],
        quality_profile_id="quality_concept_default",
        export_profile_id="export_concept_default",
    ),
    DomainPackManifest(
        pack_id="pack_vehicle_concept",
        domain="vehicle_concept",
        display_name="汽车与地面载具",
        description="未来汽车、探索车、竞速车与科幻运输工具的完整外观概念。",
        templates=["urban_scout", "exploration_vehicle", "low_racer", "heavy_transport"],
        connector_types=["surface_mount", "wheel_mount", "panel_mount", "payload_mount"],
        joint_types=["fixed", "hinge", "continuous"],
        material_preset_ids=["mat_graphite", "mat_automotive_paint", "mat_rubber"],
        quality_profile_id="quality_concept_default",
        export_profile_id="export_concept_default",
    ),
    DomainPackManifest(
        pack_id="pack_aircraft_concept",
        domain="aircraft_concept",
        display_name="飞机与航空器",
        description="未来飞机、垂直起降器与无人航空器的完整外观概念。",
        templates=["fast_single_seat", "wide_body_transport", "vertical_takeoff", "uncrewed_scout"],
        connector_types=["surface_mount", "wing_mount", "nacelle_mount", "payload_mount"],
        joint_types=["fixed", "hinge", "slider"],
        material_preset_ids=["mat_graphite", "mat_composite", "mat_dark_glass"],
        quality_profile_id="quality_concept_default",
        export_profile_id="export_concept_default",
    ),
    DomainPackManifest(
        pack_id="pack_robotic_arm_concept",
        domain="robotic_arm_concept",
        display_name="机械臂与机器人机构",
        description="机械臂、服务机器人上肢与科幻操作机构的概念资产。",
        templates=["precision_light", "heavy_handler", "long_reach_maintenance", "dual_tool_service"],
        connector_types=["surface_mount", "axial_mount", "tool_mount", "panel_mount"],
        joint_types=["fixed", "hinge", "slider", "ball", "continuous"],
        material_preset_ids=["mat_graphite", "mat_aluminum", "mat_rubber"],
        quality_profile_id="quality_concept_default",
        export_profile_id="export_concept_default",
    ),
]


def list_domain_packs() -> List[DomainPackManifest]:
    return [pack.model_copy(deep=True) for pack in DOMAIN_PACKS]


def domain_pack_by_id(pack_id: DomainPackId) -> DomainPackManifest:
    for pack in DOMAIN_PACKS:
        if pack.pack_id == pack_id:
            return pack.model_copy(deep=True)
    raise ValueError(f"Unknown registered domain pack: {pack_id}")


def domain_pack_for_message(message: str) -> DomainPackManifest:
    """Compatibility helper for callers with an explicitly recognizable brief.

    New turn handling must use ``infer_domain`` directly so ambiguous and
    unsupported input cannot be silently converted into a weapon concept.
    """

    from .domain_inference import infer_domain

    result = infer_domain(message)
    if result.status != "recognized" or result.domain_pack_id is None:
        raise ValueError(f"Domain is {result.status}; a unique domain pack is required")
    return domain_pack_by_id(result.domain_pack_id)
