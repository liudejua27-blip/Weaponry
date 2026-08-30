#!/usr/bin/env python3
"""Execute one registered single-Part aperture trial family."""

from __future__ import annotations

import argparse
import json
import tempfile
from pathlib import Path
from typing import Any

from probe_mcp010b_raw_stdio import GateFailure, build_identity
from probe_production_weapon_form_art_repair_execution_d1 import (
    CANONICALIZATION_POLICY,
    MAX_RESPONSE_BYTES,
    ORIGINAL_ARTIFACT,
    ORIGINAL_ARTIFACT_ID,
    ORIGINAL_CANDIDATE_ID,
    ORIGINAL_CANDIDATE_STATE,
    PROJECT_ID,
    REGISTRATION_LINEAGE_CANONICAL,
    REGISTRATION_LINEAGE_ID,
    SESSION_ID,
    SOURCE_FORM_ART_CANONICAL,
    SOURCE_FORM_ART_ID,
    SOURCE_FORM_ART_OBJECT,
    WRITER_POLICY,
    close_client,
    open_client,
    read_cas_json,
    require,
    with_input_hash,
)

CURRENT_CANDIDATE_ID = "candidate-6f6ddeff15b94d5db9eb74d6c639cf8a"
CURRENT_CANDIDATE_STATE = "1a0bf325f55d2cffa6924d35dbbfa46c8d1142e35fad4850ac6e2c3d56f260d2"
CURRENT_ARTIFACT = "1039baef457832e97c0facd3a51e19834e08f31024c9a700ad52bdbf0e615c80"
CURRENT_PROGRAM = "a9d447e51ddc510541568a3fa1aed86b052514f459b50168c455ca4fe38e7f11"
CURRENT_PROPOSAL_EVIDENCE = "0af3ded026923d2612560ba3e446e8c777887a3a4c2436385e83e8a110d15bff"
STAGE_SLUG = "04be-i"
TASK_ID = "FPS-FORM-04BE-I"
BASELINE_ID = "fps-form-04be-i-final-baseline-r10"
RECORD_REV = "v2"
TARGET_SOURCE_NODE_ID = "side-panel-a"
TARGET_PART_ID = "side-panel-a"
TARGET_VIEW_KIND = "left"
TARGET_STRUCTURE_ID = "left.trigger-void"
VARIANTS = (
    ("retract-min-x-20mm", "side-panel-a-retract-min-x-20mm@1"),
    ("retract-max-x-20mm", "side-panel-a-retract-max-x-20mm@1"),
    ("retract-min-x-40mm", "side-panel-a-retract-min-x-40mm@1"),
    ("retract-max-x-40mm", "side-panel-a-retract-max-x-40mm@1"),
)


def baseline_request() -> dict[str, Any]:
    return with_input_hash(
        {
            "schema_version": "ProductionWeaponFormArtBaselinePrepareRequest@1",
            "operation": "forgecad.production.weapon.form-art-baseline-prepare@1",
            "baseline_id": BASELINE_ID,
            "registration_lineage_id": REGISTRATION_LINEAGE_ID,
            "registration_lineage_canonical_sha256": REGISTRATION_LINEAGE_CANONICAL,
            "session_id": SESSION_ID,
            "project_id": PROJECT_ID,
            "candidate_id": ORIGINAL_CANDIDATE_ID,
            "candidate_state_sha256": ORIGINAL_CANDIDATE_STATE,
            "artifact_id": ORIGINAL_ARTIFACT_ID,
            "artifact_sha256": ORIGINAL_ARTIFACT,
            "base_version_id": None,
            "idempotency_key": f"{BASELINE_ID}-prepare",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
            "runtime_write_performed": False,
            "persistent_user_data_touched": False,
        }
    )


def operation(slug: str, profile_id: str) -> dict[str, Any]:
    value = {
        "schema_version": "ProductionWeaponFormArtCompositeProposalOperation@1",
        "sequence_index": 0,
        "operation_id": f"operation-{TARGET_PART_ID}-{slug}-{STAGE_SLUG}",
        "operation_kind": "registered_profile_replace",
        "source_node_id": TARGET_SOURCE_NODE_ID,
        "part_id": TARGET_PART_ID,
        "registered_profile_id": profile_id,
    }
    from probe_production_weapon_form_art_repair_execution_d1 import canonical_hash

    value["canonical_sha256"] = canonical_hash(value)
    return value


