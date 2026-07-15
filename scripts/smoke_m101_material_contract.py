#!/usr/bin/env python3
"""FGC-M101: verify complete visual material fields and legacy migration."""

from __future__ import annotations

import json
import warnings
from pathlib import Path

warnings.simplefilter("ignore", DeprecationWarning)

from jsonschema import Draft202012Validator, RefResolver, ValidationError  # noqa: E402
from pydantic import ValidationError as PydanticValidationError  # noqa: E402

from forgecad_agent.application.agent_models import AgentMaterialPreset  # noqa: E402


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "packages" / "concept-spec" / "schemas"


def schema_validator() -> Draft202012Validator:
    schema = json.loads((SCHEMA_DIR / "material-preset.schema.json").read_text(encoding="utf-8"))
    common = json.loads((SCHEMA_DIR / "common.schema.json").read_text(encoding="utf-8"))
    resolver = RefResolver.from_schema(schema, store={common["$id"]: common})
    return Draft202012Validator(schema, resolver=resolver)


def old_payload() -> dict[str, object]:
    return {
        "schema_version": "MaterialPreset@1",
        "material_id": "mat_m101_legacy",
        "display_name": "旧版兼容材质",
        "category": "metal",
        "pbr": {"base_color": "#223344", "metallic": 0.5, "roughness": 0.4, "opacity": 1},
        "visual_only": True,
        "allowed_domains": ["vehicle_concept"],
        "provenance": "forgecad_builtin",
    }


def main() -> int:
    validator = schema_validator()
    legacy = old_payload()
    validator.validate(legacy)
    migrated = AgentMaterialPreset.model_validate(legacy)
    assert migrated.source == "forgecad_builtin"
    assert migrated.license == "not_applicable"
    assert migrated.version == "1"
    assert migrated.visual_tags == ["metal"]

    complete = migrated.model_dump(mode="json")
    complete["visual_tags"] = ["metal", "brushed", "matte"]
    complete["pbr"].update({
        "normal_strength": 0.8,
        "emissive_color": "#112233",
        "emissive_strength": 0,
        "transmission": 0,
        "ior": 1.5,
        "clearcoat": 0.2,
        "clearcoat_roughness": 0.3,
        "texture_scale": [1, 1],
        "base_color_texture_asset_id": "asset_m101_base_color",
    })
    validator.validate(complete)
    AgentMaterialPreset.model_validate(complete)

    for bad in (
        {**complete, "pbr": {**complete["pbr"], "ior": 0.5}},
        {**complete, "pbr": {**complete["pbr"], "normal_texture_asset_id": "/tmp/normal.png"}},
    ):
        try:
            validator.validate(bad)
        except ValidationError:
            pass
        else:
            raise AssertionError("invalid MaterialPreset payload was accepted by JSON Schema")
    try:
        AgentMaterialPreset.model_validate({**complete, "pbr": {**complete["pbr"], "texture_scale": [0, 1]}})
    except PydanticValidationError:
        pass
    else:
        raise AssertionError("invalid texture scale was accepted by Pydantic")

    print("FGC-M101 MaterialPreset contract/migration smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
