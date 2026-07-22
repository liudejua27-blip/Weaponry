#!/usr/bin/env python3
"""FGC-R007 robotic-arm golden: read-only GLB evidence -> constrained plan."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from forgecad_agent.application.geometry_worker import compile_shape_program
from forgecad_agent.application.reference_guided_rebuild import build_robotic_arm_rebuild_plan, extract_reference_evidence


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    fixture = json.loads((ROOT / "apps/agent/tests/fixtures/r007_robotic_arm_reference.json").read_text(encoding="utf-8"))
    golden = json.loads((ROOT / "packages/concept-spec/fixtures/component-recipe-expanded-golden.json").read_text(encoding="utf-8"))
    candidate = next(item for item in golden["candidates"] if item["recipe"]["recipe_id"] == "recipe_robotic_arm_link")
    compiled = compile_shape_program(candidate["expanded_shape_program"], artifact_profile_id="interactive_preview")
    original_sha256 = hashlib.sha256(compiled.glb_bytes).hexdigest()
    evidence = extract_reference_evidence(fixture["source"], compiled.glb_bytes)
    plan = build_robotic_arm_rebuild_plan(evidence)
    assert evidence.reference_sha256 == evidence.source_unchanged_sha256 == original_sha256
    assert evidence.readback_kind == "forgecad_glb_readback"
    assert plan.c105_recipe_ids == fixture["expected"]["recipes"]
    assert plan.g819_operation_allowlist == fixture["expected"]["operations"]
    assert plan.read_only_reference and plan.rebuild_mode == "new_constrained_candidate"
    assert hashlib.sha256(compiled.glb_bytes).hexdigest() == original_sha256
    print("R007 reference rebuild smoke passed: robotic-arm read-only evidence, real GLB readback, bounded C105/D005/G819 plan")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
