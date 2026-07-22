#!/usr/bin/env python3
"""C110A contract smoke for bounded robotic-arm design intent."""

from __future__ import annotations

import json
from pathlib import Path

from forgecad_agent.application.arm_design_intent import infer_arm_design_intent


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = ROOT / "packages/concept-spec/schemas/arm-design-intent.schema.json"


def main() -> int:
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    assert schema["title"] == "ArmDesignIntent"
    intent = infer_arm_design_intent(
        "并联机械臂，六边形底座，外露环关节，双导轨连杆，内部走线，传感器探针，白色铝材，六边形纹理"
    )
    assert intent.architecture == "parallel_link"
    assert intent.base_language == "hex_platform"
    assert intent.joint_language == "exposed_ring"
    assert intent.link_language == "twin_rail"
    assert intent.cable_language == "internal_routing"
    assert intent.end_effector_language == "sensor_probe"
    assert intent.material_palette == "white_aluminum"
    assert intent.surface_language == ["hex_microgrid"]
    print(json.dumps(intent.model_dump(mode="json"), ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
