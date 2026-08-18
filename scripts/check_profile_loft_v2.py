#!/usr/bin/env python3
"""Focused source gate for the bounded ProfileLoftRequest/Program v2 slice."""

from __future__ import annotations

import copy
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCHEMAS = ROOT / "packages" / "forgecad-contracts" / "schemas"
FIXTURES = ROOT / "packages" / "forgecad-contracts" / "fixtures" / "profile-loft-v2"
sys.path.insert(0, str(ROOT / "scripts"))
from check_agentic_contracts import is_valid  # noqa: E402


def fail(message: str) -> None:
    raise SystemExit(f"Profile loft v2 violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"{path.relative_to(ROOT)} must be an object")
    return value


def canonical_hash(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def main() -> None:
    request_schema = load(SCHEMAS / "profile-loft-request-v2.schema.json")
    program_schema = load(SCHEMAS / "profile-loft-program-v2.schema.json")
    geometry_schema = load(SCHEMAS / "geometry-program-v2.schema.json")
    manifest = load(ROOT / "packages" / "forgecad-contracts" / "manifest.json")
    positive = load(FIXTURES / "positive" / "profile-loft-request.json")
    negative = load(FIXTURES / "negative" / "profile-loft-request-g1.json")

    require(is_valid(request_schema, positive), "positive request fixture is schema-invalid")
    require(not is_valid(request_schema, negative), "G1 request must fail closed")
    plan = copy.deepcopy(positive["cross_section_plan"])
    declared_plan_hash = plan["canonical_sha256"]
    plan["canonical_sha256"] = ""
    require(canonical_hash(plan) == declared_plan_hash, "CrossSectionPlan hash is stale")
    binding = copy.deepcopy(positive)
    declared_input_hash = binding.pop("input_sha256")
    require(canonical_hash(binding) == declared_input_hash, "request input hash is stale")

    policy = positive["continuity_policy"]
    require(policy["surface_continuity"] == "g0-only", "only G0 may be claimed")
    require(
        policy["profile_point_correspondence"] == "canonical-phase-arc-length",
        "correspondence must use canonical phase and arc length",
    )
    require(policy["endpoint_caps"] == "ear-clipped-planar", "unsafe fan caps are forbidden")
    require(policy["hole_policy"] == "reject", "holes/islands must fail closed in this slice")
    require(
        len({len(station["points"]) for station in positive["cross_section_plan"]["stations"]}) > 1,
        "positive fixture must prove unequal point-count resampling",
    )

    operators = geometry_schema["$defs"]["geometry_node"]["properties"]["operator_id"]["enum"]
    parameter_refs = {
        item.get("$ref")
        for item in geometry_schema["$defs"]["geometry_node"]["properties"]["parameters"]["oneOf"]
    }
    require("forgecad.geometry.profile-loft@2" in operators, "GeometryProgram@2 omits operator")
    require("#/$defs/profile_loft_v2_parameters" in parameter_refs, "GeometryProgram@2 omits parameters")
    require(
        program_schema["properties"]["lowered_operator_id"]["const"]
        == "forgecad.geometry.profile-loft@2",
        "program must bind the real v2 Worker, not a v1 lowerer",
    )
    files = set(manifest["schemas"])
    require(
        {
            "profile-loft-request-v2.schema.json",
            "profile-loft-program-v2.schema.json",
        }
        <= files,
        "contract manifest omits profile loft v2 schemas",
    )
    print(
        "Profile loft v2 source gate PASS: contract/hash/unequal-count/G0/ear-cap/hole-reject bindings"
    )


if __name__ == "__main__":
    main()
