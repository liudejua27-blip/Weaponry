#!/usr/bin/env python3
"""Offline product-contract gate for the one-patch convergence envelope."""

from __future__ import annotations

import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator, ValidationError
from referencing import Registry, Resource


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_DIR = ROOT / "packages" / "concept-spec" / "schemas"
TURN_TABLE = (
    "turntable_000",
    "turntable_045",
    "turntable_090",
    "turntable_135",
    "turntable_180",
    "turntable_225",
    "turntable_270",
    "turntable_315",
)


def load(name: str) -> dict:
    return json.loads((SCHEMA_DIR / name).read_text(encoding="utf-8"))


def validator(name: str) -> Draft202012Validator:
    schema = load(name)
    resources = []
    for path in SCHEMA_DIR.glob("*.json"):
        item = json.loads(path.read_text(encoding="utf-8"))
        if "$id" in item:
            resources.append((item["$id"], Resource.from_contents(item)))
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema, registry=Registry().with_resources(resources))


def expect_invalid(contract: Draft202012Validator, value: dict) -> None:
    try:
        contract.validate(value)
    except ValidationError:
        return
    raise AssertionError("VisualConvergence@2 accepted a second or invalid repair")


def digest(character: str) -> str:
    return character * 64


def convergence_input() -> dict:
    source = digest("a")
    outputs = [digest(character) for character in ("b", "c", "d", "e", "f", "1", "2")]
    stages = ("silhouette", "structure", "form", "material", "surface", "lighting", "optimization")
    previous = source
    passes = []
    for stage, output in zip(stages, outputs):
        passes.append({"stage": stage, "input_sha256": previous, "output_sha256": output, "completed": True})
        previous = output
    return {
        "schema_version": "VisualConvergenceInput@2",
        "ledger": {
            "schema_version": "DesignBuildLedger@1",
            "source_program_sha256": source,
            "source_revision": 1,
            "passes": passes,
        },
        "readback": {
            "glb_sha256": outputs[-1],
            "shape_program_sha256": source,
            "triangle_count": 1,
            "primitive_count": 1,
            "material_zone_count": 1,
            "closed_manifold": True,
            "surface_provenance_present": True,
            "pbr_channels_complete": True,
        },
        "fixed_views": [
            {"view_id": view_id, "glb_sha256": outputs[-1], "renderer_id": "workbench_pbr_renderer_v1", "image_sha256": digest(f"{index + 3:x}"), "readback_passed": True}
            for index, view_id in enumerate(TURN_TABLE)
        ],
        "detail_coverage": {"macro_bound": 1, "meso_bound": 1, "micro_bound": 1, "critical_unresolved": 0},
        "repairs": [],
    }


def convergence_report() -> dict:
    return {
        "schema_version": "VisualConvergenceReport@2",
        "report_sha256": digest("9"),
        "source_program_sha256": digest("a"),
        "source_revision": 1,
        "glb_sha256": digest("2"),
        "passed": True,
        "completed_stage_count": 7,
        "fixed_view_count": 8,
        "repair_attempt_count": 1,
        "failure_codes": [],
    }


def repair() -> dict:
    return {
        "repair_number": 1,
        "parent_program_sha256": digest("8"),
        "result_program_sha256": digest("a"),
        "changed_domains": ["surface"],
        "same_intent": True,
    }


def main() -> None:
    input_contract = validator("visual-convergence-input-v2.schema.json")
    report_contract = validator("visual-convergence-report-v2.schema.json")
    initial = convergence_input()
    input_contract.validate(initial)
    one_patch = copy.deepcopy(initial)
    one_patch["repairs"] = [repair()]
    input_contract.validate(one_patch)

    second_patch = copy.deepcopy(one_patch)
    second_patch["repairs"].append(repair())
    expect_invalid(input_contract, second_patch)
    wrong_number = copy.deepcopy(one_patch)
    wrong_number["repairs"][0]["repair_number"] = 2
    expect_invalid(input_contract, wrong_number)
    legacy_view = copy.deepcopy(initial)
    legacy_view["fixed_views"][0]["view_id"] = "front"
    expect_invalid(input_contract, legacy_view)

    report = convergence_report()
    report_contract.validate(report)
    report["repair_attempt_count"] = 2
    expect_invalid(report_contract, report)
    print("VisualConvergence@2 contract gate passed")


if __name__ == "__main__":
    main()