def proposal_request(slug: str, profile_id: str, baseline_sha: str) -> dict[str, Any]:
    from probe_production_weapon_form_art_repair_execution_d1 import canonical_hash

    plan = {
        "schema_version": "ProductionWeaponFormArtCompositeProposalPlan@1",
        "project_id": PROJECT_ID,
        "original_source_candidate_id": ORIGINAL_CANDIDATE_ID,
        "original_source_candidate_state_sha256": ORIGINAL_CANDIDATE_STATE,
        "original_source_artifact_sha256": ORIGINAL_ARTIFACT,
        "original_fresh_baseline_canonical_sha256": baseline_sha,
        "current_base_candidate_id": CURRENT_CANDIDATE_ID,
        "current_base_candidate_state_sha256": CURRENT_CANDIDATE_STATE,
        "current_base_artifact_sha256": CURRENT_ARTIFACT,
        "current_base_geometry_program_sha256": CURRENT_PROGRAM,
        "current_base_proposal_evidence_sha256": CURRENT_PROPOSAL_EVIDENCE,
        "operations": [operation(slug, profile_id)],
        "composition_policy": "runtime-owned-original-baseline-current-base-registered-disjoint-replacements@1",
    }
    plan["canonical_sha256"] = canonical_hash(plan)
    return with_input_hash(
        {
            "schema_version": "ProductionWeaponFormArtCompositeProposalPrepareRequest@1",
            "proposal_id": f"fps-form-{STAGE_SLUG}-{TARGET_PART_ID}-{slug}-{RECORD_REV}",
            "session_id": SESSION_ID,
            "project_id": PROJECT_ID,
            "original_fresh_baseline_id": BASELINE_ID,
            "plan": plan,
            "idempotency_key": f"fps-form-{STAGE_SLUG}-{TARGET_PART_ID}-{slug}-prepare-{RECORD_REV}",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": False,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": CANONICALIZATION_POLICY,
        }
    )


def evidence_request(
    slug: str, proposal: dict[str, Any], readback: dict[str, Any], baseline_sha: str
) -> dict[str, Any]:
    candidate = proposal["reviewable_candidate"]
    lineage = proposal["original_current_final_lineage"]
    artifact_sha = candidate["artifact_sha256"]
    return with_input_hash(
        {
            "schema_version": "ProductionWeaponFormArtCompositeEvidencePrepareRequest@1",
            "operation": "forgecad.production.weapon.form-art-composite-evidence-prepare@1",
            "composite_evidence_id": f"fps-form-{STAGE_SLUG}-{TARGET_PART_ID}-{slug}-evidence-{RECORD_REV}",
            "proposal_id": f"fps-form-{STAGE_SLUG}-{TARGET_PART_ID}-{slug}-{RECORD_REV}",
            "session_id": SESSION_ID,
            "project_id": PROJECT_ID,
            "composite_proposal_record_canonical_sha256": proposal["record_canonical_sha256"],
            "composite_proposal_receipt_object_sha256": proposal["receipt_object_sha256"],
            "original_fresh_baseline_id": BASELINE_ID,
            "original_fresh_baseline_canonical_sha256": baseline_sha,
            "source_form_art_evidence_id": SOURCE_FORM_ART_ID,
            "source_form_art_evidence_object_sha256": SOURCE_FORM_ART_OBJECT,
            "source_form_art_evidence_canonical_sha256": SOURCE_FORM_ART_CANONICAL,
            "proposal_candidate_id": candidate["candidate_id"],
            "proposal_candidate_state_sha256": candidate["candidate_state_sha256"],
            "proposal_artifact_id": artifact_sha,
            "proposal_artifact_sha256": artifact_sha,
            "proposal_artifact_readback_object_sha256": lineage[
                "proposal_artifact_readback_object_sha256"
            ],
            "proposal_artifact_readback_sha256": readback["canonical_sha256"],
            "idempotency_key": f"fps-form-{STAGE_SLUG}-{TARGET_PART_ID}-{slug}-evidence-prepare-{RECORD_REV}",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": False,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": CANONICALIZATION_POLICY,
        }
    )


