#!/usr/bin/env python3
"""Validate the first general-mechanical JSON contracts and semantic guards."""

from __future__ import annotations

import json
import warnings
from pathlib import Path
from typing import Any, Dict

warnings.simplefilter("ignore", DeprecationWarning)

from jsonschema import Draft202012Validator, RefResolver, ValidationError  # noqa: E402


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "concept-spec" / "schemas"


def load_schema(name: str) -> Dict[str, Any]:
    return json.loads((SCHEMA_ROOT / name).read_text(encoding="utf-8"))


SCHEMAS = {
    name: load_schema(name)
    for name in (
        "common.schema.json",
        "domain-pack-manifest.schema.json",
        "visual-intent-mapping.schema.json",
        "mechanical-concept-spec.schema.json",
        "assembly-graph.schema.json",
        "material-preset.schema.json",
    )
}
STORE = {schema["$id"]: schema for schema in SCHEMAS.values()}


def validate(name: str, value: Dict[str, Any]) -> None:
    schema = SCHEMAS[name]
    resolver = RefResolver.from_schema(schema, store=STORE)
    Draft202012Validator(schema, resolver=resolver).validate(value)


def expect_invalid(name: str, value: Dict[str, Any]) -> None:
    try:
        validate(name, value)
    except ValidationError:
        return
    raise AssertionError(f"{name} unexpectedly accepted invalid value")


def assert_assembly_semantics(graph: Dict[str, Any]) -> None:
    parts = graph["parts"]
    ids = {part["part_id"] for part in parts}
    assert len(ids) == len(parts), "duplicate part id"
    assert graph["root_part_id"] in ids, "root part is missing"
    for part in parts:
        parent = part["parent_part_id"]
        assert parent is None or parent in ids, f"orphan parent: {parent}"
    for part in parts:
        seen: set[str] = set()
        cursor = part["part_id"]
        while cursor is not None:
            assert cursor not in seen, f"assembly cycle at {cursor}"
            seen.add(cursor)
            parent = next(item["parent_part_id"] for item in parts if item["part_id"] == cursor)
            cursor = parent
    connectors = {
        (part["part_id"], connector["connector_id"])
        for part in parts
        for connector in part["connectors"]
    }
    for connection in graph["connections"]:
        assert (connection["from_part_id"], connection["from_connector_id"]) in connectors
        assert (connection["to_part_id"], connection["to_connector_id"]) in connectors


