#!/usr/bin/env python3
"""Prepare a frozen, anonymous C111B human-review kit without scoring it."""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import secrets
import shutil
import tempfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Mapping

from jsonschema import Draft202012Validator, FormatChecker


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "packages/concept-spec/schemas/c111b-human-review.schema.json"
DEFAULT_CONTRACT = ROOT / "packages/concept-spec/fixtures/c111b-visual-acceptance-contract.json"
VIEWS = ("iso", "front", "back", "left", "right", "top", "gripper_iso", "gripper_front")
DIMENSIONS = ("macro", "meso", "micro", "pbr", "presentation", "usability")


class HumanReviewPreparationError(ValueError):
    pass


def canonical_bytes(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True).encode("utf-8")


def sha256(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def read_json(path: Path, field: str) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise HumanReviewPreparationError(f"{field} must be readable JSON: {path}") from error
    if not isinstance(value, dict):
        raise HumanReviewPreparationError(f"{field} must be a JSON object")
    return value


def parse_timestamp(value: str) -> datetime:
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise HumanReviewPreparationError("frozen_at must be an ISO-8601 timestamp") from error
    if parsed.tzinfo is None:
        raise HumanReviewPreparationError("frozen_at must include a timezone")
    return parsed.astimezone(timezone.utc)


def format_timestamp(value: datetime) -> str:
    return value.astimezone(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def validate_schema(document: Mapping[str, Any]) -> None:
    schema = read_json(SCHEMA_PATH, "human review schema")
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = sorted(validator.iter_errors(document), key=lambda item: list(item.absolute_path))
    if errors:
        location = ".".join(str(item) for item in errors[0].absolute_path) or "root"
        raise HumanReviewPreparationError(f"C111B human-review schema rejected {location}: {errors[0].message}")


def _hash_strings(value: object) -> set[str]:
    found: set[str] = set()
    if isinstance(value, Mapping):
        for key, child in value.items():
            if key in {"glb_sha256", "production_glb_sha256", "source_glb_sha256", "source_sha256"} and isinstance(child, str):
                found.add(child)
            found.update(_hash_strings(child))
    elif isinstance(value, list):
        for child in value:
            found.update(_hash_strings(child))
    return found


def _validate_contract(contract: Mapping[str, Any]) -> None:
    if contract.get("schema_version") != "C111BVisualAcceptanceContract@2" or contract.get("status") != "frozen":
        raise HumanReviewPreparationError("contract must be frozen C111BVisualAcceptanceContract@2")
    protocol = contract.get("independent_human_review")
    if not isinstance(protocol, Mapping) or (
        protocol.get("independent_reviewers") != 3
        or protocol.get("score_scale") != [1, 2, 3, 4, 5]
        or protocol.get("score_minimum") != 4
        or protocol.get("score_dimensions") != list(DIMENSIONS)
        or protocol.get("agent_or_vlm_substitution_allowed") is not False
        or protocol.get("requires_blinded_fixed_view_package") is not True
    ):
        raise HumanReviewPreparationError("contract human-review policy is not the frozen C111B six-dimension protocol")
    if contract.get("fixed_views") != list(VIEWS):
        raise HumanReviewPreparationError("contract must freeze the exact eight C111B views")


def _resolve_capture_file(manifest_path: Path, raw: object) -> Path:
    if not isinstance(raw, str) or not raw.strip() or "\\" in raw:
        raise HumanReviewPreparationError("capture path must be a non-empty local path")
    path = Path(raw)
    candidates = []
    if path.is_absolute():
        candidates.append(path)
    else:
        candidates.extend((manifest_path.parent / path, ROOT / path, manifest_path.parent / path.name))
    for candidate in candidates:
        if candidate.is_file():
            return candidate.resolve()
    raise HumanReviewPreparationError(f"capture file is missing: {raw}")


def _capture_records(manifest_path: Path, manifest: Mapping[str, Any], stage: str) -> tuple[str, list[dict[str, Any]], str]:
    schema_version = manifest.get("schema_version")
    renderer_id = "ForgeCADWorkbenchRenderer@1"
    raw_records: list[Mapping[str, Any]] = []
    if manifest.get("formal_eligible") is not True or manifest.get("human_benchmark_evidence") is not True:
        raise HumanReviewPreparationError(
            "capture manifest is development evidence, not an eligible human-benchmark source"
        )
    if "score_status" in manifest and manifest.get("score_status") != "not_scored":
        raise HumanReviewPreparationError("capture source must be unscored before kit freeze")
    if schema_version == "C111BWorkbenchRendererCapture@1":
        renderer = manifest.get("renderer_contract")
        if not isinstance(renderer, Mapping) or renderer.get("renderer_id") != renderer_id or renderer.get("single_webgl_context") is not True:
            raise HumanReviewPreparationError("capture manifest must come from the single ForgeCAD workbench renderer")
        captures = manifest.get("captures")
        if isinstance(captures, list):
            raw_records = [item for item in captures if isinstance(item, Mapping)]
    elif schema_version == "C111BPackagedWebGL@1":
        if manifest.get("real_packaged_webview") is not True or manifest.get("single_renderer") is not True:
            raise HumanReviewPreparationError("packaged captures must prove one real ForgeCAD workbench renderer")
        captures = manifest.get("captures")
        if isinstance(captures, Mapping) and isinstance(captures.get(stage), Mapping):
            raw_records = [item for item in captures[stage].values() if isinstance(item, Mapping)]
    elif schema_version == "C111BHumanReviewCaptureSource@1":
        renderer = manifest.get("renderer_contract")
        if not isinstance(renderer, Mapping) or renderer.get("renderer_id") != renderer_id or renderer.get("single_webgl_context") is not True:
            raise HumanReviewPreparationError("self-contained capture source must bind the ForgeCAD workbench renderer")
        captures = manifest.get("captures")
        if isinstance(captures, list):
            raw_records = [item for item in captures if isinstance(item, Mapping)]
    else:
        raise HumanReviewPreparationError("unsupported C111B workbench capture manifest")

    by_view: dict[str, dict[str, Any]] = {}
    source_hashes: set[str] = set()
    for record in raw_records:
        view_id = record.get("view_id")
        if view_id not in VIEWS or view_id in by_view:
            raise HumanReviewPreparationError("capture manifest must contain each C111B view exactly once")
        declared_hash = record.get("screenshot_sha256", record.get("sha256"))
        source_hash = record.get("source_glb_sha256", record.get("source_sha256"))
        if not isinstance(declared_hash, str) or not isinstance(source_hash, str):
            raise HumanReviewPreparationError(f"capture {view_id} is missing exact hashes")
        raw_path = record.get("screenshot", record.get("file", record.get("path", record.get("relative_path"))))
        source_path = _resolve_capture_file(manifest_path, raw_path)
        payload = source_path.read_bytes()
        if sha256(payload) != declared_hash:
            raise HumanReviewPreparationError(f"capture {view_id} hash drifted")
        by_view[str(view_id)] = {"source_path": source_path, "sha256": declared_hash, "source_glb_sha256": source_hash}
        source_hashes.add(source_hash)
    if set(by_view) != set(VIEWS) or len(raw_records) != 8 or len(source_hashes) != 1:
        raise HumanReviewPreparationError("capture manifest must bind eight unique views to one GLB")
    manifest_hash = sha256(manifest_path.read_bytes())
    runtime_fingerprint = sha256(canonical_bytes({
        "capture_manifest_sha256": manifest_hash,
        "capture_schema_version": schema_version,
        "renderer_id": renderer_id,
        "source_glb_sha256": next(iter(source_hashes)),
        "capture_hashes": {view: by_view[view]["sha256"] for view in VIEWS},
    }))
    return next(iter(source_hashes)), [dict(view_id=view, **by_view[view]) for view in VIEWS], runtime_fingerprint


def _token(entropy: bytes, label: str) -> str:
    return sha256(entropy + label.encode("utf-8"))[:16]


def build_kit(
    *,
    glb_path: Path,
    contract_path: Path,
    readback_path: Path,
    capture_manifest_path: Path,
    output: Path,
    capture_stage: str = "initial",
    frozen_at: datetime | None = None,
    entropy: bytes | None = None,
) -> dict[str, Any]:
    if output.exists() and any(output.iterdir()):
        raise HumanReviewPreparationError(f"output directory must be empty: {output}")
    for path, field in ((glb_path, "GLB"), (contract_path, "contract"), (readback_path, "readback"), (capture_manifest_path, "capture manifest")):
        if not path.is_file():
            raise HumanReviewPreparationError(f"{field} file is missing: {path}")

    glb_payload = glb_path.read_bytes()
    if not glb_payload:
        raise HumanReviewPreparationError("GLB must not be empty")
    glb_hash = sha256(glb_payload)
    contract = read_json(contract_path, "contract")
    _validate_contract(contract)
    readback = read_json(readback_path, "readback")
    if glb_hash not in _hash_strings(readback):
        raise HumanReviewPreparationError("readback does not bind the exact review GLB SHA-256")
    capture_manifest = read_json(capture_manifest_path, "capture manifest")
    capture_glb_hash, captures, runtime_fingerprint = _capture_records(capture_manifest_path, capture_manifest, capture_stage)
    if capture_glb_hash != glb_hash:
        raise HumanReviewPreparationError("all eight captures must bind the exact review GLB SHA-256")

    entropy = entropy or secrets.token_bytes(32)
    if len(entropy) < 16:
        raise HumanReviewPreparationError("randomization entropy is too short")
    frozen_at = frozen_at or datetime.now(timezone.utc)
    frozen_text = format_timestamp(frozen_at)
    review_id = f"c111b_review_{_token(entropy, 'review')}"
    output.mkdir(parents=True, exist_ok=True)
    (output / "artifact").mkdir()
    (output / "evidence").mkdir()
    (output / "captures").mkdir()
    shutil.copyfile(glb_path, output / "artifact/model.glb")
    shutil.copyfile(contract_path, output / "evidence/visual-acceptance-contract.json")
    shutil.copyfile(readback_path, output / "evidence/readback.json")
    shutil.copyfile(capture_manifest_path, output / "evidence/capture-manifest.json")

    capture_entries: list[dict[str, Any]] = []
    capture_by_view = {item["view_id"]: item for item in captures}
    for view_id in VIEWS:
        destination = output / f"captures/{view_id}.png"
        shutil.copyfile(capture_by_view[view_id]["source_path"], destination)
        capture_entries.append({
            "view_id": view_id,
            "file": f"captures/{view_id}.png",
            "sha256": capture_by_view[view_id]["sha256"],
            "source_glb_sha256": glb_hash,
        })

    rng = random.Random(int.from_bytes(entropy, "big"))
    reviewer_packets: list[dict[str, Any]] = []
    used_orders: set[tuple[str, ...]] = set()
    for reviewer_index in range(3):
        reviewer_id = f"reviewer_{_token(entropy, f'reviewer:{reviewer_index}')}"
        packet_id = f"packet_{_token(entropy, f'packet:{reviewer_index}')}"
        order = list(VIEWS)
        for _ in range(64):
            rng.shuffle(order)
            if tuple(order) not in used_orders:
                break
        if tuple(order) in used_orders:
            raise HumanReviewPreparationError("could not produce three distinct randomized view orders")
        used_orders.add(tuple(order))
        reviewer_root = output / f"reviewers/{reviewer_id}"
        (reviewer_root / "views").mkdir(parents=True)
        manifest_views: list[dict[str, Any]] = []
        blind_views: list[dict[str, Any]] = []
        for position, source_view_id in enumerate(order, start=1):
            blind_view_id = f"view_{position:02d}"
            relative_file = f"reviewers/{reviewer_id}/views/{blind_view_id}.png"
            shutil.copyfile(output / f"captures/{source_view_id}.png", output / relative_file)
            manifest_views.append({
                "position": position,
                "blind_view_id": blind_view_id,
                "source_view_id": source_view_id,
                "file": relative_file,
                "sha256": capture_by_view[source_view_id]["sha256"],
            })
            blind_views.append({"position": position, "blind_view_id": blind_view_id, "file": f"views/{blind_view_id}.png", "sha256": capture_by_view[source_view_id]["sha256"]})
        packet = {
            "schema_version": "C111BBlindReviewerPacket@1",
            "review_id": review_id,
            "reviewer_id": reviewer_id,
            "packet_id": packet_id,
            "frozen_at": frozen_text,
            "anonymous_asset": True,
            "human_only_no_agent_no_vlm": True,
            "artifact_file": "../../artifact/model.glb",
            "views": blind_views,
            "dimensions": list(DIMENSIONS),
            "score_scale": [1, 2, 3, 4, 5],
            "usability_requires_forgecad_workbench_interaction_receipt": True,
        }
        packet_path = reviewer_root / "packet.json"
        packet_path.write_text(json.dumps(packet, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        reviewer_packets.append({
            "reviewer_id": reviewer_id,
            "packet_id": packet_id,
            "packet_file": f"reviewers/{reviewer_id}/packet.json",
            "packet_sha256": sha256(packet_path.read_bytes()),
            "view_order": manifest_views,
        })

    manifest = {
        "schema_version": "C111BHumanReviewKit@1",
        "review_id": review_id,
        "status": "frozen_before_scoring",
        "frozen_at": frozen_text,
        "lineage": {
            "glb_file": "artifact/model.glb",
            "glb_sha256": glb_hash,
            "glb_byte_size": len(glb_payload),
            "contract_file": "evidence/visual-acceptance-contract.json",
            "contract_sha256": sha256(contract_path.read_bytes()),
            "readback_file": "evidence/readback.json",
            "readback_sha256": sha256(readback_path.read_bytes()),
            "capture_manifest_file": "evidence/capture-manifest.json",
            "capture_manifest_sha256": sha256(capture_manifest_path.read_bytes()),
            "capture_runtime_fingerprint_sha256": runtime_fingerprint,
            "captures": capture_entries,
        },
        "blind_protocol": {
            "anonymous_reviewers": True,
            "randomized_view_order": True,
            "randomization_commitment_sha256": sha256(entropy),
            "independent_reviewer_count": 3,
            "human_only": True,
            "agent_or_vlm_allowed": False,
            "dimensions": list(DIMENSIONS),
            "score_scale": [1, 2, 3, 4, 5],
            "dimension_median_minimum": 4,
            "reviewer_median_minimum": 4,
        },
        "workbench_protocol": {
            "workbench_id": "ForgeCADWorkbench@1",
            "renderer_id": "ForgeCADWorkbenchRenderer@1",
            "same_runtime_fingerprint_required": True,
            "usability_requires_interaction_receipt": True,
            "static_images_sufficient": False,
            "required_interactions": ["orbit", "zoom", "part_selection", "material_zone_inspection"],
        },
        "reviewer_packets": reviewer_packets,
    }
    validate_schema(manifest)
    manifest_path = output / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    template = {
        "schema_version": "C111BHumanReviewResponses@1",
        "review_id": review_id,
        "kit_manifest_sha256": sha256(manifest_path.read_bytes()),
        "review_origin": "independent_human",
        "agent_or_vlm_used": False,
        "reviews": [],
    }
    (output / "review-responses.template.json").write_text(json.dumps(template, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return manifest


def write_self_test_inputs(root: Path) -> tuple[Path, Path, Path, Path]:
    glb = root / "candidate.glb"
    glb.write_bytes(b"glTF-c111b-human-review-self-test")
    glb_hash = sha256(glb.read_bytes())
    contract = root / "contract.json"
    contract.write_text(json.dumps({
        "schema_version": "C111BVisualAcceptanceContract@2", "status": "frozen", "formal_eligible": False,
        "fixed_views": list(VIEWS),
        "independent_human_review": {"independent_reviewers": 3, "score_scale": [1, 2, 3, 4, 5], "score_minimum": 4, "score_dimensions": list(DIMENSIONS), "agent_or_vlm_substitution_allowed": False, "requires_blinded_fixed_view_package": True},
    }), encoding="utf-8")
    readback = root / "readback.json"
    readback.write_text(json.dumps({"schema_version": "SelfTestReadback@1", "production": {"glb_sha256": glb_hash}}), encoding="utf-8")
    capture_root = root / "source-captures"
    capture_root.mkdir()
    records = []
    for index, view_id in enumerate(VIEWS):
        path = capture_root / f"{view_id}.png"
        path.write_bytes(b"\x89PNG\r\n\x1a\n" + bytes([index]) + view_id.encode("ascii"))
        records.append({"view_id": view_id, "file": path.name, "sha256": sha256(path.read_bytes()), "source_glb_sha256": glb_hash})
    capture_manifest = capture_root / "manifest.json"
    capture_manifest.write_text(json.dumps({"schema_version": "C111BHumanReviewCaptureSource@1", "formal_eligible": True, "human_benchmark_evidence": True, "score_status": "not_scored", "renderer_contract": {"renderer_id": "ForgeCADWorkbenchRenderer@1", "single_webgl_context": True}, "captures": records}), encoding="utf-8")
    return glb, contract, readback, capture_manifest


def self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad_c111b_human_prepare_") as directory:
        root = Path(directory)
        glb, contract, readback, captures = write_self_test_inputs(root)
        output = root / "kit"
        manifest = build_kit(glb_path=glb, contract_path=contract, readback_path=readback, capture_manifest_path=captures, output=output, frozen_at=datetime(2026, 7, 29, tzinfo=timezone.utc), entropy=bytes(range(32)))
        assert len(manifest["lineage"]["captures"]) == 8
        assert len({tuple(item["source_view_id"] for item in packet["view_order"]) for packet in manifest["reviewer_packets"]}) == 3
        assert all((output / packet["packet_file"]).is_file() for packet in manifest["reviewer_packets"])
        tampered = read_json(captures, "capture manifest")
        tampered["captures"][0]["source_glb_sha256"] = "0" * 64
        captures.write_text(json.dumps(tampered), encoding="utf-8")
        try:
            build_kit(glb_path=glb, contract_path=contract, readback_path=readback, capture_manifest_path=captures, output=root / "invalid", entropy=b"x" * 32)
        except HumanReviewPreparationError:
            pass
        else:
            raise AssertionError("prepare accepted capture lineage drift")
    print(json.dumps({"status": "pass", "tests": "c111b-human-review-prepare"}, sort_keys=True))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--glb", type=Path)
    parser.add_argument("--contract", type=Path, default=DEFAULT_CONTRACT)
    parser.add_argument("--readback", type=Path)
    parser.add_argument("--capture-manifest", type=Path)
    parser.add_argument("--capture-stage", choices=("initial", "restart"), default="initial")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--frozen-at")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if not all((args.glb, args.readback, args.capture_manifest, args.output)):
        parser.error("--glb, --readback, --capture-manifest and --output are required")
    frozen_at = parse_timestamp(args.frozen_at) if args.frozen_at else None
    manifest = build_kit(glb_path=args.glb, contract_path=args.contract, readback_path=args.readback, capture_manifest_path=args.capture_manifest, output=args.output, capture_stage=args.capture_stage, frozen_at=frozen_at)
    print(json.dumps({"status": "prepared", "review_id": manifest["review_id"], "output": str(args.output)}, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except HumanReviewPreparationError as error:
        print(json.dumps({"status": "blocked", "error": str(error)}, ensure_ascii=False, sort_keys=True))
        raise SystemExit(2)