def proposal_get_request(slug: str) -> dict[str, Any]:
    return with_input_hash(
        {
            "schema_version": "ProductionWeaponFormArtCompositeProposalGetRequest@1",
            "project_id": PROJECT_ID,
            "proposal_id": f"fps-form-{STAGE_SLUG}-{TARGET_PART_ID}-{slug}-{RECORD_REV}",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": False,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": CANONICALIZATION_POLICY,
        }
    )


def evidence_get_request(request: dict[str, Any]) -> dict[str, Any]:
    value = dict(request)
    value["schema_version"] = "ProductionWeaponFormArtCompositeEvidenceGetRequest@1"
    value["operation"] = "forgecad.production.weapon.form-art-composite-evidence-get@1"
    value.pop("idempotency_key")
    value.pop("input_sha256")
    return with_input_hash(value)


def target_trigger_row(form_art: dict[str, Any]) -> dict[str, Any]:
    for view in form_art.get("views", []):
        if view.get("view_kind") == TARGET_VIEW_KIND:
            for row in view.get("negative_space_rows", []):
                if row.get("structure_id") == TARGET_STRUCTURE_ID:
                    return row
    raise GateFailure(f"{TARGET_STRUCTURE_ID} FormArt row is unavailable")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--expected-build-cohort", required=True)
    parser.add_argument(
        "--trial-family",
        choices=(
            "retraction",
            "true-aperture",
            "camera-mapped-aperture",
            "receiver-upper-retraction",
            "receiver-upper-target-notch",
            "receiver-upper-u-topology",
            "receiver-upper-camera-target-u-topology",
        ),
        default="retraction",
    )
    parser.add_argument("--timeout", type=float, default=360.0)
    return parser.parse_args()


