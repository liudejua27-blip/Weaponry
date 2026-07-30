#!/usr/bin/env python3
"""Fail-closed validator for three independent C111B human blind reviews."""

from __future__ import annotations

import argparse
import copy
import json
import statistics
import tempfile
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Mapping

from prepare_c111b_human_review import (
    DIMENSIONS,
    VIEWS,
    HumanReviewPreparationError,
    _hash_strings,
    _validate_contract,
    build_kit,
    canonical_bytes,
    read_json,
    sha256,
    validate_schema,
    write_self_test_inputs,
)


class HumanReviewValidationError(ValueError):
    pass


def _timestamp(value: object, field: str) -> datetime:
    if not isinstance(value, str):
        raise HumanReviewValidationError(f"{field} must be an ISO-8601 timestamp")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise HumanReviewValidationError(f"{field} must be an ISO-8601 timestamp") from error
    if parsed.tzinfo is None:
        raise HumanReviewValidationError(f"{field} must include a timezone")
    return parsed.astimezone(timezone.utc)


def _safe_file(kit: Path, value: object, field: str) -> Path:
    if not isinstance(value, str) or not value or "\\" in value:
        raise HumanReviewValidationError(f"{field} must be a relative kit file")
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        raise HumanReviewValidationError(f"{field} escapes the review kit")
    path = (kit / relative).resolve()
    try:
        path.relative_to(kit.resolve())
    except ValueError as error:
        raise HumanReviewValidationError(f"{field} escapes the review kit") from error
    if not path.is_file():
        raise HumanReviewValidationError(f"{field} is missing")
    return path


def _exact_hash(path: Path, expected: object, field: str) -> None:
    if not isinstance(expected, str) or sha256(path.read_bytes()) != expected:
        raise HumanReviewValidationError(f"{field} hash drifted")


def _validate_kit(kit: Path) -> tuple[dict[str, Any], bytes]:
    manifest_path = kit / "manifest.json"
    manifest_payload = manifest_path.read_bytes() if manifest_path.is_file() else b""
    manifest = read_json(manifest_path, "C111B human review manifest")
    try:
        validate_schema(manifest)
    except HumanReviewPreparationError as error:
        raise HumanReviewValidationError(str(error)) from error
    if manifest.get("status") != "frozen_before_scoring":
        raise HumanReviewValidationError("kit was not frozen before scoring")
    lineage = manifest["lineage"]
    for file_field, hash_field in (
        ("glb_file", "glb_sha256"),
        ("contract_file", "contract_sha256"),
        ("readback_file", "readback_sha256"),
        ("capture_manifest_file", "capture_manifest_sha256"),
    ):
        path = _safe_file(kit, lineage[file_field], file_field)
        _exact_hash(path, lineage[hash_field], hash_field)
    glb_path = _safe_file(kit, lineage["glb_file"], "glb_file")
    if glb_path.stat().st_size != lineage["glb_byte_size"]:
        raise HumanReviewValidationError("GLB byte size drifted")
    contract = read_json(_safe_file(kit, lineage["contract_file"], "contract_file"), "frozen contract")
    readback = read_json(_safe_file(kit, lineage["readback_file"], "readback_file"), "frozen readback")
    capture_source = read_json(
        _safe_file(kit, lineage["capture_manifest_file"], "capture_manifest_file"),
        "frozen capture manifest",
    )
    try:
        _validate_contract(contract)
    except HumanReviewPreparationError as error:
        raise HumanReviewValidationError(str(error)) from error
    if lineage["glb_sha256"] not in _hash_strings(readback):
        raise HumanReviewValidationError("frozen readback does not bind the exact review GLB")
    if capture_source.get("formal_eligible") is not True or capture_source.get("human_benchmark_evidence") is not True:
        raise HumanReviewValidationError("development captures cannot become human-review evidence")
    if "score_status" in capture_source and capture_source.get("score_status") != "not_scored":
        raise HumanReviewValidationError("capture source was not frozen before scoring")

    captures = lineage["captures"]
    if len(captures) != 8 or {item["view_id"] for item in captures} != set(VIEWS):
        raise HumanReviewValidationError("kit must retain all eight exact C111B captures")
    capture_hashes: dict[str, str] = {}
    for capture in captures:
        path = _safe_file(kit, capture["file"], f"capture.{capture['view_id']}")
        _exact_hash(path, capture["sha256"], f"capture.{capture['view_id']}")
        if capture["source_glb_sha256"] != lineage["glb_sha256"]:
            raise HumanReviewValidationError("capture is not bound to the exact review GLB")
        capture_hashes[capture["view_id"]] = capture["sha256"]

    packets = manifest["reviewer_packets"]
    reviewer_ids = {packet["reviewer_id"] for packet in packets}
    packet_ids = {packet["packet_id"] for packet in packets}
    orders: set[tuple[str, ...]] = set()
    if len(packets) != 3 or len(reviewer_ids) != 3 or len(packet_ids) != 3:
        raise HumanReviewValidationError("kit must have exactly three anonymous reviewer packets")
    for packet in packets:
        packet_path = _safe_file(kit, packet["packet_file"], "packet_file")
        _exact_hash(packet_path, packet["packet_sha256"], "packet_sha256")
        order = packet["view_order"]
        if [item["position"] for item in order] != list(range(1, 9)):
            raise HumanReviewValidationError("blind view positions must be exactly 1..8")
        if [item["blind_view_id"] for item in order] != [f"view_{index:02d}" for index in range(1, 9)]:
            raise HumanReviewValidationError("blind view IDs must be exactly view_01..view_08")
        source_order = tuple(item["source_view_id"] for item in order)
        if set(source_order) != set(VIEWS):
            raise HumanReviewValidationError("every blind packet must contain all eight source views")
        orders.add(source_order)
        for item in order:
            path = _safe_file(kit, item["file"], "blind view")
            _exact_hash(path, item["sha256"], "blind view")
            if item["sha256"] != capture_hashes[item["source_view_id"]]:
                raise HumanReviewValidationError("blind view does not map to its frozen capture")
    if len(orders) != 3:
        raise HumanReviewValidationError("all three reviewers must receive distinct randomized view orders")
    return manifest, manifest_payload


