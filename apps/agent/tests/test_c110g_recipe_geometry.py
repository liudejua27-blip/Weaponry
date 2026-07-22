from __future__ import annotations

import json
from pathlib import Path

from forgecad_agent.application.geometry_worker import compile_shape_program


ROOT = Path(__file__).resolve().parents[3]
FIXTURE = ROOT / "packages/concept-spec/fixtures/c110g-parallel-link-component-recipe-registry.json"


def test_c110g_recipe_shape_programs_compile_in_the_restricted_worker() -> None:
    payload = json.loads(FIXTURE.read_text(encoding="utf-8"))
    assert payload["registry_id"] == "registry_c110g_parallel_link_robotic_arm_v1"
    assert len(payload["recipes"]) == 5
    for recipe in payload["recipes"]:
        compiled = compile_shape_program(recipe["shape_program_template"])
        assert compiled.readback.triangle_count > 0
