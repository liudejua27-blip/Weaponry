#!/usr/bin/env python3
"""Create or verify the reproducible, score-free FGC-M108 visual review kit.

The kit intentionally contains only deterministic ForgeCAD showcase GLBs and
their readback facts.  Review scores are collected separately by independent
humans; this tool must never manufacture a passing benchmark result.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import tempfile
from dataclasses import asdict
from pathlib import Path

from forgecad_agent.application.geometry_worker import read_shape_program_glb_facts
from smoke_m108_visual_pbr import _build_showcase_assets


ROOT = Path(__file__).resolve().parents[1]
DOMAINS = {
    "pack_future_weapon_prop": "未来概念道具",
    "pack_vehicle_concept": "汽车与地面载具",
    "pack_aircraft_concept": "飞机与航空器",
    "pack_robotic_arm_concept": "展示型机械臂",
}


def build_kit(output: Path) -> dict[str, object]:
    if output.exists():
        if any(output.iterdir()):
            raise ValueError(f"输出目录必须为空：{output}")
    else:
        output.mkdir(parents=True)
    fixtures_dir = output / "fixtures"
    fixtures_dir.mkdir()

    records: list[dict[str, object]] = []
    # `_build_showcase_assets` is the M108 compile/readback fixture source.
    # Choose its deterministic `a` showcase for each of the four packs rather
    # than creating a second visual-asset generation path for review.
    for fixture_id, glb_bytes in _build_showcase_assets()[::3]:
        pack_id, candidate_id = fixture_id.split(":", 1)
        if pack_id not in DOMAINS:
            raise ValueError(f"未知 M108 benchmark domain: {pack_id}")
        filename = f"{pack_id}.glb"
        (fixtures_dir / filename).write_bytes(glb_bytes)
        readback = read_shape_program_glb_facts(glb_bytes)
        records.append({
            "fixture_id": fixture_id,
            "domain_pack_id": pack_id,
            "domain_label": DOMAINS[pack_id],
            "candidate_id": candidate_id,
            "file": f"fixtures/{filename}",
            "glb_sha256": hashlib.sha256(glb_bytes).hexdigest(),
            "glb_byte_size": len(glb_bytes),
            "triangle_count": readback.triangle_count,
            "material_zone_count": len(readback.material_zone_faces),
            "visual_texture_set_count": len(readback.visual_texture_sets),
            "visual_environment": asdict(readback)["visual_environment"],
        })
    if len(records) != 4 or {str(item["domain_pack_id"]) for item in records} != set(DOMAINS):
        raise ValueError("M108 benchmark kit must contain exactly one fixture for every enabled domain")

    manifest: dict[str, object] = {
        "schema_version": "M108VisualBenchmarkKit@1",
        "purpose": "independent_visual_review_only",
        "score_status": "not_scored",
        "fixtures": records,
        "review_protocol": {
            "minimum_independent_reviewers": 3,
            "scores_per_fixture": ["proportion", "material_readability", "surface_detail"],
            "score_range": [1, 5],
            "passing_rule": "For every domain fixture, each score dimension has a reviewer median of at least 4; any embedded-PBR load failure invalidates the run.",
            "forbidden_evidence": [
                "self-authored score without independent reviewer declaration",
                "software concept PNG used instead of the GLB viewport",
                "parameter-only ShapeProgram fallback presented as embedded PBR",
            ],
        },
    }
    (output / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (output / "review-responses.json").write_text(json.dumps({
        "schema_version": "M108VisualBenchmarkResponses@1",
        "kit_manifest_sha256": hashlib.sha256((output / "manifest.json").read_bytes()).hexdigest(),
        "responses": [],
        "note": "Leave empty until independent reviewers submit declared scores. Do not synthesize scores.",
    }, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return manifest


def verify_kit() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad_m108_visual_benchmark_") as directory:
        output = Path(directory) / "kit"
        manifest = build_kit(output)
        decoded = json.loads((output / "manifest.json").read_text(encoding="utf-8"))
        if decoded != manifest:
            raise ValueError("benchmark manifest is not reproducible")
        if decoded["score_status"] != "not_scored" or decoded["review_protocol"]["minimum_independent_reviewers"] != 3:
            raise ValueError("benchmark kit must not imply uncollected independent scores")
        for fixture in decoded["fixtures"]:
            payload = (output / str(fixture["file"])).read_bytes()
            if hashlib.sha256(payload).hexdigest() != fixture["glb_sha256"]:
                raise ValueError("benchmark fixture hash mismatch")
            facts = read_shape_program_glb_facts(payload)
            if len(facts.visual_texture_sets) < 3 or len(facts.material_zone_faces) < 3:
                raise ValueError("benchmark fixture lacks the required multi-zone PBR evidence")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, help="empty directory receiving the four review GLBs and manifest")
    parser.add_argument("--verify", action="store_true", help="create a temporary kit and validate its deterministic evidence")
    args = parser.parse_args()
    if bool(args.output) == bool(args.verify):
        parser.error("choose exactly one of --output or --verify")
    if args.verify:
        verify_kit()
        print("M108 visual benchmark kit smoke passed: four same-GLB PBR fixtures, score-free independent review contract")
        return 0
    assert args.output is not None
    manifest = build_kit(args.output.resolve())
    print(json.dumps({"ok": True, "output": str(args.output.resolve()), "fixtures": len(manifest["fixtures"]), "score_status": "not_scored"}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