def _receipt_hash(receipt: Mapping[str, Any]) -> str:
    return sha256(canonical_bytes({key: value for key, value in receipt.items() if key != "receipt_sha256"}))


def validate_review(kit: Path, responses_path: Path) -> dict[str, Any]:
    manifest, manifest_payload = _validate_kit(kit)
    responses = read_json(responses_path, "C111B human review responses")
    try:
        validate_schema(responses)
    except HumanReviewPreparationError as error:
        raise HumanReviewValidationError(str(error)) from error
    if responses["review_id"] != manifest["review_id"] or responses["kit_manifest_sha256"] != sha256(manifest_payload):
        raise HumanReviewValidationError("responses do not bind the exact frozen kit manifest")
    if responses["review_origin"] != "independent_human" or responses["agent_or_vlm_used"] is not False:
        raise HumanReviewValidationError("Agent/VLM-authored or substituted scores are forbidden")

    packets = {packet["reviewer_id"]: packet for packet in manifest["reviewer_packets"]}
    reviews = responses["reviews"]
    if len(reviews) != 3 or {review["reviewer_id"] for review in reviews} != set(packets):
        raise HumanReviewValidationError("exactly the three frozen anonymous reviewers must submit")
    frozen_at = _timestamp(manifest["frozen_at"], "frozen_at")
    reviewer_medians: dict[str, float] = {}
    dimension_values = {dimension: [] for dimension in DIMENSIONS}
    seen_receipts: set[str] = set()
    for review in reviews:
        reviewer_id = review["reviewer_id"]
        packet = packets[reviewer_id]
        if review["packet_id"] != packet["packet_id"]:
            raise HumanReviewValidationError("reviewer response uses the wrong blind packet")
        if review["review_method"] != "human_only_no_agent_no_vlm" or review["human_reviewer"] is not True:
            raise HumanReviewValidationError("every score must be authored by a human without Agent/VLM assistance")
        if review["independent_of_implementation"] is not True or review["implementation_participant"] is not False:
            raise HumanReviewValidationError("all three reviewers must be independent of implementation")
        expected_blind_ids = [item["blind_view_id"] for item in packet["view_order"]]
        if review["viewed_blind_view_ids"] != expected_blind_ids:
            raise HumanReviewValidationError("reviewer must acknowledge all eight blind views in assigned order")
        submitted_at = _timestamp(review["submitted_at"], "submitted_at")
        if submitted_at <= frozen_at:
            raise HumanReviewValidationError("the kit freeze time must be earlier than every score")

        receipt = review["interaction_receipt"]
        if receipt["receipt_id"] in seen_receipts:
            raise HumanReviewValidationError("interaction receipts must be independent and unique")
        seen_receipts.add(receipt["receipt_id"])
        if receipt["receipt_sha256"] != _receipt_hash(receipt):
            raise HumanReviewValidationError("workbench interaction receipt hash is invalid")
        if receipt["reviewer_id"] != reviewer_id or receipt["packet_id"] != packet["packet_id"]:
            raise HumanReviewValidationError("workbench receipt is bound to another reviewer packet")
        lineage = manifest["lineage"]
        if (
            receipt["source_glb_sha256"] != lineage["glb_sha256"]
            or receipt["capture_runtime_fingerprint_sha256"] != lineage["capture_runtime_fingerprint_sha256"]
            or receipt["receipt_origin"] != "forgecad_workbench_runtime"
            or receipt["workbench_id"] != "ForgeCADWorkbench@1"
            or receipt["renderer_id"] != "ForgeCADWorkbenchRenderer@1"
            or receipt["load_state"] != "ready"
            or receipt["static_images_only"] is not False
        ):
            raise HumanReviewValidationError("usability requires a receipt from the same exact ForgeCAD workbench GLB/runtime")
        started_at = _timestamp(receipt["session_started_at"], "session_started_at")
        ended_at = _timestamp(receipt["session_ended_at"], "session_ended_at")
        if not frozen_at < started_at < ended_at <= submitted_at:
            raise HumanReviewValidationError("workbench interaction must occur after freeze and before score submission")
        interactions = receipt["interactions"]
        if any(type(interactions[action]) is not int or interactions[action] < 1 for action in ("orbit", "zoom", "part_selection", "material_zone_inspection")):
            raise HumanReviewValidationError("static captures cannot substitute for required usability interactions")

        scores = review["scores"]
        for dimension in DIMENSIONS:
            value = scores[dimension]
            if type(value) is not int or not 1 <= value <= 5:
                raise HumanReviewValidationError(f"{dimension} must be an integer from 1 to 5")
            dimension_values[dimension].append(value)
        reviewer_medians[reviewer_id] = float(statistics.median(scores[dimension] for dimension in DIMENSIONS))

    dimension_medians = {dimension: float(statistics.median(values)) for dimension, values in dimension_values.items()}
    failed_dimensions = sorted(dimension for dimension, median in dimension_medians.items() if median < 4)
    failed_reviewers = sorted(reviewer for reviewer, median in reviewer_medians.items() if median < 4)
    if failed_dimensions or failed_reviewers:
        raise HumanReviewValidationError(
            f"human review failed: dimension medians below 4={failed_dimensions}; reviewer medians below 4={failed_reviewers}"
        )
    return {
        "schema_version": "C111BHumanReviewResult@1",
        "status": "pass",
        "review_id": manifest["review_id"],
        "kit_manifest_sha256": sha256(manifest_payload),
        "reviewer_count": 3,
        "dimension_medians": dimension_medians,
        "reviewer_medians": reviewer_medians,
        "human_only": True,
        "agent_or_vlm_used": False,
        "usability_workbench_receipts": 3,
    }


