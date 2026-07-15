#!/usr/bin/env python3
"""Validate independent human scores for the FGC-M108 visual benchmark.

This validator never creates review scores.  It only accepts declared reviews
that were made in the same-GLB PBR workbench state defined by the M108 protocol.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import tempfile
from pathlib import Path
from typing import Any, Mapping

from prepare_m108_visual_benchmark import build_kit


DIMENSIONS = ("proportion", "material_readability", "surface_detail")
REQUIRED_VIEWPORT = {
    "load_state": "ready",
    "render_source": "glb_pbr",
}


class BenchmarkValidationError(ValueError):
    """The supplied human benchmark evidence cannot prove an M108 pass."""


def _read_json(path: Path) -> Mapping[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise BenchmarkValidationError(f"无法读取 JSON 工件：{path}") from error
    if not isinstance(data, dict):
        raise BenchmarkValidationError(f"JSON 工件必须是对象：{path}")
    return data


def _require_string(value: object, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise BenchmarkValidationError(f"{field} 必须是非空字符串")
    return value.strip()


def _require_score(value: object, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or not 1 <= value <= 5:
        raise BenchmarkValidationError(f"{field} 必须是 1–5 的整数")
    return value


def validate_scores(*, kit_dir: Path, response_path: Path) -> dict[str, object]:
    manifest_path = kit_dir / "manifest.json"
    manifest = _read_json(manifest_path)
    if manifest.get("schema_version") != "M108VisualBenchmarkKit@1":
        raise BenchmarkValidationError("审阅包 schema 不受支持")
    fixtures = manifest.get("fixtures")
    if not isinstance(fixtures, list) or len(fixtures) != 4:
        raise BenchmarkValidationError("审阅包必须包含恰好四个领域 fixture")
    fixture_ids = {
        _require_string(item.get("fixture_id"), "manifest.fixtures[].fixture_id")
        for item in fixtures
        if isinstance(item, dict)
    }
    if len(fixture_ids) != 4:
        raise BenchmarkValidationError("审阅包 fixture ID 不完整或重复")

    responses_document = _read_json(response_path)
    if responses_document.get("schema_version") != "M108VisualBenchmarkResponses@1":
        raise BenchmarkValidationError("评分工件 schema 不受支持")
    manifest_sha256 = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
    if responses_document.get("kit_manifest_sha256") != manifest_sha256:
        raise BenchmarkValidationError("评分工件不属于当前审阅包")
    responses = responses_document.get("responses")
    if not isinstance(responses, list) or len(responses) < 3:
        raise BenchmarkValidationError("至少需要三位独立评审者")

    reviewer_ids: set[str] = set()
    scores: dict[str, list[int]] = {dimension: [] for dimension in DIMENSIONS}
    for response_index, response in enumerate(responses):
        if not isinstance(response, dict):
            raise BenchmarkValidationError(f"responses[{response_index}] 必须是对象")
        reviewer_id = _require_string(response.get("reviewer_id"), f"responses[{response_index}].reviewer_id")
        if reviewer_id in reviewer_ids:
            raise BenchmarkValidationError("同一评审者不能重复提交")
        reviewer_ids.add(reviewer_id)
        if response.get("independent_of_implementation") is not True:
            raise BenchmarkValidationError(f"评审者 {reviewer_id} 未声明与实现独立")
        fixture_reviews = response.get("fixture_reviews")
        if not isinstance(fixture_reviews, list) or len(fixture_reviews) != len(fixture_ids):
            raise BenchmarkValidationError(f"评审者 {reviewer_id} 必须评完四个领域")
        reviewed_ids: set[str] = set()
        for fixture_index, fixture_review in enumerate(fixture_reviews):
            if not isinstance(fixture_review, dict):
                raise BenchmarkValidationError(f"{reviewer_id}.fixture_reviews[{fixture_index}] 必须是对象")
            fixture_id = _require_string(fixture_review.get("fixture_id"), "fixture_id")
            if fixture_id not in fixture_ids or fixture_id in reviewed_ids:
                raise BenchmarkValidationError(f"评审者 {reviewer_id} 的 fixture 覆盖不合法")
            reviewed_ids.add(fixture_id)
            if fixture_review.get("pbr_load_failure") is not False:
                raise BenchmarkValidationError(f"{reviewer_id} 报告了同源 PBR 加载失败")
            viewport = fixture_review.get("viewport")
            if not isinstance(viewport, dict):
                raise BenchmarkValidationError(f"{reviewer_id} 未记录 PBR 视口状态")
            if any(viewport.get(key) != value for key, value in REQUIRED_VIEWPORT.items()):
                raise BenchmarkValidationError(f"{reviewer_id} 未在同源 GLB PBR 视口中评分")
            material_count = viewport.get("embedded_pbr_material_count")
            if isinstance(material_count, bool) or not isinstance(material_count, int) or material_count < 1:
                raise BenchmarkValidationError(f"{reviewer_id} 未确认实际嵌入 PBR 材质")
            fixture_scores = fixture_review.get("scores")
            if not isinstance(fixture_scores, dict) or set(fixture_scores) != set(DIMENSIONS):
                raise BenchmarkValidationError(f"{reviewer_id} 的评分维度必须完整且不能扩展")
            for dimension in DIMENSIONS:
                scores[dimension].append(_require_score(fixture_scores[dimension], f"{reviewer_id}.{fixture_id}.{dimension}"))
        if reviewed_ids != fixture_ids:
            raise BenchmarkValidationError(f"评审者 {reviewer_id} 没有覆盖全部 fixture")

    medians = {dimension: statistics.median(values) for dimension, values in scores.items()}
    failed_dimensions = {dimension: median for dimension, median in medians.items() if median < 4}
    if failed_dimensions:
        raise BenchmarkValidationError(f"视觉基准中位数未达标：{failed_dimensions}")
    return {
        "schema_version": "M108VisualBenchmarkResult@1",
        "kit_manifest_sha256": manifest_sha256,
        "reviewer_count": len(reviewer_ids),
        "fixture_count": len(fixture_ids),
        "review_count": len(reviewer_ids) * len(fixture_ids),
        "dimension_medians": medians,
        "status": "passed",
    }


def _self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad_m108_score_validator_") as directory:
        kit_dir = Path(directory) / "kit"
        build_kit(kit_dir)
        manifest_sha256 = hashlib.sha256((kit_dir / "manifest.json").read_bytes()).hexdigest()
        fixture_ids = [item["fixture_id"] for item in _read_json(kit_dir / "manifest.json")["fixtures"]]
        responses = {
            "schema_version": "M108VisualBenchmarkResponses@1",
            "kit_manifest_sha256": manifest_sha256,
            # Synthetic contract fixtures only: they never become a human-review artifact.
            "responses": [
                {
                    "reviewer_id": f"smoke_reviewer_{index}",
                    "independent_of_implementation": True,
                    "fixture_reviews": [
                        {
                            "fixture_id": fixture_id,
                            "pbr_load_failure": False,
                            "viewport": {**REQUIRED_VIEWPORT, "embedded_pbr_material_count": 3},
                            "scores": {dimension: 4 for dimension in DIMENSIONS},
                        }
                        for fixture_id in fixture_ids
                    ],
                }
                for index in range(1, 4)
            ],
        }
        response_path = Path(directory) / "responses.json"
        response_path.write_text(json.dumps(responses), encoding="utf-8")
        result = validate_scores(kit_dir=kit_dir, response_path=response_path)
        if result["status"] != "passed" or result["dimension_medians"] != {dimension: 4 for dimension in DIMENSIONS}:
            raise AssertionError("synthetic passing contract fixture was not validated")
        responses["responses"][0]["independent_of_implementation"] = False
        response_path.write_text(json.dumps(responses), encoding="utf-8")
        try:
            validate_scores(kit_dir=kit_dir, response_path=response_path)
        except BenchmarkValidationError as error:
            if "未声明与实现独立" not in str(error):
                raise
        else:
            raise AssertionError("non-independent synthetic review was accepted")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--kit", type=Path, help="M108 review-kit directory containing manifest.json")
    parser.add_argument("--responses", type=Path, help="declared human-review response JSON")
    parser.add_argument("--self-test", action="store_true", help="validate synthetic contract fixtures without creating human evidence")
    args = parser.parse_args()
    if args.self_test:
        if args.kit or args.responses:
            parser.error("--self-test cannot be combined with --kit or --responses")
        _self_test()
        print("M108 visual score validator smoke passed: independent reviewers, four fixtures, PBR state and median threshold are enforced")
        return 0
    if not args.kit or not args.responses:
        parser.error("--kit and --responses are required unless --self-test is used")
    print(json.dumps(validate_scores(kit_dir=args.kit.resolve(), response_path=args.responses.resolve()), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
