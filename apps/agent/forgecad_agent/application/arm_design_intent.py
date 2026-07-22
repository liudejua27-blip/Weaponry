"""Deterministic bounded projection from a robotic-arm brief to ArmDesignIntent.

The projection is intentionally small and explainable.  It is a bridge until
the Rust Product Tool accepts the same contract from DeepSeek.  It never emits
dimensions, ShapeProgram operations, executable code, or engineering claims.
"""

from __future__ import annotations

from collections.abc import Iterable
from typing import List, Literal

from pydantic import BaseModel, ConfigDict, Field, model_validator


class ArmIntentModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


ArmArchitecture = Literal["serial_chain", "parallel_link", "scara", "gantry", "delta", "cantilever"]
ArmJointLanguage = Literal["armored_bearing", "exposed_ring", "gimbal_shell", "capsule_joint", "bellows_joint"]
ArmLinkLanguage = Literal["closed_shell", "twin_rail", "open_truss", "tapered_loft", "tube_frame"]
ArmBaseLanguage = Literal["round_turntable", "hex_platform", "floating_pedestal", "industrial_deck", "compact_puck"]
ArmWristLanguage = Literal["layered_wrist", "gimbal_wrist", "cylindrical_wrist", "fork_wrist"]
ArmEndEffectorLanguage = Literal["parallel_gripper", "adaptive_claw", "precision_tool", "sensor_probe", "soft_pad_gripper"]
ArmCableLanguage = Literal["internal_routing", "braided_external", "armored_harness", "minimal_cable"]
ArmSurfaceLanguage = Literal["panel_seams", "flowline", "chevron_relief", "hex_microgrid", "engraved_ribs", "fastener_bands"]
ArmMaterialPalette = Literal["graphite_blue", "white_aluminum", "industrial_yellow", "warm_copper", "monochrome_technical"]
ArmDetailDensity = Literal["light", "medium", "dense"]
ArmPose = Literal["neutral", "grounded", "elevated", "extended", "folded"]
ArmProportionProfile = Literal["compact", "balanced", "long_reach", "heavy_base", "slender"]


class ArmDesignIntent(ArmIntentModel):
    """Bounded visual intent for composable robotic-arm generation."""

    schema_version: Literal["ArmDesignIntent@1"] = "ArmDesignIntent@1"
    domain_pack_id: Literal["pack_robotic_arm_concept"] = "pack_robotic_arm_concept"
    architecture: ArmArchitecture
    joint_language: ArmJointLanguage
    link_language: ArmLinkLanguage
    base_language: ArmBaseLanguage
    wrist_language: ArmWristLanguage
    end_effector_language: ArmEndEffectorLanguage
    cable_language: ArmCableLanguage
    surface_language: List[ArmSurfaceLanguage] = Field(min_length=1, max_length=6)
    material_palette: ArmMaterialPalette
    detail_density: ArmDetailDensity
    pose: ArmPose
    proportion_profile: ArmProportionProfile
    style_keywords: List[str] = Field(default_factory=list, max_length=12)
    source: Literal["user_brief", "reference_evidence", "agent_inferred"] = "user_brief"
    visual_only: Literal[True] = True

    @model_validator(mode="after")
    def validate_intent_lists(self) -> "ArmDesignIntent":
        normalized = [item.strip() for item in self.style_keywords if item.strip()]
        if len({item.casefold() for item in normalized}) != len(normalized):
            raise ValueError("style_keywords must be unique")
        self.style_keywords = normalized
        if len(set(self.surface_language)) != len(self.surface_language):
            raise ValueError("surface_language must be unique")
        return self


def _first(text: str, choices: Iterable[tuple[tuple[str, ...], str]], default: str) -> str:
    for phrases, value in choices:
        if any(phrase in text for phrase in phrases):
            return value
    return default


