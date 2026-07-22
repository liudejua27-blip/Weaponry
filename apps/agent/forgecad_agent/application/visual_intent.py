"""Bounded, local visual-intent projection for mechanical concept blockouts.

This module never converts a brief into dimensions, executable geometry, or
engineering instructions.  It classifies a small reviewed set of visual words
and selects only one of the existing per-domain visual families.
"""

from __future__ import annotations

from typing import Literal, Sequence

from pydantic import Field, model_validator

from .concept_models import StrictApiModel


VisualSilhouette = Literal["compact", "balanced", "extended", "organic", "industrial"]
VisualDetailDensity = Literal["simple", "medium", "dense"]
VisualColorTheme = Literal["dark_neutral", "signal_accent", "light_technical", "warm_contrast"]
VisualPoseCategory = Literal["neutral", "grounded", "elevated", "extended"]


class DirectionVisualIntent(StrictApiModel):
    direction_id: str = Field(pattern=r"^direction_[a-z0-9_\-]+$")
    silhouette: VisualSilhouette
    detail_density: VisualDetailDensity
    color_theme: VisualColorTheme
    pose_category: VisualPoseCategory
    variant_family_index: int = Field(ge=0, le=3)


class VisualIntentMapping(StrictApiModel):
    # @1 is retained for persisted legacy plans that contain three review
    # directions.  V003 emits @2 and binds one visual intent to the one complete
    # synthesis allowed in the Turn.
    schema_version: Literal["VisualIntentMapping@1", "VisualIntentMapping@2"] = "VisualIntentMapping@2"
    domain_pack_id: Literal[
        "pack_future_weapon_prop",
        "pack_vehicle_concept",
        "pack_aircraft_concept",
        "pack_robotic_arm_concept",
    ]
    source: Literal["brief_lexicon_v1"] = "brief_lexicon_v1"
    directions: list[DirectionVisualIntent] = Field(min_length=1, max_length=3)

    @model_validator(mode="after")
    def validate_directions(self) -> "VisualIntentMapping":
        expected = 3 if self.schema_version == "VisualIntentMapping@1" else 1
        if len(self.directions) != expected:
            raise ValueError(f"{self.schema_version} requires exactly {expected} direction intent(s)")
        if len({item.direction_id for item in self.directions}) != len(self.directions):
            raise ValueError("visual intent direction ids must be unique")
        return self


_FAMILY_BY_SILHOUETTE: dict[VisualSilhouette, int] = {
    "compact": 0,
    "balanced": 1,
    "extended": 2,
    "industrial": 3,
    "organic": 3,
}
_SILHOUETTES: tuple[VisualSilhouette, ...] = ("compact", "balanced", "extended", "industrial")
_DETAILS: tuple[VisualDetailDensity, ...] = ("simple", "medium", "dense")
_COLORS: tuple[VisualColorTheme, ...] = ("dark_neutral", "signal_accent", "light_technical", "warm_contrast")
_POSES: tuple[VisualPoseCategory, ...] = ("neutral", "grounded", "elevated", "extended")


def build_visual_intent_mapping(
    *,
    brief: str,
    domain_pack_id: str,
    direction_ids: Sequence[str],
) -> VisualIntentMapping:
    """Classify one safe brief into the single bounded V003 visual intent."""
    if len(direction_ids) != 1 or len(set(direction_ids)) != 1:
        raise ValueError("V003 visual intent mapping requires exactly one direction id")
    if domain_pack_id not in {
        "pack_future_weapon_prop",
        "pack_vehicle_concept",
        "pack_aircraft_concept",
        "pack_robotic_arm_concept",
    }:
        raise ValueError("visual intent mapping requires a registered domain pack")
    normalized = brief.casefold()
    base_silhouette = _classify_silhouette(normalized)
    base_detail = _classify_detail(normalized)
    base_color = _classify_color(normalized)
    base_pose = _classify_pose(normalized)
    items: list[DirectionVisualIntent] = []
    for offset, direction_id in enumerate(direction_ids):
        silhouette = _SILHOUETTES[(_SILHOUETTES.index(base_silhouette if base_silhouette != "organic" else "industrial") + offset) % len(_SILHOUETTES)]
        detail = _DETAILS[(_DETAILS.index(base_detail) + offset) % len(_DETAILS)]
        color = _COLORS[(_COLORS.index(base_color) + offset) % len(_COLORS)]
        pose = _POSES[(_POSES.index(base_pose) + offset) % len(_POSES)]
        items.append(
            DirectionVisualIntent(
                direction_id=direction_id,
                silhouette=silhouette,
                detail_density=detail,
                color_theme=color,
                pose_category=pose,
                variant_family_index=_family_index(silhouette, detail, color, pose),
            )
        )
    return VisualIntentMapping(
        schema_version="VisualIntentMapping@2",
        domain_pack_id=domain_pack_id,
        directions=items,
    )


