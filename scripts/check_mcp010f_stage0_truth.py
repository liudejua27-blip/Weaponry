#!/usr/bin/env python3
"""Validate the MCP010F Stage 0 source and provisional observation truth.

This gate intentionally separates four facts that used to drift across the
documentation:

* the current checked-in contract and MCP tool surface;
* the provisional visible-view observation receipt, whose benchmark eligibility
  remains blocked until its incomplete bindings are repaired;
* the newest transport receipt, which is not automatically promoted;
* the packaged Viewer receipt, which is not yet bound to that observation.

It does not run ForgeCAD, score images, mutate Runtime/CAS state, or turn a
failed visual candidate into a passing one.

The MCP tool inventory is taken from a receipt emitted by the compiled
`--tool-manifest-summary` path. A source parser is retained only as a second,
independent drift check; it is not the count authority.
"""

from __future__ import annotations

import hashlib
import json
import math
import re
import shlex
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
TRUTH_PATH = ROOT / "docs/evidence/mcp010f/current-benchmark-truth.json"
CONTRACT_MANIFEST = ROOT / "packages/forgecad-contracts/manifest.json"
SCHEMA_ROOT = ROOT / "packages/forgecad-contracts/schemas"
MCP_SOURCE = ROOT / "apps/desktop/src-tauri/crates/forgecad-mcp/src/main.rs"
RUNTIME_SOURCE = ROOT / "apps/desktop/src-tauri/crates/forgecad-runtime/src/lib.rs"
VIEWER_SOURCE = ROOT / "apps/desktop/src/features/runtime-viewer/RuntimeViewer.tsx"
FIT_PLAN_SOURCE = ROOT / "scripts/build_mcp010f_fit_plan.py"
TOOL_SUMMARY_PATH = ROOT / "docs/evidence/mcp010f/source-tool-manifest-summary.json"
RUN_INVENTORY_PATH = ROOT / "docs/evidence/mcp010f/real-codex-run-inventory.json"
EVIDENCE_MANIFEST_PATH = ROOT / "docs/evidence/mcp010f/manifest.json"
EXPECTED_EVIDENCE_MANIFEST_SHA256 = "9d7a41b610715c1ad3bb5f97f71959c22311acf0094c9d4f7f5118933a821843"
TASK_INDEX = ROOT / "docs/CODEX_TASK_INDEX.md"

AUTHORITY_DOCS = (
    "docs/DOCUMENTATION_STATUS.md",
    "docs/CODEX_HANDOFF.md",
    "docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md",
    "docs/AUTHORITATIVE_STATE.md",
    "docs/MVP_DELIVERY_PLAN.md",
    "docs/MVP_TOOL_CATALOG.md",
    "docs/LUNA_GOAL_EXECUTION_GUIDE.md",
    "docs/MCP_RUNTIME_CONTRACT.md",
    "docs/SCHEMAS.md",
    "docs/WORKBENCH_VIEWER.md",
    "docs/CODEX_TASK_INDEX.md",
    "docs/TEST_STRATEGY.md",
    "docs/evidence/CAPABILITY_GATE_MATRIX.md",
)

WRITE_NAME_FUNCTIONS = (
    "mcp004_write_tool_names",
    "mcp005_write_tool_names",
    "mcp007_write_tool_names",
    "mcp008_write_tool_names",
    "mcp009_write_tool_names",
    "mcp010c_write_tool_names",
    "mcp010f_write_tool_names",
)

METRIC_CRITERIA = {
    "silhouette_iou": ("min", "silhouette_iou_min"),
    "boundary_f1_4px": ("min", "boundary_f1_4px_min"),
    "bbox_edge_error": ("max", "bbox_edge_error_max"),
    "centroid_error": ("max", "centroid_error_max"),
    "landmark_coverage": ("min", "landmark_coverage_min"),
    "landmark_nme": ("max", "landmark_nme_max"),
    "region_median_iou": ("min", "region_median_iou_min"),
    "critical_region_min_iou": ("min", "critical_region_min_iou_min"),
}

RUNTIME_THRESHOLD_CONSTANTS = {
    "VISIBLE_SILHOUETTE_IOU_MIN": "silhouette_iou_min",
    "VISIBLE_BOUNDARY_F1_MIN": "boundary_f1_4px_min",
    "VISIBLE_BBOX_EDGE_ERROR_MAX": "bbox_edge_error_max",
    "VISIBLE_CENTROID_ERROR_MAX": "centroid_error_max",
    "VISIBLE_LANDMARK_COVERAGE_MIN": "landmark_coverage_min",
    "VISIBLE_LANDMARK_NME_MAX": "landmark_nme_max",
    "VISIBLE_REGION_MEDIAN_IOU_MIN": "region_median_iou_min",
    "VISIBLE_CRITICAL_REGION_IOU_MIN": "critical_region_min_iou_min",
}

