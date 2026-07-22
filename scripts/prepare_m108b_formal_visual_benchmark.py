#!/usr/bin/env python3
"""Build the frozen, Recipe-backed FGC-M108B formal visual benchmark kit.

This is deliberately separate from ``prepare_m108_visual_benchmark.py``.  The
older script remains an M108A/preflight showcase generator and must never be
renamed or promoted into M108B evidence.  This builder consumes a future,
already-frozen source manifest; it does not create fixtures, choose winners, or
score anything.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import shutil
import tempfile
from dataclasses import asdict
from pathlib import Path
from typing import Any, Mapping

from forgecad_agent.application.geometry_worker import read_shape_program_glb_facts


DOMAINS = (
    "pack_future_weapon_prop",
    "pack_vehicle_concept",
    "pack_aircraft_concept",
    "pack_robotic_arm_concept",
)
DIMENSIONS = ("proportion", "material_readability", "surface_detail")
PBR_ROLES = {"base_color", "metallic_roughness", "normal", "occlusion", "emissive"}
RENDERER_LIMITS = {
    "geometry_count": 72,
    "texture_count": 48,
    "draw_calls": 96,
    "triangle_count": 24_000,
    "embedded_pbr_texture_count": 35,
    "texture_memory_bytes": 64 * 1024 * 1024,
}
FORMAL_MINIMUMS = {
    # These are admission floors, not a visual-quality score.  They prevent a
    # low-detail blockout from entering the human benchmark merely because it
    # stays under the lightweight renderer ceilings.  Human reviewers still
    # own proportion, material readability and surface-detail acceptance.
    "component_recipe_instances": 7,
    "material_zones": 5,
    "geometry_count": 7,
    "texture_count": 5,
    "draw_calls": 7,
    "triangle_count": 8_000,
    "embedded_pbr_texture_count": 5,
    "texture_memory_bytes": 1,
}
GATE_CONTRACTS = {
    "m108a": "FGC-M108A",
    "q003": "FGC-Q003",
    "g826": "FGC-G826",
}
GATE_EVIDENCE_SCHEMA = "M108BGateEvidence@1"
RENDERER_CAPTURE_SCHEMA = "M108BRendererCaptureEvidence@1"


class FormalKitBlockedError(ValueError):
    """A required M108B input does not exist; do not substitute showcase data."""


def _canonical_bytes(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")


def _sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def _read_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise FormalKitBlockedError(f"无法读取 M108B formal source manifest：{path}") from error
    if not isinstance(value, dict):
        raise FormalKitBlockedError("M108B formal source manifest 必须是 JSON 对象")
    return value


def _string(value: object, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise FormalKitBlockedError(f"{field} 必须是非空字符串")
    return value.strip()


def _hash(value: object, field: str) -> str:
    value = _string(value, field).lower()
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise FormalKitBlockedError(f"{field} 必须是 SHA-256")
    return value


def _safe_fixture_path(value: object, fixture_id: str) -> Path:
    raw = _string(value, f"{fixture_id}.source_glb")
    path = Path(raw)
    if "\\" in raw or path.is_absolute() or ".." in path.parts or path.suffix.lower() != ".glb":
        raise FormalKitBlockedError(f"{fixture_id}.source_glb 必须是 manifest 内的相对 GLB 路径")
    return path


def _require_mapping(value: object, field: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise FormalKitBlockedError(f"{field} 必须是对象")
    return value


def _require_list(value: object, field: str, *, min_items: int = 1) -> list[Any]:
    if not isinstance(value, list) or len(value) < min_items:
        raise FormalKitBlockedError(f"{field} 必须至少包含 {min_items} 项")
    return value


def _validate_recipe_fact(fixture: Mapping[str, Any], fixture_id: str) -> None:
    recipe = _require_mapping(fixture.get("recipe"), f"{fixture_id}.recipe")
    _string(recipe.get("recipe_id"), f"{fixture_id}.recipe.recipe_id")
    version = recipe.get("version")
    if type(version) is not int or version < 1:
        raise FormalKitBlockedError(f"{fixture_id}.recipe.version 必须是正整数")
    _hash(recipe.get("recipe_sha256"), f"{fixture_id}.recipe.recipe_sha256")
    registry = _require_mapping(fixture.get("registry"), f"{fixture_id}.registry")
    _hash(registry.get("registry_sha256"), f"{fixture_id}.registry.registry_sha256")
    _hash(registry.get("registry_lock_sha256"), f"{fixture_id}.registry.registry_lock_sha256")
    candidate = _require_mapping(fixture.get("candidate"), f"{fixture_id}.candidate")
    _string(candidate.get("candidate_id"), f"{fixture_id}.candidate.candidate_id")
    _hash(candidate.get("candidate_sha256"), f"{fixture_id}.candidate.candidate_sha256")
    provenance = _require_mapping(fixture.get("provenance"), f"{fixture_id}.provenance")
    instances = _require_list(
        provenance.get("component_recipe_instances"),
        f"{fixture_id}.provenance.component_recipe_instances",
        min_items=FORMAL_MINIMUMS["component_recipe_instances"],
    )
    instance_ids = {
        item.get("instance_id") if isinstance(item, Mapping) else item
        for item in instances
    }
    if len(instance_ids) != len(instances) or None in instance_ids:
        raise FormalKitBlockedError(f"{fixture_id} Component Recipe instance 必须有唯一身份")


def _validate_semantic_fact(fixture: Mapping[str, Any], fixture_id: str) -> None:
    semantic = _require_mapping(fixture.get("semantic"), f"{fixture_id}.semantic")
    for field in ("roles", "profiles", "sections", "features", "child_slots", "connectors", "pivots", "bindings"):
        values = _require_list(semantic.get(field), f"{fixture_id}.semantic.{field}")
        for index, value in enumerate(values):
            if isinstance(value, Mapping):
                continue
            _string(value, f"{fixture_id}.semantic.{field}[{index}]")
    zones = _require_list(
        fixture.get("material_zones"),
        f"{fixture_id}.material_zones",
        min_items=FORMAL_MINIMUMS["material_zones"],
    )
    unique_zones: set[str] = set()
    for index, zone in enumerate(zones):
        unique_zones.add(_string(zone, f"{fixture_id}.material_zones[{index}]"))
    if len(unique_zones) != len(zones):
        raise FormalKitBlockedError(f"{fixture_id}.material_zones 不能重复")


def _validate_pbr_declaration(value: object, fixture_id: str) -> None:
    pbr = _require_mapping(value, f"{fixture_id}.production_pbr")
    texture_sets = _require_list(pbr.get("texture_sets"), f"{fixture_id}.production_pbr.texture_sets")
    for index, texture_set_value in enumerate(texture_sets):
        texture_set = _require_mapping(texture_set_value, f"{fixture_id}.production_pbr.texture_sets[{index}]")
        if not _string(texture_set.get("visual_texture_set_id"), "visual_texture_set_id").endswith("_builtin_v4"):
            raise FormalKitBlockedError(f"{fixture_id} 不是 production v4 texture set")
        _string(texture_set.get("material_id"), "material_id")
        _string(texture_set.get("texture_material_id"), "texture_material_id")
        maps = _require_list(texture_set.get("maps"), f"{fixture_id}.production_pbr.texture_sets[{index}].maps", min_items=5)
        roles: set[str] = set()
        for map_value in maps:
            texture_map = _require_mapping(map_value, "production texture map")
            role = _string(texture_map.get("texture_role"), "production texture map.texture_role")
            texture_id = _string(texture_map.get("texture_id"), "production texture map.texture_id")
            if "_v4_" not in texture_id or texture_map.get("width") != 512 or texture_map.get("height") != 512:
                raise FormalKitBlockedError(f"{fixture_id} production PBR map 必须为 512x512 v4")
            _hash(texture_map.get("sha256"), "production texture map.sha256")
            roles.add(role)
        if roles != PBR_ROLES or len(maps) != 5:
            raise FormalKitBlockedError(f"{fixture_id} 必须声明完整且唯一的五通道 PBR maps")


def _validate_renderer_budget(value: object, fixture_id: str) -> None:
    budget = _require_mapping(value, f"{fixture_id}.renderer_budget")
    for field, limit in RENDERER_LIMITS.items():
        actual = budget.get(field)
        minimum = FORMAL_MINIMUMS[field]
        if type(actual) is not int or actual < minimum or actual > limit:
            raise FormalKitBlockedError(
                f"{fixture_id}.renderer_budget.{field} 必须为 {minimum}..{limit}"
            )


def _validate_renderer_contract(value: object, fixture_id: str) -> None:
    contract = _require_mapping(value, f"{fixture_id}.renderer_contract")
    if contract.get("renderer_id") != "ForgeCADWorkbenchRenderer@1":
        raise FormalKitBlockedError(f"{fixture_id}.renderer_contract.renderer_id 不属于 ForgeCAD 工作台")
    if contract.get("environment_id") != "env_forgecad_room_studio_v1":
        raise FormalKitBlockedError(f"{fixture_id}.renderer_contract.environment_id 不属于固定工作室环境")
    _hash(contract.get("environment_sha256"), f"{fixture_id}.renderer_contract.environment_sha256")
    if contract.get("camera_preset") != "iso" or contract.get("load_state") != "ready" or contract.get("render_source") != "glb_pbr":
        raise FormalKitBlockedError(f"{fixture_id}.renderer_contract 必须为 ready/glb_pbr 固定 iso 捕获")
    if contract.get("single_webgl_context") is not True:
        raise FormalKitBlockedError(f"{fixture_id}.renderer_contract 必须证明单一 WebGL context")
    material_count = contract.get("embedded_pbr_material_count")
    if type(material_count) is not int or material_count < 1:
        raise FormalKitBlockedError(f"{fixture_id}.renderer_contract 缺少实际嵌入 PBR 材质事实")


def _validate_gate_evidence(value: object, fixture_id: str) -> None:
    evidence = _require_mapping(value, f"{fixture_id}.gate_evidence")
    _hash(evidence.get("readback_sha256"), f"{fixture_id}.gate_evidence.readback_sha256")
    report_files: set[Path] = set()
    report_hashes: set[str] = set()
    run_ids: set[str] = set()
    for gate in GATE_CONTRACTS:
        report = _require_mapping(evidence.get(gate), f"{fixture_id}.gate_evidence.{gate}")
        if report.get("status") != "passed":
            raise FormalKitBlockedError(f"{fixture_id}.gate_evidence.{gate}.status 必须为 passed")
        report_file = _safe_evidence_path(report.get("report_file"), f"{fixture_id}.gate_evidence.{gate}.report_file")
        report_hash = _hash(report.get("report_sha256"), f"{fixture_id}.gate_evidence.{gate}.report_sha256")
        if report_file in report_files or report_hash in report_hashes:
            raise FormalKitBlockedError(f"{fixture_id} 三个自动 Gate 必须各自拥有不同的 report file/hash")
        report_files.add(report_file)
        report_hashes.add(report_hash)
        run_id = _string(report.get("execution_id"), f"{fixture_id}.gate_evidence.{gate}.execution_id")
        if run_id in run_ids:
            raise FormalKitBlockedError(f"{fixture_id} 三个自动 Gate 必须各自拥有不同的 execution_id")
        run_ids.add(run_id)
        _hash(report.get("source_glb_sha256"), f"{fixture_id}.gate_evidence.{gate}.source_glb_sha256")


def _safe_evidence_path(value: object, field: str) -> Path:
    raw = _string(value, field)
    path = Path(raw)
    if "\\" in raw or path.is_absolute() or ".." in path.parts or path.suffix.lower() != ".json":
        raise FormalKitBlockedError(f"{field} 必须是 manifest 内的相对 JSON evidence 路径")
    return path


def _validate_renderer_capture(value: object, fixture_id: str) -> None:
    capture = _require_mapping(value, f"{fixture_id}.renderer_capture")
    if capture.get("schema_version") != RENDERER_CAPTURE_SCHEMA:
        raise FormalKitBlockedError(f"{fixture_id}.renderer_capture 必须为 {RENDERER_CAPTURE_SCHEMA}")
    _string(capture.get("capture_id"), f"{fixture_id}.renderer_capture.capture_id")
    _string(capture.get("execution_id"), f"{fixture_id}.renderer_capture.execution_id")
    _safe_evidence_path(capture.get("capture_file"), f"{fixture_id}.renderer_capture.capture_file")
    _hash(capture.get("capture_sha256"), f"{fixture_id}.renderer_capture.capture_sha256")
    _hash(capture.get("source_glb_sha256"), f"{fixture_id}.renderer_capture.source_glb_sha256")
    _validate_renderer_budget(capture.get("metrics"), f"{fixture_id}.renderer_capture.metrics")
    _validate_renderer_contract(capture.get("renderer_contract"), f"{fixture_id}.renderer_capture")


def validate_source_manifest(source: Mapping[str, Any]) -> list[Mapping[str, Any]]:
    """Validate only frozen manifest facts; GLB bytes are verified by ``build_kit``."""

    if source.get("schema_version") != "M108BFormalFixtureSourceManifest@1":
        raise FormalKitBlockedError("需要 M108BFormalFixtureSourceManifest@1，不能使用旧 M108 showcase manifest")
    if source.get("selection_status") != "frozen_before_scoring" or source.get("score_status") != "not_scored":
        raise FormalKitBlockedError("M108B formal fixture 必须在评分前冻结且尚未评分")
    if source.get("formal_eligible") is not True:
        raise FormalKitBlockedError("M108B formal fixture source 必须由独立 freeze 步骤明确标记 formal_eligible=true")
    if source.get("fixture_origin") != "recipe_backed_production":
        raise FormalKitBlockedError("M108B formal source 必须明确为 recipe_backed_production，showcase shortcut 一律拒绝")
    _string(source.get("freeze_id"), "freeze_id")
    fixtures = _require_list(source.get("fixtures"), "fixtures", min_items=12)
    if len(fixtures) != 12:
        raise FormalKitBlockedError("M108B formal source 必须恰好包含 12 个 fixture")
    fixture_ids: set[str] = set()
    hashes: set[str] = set()
    domain_counts = {domain: 0 for domain in DOMAINS}
    for item in fixtures:
        fixture = _require_mapping(item, "fixture")
        fixture_id = _string(fixture.get("fixture_id"), "fixture.fixture_id")
        if fixture_id in fixture_ids or "showcase" in fixture_id.lower():
            raise FormalKitBlockedError("M108B formal fixture ID 不能重复或指向 Python showcase")
        fixture_ids.add(fixture_id)
        domain = _string(fixture.get("domain_pack_id"), f"{fixture_id}.domain_pack_id")
        if domain not in domain_counts:
            raise FormalKitBlockedError(f"{fixture_id} 不属于当前四领域")
        domain_counts[domain] += 1
        _safe_fixture_path(fixture.get("source_glb"), fixture_id)
        glb_hash = _hash(fixture.get("glb_sha256"), f"{fixture_id}.glb_sha256")
        if glb_hash in hashes:
            raise FormalKitBlockedError("M108B formal fixture 必须有 12 份不同 GLB 内容哈希")
        hashes.add(glb_hash)
        if type(fixture.get("glb_byte_size")) is not int or fixture["glb_byte_size"] <= 0:
            raise FormalKitBlockedError(f"{fixture_id}.glb_byte_size 必须是正整数")
        _validate_recipe_fact(fixture, fixture_id)
        _validate_semantic_fact(fixture, fixture_id)
        _validate_pbr_declaration(fixture.get("production_pbr"), fixture_id)
        _validate_renderer_budget(fixture.get("renderer_budget"), fixture_id)
        _validate_renderer_capture(fixture.get("renderer_capture"), fixture_id)
        _validate_gate_evidence(fixture.get("gate_evidence"), fixture_id)
    if any(count != 3 for count in domain_counts.values()):
        raise FormalKitBlockedError("M108B formal source 必须四领域各 3 个 fixture")
    return [dict(item) for item in fixtures]


def _pbr_from_readback(readback: object) -> list[dict[str, object]]:
    return sorted(asdict(readback)["visual_texture_sets"], key=lambda item: int(item["material_index"]))


def _verify_payload(source_root: Path, fixture: Mapping[str, Any]) -> bytes:
    fixture_id = str(fixture["fixture_id"])
    source_path = (source_root / _safe_fixture_path(fixture.get("source_glb"), fixture_id)).resolve()
    try:
        source_path.relative_to(source_root.resolve())
    except ValueError as error:
        raise FormalKitBlockedError(f"{fixture_id} GLB 路径越出 source manifest") from error
    if not source_path.is_file():
        raise FormalKitBlockedError(f"M108B formal kit blocked：缺少冻结的真实 production GLB：{source_path}")
    payload = source_path.read_bytes()
    if len(payload) != fixture["glb_byte_size"] or _sha256(payload) != fixture["glb_sha256"]:
        raise FormalKitBlockedError(f"{fixture_id} 冻结 source GLB hash/size 已漂移")
    try:
        readback = read_shape_program_glb_facts(payload)
    except Exception as error:
        raise FormalKitBlockedError(f"{fixture_id} 不是可回读 ForgeCAD production GLB") from error
    facts = asdict(readback)
    if readback.artifact_profile.get("artifact_profile_id") != "production_concept":
        raise FormalKitBlockedError(f"{fixture_id} 不是 production_concept")
    declared_pbr = _require_mapping(fixture["production_pbr"], f"{fixture_id}.production_pbr")
    if declared_pbr.get("texture_sets") != _pbr_from_readback(readback):
        raise FormalKitBlockedError(f"{fixture_id} production v4 texture/map hashes 与真实 GLB readback 不一致")
    if sorted(fixture["material_zones"]) != sorted({str(item["material_zone_id"]) for item in readback.material_zone_faces}):
        raise FormalKitBlockedError(f"{fixture_id} Material Zone 与真实 GLB readback 不一致")
    roles = {str(item["part_role"]) for item in readback.surface_provenance}
    declared_roles = {
        _string(role, f"{fixture_id}.semantic.roles")
        for role in _require_mapping(fixture["semantic"], "semantic")["roles"]
    }
    if not roles.issuperset(declared_roles):
        raise FormalKitBlockedError(f"{fixture_id} semantic roles 与真实 GLB readback 不一致")
    readback_hash = _sha256(_canonical_bytes(facts))
    gates = _require_mapping(fixture["gate_evidence"], f"{fixture_id}.gate_evidence")
    if gates["readback_sha256"] != readback_hash:
        raise FormalKitBlockedError(f"{fixture_id} readback evidence 已过期或不属于此 GLB")
    for gate in ("m108a", "q003", "g826"):
        report = _require_mapping(gates[gate], f"{fixture_id}.gate_evidence.{gate}")
        report_path = (source_root / _safe_evidence_path(report["report_file"], f"{fixture_id}.{gate}.report_file")).resolve()
        if not report_path.is_file() or _sha256(report_path.read_bytes()) != report["report_sha256"]:
            raise FormalKitBlockedError(f"{fixture_id} {gate} gate report 缺失、篡改或 hash 漂移")
        report_document = _read_object(report_path)
        if (
            report_document.get("schema_version") != GATE_EVIDENCE_SCHEMA
            or report_document.get("evidence_origin") != "authoritative_gate_run"
            or report_document.get("formal_eligible") is not True
            or report_document.get("gate_id") != GATE_CONTRACTS[gate]
            or report_document.get("status") != "passed"
            or report_document.get("source_glb_sha256") != fixture["glb_sha256"]
            or report_document.get("readback_sha256") != gates["readback_sha256"]
            or report_document.get("execution_id") != report["execution_id"]
        ):
            raise FormalKitBlockedError(f"{fixture_id} {gate} gate report 未通过或不属于此 GLB")
    capture = _require_mapping(fixture["renderer_capture"], f"{fixture_id}.renderer_capture")
    capture_path = (source_root / _safe_evidence_path(capture["capture_file"], f"{fixture_id}.renderer_capture.capture_file")).resolve()
    if not capture_path.is_file() or _sha256(capture_path.read_bytes()) != capture["capture_sha256"]:
        raise FormalKitBlockedError(f"{fixture_id} renderer capture 缺失、篡改或 hash 漂移")
    capture_document = _read_object(capture_path)
    if (
        capture_document.get("schema_version") != RENDERER_CAPTURE_SCHEMA
        or capture_document.get("evidence_origin") != "workbench_runtime_capture"
        or capture_document.get("formal_eligible") is not True
        or capture_document.get("capture_id") != capture["capture_id"]
        or capture_document.get("execution_id") != capture["execution_id"]
        or capture_document.get("source_glb_sha256") != fixture["glb_sha256"]
        or capture_document.get("metrics") != fixture["renderer_budget"]
        or capture.get("metrics") != fixture["renderer_budget"]
        or capture_document.get("renderer_contract") != capture["renderer_contract"]
    ):
        raise FormalKitBlockedError(f"{fixture_id} renderer capture 与正式 GLB/budget 不一致")
    if fixture["renderer_budget"]["triangle_count"] != readback.triangle_count:
        raise FormalKitBlockedError(f"{fixture_id} renderer triangle budget 与真实 GLB readback 不一致")
    return payload


def build_kit(*, source_path: Path, output: Path) -> dict[str, object]:
    if output.exists() and any(output.iterdir()):
        raise FormalKitBlockedError(f"输出目录必须为空：{output}")
    source = _read_object(source_path)
    fixtures = validate_source_manifest(source)
    source_root = source_path.resolve().parent
    payloads = [_verify_payload(source_root, fixture) for fixture in fixtures]
    output.mkdir(parents=True, exist_ok=True)
    fixture_dir = output / "fixtures"
    fixture_dir.mkdir()
    formal_fixtures: list[dict[str, object]] = []
    evidence_files: set[Path] = set()
    for fixture, payload in zip(fixtures, payloads):
        filename = f"{fixture['fixture_id'].replace(':', '__')}.glb"
        (fixture_dir / filename).write_bytes(payload)
        record = dict(fixture)
        record["file"] = f"fixtures/{filename}"
        record.pop("source_glb", None)
        formal_fixtures.append(record)
        capture = _require_mapping(fixture["renderer_capture"], "renderer_capture")
        evidence_files.add(_safe_evidence_path(capture["capture_file"], "renderer_capture.capture_file"))
        gates = _require_mapping(fixture["gate_evidence"], "gate_evidence")
        for gate in ("m108a", "q003", "g826"):
            report = _require_mapping(gates[gate], f"gate_evidence.{gate}")
            evidence_files.add(_safe_evidence_path(report["report_file"], f"gate_evidence.{gate}.report_file"))
    for relative_path in evidence_files:
        destination = output / relative_path
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source_root / relative_path, destination)
    copied_source = output / "frozen-source-manifest.json"
    shutil.copyfile(source_path, copied_source)
    manifest: dict[str, object] = {
        "schema_version": "M108BFormalVisualBenchmarkKit@1",
        "formal_m108b": True,
        "fixture_origin": "recipe_backed_production",
        "selection_status": "frozen_before_scoring",
        "score_status": "not_scored",
        "source_manifest_sha256": _sha256(copied_source.read_bytes()),
        "source_manifest_file": "frozen-source-manifest.json",
        "fixtures": formal_fixtures,
        "review_protocol": {"minimum_independent_human_reviewers": 3, "fixture_count_per_reviewer": 12, "dimensions": list(DIMENSIONS), "fixture_median_minimum": 4, "domain_median_minimum": 4},
    }
    manifest_path = output / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    responses = {"schema_version": "M108BFormalVisualBenchmarkResponses@1", "kit_manifest_sha256": _sha256(manifest_path.read_bytes()), "responses": [], "note": "Human-only, independent, complete 12-fixture reviews. Automated or agent scores are invalid."}
    (output / "review-responses.json").write_text(json.dumps(responses, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return manifest


def _self_test() -> None:
    blocked_source = {"schema_version": "M108VisualBenchmarkKit@1", "fixtures": []}
    try:
        validate_source_manifest(blocked_source)
    except FormalKitBlockedError as error:
        if "M108BFormalFixtureSourceManifest" not in str(error):
            raise
    else:
        raise AssertionError("legacy Python showcase kit was accepted as formal M108B source")
    def fixture(index: int, domain: str) -> dict[str, object]:
        digest = f"{index:064x}"
        maps = [{"texture_role": role, "texture_id": f"vtex_self_v4_{role}", "width": 512, "height": 512, "sha256": digest} for role in sorted(PBR_ROLES)]
        renderer_contract = {"renderer_id": "ForgeCADWorkbenchRenderer@1", "environment_id": "env_forgecad_room_studio_v1", "environment_sha256": digest, "camera_preset": "iso", "load_state": "ready", "render_source": "glb_pbr", "single_webgl_context": True, "embedded_pbr_material_count": 1}
        return {
            "fixture_id": f"{domain}:formal_{index}", "domain_pack_id": domain, "source_glb": f"fixtures/formal_{index}.glb", "glb_sha256": digest, "glb_byte_size": 1,
            "recipe": {"recipe_id": f"recipe_{index}", "version": 1, "recipe_sha256": digest},
            "registry": {"registry_sha256": digest, "registry_lock_sha256": digest},
            "candidate": {"candidate_id": f"candidate_{index}", "candidate_sha256": digest},
            "provenance": {
                "component_recipe_instances": [
                    f"recipeinst_{index}_{instance_index}"
                    for instance_index in range(FORMAL_MINIMUMS["component_recipe_instances"])
                ]
            },
            "semantic": {field: [f"{field}_{index}"] for field in ("roles", "profiles", "sections", "features", "child_slots", "connectors", "pivots", "bindings")},
            "material_zones": [
                f"zone_{index}_{zone_index}"
                for zone_index in range(FORMAL_MINIMUMS["material_zones"])
            ],
            "production_pbr": {"texture_sets": [{"visual_texture_set_id": f"vtexset_self_{index}_builtin_v4", "material_id": f"mat_{index}", "texture_material_id": f"mat_{index}", "maps": maps}]},
            "renderer_budget": {field: FORMAL_MINIMUMS[field] for field in RENDERER_LIMITS},
            "renderer_capture": {"schema_version": RENDERER_CAPTURE_SCHEMA, "capture_id": f"capture_{index}", "execution_id": f"renderer_run_{index}", "capture_file": f"evidence/capture_{index}.json", "capture_sha256": digest, "source_glb_sha256": digest, "metrics": {field: FORMAL_MINIMUMS[field] for field in RENDERER_LIMITS}, "renderer_contract": renderer_contract},
            "gate_evidence": {"readback_sha256": digest, **{gate: {"status": "passed", "execution_id": f"{gate}_run_{index}", "report_file": f"evidence/{gate}_{index}.json", "report_sha256": f"{index * 10 + gate_index:064x}", "source_glb_sha256": digest} for gate_index, gate in enumerate(GATE_CONTRACTS, start=1)}},
        }
    fixtures = [fixture(index, domain) for domain_index, domain in enumerate(DOMAINS) for index in range(domain_index * 3 + 1, domain_index * 3 + 4)]
    valid_source = {"schema_version": "M108BFormalFixtureSourceManifest@1", "selection_status": "frozen_before_scoring", "score_status": "not_scored", "formal_eligible": True, "fixture_origin": "recipe_backed_production", "freeze_id": "freeze_self_test", "fixtures": fixtures}
    validate_source_manifest(valid_source)
    for description, mutate in (
        ("missing recipe", lambda value: value["fixtures"][0].pop("recipe")),
        ("showcase shortcut", lambda value: value.__setitem__("fixture_origin", "python_showcase")),
        ("duplicate GLB", lambda value: value["fixtures"][1].__setitem__("glb_sha256", value["fixtures"][0]["glb_sha256"])),
        ("scored selection", lambda value: value.__setitem__("score_status", "scored")),
        ("preflight source", lambda value: value.__setitem__("formal_eligible", False)),
        ("wrong distribution", lambda value: value["fixtures"][0].__setitem__("domain_pack_id", DOMAINS[1])),
        ("blockout instances", lambda value: value["fixtures"][0]["provenance"].__setitem__("component_recipe_instances", ["one"])),
        ("blockout zones", lambda value: value["fixtures"][0].__setitem__("material_zones", ["one"])),
        ("blockout triangles", lambda value: value["fixtures"][0]["renderer_budget"].__setitem__("triangle_count", 416)),
        ("PBR drift", lambda value: value["fixtures"][0]["production_pbr"]["texture_sets"][0]["maps"][0].__setitem__("width", 128)),
        ("reused gate report hash", lambda value: value["fixtures"][0]["gate_evidence"]["q003"].__setitem__("report_sha256", value["fixtures"][0]["gate_evidence"]["m108a"]["report_sha256"])),
        ("reused gate execution", lambda value: value["fixtures"][0]["gate_evidence"]["q003"].__setitem__("execution_id", value["fixtures"][0]["gate_evidence"]["m108a"]["execution_id"])),
        ("untyped renderer capture", lambda value: value["fixtures"][0]["renderer_capture"].__setitem__("schema_version", "M108WorkbenchCapture@1")),
        ("non-workbench renderer capture", lambda value: value["fixtures"][0]["renderer_capture"]["renderer_contract"].__setitem__("single_webgl_context", False)),
    ):
        invalid = copy.deepcopy(valid_source)
        mutate(invalid)
        try:
            validate_source_manifest(invalid)
        except FormalKitBlockedError:
            continue
        raise AssertionError(f"formal source accepted {description}")
    with tempfile.TemporaryDirectory(prefix="forgecad_m108b_formal_") as directory:
        source = Path(directory) / "missing-source.json"
        source.write_text(json.dumps({"schema_version": "M108BFormalFixtureSourceManifest@1", "selection_status": "frozen_before_scoring", "score_status": "not_scored", "formal_eligible": True, "fixture_origin": "recipe_backed_production", "freeze_id": "freeze_self_test", "fixtures": []}), encoding="utf-8")
        try:
            build_kit(source_path=source, output=Path(directory) / "out")
        except FormalKitBlockedError as error:
            if "12" not in str(error):
                raise
        else:
            raise AssertionError("formal M108B builder accepted fewer than 12 fixtures")
    # Test-only use of the existing M108A production compiler fixture.  It is
    # never packaged, never scored, and never named as M108B evidence; it only
    # exercises the real dict-shaped GLB readback boundary until M108B-04 has
    # supplied the frozen twelve-fixture source manifest.
    from forgecad_agent.application.domain_packs import domain_pack_for_message
    from forgecad_agent.application.geometry_worker import (
        build_blockout,
        compile_production_concept_shape_program,
        list_blockout_variants,
    )
    from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner
    from smoke_m108_visual_pbr import BRIEFS

    with tempfile.TemporaryDirectory(prefix="forgecad_m108b_real_readback_") as directory:
        root = Path(directory)
        brief = BRIEFS[0]
        pack = domain_pack_for_message(brief)
        plan = DeterministicMechanicalPlanner().plan_complete_concept(
            brief=brief, pack=pack, project_id="prj_m108b_self_test", action_loop_enabled=False,
        )
        variant_id = next(item for item in list_blockout_variants(pack.pack_id) if item.endswith("_a"))
        result = build_blockout(plan, plan.directions[0].direction_id, variant_id=variant_id, presentation_profile="showcase")
        fixture_id = f"{pack.pack_id}:{variant_id}"
        payload = compile_production_concept_shape_program(result.shape_program).glb_bytes
        (root / "fixtures").mkdir()
        (root / "fixtures" / "test.glb").write_bytes(payload)
        readback = read_shape_program_glb_facts(payload)
        facts = asdict(readback)
        glb_hash = _sha256(payload)
        readback_hash = _sha256(_canonical_bytes(facts))
        budget = {field: 0 for field in RENDERER_LIMITS}
        budget["triangle_count"] = readback.triangle_count
        (root / "evidence").mkdir()
        renderer_contract = {
            "renderer_id": "ForgeCADWorkbenchRenderer@1",
            "environment_id": "env_forgecad_room_studio_v1",
            "environment_sha256": glb_hash,
            "camera_preset": "iso",
            "load_state": "ready",
            "render_source": "glb_pbr",
            "single_webgl_context": True,
            "embedded_pbr_material_count": max(1, len(readback.visual_texture_sets)),
        }
        capture_document = {
            "schema_version": RENDERER_CAPTURE_SCHEMA,
            "evidence_origin": "workbench_runtime_capture",
            "formal_eligible": True,
            "capture_id": "capture_self_test",
            "execution_id": "renderer_self_test_run",
            "source_glb_sha256": glb_hash,
            "metrics": budget,
            "renderer_contract": renderer_contract,
        }
        capture_path = root / "evidence" / "capture.json"
        capture_path.write_text(json.dumps(capture_document), encoding="utf-8")
        reports: dict[str, dict[str, object]] = {}
        for gate in GATE_CONTRACTS:
            report_path = root / "evidence" / f"{gate}.json"
            report_path.write_text(json.dumps({
                "schema_version": GATE_EVIDENCE_SCHEMA,
                "evidence_origin": "authoritative_gate_run",
                "formal_eligible": True,
                "gate_id": GATE_CONTRACTS[gate],
                "status": "passed",
                "execution_id": f"{gate}_self_test_run",
                "source_glb_sha256": glb_hash,
                "readback_sha256": readback_hash,
            }), encoding="utf-8")
            reports[gate] = {"status": "passed", "execution_id": f"{gate}_self_test_run", "report_file": f"evidence/{gate}.json", "report_sha256": _sha256(report_path.read_bytes()), "source_glb_sha256": glb_hash}
        test_fixture: dict[str, object] = {
            "fixture_id": fixture_id.replace(":", "__"), "source_glb": "fixtures/test.glb", "glb_sha256": glb_hash, "glb_byte_size": len(payload),
            "production_pbr": {"texture_sets": _pbr_from_readback(readback)},
            "material_zones": sorted({str(item["material_zone_id"]) for item in readback.material_zone_faces}),
            "semantic": {"roles": sorted({str(item["part_role"]) for item in readback.surface_provenance}), "profiles": ["test"], "sections": ["test"], "features": ["test"], "child_slots": ["test"], "connectors": ["test"], "pivots": ["test"], "bindings": ["test"]},
            "renderer_budget": budget,
            "renderer_capture": {"schema_version": RENDERER_CAPTURE_SCHEMA, "capture_id": "capture_self_test", "execution_id": "renderer_self_test_run", "capture_file": "evidence/capture.json", "capture_sha256": _sha256(capture_path.read_bytes()), "source_glb_sha256": glb_hash, "metrics": budget, "renderer_contract": renderer_contract},
            "gate_evidence": {"readback_sha256": readback_hash, **reports},
        }
        _verify_payload(root, test_fixture)
        for description, mutate in (
            ("PBR", lambda value: value["production_pbr"].__setitem__("texture_sets", [])),
            ("zone", lambda value: value.__setitem__("material_zones", ["zone_missing"])),
            ("role", lambda value: value["semantic"].__setitem__("roles", ["role_missing"])),
            ("budget", lambda value: value["renderer_budget"].__setitem__("triangle_count", 0)),
            ("GLB hash", lambda value: value.__setitem__("glb_sha256", "0" * 64)),
            ("gate identity", lambda value: value["gate_evidence"]["m108a"].__setitem__("execution_id", "q003_self_test_run")),
        ):
            invalid = copy.deepcopy(test_fixture)
            mutate(invalid)
            try:
                _verify_payload(root, invalid)
            except FormalKitBlockedError:
                continue
            raise AssertionError(f"real readback accepted tampered {description}")
        # A source manifest must not be able to promote a development report
        # simply by flipping its own status fields.  Report and capture origin
        # are independently fail-closed, and the file hashes are refreshed in
        # the fixture only so this exercises semantic validation rather than a
        # simpler hash mismatch.
        gate_path = root / "evidence" / "m108a.json"
        original_gate = json.loads(gate_path.read_text(encoding="utf-8"))
        development_gate = copy.deepcopy(original_gate)
        development_gate["evidence_origin"] = "development_preflight"
        development_gate["formal_eligible"] = False
        gate_path.write_text(json.dumps(development_gate), encoding="utf-8")
        invalid = copy.deepcopy(test_fixture)
        invalid["gate_evidence"]["m108a"]["report_sha256"] = _sha256(gate_path.read_bytes())
        try:
            _verify_payload(root, invalid)
        except FormalKitBlockedError:
            pass
        else:
            raise AssertionError("formal M108B payload accepted development gate evidence")
        gate_path.write_text(json.dumps(original_gate), encoding="utf-8")
        capture_path = root / "evidence" / "capture.json"
        original_capture = json.loads(capture_path.read_text(encoding="utf-8"))
        pending_capture = copy.deepcopy(original_capture)
        pending_capture["evidence_origin"] = "development_preflight"
        pending_capture["formal_eligible"] = False
        capture_path.write_text(json.dumps(pending_capture), encoding="utf-8")
        invalid = copy.deepcopy(test_fixture)
        invalid["renderer_capture"]["capture_sha256"] = _sha256(capture_path.read_bytes())
        try:
            _verify_payload(root, invalid)
        except FormalKitBlockedError:
            pass
        else:
            raise AssertionError("formal M108B payload accepted non-workbench capture evidence")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source", type=Path, help="frozen M108BFormalFixtureSourceManifest@1 from future fixture production")
    parser.add_argument("--output", type=Path, help="empty formal kit output directory")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        if args.source or args.output:
            parser.error("--self-test cannot be combined with --source/--output")
        _self_test()
        print("M108B formal benchmark builder self-test passed: legacy/pre-score/underfilled sources fail closed")
        return 0
    if not args.source or not args.output:
        parser.error("--source and --output are both required; no production GLB source is bundled")
    manifest = build_kit(source_path=args.source.resolve(), output=args.output.resolve())
    print(json.dumps({"ok": True, "fixtures": len(manifest["fixtures"]), "score_status": "not_scored"}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