def infer_arm_design_intent(brief: str, *, source: str = "user_brief") -> ArmDesignIntent:
    """Resolve visual vocabulary without silently inventing geometry."""

    text = brief.casefold().strip()
    architecture = _first(
        text,
        (
            (("scara",), "scara"),
            (("龙门", "gantry"), "gantry"),
            (("并联", "delta"), "parallel_link"),
            (("三角",), "delta"),
            (("悬臂", "cantilever"), "cantilever"),
        ),
        "serial_chain",
    )
    joint_language = _first(
        text,
        (
            (("轴承盒", "轴承", "bearing"), "armored_bearing"),
            (("外露环", "外露轴", "exposed ring"), "exposed_ring"),
            (("万向", "gimbal"), "gimbal_shell"),
            (("波纹", "bellows"), "bellows_joint"),
        ),
        "capsule_joint",
    )
    link_language = _first(
        text,
        (
            (("开放桁架", "桁架", "open truss"), "open_truss"),
            (("双导轨", "导轨", "twin rail"), "twin_rail"),
            (("管状", "tube frame"), "tube_frame"),
            (("锥形", "tapered"), "tapered_loft"),
        ),
        "closed_shell",
    )
    base_language = _first(
        text,
        (
            (("六边形", "hex"), "hex_platform"),
            (("悬浮", "floating"), "floating_pedestal"),
            (("圆盘", "圆形底座", "turntable"), "round_turntable"),
            (("紧凑底座", "compact base"), "compact_puck"),
        ),
        "industrial_deck",
    )
    wrist_language = _first(
        text,
        (
            (("叉形腕", "fork wrist"), "fork_wrist"),
            (("万向腕", "gimbal wrist"), "gimbal_wrist"),
            (("圆柱腕", "cylindrical wrist"), "cylindrical_wrist"),
        ),
        "layered_wrist",
    )
    end_effector_language = _first(
        text,
        (
            (("自适应夹爪", "adaptive claw"), "adaptive_claw"),
            (("探针", "传感器", "sensor probe"), "sensor_probe"),
            (("精密工具", "precision tool"), "precision_tool"),
            (("软垫", "soft pad"), "soft_pad_gripper"),
        ),
        "parallel_gripper",
    )
    cable_language = _first(
        text,
        (
            (("内部走线", "隐藏线缆", "internal routing"), "internal_routing"),
            (("编织线", "braided"), "braided_external"),
            (("装甲线束", "armored harness"), "armored_harness"),
            (("少线缆", "minimal cable"), "minimal_cable"),
        ),
        "armored_harness",
    )
    surface_language: list[str] = []
    for phrases, value in (
        (("流线", "flowline"), "flowline"),
        (("人字", "chevron"), "chevron_relief"),
        (("六边形纹理", "蜂窝", "hex grid"), "hex_microgrid"),
        (("雕刻", "肋条", "engraved"), "engraved_ribs"),
        (("接缝", "panel seam"), "panel_seams"),
        (("紧固件", "fastener"), "fastener_bands"),
    ):
        if any(phrase in text for phrase in phrases):
            surface_language.append(value)
    if not surface_language:
        surface_language = ["panel_seams", "flowline"]

    material_palette = _first(
        text,
        (
            (("蓝黑", "蓝色", "graphite blue"), "graphite_blue"),
            (("白色", "铝白", "white aluminum"), "white_aluminum"),
            (("黄色", "工业黄", "industrial yellow"), "industrial_yellow"),
            (("铜色", "暖铜", "copper"), "warm_copper"),
        ),
        "monochrome_technical",
    )
    detail_density = _first(
        text,
        ((("非常细节", "高细节", "dense", "精致"), "dense"), (("简单", "简洁", "light"), "light")),
        "medium",
    )
    pose = _first(
        text,
        (
            (("收起", "折叠", "folded"), "folded"),
            (("抬起", "抬升", "elevated"), "elevated"),
            (("舒展", "伸展", "extended"), "extended"),
            (("低姿态", "grounded"), "grounded"),
        ),
        "neutral",
    )
    proportion_profile = _first(
        text,
        (
            (("长臂", "远 reach", "long reach", "修长"), "long_reach"),
            (("厚重", "重型", "heavy base"), "heavy_base"),
            (("纤细", "slender"), "slender"),
            (("紧凑", "compact"), "compact"),
        ),
        "balanced",
    )
    keywords = [
        token
        for token in (
            "robotic_arm",
            architecture,
            joint_language,
            link_language,
            base_language,
            wrist_language,
            end_effector_language,
            cable_language,
            material_palette,
            detail_density,
            pose,
            proportion_profile,
        )
        if token
    ]
    return ArmDesignIntent(
        architecture=architecture,
        joint_language=joint_language,
        link_language=link_language,
        base_language=base_language,
        wrist_language=wrist_language,
        end_effector_language=end_effector_language,
        cable_language=cable_language,
        surface_language=surface_language,
        material_palette=material_palette,
        detail_density=detail_density,
        pose=pose,
        proportion_profile=proportion_profile,
        style_keywords=keywords,
        source=source,
    )
