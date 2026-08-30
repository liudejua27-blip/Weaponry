#!/usr/bin/env python3
"""Replay and restart-verify the exact 04BE-G visibility calibration."""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path

from probe_mcp010b_raw_stdio import GateFailure, build_identity
from probe_production_weapon_form_art_failure_diagnostic_d1 import (
    CROSS_VIEW,
    EVIDENCE_ID,
    EVIDENCE_RECEIPT,
    EVIDENCE_RECORD,
    FORM_ART,
    PROJECT_ID,
    PROPOSAL_ID,
    SESSION_ID,
    canonical_hash,
    cas_snapshot,
    close_client,
    diagnostic_request,
    open_client,
    require,
    sqlite_snapshot,
)

FAILURE_CANONICAL = "68e838cebbabccec0fa042d59c5f9d4685b727216dc5a45171282cf957f16fa3"
CALIBRATION_POLICY = "exact-before-after-triangle-owner-depth-and-side-aperture-calibration@1"
CANONICALIZATION_POLICY = "canonical-json-sha256-excluding-input-sha256@1"
MAX_RESPONSE_BYTES = 1_048_576


def calibration_request() -> dict[str, object]:
    failure = diagnostic_request()
    value: dict[str, object] = {
        "schema_version": "ProductionWeaponFormArtVisibilityCalibrationGetRequest@1",
        "operation": "forgecad.production.weapon.form-art-visibility-calibration-get@1",
        "calibration_id": "fps-form-04be-g-visibility-calibration-v1",
        "failure_diagnostic_id": failure["diagnostic_id"],
        "failure_diagnostic_canonical_sha256": FAILURE_CANONICAL,
        "failure_diagnostic_input_sha256": failure["input_sha256"],
        "composite_evidence_id": EVIDENCE_ID,
        "proposal_id": PROPOSAL_ID,
        "session_id": SESSION_ID,
        "project_id": PROJECT_ID,
        "composite_evidence_record_canonical_sha256": EVIDENCE_RECORD,
        "composite_evidence_receipt_object_sha256": EVIDENCE_RECEIPT,
        "cross_view_evidence_bundle_sha256": CROSS_VIEW,
        "proposal_form_art_evidence_receipt_object_sha256": FORM_ART,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "calibration_policy": CALIBRATION_POLICY,
        "canonicalization_policy": CANONICALIZATION_POLICY,
    }
    value["input_sha256"] = canonical_hash(value)
    return value


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--expected-build-cohort", required=True)
    parser.add_argument("--timeout", type=float, default=240.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    identities = {"mcp": build_identity(args.mcp), "runtime": build_identity(args.runtime)}
    require(
        all(
            identity.get("build_cohort_sha256") == args.expected_build_cohort
            for identity in identities.values()
        ),
        "Runtime/MCP build cohort differs",
    )
    database = args.data_root / "runtime.sqlite"
    cas_root = args.data_root / "runtime.cas"
    require(database.is_file() and cas_root.is_dir(), "D1 Runtime data is unavailable")
    repository = Path(__file__).resolve().parents[1]
    evidence_path = args.evidence if args.evidence.is_absolute() else repository / args.evidence
    evidence_path.resolve().relative_to((repository / "docs" / "evidence").resolve())

    sqlite_before = sqlite_snapshot(database)
    cas_before = cas_snapshot(cas_root)
    request = calibration_request()
    runs: list[dict[str, object]] = []
    with tempfile.TemporaryDirectory(prefix="forgecad-04be-g-", dir="/tmp") as temporary:
        temporary_root = Path(temporary)
        for index in range(2):
            runtime, ready_path, ready, client = open_client(
                args.mcp,
                args.runtime,
                args.data_root,
                temporary_root / f"run-{index + 1}",
                args.timeout,
            )
            try:
                names = {
                    item.get("name")
                    for item in client.request("tools/list").get("result", {}).get("tools", [])
                    if isinstance(item, dict)
                }
                require(
                    "production_weapon_form_art_visibility_calibration_get" in names,
                    "04BE-G calibration tool is not exposed",
                )
                runs.append(
                    client.tool(
                        "production_weapon_form_art_visibility_calibration_get", request
                    )
                )
            finally:
                close_client(runtime, ready_path, ready, client)

    sqlite_after = sqlite_snapshot(database)
    cas_after = cas_snapshot(cas_root)
    first, restart = runs
    require(
        first.get("canonical_sha256") == restart.get("canonical_sha256"),
        "04BE-G restart canonical hash differs",
    )
    require(sqlite_before == sqlite_after, "04BE-G read-only calibration changed SQLite")
    require(cas_before == cas_after, "04BE-G read-only calibration changed CAS")
    require(
        first.get("quality_status") == "QUALITY_TARGET_NOT_MET"
        and first.get("form_quality_v2_status") == "NOT_CREATED"
        and first.get("side_aperture_occluders_calibrated") is True
        and first.get("repair_plan_authorized") is True
        and first.get("geometry_repair_authorized") is False
        and first.get("production_stage_advanced") is False
        and first.get("runtime_write_performed") is False
        and first.get("persistent_user_data_touched") is False
        and first.get("worker_started") is True,
        "04BE-G non-promotion boundary differs",
    )
    views = first.get("views", [])
    require(
        isinstance(views, list)
        and [view.get("view_kind") for view in views]
        == ["left", "right", "rear-three-quarter"]
        and all(len(view.get("structures", [])) == 2 for view in views),
        "04BE-G view calibration shape differs",
    )

    receipt = {
        "schema_version": "ForgeCADProductionWeaponFormArtVisibilityCalibrationRealD1Gate@1",
        "task_id": "FPS-FORM-04BE-G",
        "recorded_on": "2026-08-28",
        "status": "PASS_READ_ONLY_EXACT_RASTER_VISIBILITY_CALIBRATION",
        "build": {
            "build_cohort_sha256": args.expected_build_cohort,
            "mcp_identity": identities["mcp"],
            "runtime_identity": identities["runtime"],
        },
        "mandatory_ponytail_preflight": "PASS_EACH_RUNTIME_SESSION",
        "request": request,
        "calibration": first,
        "restart_readback": {
            "calibration": restart,
            "canonical_hash_equal": True,
        },
        "read_only_integrity": {
            "sqlite_before": sqlite_before,
            "sqlite_after": sqlite_after,
            "sqlite_unchanged": True,
            "cas_before": cas_before,
            "cas_after": cas_after,
            "cas_unchanged": True,
        },
        "non_promotion_boundary": {
            "quality_status": "QUALITY_TARGET_NOT_MET",
            "form_quality_v2_status": "NOT_CREATED",
            "candidate_confirm_allowed": False,
            "secondary_form_approved": "NOT_CREATED",
            "production_stage_advanced": False,
            "candidate_confirmed": False,
            "version_created": False,
            "export_performed": False,
            "high_low_uv_bake_started": False,
            "human_review": "NOT_RUN",
            "engine_validation": "NOT_RUN",
            "commercial_quality": "NOT_PROVEN",
        },
    }
    evidence_path.parent.mkdir(parents=True, exist_ok=True)
    evidence_path.write_text(
        json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(receipt, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(f"04BE-G probe failed: {error}")
        raise SystemExit(1)
