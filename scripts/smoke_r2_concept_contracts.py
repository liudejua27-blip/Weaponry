#!/usr/bin/env python3
from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Dict, Type

from jsonschema import Draft202012Validator
from pydantic import BaseModel, ValidationError
from referencing import Registry, Resource

from forgecad_agent.domain.concepts import (
    DesignChangeSet,
    DesignDomainProfile,
    JobEventV2,
    ModelQualityReport,
    ModuleAssetManifest,
    ModuleGraph,
    WeaponConceptSpec,
)


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "packages" / "concept-spec" / "schemas"


def main() -> int:
    schemas = {
        path.name: json.loads(path.read_text(encoding="utf-8"))
        for path in sorted(SCHEMA_DIR.glob("*.json"))
    }
    registry = Registry().with_resources(
        (schema["$id"], Resource.from_contents(schema))
        for schema in schemas.values()
    )
    for schema in schemas.values():
        Draft202012Validator.check_schema(schema)

    contracts: list[tuple[str, Type[BaseModel], Dict[str, Any]]] = [
        ("design-domain-profile.schema.json", DesignDomainProfile, _profile()),
        ("weapon-concept-spec.schema.json", WeaponConceptSpec, _concept_spec()),
        ("module-asset-manifest.schema.json", ModuleAssetManifest, _module_manifest()),
        ("module-graph.schema.json", ModuleGraph, _module_graph()),
        ("design-change-set.schema.json", DesignChangeSet, _change_set()),
        ("model-quality-report.schema.json", ModelQualityReport, _quality_report()),
        ("job-event-v2.schema.json", JobEventV2, _job_event()),
    ]

    validated: list[str] = []
    for schema_name, model_type, payload in contracts:
        model = model_type.model_validate(payload)
        schema = schemas[schema_name]
        Draft202012Validator(
            schema,
            registry=registry,
        ).validate(model.model_dump(mode="json", exclude_none=True))
        validated.append(model.__class__.__name__)

    _assert_rejects_unknown_field()
    _assert_rejects_disconnected_graph()
    _assert_rejects_protected_node_change()
    _assert_rejects_inconsistent_quality_summary()

    print(
        json.dumps(
            {
                "ok": True,
                "contract_count": len(validated),
                "contracts": validated,
                "negative_invariants": [
                    "unknown_field",
                    "disconnected_graph",
                    "protected_node_change",
                    "quality_summary",
                ],
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


def _profile() -> Dict[str, Any]:
    return {
        "schema_version": "DesignDomainProfile@1",
        "profile_id": "profile_weapon_concept_v1",
        "domain_type": "weapon_concept",
        "display_name": "Weapon Concept Pack",
        "pack_id": "pack_weapon_concept_v1",
        "intended_uses": ["game_asset", "film_prop", "non_functional_display"],
        "module_categories": [
            "core_shell",
            "front_shell",
            "rear_shell",
            "grip_shell",
            "top_accessory",
            "side_accessory",
            "lower_structure",
            "storage_visual",
            "armor_panel",
        ],
        "required_connectors": ["core.front", "core.rear", "core.grip"],
        "optional_connectors": [
            "core.top",
            "core.bottom",
            "core.left",
            "core.right",
            "core.side_panel_left",
            "core.side_panel_right",
        ],
        "export_profiles": ["visual_asset", "game_asset", "film_prop"],
        "non_functional_only": True,
    }


def _concept_spec() -> Dict[str, Any]:
    return {
        "schema_version": "WeaponConceptSpec@1",
        "project_id": "prj_arctic_patrol_s1",
        "profile_id": "profile_weapon_concept_v1",
        "name": "寒地巡逻 S1",
        "archetype": "future_modular_sidearm",
        "intended_uses": ["game_asset", "film_prop", "non_functional_display"],
        "style": {
            "keywords": ["寒地", "工业", "紧凑", "硬表面"],
            "palette": ["graphite", "gunmetal", "signal_red"],
            "detail_density": 0.68,
        },
        "proportions": {
            "overall_length_mm": 230,
            "body_height_mm": 54,
            "grip_angle_deg": 15,
        },
        "required_slots": ["core", "front", "rear", "grip"],
        "optional_slots": ["top", "left", "right", "bottom", "side_panels"],
        "constraints": {
            "symmetry": "mostly_symmetric",
            "max_triangle_count": 180000,
        },
        "assumptions": ["非功能性概念模型，不用于真实制造或使用"],
    }


def _transform() -> Dict[str, Any]:
    return {
        "position": [0.0, 0.0, 0.0],
        "rotation": [0.0, 0.0, 0.0],
        "scale": [1.0, 1.0, 1.0],
    }


def _module_manifest() -> Dict[str, Any]:
    return {
        "schema_version": "ModuleAssetManifest@1",
        "module_id": "module_core_shell_01",
        "pack_id": "pack_weapon_concept_v1",
        "category": "core_shell",
        "asset_id": "asset_core_shell_01",
        "sha256": "a" * 64,
        "bounds_mm": [148, 56, 42],
        "triangle_count": 28400,
        "material_slots": ["primary", "secondary", "accent"],
        "connectors": [
            {
                "connector_id": "connector_core_front",
                "slot": "core.front",
                "connector_type": "shell_front",
                "transform": _transform(),
                "scale_range": [0.9, 1.1],
                "exclusive": True,
            },
            {
                "connector_id": "connector_core_grip",
                "slot": "core.grip",
                "connector_type": "grip_mount",
                "transform": _transform(),
                "scale_range": [0.92, 1.08],
                "exclusive": True,
            },
        ],
    }


def _module_graph() -> Dict[str, Any]:
    return {
        "schema_version": "ModuleGraph@1",
        "graph_id": "mg_arctic_patrol_v1",
        "project_id": "prj_arctic_patrol_s1",
        "root_node_id": "node_core",
        "nodes": [
            {
                "node_id": "node_core",
                "module_id": "module_core_shell_01",
                "transform": _transform(),
                "locked": True,
                "visible": True,
            },
            {
                "node_id": "node_front",
                "module_id": "module_front_shell_02",
                "transform": _transform(),
                "locked": False,
                "visible": True,
            },
        ],
        "edges": [
            {
                "edge_id": "edge_core_front",
                "from_node_id": "node_core",
                "from_connector_id": "connector_core_front",
                "to_node_id": "node_front",
                "to_connector_id": "connector_front_core",
                "status": "connected",
            }
        ],
    }


def _change_set() -> Dict[str, Any]:
    return {
        "schema_version": "DesignChangeSet@1",
        "change_set_id": "change_top_profile_01",
        "project_id": "prj_arctic_patrol_s1",
        "base_version_id": "ver_arctic_patrol_v1",
        "summary": "降低前部外壳高度并保留锁定核心",
        "operations": [
            {
                "operation_id": "op_front_scale",
                "op": "set_transform",
                "node_id": "node_front",
                "transform": {
                    "position": [0.0, 0.0, 0.0],
                    "rotation": [0.0, 0.0, 0.0],
                    "scale": [1.05, 0.9, 1.0],
                },
            }
        ],
        "protected_node_ids": ["node_core"],
        "status": "proposed",
    }


def _quality_report() -> Dict[str, Any]:
    return {
        "schema_version": "ModelQualityReport@1",
        "report_id": "quality_arctic_patrol_v1",
        "project_id": "prj_arctic_patrol_s1",
        "version_id": "ver_arctic_patrol_v1",
        "ruleset_version": "weapon-concept-quality/1.0",
        "status": "warning",
        "findings": [
            {
                "finding_id": "finding_symmetry_01",
                "check_id": "assembly.symmetry_deviation",
                "category": "assembly",
                "severity": "warning",
                "status": "warning",
                "node_ids": ["node_front"],
                "measured_value": 0.4,
                "threshold": 0.25,
                "message": "前部外壳超出目标对称偏差。",
                "suggestion": "重新吸附右侧面板或接受非对称风格。",
            }
        ],
        "created_at": "2026-07-10T12:00:00+00:00",
    }


def _job_event() -> Dict[str, Any]:
    return {
        "schema_version": "JobEvent@2",
        "event_id": "evt_concept_0001",
        "job_id": "job_concept_assemble_01",
        "seq": 1,
        "project_id": "prj_arctic_patrol_s1",
        "version_id": "ver_arctic_patrol_v1",
        "step": "validate_module_graph",
        "level": "info",
        "status": "succeeded",
        "message": "ModuleGraph validation passed.",
        "progress": 1.0,
        "artifact_asset_id": None,
        "metadata": {"node_count": 2, "edge_count": 1},
        "created_at": "2026-07-10T12:00:00+00:00",
    }


def _assert_rejects_unknown_field() -> None:
    payload = {**_concept_spec(), "manufacturing_ready": True}
    _expect_validation_error(WeaponConceptSpec, payload)


def _assert_rejects_disconnected_graph() -> None:
    payload = _module_graph()
    payload["edges"] = []
    _expect_validation_error(ModuleGraph, payload)


def _assert_rejects_protected_node_change() -> None:
    payload = _change_set()
    payload["operations"][0]["node_id"] = "node_core"
    _expect_validation_error(DesignChangeSet, payload)


def _assert_rejects_inconsistent_quality_summary() -> None:
    payload = _quality_report()
    payload["status"] = "passed"
    _expect_validation_error(ModelQualityReport, payload)


def _expect_validation_error(model_type: Type[BaseModel], payload: Dict[str, Any]) -> None:
    try:
        model_type.model_validate(payload)
    except ValidationError:
        return
    raise AssertionError(f"{model_type.__name__} accepted an invalid fixture")


if __name__ == "__main__":
    raise SystemExit(main())
