#!/usr/bin/env python3
"""Execute the closed 04BE-E rear-stock repair and bind exact six-view evidence.

This is a focused production evidence probe, not a unit-test generator. It
opens the existing D1 Runtime, performs the mandatory Ponytail preflight,
re-reads the 04BE-D evidence-ranked plan, prepares one cumulative candidate
through the registered half-Y/flat-Z profile, renders the exact original
six-camera 6x9 AOV set, persists CrossView/FormArt evidence, restarts Runtime,
and proves exact durable readback. It never confirms, versions, exports or
advances ProductionStage.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_ROOT))

from probe_mcp010b_raw_stdio import (  # noqa: E402
    GateFailure,
    MCP_PROTOCOL_VERSION,
    McpClient,
    build_identity,
    shutdown_runtime,
    wait_for_ready,
)

PROJECT_ID = "project-0d236b8acdde4f1187b3a46a7d5e4f0f"
SESSION_ID = "fps-form-04a-session"
ORIGINAL_CANDIDATE_ID = "candidate-86f6ed6ac95c413d9280c1061b33ee72"
ORIGINAL_CANDIDATE_STATE = "dd1fdcf09aa62c6a0f5e84424ebbea2314f386d12785d1b758f37a5bef97a160"
ORIGINAL_ARTIFACT = "87ca869f14f2ff5cd301886181974512b11916339587a34fd780b1bb9ee29c55"
PRIOR_BASELINE_ID = "fps-form-04be-c-final-baseline-r7"
PRIOR_BASELINE_CANONICAL = "ba5e08dc68efb784f25f552aecfdf2c82575b591e5173bbc54c98bead845b32a"
BASELINE_ID = "fps-form-04be-e-final-baseline-r9"
REGISTRATION_LINEAGE_ID = "fps-form-04az-camera-lineage-v2"
REGISTRATION_LINEAGE_CANONICAL = "1b4358c038af9c4cb924d14d7f5e6b38d19b962337a4f1a86a7ee2505bd47a29"
ORIGINAL_ARTIFACT_ID = "geometry-object-87ca869f14f2ff5cd301886181974512"
CURRENT_CANDIDATE_ID = "candidate-f4a7d01ea8174dd4821c21fcd8ff06fa"
CURRENT_CANDIDATE_STATE = "da53deaadbbc74e1c859f9c0d4b1c85427a8b2dcfa9f9bb6748c07bd6da354aa"
CURRENT_ARTIFACT = "e301dd8c1881ff1334acde4cd61bb0ffdb3eb7017c8cd7784fe58b1aac7ee007"
CURRENT_PROGRAM = "ca7db8dae76a8ccb885756a871def18dbe795057a8775f5b53ae1cf18b852386"
CURRENT_PROPOSAL_EVIDENCE = "e1240c5f175569b029341638218788197d0bff5dc7e88f85f586912dae54247e"
SOURCE_FORM_ART_ID = "fps-form-04a-form-art"
SOURCE_FORM_ART_OBJECT = "fb0092cfd31f75dec23480af69e4e3055f756d47085f7d17409dc69674423114"
SOURCE_FORM_ART_CANONICAL = "ed829254f6e543693690f0cbca99dea8b3af52caf7d0e223ebc1d8754caccec4"
PROFILE_ID = "registered-boundary-bridge-half-y-flat-z-owner-void@1"
PROPOSAL_ID = "fps-form-04be-e-owner-void-half-y-v1"
EVIDENCE_ID = "fps-form-04be-e-owner-void-half-y-evidence-v1"
MAX_RESPONSE_BYTES = 1_048_576
WRITER_POLICY = "forgecad-runtime-only-state-writer@1"
CANONICALIZATION_POLICY = "canonical-json-sha256-excluding-input-sha256@1"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateFailure(message)


def canonical_hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    ).hexdigest()


def with_input_hash(value: dict[str, Any]) -> dict[str, Any]:
    result = dict(value)
    result["input_sha256"] = canonical_hash(value)
    return result


def read_cas_json(data_root: Path, object_sha256: str) -> dict[str, Any]:
    require(
        len(object_sha256) == 64
        and all(character in "0123456789abcdef" for character in object_sha256),
        "Runtime returned an invalid CAS object hash",
    )
    object_path = data_root / "runtime.cas" / "objects" / object_sha256[:2] / object_sha256
    require(object_path.is_file(), f"CAS object is unavailable: {object_sha256}")
    payload = object_path.read_bytes()
    require(
        hashlib.sha256(payload).hexdigest() == object_sha256,
        f"CAS object bytes differ from the bound hash: {object_sha256}",
    )
    value = json.loads(payload)
    require(isinstance(value, dict), "CAS evidence object is not a JSON object")
    return value


def evidence_view(value: dict[str, Any], view_kind: str) -> dict[str, Any]:
    for row in value.get("views", []):
        if isinstance(row, dict) and row.get("view_kind") == view_kind:
            return row
    raise GateFailure(f"FormArt evidence view is unavailable: {view_kind}")


def owner_metric_row(value: dict[str, Any], view_kind: str) -> dict[str, Any]:
    row = evidence_view(value, view_kind)
    owner = row.get("owner_evidence")
    require(isinstance(owner, dict), f"owner evidence is unavailable: {view_kind}")
    return {
        "owner_expected_void_overlap_milli": owner.get(
            "owner_expected_void_overlap_milli"
        ),
        "owner_region_pixel_count": owner.get("owner_region_pixel_count"),
        "owner_boundary_adjacency_milli": owner.get(
            "owner_boundary_adjacency_milli"
        ),
        "owner_bbox_px": owner.get("owner_bbox_px"),
        "expected_void_bbox_px": owner.get("expected_void_bbox_px"),
        "ranked_transform": owner.get("ranked_transform"),
        "strict_owner_void_passed": owner.get("strict_owner_void_passed"),
    }


def operation() -> dict[str, Any]:
    value = {
        "schema_version": "ProductionWeaponFormArtCompositeProposalOperation@1",
        "sequence_index": 0,
        "operation_id": "operation-rear-stock-owner-void-half-y-04be-e",
        "operation_kind": "registered_profile_replace",
        "source_node_id": "rear-stock",
        "part_id": "rear-stock",
        "registered_profile_id": PROFILE_ID,
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def baseline_get_request() -> dict[str, Any]:
    return with_input_hash(
        {
            "schema_version": "ProductionWeaponFormArtBaselineGetRequest@1",
            "operation": "forgecad.production.weapon.form-art-baseline-get@1",
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
            "idempotency_key": "fps-form-04be-e-final-baseline-r9-prepare",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256@1",
            "runtime_write_performed": False,
            "persistent_user_data_touched": False,
        }
    )


def plan(baseline_canonical: str) -> dict[str, Any]:
    value = {
        "schema_version": "ProductionWeaponFormArtCompositeProposalPlan@1",
        "project_id": PROJECT_ID,
        "original_source_candidate_id": ORIGINAL_CANDIDATE_ID,
        "original_source_candidate_state_sha256": ORIGINAL_CANDIDATE_STATE,
        "original_source_artifact_sha256": ORIGINAL_ARTIFACT,
        "original_fresh_baseline_canonical_sha256": baseline_canonical,
        "current_base_candidate_id": CURRENT_CANDIDATE_ID,
        "current_base_candidate_state_sha256": CURRENT_CANDIDATE_STATE,
        "current_base_artifact_sha256": CURRENT_ARTIFACT,
        "current_base_geometry_program_sha256": CURRENT_PROGRAM,
        "current_base_proposal_evidence_sha256": CURRENT_PROPOSAL_EVIDENCE,
        "operations": [operation()],
        "composition_policy": "runtime-owned-original-baseline-current-base-registered-disjoint-replacements@1",
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def repair_plan_request() -> dict[str, Any]:
    return with_input_hash(
        {
            "schema_version": "ProductionWeaponFormArtRepairPlanGetRequest@1",
            "operation": "forgecad.production.weapon.form-art-repair-plan-get@1",
            "repair_plan_id": "fps-form-04be-d-repair-plan-v1",
            "composite_evidence_id": "fps-form-04be-c-evidence-v1",
            "proposal_id": "fps-form-04be-b-composite-v1",
            "session_id": SESSION_ID,
            "project_id": PROJECT_ID,
            "composite_evidence_record_canonical_sha256": "86043db0e9e8dd5adb8a8d31a677c048ea7eba5e2458f6c17dec78b433120496",
            "composite_evidence_receipt_object_sha256": "7fb0a11d205f33e1469c88e4780ea8e062759475300e6884e11c9b2593a75106",
            "cross_view_evidence_bundle_sha256": "c93ccb2c4e7ce3cb8d7958a12ab2e31784f3e014b354c351cb0a2de4dade02f4",
            "proposal_form_art_evidence_receipt_object_sha256": CURRENT_PROPOSAL_EVIDENCE,
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": False,
            "derivation_policy": "durable-cross-view-form-art-owner-void-repair-plan@1",
            "canonicalization_policy": CANONICALIZATION_POLICY,
        }
    )


def proposal_prepare_request(baseline_canonical: str) -> dict[str, Any]:
    return with_input_hash(
        {
            "schema_version": "ProductionWeaponFormArtCompositeProposalPrepareRequest@1",
            "proposal_id": PROPOSAL_ID,
            "session_id": SESSION_ID,
            "project_id": PROJECT_ID,
            "original_fresh_baseline_id": BASELINE_ID,
            "plan": plan(baseline_canonical),
            "idempotency_key": "fps-form-04be-e-owner-void-half-y-prepare",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": False,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": CANONICALIZATION_POLICY,
        }
    )


def proposal_get_request() -> dict[str, Any]:
    return with_input_hash(
        {
            "schema_version": "ProductionWeaponFormArtCompositeProposalGetRequest@1",
            "project_id": PROJECT_ID,
            "proposal_id": PROPOSAL_ID,
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": False,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": CANONICALIZATION_POLICY,
        }
    )


def evidence_prepare_request(
    proposal: dict[str, Any], readback: dict[str, Any], baseline_canonical: str
) -> dict[str, Any]:
    lineage = proposal["original_current_final_lineage"]
    candidate = proposal["reviewable_candidate"]
    artifact_sha = candidate["artifact_sha256"]
    return with_input_hash(
        {
            "schema_version": "ProductionWeaponFormArtCompositeEvidencePrepareRequest@1",
            "operation": "forgecad.production.weapon.form-art-composite-evidence-prepare@1",
            "composite_evidence_id": EVIDENCE_ID,
            "proposal_id": PROPOSAL_ID,
            "session_id": SESSION_ID,
            "project_id": PROJECT_ID,
            "composite_proposal_record_canonical_sha256": proposal["record_canonical_sha256"],
            "composite_proposal_receipt_object_sha256": proposal["receipt_object_sha256"],
            "original_fresh_baseline_id": BASELINE_ID,
            "original_fresh_baseline_canonical_sha256": baseline_canonical,
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
            "idempotency_key": "fps-form-04be-e-owner-void-half-y-evidence-prepare",
            "max_response_bytes": MAX_RESPONSE_BYTES,
            "runtime_write_performed": False,
            "writer_policy": WRITER_POLICY,
            "canonicalization_policy": CANONICALIZATION_POLICY,
        }
    )


def evidence_get_request(evidence_request: dict[str, Any]) -> dict[str, Any]:
    value = dict(evidence_request)
    value["schema_version"] = "ProductionWeaponFormArtCompositeEvidenceGetRequest@1"
    value["operation"] = "forgecad.production.weapon.form-art-composite-evidence-get@1"
    value.pop("idempotency_key")
    value.pop("input_sha256")
    return with_input_hash(value)


def start_runtime(
    runtime_binary: Path,
    data_root: Path,
    endpoint_root: Path,
    environment: dict[str, str],
    timeout: float,
) -> tuple[subprocess.Popen[str], Path, dict[str, Any]]:
    endpoint_root.mkdir(mode=0o700, parents=True, exist_ok=False)
    ready_path = endpoint_root / "ready.json"
    process = subprocess.Popen(
        [
            str(runtime_binary),
            "serve",
            "--database",
            str(data_root / "runtime.sqlite"),
            "--cas-root",
            str(data_root / "runtime.cas"),
            "--endpoint-dir",
            str(endpoint_root / "ipc"),
            "--ready-file",
            str(ready_path),
        ],
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    return process, ready_path, wait_for_ready(ready_path, process, timeout)


def open_client(
    mcp_binary: Path,
    runtime_binary: Path,
    data_root: Path,
    endpoint_root: Path,
    timeout: float,
) -> tuple[subprocess.Popen[str], Path, dict[str, Any], McpClient]:
    environment = os.environ.copy()
    for key in (
        "FORGECAD_RUNTIME_SOCKET",
        "FORGECAD_RUNTIME_TOKEN",
        "FORGECAD_RUNTIME_DATA_DIR",
        "FORGECAD_RUNTIME_COMMAND",
    ):
        environment.pop(key, None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    runtime, ready_path, ready = start_runtime(
        runtime_binary, data_root, endpoint_root, environment, timeout
    )
    environment["FORGECAD_RUNTIME_SOCKET"] = str(ready["socket_path"])
    environment["FORGECAD_RUNTIME_TOKEN"] = str(ready["token"])
    client = McpClient(mcp_binary, environment, timeout)
    initialized = client.request(
        "initialize",
        {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "forgecad-04be-e-d1-probe", "version": "1"},
        },
    )
    require(
        initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION,
        "MCP initialize failed",
    )
    client.notify("notifications/initialized")
    preflight = client.tool(
        "skill_get", {"skill_id": "ponytail-preflight", "version": "0.1.0"}
    )
    require(
        isinstance(preflight, dict)
        and preflight.get("skill", {}).get("skill_id") == "ponytail-preflight",
        "mandatory Ponytail preflight failed",
    )
    return runtime, ready_path, ready, client


def close_client(
    runtime: subprocess.Popen[str],
    ready_path: Path,
    ready: dict[str, Any],
    client: McpClient,
) -> None:
    client.close()
    shutdown_runtime(ready, ready_path, runtime)
    if runtime.stderr is not None:
        diagnostic = runtime.stderr.read().strip()
        if diagnostic:
            print(diagnostic, file=sys.stderr)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--expected-build-cohort", required=True)
    parser.add_argument("--timeout", type=float, default=180.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    require(args.mcp.is_file() and args.runtime.is_file(), "build binaries are unavailable")
    identities = {
        "mcp": build_identity(args.mcp),
        "runtime": build_identity(args.runtime),
    }
    require(
        all(
            identity.get("build_cohort_sha256") == args.expected_build_cohort
            for identity in identities.values()
        ),
        "Runtime/MCP build cohort differs",
    )
    require((args.data_root / "runtime.sqlite").is_file(), "D1 database is unavailable")
    require((args.data_root / "runtime.cas").is_dir(), "D1 CAS is unavailable")
    root = SCRIPT_ROOT.parent
    evidence_path = args.evidence if args.evidence.is_absolute() else root / args.evidence
    evidence_path.resolve().relative_to((root / "docs" / "evidence").resolve())

    with tempfile.TemporaryDirectory(prefix="forgecad-04be-e-", dir="/tmp") as temporary:
        temporary_root = Path(temporary)
        runtime, ready_path, ready, client = open_client(
            args.mcp,
            args.runtime,
            args.data_root,
            temporary_root / "prepare",
            args.timeout,
        )
        try:
            names = {
                item.get("name")
                for item in client.request("tools/list").get("result", {}).get("tools", [])
                if isinstance(item, dict)
            }
            required_tools = {
                "production_weapon_form_art_repair_plan_get",
                "production_weapon_form_art_baseline_get",
                "production_weapon_form_art_composite_proposal_prepare",
                "production_weapon_form_art_composite_proposal_get",
                "production_weapon_form_art_composite_evidence_prepare",
                "production_weapon_form_art_composite_evidence_get",
                "artifact_readback_get",
            }
            require(required_tools.issubset(names), "04BE-E tools are not exposed")
            repair_plan = client.tool(
                "production_weapon_form_art_repair_plan_get", repair_plan_request()
            )
            require(
                repair_plan.get("target_profile", {}).get("profile_id") == PROFILE_ID,
                "04BE-D target profile differs",
            )
            baseline_readback = client.tool(
                "production_weapon_form_art_baseline_get", baseline_get_request()
            )
            baseline = baseline_readback.get("baseline")
            require(
                isinstance(baseline, dict)
                and baseline.get("baseline_id") == BASELINE_ID
                and baseline.get("runtime_build_cohort_sha256")
                == args.expected_build_cohort
                and len(baseline.get("views", [])) == 6,
                "04BE-E same-camera same-source baseline materialization failed",
            )
            baseline_canonical = baseline["canonical_sha256"]
            proposal = client.tool(
                "production_weapon_form_art_composite_proposal_prepare",
                proposal_prepare_request(baseline_canonical),
            )
            require(
                proposal.get("runtime_write_performed") is True
                or proposal.get("replayed") is True,
                "04BE-E candidate was neither prepared nor replayed",
            )
            candidate = proposal["reviewable_candidate"]
            readback = client.tool(
                "artifact_readback_get",
                {
                    "artifact_id": candidate["artifact_sha256"],
                    "candidate_id": candidate["candidate_id"],
                },
            )
            require(
                readback.get("hard_gate_passed") is True
                and readback.get("validator_status") == "passed",
                "04BE-E strict GLB readback failed",
            )
            evidence_request = evidence_prepare_request(
                proposal, readback, baseline_canonical
            )
            evidence = client.tool(
                "production_weapon_form_art_composite_evidence_prepare",
                evidence_request,
            )
            proposal_form_art = read_cas_json(
                args.data_root,
                evidence["proposal_form_art_evidence_receipt_object_sha256"],
            )
            source_proposal_form_art = read_cas_json(
                args.data_root, CURRENT_PROPOSAL_EVIDENCE
            )
            cross_view = read_cas_json(
                args.data_root, evidence["cross_view_evidence_bundle_sha256"]
            )
            require(
                evidence.get("aov_count") == 54
                and proposal_form_art.get("part_id_all_views_observed") is True,
                "04BE-E did not produce exact six-view 54-AOV evidence",
            )
        finally:
            close_client(runtime, ready_path, ready, client)

        runtime, ready_path, ready, client = open_client(
            args.mcp,
            args.runtime,
            args.data_root,
            temporary_root / "restart",
            args.timeout,
        )
        try:
            proposal_restart = client.tool(
                "production_weapon_form_art_composite_proposal_get", proposal_get_request()
            )
            evidence_restart = client.tool(
                "production_weapon_form_art_composite_evidence_get",
                evidence_get_request(evidence_request),
            )
        finally:
            close_client(runtime, ready_path, ready, client)

    require(
        proposal_restart["record_canonical_sha256"] == proposal["record_canonical_sha256"]
        and proposal_restart["receipt_object_sha256"] == proposal["receipt_object_sha256"],
        "04BE-E proposal restart hash differs",
    )
    require(
        evidence_restart["record_canonical_sha256"] == evidence["record_canonical_sha256"]
        and evidence_restart["receipt_object_sha256"] == evidence["receipt_object_sha256"]
        and evidence_restart["cross_view_evidence_bundle_sha256"]
        == evidence["cross_view_evidence_bundle_sha256"]
        and evidence_restart["proposal_form_art_evidence_receipt_object_sha256"]
        == evidence["proposal_form_art_evidence_receipt_object_sha256"],
        "04BE-E evidence restart hash differs",
    )

    form_art_ready = proposal_form_art.get("proposal_form_art_evidence_ready") is True
    quality_status = "READY_FOR_FRESH_FORM_QUALITY_V2" if form_art_ready else "QUALITY_TARGET_NOT_MET"
    receipt = {
        "schema_version": "ForgeCADProductionWeaponFormArtRepairExecutionRealD1Gate@1",
        "task_id": "FPS-FORM-04BE-E",
        "recorded_on": "2026-08-28",
        "status": (
            "PASS_REPAIR_AND_EVIDENCE_READY_FOR_FORM_QUALITY_V2"
            if form_art_ready
            else "PASS_REPAIR_EXECUTION_WITH_QUALITY_TARGET_NOT_MET"
        ),
        "build": {
            "build_cohort_sha256": args.expected_build_cohort,
            "mcp_identity": identities["mcp"],
            "runtime_identity": identities["runtime"],
        },
        "repair_plan": {
            "repair_plan_id": repair_plan["repair_plan_id"],
            "strategy_id": repair_plan["strategy_id"],
            "target_profile_id": repair_plan["target_profile"]["profile_id"],
            "canonical_sha256": repair_plan["canonical_sha256"],
        },
        "original_fresh_baseline": {
            "prior_baseline_id": PRIOR_BASELINE_ID,
            "prior_baseline_canonical_sha256": PRIOR_BASELINE_CANONICAL,
            "baseline": baseline,
            "same_original_candidate": True,
            "same_registration_lineage": True,
            "same_registered_six_camera_rig": True,
            "rerender_reason": "CURRENT_BUILD_COHORT_REQUIRES_SAME_COHORT_BASELINE",
        },
        "candidate": proposal,
        "artifact_readback": {
            "artifact_id": readback.get("artifact_id"),
            "candidate_id": readback.get("candidate_id"),
            "canonical_sha256": readback.get("canonical_sha256"),
            "hard_gate_passed": readback.get("hard_gate_passed"),
            "validator_status": readback.get("validator_status"),
        },
        "six_view_evidence": evidence,
        "proposal_form_art_evidence": proposal_form_art,
        "failure_delta_analysis": {
            "cross_view": {
                "hard_gate_passed": cross_view.get("hard_gate_passed"),
                "non_regressing": cross_view.get("non_regressing"),
                "strict_improvement": cross_view.get("strict_improvement"),
                "baseline_score": cross_view.get("baseline_score"),
                "proposal_score": cross_view.get("proposal_score"),
                "promotion": cross_view.get("promotion"),
            },
            "owner_metrics_before": {
                kind: owner_metric_row(source_proposal_form_art, kind)
                for kind in ("left", "right", "rear-three-quarter")
            },
            "owner_metrics_after": {
                kind: owner_metric_row(proposal_form_art, kind)
                for kind in ("left", "right", "rear-three-quarter")
            },
            "rear_three_quarter_negative_space": evidence_view(
                proposal_form_art, "rear-three-quarter"
            ).get("negative_space_rows"),
            "side_trigger_voids": {
                kind: [
                    row
                    for row in evidence_view(proposal_form_art, kind).get(
                        "negative_space_rows", []
                    )
                    if row.get("structure_id") == f"{kind}.trigger-void"
                ]
                for kind in ("left", "right")
            },
            "next_atomic_boundary": "DIAGNOSE_AXIS_CAMERA_ATTRIBUTION_AND_SIDE_TRIGGER_VISIBILITY_BEFORE_ANOTHER_REGISTERED_REPAIR",
        },
        "restart_readback": {
            "proposal": proposal_restart,
            "evidence": evidence_restart,
            "proposal_hashes_equal": True,
            "evidence_hashes_equal": True,
        },
        "form_quality_v2": {
            "status": "NOT_RUN_AWAITING_READY_PROPOSAL_FORM_ART"
            if not form_art_ready
            else "NEXT_ATOMIC_ACTION",
            "quality_status": quality_status,
        },
        "non_promotion_boundary": {
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
        print(f"04BE-E probe failed: {error}", file=sys.stderr)
        raise SystemExit(1)
