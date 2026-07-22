#!/usr/bin/env python3
"""Fail-closed validator for declared M108B formal human review evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import statistics
import tempfile
from pathlib import Path
from typing import Any, Mapping

from prepare_m108b_formal_visual_benchmark import (
    DIMENSIONS,
    DOMAINS,
    FormalKitBlockedError,
    _verify_payload,
    validate_source_manifest,
)


class FormalScoreValidationError(ValueError):
    pass


def _read(path: Path) -> Mapping[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise FormalScoreValidationError(f"无法读取正式评分 JSON：{path}") from error
    if not isinstance(value, Mapping):
        raise FormalScoreValidationError("正式评分 JSON 必须是对象")
    return value


def _sha(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _text(value: object, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise FormalScoreValidationError(f"{field} 必须是非空字符串")
    return value.strip()


def _score(value: object, field: str) -> int:
    if type(value) is not int or not 1 <= value <= 5:
        raise FormalScoreValidationError(f"{field} 必须是 1–5 整数")
    return value


def _validate_kit(kit_dir: Path) -> tuple[Mapping[str, Any], dict[str, str]]:
    manifest_path = kit_dir / "manifest.json"
    manifest = _read(manifest_path)
    if manifest.get("schema_version") != "M108BFormalVisualBenchmarkKit@1" or manifest.get("formal_m108b") is not True:
        raise FormalScoreValidationError("只接受 M108BFormalVisualBenchmarkKit@1；旧 M108A/Python showcase kit 一律拒绝")
    if manifest.get("fixture_origin") != "recipe_backed_production" or manifest.get("selection_status") != "frozen_before_scoring" or manifest.get("score_status") != "not_scored":
        raise FormalScoreValidationError("正式 kit 必须为 recipe-backed、评分前冻结、且未被评分结果污染")
    source_file = manifest.get("source_manifest_file")
    if source_file != "frozen-source-manifest.json":
        raise FormalScoreValidationError("正式 kit 缺少冻结 source manifest")
    source_path = kit_dir / source_file
    if not source_path.is_file() or manifest.get("source_manifest_sha256") != _sha(source_path.read_bytes()):
        raise FormalScoreValidationError("冻结 source manifest 已篡改或 hash 漂移")
    try:
        source = _read(source_path)
        source_fixtures = validate_source_manifest(source)
    except (FormalKitBlockedError, FormalScoreValidationError) as error:
        raise FormalScoreValidationError(f"冻结 source manifest 不再满足 M108B 合同：{error}") from error
    fixtures = manifest.get("fixtures")
    if not isinstance(fixtures, list) or len(fixtures) != 12:
        raise FormalScoreValidationError("正式 kit 必须恰好包含 12 个 fixture")
    ids: dict[str, str] = {}
    hashes: set[str] = set()
    counts = {domain: 0 for domain in DOMAINS}
    source_by_id = {str(fixture["fixture_id"]): fixture for fixture in source_fixtures}
    for fixture in fixtures:
        if not isinstance(fixture, Mapping):
            raise FormalScoreValidationError("fixture 必须是对象")
        fixture_id = _text(fixture.get("fixture_id"), "fixture_id")
        domain = _text(fixture.get("domain_pack_id"), f"{fixture_id}.domain_pack_id")
        glb_hash = _text(fixture.get("glb_sha256"), f"{fixture_id}.glb_sha256")
        file = _text(fixture.get("file"), f"{fixture_id}.file")
        if fixture_id in ids or glb_hash in hashes or domain not in counts or "showcase" in fixture_id.lower():
            raise FormalScoreValidationError("正式 fixture 必须唯一、四领域合法，且不能是 showcase shortcut")
        if not (kit_dir / file).is_file() or _sha((kit_dir / file).read_bytes()) != glb_hash:
            raise FormalScoreValidationError(f"{fixture_id} GLB 已缺失、篡改或 hash 漂移")
        source_fixture = source_by_id.get(fixture_id)
        if source_fixture is None:
            raise FormalScoreValidationError(f"{fixture_id} 不属于冻结 source manifest")
        formal_facts = dict(fixture)
        formal_facts.pop("file", None)
        source_facts = dict(source_fixture)
        source_facts.pop("source_glb", None)
        if formal_facts != source_facts:
            raise FormalScoreValidationError(f"{fixture_id} formal manifest 与冻结 source facts 不一致")
        readback_fixture = dict(source_fixture)
        readback_fixture["source_glb"] = file
        try:
            _verify_payload(kit_dir, readback_fixture)
        except FormalKitBlockedError as error:
            raise FormalScoreValidationError(f"{fixture_id} production GLB/readback/M108A/Q003/G826/PBR 合同失败：{error}") from error
        ids[fixture_id] = domain
        hashes.add(glb_hash)
        counts[domain] += 1
    if any(count != 3 for count in counts.values()):
        raise FormalScoreValidationError("正式 kit 必须四领域各 3 个 fixture")
    return manifest, ids


def validate_scores(*, kit_dir: Path, response_path: Path) -> dict[str, object]:
    manifest, fixture_domains = _validate_kit(kit_dir)
    response_document = _read(response_path)
    if response_document.get("schema_version") != "M108BFormalVisualBenchmarkResponses@1":
        raise FormalScoreValidationError("正式评分必须使用 M108BFormalVisualBenchmarkResponses@1")
    if response_document.get("kit_manifest_sha256") != _sha((kit_dir / "manifest.json").read_bytes()):
        raise FormalScoreValidationError("评分不属于当前冻结 formal kit，可能是 stale/评分后选择")
    responses = response_document.get("responses")
    if not isinstance(responses, list) or len(responses) < 3:
        raise FormalScoreValidationError("至少需要三位不同的真人评审者")
    scores = {fixture_id: {dimension: [] for dimension in DIMENSIONS} for fixture_id in fixture_domains}
    reviewer_ids: set[str] = set()
    for response in responses:
        if not isinstance(response, Mapping):
            raise FormalScoreValidationError("response 必须是对象")
        reviewer_id = _text(response.get("reviewer_id"), "reviewer_id")
        if reviewer_id in reviewer_ids:
            raise FormalScoreValidationError("同一评审者不能重复提交")
        reviewer_ids.add(reviewer_id)
        if response.get("independent_of_implementation") is not True or response.get("reviewer_kind") != "human" or response.get("automated_or_agent_score") is not False:
            raise FormalScoreValidationError(f"{reviewer_id} 必须声明独立真人，代理或自动评分不能补齐")
        reviews = response.get("fixture_reviews")
        if not isinstance(reviews, list) or len(reviews) != 12:
            raise FormalScoreValidationError(f"{reviewer_id} 必须完整评完 12 个 fixture")
        reviewed: set[str] = set()
        for review in reviews:
            if not isinstance(review, Mapping):
                raise FormalScoreValidationError(f"{reviewer_id} 的 fixture review 必须是对象")
            fixture_id = _text(review.get("fixture_id"), "fixture_id")
            if fixture_id not in fixture_domains or fixture_id in reviewed:
                raise FormalScoreValidationError(f"{reviewer_id} fixture 覆盖不完整或重复")
            reviewed.add(fixture_id)
            if review.get("pbr_load_failure") is not False:
                raise FormalScoreValidationError(f"{reviewer_id} 报告 PBR 加载失败")
            viewport = review.get("viewport")
            if not isinstance(viewport, Mapping) or viewport.get("load_state") != "ready" or viewport.get("render_source") != "glb_pbr" or type(viewport.get("embedded_pbr_material_count")) is not int or viewport["embedded_pbr_material_count"] < 1:
                raise FormalScoreValidationError(f"{reviewer_id} 未在真实 production GLB PBR 视口评分")
            review_scores = review.get("scores")
            if not isinstance(review_scores, Mapping) or set(review_scores) != set(DIMENSIONS):
                raise FormalScoreValidationError(f"{reviewer_id} 必须提供且只能提供三项评分")
            for dimension in DIMENSIONS:
                scores[fixture_id][dimension].append(_score(review_scores[dimension], f"{reviewer_id}.{fixture_id}.{dimension}"))
        if reviewed != set(fixture_domains):
            raise FormalScoreValidationError(f"{reviewer_id} 没有完整覆盖 12 个 fixture")
    fixture_medians = {fixture_id: {dimension: statistics.median(values) for dimension, values in per_dimension.items()} for fixture_id, per_dimension in scores.items()}
    failed_fixtures = {fixture_id: values for fixture_id, values in fixture_medians.items() if any(value < 4 for value in values.values())}
    if failed_fixtures:
        raise FormalScoreValidationError(f"任一 fixture 三项中位数必须 >=4：{failed_fixtures}")
    domain_medians: dict[str, dict[str, float]] = {}
    for domain in DOMAINS:
        domain_medians[domain] = {dimension: statistics.median([score for fixture_id, per_dimension in scores.items() if fixture_domains[fixture_id] == domain for score in per_dimension[dimension]]) for dimension in DIMENSIONS}
    failed_domains = {domain: values for domain, values in domain_medians.items() if any(value < 4 for value in values.values())}
    if failed_domains:
        raise FormalScoreValidationError(f"任一领域三项聚合中位数必须 >=4：{failed_domains}")
    return {"schema_version": "M108BFormalVisualBenchmarkResult@1", "kit_manifest_sha256": response_document["kit_manifest_sha256"], "reviewer_count": len(reviewer_ids), "fixture_count": 12, "fixture_dimension_medians": fixture_medians, "domain_dimension_medians": domain_medians, "status": "passed"}


def _self_test() -> None:
    # No production GLB is bundled with this task.  Verify the formal-only gate
    # rejects the legacy preflight kit before it can see any alleged scores.
    with tempfile.TemporaryDirectory(prefix="forgecad_m108b_score_") as raw:
        kit = Path(raw) / "kit"
        kit.mkdir()
        (kit / "manifest.json").write_text(json.dumps({"schema_version": "M108VisualBenchmarkKit@1", "fixtures": []}), encoding="utf-8")
        response = Path(raw) / "responses.json"
        response.write_text(json.dumps({"schema_version": "M108BFormalVisualBenchmarkResponses@1", "responses": []}), encoding="utf-8")
        try:
            validate_scores(kit_dir=kit, response_path=response)
        except FormalScoreValidationError as error:
            if "M108BFormalVisualBenchmarkKit" not in str(error):
                raise
        else:
            raise AssertionError("legacy showcase kit was accepted as formal score evidence")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--kit", type=Path)
    parser.add_argument("--responses", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        if args.kit or args.responses:
            parser.error("--self-test cannot be combined with --kit/--responses")
        _self_test()
        print("M108B formal score validator self-test passed: legacy/preflight evidence fails closed")
        return 0
    if not args.kit or not args.responses:
        parser.error("--kit and --responses are required")
    print(json.dumps(validate_scores(kit_dir=args.kit.resolve(), response_path=args.responses.resolve()), ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
