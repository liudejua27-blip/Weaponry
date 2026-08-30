#!/usr/bin/env python3
"""Run and restart-verify the exact 04BE-M target occlusion attribution."""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path
from typing import Any

from probe_mcp010b_raw_stdio import GateFailure, build_identity
from probe_production_weapon_form_art_failure_diagnostic_d1 import (
    canonical_hash,
    cas_snapshot,
    close_client,
    open_client,
    require,
    sqlite_snapshot,
)

POLICY = "exact-parent-closed-receiver-upper-family-right-trigger-void-attribution@1"
CANONICALIZATION = "canonical-json-sha256-excluding-input-sha256@1"
MAX_RESPONSE_BYTES = 1_048_576


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--l-evidence", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--expected-build-cohort", required=True)
    parser.add_argument("--timeout", type=float, default=240.0)
    return parser.parse_args()


def request_from_l_receipt(receipt: dict[str, Any]) -> dict[str, Any]:
    parent = receipt["parent"]
    trials = []
    for trial in receipt["trials"]:
        proposal = trial["proposal"]
        evidence = trial["evidence"]
        candidate = proposal["reviewable_candidate"]
        trials.append(
            {
                "registered_profile_id": trial["registered_profile_id"],
                "proposal_id": proposal["proposal_id"],
                "composite_evidence_id": evidence["composite_evidence_id"],
                "candidate_id": candidate["candidate_id"],
                "candidate_state_sha256": candidate["candidate_state_sha256"],
                "artifact_sha256": candidate["artifact_sha256"],
                "form_art_evidence_receipt_object_sha256": evidence[
                    "proposal_form_art_evidence_receipt_object_sha256"
                ],
            }
        )
    value: dict[str, Any] = {
        "schema_version": "ProductionWeaponFormArtTargetOcclusionAttributionGetRequest@1",
        "operation": "forgecad.production.weapon.form-art-target-occlusion-attribution-get@1",
        "attribution_id": f"{receipt['task_id'].lower()}-right-trigger-void-attribution-v1",
        "project_id": receipt["fresh_original_baseline"]["project_id"],
        "session_id": receipt["fresh_original_baseline"]["session_id"],
        "parent": {
            "candidate_id": parent["candidate_id"],
            "candidate_state_sha256": parent["candidate_state_sha256"],
            "artifact_sha256": parent["artifact_sha256"],
            "form_art_evidence_receipt_object_sha256": parent[
                "form_art_evidence_receipt_object_sha256"
            ],
        },
        "trials": trials,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "attribution_policy": POLICY,
        "canonicalization_policy": CANONICALIZATION,
    }
    value["input_sha256"] = canonical_hash(value)
    return value


def main() -> int:
    args = parse_args()
    repository = Path(__file__).resolve().parents[1]
    l_path = args.l_evidence if args.l_evidence.is_absolute() else repository / args.l_evidence
    evidence_path = args.evidence if args.evidence.is_absolute() else repository / args.evidence
    evidence_path.resolve().relative_to((repository / "docs" / "evidence").resolve())
    l_receipt = json.loads(l_path.read_text(encoding="utf-8"))
    require(
        l_receipt.get("task_id") in {"FPS-FORM-04BE-L", "FPS-FORM-04BE-N", "FPS-FORM-04BE-P", "FPS-FORM-04BE-R"},
        "closed receiver-upper trial receipt differs",
    )
    task_id = {
        "FPS-FORM-04BE-L": "FPS-FORM-04BE-M",
        "FPS-FORM-04BE-N": "FPS-FORM-04BE-O",
        "FPS-FORM-04BE-P": "FPS-FORM-04BE-Q",
        "FPS-FORM-04BE-R": "FPS-FORM-04BE-S",
    }[l_receipt["task_id"]]

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
    sqlite_before = sqlite_snapshot(database)
    cas_before = cas_snapshot(cas_root)
    request = request_from_l_receipt(l_receipt)

    runs: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="forgecad-04be-m-", dir="/tmp") as temporary:
        temp = Path(temporary)
        for index in range(2):
            runtime, ready_path, ready, client = open_client(
                args.mcp,
                args.runtime,
                args.data_root,
                temp / f"run-{index + 1}",
                args.timeout,
            )
            try:
                names = {
                    item.get("name")
                    for item in client.request("tools/list").get("result", {}).get("tools", [])
                    if isinstance(item, dict)
                }
                require(
                    "production_weapon_form_art_target_occlusion_attribution_get" in names,
                    "04BE-M attribution tool is not exposed",
                )
                runs.append(
                    client.tool(
                        "production_weapon_form_art_target_occlusion_attribution_get",
                        request,
                    )
                )
            finally:
                close_client(runtime, ready_path, ready, client)

    first, restart = runs
    sqlite_after = sqlite_snapshot(database)
    cas_after = cas_snapshot(cas_root)
    require(first.get("canonical_sha256") == restart.get("canonical_sha256"), "restart hash differs")
    require(sqlite_before == sqlite_after, "read-only attribution changed SQLite")
    require(cas_before == cas_after, "read-only attribution changed CAS")
    require(
        first.get("quality_status") == "QUALITY_TARGET_NOT_MET"
        and first.get("form_quality_v2_status") == "NOT_CREATED"
        and first.get("appearance_uv_pbr_write_authorized") is False
        and first.get("topology_stage_unlocked") is False
        and first.get("runtime_write_performed") is False
        and first.get("persistent_user_data_touched") is False
        and first.get("production_stage_advanced") is False,
        "non-promotion boundary differs",
    )
    require(len(first.get("trials", [])) == 4, "trial attribution count differs")

    receipt = {
        "schema_version": "ForgeCADProductionWeaponFormArtTargetOcclusionAttributionRealD1Gate@1",
        "task_id": task_id,
        "recorded_on": "2026-08-29",
        "status": "PASS_READ_ONLY_EXACT_TARGET_OCCLUSION_ATTRIBUTION",
        "build": {
            "build_cohort_sha256": args.expected_build_cohort,
            "mcp_identity": identities["mcp"],
            "runtime_identity": identities["runtime"],
        },
        "mandatory_ponytail_preflight": "PASS_EACH_RUNTIME_SESSION",
        "source_receipt": {
            "task_id": l_receipt["task_id"],
            "status": l_receipt["status"],
            "path": str(l_path.relative_to(repository)) if l_path.is_relative_to(repository) else l_path.name,
        },
        "request": request,
        "attribution": first,
        "restart_readback": {
            "canonical_sha256": restart["canonical_sha256"],
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
            "topology_stage_unlocked": False,
            "appearance_uv_pbr_write_authorized": False,
            "candidate_confirmed": False,
            "production_stage_advanced": False,
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
        print(f"04BE-M probe failed: {error}")
        raise SystemExit(1)