def _valid_responses(manifest: Mapping[str, Any], manifest_payload: bytes) -> dict[str, Any]:
    frozen_at = _timestamp(manifest["frozen_at"], "frozen_at")
    reviews = []
    for index, packet in enumerate(manifest["reviewer_packets"]):
        started = frozen_at + timedelta(minutes=1 + index * 5)
        ended = started + timedelta(minutes=3)
        submitted = ended + timedelta(minutes=1)
        receipt: dict[str, Any] = {
            "schema_version": "C111BWorkbenchInteractionReceipt@1",
            "receipt_id": f"receipt_{index + 1:016x}",
            "receipt_origin": "forgecad_workbench_runtime",
            "receipt_sha256": "",
            "reviewer_id": packet["reviewer_id"],
            "packet_id": packet["packet_id"],
            "source_glb_sha256": manifest["lineage"]["glb_sha256"],
            "capture_runtime_fingerprint_sha256": manifest["lineage"]["capture_runtime_fingerprint_sha256"],
            "workbench_id": "ForgeCADWorkbench@1",
            "renderer_id": "ForgeCADWorkbenchRenderer@1",
            "load_state": "ready",
            "static_images_only": False,
            "session_started_at": started.isoformat().replace("+00:00", "Z"),
            "session_ended_at": ended.isoformat().replace("+00:00", "Z"),
            "event_log_sha256": f"{100 + index:064x}",
            "interactions": {"orbit": 2, "zoom": 1, "part_selection": 1, "material_zone_inspection": 1},
        }
        receipt["receipt_sha256"] = _receipt_hash(receipt)
        reviews.append({
            "reviewer_id": packet["reviewer_id"],
            "packet_id": packet["packet_id"],
            "human_reviewer": True,
            "independent_of_implementation": True,
            "implementation_participant": False,
            "review_method": "human_only_no_agent_no_vlm",
            "submitted_at": submitted.isoformat().replace("+00:00", "Z"),
            "viewed_blind_view_ids": [item["blind_view_id"] for item in packet["view_order"]],
            "scores": {dimension: 4 for dimension in DIMENSIONS},
            "interaction_receipt": receipt,
        })
    return {
        "schema_version": "C111BHumanReviewResponses@1",
        "review_id": manifest["review_id"],
        "kit_manifest_sha256": sha256(manifest_payload),
        "review_origin": "independent_human",
        "agent_or_vlm_used": False,
        "reviews": reviews,
    }


