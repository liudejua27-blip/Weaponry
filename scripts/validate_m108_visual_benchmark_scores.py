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
from dataclasses import asdict
from pathlib import Path
from typing import Any, Mapping

from forgecad_agent.application.geometry_worker import read_shape_program_glb_facts
from forgecad_agent.application.visual_texture_sets import geometry_artifact_profile_manifest
from prepare_m108_visual_benchmark import DOMAINS, build_kit


DIMENSIONS = ("proportion", "material_readability", "surface_detail")
PBR_TEXTURE_ROLES = {
    "base_color",
    "metallic_roughness",
    "normal",
    "occlusion",
    "emissive",
}
REQUIRED_VIEWPORT = {
    "load_state": "ready",
    "render_source": "glb_pbr",
}
PRODUCTION_TEXTURE_SIZE = int(
    geometry_artifact_profile_manifest("production_concept")["texture_width"]
)


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


def _validate_current_review_texture_sets(texture_sets: object, *, fixture_id: str) -> None:
    if not isinstance(texture_sets, list) or len(texture_sets) < 5:
        raise BenchmarkValidationError(f"{fixture_id} 必须包含至少 5 套生产级 v4 视觉材质 readback")
    material_indices: set[int] = set()
    texture_set_ids: set[str] = set()
    texture_material_ids: set[str] = set()
    for texture_set_index, texture_set in enumerate(texture_sets):
        field = f"{fixture_id}.visual_texture_sets[{texture_set_index}]"
        if not isinstance(texture_set, Mapping):
            raise BenchmarkValidationError(f"{field} 必须是对象")
        material_index = texture_set.get("material_index")
        if isinstance(material_index, bool) or not isinstance(material_index, int) or material_index < 0:
            raise BenchmarkValidationError(f"{field}.material_index 必须是非负整数")
        texture_set_id = _require_string(texture_set.get("visual_texture_set_id"), f"{field}.visual_texture_set_id")
        texture_material_id = _require_string(texture_set.get("texture_material_id"), f"{field}.texture_material_id")
        if not texture_set_id.endswith("_builtin_v4"):
            raise BenchmarkValidationError(f"{field} 不是当前 production builtin v4 视觉材质")
        material_indices.add(material_index)
        texture_set_ids.add(texture_set_id)
        texture_material_ids.add(texture_material_id)
        maps = texture_set.get("maps")
        if not isinstance(maps, list) or len(maps) != len(PBR_TEXTURE_ROLES):
            raise BenchmarkValidationError(f"{field} 必须包含完整五通道 PBR map readback")
        map_roles: set[str] = set()
        for map_index, texture_map in enumerate(maps):
            map_field = f"{field}.maps[{map_index}]"
            if not isinstance(texture_map, Mapping):
                raise BenchmarkValidationError(f"{map_field} 必须是对象")
            texture_id = _require_string(texture_map.get("texture_id"), f"{map_field}.texture_id")
            texture_role = _require_string(texture_map.get("texture_role"), f"{map_field}.texture_role")
            if "_v4_" not in texture_id:
                raise BenchmarkValidationError(f"{map_field} 不是当前 production v4 纹理 readback")
            if (
                texture_map.get("width") != PRODUCTION_TEXTURE_SIZE
                or texture_map.get("height") != PRODUCTION_TEXTURE_SIZE
            ):
                raise BenchmarkValidationError(
                    f"{map_field} 必须是当前 {PRODUCTION_TEXTURE_SIZE}x{PRODUCTION_TEXTURE_SIZE} 生产级嵌入纹理"
                )
            map_roles.add(texture_role)
        if map_roles != PBR_TEXTURE_ROLES:
            raise BenchmarkValidationError(f"{field} 的五通道 PBR map role 不完整或重复")
    if min(len(material_indices), len(texture_set_ids), len(texture_material_ids)) < 5:
        raise BenchmarkValidationError(
            f"{fixture_id} 必须回读至少 5 个不同 material index、texture-set ID 和规范纹理材质"
        )


