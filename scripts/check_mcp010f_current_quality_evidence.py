#!/usr/bin/env python3
"""Validate the additive current MCP010F quality-evidence ledger.

The Stage 0 truth file intentionally retains the older provisional observation.
This gate validates the newer CADFit receipts without overwriting that historical
observation, and refuses to merge isolated Boolean or ActionRun receipts into the
current source cohort when their receipts do not provide the required binding.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / "docs/evidence/mcp010f/current-quality-evidence-ledger.json"


def fail(message: str) -> None:
    raise SystemExit(f"MCP010F current quality evidence violation: {message}")


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def load_json(path: Path) -> dict[str, Any]:
    require(path.is_file(), f"missing evidence: {path.relative_to(ROOT)}")
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"evidence is not an object: {path.relative_to(ROOT)}")
    return value


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def evidence_path(row: dict[str, Any]) -> Path:
    value = row.get("evidence_path")
    require(isinstance(value, str) and value, "evidence_path must be a non-empty relative path")
    path = ROOT / value
    require(path.is_file() and not path.is_symlink(), f"evidence path is not a regular file: {value}")
    expected = row.get("evidence_sha256")
    require(isinstance(expected, str) and len(expected) == 64, f"evidence hash missing: {value}")
    require(sha256_file(path) == expected, f"evidence bytes drifted: {value}")
    return path


def require_same_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    require(set(value) == expected, f"{label} key set drifted")


def main() -> int:
    ledger = load_json(LEDGER)
    require(ledger.get("schema_version") == "ForgeCADMCP010FCurrentQualityEvidenceLedger@1", "schema version drifted")
    require(ledger.get("task_id") == "FGC-MCP010F", "task id drifted")
    require(ledger.get("status") == "PASS_EVIDENCE_LEDGER_WITH_QUALITY_TARGET_NOT_MET", "ledger status drifted")

    source_row = ledger["current_source_transport"]
    source_path = evidence_path(source_row)
    source = load_json(source_path)
    require(source.get("status") == source_row["transport_status"], "current transport status drifted")
    require(source.get("reference_sha256") == source_row["reference_sha256"], "current reference hash drifted")
    require(source.get("build_cohorts") == source_row["build_cohorts"], "current source cohorts drifted")
    cohorts = source["build_cohorts"]
    require(set(cohorts) == {"mcp", "runtime", "worker"}, "current cohort keys drifted")
    require(len(set(cohorts.values())) == 1, "current source cohort is not unified")
    require(source.get("camera_binding_status") == source_row["camera_binding_status"], "current camera binding drifted")
    require(source.get("quality_visual_status") == source_row["quality_visual_status"], "current visual status drifted")
    require(source.get("quality_hard_gate_passed") is source_row["quality_hard_gate_passed"], "current hard gate drifted")
    require(source.get("comparison_metrics") == source_row["metrics"], "current metrics drifted")
    downstream = source.get("downstream_status")
    require(isinstance(downstream, dict), "current downstream status is missing")
    require(downstream.get("candidate_confirm") == "NOT_RUN", "current candidate crossed confirm boundary")
    require(downstream.get("export") == "NOT_RUN", "current transport unexpectedly exported")
    require(downstream.get("restart_hash") == "NOT_RUN", "current transport unexpectedly claimed restart hash")
    require(source.get("candidate_confirmed", False) is False, "current candidate was unexpectedly confirmed")
    require(source.get("version_count", 0) == 0, "current transport unexpectedly created a version")
    require(source.get("persistent_user_data_touched") is False, "current transport touched persistent user data")

    surface_row = ledger["same_cohort_surface_signal_transport"]
    surface_path = evidence_path(surface_row)
    surface = load_json(surface_path)
    require(surface.get("status") == surface_row["status"], "same-cohort surface transport status drifted")
    require(surface.get("reference_sha256") == surface_row["reference_sha256"], "same-cohort surface reference drifted")
    require(surface.get("expected_build_cohort_sha256") == surface_row["expected_build_cohort_sha256"], "same-cohort expected cohort drifted")
    require(surface.get("build_cohorts") == surface_row["build_cohorts"], "same-cohort worker bindings drifted")
    surface_cohorts = surface["build_cohorts"]
    require(set(surface_cohorts) == {"mcp", "runtime", "geometry_worker", "render_worker"}, "same-cohort worker keys drifted")
    require(len(set(surface_cohorts.values())) == 1, "same-cohort surface binaries are not unified")
    require(surface_cohorts["mcp"] == surface_row["expected_build_cohort_sha256"], "same-cohort surface cohort does not match expected")
    require(surface.get("evaluations_count") == surface_row["evaluations_count"], "same-cohort evaluation count drifted")
    require(surface.get("fidelity_counts") == surface_row["fidelity_counts"], "same-cohort fidelity counts drifted")
    require(surface.get("baseline_loss") == surface_row["baseline_loss"], "same-cohort baseline loss drifted")
    require(surface.get("best_loss") == surface_row["best_loss"], "same-cohort best loss drifted")
    require(surface.get("strict_improvement") is True, "same-cohort strict improvement drifted")
    require(surface.get("proposal_status") == surface_row["proposal_status"], "same-cohort proposal boundary drifted")
    require(surface.get("candidate_confirmed") is surface_row["candidate_confirmed"], "same-cohort candidate crossed confirm boundary")
    require(surface.get("version_count") == surface_row["version_count"], "same-cohort version boundary drifted")
    require(surface.get("persistent_user_data_touched") is surface_row["persistent_user_data_touched"], "same-cohort transport touched persistent data")
    require(surface.get("camera_binding_status") == surface_row["optimization_camera_binding_status"], "same-cohort optimization camera binding drifted")
    require(surface.get("camera_hash") == surface_row["optimization_camera_hash"], "same-cohort optimization camera hash drifted")
    require(surface.get("comparison_status") == surface_row["comparison_status"], "same-cohort comparison status drifted")
    require(surface.get("comparison_camera_hash") == surface_row["comparison_camera_hash"], "same-cohort comparison camera hash drifted")
    require(surface.get("comparison_render_set_hash") == surface_row["comparison_render_set_hash"], "same-cohort RenderSet binding drifted")
    require(surface.get("comparison_report_sha256") == surface_row["comparison_report_sha256"], "same-cohort comparison report drifted")
    require(surface.get("comparison_metrics") == surface_row["comparison_metrics"], "same-cohort comparison metrics drifted")
    require(surface_row["optimization_camera_hash"] == surface_row["comparison_camera_hash"], "same-cohort camera binding is not unified")
    require(surface.get("surface_signal_status") == surface_row["surface_signal_status"], "surface signal status drifted")
    require(surface.get("surface_signal_canonical_sha256") == surface_row["surface_signal_canonical_sha256"], "surface signal hash drifted")
    residual = surface.get("residual") or {}
    require(residual.get("source_visual_surface_sha256") == surface_row["residual_source_visual_surface_sha256"], "residual surface signal binding drifted")
    require(surface_row["surface_signal_canonical_sha256"] == surface_row["residual_source_visual_surface_sha256"], "surface signal and residual hashes diverged")
    require(surface.get("boolean_backend") == surface_row["boolean_backend"], "same-cohort Boolean backend drifted")
    require(surface.get("boolean_lane_node_ids") == [surface_row["boolean_lane_node_id"]], "same-cohort Boolean node drifted")
    require(surface.get("boolean_lane_candidate_indices") == surface_row["boolean_candidate_indices"], "same-cohort Boolean candidate coverage drifted")
    require(surface.get("quality_claim") == surface_row["quality_claim"], "same-cohort quality claim boundary drifted")

    action_surface_row = ledger["same_cohort_action_run_transport"]
    action_surface_path = evidence_path(action_surface_row)
    action_surface = load_json(action_surface_path)
    require(action_surface.get("status") == action_surface_row["status"], "same-cohort ActionRun status drifted")
    require(action_surface.get("reference_sha256") == action_surface_row["reference_sha256"], "same-cohort ActionRun reference drifted")
    require(action_surface.get("expected_build_cohort_sha256") == action_surface_row["expected_build_cohort_sha256"], "same-cohort ActionRun expected cohort drifted")
    require(action_surface.get("build_cohorts") == action_surface_row["build_cohorts"], "same-cohort ActionRun worker bindings drifted")
    action_surface_cohorts = action_surface["build_cohorts"]
    require(set(action_surface_cohorts) == {"mcp", "runtime", "geometry_worker", "render_worker"}, "same-cohort ActionRun worker keys drifted")
    require(len(set(action_surface_cohorts.values())) == 1, "same-cohort ActionRun binaries are not unified")
    require(action_surface_cohorts["mcp"] == action_surface_row["expected_build_cohort_sha256"], "same-cohort ActionRun cohort does not match expected")
    for key in ("run_status", "completed_stage", "repair_probe_status", "repair_probe_gate", "source_visual_status", "camera_hash", "candidate_confirmed", "version_count", "persistent_user_data_touched", "quality_claim"):
        require(action_surface.get(key) == action_surface_row[key], f"same-cohort ActionRun field drifted: {key}")
    require(action_surface.get("run_sha256") == action_surface.get("replay_sha256"), "same-cohort ActionRun replay hash drifted")
    action_optimization = action_surface.get("optimization")
    require(isinstance(action_optimization, dict), "same-cohort ActionRun CADFit child is missing")
    for key in ("optimization_camera_hash", "camera_binding_status", "evaluations_count", "fidelity_counts", "baseline_loss", "best_loss", "proposal_status"):
        require(action_optimization.get(key) == action_surface_row[key], f"same-cohort ActionRun CADFit field drifted: {key}")
    require(action_optimization.get("strict_improvement") is action_surface_row["strict_improvement"], "same-cohort ActionRun strict improvement drifted")
    require(action_optimization.get("comparison_camera_hash") == action_surface_row["camera_hash"], "same-cohort ActionRun comparison camera drifted")
    require(action_optimization.get("optimization_camera_hash") == action_optimization.get("comparison_camera_hash"), "same-cohort ActionRun camera binding is not unified")
    require(action_optimization.get("source_candidate_unchanged") is action_surface_row["source_candidate_unchanged"], "same-cohort ActionRun source candidate changed")
    continuation = action_optimization.get("proposal_continuation")
    require(isinstance(continuation, dict), "same-cohort ActionRun proposal continuation is missing")
    for key in ("proposal_candidate_id", "proposal_candidate_state_sha256", "visual_status", "visual_gate_passed", "confirm_allowed", "repair_apply_status"):
        mapped = "proposal_visual_status" if key == "visual_status" else "proposal_visual_gate_passed" if key == "visual_gate_passed" else key
        require(continuation.get(key) == action_surface_row[mapped], f"same-cohort ActionRun proposal field drifted: {key}")
    require(action_surface_row["deterministic_replay"] is True, "same-cohort ActionRun replay boundary drifted")

    part_row = ledger["same_cohort_part_correction_transport"]
    part_path = evidence_path(part_row)
    part = load_json(part_path)
    require(part.get("status") == part_row["status"], "same-cohort Part correction status drifted")
    require(part.get("reference_sha256") == part_row["reference_sha256"], "same-cohort Part correction reference drifted")
    require(part.get("expected_build_cohort_sha256") == part_row["expected_build_cohort_sha256"], "same-cohort Part correction expected cohort drifted")
    require(part.get("build_cohorts") == part_row["build_cohorts"], "same-cohort Part correction worker bindings drifted")
    part_cohorts = part["build_cohorts"]
    require(set(part_cohorts) == {"mcp", "runtime", "geometry_worker", "render_worker"}, "same-cohort Part correction worker keys drifted")
    require(len(set(part_cohorts.values())) == 1, "same-cohort Part correction binaries are not unified")
    require(part_cohorts["mcp"] == part_row["expected_build_cohort_sha256"], "same-cohort Part correction cohort does not match expected")
    for key in ("part_id", "candidate_count", "automatic_target_sha256", "refined_part_target_sha256", "same_camera", "comparison_camera_hashes", "winner_candidate_id", "strict_improvement", "part_winner_candidate_id", "part_strict_improvement", "target_ranking_consistent", "persistent_user_data_touched", "quality_claim"):
        require(part.get(key) == part_row[key], f"same-cohort Part correction field drifted: {key}")
    for metric in ("silhouette_iou", "boundary_f1_4px", "critical_region_min_iou"):
        require((part.get("baseline_metrics") or {}).get(metric) == part_row["baseline_metrics"][metric], f"same-cohort Part correction baseline metric drifted: {metric}")
        winner_comparison_metrics = next(
            (
                row.get("metrics")
                for row in (part.get("candidate_comparisons") or [])
                if isinstance(row, dict) and row.get("candidate_id") == part_row["winner_candidate_id"]
            ),
            {},
        )
        require(winner_comparison_metrics.get(metric) == part_row["winner_metrics"][metric], f"same-cohort Part correction winner metric drifted: {metric}")
    part_winner_metrics = part.get("part_winner_metrics") or {}
    for metric in ("silhouette_iou", "boundary_f1_4px", "sdf_chamfer_px"):
        require(part_winner_metrics.get(metric) == part_row["part_winner_metrics"][metric], f"same-cohort Part correction Part-target metric drifted: {metric}")
    part_camera = part.get("camera")
    require(isinstance(part_camera, dict), "same-cohort Part correction camera evidence is missing")
    require(part_camera.get("camera_hash") == part_row["camera_hash"], "same-cohort Part correction camera hash drifted")
    require(part_camera.get("canonical_sha256") == part_row["camera_canonical_sha256"], "same-cohort Part correction camera canonical hash drifted")
    require(part_camera.get("binding_status") == part_row["camera_binding_status"], "same-cohort Part correction camera binding drifted")
    part_error_rows = (part.get("part_error") or {}).get("parts") if isinstance(part.get("part_error"), dict) else None
    require(isinstance(part_error_rows, list), "same-cohort Part correction PartError rows are missing")
    chest_error = next((row for row in part_error_rows if isinstance(row, dict) and row.get("part_id") == part_row["part_id"]), None)
    require(isinstance(chest_error, dict), "same-cohort Part correction chest PartError is missing")
    require(chest_error.get("boundary_error_px") == part_row["baseline_part_boundary_error_px"], "same-cohort Part correction boundary error drifted")
    require(part.get("same_camera") is True, "same-cohort Part correction camera was not fixed")
    require(part.get("comparison_camera_hashes") == [part_row["camera_hash"]], "same-cohort Part correction comparison camera drifted")
    comparisons = part.get("candidate_comparisons")
    require(isinstance(comparisons, list) and len(comparisons) == part_row["candidate_count"], "same-cohort Part correction candidate comparisons are incomplete")
    require(all(row.get("status") == part_row["comparison_status"] for row in comparisons if isinstance(row, dict)), "same-cohort Part correction visual status was promoted")
    require(part.get("strict_improvement") is True, "same-cohort Part correction strict improvement drifted")
    require(part.get("part_strict_improvement") is False, "same-cohort Part correction Part-target improvement was promoted")
    require(part.get("target_ranking_consistent") is False, "same-cohort Part/global target divergence was hidden")
    part_winner = part.get("part_winner")
    require(isinstance(part_winner, dict) and part_winner.get("winner_candidate_id") == part_row["part_winner_candidate_id"], "same-cohort Part correction Part-target winner drifted")
    require(part.get("candidate_confirmed", False) is False and part.get("version_count", 0) == 0, "same-cohort Part correction crossed approval boundary")
    require(part.get("persistent_user_data_touched") is False, "same-cohort Part correction touched persistent data")
    winner_proposal = part.get("winner", {}).get("candidates", []) if isinstance(part.get("winner"), dict) else []
    require(any(row.get("candidate_id") == part_row["winner_candidate_id"] for row in winner_proposal if isinstance(row, dict)), "same-cohort Part correction winner is missing")
    receipt_proposal = next((row.get("proposal") for row in part.get("candidate_comparisons", []) if isinstance(row, dict) and row.get("candidate_id") == part_row["winner_candidate_id"]), None)
    require(receipt_proposal == part_row["winner_proposal"], "same-cohort Part correction winner proposal drifted")
    require(part_path != source_path, "Part correction receipt was merged with current source transport")

    unified_row = ledger["same_cohort_unified_objective_transport"]
    unified_path = evidence_path(unified_row)
    unified = load_json(unified_path)
    require(unified.get("status") == unified_row["status"], "unified-objective Part correction status drifted")
    require(unified.get("reference_sha256") == unified_row["reference_sha256"], "unified-objective reference drifted")
    require(unified.get("expected_build_cohort_sha256") == unified_row["expected_build_cohort_sha256"], "unified-objective expected cohort drifted")
    require(unified.get("build_cohorts") == unified_row["build_cohorts"], "unified-objective worker bindings drifted")
    unified_cohorts = unified["build_cohorts"]
    require(set(unified_cohorts) == {"mcp", "runtime", "geometry_worker", "render_worker"}, "unified-objective worker keys drifted")
    require(len(set(unified_cohorts.values())) == 1, "unified-objective binaries are not unified")
    require(unified_cohorts["mcp"] == unified_row["expected_build_cohort_sha256"], "unified-objective cohort does not match expected")
    for key in (
        "part_id",
        "candidate_count",
        "automatic_target_sha256",
        "refined_part_target_sha256",
        "same_camera",
        "comparison_camera_hashes",
        "target_consistency_status",
        "target_ranking_consistent",
        "persistent_user_data_touched",
        "quality_claim",
    ):
        require(unified.get(key) == unified_row[key], f"unified-objective field drifted: {key}")
    require(unified.get("same_camera") is True, "unified-objective camera was not fixed")
    require(unified.get("comparison_camera_hashes") == [unified_row["camera_hash"]], "unified-objective comparison camera drifted")
    unified_camera = unified.get("camera")
    require(isinstance(unified_camera, dict), "unified-objective camera evidence is missing")
    require(unified_camera.get("camera_hash") == unified_row["camera_hash"], "unified-objective camera hash drifted")
    require(unified_camera.get("canonical_sha256") == unified_row["camera_canonical_sha256"], "unified-objective camera canonical hash drifted")
    require(unified_camera.get("binding_status") == unified_row["camera_binding_status"], "unified-objective camera binding drifted")
    unified_objective_result = unified.get("evaluation_objective")
    require(isinstance(unified_objective_result, dict), "unified-objective prepare result is missing")
    require(unified_objective_result.get("objective_sha256") == unified_row["evaluation_objective_sha256"], "unified-objective prepare hash drifted")
    require(unified_objective_result.get("canonical_sha256") == unified_row["evaluation_objective_canonical_sha256"], "unified-objective prepare canonical hash drifted")
    objective = unified_objective_result.get("objective")
    require(isinstance(objective, dict), "unified-objective payload is missing")
    require(objective.get("schema_version") == "SilhouetteEvaluationObjective@1", "unified-objective schema drifted")
    require(objective.get("canonical_sha256") == unified_row["evaluation_objective_payload_canonical_sha256"], "unified-objective payload canonical hash drifted")
    require(objective.get("global_target_sha256") == unified_row["automatic_target_sha256"], "unified-objective global target drifted")
    require(objective.get("part_target_sha256") == unified_row["refined_part_target_sha256"], "unified-objective Part target drifted")
    require(objective.get("camera_hash") == unified_row["camera_hash"], "unified-objective camera binding drifted")
    require(objective.get("camera_canonical_sha256") == unified_row["camera_canonical_sha256"], "unified-objective camera canonical binding drifted")
    require(objective.get("source_part_error_sha256") == unified.get("part_error", {}).get("canonical_sha256"), "unified-objective PartError binding drifted")
    objective_compare = unified.get("objective_compare")
    require(isinstance(objective_compare, dict), "unified-objective compare result is missing")
    require(objective_compare.get("schema_version") == "SilhouetteEvaluationCompareResult@1", "unified-objective compare schema drifted")
    require(objective_compare.get("objective_sha256") == unified_row["evaluation_objective_sha256"], "unified-objective compare objective drifted")
    require(objective_compare.get("global_target_sha256") == unified_row["automatic_target_sha256"], "unified-objective compare global target drifted")
    require(objective_compare.get("part_target_sha256") == unified_row["refined_part_target_sha256"], "unified-objective compare Part target drifted")
    require(objective_compare.get("camera_hash") == unified_row["camera_hash"], "unified-objective compare camera drifted")
    require(objective_compare.get("canonical_sha256") == unified_row["objective_compare_canonical_sha256"], "unified-objective compare canonical hash drifted")
    require(objective_compare.get("status") == unified_row["objective_compare_status"], "unified-objective compare status drifted")
    require(objective_compare.get("promotion_status") == unified_row["promotion_status"], "unified-objective promotion status drifted")
    require(objective_compare.get("strict_improvement") is unified_row["strict_improvement"], "unified-objective strict improvement was promoted")
    require(objective_compare.get("winner_candidate_id") is unified_row["winner_candidate_id"], "unified-objective winner was promoted")
    objective_candidates = objective_compare.get("candidates")
    require(isinstance(objective_candidates, list) and len(objective_candidates) == unified_row["candidate_count"], "unified-objective candidate comparisons are incomplete")
    require(all(row.get("global_metrics") and row.get("part_metrics") for row in objective_candidates if isinstance(row, dict)), "unified-objective metric rows are incomplete")
    require(any(row.get("global_non_regressing") is True and row.get("part_strict_improvement") is False for row in objective_candidates), "unified-objective global-only evidence disappeared")
    require(any(row.get("global_non_regressing") is False and row.get("part_strict_improvement") is True for row in objective_candidates), "unified-objective Part-only evidence disappeared")
    unified_comparisons = unified.get("candidate_comparisons")
    require(isinstance(unified_comparisons, list) and len(unified_comparisons) == unified_row["candidate_count"], "unified-objective visual candidate comparisons are incomplete")
    require(all(row.get("status") == unified_row["comparison_status"] for row in unified_comparisons if isinstance(row, dict)), "unified-objective visual status was promoted")
    require(unified.get("winner_candidate_id") is unified_row["winner_candidate_id"], "unified-objective winner field was promoted")
    require(unified.get("strict_improvement") is unified_row["strict_improvement"], "unified-objective top-level strict improvement drifted")
    require(unified.get("part_winner_candidate_id") is unified_row["part_winner_candidate_id"], "unified-objective Part winner was promoted")
    require(unified.get("part_strict_improvement") is unified_row["part_strict_improvement"], "unified-objective Part strict improvement drifted")
    require(unified.get("persistent_user_data_touched") is False, "unified-objective touched persistent data")
    require(unified.get("candidate_confirmed") in (None, False), "unified-objective crossed confirm boundary")
    require(unified.get("version_count") in (None, 0), "unified-objective created a version")
    require(unified_path != source_path and unified_path != part_path, "unified-objective receipt was merged with another cohort")

    optimization_row = ledger["same_cohort_unified_objective_optimization_transport"]
    optimization_path = evidence_path(optimization_row)
    optimization = load_json(optimization_path)
    require(optimization.get("status") == optimization_row["status"], "unified-objective OptimizationJob status drifted")
    require(optimization.get("reference_sha256") == optimization_row["reference_sha256"], "unified-objective OptimizationJob reference drifted")
    require(optimization.get("expected_build_cohort_sha256") == optimization_row["expected_build_cohort_sha256"], "unified-objective OptimizationJob expected cohort drifted")
    require(optimization.get("build_cohorts") == optimization_row["build_cohorts"], "unified-objective OptimizationJob worker bindings drifted")
    optimization_cohorts = optimization["build_cohorts"]
    require(set(optimization_cohorts) == {"mcp", "runtime", "geometry_worker", "render_worker"}, "unified-objective OptimizationJob worker keys drifted")
    require(len(set(optimization_cohorts.values())) == 1, "unified-objective OptimizationJob binaries are not unified")
    require(optimization_cohorts["mcp"] == optimization_row["expected_build_cohort_sha256"], "unified-objective OptimizationJob cohort does not match expected")
    for key in (
        "evaluations_count",
        "fidelity_counts",
        "baseline_loss",
        "best_loss",
        "strict_improvement",
        "proposal_status",
        "evaluation_objective_sha256",
        "promotion_policy",
        "promotion_status",
        "global_target_sha256",
        "target_sha256",
        "camera_hash",
        "comparison_camera_hash",
        "camera_binding_status",
        "comparison_status",
        "comparison_render_set_hash",
        "comparison_report_sha256",
        "comparison_metrics",
        "boolean_backend",
        "boolean_lane_candidate_indices",
        "boolean_lane_node_ids",
        "quality_claim",
        "candidate_confirmed",
        "version_count",
        "persistent_user_data_touched",
    ):
        require(optimization.get(key) == optimization_row[key], f"unified-objective OptimizationJob field drifted: {key}")
    require(optimization.get("evaluations_count") == 39, "unified-objective OptimizationJob evaluation count drifted")
    require(len(optimization.get("evaluation_object_sha256s") or []) == optimization_row["evaluations_count"], "unified-objective OptimizationJob evaluation checkpoint list is incomplete")
    require(all(isinstance(value, str) and len(value) == 64 for value in optimization["evaluation_object_sha256s"]), "unified-objective OptimizationJob evaluation object hash is invalid")
    require(optimization.get("camera_hash") == optimization.get("comparison_camera_hash"), "unified-objective OptimizationJob camera binding diverged")
    require(optimization.get("comparison_status") == "QUALITY_TARGET_NOT_MET", "unified-objective OptimizationJob visual quality was promoted")
    require(optimization.get("promotion_policy") == "silhouette-evaluation-objective-v1", "unified-objective OptimizationJob policy drifted")
    require(optimization.get("promotion_status") == "ready", "unified-objective OptimizationJob internal objective gate was not ready")
    require(optimization.get("proposal_status") == "proposed", "unified-objective OptimizationJob crossed proposal boundary")
    require(optimization.get("candidate_confirmed") is False and optimization.get("version_count") == 0, "unified-objective OptimizationJob crossed approval boundary")
    require(optimization.get("persistent_user_data_touched") is False, "unified-objective OptimizationJob touched persistent data")
    require(optimization_path != source_path and optimization_path != part_path and optimization_path != unified_path, "unified-objective OptimizationJob receipt was merged with another receipt")

    cadfit = source.get("cadfit_optimization")
    require(isinstance(cadfit, dict), "current CADFit result is missing")
    expected_counts = {"coarse": 32, "mid": 4, "final": 3}
    require(cadfit.get("evaluations_count") == source_row["cadfit"]["evaluations_count"], "CADFit evaluation count drifted")
    require(cadfit.get("fidelity_counts") == expected_counts, "CADFit fidelity counts drifted")
    require(cadfit.get("baseline_loss") == source_row["cadfit"]["baseline_loss"], "CADFit baseline loss drifted")
    require(cadfit.get("best_loss") == source_row["cadfit"]["best_loss"], "CADFit best loss drifted")
    require(cadfit.get("best_loss") < cadfit.get("baseline_loss"), "CADFit strict improvement is not numeric")
    require(cadfit.get("strict_improvement") is True, "CADFit strict improvement flag drifted")
    require(cadfit.get("job_status") == "succeeded", "CADFit job did not succeed")
    require(cadfit.get("proposal_status") == "proposed", "CADFit proposal boundary drifted")
    require(cadfit.get("result_object_sha256") is None, "CADFit result was confused with proposal object")

    historical_row = ledger["historical_provisional_observation"]
    historical_path = evidence_path(historical_row)
    historical = load_json(historical_path)
    require(historical_row["retained_as_historical"] is True, "historical observation was not retained")
    require(historical.get("status") == historical_row["status"], "historical status drifted")
    require(historical_row["camera_binding"] == "MISMATCH", "historical camera mismatch was silently removed")
    require(historical_row["benchmark_eligibility"] == "BLOCKED_INCOMPLETE_BINDING", "historical eligibility was promoted")
    require(historical_row["must_not_be_promoted_to_current_best"] is True, "historical promotion guard missing")
    require(historical_path != source_path, "historical and current receipts were merged")

    boolean_row = ledger["isolated_boolean_residual"]
    boolean_path = evidence_path(boolean_row)
    boolean = load_json(boolean_path)
    require(boolean.get("status") == "PASS", "Boolean residual receipt is not PASS")
    require(boolean.get("reference_sha256") == source_row["reference_sha256"], "Boolean reference is not the current authorized reference")
    require(boolean.get("boolean_backend") == boolean_row["backend"], "Boolean backend drifted")
    require(boolean.get("boolean_residual_mode") is True, "Boolean residual mode drifted")
    require(boolean.get("boolean_lane_candidate_indices") == boolean_row["candidate_indices"], "Boolean lane candidate coverage drifted")
    require(boolean.get("boolean_lane_node_ids") == [boolean_row["boolean_lane_node_id"]], "Boolean lane node binding drifted")
    require(boolean.get("evaluations_count") == boolean_row["evaluations_count"], "Boolean evaluation count drifted")
    require(boolean.get("fidelity_counts") == boolean_row["fidelity_counts"], "Boolean fidelity counts drifted")
    require(boolean.get("baseline_loss") == boolean_row["baseline_loss"], "Boolean baseline loss drifted")
    require(boolean.get("best_loss") == boolean_row["best_loss"], "Boolean best loss drifted")
    require(boolean.get("best_loss") < boolean.get("baseline_loss"), "Boolean strict improvement is not numeric")
    require(boolean.get("strict_improvement") is True, "Boolean strict improvement flag drifted")
    require(boolean.get("proposal_status") == "proposed", "Boolean proposal boundary drifted")
    require(boolean.get("candidate_confirmed") is False and boolean.get("version_count") == 0, "Boolean receipt crossed approval boundary")
    require(boolean.get("persistent_user_data_touched") is False, "Boolean receipt touched persistent user data")
    require("build_cohorts" not in boolean and "build_cohort_sha256" not in boolean, "Boolean receipt unexpectedly gained a cohort binding")
    require(boolean_row["cohort_binding"].startswith("NOT_EMITTED"), "Boolean cohort guard drifted")
    require(boolean_path != source_path, "Boolean receipt was merged with current source transport")

    boolean_gate_row = ledger["boolean_source_gate"]
    boolean_gate = load_json(evidence_path(boolean_gate_row))
    for key in ("boolean_union_difference_intersection", "negative_boolean_unsupported_shape", "negative_future_input", "lineage"):
        require(boolean_gate.get(key) == "PASS", f"Boolean source gate drifted: {key}")

    adoption_row = ledger["boolean_adoption_gate"]
    adoption = load_json(evidence_path(adoption_row))
    require(adoption.get("status") == adoption_row["status"], "Boolean adoption status drifted")
    require(adoption.get("source", {}).get("revision") == adoption_row["manifold_revision"], "Manifold revision drifted")
    require(adoption.get("checks", {}).get("meshgl_topology_readback") == adoption_row["checks"]["meshgl_topology_readback"], "Boolean MeshGL readback evidence drifted")
    require(adoption.get("checks", {}).get("determinism") == adoption_row["checks"]["determinism"], "Boolean determinism evidence drifted")
    require(adoption.get("checks", {}).get("resource_timeout_crash_fd") == adoption_row["checks"]["resource_timeout_crash_fd"], "Boolean resource evidence drifted")
    require(adoption.get("checks", {}).get("negative_input") == adoption_row["checks"]["negative_input"], "Boolean negative-input evidence drifted")
    require(adoption.get("checks", {}).get("removal_fallback") == adoption_row["checks"]["removal_fallback"], "Boolean fallback evidence drifted")
    require(adoption.get("checks", {}).get("negative_boolean_gate") == adoption_row["checks"]["negative_boolean_gate"], "Boolean fail-closed evidence drifted")
    product = adoption.get("product_integration")
    require(isinstance(product, dict), "Boolean product integration evidence is missing")
    require(product.get("source_vendored") is adoption_row["source_vendored"], "Boolean vendor boundary drifted")
    require(product.get("runtime_allowlist_change") is adoption_row["runtime_allowlist_change"], "Boolean Runtime allowlist boundary drifted")
    require(product.get("operator_catalog_activation") is adoption_row["operator_catalog_activation"], "Boolean catalog activation boundary drifted")
    require(adoption.get("persistent_user_data_touched") is adoption_row["persistent_user_data_touched"], "Boolean adoption touched persistent user data")
    require(adoption_row["cohort_binding"] != source_row["build_cohorts"]["mcp"], "Historical Boolean adoption was falsely promoted to current cohort")

    action_row = ledger["action_run_quality_gate"]
    action = load_json(evidence_path(action_row))
    require(action.get("status") == action_row["status"], "ActionRun status drifted")
    require(action.get("run_status") == "blocked" and action.get("completed_stage") == "render", "ActionRun stop boundary drifted")
    require(action.get("source_visual_status") == "QUALITY_TARGET_NOT_MET", "ActionRun visual status drifted")
    require(action.get("repair_probe_status") == "blocked", "ActionRun Repair boundary drifted")
    require(action.get("proposal") is None, "ActionRun direct Repair receipt unexpectedly claims a materialized proposal")
    require(action_row["direct_repair_proposal_materialized"] is False, "ActionRun direct Repair proposal boundary drifted")
    require(action.get("run_sha256") == action.get("replay_sha256") == action_row["run_sha256"], "ActionRun replay hash is not deterministic")
    require(action_row["deterministic_replay"] is True, "ActionRun deterministic replay guard drifted")
    require(action.get("candidate_confirmed") is False and action.get("version_count") == 0, "ActionRun crossed approval boundary")
    require(action.get("persistent_user_data_touched") is False, "ActionRun touched persistent user data")
    require(action_row["cohort_binding"] != source_row["build_cohorts"]["mcp"], "ActionRun was falsely merged into current cohort")
    require(action_row["must_not_be_described_as_direct_repair_proposal"] is True, "ActionRun direct Repair wording guard missing")

    child_row = action_row["cadfit_child"]
    child = action.get("optimization")
    require(isinstance(child, dict), "ActionRun CADFit child continuation is missing")
    require(child.get("optimization_job_status") == child_row["status"], "ActionRun CADFit child status drifted")
    require(child.get("optimization_result_status") == "succeeded", "ActionRun CADFit result did not succeed")
    require(child.get("evaluations_count") == child_row["evaluations_count"], "ActionRun CADFit evaluation count drifted")
    require(child.get("fidelity_counts") == child_row["fidelity_counts"], "ActionRun CADFit fidelity counts drifted")
    require(child.get("baseline_loss") == child_row["baseline_loss"], "ActionRun CADFit baseline loss drifted")
    require(child.get("best_loss") == child_row["best_loss"], "ActionRun CADFit best loss drifted")
    require(child.get("strict_improvement") is True, "ActionRun CADFit strict improvement drifted")
    require(child.get("proposal_status") == child_row["proposal_status"], "ActionRun CADFit proposal status drifted")
    require(child.get("source_candidate_unchanged") is True, "ActionRun CADFit source candidate was mutated")
    require(child.get("version_count") == child_row["version_count"], "ActionRun CADFit version boundary drifted")
    require(child.get("run_sha256") == child.get("replay_sha256") == child_row["run_sha256"], "ActionRun CADFit replay is not deterministic")

    continuation = child.get("proposal_continuation")
    require(isinstance(continuation, dict), "ActionRun CADFit proposal continuation is missing")
    require(continuation.get("status") == child_row["proposal_continuation_status"], "ActionRun CADFit continuation status drifted")
    require(continuation.get("reason_code") == child_row["proposal_reason_code"], "ActionRun CADFit continuation reason drifted")
    require(continuation.get("proposal_job_id") == child_row["proposal_job_id"], "ActionRun CADFit proposal job drifted")
    require(continuation.get("proposal_candidate_id") == child_row["proposal_candidate_id"], "ActionRun CADFit proposal candidate drifted")
    require(continuation.get("proposal_candidate_state_sha256") == child_row["proposal_candidate_state_sha256"], "ActionRun CADFit proposal candidate state drifted")
    require(continuation.get("visual_status") == child_row["proposal_visual_status"], "ActionRun CADFit proposal visual status drifted")
    require(continuation.get("visual_gate_passed") is False, "ActionRun CADFit proposal visual gate unexpectedly passed")
    require(continuation.get("confirm_allowed") is child_row["confirm_allowed"], "ActionRun CADFit confirm boundary drifted")
    require(continuation.get("repair_apply_status") == child_row["repair_apply_status"], "ActionRun CADFit Repair apply boundary drifted")
    require(continuation.get("replay_sha256") == child_row["proposal_replay_sha256"], "ActionRun CADFit proposal replay hash drifted")

    restart_row = ledger["restart_readback"]
    restart = load_json(evidence_path(restart_row))
    require(restart.get("status") == restart_row["status"], "restart receipt status drifted")
    require(restart.get("run_status") == "completed", "restart readback did not complete")
    require(restart.get("candidate_confirmed") is False and restart.get("version_count") == 0, "restart readback crossed approval boundary")
    require(restart_row["not_a_current_cadfit_restart_hash"] is True, "restart receipt was over-promoted")

    boundary = ledger["quality_boundary"]
    require(boundary["visual_quality_claim"] == "NOT_CLAIMED", "visual claim boundary drifted")
    require(boundary["human_review"] == "NOT_RUN", "human review boundary drifted")
    require(boundary["pbr_likeness"] == "NOT_RUN", "PBR boundary drifted")
    require(boundary["export_restart_hash"] == "NOT_RUN", "export/restart boundary drifted")
    require(boundary["hq_360"] == "BLOCKED_REFERENCE_COVERAGE", "360 boundary drifted")

    print(json.dumps({
        "schema_version": "ForgeCADMCP010FCurrentQualityEvidenceGate@1",
        "status": "PASS",
        "ledger": str(LEDGER.relative_to(ROOT)),
        "current_source_cohort": cohorts["mcp"],
        "current_cadfit": "PASS_TRANSPORT_PROPOSAL_ONLY",
        "same_cohort_surface_signal": "PASS_SURFACE_SIGNAL_CADFIT_BOOLEAN_TRANSPORT_ONLY",
        "same_cohort_action_run": "PASS_ACTION_RUN_CADFIT_HANDOFF_CAMERA_REBIND_REQUIRED",
        "same_cohort_unified_objective": "PASS_UNIFIED_OBJECTIVE_TRANSPORT_BLOCKED_PROMOTION",
        "same_cohort_unified_objective_optimization": "PASS_UNIFIED_OBJECTIVE_CADFIT_READY_QUALITY_TARGET_NOT_MET",
        "current_visual_quality": "QUALITY_TARGET_NOT_MET",
        "historical_observation": "RETAINED_BLOCKED_INCOMPLETE_BINDING",
        "boolean_residual": "PASS_BOUNDED_NO_COHORT_PROMOTION",
        "boolean_adoption": "PASS_ISOLATED_DETERMINISM_RESOURCE_NEGATIVE_GATES",
        "action_run": "PASS_DIRECT_REPAIR_BLOCKED_CADFIT_PROPOSAL_MATERIALIZED_FOR_REVIEW",
        "restart_readback": "PASS_DURABLE_ACTION_RUN_READBACK_NOT_CADFIT_HASH",
    }, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