def self_test() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad_c111b_human_validate_") as directory:
        root = Path(directory)
        glb, contract, readback, captures = write_self_test_inputs(root)
        kit = root / "kit"
        manifest = build_kit(glb_path=glb, contract_path=contract, readback_path=readback, capture_manifest_path=captures, output=kit, frozen_at=datetime(2026, 7, 29, tzinfo=timezone.utc), entropy=bytes(range(32)))
        manifest_payload = (kit / "manifest.json").read_bytes()
        valid = _valid_responses(manifest, manifest_payload)
        responses_path = root / "responses.json"
        responses_path.write_text(json.dumps(valid), encoding="utf-8")
        result = validate_review(kit, responses_path)
        assert result["status"] == "pass" and set(result["dimension_medians"]) == set(DIMENSIONS)

        mutations = (
            ("dimension median", lambda value: [review["scores"].__setitem__("macro", 3) for review in value["reviews"]]),
            ("reviewer median", lambda value: value["reviews"][0].__setitem__("scores", {dimension: 3 for dimension in DIMENSIONS})),
            ("Agent score", lambda value: value.__setitem__("agent_or_vlm_used", True)),
            ("non-independent", lambda value: value["reviews"][0].__setitem__("independent_of_implementation", False)),
            ("static-only usability", lambda value: value["reviews"][0]["interaction_receipt"].__setitem__("static_images_only", True)),
            ("missing interaction", lambda value: value["reviews"][0]["interaction_receipt"]["interactions"].__setitem__("part_selection", 0)),
            ("wrong GLB", lambda value: value["reviews"][0]["interaction_receipt"].__setitem__("source_glb_sha256", "0" * 64)),
            ("score before freeze", lambda value: value["reviews"][0].__setitem__("submitted_at", manifest["frozen_at"])),
            ("wrong view order", lambda value: value["reviews"][0]["viewed_blind_view_ids"].reverse()),
        )
        for description, mutate in mutations:
            invalid = copy.deepcopy(valid)
            mutate(invalid)
            receipt = invalid["reviews"][0]["interaction_receipt"]
            if description in {"static-only usability", "missing interaction", "wrong GLB"}:
                receipt["receipt_sha256"] = _receipt_hash(receipt)
            responses_path.write_text(json.dumps(invalid), encoding="utf-8")
            try:
                validate_review(kit, responses_path)
            except (HumanReviewValidationError, HumanReviewPreparationError):
                continue
            raise AssertionError(f"validator accepted {description}")
        capture_path = kit / manifest["lineage"]["captures"][0]["file"]
        capture_path.write_bytes(capture_path.read_bytes() + b"tamper")
        responses_path.write_text(json.dumps(valid), encoding="utf-8")
        try:
            validate_review(kit, responses_path)
        except HumanReviewValidationError:
            pass
        else:
            raise AssertionError("validator accepted a tampered frozen capture")
    print(json.dumps({"status": "pass", "tests": "c111b-human-review-validator"}, sort_keys=True))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--kit", type=Path)
    parser.add_argument("--responses", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return 0
    if not args.kit or not args.responses:
        parser.error("--kit and --responses are required")
    print(json.dumps(validate_review(args.kit, args.responses), ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (HumanReviewValidationError, HumanReviewPreparationError) as error:
        print(json.dumps({"status": "fail", "error": str(error)}, ensure_ascii=False, sort_keys=True))
        raise SystemExit(2)