def _validate_fixture_payload(*, kit_dir: Path, fixture: Mapping[str, Any], fixture_id: str) -> tuple[str, str]:
    relative_file = _require_string(fixture.get("file"), f"{fixture_id}.file")
    relative_path = Path(relative_file)
    if (
        "\\" in relative_file
        or relative_path.is_absolute()
        or len(relative_path.parts) != 2
        or relative_path.parts[0] != "fixtures"
        or relative_path.suffix.lower() != ".glb"
    ):
        raise BenchmarkValidationError(f"{fixture_id} 必须引用审阅包 fixtures/ 下的安全相对 GLB 路径")
    kit_root = kit_dir.resolve()
    fixture_path = (kit_root / relative_path).resolve()
    try:
        fixture_path.relative_to(kit_root)
    except ValueError as error:
        raise BenchmarkValidationError(f"{fixture_id} 的 GLB 路径越出审阅包") from error
    if not fixture_path.is_file():
        raise BenchmarkValidationError(f"{fixture_id} 的 GLB 文件不存在或不是普通文件")
    try:
        actual_size = fixture_path.stat().st_size
        expected_size = fixture.get("glb_byte_size")
        if isinstance(expected_size, bool) or not isinstance(expected_size, int) or expected_size != actual_size:
            raise BenchmarkValidationError(f"{fixture_id} 的 GLB 字节数与 manifest 不一致")
        if actual_size > 32 * 1024 * 1024:
            raise BenchmarkValidationError(f"{fixture_id} 超出 32 MB 视觉基准 GLB 上限")
        payload = fixture_path.read_bytes()
    except OSError as error:
        raise BenchmarkValidationError(f"无法读取 {fixture_id} 的 GLB 文件") from error
    if len(payload) != actual_size:
        raise BenchmarkValidationError(f"{fixture_id} 的 GLB 字节数与 manifest 不一致")
    expected_sha256 = _require_string(fixture.get("glb_sha256"), f"{fixture_id}.glb_sha256").lower()
    if hashlib.sha256(payload).hexdigest() != expected_sha256:
        raise BenchmarkValidationError(f"{fixture_id} 的 GLB SHA-256 与 manifest 不一致")
    try:
        readback = read_shape_program_glb_facts(payload)
    except Exception as error:
        raise BenchmarkValidationError(f"{fixture_id} 不是可回读的 ForgeCAD GLB") from error
    _validate_current_review_texture_sets(readback.visual_texture_sets, fixture_id=fixture_id)
    if readback.artifact_profile.get("artifact_profile_id") != "production_concept":
        raise BenchmarkValidationError(f"{fixture_id} 不是生产级概念工件")
    readback_facts = {
        "triangle_count": readback.triangle_count,
        "material_zone_count": len(readback.material_zone_faces),
        "visual_texture_set_count": len(readback.visual_texture_sets),
        "artifact_profile": readback.artifact_profile,
        "visual_environment": asdict(readback)["visual_environment"],
    }
    for field, value in readback_facts.items():
        if fixture.get(field) != value:
            raise BenchmarkValidationError(f"{fixture_id} 的 {field} 与真实 GLB readback 不一致")
    if readback_facts["material_zone_count"] < 3:
        raise BenchmarkValidationError(f"{fixture_id} 缺少多 zone 或完整嵌入 PBR readback 证据")
    return relative_path.as_posix(), expected_sha256


