#!/usr/bin/env python3
"""Gate the four-domain deterministic blockout catalog (12 variants x 4 packs)."""

from __future__ import annotations

import hashlib
import json
import struct
from typing import Any

from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.geometry_worker import build_blockout, list_blockout_variants, read_shape_program_glb
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner


BRIEFS = (
    "设计一个未来概念道具",
    "设计一辆冰原探索车",
    "设计一架垂直起降飞行器",
    "设计一台三关节机械臂",
)


def _structure_signature(result: Any) -> str:
    summary = [
        {
            "op": operation.get("op"),
            "role": operation.get("args", {}).get("part_role"),
            "args": {
                key: operation.get("args", {}).get(key)
                for key in ("size", "radius", "height", "axis", "angle", "points")
                if key in operation.get("args", {})
            },
        }
        for operation in result.shape_program.get("operations", [])
        if operation.get("op") != "profile"
    ]
    payload = json.dumps(summary, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def main() -> int:
    planner = DeterministicMechanicalPlanner()
    all_signatures: set[str] = set()
    total = 0
    for brief in BRIEFS:
        pack = domain_pack_for_message(brief)
        variants = list_blockout_variants(pack.pack_id)
        assert len(variants) == 12 and len(set(variants)) == 12, pack.pack_id
        plan = planner.plan_complete_concept(brief=brief, pack=pack, project_id="prj_g807_smoke")
        direction_id = plan.directions[0].direction_id
        signatures: set[str] = set()
        for variant_id in variants:
            result = build_blockout(plan, direction_id, variant_id=variant_id)
            assert result.variant_id == variant_id
            assert result.glb_bytes[:4] == b"glTF"
            assert struct.unpack_from("<I", result.glb_bytes, 4)[0] == 2
            assert 0 < result.triangle_count <= 100000
            assert all(value > 0 for value in result.bounds_mm)
            assert read_shape_program_glb(result.glb_bytes) == (result.triangle_count, result.bounds_mm)
            repeat = build_blockout(plan, direction_id, variant_id=variant_id)
            assert result.topology_hash == repeat.topology_hash
            assert result.glb_bytes == repeat.glb_bytes
            parts = result.assembly_graph["parts"]
            assert len(parts) >= 3
            assert result.assembly_graph["root_part_id"] in {part["part_id"] for part in parts}
            assert len(result.assembly_graph["connections"]) == len(parts) - 1
            if pack.pack_id == "pack_robotic_arm_concept":
                assert sum(len(part["joints"]) for part in parts) == len(parts) - 1
            signature = _structure_signature(result)
            assert signature not in signatures, f"duplicate structure in {pack.pack_id}: {variant_id}"
            assert signature not in all_signatures, f"duplicate structure across packs: {variant_id}"
            signatures.add(signature)
            all_signatures.add(signature)
            total += 1
        assert len(signatures) == 12
    assert total == 48 and len(all_signatures) == 48
    print("G807 blockout diversity smoke passed: 48 deterministic structures across 4 domain packs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
