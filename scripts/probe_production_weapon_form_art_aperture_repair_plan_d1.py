#!/usr/bin/env python3
"""Derive and restart-verify the exact 04BE-H aperture repair plan."""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path

from probe_mcp010b_raw_stdio import GateFailure, build_identity
from probe_production_weapon_form_art_failure_diagnostic_d1 import (
    canonical_hash,
    cas_snapshot,
    close_client,
    open_client,
    require,
    sqlite_snapshot,
)
from probe_production_weapon_form_art_visibility_calibration_d1 import (
    calibration_request,
)

CALIBRATION_CANONICAL = "3d3cd762570a8901b4301cd0287a76a8ad20382fb18a638b4ec3bb32212e7196"
DERIVATION_POLICY = "exact-raster-calibrated-sequential-aperture-sensitivity-plan@1"
CANONICALIZATION_POLICY = "canonical-json-sha256-excluding-input-sha256@1"
MAX_RESPONSE_BYTES = 1_048_576


def aperture_plan_request() -> dict[str, object]:
    calibration = calibration_request()
    value: dict[str, object] = {
        "schema_version": "ProductionWeaponFormArtApertureRepairPlanGetRequest@1",
        "operation": "forgecad.production.weapon.form-art-aperture-repair-plan-get@1",
        "aperture_repair_plan_id": "fps-form-04be-h-aperture-repair-plan-v1",
        "visibility_calibration_id": calibration["calibration_id"],
        "visibility_calibration_canonical_sha256": CALIBRATION_CANONICAL,
        "visibility_calibration_input_sha256": calibration["input_sha256"],
        "failure_diagnostic_id": calibration["failure_diagnostic_id"],
        "failure_diagnostic_canonical_sha256": calibration[
            "failure_diagnostic_canonical_sha256"
        ],
        "failure_diagnostic_input_sha256": calibration[
            "failure_diagnostic_input_sha256"
        ],
        "composite_evidence_id": calibration["composite_evidence_id"],
        "proposal_id": calibration["proposal_id"],
        "session_id": calibration["session_id"],
        "project_id": calibration["project_id"],
        "composite_evidence_record_canonical_sha256": calibration[
            "composite_evidence_record_canonical_sha256"
        ],
        "composite_evidence_receipt_object_sha256": calibration[
            "composite_evidence_receipt_object_sha256"
        ],
        "cross_view_evidence_bundle_sha256": calibration[
            "cross_view_evidence_bundle_sha256"
        ],
        "proposal_form_art_evidence_receipt_object_sha256": calibration[
            "proposal_form_art_evidence_receipt_object_sha256"
        ],
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "derivation_policy": DERIVATION_POLICY,
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
    request = aperture_plan_request()
    runs: list[dict[str, object]] = []
    with tempfile.TemporaryDirectory(prefix="forgecad-04be-h-", dir="/tmp") as temporary:
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
                    "production_weapon_form_art_aperture_repair_plan_get" in names,
                    "04BE-H aperture repair-plan tool is not exposed",
                )
                runs.append(
                    client.tool(
                        "production_weapon_form_art_aperture_repair_plan_get", request
                    )
                )
            finally:
                close_client(runtime, ready_path, ready, client)

    sqlite_after = sqlite_snapshot(database)
    cas_after = cas_snapshot(cas_root)
    first, restart = runs
    require(
        first.get("canonical_sha256") == restart.get("canonical_sha256"),
        "04BE-H restart canonical hash differs",
    )
    require(sqlite_before == sqlite_after, "04BE-H read-only plan changed SQLite")
    require(cas_before == cas_after, "04BE-H read-only plan changed CAS")
    require(
        first.get("plan_status")
        == "READY_HASH_BOUND_SEQUENTIAL_TWO_PART_APERTURE_SENSITIVITY_PLAN"
        and first.get("next_trial_registration_authorized") is True
        and first.get("repair_execution_allowed_by_this_tool") is False
        and first.get("geometry_repair_performed") is False
        and first.get("quality_status") == "QUALITY_TARGET_NOT_MET"
        and first.get("form_quality_v2_status") == "NOT_CREATED"
        and first.get("production_stage_advanced") is False
        and first.get("runtime_write_performed") is False
        and first.get("persistent_user_data_touched") is False
        and first.get("worker_started") is True,
        "04BE-H non-promotion boundary differs",
    )
    bindings = first.get("calibrated_source_bindings", [])
    require(
        isinstance(bindings, list)
        and [(row.get("view_kind"), row.get("source_node_id")) for row in bindings]
        == [("left", "side-panel-a"), ("right", "receiver-upper")]
        and [row.get("primary_visible_pixel_count") for row in bindings] == [175, 186]
        and [row.get("expected_void_pixel_count") for row in bindings] == [238, 257],
        "04BE-H calibrated source bindings differ",
    )
    steps = first.get("plan_steps", [])
    require(
        isinstance(steps, list)
        and [step.get("sequence") for step in steps] == [1, 2]
        and [step.get("source_node_id") for step in steps]
        == ["side-panel-a", "receiver-upper"]
        and all(len(step.get("trial_variants", [])) == 4 for step in steps)
        and steps[0].get("dependency") is None
        and steps[1].get("dependency", {}).get("required_status")
        == "RETAINED_SIX_VIEW_NON_REGRESSING_APERTURE_RESPONSE",
        "04BE-H sequential step contract differs",
    )

    receipt = {
        "schema_version": "ForgeCADProductionWeaponFormArtApertureRepairPlanRealD1Gate@1",
        "task_id": "FPS-FORM-04BE-H",
        "recorded_on": "2026-08-28",
        "status": "PASS_READ_ONLY_HASH_BOUND_SEQUENTIAL_TWO_PART_APERTURE_PLAN",
        "build": {
            "build_cohort_sha256": args.expected_build_cohort,
            "mcp_identity": identities["mcp"],
            "runtime_identity": identities["runtime"],
        },
        "mandatory_ponytail_preflight": "PASS_EACH_RUNTIME_SESSION",
        "request": request,
        "aperture_repair_plan": first,
        "restart_readback": {
            "aperture_repair_plan": restart,
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
            "geometry_repair_performed": False,
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
        print(f"04BE-H probe failed: {error}")
        raise SystemExit(1)