def main() -> int:
    global STAGE_SLUG, TASK_ID, BASELINE_ID, RECORD_REV, VARIANTS
    global TARGET_SOURCE_NODE_ID, TARGET_PART_ID, TARGET_VIEW_KIND, TARGET_STRUCTURE_ID
    args = parse_args()
    if args.trial_family == "true-aperture":
        STAGE_SLUG = "04be-j"
        TASK_ID = "FPS-FORM-04BE-J"
        BASELINE_ID = f"fps-form-04be-j-final-baseline-{args.expected_build_cohort[:12]}"
        RECORD_REV = "v1"
        VARIANTS = (
            ("true-aperture-narrow", "side-panel-a-true-aperture-narrow@1"),
            ("true-aperture-calibrated", "side-panel-a-true-aperture-calibrated@1"),
            ("true-aperture-forward", "side-panel-a-true-aperture-forward@1"),
            ("true-aperture-wide", "side-panel-a-true-aperture-wide@1"),
        )
    elif args.trial_family == "camera-mapped-aperture":
        STAGE_SLUG = "04be-k"
        TASK_ID = "FPS-FORM-04BE-K"
        BASELINE_ID = f"fps-form-04be-k-final-baseline-{args.expected_build_cohort[:12]}"
        # v1 was intentionally abandoned after the Worker-axis audit proved
        # its local-V direction wrong. Never replay that immutable idempotency
        # lineage for the corrected camera-mapped implementation.
        RECORD_REV = "v2"
        VARIANTS = (
            ("camera-mapped-aperture-narrow", "side-panel-a-camera-mapped-aperture-narrow@2"),
            ("camera-mapped-aperture-calibrated", "side-panel-a-camera-mapped-aperture-calibrated@2"),
            ("camera-mapped-aperture-raised", "side-panel-a-camera-mapped-aperture-raised@2"),
            ("camera-mapped-aperture-wide", "side-panel-a-camera-mapped-aperture-wide@2"),
        )
    elif args.trial_family == "receiver-upper-retraction":
        STAGE_SLUG = "04be-l"
        TASK_ID = "FPS-FORM-04BE-L"
        BASELINE_ID = f"fps-form-04be-l-final-baseline-{args.expected_build_cohort[:12]}"
        RECORD_REV = "v1"
        TARGET_SOURCE_NODE_ID = "receiver-upper"
        TARGET_PART_ID = "receiver-upper"
        TARGET_VIEW_KIND = "right"
        TARGET_STRUCTURE_ID = "right.trigger-void"
        VARIANTS = (
            ("retract-min-x-20mm", "receiver-upper-retract-min-x-20mm@1"),
            ("retract-max-x-20mm", "receiver-upper-retract-max-x-20mm@1"),
            ("retract-min-x-40mm", "receiver-upper-retract-min-x-40mm@1"),
            ("retract-max-x-40mm", "receiver-upper-retract-max-x-40mm@1"),
        )
    elif args.trial_family == "receiver-upper-target-notch":
        STAGE_SLUG = "04be-n"
        TASK_ID = "FPS-FORM-04BE-N"
        BASELINE_ID = f"fps-form-04be-n-final-baseline-{args.expected_build_cohort[:12]}"
        RECORD_REV = "v1"
        TARGET_SOURCE_NODE_ID = "receiver-upper"
        TARGET_PART_ID = "receiver-upper"
        TARGET_VIEW_KIND = "right"
        TARGET_STRUCTURE_ID = "right.trigger-void"
        VARIANTS = (
            ("target-notch-narrow", "receiver-upper-target-notch-narrow@1"),
            ("target-notch-calibrated", "receiver-upper-target-notch-calibrated@1"),
            ("target-notch-raised", "receiver-upper-target-notch-raised@1"),
            ("target-notch-wide", "receiver-upper-target-notch-wide@1"),
        )
    elif args.trial_family == "receiver-upper-u-topology":
        STAGE_SLUG = "04be-p"
        TASK_ID = "FPS-FORM-04BE-P"
        BASELINE_ID = f"fps-form-04be-p-final-baseline-{args.expected_build_cohort[:12]}"
        RECORD_REV = "v1"
        TARGET_SOURCE_NODE_ID = "receiver-upper"
        TARGET_PART_ID = "receiver-upper"
        TARGET_VIEW_KIND = "right"
        TARGET_STRUCTURE_ID = "right.trigger-void"
        VARIANTS = (
            ("u-topology-narrow", "receiver-upper-target-notch-narrow@1"),
            ("u-topology-calibrated", "receiver-upper-target-notch-calibrated@1"),
            ("u-topology-raised", "receiver-upper-target-notch-raised@1"),
            ("u-topology-wide", "receiver-upper-target-notch-wide@1"),
        )
    elif args.trial_family == "receiver-upper-camera-target-u-topology":
        STAGE_SLUG = "04be-r"
        TASK_ID = "FPS-FORM-04BE-R"
        BASELINE_ID = f"fps-form-04be-r-final-baseline-{args.expected_build_cohort[:12]}"
        RECORD_REV = "v1"
        TARGET_SOURCE_NODE_ID = "receiver-upper"
        TARGET_PART_ID = "receiver-upper"
        TARGET_VIEW_KIND = "right"
        TARGET_STRUCTURE_ID = "right.trigger-void"
        VARIANTS = (
            ("camera-target-u-narrow", "receiver-upper-camera-target-notch-narrow@2"),
            ("camera-target-u-calibrated", "receiver-upper-camera-target-notch-calibrated@2"),
            ("camera-target-u-raised", "receiver-upper-camera-target-notch-raised@2"),
            ("camera-target-u-wide", "receiver-upper-camera-target-notch-wide@2"),
        )
    identities = {"mcp": build_identity(args.mcp), "runtime": build_identity(args.runtime)}
    require(
        all(item.get("build_cohort_sha256") == args.expected_build_cohort for item in identities.values()),
        "Runtime/MCP build cohort differs",
    )
    repository = Path(__file__).resolve().parents[1]
    evidence_path = args.evidence if args.evidence.is_absolute() else repository / args.evidence
    evidence_path.resolve().relative_to((repository / "docs" / "evidence").resolve())
    trials: list[dict[str, Any]] = []
    restart_inputs: list[tuple[str, dict[str, Any], dict[str, Any]]] = []
    with tempfile.TemporaryDirectory(prefix=f"forgecad-{STAGE_SLUG}-", dir="/tmp") as temporary:
        runtime, ready_path, ready, client = open_client(
            args.mcp, args.runtime, args.data_root, Path(temporary) / "execute", args.timeout
        )
        try:
            baseline_result = client.tool("production_weapon_form_art_baseline_prepare", baseline_request())
            baseline = baseline_result["baseline"]
            require(
                baseline.get("runtime_build_cohort_sha256") == args.expected_build_cohort
                and len(baseline.get("views", [])) == 6,
                f"{TASK_ID} fresh same-cohort baseline failed",
            )
            baseline_sha = baseline["canonical_sha256"]
            for slug, profile_id in VARIANTS:
                proposal = client.tool(
                    "production_weapon_form_art_composite_proposal_prepare",
                    proposal_request(slug, profile_id, baseline_sha),
                )
                candidate = proposal["reviewable_candidate"]
                readback = client.tool(
                    "artifact_readback_get",
                    {"artifact_id": candidate["artifact_sha256"], "candidate_id": candidate["candidate_id"]},
                )
                require(
                    readback.get("hard_gate_passed") is True
                    and readback.get("validator_status") == "passed",
                    f"{TASK_ID} strict GLB readback failed: {slug}",
                )
                evidence_prepare = evidence_request(slug, proposal, readback, baseline_sha)
                evidence = client.tool(
                    "production_weapon_form_art_composite_evidence_prepare",
                    evidence_prepare,
                )
                form_art = read_cas_json(
                    args.data_root, evidence["proposal_form_art_evidence_receipt_object_sha256"]
                )
                cross_view = read_cas_json(args.data_root, evidence["cross_view_evidence_bundle_sha256"])
                trials.append(
                    {
                        "variant_id": slug,
                        "registered_profile_id": profile_id,
                        "proposal": proposal,
                        "artifact_readback": {
                            "canonical_sha256": readback.get("canonical_sha256"),
                            "hard_gate_passed": readback.get("hard_gate_passed"),
                            "validator_status": readback.get("validator_status"),
                        },
                        "evidence": evidence,
                        "cross_view": {
                            "canonical_sha256": cross_view.get("canonical_sha256"),
                            "hard_gate_passed": cross_view.get("hard_gate_passed"),
                            "non_regressing": cross_view.get("non_regressing"),
                            "strict_improvement": cross_view.get("strict_improvement"),
                            "baseline_score": cross_view.get("baseline_score"),
                            "proposal_score": cross_view.get("proposal_score"),
                            "promotion": cross_view.get("promotion"),
                        },
                        "target_trigger_void": target_trigger_row(form_art),
                        "proposal_form_art_ready": form_art.get("proposal_form_art_evidence_ready"),
                    }
                )
                restart_inputs.append(
                    (slug, proposal_get_request(slug), evidence_get_request(evidence_prepare))
                )
        finally:
            close_client(runtime, ready_path, ready, client)

        runtime, ready_path, ready, client = open_client(
            args.mcp, args.runtime, args.data_root, Path(temporary) / "restart", args.timeout
        )
        try:
            for trial, (slug, proposal_get, evidence_get) in zip(trials, restart_inputs):
                proposal_restart = client.tool(
                    "production_weapon_form_art_composite_proposal_get", proposal_get
                )
                evidence_restart = client.tool(
                    "production_weapon_form_art_composite_evidence_get", evidence_get
                )
                require(
                    proposal_restart.get("record_canonical_sha256")
                    == trial["proposal"].get("record_canonical_sha256")
                    and proposal_restart.get("receipt_object_sha256")
                    == trial["proposal"].get("receipt_object_sha256"),
                    f"{TASK_ID} proposal restart differs: {slug}",
                )
                require(
                    evidence_restart.get("record_canonical_sha256")
                    == trial["evidence"].get("record_canonical_sha256")
                    and evidence_restart.get("receipt_object_sha256")
                    == trial["evidence"].get("receipt_object_sha256")
                    and evidence_restart.get("cross_view_evidence_bundle_sha256")
                    == trial["evidence"].get("cross_view_evidence_bundle_sha256")
                    and evidence_restart.get(
                        "proposal_form_art_evidence_receipt_object_sha256"
                    )
                    == trial["evidence"].get(
                        "proposal_form_art_evidence_receipt_object_sha256"
                    ),
                    f"{TASK_ID} evidence restart differs: {slug}",
                )
                trial["restart_readback"] = {
                    "proposal_record_canonical_sha256": proposal_restart[
                        "record_canonical_sha256"
                    ],
                    "evidence_record_canonical_sha256": evidence_restart[
                        "record_canonical_sha256"
                    ],
                    "exact_hashes_equal": True,
                }
        finally:
            close_client(runtime, ready_path, ready, client)

    eligible = [
        trial
        for trial in trials
        if trial["cross_view"].get("non_regressing") is True
        and trial["target_trigger_void"].get("sealed") is False
    ]
    selected = max(
        eligible,
        key=lambda trial: (
            trial["cross_view"].get("proposal_score", 0),
            trial["target_trigger_void"].get("iou_milli", 0),
        ),
        default=None,
    )
    if args.trial_family == "retraction":
        require(
            not eligible,
            "04BE-I unexpectedly produced an eligible trial; explicit selection is required",
        )
    for trial in trials:
        retained = selected is not None and trial is selected
        trial["decision"] = {
            "status": "SELECTED_FOR_HUMAN_REVIEW" if retained else "REJECTED_RETAIN_PARENT",
            "reasons": [
                "SIX_VIEW_REGRESSION"
                if trial["cross_view"].get("non_regressing") is not True
                else "SIX_VIEW_NON_REGRESSION_PASS",
                f"{TARGET_STRUCTURE_ID.upper().replace('.', '_').replace('-', '_')}_STILL_SEALED"
                if trial["target_trigger_void"].get("sealed") is True
                else f"{TARGET_STRUCTURE_ID.upper().replace('.', '_').replace('-', '_')}_OPEN_RESPONSE",
            ],
        }

    if selected is None:
        selected_candidate_id = CURRENT_CANDIDATE_ID
        selected_candidate_state = CURRENT_CANDIDATE_STATE
        selected_geometry_program = CURRENT_PROGRAM
        selection_status = (
            "RETAINED_PARENT_ALL_RECEIVER_UPPER_TRIALS_REJECTED"
            if args.trial_family in {"receiver-upper-retraction", "receiver-upper-target-notch", "receiver-upper-u-topology", "receiver-upper-camera-target-u-topology"}
            else "RETAINED_PARENT_ALL_STEP_1_TRIALS_REJECTED"
        )
        next_atomic_action = (
            "RUN_TARGET_REGION_OCCLUSION_ATTRIBUTION_AFTER_AUTHORIZED_RECEIVER_UPPER_TRIALS"
            if args.trial_family == "receiver-upper-retraction"
            else "REDESIGN_RECEIVER_UPPER_TARGET_NOTCH_OR_RECALIBRATE_TARGET_STRUCTURE"
            if args.trial_family == "receiver-upper-target-notch"
            else "RUN_EXACT_TARGET_OCCLUSION_ATTRIBUTION_FOR_TYPED_U_TOPOLOGY"
            if args.trial_family == "receiver-upper-u-topology"
            else "RUN_EXACT_TARGET_OCCLUSION_ATTRIBUTION_FOR_CAMERA_TARGET_U_TOPOLOGY"
            if args.trial_family == "receiver-upper-camera-target-u-topology"
            else "EXPAND_OR_REDESIGN_SIDE_PANEL_A_APERTURE_MUTATION_FAMILY_BEFORE_RECEIVER_UPPER"
        )
    else:
        selected_candidate = selected["proposal"]["reviewable_candidate"]
        selected_lineage = selected["proposal"]["original_current_final_lineage"]
        selected_candidate_id = selected_candidate["candidate_id"]
        selected_candidate_state = selected_candidate["candidate_state_sha256"]
        selected_geometry_program = selected_lineage["composed_geometry_program_sha256"]
        selection_status = (
            "RECEIVER_UPPER_CANDIDATE_SELECTED_FOR_HUMAN_REVIEW"
            if args.trial_family in {"receiver-upper-retraction", "receiver-upper-target-notch", "receiver-upper-u-topology", "receiver-upper-camera-target-u-topology"}
            else "TRUE_APERTURE_CANDIDATE_SELECTED_FOR_HUMAN_REVIEW"
        )
        next_atomic_action = "HUMAN_REVIEW_SELECTED_APERTURE_CANDIDATE"

    receipt = {
        "schema_version": "ForgeCADProductionWeaponFormArtApertureTrialsRealD1Gate@1",
        "task_id": TASK_ID,
        "recorded_on": "2026-08-29" if args.trial_family in {"receiver-upper-target-notch", "receiver-upper-u-topology", "receiver-upper-camera-target-u-topology"} else "2026-08-28",
        "status": (
            "PASS_TRUE_APERTURE_CANDIDATE_SELECTED_FOR_HUMAN_REVIEW"
            if selected is not None
            else {
                "retraction": "PASS_FOUR_REGISTERED_SIDE_PANEL_A_RETRACTION_TRIALS_REJECTED_PARENT_RETAINED",
                "true-aperture": "PASS_FOUR_REGISTERED_SIDE_PANEL_A_TRUE_APERTURE_TRIALS_REJECTED_PARENT_RETAINED",
                "camera-mapped-aperture": "PASS_FOUR_REGISTERED_SIDE_PANEL_A_CAMERA_MAPPED_APERTURE_TRIALS_REJECTED_PARENT_RETAINED",
                "receiver-upper-retraction": "PASS_FOUR_USER_AUTHORIZED_RECEIVER_UPPER_TRIALS_REJECTED_PARENT_RETAINED",
                "receiver-upper-target-notch": "PASS_FOUR_TARGET_MAPPED_RECEIVER_UPPER_NOTCH_TRIALS_REJECTED_PARENT_RETAINED",
                "receiver-upper-u-topology": "PASS_FOUR_TYPED_RECEIVER_UPPER_U_TOPOLOGY_TRIALS_REJECTED_PARENT_RETAINED",
                "receiver-upper-camera-target-u-topology": "PASS_FOUR_CAMERA_TARGET_MAPPED_RECEIVER_UPPER_U_TOPOLOGY_TRIALS_REJECTED_PARENT_RETAINED",
            }[args.trial_family]
        ),
        "build": {
            "build_cohort_sha256": args.expected_build_cohort,
            "mcp_identity": identities["mcp"],
            "runtime_identity": identities["runtime"],
        },
        "mandatory_ponytail_preflight": "PASS",
        "fresh_original_baseline": baseline,
        "parent": {
            "candidate_id": CURRENT_CANDIDATE_ID,
            "candidate_state_sha256": CURRENT_CANDIDATE_STATE,
            "artifact_sha256": CURRENT_ARTIFACT,
            "geometry_program_sha256": CURRENT_PROGRAM,
            "form_art_evidence_receipt_object_sha256": CURRENT_PROPOSAL_EVIDENCE,
        },
        "trials": trials,
        "selection": {
            "policy": "target-aperture-response-and-six-view-non-regression@1",
            "eligible_trial_count": len(eligible),
            "selected_candidate_id": selected_candidate_id,
            "selected_candidate_state_sha256": selected_candidate_state,
            "selected_geometry_program_sha256": selected_geometry_program,
            "status": selection_status,
            "step_2_receiver_upper_authorized": args.trial_family
            in {"receiver-upper-retraction", "receiver-upper-target-notch", "receiver-upper-u-topology", "receiver-upper-camera-target-u-topology"},
            "receiver_upper_authorization_source": (
                "EXPLICIT_USER_AUTHORIZATION_2026-08-28"
                if args.trial_family in {"receiver-upper-retraction", "receiver-upper-target-notch", "receiver-upper-u-topology", "receiver-upper-camera-target-u-topology"}
                else None
            ),
            "next_atomic_action": next_atomic_action,
        },
        "non_promotion_boundary": {
            "candidate_confirmed": False,
            "version_created": False,
            "export_performed": False,
            "production_stage_advanced": False,
            "form_quality_v2_status": "NOT_CREATED",
            "quality_status": "QUALITY_TARGET_NOT_MET",
        },
    }
    evidence_path.parent.mkdir(parents=True, exist_ok=True)
    evidence_path.write_text(json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    print(
        json.dumps(
            {
                "task_id": receipt["task_id"],
                "status": receipt["status"],
                "trial_decisions": [
                    {
                        "variant_id": trial["variant_id"],
                        "candidate_id": trial["proposal"]["reviewable_candidate"]["candidate_id"],
                        "cross_view_non_regressing": trial["cross_view"]["non_regressing"],
                        "target_structure_id": TARGET_STRUCTURE_ID,
                        "target_trigger_void_sealed": trial["target_trigger_void"]["sealed"],
                        "decision": trial["decision"]["status"],
                    }
                    for trial in trials
                ],
                "selection": receipt["selection"],
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (GateFailure, KeyError, OSError, ValueError) as error:
        print(f"aperture trial probe failed: {error}")
        raise SystemExit(1)
