#!/usr/bin/env python3
"""Gate the bounded brief-to-visual-family projection for all four packs."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from jsonschema import Draft202012Validator, RefResolver

from forgecad_agent.application.geometry_worker import (
    BLOCKOUT_VARIANT_IDS,
    build_blockout,
    resolve_blockout_variant,
)
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner
from forgecad_agent.application.visual_intent import VisualIntentMapping
from forgecad_agent.application.domain_packs import domain_pack_by_id


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "concept-spec" / "schemas"
PACK_CASES = {
    "pack_future_weapon_prop": (
        "设计一个紧凑、简洁、深色、低姿态的非功能未来概念道具，用于游戏展示。",
        "设计一个延展、细节丰富、信号色、抬升展示的非功能未来概念道具，用于影视展示。",
    ),
    "pack_vehicle_concept": (
        "设计一辆紧凑、简洁、深色、低姿态的未来城市汽车，完整外观展示。",
        "设计一辆延展、细节丰富、信号色、抬升展示的未来探索汽车，完整外观展示。",
    ),
    "pack_aircraft_concept": (
        "设计一架紧凑、简洁、深色、低姿态展示的未来概念飞机，完整外观展示。",
        "设计一架延展、细节丰富、信号色、抬升展示的未来概念飞机，完整外观展示。",
    ),
    "pack_robotic_arm_concept": (
        "设计一台紧凑、简洁、深色、低姿态展示的三关节机械臂概念模型。",
        "设计一台延展、细节丰富、信号色、抬升展示的长臂机械臂概念模型。",
    ),
}


def _schema_validator() -> Draft202012Validator:
    common = json.loads((SCHEMA_ROOT / "common.schema.json").read_text(encoding="utf-8"))
    mapping = json.loads((SCHEMA_ROOT / "visual-intent-mapping.schema.json").read_text(encoding="utf-8"))
    return Draft202012Validator(mapping, resolver=RefResolver.from_schema(mapping, store={common["$id"]: common, mapping["$id"]: mapping}))


def _plan(pack_id: str, brief: str, plan_id: str):
    plan = DeterministicMechanicalPlanner().plan_complete_concept(
        brief=brief,
        pack=domain_pack_by_id(pack_id),
        project_id="prj_g815_projection",
    )
    return plan.model_copy(update={"plan_id": plan_id})


def main() -> int:
    validator = _schema_validator()
    for pack_id, (first_brief, second_brief) in PACK_CASES.items():
        first = _plan(pack_id, first_brief, f"plan_g815_{pack_id}_fixed")
        second = _plan(pack_id, second_brief, f"plan_g815_{pack_id}_fixed")
        first_mapping = VisualIntentMapping.model_validate(first.spec["visual_intent_mapping"])
        second_mapping = VisualIntentMapping.model_validate(second.spec["visual_intent_mapping"])
        validator.validate(first_mapping.model_dump(mode="json"))
        validator.validate(second_mapping.model_dump(mode="json"))
        assert first_mapping.domain_pack_id == second_mapping.domain_pack_id == pack_id
        assert len(first_mapping.directions) == len(second_mapping.directions) == 3
        assert first_mapping.directions[0].variant_family_index != second_mapping.directions[0].variant_family_index
        assert first.directions[0].silhouette == first_mapping.directions[0].silhouette
        assert second.directions[0].silhouette == second_mapping.directions[0].silhouette

        first_variant = resolve_blockout_variant(first, "direction_1")
        second_variant = resolve_blockout_variant(second, "direction_1")
        variants = BLOCKOUT_VARIANT_IDS[pack_id]
        assert variants.index(first_variant) // 3 == first_mapping.directions[0].variant_family_index
        assert variants.index(second_variant) // 3 == second_mapping.directions[0].variant_family_index
        assert first_variant != second_variant

        first_build = build_blockout(first, "direction_1", first_variant)
        first_replay = build_blockout(first, "direction_1", first_variant)
        second_build = build_blockout(second, "direction_1", second_variant)
        assert first_build.glb_bytes == first_replay.glb_bytes
        assert first_build.topology_hash == first_replay.topology_hash
        assert first_build.topology_hash != second_build.topology_hash
        assert hashlib.sha256(first_build.glb_bytes).hexdigest() != hashlib.sha256(second_build.glb_bytes).hexdigest()
        assert all(item["op"] in {"box", "cylinder", "capsule", "wedge"} for item in first_build.shape_program["operations"])

        malformed = first.model_copy(update={"spec": {**first.spec, "visual_intent_mapping": {"unexpected": True}}})
        fallback = resolve_blockout_variant(malformed, "direction_1")
        expected_family = {"compact": 0, "balanced": 1, "extended": 2, "industrial": 3, "organic": 3}[malformed.directions[0].silhouette]
        assert variants.index(fallback) // 3 == expected_family

    print("G815 visual intent projection smoke passed: four packs, bounded mapping, deterministic fingerprints and malformed fallback")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