TRUTH_TOP_LEVEL_KEYS = frozenset(
    "assertion_ledger authority_docs auxiliary_runs canonical_sha256 current_source evidence_manifest "
    "evidence_status latest_attempt latest_completed_transport observation_id packaged_viewer phase_zero "
    "provisional_retained_observation purpose real_codex_run_inventory recorded_on schema_version task_id".split()
)
ASSERTION_KEYS = frozenset(f"BT{index:03d}_{suffix}" for index, suffix in enumerate((
    "COHORT_EQUAL", "PROJECT_PROPAGATION", "CANDIDATE_PROPAGATION", "PROGRAM_CATALOG_BINDING",
    "ARTIFACT_BINDING", "CAMERA_BINDING", "TARGET_BINDING", "AOV_ORDER", "AOV_HASH_COMPLETENESS",
    "METRIC_EXACT_SET", "THRESHOLD_EXACT_SET_IN_RECEIPT", "STATUS_DERIVATION", "NO_APPEARANCE_CLAIM",
    "UNRUN_EXPLICITNESS", "NO_CROSS_RUN_FIELD_BORROW", "SURFACE_RAW_PAIR", "ARMOR_RAW_PAIR",
    "MATERIAL_PREDECESSOR_BINDING", "BENCHMARK_ELIGIBILITY", "LEGACY_RECEIPT_RECORDED_AT",
), start=1))
OBSERVATION_KEYS = frozenset(
    "aov_order artifact_id artifact_readback_canonical_sha256 artifact_sha256 benchmark_eligibility "
    "build_cohorts camera_binding candidate_canonical_sha256 candidate_id catalog_sha256 comparison_hash_kind "
    "comparison_report_hash confirmation_eligibility current_candidate_visible_view_gate export_restart_hash "
    "geometry_route geometry_variant hq_360 human_review material_variant metric_gate_results metrics part_count "
    "pbr_material_pack persistent_user_data_touched program_sha256 project_id quality_visual_status "
    "receipt_completeness reference_id reference_sha256 render_hash_kind render_pass_image_blocks render_set_hash "
    "selection_policy semantic_claim silhouette_camera_hash silhouette_rig_sha256 silhouette_target_sha256 "
    "source_receipt_path source_receipt_sha256 status strict_visible_view_policy_implemented threshold_binding "
    "thresholds triangle_count validator_status view_spec_sha256 visual_intake visual_review_status".split()
)
EVIDENCE_MANIFEST_GATE_KEYS = frozenset(
    "boundary_error_runtime camera_fit_runtime codex_correction_queue comparison_sheet_helper contour_canvas "
    "contour_draft_binding_validator contour_first_workflow_display contour_target_runtime difference_heatmap "
    "export_restart_hash fit_plan_helper full_360_reference human_visual_review latest_attempt latest_completed_transport "
    "packaged_current_cohort_contour_rebuild packaged_current_cohort_viewer packaged_viewer_core_controls "
    "packaged_viewer_provisional_observation_binding packaged_viewer_read_model packaged_viewer_window "
    "part_aware_rig_proposal part_contour_fit_runtime part_contour_target_slice_runtime part_correction_source_probe "
    "provisional_observation_benchmark_eligibility provisional_observation_camera_binding "
    "provisional_observation_truth_binding provisional_observation_visible_view_gate real_codex_camera_ref_transport "
    "real_codex_image_block_observation real_codex_landmark_aware_rig_fit real_codex_rig_fit_expanded_transport "
    "real_codex_rig_fit_review_recovery_transport real_codex_rig_fit_transport real_codex_silhouette_first "
    "real_codex_single_part_attempt36 reference_contour_aid silhouette_candidate_compare_runtime "
    "silhouette_fit_runtime silhouette_part_error_runtime silhouette_rig_hash_runtime stage0_truth_integrity "
    "strict_visible_view_policy_implemented viewer_accessibility_e2e viewer_browser_dom_smoke "
    "viewer_contour_annotation viewer_contour_real_execution viewer_keyboard_navigation viewer_native_window_smoke "
    "viewer_source_contract viewer_tauri_compile viewer_typescript_build viewer_write_boundary".split()
)
EXPECTED_EVIDENCE_MANIFEST_GATES = {
    "boundary_error_runtime": "PASS_DIRECTIONAL_SDF_SEGMENT_EVIDENCE",
    "camera_fit_runtime": "PASS_BOUNDED_TYPED_CAMERA_SEARCH",
    "codex_correction_queue": "PASS_SOURCE_READ_ONLY_HASH_BOUND_INTENTS",
    "comparison_sheet_helper": "PASS_SOURCE_STANDARD_LIBRARY_HASH_ONLY_MANIFEST",
    "contour_canvas": "PASS_SOURCE_SILHOUETTE_AOV_OVERLAY",
    "contour_draft_binding_validator": "PASS_SOURCE_HASH_BOUND_SINGLE_PART_INTENT",
    "contour_first_workflow_display": "PASS_SOURCE_UI_DERIVED_CUMULATIVE_GATES",
    "contour_target_runtime": "PASS_HASH_BOUND_AUTOMATIC_AND_USER_REFINED",
    "difference_heatmap": "PASS_SOURCE_EPHEMERAL_PIXEL_DIFF_512X512",
    "export_restart_hash": "NOT_RUN",
    "fit_plan_helper": "PASS_SOURCE_STANDARD_LIBRARY_HASH_BOUND_INTENTS_ONLY",
    "full_360_reference": "BLOCKED_REFERENCE_COVERAGE",
    "human_visual_review": "NOT_RUN",
    "latest_attempt": "BLOCKED_WITHOUT_QUALITY_RESULT",
    "latest_completed_transport": "PASS_WITH_QUALITY_TARGET_NOT_MET_METRIC_SEMANTICS_CHANGED_NOT_PROMOTED",
    "packaged_current_cohort_contour_rebuild": "PASS_AD_HOC_DEEP_STRICT_ISOLATED_READY_WINDOW",
    "packaged_current_cohort_viewer": "PASS_STRUCTURAL_READ_MODEL_UI_NOT_RUN",
    "packaged_viewer_core_controls": "PASS_PACKAGED_AX_CORE_CONTROLS",
    "packaged_viewer_provisional_observation_binding": "NOT_RUN_DIFFERENT_COHORT_AND_ARTIFACT",
    "packaged_viewer_read_model": "PASS_STRUCTURAL: same-cohort Dev.app CLI read-only projection over an isolated user-reference candidate",
    "packaged_viewer_window": "PASS_STRUCTURAL_WINDOW: same-cohort Dev.app opened ForgeCAD Runtime Viewer at 1296x803 over an isolated ready Runtime",
    "part_aware_rig_proposal": "PASS_RUNTIME_LOCAL_PART_ENVELOPE_WITH_GLOBAL_FALLBACK",
    "part_contour_fit_runtime": "PASS_SINGLE_PART_READ_ONLY_PROPOSAL",
    "part_contour_target_slice_runtime": "PASS_DISJOINT_TARGET_SLICE_AND_PART_BOUNDARY_ATTRIBUTION",
    "part_correction_source_probe": "PASS_TRANSPORT_WITH_METRICS_BEST_EFFORT_IOU_0.7459_NOT_QUALITY_PASS",
    "provisional_observation_benchmark_eligibility": "BLOCKED_INCOMPLETE_BINDING",
    "provisional_observation_camera_binding": "MISMATCH_FIT_VS_COMPARISON_CAMERA",
    "provisional_observation_truth_binding": "INCOMPLETE_TRUTH_BINDING",
    "provisional_observation_visible_view_gate": "FAIL_QUALITY_TARGET_NOT_MET",
    "real_codex_camera_ref_transport": "PASS_WITH_QUALITY_TARGET_NOT_MET_CURRENT_SOURCE_BUILT",
    "real_codex_image_block_observation": "NOT_OBSERVED_IN_SANITIZED_CLI_EVENTS",
    "real_codex_landmark_aware_rig_fit": "PASS_WITH_QUALITY_TARGET_NOT_MET_NOT_PROMOTED",
    "real_codex_rig_fit_expanded_transport": "BLOCKED_REVIEW_TOOL_DRIFT",
    "real_codex_rig_fit_review_recovery_transport": "PASS_WITH_QUALITY_TARGET_NOT_MET_NOT_BENCHMARK_ELIGIBLE",
    "real_codex_rig_fit_transport": "PASS_WITH_QUALITY_TARGET_NOT_MET",
    "real_codex_silhouette_first": "PASS_WITH_QUALITY_TARGET_NOT_MET",
    "real_codex_single_part_attempt36": "BLOCKED_SETUP_AND_DETAIL_TURN_TIMEOUT",
    "reference_contour_aid": "PASS_SOURCE_EPHEMERAL_BORDER_FLOOD_FILL_AID",
    "silhouette_candidate_compare_runtime": "PASS_HASH_BOUND_TWO_TO_EIGHT_COMPARE",
    "silhouette_fit_runtime": "PASS_BOUNDED_RIG_CAMERA_AND_GEOMETRY_VARIANT_SEARCH",
    "silhouette_part_error_runtime": "PASS_HASH_BOUND_MULTI_PART_ERROR_TABLE",
    "silhouette_rig_hash_runtime": "PASS_RUNTIME_OWNED_CANDIDATE_BOUND_CANONICAL_HASH",
    "stage0_truth_integrity": "PASS_MACHINE_READABLE_DRIFT_AND_CROSS_RUN_ISOLATION",
    "strict_visible_view_policy_implemented": "PASS_RUNTIME_OWNED_IOU_0.90_BOUNDARY_F1_0.90_BBOX_CENTROID_0.02_LANDMARK_0.80_NME_0.03_REGION_0.85_CRITICAL_0.85",
    "viewer_accessibility_e2e": "NOT_RUN",
    "viewer_browser_dom_smoke": "PASS_ISOLATED_VITE_BROWSER_DOM_SMOKE",
    "viewer_contour_annotation": "PASS_EPHEMERAL_NORMALIZED_POINTER_DRAFT",
    "viewer_contour_real_execution": "PASS_TRANSPORT_WITH_QUALITY_TARGET_NOT_MET",
    "viewer_keyboard_navigation": "PASS_TABLIST_ARROW_HOME_END",
    "viewer_native_window_smoke": "PASS_STRUCTURAL_NATIVE_WINDOW_VISUAL_SMOKE",
    "viewer_source_contract": "PASS",
    "viewer_tauri_compile": "PASS",
    "viewer_typescript_build": "PASS",
    "viewer_write_boundary": "PASS",
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"MCP010F Stage 0 truth violation: {message}")


def reject_duplicate_object_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, child in pairs:
        require(key not in value, f"duplicate JSON object key: {key}")
        value[key] = child
    return value


def load_json(path: Path) -> dict[str, Any]:
    require(path.is_file(), f"missing JSON evidence: {path.relative_to(ROOT)}")
    value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicate_object_keys)
    require(isinstance(value, dict), f"expected a JSON object: {path.relative_to(ROOT)}")
    return value