def main() -> int:
    pack = {
        "schema_version": "DomainPackManifest@1",
        "pack_id": "pack_vehicle_concept",
        "domain": "vehicle_concept",
        "display_name": "汽车与地面载具",
        "description": "用于非功能性汽车概念展示的领域包。",
        "non_functional_only": True,
        "templates": ["exploration_vehicle", "urban_scout"],
        "connector_types": ["mount", "wheel_hub"],
        "joint_types": ["fixed", "hinge"],
        "material_preset_ids": ["mat_graphite", "mat_rubber"],
        "quality_profile_id": "quality_concept_default",
        "export_profile_id": "export_concept_default",
    }
    validate("domain-pack-manifest.schema.json", pack)
    expect_invalid("domain-pack-manifest.schema.json", {**pack, "non_functional_only": False})

    concept = {
        "schema_version": "MechanicalConceptSpec@1",
        "concept_id": "asset_concept_vehicle_demo",
        "project_id": "prj_g2_demo",
        "domain_pack_id": "pack_vehicle_concept",
        "brief": "冰原探索车，完整外观，便于继续调整。",
        "design_language": {
            "keywords": ["冰原", "紧凑", "工业"],
            "silhouette": "compact",
            "detail_density": "medium",
            "color_direction": "石墨灰与冷蓝色点缀",
        },
        "visual_intent_mapping": {
            "schema_version": "VisualIntentMapping@1",
            "domain_pack_id": "pack_vehicle_concept",
            "source": "brief_lexicon_v1",
            "directions": [
                {"direction_id": "direction_1", "silhouette": "compact", "detail_density": "medium", "color_theme": "dark_neutral", "pose_category": "neutral", "variant_family_index": 1},
                {"direction_id": "direction_2", "silhouette": "balanced", "detail_density": "dense", "color_theme": "signal_accent", "pose_category": "elevated", "variant_family_index": 2},
                {"direction_id": "direction_3", "silhouette": "extended", "detail_density": "simple", "color_theme": "light_technical", "pose_category": "grounded", "variant_family_index": 1},
            ],
        },
        "envelope": {"min_mm": [0, 0, 0], "max_mm": [2400, 1800, 1500]},
        "pose": {"position": [0, 0, 0], "rotation": [0, 0, 0]},
        "full_look": {
            "completeness": "full_exterior",
            "generation_stage": "blockout",
            "primary_part_roles": ["body", "cabin", "wheel"],
            "preview_views": ["perspective", "front", "side"],
        },
        "material_intents": [
            {"zone_role": "body_shell", "material_preset_id": "mat_graphite"},
            {"zone_role": "tire", "material_preset_id": "mat_rubber"},
        ],
        "non_functional_only": True,
    }
    validate("mechanical-concept-spec.schema.json", concept)
    expect_invalid("mechanical-concept-spec.schema.json", {**concept, "unexpected": True})

    transform = {"position": [0, 0, 0], "rotation": [0, 0, 0], "scale": [1, 1, 1]}
    graph = {
        "schema_version": "AssemblyGraph@1",
        "graph_id": "mg_g2_demo",
        "concept_id": "asset_concept_vehicle_demo",
        "root_part_id": "part_body",
        "parts": [
            {
                "part_id": "part_body",
                "role": "body",
                "parent_part_id": None,
                "geometry_source": "shape_program",
                "transform": transform,
                "connectors": [{"connector_id": "connector_body_mount", "kind": "mount", "position": [0, 0, 0], "normal": [0, 1, 0]}],
                "joints": [],
                "material_zones": ["zone_body_shell"],
                "editable_parameters": ["body_length"],
                "locked": False,
                "provenance": "agent_generated",
            },
            {
                "part_id": "part_wheel",
                "role": "wheel",
                "parent_part_id": "part_body",
                "geometry_source": "module_asset",
                "transform": transform,
                "connectors": [{"connector_id": "connector_wheel_mount", "kind": "wheel_hub", "position": [1, 0, 0], "normal": [0, 1, 0]}],
                "joints": [{"joint_id": "joint_wheel_spin", "kind": "continuous", "target_part_id": "part_wheel", "axis": [1, 0, 0], "min_value": -3.14, "max_value": 3.14}],
                "material_zones": ["zone_tire"],
                "editable_parameters": ["wheel_radius"],
                "locked": False,
                "provenance": "agent_generated",
            },
        ],
        "connections": [{"connection_id": "conn_body_wheel", "from_part_id": "part_body", "from_connector_id": "connector_body_mount", "to_part_id": "part_wheel", "to_connector_id": "connector_wheel_mount", "status": "connected"}],
    }
    validate("assembly-graph.schema.json", graph)
    assert_assembly_semantics(graph)
    cyclic = json.loads(json.dumps(graph))
    cyclic["parts"][0]["parent_part_id"] = "part_wheel"
    expect_invalid("assembly-graph.schema.json", {**cyclic, "extra": True})
    try:
        assert_assembly_semantics(cyclic)
    except AssertionError:
        pass
    else:
        raise AssertionError("assembly cycle was not rejected by semantic guard")

    material = {
        "schema_version": "MaterialPreset@1",
        "material_id": "mat_graphite",
        "display_name": "石墨灰金属外观",
        "category": "metal",
        "pbr": {"base_color": "#252B33", "metallic": 0.85, "roughness": 0.32, "opacity": 1},
        "visual_only": True,
        "allowed_domains": ["future_weapon_prop", "vehicle_concept", "aircraft_concept", "robotic_arm_concept"],
        "provenance": "forgecad_builtin",
    }
    validate("material-preset.schema.json", material)
    full_material = {
        **material,
        "visual_tags": ["metal", "brushed", "matte"],
        "source": "forgecad_builtin",
        "license": "not_applicable",
        "version": "1.0",
        "pbr": {
            **material["pbr"],
            "normal_strength": 0.8,
            "emissive_color": "#112233",
            "emissive_strength": 0,
            "transmission": 0,
            "ior": 1.5,
            "clearcoat": 0.2,
            "clearcoat_roughness": 0.3,
            "texture_scale": [1, 1],
        },
    }
    validate("material-preset.schema.json", full_material)
    expect_invalid("material-preset.schema.json", {**full_material, "pbr": {**full_material["pbr"], "ior": 0.5}})
    expect_invalid("material-preset.schema.json", {**material, "visual_only": False})

    print("G2 general contracts smoke passed: schemas, strict fields, non-functional boundary, assembly semantics")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
