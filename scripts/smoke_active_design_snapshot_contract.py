#!/usr/bin/env python3
"""S001 contract smoke for the not-yet-persisted ActiveDesignSnapshot@1."""

from __future__ import annotations

import json
import warnings
from pathlib import Path
from typing import Any, Dict

warnings.simplefilter("ignore", DeprecationWarning)

from jsonschema import Draft202012Validator, RefResolver, ValidationError
from pydantic import ValidationError as PydanticValidationError

from forgecad_agent.application.agent_models import ActiveDesignSnapshot


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "concept-spec" / "schemas"


def load_schema(name: str) -> Dict[str, Any]:
    return json.loads((SCHEMA_ROOT / name).read_text(encoding="utf-8"))


COMMON = load_schema("common.schema.json")
SNAPSHOT_SCHEMA = load_schema("active-design-snapshot.schema.json")
STORE = {COMMON["$id"]: COMMON, SNAPSHOT_SCHEMA["$id"]: SNAPSHOT_SCHEMA}


def validate_schema(value: Dict[str, Any]) -> None:
    resolver = RefResolver.from_schema(SNAPSHOT_SCHEMA, store=STORE)
    Draft202012Validator(SNAPSHOT_SCHEMA, resolver=resolver).validate(value)


def expect_schema_invalid(value: Dict[str, Any]) -> None:
    try:
        validate_schema(value)
    except ValidationError:
        return
    raise AssertionError("snapshot schema unexpectedly accepted invalid data")


def expect_model_invalid(value: Dict[str, Any]) -> None:
    try:
        ActiveDesignSnapshot.model_validate(value)
    except PydanticValidationError:
        return
    raise AssertionError("ActiveDesignSnapshot model unexpectedly accepted invalid data")


def agent_snapshot() -> Dict[str, Any]:
    return {
        "schema_version": "ActiveDesignSnapshot@1",
        "project_id": "prj_snapshot_demo",
        "active_design": {
            "source": "agent_asset",
            "project_id": "prj_snapshot_demo",
            "asset_version_id": "assetver_snapshot_v3",
            "assembly_graph_id": "mg_snapshot_v3",
        },
        "selected_part_id": "part_body_shell",
        "preview": {
            "project_id": "prj_snapshot_demo",
            "change_set_id": "assetcs_snapshot_preview",
            "base_asset_version_id": "assetver_snapshot_v3",
        },
        "quality": {
            "project_id": "prj_snapshot_demo",
            "quality_report_id": "quality_snapshot_v3",
            "asset_version_id": "assetver_snapshot_v3",
        },
        "export": {
            "source": "agent_asset",
            "project_id": "prj_snapshot_demo",
            "source_version_id": "assetver_snapshot_v3",
        },
        "revision": 3,
        "updated_at": "2026-07-13T12:00:00Z",
    }


def main() -> int:
    agent = agent_snapshot()
    validate_schema(agent)
    ActiveDesignSnapshot.model_validate(agent)

    legacy = {
        **agent,
        "active_design": {
            "source": "legacy_concept_read_only",
            "project_id": "prj_snapshot_demo",
            "legacy_version_id": "ver_legacy_v7",
            "module_graph_id": "mg_legacy_v7",
        },
        "selected_part_id": None,
        "preview": None,
        "quality": None,
        "export": {
            "source": "legacy_concept_read_only",
            "project_id": "prj_snapshot_demo",
            "source_version_id": "ver_legacy_v7",
        },
    }
    validate_schema(legacy)
    ActiveDesignSnapshot.model_validate(legacy)

    unknown_field = {**agent, "unexpected": True}
    expect_schema_invalid(unknown_field)
    expect_model_invalid(unknown_field)

    cross_project = json.loads(json.dumps(agent))
    cross_project["active_design"]["project_id"] = "prj_other"
    validate_schema(cross_project)
    expect_model_invalid(cross_project)

    invalid_preview = json.loads(json.dumps(agent))
    invalid_preview["preview"]["base_asset_version_id"] = "assetver_other"
    validate_schema(invalid_preview)
    expect_model_invalid(invalid_preview)

    invalid_quality = json.loads(json.dumps(agent))
    invalid_quality["quality"]["asset_version_id"] = "assetver_other"
    validate_schema(invalid_quality)
    expect_model_invalid(invalid_quality)

    invalid_export = json.loads(json.dumps(agent))
    invalid_export["export"]["source_version_id"] = "assetver_other"
    validate_schema(invalid_export)
    expect_model_invalid(invalid_export)

    conflicting_source = json.loads(json.dumps(agent))
    conflicting_source["active_design"]["legacy_version_id"] = "ver_conflict"
    expect_schema_invalid(conflicting_source)
    expect_model_invalid(conflicting_source)

    invalid_legacy = json.loads(json.dumps(legacy))
    invalid_legacy["selected_part_id"] = "part_not_allowed"
    validate_schema(invalid_legacy)
    expect_model_invalid(invalid_legacy)

    print("S001 ActiveDesignSnapshot contract smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