def require_exact_keys(value: Any, expected: frozenset[str], label: str) -> None:
    require(isinstance(value, dict), f"{label} must be an object")
    actual = set(value)
    require(
        actual == expected,
        f"{label} key set drifted: missing={sorted(expected - actual)} extra={sorted(actual - expected)}",
    )


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical_sha256(value: dict[str, Any]) -> str:
    payload = dict(value)
    payload.pop("canonical_sha256", None)
    encoded = json.dumps(
        payload,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def contract_schema_content_set_sha256(paths: list[Path]) -> str:
    rows = [
        {"path": path.name, "sha256": sha256_file(path)}
        for path in sorted(paths, key=lambda item: item.name)
    ]
    encoded = json.dumps(rows, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def source_tool_names() -> tuple[list[str], list[str]]:
    source = MCP_SOURCE.read_text(encoding="utf-8")
    read_start = source.find("fn read_only_tools()")
    read_end = source.find("\nfn tool(", read_start)
    require(read_start >= 0 and read_end > read_start, "cannot locate read_only_tools source")
    read_names = re.findall(r'\btool\(\s*"([a-z0-9_]+)"', source[read_start:read_end])

    write_names: list[str] = []
    for function_name in WRITE_NAME_FUNCTIONS:
        match = re.search(
            rf"fn {re.escape(function_name)}\(\) -> Vec<String> \{{(.*?)\n\}}",
            source,
            flags=re.DOTALL,
        )
        require(match is not None, f"cannot locate {function_name}")
        names = re.findall(r'"([a-z0-9_]+)"', match.group(1))
        require(names, f"{function_name} contains no tool names")
        write_names.extend(names)

    require(len(read_names) == len(set(read_names)), "duplicate read-only tool names")
    require(len(write_names) == len(set(write_names)), "duplicate write tool names")
    require(not set(read_names) & set(write_names), "a tool is classified as both read and write")
    return sorted(read_names), sorted(write_names)


def runtime_visible_view_thresholds() -> dict[str, float]:
    source = RUNTIME_SOURCE.read_text(encoding="utf-8")
    thresholds: dict[str, float] = {}
    for constant, truth_name in RUNTIME_THRESHOLD_CONSTANTS.items():
        match = re.search(rf"const {re.escape(constant)}: f64 = ([0-9]+(?:\.[0-9]+)?);", source)
        require(match is not None, f"Runtime visible-view threshold is missing: {constant}")
        thresholds[truth_name] = float(match.group(1))
    return thresholds


def fit_plan_visible_view_thresholds() -> dict[str, float]:
    source = FIT_PLAN_SOURCE.read_text(encoding="utf-8")
    thresholds: dict[str, float] = {}
    for metric_name, (direction, threshold_name) in METRIC_CRITERIA.items():
        expected_operator = ">=" if direction == "min" else "<="
        match = re.search(
            rf'"{re.escape(metric_name)}"\s*:\s*\("(>=|<=)",\s*([0-9]+(?:\.[0-9]+)?)\)',
            source,
        )
        require(match is not None, f"fit-plan threshold is missing: {metric_name}")
        require(match.group(1) == expected_operator, f"fit-plan operator drifted: {metric_name}")
        thresholds[threshold_name] = float(match.group(2))
    return thresholds


def viewer_visible_view_thresholds() -> dict[str, float]:
    source = VIEWER_SOURCE.read_text(encoding="utf-8")
    thresholds: dict[str, float] = {}
    for metric_name, (direction, threshold_name) in METRIC_CRITERIA.items():
        expected_operator = ">=" if direction == "min" else "<="
        match = re.search(
            rf'{re.escape(metric_name)}\s*:\s*\{{\s*operator:\s*[\'\"](>=|<=)[\'\"]\s*,\s*threshold:\s*([0-9]+(?:\.[0-9]+)?)\s*\}}',
            source,
        )
        require(match is not None, f"Viewer threshold is missing: {metric_name}")
        require(match.group(1) == expected_operator, f"Viewer operator drifted: {metric_name}")
        thresholds[threshold_name] = float(match.group(2))
    return thresholds


def task_rows() -> dict[str, dict[str, str]]:
    rows: dict[str, dict[str, str]] = {}
    pattern = re.compile(
        r"^\|\s*(FGC-MCP[0-9]+[A-Z]?)\s*\|\s*"
        r"(ready|in_progress|blocked|done|superseded)\s*\|\s*([^|]*)\|"
    )
    for line in TASK_INDEX.read_text(encoding="utf-8").splitlines():
        match = pattern.match(line)
        if match:
            task_id = match.group(1)
            require(task_id not in rows, f"duplicate task row: {task_id}")
            rows[task_id] = {"status": match.group(2), "dependency": match.group(3).strip()}
    require(rows, "no task rows were parsed from CODEX_TASK_INDEX.md")
    return rows


def metric_gate_results(metrics: dict[str, Any], thresholds: dict[str, Any]) -> dict[str, str]:
    results: dict[str, str] = {}
    for metric_name, (direction, threshold_name) in METRIC_CRITERIA.items():
        require(metric_name in metrics, f"retained receipt is missing metric {metric_name}")
        require(threshold_name in thresholds, f"truth is missing threshold {threshold_name}")
        measured = float(metrics[metric_name])
        threshold = float(thresholds[threshold_name])
        passed = measured >= threshold if direction == "min" else measured <= threshold
        results[metric_name] = "PASS" if passed else "FAIL"
    return results


def tool_calls(receipt: dict[str, Any], tool_name: str) -> list[dict[str, Any]]:
    return [call for call in receipt.get("mcp_tool_calls", []) if call.get("tool") == tool_name]


def single_tool_call(receipt: dict[str, Any], tool_name: str) -> dict[str, Any]:
    calls = tool_calls(receipt, tool_name)
    require(len(calls) == 1, f"expected exactly one {tool_name} call, found {len(calls)}")
    return calls[0]


def auxiliary_binding_tuple(receipt: dict[str, Any]) -> tuple[Any, ...]:
    comparison = receipt.get("comparison") or receipt.get("reference_compare") or {}
    return (
        receipt.get("geometry_program_sha256"),
        receipt.get("geometry_artifact_sha256"),
        receipt.get("appearance_artifact_sha256"),
        comparison.get("render_set_hash"),
        comparison.get("comparison_report_hash"),
        comparison.get("metrics"),
    )


def compute_assertion_ledger(truth: dict[str, Any], retained: dict[str, Any]) -> dict[str, str]:
    retained_truth = truth["provisional_retained_observation"]
    calls = retained.get("mcp_tool_calls", [])

    cohorts = list(retained.get("build_cohorts", {}).values())
    cohort_equal = len(cohorts) == 3 and len(set(cohorts)) == 1

    project_values = [
        value
        for call in calls
        for value in (call.get("project_id"), call.get("fit_argument_project_id"))
        if value is not None
    ]
    project_propagation = bool(project_values) and all(value == retained.get("project_id") for value in project_values)

    candidate_values = [call.get("candidate_id") for call in calls if call.get("candidate_id") is not None]
    geometry_call = single_tool_call(retained, "geometry_prepare")
    artifact = geometry_call.get("artifact", {})
    if artifact.get("candidate_id") is not None:
        candidate_values.append(artifact["candidate_id"])
    candidate_propagation = bool(candidate_values) and all(value == retained.get("candidate_id") for value in candidate_values)

    program_call = single_tool_call(retained, "geometry_program_hash")
    catalog_call = single_tool_call(retained, "operator_catalog_get")
    program_catalog_binding = (
        retained.get("program_sha256") == program_call.get("canonical_sha256")
        and retained.get("catalog_sha256") == catalog_call.get("canonical_sha256")
    )

    artifact_binding = (
        retained.get("artifact_id") == artifact.get("artifact_id")
        and retained.get("candidate_id") == artifact.get("candidate_id")
        and retained.get("triangle_count") == artifact.get("triangle_count")
        and retained.get("validator_status") == artifact.get("validator_status")
        and retained.get("part_count") == len(artifact.get("part_ids", []))
    )

    compare_call = single_tool_call(retained, "reference_compare_prepare")
    compare_camera = compare_call.get("camera", {})
    camera_binding = retained.get("silhouette_camera_hash") == compare_camera.get("camera_hash")

    target_values = [
        value
        for call in calls
        for value in (call.get("target_sha256"), call.get("fit_argument_target_sha256"))
        if value is not None
    ]
    target_binding = bool(target_values) and all(value == retained.get("silhouette_target_sha256") for value in target_values)

    aov_order = retained_truth.get("aov_order", [])
    render_calls = tool_calls(retained, "render_pass_get")
    aov_order_pass = (
        retained.get("render_pass_calls") == 9
        and len(render_calls) == 9
        and retained.get("render_pass_order") == aov_order
    )
    aov_hashes_complete = all(
        isinstance(call.get("sha256"), str)
        and len(call["sha256"]) == 64
        and call.get("width") == 512
        and call.get("height") == 512
        and call.get("render_set_hash") == retained.get("render_set_hash")
        for call in render_calls
    )

    metrics = retained.get("comparison_metrics", {})
    metric_exact = set(metrics) == set(METRIC_CRITERIA) and all(
        isinstance(value, (int, float)) and math.isfinite(float(value)) for value in metrics.values()
    )
    receipt_thresholds = retained.get("thresholds")
    threshold_exact = (
        isinstance(receipt_thresholds, dict)
        and set(receipt_thresholds) == {item[1] for item in METRIC_CRITERIA.values()}
        and all(isinstance(value, (int, float)) and math.isfinite(float(value)) for value in receipt_thresholds.values())
        and isinstance(retained.get("threshold_revision"), str)
    )

    metric_results = metric_gate_results(metrics, retained_truth["thresholds"])
    numeric_pass = all(value == "PASS" for value in metric_results.values())
    status_derivation = (
        not numeric_pass
        and retained.get("quality_hard_gate_passed") is False
        and retained.get("quality_visual_status") == "QUALITY_TARGET_NOT_MET"
        and retained.get("visual_review_status") == "needs_revision"
    )

    appearance_calls = tool_calls(retained, "appearance_prepare")
    forbidden_downstream_calls = (
        tool_calls(retained, "candidate_confirm")
        + tool_calls(retained, "export_prepare")
        + tool_calls(retained, "export_confirm")
    )
    no_appearance_claim = (
        not appearance_calls
        and retained.get("pbr_material_pack") == "NOT_RUN"
        and retained.get("detail_material_stages") == "LOCKED_UNTIL_SILHOUETTE_GATE"
        and not forbidden_downstream_calls
    )
    unrun_keys = (
        "candidate_confirm",
        "export",
        "restart_hash",
        "packaged_reference_visual_e2e",
        "viewer_accessibility_e2e",
    )
    unrun_explicit = all(key in retained for key in unrun_keys)

    auxiliaries = truth["auxiliary_runs"]
    surface = load_json(ROOT / auxiliaries["surface_linework"]["curated_path"])
    surface_raw = load_json(ROOT / auxiliaries["surface_linework"]["raw_path"])
    armor = load_json(ROOT / auxiliaries["armor_shell_zones"]["curated_path"])
    armor_raw = load_json(ROOT / auxiliaries["armor_shell_zones"]["raw_path"])
    primary_tuple = (
        retained.get("program_sha256"),
        retained.get("artifact_sha256"),
        None,
        retained.get("render_set_hash"),
        retained.get("comparison_report_hash"),
        retained.get("comparison_metrics"),
    )
    no_cross_run_borrow = (
        auxiliaries["surface_linework"]["relation_to_primary"] == "UNBOUND_SEPARATE_RUN"
        and auxiliaries["armor_shell_zones"]["relation_to_primary"] == "SELF_CONSISTENT_AUXILIARY_RUN"
        and primary_tuple != auxiliary_binding_tuple(surface)
        and primary_tuple != auxiliary_binding_tuple(armor)
    )
    surface_pair = auxiliary_binding_tuple(surface) == auxiliary_binding_tuple(surface_raw)
    armor_pair = auxiliary_binding_tuple(armor) == auxiliary_binding_tuple(armor_raw)

    return {
        "BT001_COHORT_EQUAL": "PASS" if cohort_equal else "FAIL",
        "BT002_PROJECT_PROPAGATION": "PASS" if project_propagation else "FAIL",
        "BT003_CANDIDATE_PROPAGATION": "PASS" if candidate_propagation else "FAIL",
        "BT004_PROGRAM_CATALOG_BINDING": "PASS" if program_catalog_binding else "FAIL",
        "BT005_ARTIFACT_BINDING": "PASS" if artifact_binding else "FAIL",
        "BT006_CAMERA_BINDING": "PASS" if camera_binding else "FAIL",
        "BT007_TARGET_BINDING": "PASS" if target_binding else "FAIL",
        "BT008_AOV_ORDER": "PASS" if aov_order_pass else "FAIL",
        "BT009_AOV_HASH_COMPLETENESS": "PASS" if aov_hashes_complete else "MISSING",
        "BT010_METRIC_EXACT_SET": "PASS" if metric_exact else "FAIL",
        "BT011_THRESHOLD_EXACT_SET_IN_RECEIPT": "PASS" if threshold_exact else "MISSING",
        "BT012_STATUS_DERIVATION": "PASS" if status_derivation else "FAIL",
        "BT013_NO_APPEARANCE_CLAIM": "PASS" if no_appearance_claim else "FAIL",
        "BT014_UNRUN_EXPLICITNESS": "PASS" if unrun_explicit else "MISSING",
        "BT015_NO_CROSS_RUN_FIELD_BORROW": "PASS" if no_cross_run_borrow else "FAIL",
        "BT016_SURFACE_RAW_PAIR": "PASS" if surface_pair else "FAIL",
        "BT017_ARMOR_RAW_PAIR": "PASS" if armor_pair else "FAIL",
        "BT018_MATERIAL_PREDECESSOR_BINDING": "MISSING"
        if auxiliaries["armor_shell_zones"]["predecessor_geometry_binding"] == "MISSING"
        else "PASS",
        "BT019_BENCHMARK_ELIGIBILITY": "MISSING"
        if retained_truth["benchmark_eligibility"] == "BLOCKED_INCOMPLETE_BINDING"
        else "PASS",
        "BT020_LEGACY_RECEIPT_RECORDED_AT": "MISSING",
    }


def check_receipt_binding(truth: dict[str, Any]) -> None:
    retained_truth = truth["provisional_retained_observation"]
    retained_path = ROOT / retained_truth["source_receipt_path"]
    retained = load_json(retained_path)
    require(
        sha256_file(retained_path) == retained_truth["source_receipt_sha256"],
        "retained benchmark receipt bytes changed",
    )

    direct_fields = (
        "status",
        "project_id",
        "reference_id",
        "reference_sha256",
        "candidate_id",
        "artifact_id",
        "artifact_sha256",
        "program_sha256",
        "catalog_sha256",
        "render_set_hash",
        "comparison_report_hash",
        "view_spec_sha256",
        "silhouette_target_sha256",
        "silhouette_rig_sha256",
        "silhouette_camera_hash",
        "geometry_route",
        "geometry_variant",
        "material_variant",
        "part_count",
        "triangle_count",
        "validator_status",
        "quality_visual_status",
        "visual_review_status",
        "human_review",
        "pbr_material_pack",
        "hq_360",
        "render_pass_image_blocks",
        "persistent_user_data_touched",
    )
    for field in direct_fields:
        require(
            retained.get(field) == retained_truth.get(field),
            f"retained benchmark field drifted: {field}",
        )
    require(retained.get("build_cohorts") == retained_truth.get("build_cohorts"), "retained cohort drifted")
    require(retained.get("comparison_metrics") == retained_truth.get("metrics"), "retained metrics drifted")
    require(retained.get("render_pass_order") == retained_truth.get("aov_order"), "retained AOV order drifted")
    require(retained.get("render_pass_calls") == len(retained_truth.get("aov_order", [])), "retained AOV count drifted")
    require(retained.get("visual_intake") == retained_truth.get("visual_intake"), "visual intake drifted")

    candidate_call = single_tool_call(retained, "candidate_get")
    readback_call = single_tool_call(retained, "artifact_readback_get")
    compare_call = single_tool_call(retained, "reference_compare_prepare")
    fit_call = single_tool_call(retained, "camera_fit_prepare")
    camera_truth = retained_truth["camera_binding"]
    require(candidate_call.get("canonical_sha256") == retained_truth["candidate_canonical_sha256"], "candidate canonical hash drifted")
    require(readback_call.get("canonical_sha256") == retained_truth["artifact_readback_canonical_sha256"], "readback canonical hash drifted")
    require(fit_call.get("selected_camera", {}).get("camera_hash") == camera_truth["fit_camera_hash"], "fit camera hash drifted")
    require(
        fit_call.get("selected_camera", {}).get("canonical_sha256") == camera_truth["fit_camera_canonical_sha256"],
        "fit camera canonical hash drifted",
    )
    require(compare_call.get("camera", {}).get("camera_hash") == camera_truth["comparison_camera_hash"], "comparison camera hash drifted")
    require(
        compare_call.get("camera", {}).get("canonical_sha256") == camera_truth["comparison_camera_canonical_sha256"],
        "comparison camera canonical hash drifted",
    )
    require(camera_truth["binding_status"] == "MISMATCH", "known camera mismatch must not be hidden")
    require(camera_truth["fit_camera_hash"] != camera_truth["comparison_camera_hash"], "camera mismatch status contradicts hashes")

    completeness = retained_truth["receipt_completeness"]
    require(completeness["status"] == truth["evidence_status"] == "INCOMPLETE_TRUTH_BINDING", "incomplete benchmark status drifted")
    require(completeness["camera_binding"] == "MISMATCH", "receipt completeness hides camera mismatch")
    require(
        all(value in {"MISSING", "MISSING_FROM_PRIMARY_RECEIPT", "MISMATCH"} for key, value in completeness.items() if key != "status"),
        "receipt completeness contains an unsupported passing claim",
    )
    require(
        retained_truth["benchmark_eligibility"] == "BLOCKED_INCOMPLETE_BINDING",
        "incomplete observation was promoted to a benchmark",
    )
    require(
        retained_truth["semantic_claim"] == "PROVISIONAL_RETAINED_OBSERVATION_NOT_PROVEN_GLOBAL_BEST",
        "provisional observation semantics drifted",
    )
    selection = retained_truth["selection_policy"]
    require(selection["selection_status"] == "INCOMPLETE_ELIGIBILITY_AND_METRIC_REVISION", "selection gap was hidden")
    require(selection["claim"] == retained_truth["semantic_claim"], "selection claim contradicts observation semantics")
    require(selection["chosen_path"] == retained_truth["source_receipt_path"], "selection path drifted")
    require(
        selection["known_comparison_ledger"][0]["path"] == retained_truth["source_receipt_path"],
        "selection ledger does not start from the provisional observation",
    )
    require(
        all(row["benchmark_eligible"] is False for row in selection["known_comparison_ledger"]),
        "selection ledger falsely marks an incomplete run benchmark-eligible",
    )
    expected_selection_reasons = {
        retained_truth["source_receipt_path"]: "BLOCKED_CAMERA_MISMATCH_AND_INCOMPLETE_RECEIPT_BINDINGS",
        "docs/evidence/mcp010f/part-correction-source-20260813.json": "SOURCE_PROBE_NOT_COMPLETE_REAL_CODEX_AND_BUILD_COHORT_NULL",
        "docs/evidence/mcp010f/real-codex-cli-semantic-landmark-compare-20260813.json": "METRIC_SEMANTICS_CHANGED_AND_QUALITY_TARGET_NOT_MET",
        "docs/evidence/mcp010f/real-codex-cli-semantic-aligned-fast-20260813.json": "BLOCKED_NO_QUALITY_RESULT",
    }
    require(
        {row["path"]: row["reason"] for row in selection["known_comparison_ledger"]}
        == expected_selection_reasons,
        "known comparison eligibility ledger drifted",
    )
    for row in selection["known_comparison_ledger"]:
        source_path = ROOT / row["path"]
        require(source_path.is_file(), f"selection ledger receipt is missing: {row['path']}")
        require(row["sha256"] == sha256_file(source_path), f"selection ledger receipt bytes changed: {row['path']}")

    results = metric_gate_results(retained["comparison_metrics"], retained_truth["thresholds"])
    require(results == retained_truth["metric_gate_results"], "stored metric gate results are stale")
    require(any(result == "FAIL" for result in results.values()), "retained candidate unexpectedly passes every metric")
    require(
        retained_truth["strict_visible_view_policy_implemented"] == "PASS",
        "policy implementation and candidate result must remain separate",
    )
    require(
        retained_truth["current_candidate_visible_view_gate"] == "FAIL_QUALITY_TARGET_NOT_MET",
        "retained candidate must remain a visible-view quality failure",
    )
    require(retained.get("quality_hard_gate_passed") is False, "failed visual candidate cannot have a passing hard gate")

    assertions = compute_assertion_ledger(truth, retained)
    require(assertions == truth["assertion_ledger"], "Stage 0 assertion ledger drifted")


def check_auxiliary_runs(truth: dict[str, Any]) -> None:
    auxiliary = truth["auxiliary_runs"]
    for name in ("surface_linework", "armor_shell_zones"):
        item = auxiliary[name]
        curated_path = ROOT / item["curated_path"]
        raw_path = ROOT / item["raw_path"]
        require(sha256_file(curated_path) == item["curated_sha256"], f"{name} curated receipt bytes changed")
        require(sha256_file(raw_path) == item["raw_sha256"], f"{name} raw receipt bytes changed")
    require(auxiliary["surface_linework"]["curated_raw_binding"] == "FAIL_HASHES_DIFFER", "surface raw mismatch is hidden")
    require(auxiliary["armor_shell_zones"]["curated_raw_binding"] == "PASS", "armor raw binding status drifted")
    require(auxiliary["armor_shell_zones"]["predecessor_geometry_binding"] == "MISSING", "armor predecessor gap is hidden")


def check_run_inventory(truth: dict[str, Any]) -> None:
    inventory = load_json(RUN_INVENTORY_PATH)
    require_exact_keys(
        inventory,
        frozenset(
            "canonical_sha256 latest_attempt_path latest_completed_transport_path ordering_basis recorded_on runs "
            "schema_version scope task_id".split()
        ),
        "real Codex run inventory",
    )
    require(inventory.get("schema_version") == "ForgeCADRealCodexRunInventory@1", "unexpected real Codex inventory schema")
    require(inventory["task_id"] == "FGC-MCP010F", "real Codex inventory task drifted")
    require(inventory["recorded_on"] == truth["recorded_on"], "real Codex inventory date drifted")
    require(
        inventory["scope"] == "all docs/evidence/mcp010f/real-codex-cli-*.json present at Stage 0 freeze",
        "real Codex inventory scope drifted",
    )
    require(
        inventory.get("ordering_basis") == "ONE_TIME_FILESYSTEM_MTIME_SNAPSHOT_EXISTING_RECEIPTS_LACK_RECORDED_AT",
        "legacy run ordering limitation was hidden",
    )
    require(inventory.get("canonical_sha256") == canonical_sha256(inventory), "real Codex inventory canonical hash mismatch")
    inventory_truth = truth["real_codex_run_inventory"]
    require(
        inventory_truth["ordering_confidence"] == "SNAPSHOT_ONLY_LEGACY_RECEIPTS_LACK_RECORDED_AT",
        "truth hides the legacy chronology limitation",
    )
    require(inventory_truth["sha256"] == sha256_file(RUN_INVENTORY_PATH), "real Codex inventory bytes changed")
    runs = inventory.get("runs")
    require(isinstance(runs, list) and runs, "real Codex inventory has no runs")
    require(inventory_truth["run_count"] == len(runs), "real Codex inventory count drifted")
    require([row.get("sequence") for row in runs] == list(range(1, len(runs) + 1)), "real Codex inventory sequence is not contiguous")
    inventory_paths = [row.get("path") for row in runs]
    require(len(inventory_paths) == len(set(inventory_paths)), "real Codex inventory contains duplicate paths")
    actual_paths = sorted(
        str(path.relative_to(ROOT))
        for path in (ROOT / "docs/evidence/mcp010f").glob("real-codex-cli-*.json")
    )
    require(sorted(inventory_paths) == actual_paths, "real Codex inventory does not cover every current receipt")
    for row in runs:
        require_exact_keys(row, frozenset("completed_transport path sequence sha256 status".split()), f"run inventory row {row.get('sequence')}")
        path = ROOT / row["path"]
        receipt = load_json(path)
        require(row.get("sha256") == sha256_file(path), f"real Codex receipt bytes changed: {row['path']}")
        require(row.get("status") == receipt.get("status"), f"real Codex receipt status drifted: {row['path']}")
        require(
            row.get("completed_transport") == (receipt.get("status") == "PASS_WITH_QUALITY_TARGET_NOT_MET"),
            f"real Codex completed-transport classification drifted: {row['path']}",
        )
    latest_attempt = max(runs, key=lambda row: row["sequence"])
    completed = [row for row in runs if row["completed_transport"]]
    require(completed, "real Codex inventory has no completed transport")
    latest_completed = max(completed, key=lambda row: row["sequence"])
    require(inventory["latest_attempt_path"] == latest_attempt["path"], "latest-attempt pointer is stale")
    require(inventory["latest_completed_transport_path"] == latest_completed["path"], "latest-completed pointer is stale")

    attempt_truth = truth["latest_attempt"]
    require_exact_keys(attempt_truth["build_cohorts"], frozenset("mcp runtime worker".split()), "latest_attempt.build_cohorts")
    attempt_path = ROOT / attempt_truth["source_receipt_path"]
    attempt = load_json(attempt_path)
    require(attempt_truth["source_receipt_path"] == latest_attempt["path"], "truth latest attempt is stale")
    require(attempt_truth["source_receipt_sha256"] == sha256_file(attempt_path), "latest attempt receipt bytes changed")
    for field in ("status", "reason"):
        require(attempt_truth[field] == attempt.get(field), f"latest attempt field drifted: {field}")
    require(attempt_truth["build_cohorts"] == attempt.get("build_cohorts"), "latest attempt cohort drifted")
    require(
        attempt_truth["cohort_provenance"] == "UNVERIFIED_SENTINEL_LIKE_DECLARED_VALUE",
        "latest attempt sentinel-like cohort provenance was hidden",
    )
    require(
        attempt_truth["classification"]
        == "DECLARED_REAL_CODEX_BLOCKED_DIAGNOSTIC_WITH_UNVERIFIED_HOST_AND_COHORT_PROVENANCE",
        "latest attempt diagnostic classification drifted",
    )
    require(
        attempt_truth["host_provenance"]
        == "UNVERIFIED_COMPACT_RECEIPT_LACKS_RAW_EVENTS_EXIT_CODES_AND_TRANSCRIPT_HASH",
        "latest attempt host provenance was falsely promoted",
    )
    require(
        attempt_truth["attempt_count_evidence"]
        == "UNVERIFIED_DECLARED_REASON_ONLY_NO_RAW_TRANSCRIPT_OR_TURN_COUNT",
        "latest attempt count was falsely promoted",
    )
    require(
        len(set(attempt_truth["build_cohorts"].values())) == 1
        and next(iter(attempt_truth["build_cohorts"].values())) == "b" * 64,
        "latest attempt no longer matches the explicitly unverified sentinel-like cohort",
    )
    require(attempt_truth["quality_result"] == "NOT_PRODUCED", "blocked latest attempt cannot claim a quality result")
    require(attempt.get("comparison_metrics") is None, "blocked latest attempt unexpectedly contains comparison metrics")

    transport_truth = truth["latest_completed_transport"]
    require_exact_keys(
        transport_truth["build_cohorts"],
        frozenset("mcp runtime worker".split()),
        "latest_completed_transport.build_cohorts",
    )
    require_exact_keys(transport_truth["metrics"], frozenset(METRIC_CRITERIA), "latest_completed_transport.metrics")
    transport_path = ROOT / transport_truth["source_receipt_path"]
    transport = load_json(transport_path)
    require(transport_truth["source_receipt_path"] == latest_completed["path"], "truth latest completed transport is stale")
    require(sha256_file(transport_path) == transport_truth["source_receipt_sha256"], "latest completed transport receipt bytes changed")
    for field in ("status", "candidate_id", "artifact_sha256", "quality_visual_status"):
        require(transport.get(field) == transport_truth.get(field), f"latest completed transport field drifted: {field}")
    require(transport.get("build_cohorts") == transport_truth.get("build_cohorts"), "latest completed transport cohort drifted")
    require(transport.get("comparison_metrics") == transport_truth.get("metrics"), "latest completed transport metrics drifted")
    require(transport.get("render_set_hash") == transport_truth.get("render_set_hash"), "latest completed render set drifted")
    require(transport.get("comparison_report_hash") == transport_truth.get("comparison_report_hash"), "latest completed comparison drifted")
    require(
        transport_truth["promotion_decision"] == "NOT_PROMOTED_METRIC_SEMANTICS_CHANGED_AND_QUALITY_TARGET_NOT_MET",
        "latest completed transport must not silently replace a differently measured retained benchmark",
    )
    require(
        transport_truth["metric_semantics"] == "SEMANTIC_PART_ANCHOR_CHECKPOINT_NOT_COMPARABLE_TO_ATTEMPT35_LANDMARK_METRICS",
        "latest completed metric-semantics boundary drifted",
    )


def check_packaged_viewer(truth: dict[str, Any]) -> None:
    viewer_truth = truth["packaged_viewer"]
    viewer_path = ROOT / viewer_truth["source_receipt_path"]
    viewer = load_json(viewer_path)
    require(sha256_file(viewer_path) == viewer_truth["source_receipt_sha256"], "packaged Viewer receipt bytes changed")
    packaged = viewer.get("packaged_viewer", {})
    compare = viewer.get("reference_compare", {})
    require(packaged.get("build_cohort_sha256") == viewer_truth["build_cohort_sha256"], "packaged Viewer cohort drifted")
    require(viewer.get("appearance_artifact_sha256") == viewer_truth["artifact_sha256"], "packaged Viewer artifact drifted")
    require(compare.get("render_set_hash") == viewer_truth["render_set_hash"], "packaged Viewer render set drifted")
    require(compare.get("quality_visual_status") == viewer_truth["quality_visual_status"], "packaged Viewer quality status drifted")
    require(packaged.get("ui_e2e") == viewer_truth["ui_e2e"], "packaged Viewer UI gate drifted")
    retained = truth["provisional_retained_observation"]
    require(
        viewer_truth["provisional_observation_binding"] == "NOT_RUN_DIFFERENT_COHORT_AND_ARTIFACT",
        "packaged Viewer binding must remain explicit until a same-benchmark replay exists",
    )
    require(viewer_truth["build_cohort_sha256"] != retained["build_cohorts"]["mcp"], "packaged Viewer unexpectedly claims the retained cohort")
    require(viewer_truth["artifact_sha256"] != retained["artifact_sha256"], "packaged Viewer unexpectedly claims the retained artifact")


def check_authority_docs(truth: dict[str, Any]) -> None:
    pointer = "docs/evidence/mcp010f/current-benchmark-truth.json"
    require(tuple(truth["authority_docs"]) == AUTHORITY_DOCS, "authority document set drifted")
    observation = truth["provisional_retained_observation"]
    marker = (
        "<!-- forgecad-stage0: "
        f"schemas={truth['current_source']['contracts']['schema_count']} "
        f"schema_set_sha256={truth['current_source']['contracts']['schema_content_set_sha256']} "
        f"read_tools={truth['current_source']['mcp_tools']['read_count']} "
        f"write_tools={truth['current_source']['mcp_tools']['write_count']} "
        f"total_tools={truth['current_source']['mcp_tools']['total_count']} "
        f"task={truth['task_id']} observation={observation['quality_visual_status']} "
        f"eligibility={observation['benchmark_eligibility']} evidence={truth['evidence_status']} "
        f"camera={observation['camera_binding']['binding_status']} "
        f"packaged={truth['packaged_viewer']['provisional_observation_binding']} "
        f"latest_attempt={Path(truth['latest_attempt']['source_receipt_path']).name} "
        f"latest_completed={Path(truth['latest_completed_transport']['source_receipt_path']).name} -->"
    )
    for relative in AUTHORITY_DOCS:
        path = ROOT / relative
        require(path.is_file(), f"missing authority doc: {relative}")
        source = path.read_text(encoding="utf-8")
        require(pointer in source, f"{relative} does not point to the Stage 0 truth")
        require(marker in source, f"{relative} is missing the exact Stage 0 status marker")

    stale_current_claims = {
        "docs/DOCUMENTATION_STATUS.md": (
            "当前源码合同/工具面的最新数量为 77 Schema、28 read + 18 write",
            "当前 77 Schema",
            "当前总源合同 77",
        ),
        "docs/CODEX_HANDOFF.md": (
            "当前源码共 78 contracts、28 read + 18 opt-in write = 46 tools",
            "当前源码 78 contracts、28 read + 18",
        ),
        "docs/AUTHORITATIVE_STATE.md": ("当前共 77 个 JSON Schema",),
        "docs/MVP_DELIVERY_PLAN.md": ("总源合同 77", "默认工具面为 28 read + 18"),
        "docs/MCP_RUNTIME_CONTRACT.md": (
            "当前 `forgecad-mcp` 源码的默认 stdio tool manifest 包含 28 个只读工具",
            "当前 source manifest 为 28 read + 18",
        ),
        "docs/evidence/CAPABILITY_GATE_MATRIX.md": (
            "当前源码默认 28 个只读 tools",
            "当前 source tools 28 read + 18",
        ),
        "docs/WORKBENCH_VIEWER.md": ("MCP010F compare/selection/explosion/a11y 为 planned/unavailable",),
    }
    for relative, claims in stale_current_claims.items():
        source = (ROOT / relative).read_text(encoding="utf-8")
        leaked = [claim for claim in claims if claim in source]
        require(not leaked, f"{relative} retains stale current-state claims: {leaked}")


def check_truth_negative_semantics(truth: dict[str, Any]) -> None:
    retained = truth["provisional_retained_observation"]
    assertions = truth["assertion_ledger"]
    require(truth["evidence_status"] == "INCOMPLETE_TRUTH_BINDING", "benchmark incompleteness was hidden")
    require(assertions["BT006_CAMERA_BINDING"] == "FAIL", "known camera-binding failure was hidden")
    require(assertions["BT009_AOV_HASH_COMPLETENESS"] == "MISSING", "missing per-AOV evidence was hidden")
    require(assertions["BT011_THRESHOLD_EXACT_SET_IN_RECEIPT"] == "MISSING", "missing threshold receipt was hidden")
    require(assertions["BT014_UNRUN_EXPLICITNESS"] == "MISSING", "missing explicit downstream status was hidden")
    require(assertions["BT016_SURFACE_RAW_PAIR"] == "FAIL", "surface curated/raw mismatch was hidden")
    require(assertions["BT019_BENCHMARK_ELIGIBILITY"] == "MISSING", "benchmark eligibility gap was hidden")
    require(assertions["BT020_LEGACY_RECEIPT_RECORDED_AT"] == "MISSING", "legacy timestamp gap was hidden")
    require(retained["benchmark_eligibility"] == "BLOCKED_INCOMPLETE_BINDING", "observation was promoted to benchmark status")
    require(retained["current_candidate_visible_view_gate"] == "FAIL_QUALITY_TARGET_NOT_MET", "failed visual gate was promoted")
    require(retained["human_review"] == "NOT_RUN", "human review was falsely promoted")
    require(retained["pbr_material_pack"] == "NOT_RUN", "PBR was falsely promoted")
    require(retained["export_restart_hash"] == "NOT_RUN", "export/restart was falsely promoted")
    require(retained["hq_360"] == "BLOCKED_REFERENCE_COVERAGE", "360 gate was falsely promoted")
    require(retained["persistent_user_data_touched"] is False, "Stage 0 must not claim a persistent user-data write")
    require(
        truth["packaged_viewer"]["provisional_observation_binding"] == "NOT_RUN_DIFFERENT_COHORT_AND_ARTIFACT",
        "packaged Viewer was falsely bound to the retained benchmark",
    )


def check_truth_shape(truth: dict[str, Any]) -> None:
    require_exact_keys(truth, TRUTH_TOP_LEVEL_KEYS, "Stage 0 truth")
    require_exact_keys(truth["assertion_ledger"], ASSERTION_KEYS, "Stage 0 assertion ledger")
    require_exact_keys(truth["current_source"], frozenset("contracts mcp_tools task_chain visible_view_policy".split()), "current_source")
    require_exact_keys(
        truth["current_source"]["contracts"],
        frozenset("manifest_path manifest_sha256 schema_content_set_algorithm schema_content_set_sha256 schema_count".split()),
        "current_source.contracts",
    )
    require_exact_keys(
        truth["current_source"]["mcp_tools"],
        frozenset(
            "read_count read_manifest_sha256 read_names source_path source_sha256 summary_receipt_path "
            "summary_receipt_sha256 total_count write_count write_enabled_manifest_sha256 write_names".split()
        ),
        "current_source.mcp_tools",
    )
    require_exact_keys(truth["current_source"]["task_chain"], frozenset("dependency only_in_progress".split()), "current_source.task_chain")
    require_exact_keys(
        truth["current_source"]["visible_view_policy"],
        frozenset(
            "authority fit_plan_projection_path fit_plan_projection_sha256 runtime_source_path runtime_source_sha256 "
            "viewer_projection_path viewer_projection_sha256".split()
        ),
        "current_source.visible_view_policy",
    )
    require_exact_keys(truth["evidence_manifest"], frozenset("path sha256".split()), "evidence_manifest")
    require_exact_keys(
        truth["latest_attempt"],
        frozenset(
            "attempt_count_evidence build_cohorts classification cohort_provenance host_provenance quality_result "
            "reason source_receipt_path "
            "source_receipt_sha256 status".split()
        ),
        "latest_attempt",
    )
    require_exact_keys(
        truth["latest_completed_transport"],
        frozenset(
            "artifact_sha256 build_cohorts candidate_id comparison_report_hash metric_semantics metrics "
            "promotion_decision quality_visual_status render_set_hash source_receipt_path source_receipt_sha256 status".split()
        ),
        "latest_completed_transport",
    )
    require_exact_keys(
        truth["packaged_viewer"],
        frozenset(
            "artifact_sha256 build_cohort_sha256 provisional_observation_binding quality_visual_status render_set_hash "
            "source_receipt_path source_receipt_sha256 ui_e2e".split()
        ),
        "packaged_viewer",
    )
    require_exact_keys(truth["phase_zero"], frozenset("completed remaining status".split()), "phase_zero")
    require_exact_keys(
        truth["real_codex_run_inventory"],
        frozenset("ordering_confidence path run_count sha256".split()),
        "real_codex_run_inventory",
    )
    observation = truth["provisional_retained_observation"]
    require_exact_keys(observation, OBSERVATION_KEYS, "provisional_retained_observation")
    for label in ("build_cohorts",):
        require_exact_keys(observation[label], frozenset("mcp runtime worker".split()), f"provisional_retained_observation.{label}")
    require_exact_keys(
        observation["camera_binding"],
        frozenset(
            "binding_status comparison_camera_canonical_sha256 comparison_camera_hash "
            "fit_camera_canonical_sha256 fit_camera_hash".split()
        ),
        "provisional_retained_observation.camera_binding",
    )
    metric_keys = frozenset(METRIC_CRITERIA)
    require_exact_keys(observation["metrics"], metric_keys, "provisional_retained_observation.metrics")
    require_exact_keys(observation["metric_gate_results"], metric_keys, "provisional_retained_observation.metric_gate_results")
    require_exact_keys(
        observation["thresholds"],
        frozenset(threshold_name for _, threshold_name in METRIC_CRITERIA.values()),
        "provisional_retained_observation.thresholds",
    )
    require_exact_keys(
        observation["receipt_completeness"],
        frozenset(
            "artifact_readback_integrity_counters camera_binding candidate_confirm candidate_state "
            "comparison_canonical_vs_object_hashes export mask_sha256_and_revision metric_revision "
            "per_aov_hashes_and_dimensions render_canonical_vs_object_hashes restart_hash status structured_thresholds "
            "threshold_revision visual_review_receipt_hash".split()
        ),
        "provisional_retained_observation.receipt_completeness",
    )
    selection = observation["selection_policy"]
    require_exact_keys(
        selection,
        frozenset(
            "chosen_path claim comparator_priority_after_eligibility known_comparison_ledger policy_id "
            "required_future_fields selection_status tie_breaker".split()
        ),
        "provisional_retained_observation.selection_policy",
    )
    require(isinstance(selection["known_comparison_ledger"], list), "known comparison ledger must be a list")
    for index, row in enumerate(selection["known_comparison_ledger"]):
        require_exact_keys(row, frozenset("benchmark_eligible path reason sha256".split()), f"known_comparison_ledger[{index}]")
    require_exact_keys(observation["visual_intake"], frozenset("landmark_count region_count source_sha256 status".split()), "visual_intake")
    require_exact_keys(truth["auxiliary_runs"], frozenset("armor_shell_zones surface_linework".split()), "auxiliary_runs")
    require_exact_keys(
        truth["auxiliary_runs"]["surface_linework"],
        frozenset("curated_path curated_raw_binding curated_sha256 raw_path raw_sha256 relation_to_primary".split()),
        "auxiliary_runs.surface_linework",
    )
    require_exact_keys(
        truth["auxiliary_runs"]["armor_shell_zones"],
        frozenset(
            "curated_path curated_raw_binding curated_sha256 predecessor_geometry_binding raw_path raw_sha256 "
            "relation_to_primary".split()
        ),
        "auxiliary_runs.armor_shell_zones",
    )


def check_truth_declared_semantics(truth: dict[str, Any]) -> None:
    require(truth["observation_id"] == "robot-three-quarter-visible-view-attempt35-provisional", "observation id drifted")
    require(truth["recorded_on"] == "2026-08-13", "Stage 0 recorded date drifted")
    require(
        truth["purpose"]
        == "Stage 0 machine-readable source and provisional-observation snapshot; evidence index only, never Runtime product truth or an eligible benchmark",
        "Stage 0 purpose was promoted or drifted",
    )
    require(truth["evidence_status"] == "INCOMPLETE_TRUTH_BINDING", "Stage 0 evidence status drifted")

    current = truth["current_source"]
    require(current["contracts"]["manifest_path"] == "packages/forgecad-contracts/manifest.json", "contract manifest path drifted")
    require(current["mcp_tools"]["source_path"] == "apps/desktop/src-tauri/crates/forgecad-mcp/src/main.rs", "MCP source path drifted")
    require(
        current["mcp_tools"]["summary_receipt_path"] == "docs/evidence/mcp010f/source-tool-manifest-summary.json",
        "MCP tool summary path drifted",
    )
    expected_policy_paths = {
        "runtime_source_path": "apps/desktop/src-tauri/crates/forgecad-runtime/src/lib.rs",
        "viewer_projection_path": "apps/desktop/src/features/runtime-viewer/RuntimeViewer.tsx",
        "fit_plan_projection_path": "scripts/build_mcp010f_fit_plan.py",
    }
    for key, expected in expected_policy_paths.items():
        require(current["visible_view_policy"][key] == expected, f"visible-view policy path drifted: {key}")

    phase = truth["phase_zero"]
    expected_completed = [
        "machine-readable current source counts and tool names",
        "one provisional retained observation pointer with frozen source hashes and benchmark eligibility explicitly blocked",
        "separate newest-transport and packaged-Viewer facts",
        "automatic source drift, contract-content, cross-run isolation and candidate-gate semantic checks",
    ]
    expected_remaining = [
        "regenerate one current-cohort compact receipt with one camera binding, canonical-vs-object hashes, per-AOV hashes, readback counters, structured threshold revision, metric revision and explicit NOT_RUN fields",
        "bind the packaged Viewer to that exact candidate, artifact, RenderSet and comparison hashes",
        "prove the real Codex host consumed returned image blocks rather than only calling render_pass_get",
        "run formal VoiceOver, independent human review, PBR likeness and export/restart hash gates",
    ]
    require(phase == {"completed": expected_completed, "remaining": expected_remaining, "status": "IN_PROGRESS"}, "Stage 0 phase ledger drifted")

    require(
        truth["latest_attempt"]["source_receipt_path"]
        == "docs/evidence/mcp010f/real-codex-cli-semantic-aligned-fast-20260813.json",
        "frozen latest-attempt path drifted",
    )
    require(
        truth["latest_completed_transport"]["source_receipt_path"]
        == "docs/evidence/mcp010f/real-codex-cli-semantic-landmark-compare-20260813.json",
        "frozen latest-completed path drifted",
    )
    require(
        truth["packaged_viewer"]["source_receipt_path"] == "docs/evidence/mcp010f/packaged-viewer-read-model.json",
        "packaged Viewer receipt path drifted",
    )
    require(truth["packaged_viewer"]["ui_e2e"] == "NOT_RUN", "packaged Viewer UI was falsely promoted")
    require(
        truth["real_codex_run_inventory"]["path"] == "docs/evidence/mcp010f/real-codex-run-inventory.json",
        "real Codex inventory path drifted",
    )

    observation = truth["provisional_retained_observation"]
    expected_observation_semantics = {
        "benchmark_eligibility": "BLOCKED_INCOMPLETE_BINDING",
        "comparison_hash_kind": "UNSPECIFIED_CANONICAL_OR_CAS_OBJECT",
        "confirmation_eligibility": "BLOCKED_QUALITY_TARGET_NOT_MET",
        "current_candidate_visible_view_gate": "FAIL_QUALITY_TARGET_NOT_MET",
        "export_restart_hash": "NOT_RUN",
        "hq_360": "BLOCKED_REFERENCE_COVERAGE",
        "human_review": "NOT_RUN",
        "pbr_material_pack": "NOT_RUN",
        "quality_visual_status": "QUALITY_TARGET_NOT_MET",
        "render_hash_kind": "UNSPECIFIED_CANONICAL_OR_CAS_OBJECT",
        "render_pass_image_blocks": "NOT_OBSERVED_IN_SANITIZED_CLI_EVENTS",
        "semantic_claim": "PROVISIONAL_RETAINED_OBSERVATION_NOT_PROVEN_GLOBAL_BEST",
        "status": "PASS_WITH_QUALITY_TARGET_NOT_MET",
        "strict_visible_view_policy_implemented": "PASS",
        "threshold_binding": "CURRENT_RUNTIME_SOURCE_POLICY_NOT_EMBEDDED_IN_ATTEMPT35_RECEIPT",
        "visual_review_status": "needs_revision",
    }
    for key, expected in expected_observation_semantics.items():
        require(observation[key] == expected, f"provisional observation semantic field drifted: {key}")
    require(observation["persistent_user_data_touched"] is False, "provisional observation claims a persistent write")
    require(
        observation["source_receipt_path"]
        == "docs/evidence/mcp010f/real-codex-cli-silhouette-first-20260813-attempt35-detail-camera-ref.json",
        "provisional observation receipt path drifted",
    )
    require(
        observation["aov_order"]
        == ["beauty", "silhouette", "depth", "normal", "ao", "part-id", "material-id", "wireframe", "uv-stretch"],
        "provisional observation AOV order drifted",
    )
    expected_completeness = {
        "artifact_readback_integrity_counters": "MISSING",
        "camera_binding": "MISMATCH",
        "candidate_confirm": "MISSING_FROM_PRIMARY_RECEIPT",
        "candidate_state": "MISSING",
        "comparison_canonical_vs_object_hashes": "MISSING",
        "export": "MISSING_FROM_PRIMARY_RECEIPT",
        "mask_sha256_and_revision": "MISSING",
        "metric_revision": "MISSING",
        "per_aov_hashes_and_dimensions": "MISSING",
        "render_canonical_vs_object_hashes": "MISSING",
        "restart_hash": "MISSING_FROM_PRIMARY_RECEIPT",
        "status": "INCOMPLETE_TRUTH_BINDING",
        "structured_thresholds": "MISSING",
        "threshold_revision": "MISSING",
        "visual_review_receipt_hash": "MISSING",
    }
    require(observation["receipt_completeness"] == expected_completeness, "receipt completeness semantics drifted")

    selection = observation["selection_policy"]
    require(selection["policy_id"] == "MCP010F_PROVISIONAL_OBSERVATION_SELECTION@1", "selection policy id drifted")
    require(selection["selection_status"] == "INCOMPLETE_ELIGIBILITY_AND_METRIC_REVISION", "selection status drifted")
    require(selection["claim"] == observation["semantic_claim"], "selection claim drifted")
    require(selection["tie_breaker"] == "latest recorded_at only after identical metric revision and complete eligibility", "selection tie breaker drifted")
    require(
        selection["comparator_priority_after_eligibility"]
        == [
            "boundary_f1_4px:max", "silhouette_iou:max", "bbox_edge_error:min", "centroid_error:min",
            "landmark_coverage:max", "landmark_nme:min", "region_median_iou:max", "critical_region_min_iou:max",
        ],
        "selection comparator priority drifted",
    )
    require(
        selection["required_future_fields"]
        == [
            "recorded_at", "metric_revision", "threshold_revision", "single_camera_binding",
            "per_aov_hashes_and_dimensions", "canonical_vs_object_hashes", "artifact_readback_integrity_counters",
            "explicit_downstream_not_run_fields",
        ],
        "selection required-future-fields ledger drifted",
    )

    expected_auxiliary_paths = {
        "surface_linework": (
            "docs/evidence/mcp010f/surface-linework-real-reference.json",
            "docs/evidence/mcp010f/surface-linework-real-reference-raw.json",
        ),
        "armor_shell_zones": (
            "docs/evidence/mcp010f/armor-shell-zones-real-reference.json",
            "docs/evidence/mcp010f/armor-shell-zones-real-reference-raw.json",
        ),
    }
    for name, (curated_path, raw_path) in expected_auxiliary_paths.items():
        require(truth["auxiliary_runs"][name]["curated_path"] == curated_path, f"{name} curated path drifted")
        require(truth["auxiliary_runs"][name]["raw_path"] == raw_path, f"{name} raw path drifted")


def check_evidence_manifest(truth: dict[str, Any]) -> None:
    pointer = truth["evidence_manifest"]
    require(pointer["path"] == "docs/evidence/mcp010f/manifest.json", "evidence manifest path drifted")
    require(pointer["sha256"] == sha256_file(EVIDENCE_MANIFEST_PATH), "evidence manifest bytes changed")
    require(
        pointer["sha256"] == EXPECTED_EVIDENCE_MANIFEST_SHA256,
        "frozen Stage 0 evidence manifest changed without an explicit checker revision",
    )
    manifest = load_json(EVIDENCE_MANIFEST_PATH)
    require_exact_keys(
        manifest,
        frozenset("evidence gates limitations persistent_user_data_touched recorded_on schema_version scope status task_id".split()),
        "MCP010F evidence manifest",
    )
    require(manifest["task_id"] == truth["task_id"], "evidence manifest task drifted")
    require(manifest["schema_version"] == "ForgeCADEvidenceManifest@1", "evidence manifest schema drifted")
    require(manifest["recorded_on"] == truth["recorded_on"], "evidence manifest date drifted")
    require(
        manifest["status"] == "stage0-truth-and-source-and-packaged-read-model-structural-with-visual-quality-not-met",
        "evidence manifest status drifted or was promoted",
    )
    require(manifest["persistent_user_data_touched"] is False, "evidence manifest claims a persistent user-data write")
    gates = manifest["gates"]
    require_exact_keys(gates, EVIDENCE_MANIFEST_GATE_KEYS, "MCP010F evidence manifest gates")
    require(gates == EXPECTED_EVIDENCE_MANIFEST_GATES, "MCP010F evidence manifest gate values drifted")
    observation = truth["provisional_retained_observation"]
    expected_projection = {
        "provisional_observation_truth_binding": truth["evidence_status"],
        "provisional_observation_benchmark_eligibility": observation["benchmark_eligibility"],
        "provisional_observation_camera_binding": "MISMATCH_FIT_VS_COMPARISON_CAMERA",
        "provisional_observation_visible_view_gate": observation["current_candidate_visible_view_gate"],
        "packaged_viewer_provisional_observation_binding": truth["packaged_viewer"]["provisional_observation_binding"],
        "latest_completed_transport": "PASS_WITH_QUALITY_TARGET_NOT_MET_METRIC_SEMANTICS_CHANGED_NOT_PROMOTED",
        "latest_attempt": "BLOCKED_WITHOUT_QUALITY_RESULT",
        "real_codex_image_block_observation": observation["render_pass_image_blocks"],
        "viewer_accessibility_e2e": "NOT_RUN",
        "human_visual_review": observation["human_review"],
        "export_restart_hash": observation["export_restart_hash"],
        "full_360_reference": observation["hq_360"],
    }
    for key, expected in expected_projection.items():
        require(gates[key] == expected, f"evidence manifest projection drifted: {key}")
    limitation_text = "\n".join(manifest["limitations"])
    for forbidden in ("Attempt35 remains the retained metrics baseline", "retained candidate passed visual quality"):
        require(forbidden not in limitation_text, f"evidence manifest contains a forbidden promotion claim: {forbidden}")
    require(isinstance(manifest["scope"], list) and manifest["scope"], "evidence manifest scope must be a non-empty list")
    require(isinstance(manifest["limitations"], list) and manifest["limitations"], "evidence manifest limitations must be non-empty")
    require(isinstance(manifest["evidence"], list) and len(manifest["evidence"]) == 120, "evidence manifest frozen evidence count drifted")
    require(len(set(manifest["evidence"])) == len(manifest["evidence"]), "evidence manifest contains duplicate entries")
    for index, entry in enumerate(manifest["evidence"]):
        require(isinstance(entry, str) and entry, f"evidence entry {index} must be a non-empty string")
        symbol: str | None = None
        if "::" in entry:
            path_text, symbol = entry.split("::", 1)
            require(bool(symbol) and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", symbol) is not None, f"invalid evidence symbol: {entry}")
        else:
            arguments = shlex.split(entry)
            require(arguments, f"empty evidence command entry: {entry}")
            path_text = arguments[0]
            if len(arguments) > 1:
                require(
                    entry == "scripts/probe_mcp010e_raw_stdio.py --receipt-task-id FGC-MCP010F",
                    f"unapproved command-shaped evidence entry: {entry}",
                )
        evidence_path = ROOT / path_text
        require(evidence_path.is_file(), f"evidence path is missing: {path_text}")
        if symbol is not None:
            source = evidence_path.read_text(encoding="utf-8")
            require(re.search(rf"\b{re.escape(symbol)}\b", source) is not None, f"evidence symbol is missing: {entry}")


def check_truth() -> dict[str, Any]:
    truth = load_json(TRUTH_PATH)
    check_truth_shape(truth)
    check_truth_declared_semantics(truth)
    require(truth.get("schema_version") == "ForgeCADMCP010FStage0Truth@2", "unexpected truth schema")
    require(truth.get("task_id") == "FGC-MCP010F", "unexpected truth task")
    require(truth.get("canonical_sha256") == canonical_sha256(truth), "truth canonical hash mismatch")

    contract_manifest = load_json(CONTRACT_MANIFEST)
    declared = sorted(contract_manifest.get("schemas", []))
    schema_paths = list(SCHEMA_ROOT.glob("*.json"))
    actual = sorted(path.name for path in schema_paths)
    require(declared == actual, "contract manifest and schema directory drifted")

    parsed_read_names, parsed_write_names = source_tool_names()
    tool_summary = load_json(TOOL_SUMMARY_PATH)
    require_exact_keys(
        tool_summary,
        frozenset(
            "build_cohort_sha256 canonical_sha256 read_count read_manifest_sha256 read_names schema_version "
            "total_count write_count write_enabled_manifest_sha256 write_names".split()
        ),
        "MCP tool manifest summary",
    )
    require(tool_summary.get("schema_version") == "ForgeCADMcpToolManifestSummary@1", "unexpected MCP tool summary schema")
    require(tool_summary["build_cohort_sha256"] is None, "source tool summary unexpectedly claims a build cohort")
    read_names = tool_summary.get("read_names")
    write_names = tool_summary.get("write_names")
    require(isinstance(read_names, list) and all(isinstance(name, str) for name in read_names), "tool summary read names are invalid")
    require(isinstance(write_names, list) and all(isinstance(name, str) for name in write_names), "tool summary write names are invalid")
    require(read_names == sorted(set(read_names)), "tool summary read names are duplicate or unsorted")
    require(write_names == sorted(set(write_names)), "tool summary write names are duplicate or unsorted")
    require(set(read_names).isdisjoint(write_names), "tool summary classifies a tool as both read and write")
    require(parsed_read_names == read_names, "MCP source parser and compiled summary disagree on read tools")
    require(parsed_write_names == write_names, "MCP source parser and compiled summary disagree on write tools")
    require(tool_summary.get("read_count") == len(read_names), "tool summary read count is stale")
    require(tool_summary.get("write_count") == len(write_names), "tool summary write count is stale")
    require(tool_summary.get("total_count") == len(read_names) + len(write_names), "tool summary total count is stale")
    require(
        tool_summary.get("canonical_sha256") == canonical_sha256(tool_summary),
        "tool summary canonical hash mismatch",
    )
    tasks = task_rows()
    in_progress = sorted(task_id for task_id, row in tasks.items() if row["status"] == "in_progress")
    require(in_progress == ["FGC-MCP010F"], f"expected only MCP010F in progress, found {in_progress}")
    require(tasks["FGC-MCP010F"]["dependency"] == "MCP010E", "MCP010F dependency drifted")

    source_truth = truth["current_source"]
    require(source_truth["contracts"]["schema_count"] == len(actual), "truth schema count drifted")
    require(source_truth["contracts"]["manifest_sha256"] == sha256_file(CONTRACT_MANIFEST), "contract manifest hash drifted")
    require(
        source_truth["contracts"]["schema_content_set_sha256"] == contract_schema_content_set_sha256(schema_paths),
        "contract schema content-set hash drifted",
    )
    require(
        source_truth["contracts"]["schema_content_set_algorithm"] == "sha256(canonical-json(sorted[{path,sha256(bytes)}]))",
        "contract schema content-set algorithm drifted",
    )
    require(source_truth["mcp_tools"]["read_count"] == len(read_names), "truth read tool count drifted")
    require(source_truth["mcp_tools"]["write_count"] == len(write_names), "truth write tool count drifted")
    require(source_truth["mcp_tools"]["total_count"] == len(read_names) + len(write_names), "truth total tool count drifted")
    require(source_truth["mcp_tools"]["read_names"] == read_names, "truth read tool names drifted")
    require(source_truth["mcp_tools"]["write_names"] == write_names, "truth write tool names drifted")
    require(source_truth["mcp_tools"]["source_sha256"] == sha256_file(MCP_SOURCE), "MCP source hash drifted")
    require(source_truth["mcp_tools"]["summary_receipt_sha256"] == sha256_file(TOOL_SUMMARY_PATH), "MCP tool summary receipt bytes changed")
    require(source_truth["mcp_tools"]["read_manifest_sha256"] == tool_summary["read_manifest_sha256"], "read manifest hash drifted")
    require(
        source_truth["mcp_tools"]["write_enabled_manifest_sha256"] == tool_summary["write_enabled_manifest_sha256"],
        "write-enabled manifest hash drifted",
    )
    require(source_truth["task_chain"]["only_in_progress"] == "FGC-MCP010F", "truth task chain drifted")
    require(source_truth["task_chain"]["dependency"] == "MCP010E", "truth task dependency drifted")

    policy_truth = source_truth["visible_view_policy"]
    require(policy_truth["authority"] == "RUNTIME_SOURCE_POLICY_NOT_EMBEDDED_IN_ATTEMPT35_RECEIPT", "threshold authority drifted")
    require(policy_truth["runtime_source_sha256"] == sha256_file(RUNTIME_SOURCE), "Runtime threshold source drifted")
    require(policy_truth["viewer_projection_sha256"] == sha256_file(VIEWER_SOURCE), "Viewer threshold projection drifted")
    require(policy_truth["fit_plan_projection_sha256"] == sha256_file(FIT_PLAN_SOURCE), "fit-plan threshold projection drifted")
    require(
        runtime_visible_view_thresholds() == truth["provisional_retained_observation"]["thresholds"],
        "Runtime visible-view policy and benchmark thresholds disagree",
    )
    require(
        fit_plan_visible_view_thresholds() == truth["provisional_retained_observation"]["thresholds"],
        "fit-plan visible-view policy and Runtime truth disagree",
    )
    require(
        viewer_visible_view_thresholds() == truth["provisional_retained_observation"]["thresholds"],
        "Viewer visible-view policy and Runtime truth disagree",
    )

    check_receipt_binding(truth)
    check_auxiliary_runs(truth)
    check_run_inventory(truth)
    check_packaged_viewer(truth)
    check_evidence_manifest(truth)
    check_authority_docs(truth)
    check_truth_negative_semantics(truth)

    return {
        "schema_count": len(actual),
        "read_tool_count": len(read_names),
        "write_tool_count": len(write_names),
        "total_tool_count": len(read_names) + len(write_names),
        "provisional_observation_candidate": truth["provisional_retained_observation"]["candidate_id"],
        "benchmark_eligibility": truth["provisional_retained_observation"]["benchmark_eligibility"],
        "provisional_visible_view_gate": truth["provisional_retained_observation"]["current_candidate_visible_view_gate"],
        "benchmark_evidence_status": truth["evidence_status"],
        "camera_binding": truth["provisional_retained_observation"]["camera_binding"]["binding_status"],
        "assertions": truth["assertion_ledger"],
        "latest_attempt": truth["latest_attempt"]["source_receipt_path"],
        "latest_attempt_status": truth["latest_attempt"]["status"],
        "latest_completed_transport": truth["latest_completed_transport"]["source_receipt_path"],
        "packaged_viewer_binding": truth["packaged_viewer"]["provisional_observation_binding"],
    }


def main() -> int:
    summary = check_truth()
    print(json.dumps({"schema_version": "ForgeCADMCP010FStage0TruthGate@1", "status": "PASS", **summary}, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
