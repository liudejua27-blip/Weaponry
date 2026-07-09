#!/usr/bin/env python3
"""Fixed prompt quality gate for Wushen Forge release candidates.

This is a deterministic planner-level gate. It does not claim real image or 3D
provider quality; it verifies that the first-stage spec contract stays ready for
fictional Unity asset generation across the documented fixed prompt set.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.models import CreateWeaponRequest  # noqa: E402
from wushen_agent.providers.llm import (  # noqa: E402
    QUALITY_NEGATIVE_PROMPT_EXCLUSIONS,
    build_fallback_weapon_spec,
    derive_display_name,
    derive_weapon_family,
)
from wushen_agent.spec_validation import validate_weapon_design_spec  # noqa: E402


FIXED_PROMPTS: list[dict[str, Any]] = [
    {
        "id": "sword_dragon",
        "category": "sword",
        "text": "赤金龙纹长剑，宽厚剑脊，玉石能量核心，3渲2国风神兵，适合 Unity 游戏资产",
    },
    {
        "id": "blade_moon",
        "category": "blade",
        "text": "玄铁月牙刀，刀背有火焰灵纹和金色护手，强剪影，高拟真游戏外观",
    },
    {
        "id": "spear_thunder",
        "category": "spear",
        "text": "雷纹破阵长枪，枪头包裹蓝紫电光，红缨与青铜云纹结合",
    },
    {
        "id": "halberd_celestial",
        "category": "halberd",
        "text": "星宿方天画戟，左右月刃不对称，云雷纹浮雕，适合单图转 3D",
    },
    {
        "id": "bow_phoenix",
        "category": "bow",
        "text": "凤凰灵弓，弓臂像展开羽翼，中心红玉发光，国风符箓点缀",
    },
    {
        "id": "crossbow_mechanism",
        "category": "crossbow",
        "text": "机关弩弓，木金混合外观，齿轮和符箓作为幻想装饰，非现实制造说明",
    },
    {
        "id": "scythe_shadow",
        "category": "scythe",
        "text": "幽影镰刀，弯月刀刃与黑金长柄，紫色灵气缠绕，强轮廓线",
    },
    {
        "id": "staff_jade",
        "category": "staff",
        "text": "青玉法杖，杖首悬浮玉环和小型灵火，金属包边，三渲二质感",
    },
    {
        "id": "umbrella_rain",
        "category": "umbrella",
        "text": "听雨机关伞，伞骨如青铜兽骨，伞面水墨云纹，手持游戏道具外观",
    },
    {
        "id": "fan_wind",
        "category": "fan",
        "text": "裂风战扇，扇骨金属化，扇面水墨白鹤与青色风刃，国风神兵",
    },
    {
        "id": "mechanical_axe",
        "category": "mechanical",
        "text": "机械齿轮战斧，斧身有可视化灵能齿轮和铜质外壳，只作为虚构资产",
    },
    {
        "id": "energy_sword",
        "category": "energy",
        "text": "灵能光剑，实体剑柄连接半透明玉色能量刃，古代符文悬浮",
    },
    {
        "id": "alien_blade",
        "category": "alien",
        "text": "异形骨刃刀，像远古妖兽骨片生长出的刀身，金色封印纹稳定结构",
    },
    {
        "id": "hybrid_umbrella_sword",
        "category": "hybrid",
        "text": "伞剑组合神兵，合拢是细长剑，展开是水墨伞盾，视觉结构清晰",
    },
    {
        "id": "mace_fire",
        "category": "mace",
        "text": "狼牙棒式月脉锤，圆鼓锤头带金齿冠纹，适合大动作打击姿态",
    },
    {
        "id": "trident_aquatic",
        "category": "trident",
        "text": "三叉潮汐戟，三齿锋利分叉对齐，前端有灵气波动，保持结构清晰",
    },
    {
        "id": "dagger_radiant",
        "category": "dagger",
        "text": "匕首短刃与环绕玉轮的握柄结构，紧凑贴手，快速闪避动作姿态",
    },
    {
        "id": "bone_staff",
        "category": "staff",
        "text": "白骨镇妖杖，骨质外观与青铜箍、红色宝石组合，暗色写实但游戏化",
    },
    {
        "id": "pants_cannon",
        "category": "other",
        "text": "穿着战斗服的防弹裤，直接神兵化成可旋转开火的神圣防线武器，保持单件清晰主体",
    },
    {
        "id": "stick_cannon",
        "category": "staff",
        "text": "一根普通木棍，变成能发射震荡光的神炮棍，稳定站姿与战斗重心，3渲2可用",
    },
]

REQUIRED_CATEGORIES = {
    "sword",
    "blade",
    "spear",
    "bow",
    "staff",
    "umbrella",
    "fan",
    "mechanical",
    "energy",
    "alien",
    "mace",
    "trident",
    "dagger",
}

BANNED_ACTIONABLE_TERMS = [
    "manufacturing drawing",
    "manufacturing dimensions",
    "material recipe",
    "material formula",
    "fabrication process",
    "assembly instruction",
    "machining steps",
    "real-world dimensions",
]


def main() -> int:
    failures: list[dict[str, str]] = []
    results: list[dict[str, Any]] = []

    if len(FIXED_PROMPTS) != 20:
        failures.append({"code": "PROMPT_SET_SIZE", "message": f"expected 20 prompts, got {len(FIXED_PROMPTS)}"})

    categories = {item["category"] for item in FIXED_PROMPTS}
    missing_categories = sorted(REQUIRED_CATEGORIES - categories)
    if missing_categories:
        failures.append({"code": "PROMPT_CATEGORY_COVERAGE", "message": f"missing categories: {missing_categories}"})

    for index, item in enumerate(FIXED_PROMPTS, start=1):
        request = CreateWeaponRequest(client_request_id=f"release-prompt-quality-{item['id']}", text=item["text"])
        spec = build_fallback_weapon_spec(
            request,
            weapon_id=f"weapon_prompt_quality_{index:02d}",
            display_name=derive_display_name(item["text"]),
            weapon_family=derive_weapon_family(item["text"]),
            planner_provider="mock",
        )
        try:
            validated = validate_weapon_design_spec(spec, provider_id="release_prompt_quality")
            schema_valid = True
        except Exception as exc:  # noqa: BLE001 - gate reports validation failures as findings.
            validated = spec
            schema_valid = False
            failures.append({"code": "SCHEMA_INVALID", "message": f"{item['id']}: {exc}"})

        result = {
            "id": item["id"],
            "category": item["category"],
            "weapon_family": validated.get("weapon_family"),
            "schema_valid": schema_valid,
            "weapon_like": _is_weapon_like(validated),
            "style_ready": _is_style_ready(validated),
            "manufacturing_safe": _is_manufacturing_safe(validated),
            "image_artifact_guarded": _is_image_artifact_guarded(validated),
            "single_image_3d_ready": _is_single_image_3d_ready(validated),
        }
        results.append(result)

    counts = {
        "prompt_count": len(FIXED_PROMPTS),
        "weapon_like": sum(1 for item in results if item["weapon_like"]),
        "style_ready": sum(1 for item in results if item["style_ready"]),
        "manufacturing_unsafe": sum(1 for item in results if not item["manufacturing_safe"]),
        "image_artifact_risk": sum(1 for item in results if not item["image_artifact_guarded"]),
        "single_image_3d_ready": sum(1 for item in results if item["single_image_3d_ready"]),
        "schema_valid": sum(1 for item in results if item["schema_valid"]),
    }

    _threshold(failures, counts["weapon_like"] >= 18, "WEAPON_LIKE_THRESHOLD", f"weapon_like={counts['weapon_like']}/20")
    _threshold(failures, counts["style_ready"] >= 16, "STYLE_THRESHOLD", f"style_ready={counts['style_ready']}/20")
    _threshold(
        failures,
        counts["manufacturing_unsafe"] == 0,
        "MANUFACTURING_SAFETY_THRESHOLD",
        f"manufacturing_unsafe={counts['manufacturing_unsafe']}/20",
    )
    _threshold(
        failures,
        counts["image_artifact_risk"] <= 2,
        "IMAGE_ARTIFACT_THRESHOLD",
        f"image_artifact_risk={counts['image_artifact_risk']}/20",
    )
    _threshold(
        failures,
        counts["single_image_3d_ready"] >= 15,
        "SINGLE_IMAGE_3D_THRESHOLD",
        f"single_image_3d_ready={counts['single_image_3d_ready']}/20",
    )
    _threshold(failures, counts["schema_valid"] == 20, "SCHEMA_VALID_THRESHOLD", f"schema_valid={counts['schema_valid']}/20")

    report = {
        "ok": not failures,
        "mode": "deterministic_mock_planner",
        "thresholds": {
            "weapon_like": ">=18/20",
            "style_ready": ">=16/20",
            "manufacturing_unsafe": "0/20",
            "image_artifact_risk": "<=2/20",
            "single_image_3d_ready": ">=15/20",
            "schema_valid": "20/20",
        },
        "counts": counts,
        "failures": failures,
        "results": results,
    }
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if not failures else 1


def _is_weapon_like(spec: dict[str, Any]) -> bool:
    family = spec.get("weapon_family")
    return isinstance(family, str) and family not in {"", "other"}


def _is_style_ready(spec: dict[str, Any]) -> bool:
    style_text = f"{spec.get('style', '')} {spec.get('generation', {}).get('concept_prompt', '')}".lower()
    silhouette = spec.get("silhouette") if isinstance(spec.get("silhouette"), dict) else {}
    toon_rules = spec.get("toon_rules") if isinstance(spec.get("toon_rules"), dict) else {}
    material_zones = spec.get("material_zones") if isinstance(spec.get("material_zones"), list) else []
    visual_keywords = spec.get("visual_keywords") if isinstance(spec.get("visual_keywords"), list) else []
    return (
        "3渲2" in style_text
        and "国风" in style_text
        and silhouette.get("readability") == "strong"
        and len(visual_keywords) >= 3
        and len(material_zones) >= 3
        and bool(toon_rules.get("outline"))
    )


def _is_manufacturing_safe(spec: dict[str, Any]) -> bool:
    safety = spec.get("safety_boundary") if isinstance(spec.get("safety_boundary"), dict) else {}
    unity_target = spec.get("unity_target") if isinstance(spec.get("unity_target"), dict) else {}
    scale_contract = unity_target.get("scale_contract") if isinstance(unity_target.get("scale_contract"), dict) else {}
    negative_prompt = str(spec.get("generation", {}).get("negative_prompt", "")).lower()
    positive_text = _positive_design_text(spec)
    return (
        safety.get("real_world_manufacturing_details") is False
        and scale_contract.get("forbid_real_world_dimensions") is True
        and all(term in negative_prompt for term in ["manufacturing drawing", "dimensions", "material formula", "machining steps"])
        and not any(term in positive_text for term in BANNED_ACTIONABLE_TERMS)
    )


def _is_image_artifact_guarded(spec: dict[str, Any]) -> bool:
    negative_prompt = str(spec.get("generation", {}).get("negative_prompt", "")).lower()
    return all(term in negative_prompt for term in QUALITY_NEGATIVE_PROMPT_EXCLUSIONS[-4:])


def _is_single_image_3d_ready(spec: dict[str, Any]) -> bool:
    unity_target = spec.get("unity_target") if isinstance(spec.get("unity_target"), dict) else {}
    model_3d = unity_target.get("model_3d") if isinstance(unity_target.get("model_3d"), dict) else {}
    expected_outputs = set(model_3d.get("expected_outputs") or [])
    required_outputs = {"rough_raw_glb", "rough_optimized_glb", "unity_material_json", "quality_report"}
    return (
        unity_target.get("format") == "glb"
        and unity_target.get("scale_policy") == "normalized_game_asset_scale"
        and model_3d.get("source_image_role") in {"concept_image", "model_sheet_image"}
        and required_outputs <= expected_outputs
    )


def _positive_design_text(spec: dict[str, Any]) -> str:
    safe_projection = {
        "name": spec.get("name"),
        "style": spec.get("style"),
        "weapon_family": spec.get("weapon_family"),
        "fantasy_category": spec.get("fantasy_category"),
        "silhouette": spec.get("silhouette"),
        "visual_keywords": spec.get("visual_keywords"),
        "color_palette": spec.get("color_palette"),
        "material_zones": spec.get("material_zones"),
        "toon_rules": spec.get("toon_rules"),
        "unity_target": spec.get("unity_target"),
    }
    return json.dumps(safe_projection, ensure_ascii=False).lower()


def _threshold(failures: list[dict[str, str]], condition: bool, code: str, message: str) -> None:
    if not condition:
        failures.append({"code": code, "message": message})


if __name__ == "__main__":
    raise SystemExit(main())
