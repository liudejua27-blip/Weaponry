#!/usr/bin/env python3
"""Compose the proven receiver opening with four bounded side-panel apertures."""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path
from typing import Any

import probe_production_weapon_form_art_aperture_trials_d1 as aperture
from probe_mcp010b_raw_stdio import GateFailure, build_identity
from probe_production_weapon_form_art_repair_execution_d1 import (
    CANONICALIZATION_POLICY,
    MAX_RESPONSE_BYTES,
    PROJECT_ID,
    SESSION_ID,
    WRITER_POLICY,
    canonical_hash,
    close_client,
    open_client,
    read_cas_json,
    require,
    with_input_hash,
)

TASK_ID = "FPS-FORM-04BE-T"
STAGE_SLUG = "04be-t"
RECORD_REV = "v1"
RECEIVER_PROFILE = "receiver-upper-camera-target-notch-calibrated@2"
VARIANTS = (
    ("layered-narrow", "side-panel-a-camera-mapped-aperture-narrow@2"),
    ("layered-calibrated", "side-panel-a-camera-mapped-aperture-calibrated@2"),
    ("layered-raised", "side-panel-a-camera-mapped-aperture-raised@2"),
    ("layered-wide", "side-panel-a-camera-mapped-aperture-wide@2"),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--expected-build-cohort", required=True)
    parser.add_argument("--timeout", type=float, default=360.0)
    parser.add_argument("--task-id", default=TASK_ID)
    parser.add_argument("--stage-slug", default=STAGE_SLUG)
    parser.add_argument("--record-rev", default=RECORD_REV)
    parser.add_argument(
        "--selection-policy",
        choices=("legacy-exact", "bounded-core-0.01"),
        default="legacy-exact",
    )
    return parser.parse_args()


def registered_operation(
    sequence_index: int, slug: str, source_node_id: str, part_id: str, profile_id: str
) -> dict[str, Any]:
    value = {
        "schema_version": "ProductionWeaponFormArtCompositeProposalOperation@1",
        "sequence_index": sequence_index,
        "operation_id": f"operation-{part_id}-{slug}-{STAGE_SLUG}",
        "operation_kind": "registered_profile_replace",
        "source_node_id": source_node_id,
        "part_id": part_id,
        "registered_profile_id": profile_id,
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def proposal_request(slug: str, side_panel_profile: str, baseline_sha256: str) -> dict[str, Any]:
    plan = {
        "schema_version": "ProductionWeaponFormArtCompositeProposalPlan@1",
        "project_id": PROJECT_ID,
        "original_source_candidate_id": aperture.ORIGINAL_CANDIDATE_ID,
        "original_source_candidate_state_sha256": aperture.ORIGINAL_CANDIDATE_STATE,
        "original_source_artifact_sha256": aperture.ORIGINAL_ARTIFACT,
        "original_fresh_baseline_canonical_sha256": baseline_sha256,
        "current_base_candidate_id": aperture.CURRENT_CANDIDATE_ID,
        "current_base_candidate_state_sha256": aperture.CURRENT_CANDIDATE_STATE,
        "current_base_artifact_sha256": aperture.CURRENT_ARTIFACT,
        "current_base_geometry_program_sha256": aperture.CURRENT_PROGRAM,
        "current_base_proposal_evidence_sha256": aperture.CURRENT_PROPOSAL_EVIDENCE,
        "operations": [
            registered_operation(0, slug, "receiver-upper", "receiver-upper", RECEIVER_PROFILE),
            registered_operation(1, slug, "side-panel-a", "side-panel-a", side_panel_profile),
        ],
        "composition_policy": "runtime-owned-original-baseline-current-base-registered-disjoint-replacements@1",
    }
    plan["canonical_sha256"] = canonical_hash(plan)
    return with_input_hash(
        {
            "schema_version": "ProductionWeaponFormArtCompositeProposalPrepareRequest@1",
            "proposal_id": f"fps-form-{STAGE_SLUG}-layered-aperture-{slug}-{RECORD_REV}",
            "session_id": SESSION_ID,
            "project_id": PROJECT_ID,
            "original_fresh_baseline_id": aperture.BASELINE_ID,
            "plan": plan,
            "idempotency_key": f"fps-form-{STAGE_SLUG}-layered-aperture-{slug}-prepare-{RECORD_REV}",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": False,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": CANONICALIZATION_POLICY,
        }
    )


def main() -> int:
    global TASK_ID, STAGE_SLUG, RECORD_REV
    args = parse_args()
    TASK_ID = args.task_id
    STAGE_SLUG = args.stage_slug
    RECORD_REV = args.record_rev
    repository = Path(__file__).resolve().parents[1]
    evidence_path = args.evidence if args.evidence.is_absolute() else repository / args.evidence
    evidence_path.resolve().relative_to((repository / "docs" / "evidence").resolve())
    aperture.STAGE_SLUG = STAGE_SLUG
    aperture.TASK_ID = TASK_ID
    # Reuse the exact immutable same-cohort baseline already materialized by
    # 04BE-R. The store intentionally forbids a second identity for the same
    # original candidate/cohort.
    aperture.BASELINE_ID = f"fps-form-04be-r-final-baseline-{args.expected_build_cohort[:12]}"
    aperture.RECORD_REV = RECORD_REV
    aperture.TARGET_PART_ID = "layered-aperture"
    aperture.TARGET_VIEW_KIND = "right"
    aperture.TARGET_STRUCTURE_ID = "right.trigger-void"

    identities = {"mcp": build_identity(args.mcp), "runtime": build_identity(args.runtime)}
    require(
        all(value.get("build_cohort_sha256") == args.expected_build_cohort for value in identities.values()),
        "Runtime/MCP build cohort differs",
    )
    trials: list[dict[str, Any]] = []
    restart_requests: list[tuple[dict[str, Any], dict[str, Any]]] = []
    with tempfile.TemporaryDirectory(prefix="forgecad-04be-t-", dir="/tmp") as temporary:
        temp = Path(temporary)
        runtime, ready_path, ready, client = open_client(
            args.mcp, args.runtime, args.data_root, temp / "execute", args.timeout
        )
        try:
            baseline = client.tool("production_weapon_form_art_baseline_prepare", aperture.baseline_request())[
                "baseline"
            ]
            baseline_sha256 = baseline["canonical_sha256"]
            for slug, side_panel_profile in VARIANTS:
                proposal = client.tool(
                    "production_weapon_form_art_composite_proposal_prepare",
                    proposal_request(slug, side_panel_profile, baseline_sha256),
                )
                candidate = proposal["reviewable_candidate"]
                readback = client.tool(
                    "artifact_readback_get",
                    {"artifact_id": candidate["artifact_sha256"], "candidate_id": candidate["candidate_id"]},
                )
                require(
                    readback.get("hard_gate_passed") is True and readback.get("validator_status") == "passed",
                    f"{TASK_ID} strict GLB readback failed: {slug}",
                )
                evidence_request = aperture.evidence_request(slug, proposal, readback, baseline_sha256)
                evidence = client.tool(
                    "production_weapon_form_art_composite_evidence_prepare", evidence_request
                )
                form_art = read_cas_json(
                    args.data_root, evidence["proposal_form_art_evidence_receipt_object_sha256"]
                )
                cross_view = read_cas_json(args.data_root, evidence["cross_view_evidence_bundle_sha256"])
                secondary_form_gate = evidence.get("evaluation", {}).get("secondary_form_gate", {})
                trials.append(
                    {
                        "variant_id": slug,
                        "receiver_profile_id": RECEIVER_PROFILE,
                        "side_panel_profile_id": side_panel_profile,
                        "proposal": proposal,
                        "evidence": evidence,
                        "artifact_readback": {
                            "canonical_sha256": readback["canonical_sha256"],
                            "hard_gate_passed": True,
                            "validator_status": "passed",
                        },
                        "cross_view": {
                            "canonical_sha256": cross_view["canonical_sha256"],
                            "non_regressing": cross_view["non_regressing"],
                            "strict_improvement": cross_view["strict_improvement"],
                            "baseline_score": cross_view["baseline_score"],
                            "proposal_score": cross_view["proposal_score"],
                            "promotion": cross_view["promotion"],
                        },
                        "bounded_secondary_form_gate": {
                            "policy": secondary_form_gate.get("policy"),
                            "policy_sha256": secondary_form_gate.get("policy_sha256"),
                            "max_core_tradeoff_ppm": secondary_form_gate.get(
                                "policy_definition", {}
                            ).get("max_core_tradeoff_ppm"),
                            "semantic_non_regressing": secondary_form_gate.get(
                                "semantic_non_regressing"
                            ),
                            "bounded_core_tradeoff": secondary_form_gate.get(
                                "bounded_core_tradeoff"
                            ),
                            "aggregate_improved": secondary_form_gate.get("aggregate_improved"),
                            "strict_primary_improvement": secondary_form_gate.get(
                                "strict_primary_improvement"
                            ),
                            "reviewable_tradeoff": secondary_form_gate.get(
                                "reviewable_tradeoff"
                            ),
                            "proposal_form_art_evidence_ready": secondary_form_gate.get(
                                "proposal_form_art_evidence_ready"
                            ),
                            "status": secondary_form_gate.get("status"),
                        },
                        "target_trigger_void": aperture.target_trigger_row(form_art),
                    }
                )
                restart_requests.append(
                    (aperture.proposal_get_request(slug), aperture.evidence_get_request(evidence_request))
                )
        finally:
            close_client(runtime, ready_path, ready, client)

        runtime, ready_path, ready, client = open_client(
            args.mcp, args.runtime, args.data_root, temp / "restart", args.timeout
        )
        try:
            for trial, (proposal_get, evidence_get) in zip(trials, restart_requests):
                proposal_restart = client.tool(
                    "production_weapon_form_art_composite_proposal_get", proposal_get
                )
                evidence_restart = client.tool(
                    "production_weapon_form_art_composite_evidence_get", evidence_get
                )
                require(
                    proposal_restart["record_canonical_sha256"]
                    == trial["proposal"]["record_canonical_sha256"]
                    and evidence_restart["record_canonical_sha256"]
                    == trial["evidence"]["record_canonical_sha256"],
                    "layered aperture restart readback differs",
                )
                trial["restart_readback"] = {"exact_hashes_equal": True}
        finally:
            close_client(runtime, ready_path, ready, client)

    if args.selection_policy == "bounded-core-0.01":
        eligible = [
            trial
            for trial in trials
            if trial["bounded_secondary_form_gate"]["reviewable_tradeoff"] is True
            and trial["bounded_secondary_form_gate"]["max_core_tradeoff_ppm"] == 10_000
            and trial["target_trigger_void"]["sealed"] is False
        ]
    else:
        eligible = [
            trial
            for trial in trials
            if trial["cross_view"]["non_regressing"] is True
            and trial["target_trigger_void"]["sealed"] is False
        ]
    selected = max(
        eligible,
        key=lambda trial: (
            trial["cross_view"]["proposal_score"],
            trial["target_trigger_void"].get("iou_milli", 0),
        ),
        default=None,
    )
    for trial in trials:
        trial["decision"] = {
            "status": (
                "SELECTED_FOR_NEXT_FORM_REPAIR"
                if trial is selected and args.selection_policy == "bounded-core-0.01"
                else "SELECTED_FOR_HUMAN_REVIEW"
                if trial is selected
                else "REJECTED_RETAIN_PARENT"
            ),
            "target_open": trial["target_trigger_void"]["sealed"] is False,
            "six_view_non_regressing": trial["cross_view"]["non_regressing"],
            "bounded_core_tradeoff_reviewable": trial["bounded_secondary_form_gate"][
                "reviewable_tradeoff"
            ],
        }
    selected_candidate = (
        selected["proposal"]["reviewable_candidate"] if selected is not None else None
    )
    receipt = {
        "schema_version": "ForgeCADProductionWeaponFormArtLayeredApertureRealD1Gate@1",
        "task_id": TASK_ID,
        "recorded_on": "2026-08-29",
        "status": (
            "PASS_LAYERED_APERTURE_CANDIDATE_SELECTED_FOR_NEXT_FORM_REPAIR"
            if selected is not None and args.selection_policy == "bounded-core-0.01"
            else "PASS_LAYERED_APERTURE_CANDIDATE_SELECTED_FOR_HUMAN_REVIEW"
            if selected is not None
            else "PASS_LAYERED_APERTURE_TRIALS_REJECTED_PARENT_RETAINED"
        ),
        "build": {"build_cohort_sha256": args.expected_build_cohort, **identities},
        "mandatory_ponytail_preflight": "PASS",
        "selection_policy": args.selection_policy,
        "fresh_original_baseline": baseline,
        "trials": trials,
        "selection": {
            "eligible_trial_count": len(eligible),
            "selected_candidate_id": (
                selected_candidate["candidate_id"]
                if selected_candidate is not None
                else aperture.CURRENT_CANDIDATE_ID
            ),
            "selected_candidate_state_sha256": (
                selected_candidate["candidate_state_sha256"]
                if selected_candidate is not None
                else aperture.CURRENT_CANDIDATE_STATE
            ),
            "status": (
                "SELECTED_FOR_NEXT_FORM_REPAIR"
                if selected is not None and args.selection_policy == "bounded-core-0.01"
                else "SELECTED_FOR_HUMAN_REVIEW"
                if selected is not None
                else "RETAINED_PARENT"
            ),
            "candidate_confirmed": False,
            "next_atomic_action": (
                "CONTINUE_FORMART_NEGATIVE_SPACE_OWNER_LINE_FLOW_ON_SELECTED_CANDIDATE"
                if selected is not None and args.selection_policy == "bounded-core-0.01"
                else "HUMAN_REVIEW_SELECTED_LAYERED_APERTURE_CANDIDATE"
                if selected is not None
                else "RUN_LAYERED_TARGET_OCCLUSION_ATTRIBUTION"
            ),
        },
        "non_promotion_boundary": {
            "candidate_confirmed": False,
            "version_created": False,
            "production_stage_advanced": False,
            "form_quality_v2_status": "NOT_CREATED",
            "quality_status": "QUALITY_TARGET_NOT_MET",
            "appearance_uv_pbr_write_authorized": False,
            "core_raster_absolute_tolerance": 0.01,
            "tolerance_does_not_apply_to": [
                "semantic_metrics",
                "topology",
                "hash_lineage",
                "uv_overlap",
                "bake_miss_cross_hit",
                "human_approval",
            ],
        },
    }
    evidence_path.parent.mkdir(parents=True, exist_ok=True)
    evidence_path.write_text(
        json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        json.dumps(
            {
                "task_id": TASK_ID,
                "status": receipt["status"],
                "selection": receipt["selection"],
                "trial_decisions": [
                    {
                        "variant_id": trial["variant_id"],
                        "candidate_id": trial["proposal"]["reviewable_candidate"]["candidate_id"],
                        "sealed": trial["target_trigger_void"]["sealed"],
                        "iou_milli": trial["target_trigger_void"].get("iou_milli"),
                        "non_regressing": trial["cross_view"]["non_regressing"],
                        "decision": trial["decision"]["status"],
                    }
                    for trial in trials
                ],
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(f"layered aperture probe failed: {error}")
        raise SystemExit(1)