def validate_scores(*, kit_dir: Path, response_path: Path) -> dict[str, object]:
    manifest_path = kit_dir / "manifest.json"
    manifest = _read_json(manifest_path)
    if manifest.get("schema_version") != "M108VisualBenchmarkKit@1":
        raise BenchmarkValidationError("审阅包 schema 不受支持")
    fixtures = manifest.get("fixtures")
    if not isinstance(fixtures, list) or len(fixtures) != 4:
        raise BenchmarkValidationError("审阅包必须包含恰好四个领域 fixture")
    fixture_domains: dict[str, str] = {}
    fixture_files: set[str] = set()
    fixture_hashes: set[str] = set()
    for fixture_index, fixture in enumerate(fixtures):
        if not isinstance(fixture, dict):
            raise BenchmarkValidationError(f"manifest.fixtures[{fixture_index}] 必须是对象")
        fixture_id = _require_string(fixture.get("fixture_id"), "manifest.fixtures[].fixture_id")
        domain_pack_id = _require_string(fixture.get("domain_pack_id"), "manifest.fixtures[].domain_pack_id")
        if fixture_id in fixture_domains:
            raise BenchmarkValidationError("审阅包 fixture ID 不完整或重复")
        if domain_pack_id in fixture_domains.values():
            raise BenchmarkValidationError("审阅包必须为每个领域提供唯一 fixture")
        fixture_id_parts = fixture_id.split(":", 1)
        if len(fixture_id_parts) != 2 or fixture_id_parts[0] != domain_pack_id:
            raise BenchmarkValidationError(f"{fixture_id} 的 fixture ID 与 domain_pack_id 不一致")
        fixture_file, fixture_hash = _validate_fixture_payload(kit_dir=kit_dir, fixture=fixture, fixture_id=fixture_id)
        if fixture_file in fixture_files or fixture_hash in fixture_hashes:
            raise BenchmarkValidationError("四个领域 fixture 必须引用不同的 GLB 路径和内容哈希")
        fixture_files.add(fixture_file)
        fixture_hashes.add(fixture_hash)
        fixture_domains[fixture_id] = domain_pack_id
    if set(fixture_domains.values()) != set(DOMAINS):
        raise BenchmarkValidationError("审阅包领域集合与当前四领域基准不一致")
    fixture_ids = set(fixture_domains)

    responses_document = _read_json(response_path)
    if responses_document.get("schema_version") != "M108VisualBenchmarkResponses@1":
        raise BenchmarkValidationError("评分工件 schema 不受支持")
    manifest_sha256 = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
    if responses_document.get("kit_manifest_sha256") != manifest_sha256:
        raise BenchmarkValidationError("评分工件不属于当前审阅包")
    responses = responses_document.get("responses")
    if not isinstance(responses, list) or len(responses) < 3:
        raise BenchmarkValidationError("至少需要三个不同评审 ID")

    reviewer_ids: set[str] = set()
    scores: dict[str, list[int]] = {dimension: [] for dimension in DIMENSIONS}
    scores_by_fixture: dict[str, dict[str, list[int]]] = {
        fixture_id: {dimension: [] for dimension in DIMENSIONS}
        for fixture_id in fixture_ids
    }
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
            review_scores = fixture_review.get("scores")
            if not isinstance(review_scores, dict) or set(review_scores) != set(DIMENSIONS):
                raise BenchmarkValidationError(f"{reviewer_id} 的评分维度必须完整且不能扩展")
            for dimension in DIMENSIONS:
                score = _require_score(review_scores[dimension], f"{reviewer_id}.{fixture_id}.{dimension}")
                scores[dimension].append(score)
                scores_by_fixture[fixture_id][dimension].append(score)
        if reviewed_ids != fixture_ids:
            raise BenchmarkValidationError(f"评审者 {reviewer_id} 没有覆盖全部 fixture")

    medians = {dimension: statistics.median(values) for dimension, values in scores.items()}
    domain_dimension_medians = {
        fixture_domains[fixture_id]: {
            dimension: statistics.median(values)
            for dimension, values in scores_by_fixture[fixture_id].items()
        }
        for fixture_id in sorted(fixture_ids)
    }
    failed_domains = {
        domain_pack_id: {
            dimension: median
            for dimension, median in dimension_medians.items()
            if median < 4
        }
        for domain_pack_id, dimension_medians in domain_dimension_medians.items()
        if any(median < 4 for median in dimension_medians.values())
    }
    if failed_domains:
        raise BenchmarkValidationError(f"领域视觉基准中位数未达标：{failed_domains}")
    return {
        "schema_version": "M108VisualBenchmarkResult@1",
        "kit_manifest_sha256": manifest_sha256,
        "reviewer_count": len(reviewer_ids),
        "fixture_count": len(fixture_ids),
        "review_count": len(reviewer_ids) * len(fixture_ids),
        "dimension_medians": medians,
        "domain_dimension_medians": domain_dimension_medians,
        "status": "passed",
    }