def visual_intent_for_direction(
    mapping_payload: object,
    *,
    domain_pack_id: str,
    direction_id: str,
) -> DirectionVisualIntent | None:
    """Read a valid mapping only; malformed/old plans safely fall back."""
    try:
        mapping = VisualIntentMapping.model_validate(mapping_payload)
    except Exception:  # Old plans and untrusted Provider payloads use existing behavior.
        return None
    if mapping.domain_pack_id != domain_pack_id:
        return None
    return next((item for item in mapping.directions if item.direction_id == direction_id), None)


def visual_intent_description(intent: DirectionVisualIntent) -> str:
    silhouette_labels = {"compact": "紧凑", "balanced": "均衡", "extended": "延展", "organic": "流线", "industrial": "工业"}
    detail_labels = {"simple": "简洁", "medium": "适中细节", "dense": "细节丰富"}
    color_labels = {"dark_neutral": "深色中性", "signal_accent": "信号色点缀", "light_technical": "浅色技术感", "warm_contrast": "暖色对比"}
    pose_labels = {"neutral": "中性展示", "grounded": "低姿态展示", "elevated": "抬升展示", "extended": "舒展展示"}
    return "、".join((silhouette_labels[intent.silhouette], detail_labels[intent.detail_density], color_labels[intent.color_theme], pose_labels[intent.pose_category]))


def _family_index(
    silhouette: VisualSilhouette,
    detail: VisualDetailDensity,
    color: VisualColorTheme,
    pose: VisualPoseCategory,
) -> int:
    # Each supported visual category participates in a bounded 0..3 family
    # choice. It remains a catalog selection, not a geometric parameter.
    detail_offset = {"simple": 0, "medium": 1, "dense": 2}[detail]
    color_offset = {"dark_neutral": 0, "signal_accent": 1, "light_technical": 2, "warm_contrast": 3}[color]
    pose_offset = {"neutral": 0, "grounded": 1, "elevated": 2, "extended": 3}[pose]
    return (_FAMILY_BY_SILHOUETTE[silhouette] + detail_offset + color_offset + pose_offset) % 4


def _classify_silhouette(brief: str) -> VisualSilhouette:
    if _contains(brief, ("紧凑", "短", "compact", "small")):
        return "compact"
    if _contains(brief, ("延展", "细长", "长轴", "长臂", "long", "extended")):
        return "extended"
    if _contains(brief, ("流线", "圆润", "柔和", "organic", "streamlined")):
        return "organic"
    if _contains(brief, ("厚重", "工业", "重装", "装甲", "heavy", "industrial")):
        return "industrial"
    return "balanced"


def _classify_detail(brief: str) -> VisualDetailDensity:
    if _contains(brief, ("简洁", "极简", "干净", "minimal", "simple")):
        return "simple"
    if _contains(brief, ("细节丰富", "层叠", "面板", "复杂", "dense", "detailed")):
        return "dense"
    return "medium"


def _classify_color(brief: str) -> VisualColorTheme:
    if _contains(brief, ("信号色", "霓虹", "红色", "蓝色", "绿色", "橙色", "signal", "neon")):
        return "signal_accent"
    if _contains(brief, ("白色", "银色", "浅色", "亮色", "white", "silver", "light")):
        return "light_technical"
    if _contains(brief, ("暖色", "金色", "铜色", "橙黄", "warm", "gold", "copper")):
        return "warm_contrast"
    return "dark_neutral"


def _classify_pose(brief: str) -> VisualPoseCategory:
    if _contains(brief, ("低姿态", "贴地", "低矮", "grounded", "low stance")):
        return "grounded"
    if _contains(brief, ("抬升", "悬浮", "高姿态", "elevated", "raised")):
        return "elevated"
    if _contains(brief, ("展开", "舒展", "宽翼", "伸展", "extended pose", "spread")):
        return "extended"
    return "neutral"


def _contains(text: str, phrases: Sequence[str]) -> bool:
    return any(phrase.casefold() in text for phrase in phrases)