def _self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad_m108_score_validator_") as directory:
        kit_dir = Path(directory) / "kit"
        build_kit(kit_dir)
        manifest_sha256 = hashlib.sha256((kit_dir / "manifest.json").read_bytes()).hexdigest()
        manifest = _read_json(kit_dir / "manifest.json")
        original_manifest_text = (kit_dir / "manifest.json").read_text(encoding="utf-8")
        fixture_ids = [item["fixture_id"] for item in manifest["fixtures"]]
        fixture_path = kit_dir / manifest["fixtures"][0]["file"]
        current_texture_sets = read_shape_program_glb_facts(fixture_path.read_bytes()).visual_texture_sets
        _validate_current_review_texture_sets(current_texture_sets, fixture_id=fixture_ids[0])
        invalid_texture_sets = json.loads(json.dumps(current_texture_sets))
        invalid_texture_sets[0]["visual_texture_set_id"] = invalid_texture_sets[0]["visual_texture_set_id"].replace(
            "_builtin_v4", "_builtin_v2"
        )
        try:
            _validate_current_review_texture_sets(invalid_texture_sets, fixture_id=fixture_ids[0])
        except BenchmarkValidationError as error:
            if "builtin v4" not in str(error):
                raise
        else:
            raise AssertionError("historical v2 visual texture truth was accepted for M108 review")
        invalid_texture_sets = json.loads(json.dumps(current_texture_sets))
        invalid_texture_sets[0]["maps"][0]["texture_id"] = invalid_texture_sets[0]["maps"][0]["texture_id"].replace(
            "_v4_", "_v2_"
        )
        try:
            _validate_current_review_texture_sets(invalid_texture_sets, fixture_id=fixture_ids[0])
        except BenchmarkValidationError as error:
            if "v4 纹理" not in str(error):
                raise
        else:
            raise AssertionError("historical v2 texture map truth was accepted for M108 review")
        invalid_texture_sets = json.loads(json.dumps(current_texture_sets))
        invalid_texture_sets[0]["maps"][0]["width"] = 64
        try:
            _validate_current_review_texture_sets(invalid_texture_sets, fixture_id=fixture_ids[0])
        except BenchmarkValidationError as error:
            if f"{PRODUCTION_TEXTURE_SIZE}x{PRODUCTION_TEXTURE_SIZE}" not in str(error):
                raise
        else:
            raise AssertionError("non-current embedded texture dimensions were accepted for M108 review")
        try:
            _validate_current_review_texture_sets(current_texture_sets[:4], fixture_id=fixture_ids[0])
        except BenchmarkValidationError as error:
            if "至少 5 套" not in str(error):
                raise
        else:
            raise AssertionError("fewer than five visual texture sets were accepted for M108 review")
        alias_duplicate_sets = [
            json.loads(json.dumps(current_texture_sets[index % 2]))
            for index in range(5)
        ]
        try:
            _validate_current_review_texture_sets(alias_duplicate_sets, fixture_id=fixture_ids[0])
        except BenchmarkValidationError as error:
            if "至少 5 个不同" not in str(error):
                raise
        else:
            raise AssertionError("authored aliases were counted as five distinct review materials")
        invalid_texture_sets = json.loads(json.dumps(current_texture_sets))
        invalid_texture_sets[0]["maps"][1]["texture_role"] = invalid_texture_sets[0]["maps"][0]["texture_role"]
        try:
            _validate_current_review_texture_sets(invalid_texture_sets, fixture_id=fixture_ids[0])
        except BenchmarkValidationError as error:
            if "不完整或重复" not in str(error):
                raise
        else:
            raise AssertionError("an incomplete or duplicate five-channel role set was accepted for M108 review")
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
        responses["responses"][0]["independent_of_implementation"] = True
        low_fixture_id = fixture_ids[0]
        for response in responses["responses"]:
            for fixture_review in response["fixture_reviews"]:
                if fixture_review["fixture_id"] == low_fixture_id:
                    fixture_review["scores"] = {dimension: 1 for dimension in DIMENSIONS}
        response_path.write_text(json.dumps(responses), encoding="utf-8")
        try:
            validate_scores(kit_dir=kit_dir, response_path=response_path)
        except BenchmarkValidationError as error:
            if "领域视觉基准中位数未达标" not in str(error):
                raise
        else:
            raise AssertionError("a failing domain was hidden by the cross-domain aggregate median")
        for response in responses["responses"]:
            for fixture_review in response["fixture_reviews"]:
                fixture_review["scores"] = {dimension: 4 for dimension in DIMENSIONS}
        duplicate_manifest = json.loads(original_manifest_text)
        duplicate_source = duplicate_manifest["fixtures"][0]
        duplicate_target = duplicate_manifest["fixtures"][1]
        for field in (
            "file",
            "glb_sha256",
            "glb_byte_size",
            "triangle_count",
            "material_zone_count",
            "visual_texture_set_count",
            "visual_environment",
        ):
            duplicate_target[field] = duplicate_source[field]
        (kit_dir / "manifest.json").write_text(json.dumps(duplicate_manifest), encoding="utf-8")
        responses["kit_manifest_sha256"] = hashlib.sha256((kit_dir / "manifest.json").read_bytes()).hexdigest()
        response_path.write_text(json.dumps(responses), encoding="utf-8")
        try:
            validate_scores(kit_dir=kit_dir, response_path=response_path)
        except BenchmarkValidationError as error:
            if "不同的 GLB 路径和内容哈希" not in str(error):
                raise
        else:
            raise AssertionError("four benchmark domains were allowed to reuse one GLB")
        mismatch_manifest = json.loads(original_manifest_text)
        mismatch_manifest["fixtures"][0]["domain_pack_id"], mismatch_manifest["fixtures"][1]["domain_pack_id"] = (
            mismatch_manifest["fixtures"][1]["domain_pack_id"],
            mismatch_manifest["fixtures"][0]["domain_pack_id"],
        )
        (kit_dir / "manifest.json").write_text(json.dumps(mismatch_manifest), encoding="utf-8")
        responses["kit_manifest_sha256"] = hashlib.sha256((kit_dir / "manifest.json").read_bytes()).hexdigest()
        response_path.write_text(json.dumps(responses), encoding="utf-8")
        try:
            validate_scores(kit_dir=kit_dir, response_path=response_path)
        except BenchmarkValidationError as error:
            if "fixture ID 与 domain_pack_id 不一致" not in str(error):
                raise
        else:
            raise AssertionError("a benchmark fixture was allowed to claim another domain")
        (kit_dir / "manifest.json").write_text(original_manifest_text, encoding="utf-8")
        responses["kit_manifest_sha256"] = manifest_sha256
        response_path.write_text(json.dumps(responses), encoding="utf-8")
        tampered_fixture_path = kit_dir / manifest["fixtures"][0]["file"]
        tampered_fixture_path.write_bytes(b"not a GLB")
        try:
            validate_scores(kit_dir=kit_dir, response_path=response_path)
        except BenchmarkValidationError as error:
            if "GLB 字节数" not in str(error) and "GLB SHA-256" not in str(error):
                raise
        else:
            raise AssertionError("tampered benchmark GLB was accepted against an unchanged manifest")


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
        print("M108 visual score validator smoke passed: four unique production-concept domain GLBs, current v4 five-channel PBR readback, reviewer declarations and per-domain median thresholds are enforced")
        return 0
    if not args.kit or not args.responses:
        parser.error("--kit and --responses are required unless --self-test is used")
    print(json.dumps(validate_scores(kit_dir=args.kit.resolve(), response_path=args.responses.resolve()), ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
