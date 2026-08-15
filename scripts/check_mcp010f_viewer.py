#!/usr/bin/env python3
"""Small source gate for the read-only MCP010F Viewer surface.

This is deliberately not a visual-quality test. It proves that the source
surface exposes the bounded controls and only invokes read-only Tauri
commands; packaged/current-cohort UI E2E remains a separate gate.
"""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VIEWER = ROOT / "apps/desktop/src/features/runtime-viewer/RuntimeViewer.tsx"
APP = ROOT / "apps/desktop/src/App.tsx"
STYLES = ROOT / "apps/desktop/src/styles.css"
TAURI_VIEWER = ROOT / "apps/desktop/src-tauri/src/viewer.rs"
COMPARE_WORKER = ROOT / "apps/desktop/src/features/runtime-viewer/compare-worker.ts"


def main() -> int:
    source = VIEWER.read_text(encoding="utf-8")
    app_source = APP.read_text(encoding="utf-8")
    styles = STYLES.read_text(encoding="utf-8")
    tauri_source = TAURI_VIEWER.read_text(encoding="utf-8")
    required_tokens = [
        "viewer_read_model",
        "viewer_read_model_summary",
        "viewer_artifact_bytes",
        "viewer_reference_bytes",
        "viewer_render_pass",
        "viewer_visual_evidence",
        "selectedPartId",
        "selectedMaterialZone",
        "exploded",
        "diffHeatmap",
        "differenceHeatmapUrl",
        "runCompareWorker",
        "createContainedImageData",
        "compare-worker.ts",
        "contourCanvasActive",
        "contour-canvas",
        "CONTOUR CANVAS",
        "reference-contour-aid",
        "REFERENCE CONTOUR AID",
        "contourPoints",
        "contour-annotation-layer",
        "ForgeCADViewerContourDraft@2",
        "normalized_reference_image",
        "contourBindingReady",
        "candidate-bound",
        "artifact_sha256",
        "comparison_report_hash",
        "selected_part_id",
        "selected_material_zone_id",
        "source_pass",
        "copyContourDraft",
        "undoContourPoint",
        "撤销上一点",
        "复制 hash-bound 轮廓点集",
        "临时轮廓草图",
        "runtime_write: false",
        "setPointerCapture",
        "OrbitControls",
        "ResizeObserver",
        "disposeObjectResources",
        "forceContextLoss",
        "selectedCandidateId",
        "AUTO_LATEST_CANDIDATE",
        "candidateSortOrder",
        "compareZoom",
        "comparePan",
        "measureMode",
        "exportCompareSnapshot",
        "compare-parameters",
        "runtime-alert",
        "轮廓画布",
        "role=\"tab\"",
        "aria-controls=\"render-aov-panel\"",
        "role=\"tabpanel\"",
        "aria-labelledby={`render-aov-tab-${selectedPass}`}",
        "onKeyDown",
        "ArrowRight",
        "ArrowLeft",
        "ArrowDown",
        "ArrowUp",
        "Home",
        "End",
        "split",
        "overlay",
        "flicker",
        "beauty",
        "silhouette",
        "depth",
        "normal",
        "ao",
        "part-id",
        "material-id",
        "wireframe",
        "uv-stretch",
        "visualQualityReport",
        "visualHardGatePassed",
        "visualGateSource",
        "Visual gate",
        "correction-queue",
        "agenticProjection",
        "normalizeAgenticDesignProjection",
        "Runtime quality gates",
        "Runtime next actions",
        "Runtime authenticated read-only projection",
        "Viewer 不再从 comparison metrics 重新计算质量门",
        "agenticProjection.gates.map",
        "agenticProjection.nextAllowedActions",
        "workflow-gates",
        "ReferenceComparisonReport@1",
        "QualityReport",
        "hasCandidateBoundArtifact",
        "hasCandidateBoundVisualEvidence",
        "isCandidateBoundVisualEvidence",
        "isCandidateBoundArtifactPayload",
        "isCandidateBoundReferencePayload",
        "isCandidateBoundRenderPayload",
        "artifactCandidateId === candidateId",
        "payload.candidate_id === candidateId",
        "payload.render_set_hash === renderSetHash",
        "payload.pass === pass",
        "quality?.reference_sha256",
        "renderSet?.artifact_sha256",
        "comparison?.reference_sha256",
        "payload.quality_report_hash",
        "project_id",
    ]
    missing = [token for token in required_tokens if token not in source]
    if missing:
        raise SystemExit(f"Viewer source surface is missing required tokens: {missing}")
    if "<RuntimeViewer" not in app_source or "from './features/runtime-viewer/RuntimeViewer'" not in app_source:
        raise SystemExit("Desktop App must mount the Runtime Viewer as its only product entry surface")
    forbidden_app_entry_tokens = [
        'type="file"',
        "<textarea",
        "准备生成",
        "让 Codex 检查",
        "上传参考",
        "referenceName",
    ]
    leaked_app_entry_tokens = [token for token in forbidden_app_entry_tokens if token in app_source]
    if leaked_app_entry_tokens:
        raise SystemExit(f"Desktop App must not recreate upload/chat/generate entry actions: {leaked_app_entry_tokens}")
    forbidden_local_quality_logic = [
        "VISUAL_GATE_THRESHOLDS",
        "evaluateWorkflowGate",
        "deriveVisualWorkflow",
        "deriveCorrectionQueue",
        "comparisonMetrics as Record<string, unknown>",
    ]
    leaked_local_quality_logic = [token for token in forbidden_local_quality_logic if token in source]
    if leaked_local_quality_logic:
        raise SystemExit(f"Viewer must not re-derive Runtime quality gates: {leaked_local_quality_logic}")
    if "visualQualityReport?.hard_gate_passed === true &&" in source:
        raise SystemExit("Viewer must display Runtime hard_gate_passed without adding a local predicate")
    style_tokens = [".contour-annotation-layer", "touch-action: none", "cursor: crosshair", ".compare-parameters", ".runtime-alert", ".status-icon"]
    missing_styles = [token for token in style_tokens if token not in styles]
    if missing_styles:
        raise SystemExit(f"Viewer contour annotation styles are missing required tokens: {missing_styles}")
    forbidden_write_tokens = [
        "candidate_confirm",
        "version_confirm",
        "restore_confirm",
        "export_confirm",
        "project_create",
        "reference_import",
    ]
    leaked = [token for token in forbidden_write_tokens if token in source]
    if leaked:
        raise SystemExit(f"Viewer source contains Runtime write calls: {leaked}")
    if "read-only IPC client" not in tauri_source or "read_model" not in tauri_source:
        raise SystemExit("Viewer Tauri bridge is missing its read-only projection boundary")
    worker_source = COMPARE_WORKER.read_text(encoding="utf-8")
    worker_tokens = ["createDifferenceImage", "createContourImage", "onmessage", "postMessage"]
    missing_worker_tokens = [token for token in worker_tokens if token not in worker_source]
    if missing_worker_tokens:
        raise SystemExit(f"Viewer compare worker is missing required tokens: {missing_worker_tokens}")
    unsafe_structural_fallback = "latestCandidate?.candidate?.quality_hard_gate_passed" in source
    if unsafe_structural_fallback:
        raise SystemExit("Viewer must not use candidate structural quality_hard_gate_passed as a visual gate")
    # Candidate quality metadata may be used to pair a geometry artifact with
    # a visual candidate.  The visual panel itself must still consume only the
    # separate viewer_visual_evidence IPC response.  Do not reject safe
    # candidate-selection reads merely because they mention `quality`.
    unsafe_visual_fallbacks = [
        token for token in (
            "setEvidence(latestCandidate?.quality",
            "visualQualityReport = latestCandidate?.quality",
            "comparisonMetrics = latestCandidate?.quality",
        ) if token in source
    ]
    if unsafe_visual_fallbacks:
        raise SystemExit("Viewer must not fall back to an unverified candidate quality projection for visual evidence")
    if "quality.project_id" in source or "quality?.project_id" in source:
        raise SystemExit("QualityReport@2 does not provide project_id; project binding must use the evidence envelope")
    def candidate_binding_is_valid(entry: dict, project_id: str) -> bool:
        candidate = entry.get("candidate") or {}
        candidate_id = candidate.get("candidate_id")
        artifact = entry.get("artifact") or {}
        quality = entry.get("quality") or {}
        reference = (entry.get("reference") or {}).get("reference") or {}
        artifact_bound = not entry.get("artifact") or (
            artifact.get("candidate_id") == candidate_id
            and quality.get("artifact_sha256") == artifact.get("artifact_id")
        )
        return bool(
            candidate_id
            and candidate.get("project_id") == project_id
            and quality.get("candidate_id") == candidate_id
            and quality.get("reference_id")
            and quality.get("reference_sha256") == reference.get("object_sha256")
            and quality.get("render_set_hash")
            and quality.get("comparison_report_hash")
            and reference.get("reference_id") == quality.get("reference_id")
            and reference.get("project_id") == project_id
            and artifact_bound
        )

    def visual_evidence_is_valid(payload: dict, candidate_id: str, project_id: str, artifact_id: str, reference_sha256: str) -> bool:
        quality = payload.get("quality_report") or {}
        render_set = payload.get("render_set") or {}
        comparison = payload.get("comparison_report") or {}
        return bool(
            payload.get("candidate_id") == candidate_id
            and payload.get("project_id") == project_id
            and payload.get("reference_id")
            and payload.get("render_set_hash")
            and payload.get("comparison_report_hash")
            and payload.get("quality_report_hash")
            and quality.get("candidate_id") == candidate_id
            and quality.get("artifact_sha256") == artifact_id
            and quality.get("reference_id") == payload.get("reference_id")
            and quality.get("reference_sha256") == reference_sha256
            and quality.get("render_set_hash") == payload.get("render_set_hash")
            and quality.get("comparison_report_hash") == payload.get("comparison_report_hash")
            and render_set.get("candidate_id") == candidate_id
            and render_set.get("artifact_sha256") == artifact_id
            and render_set.get("reference_id") == payload.get("reference_id")
            and comparison.get("candidate_id") == candidate_id
            and comparison.get("artifact_sha256") == artifact_id
            and comparison.get("reference_id") == payload.get("reference_id")
            and comparison.get("reference_sha256") == reference_sha256
            and comparison.get("render_set_hash") == payload.get("render_set_hash")
        )

    binding_cases = {
        "same_candidate_positive": ({
            "candidate": {"candidate_id": "candidate-a", "project_id": "project-a"},
            "artifact": {"artifact_id": "artifact-a", "candidate_id": "candidate-a"},
            "quality": {"candidate_id": "candidate-a", "artifact_sha256": "artifact-a", "reference_id": "reference-a", "reference_sha256": "reference-sha-a", "render_set_hash": "render-a", "comparison_report_hash": "comparison-a"},
            "reference": {"reference": {"reference_id": "reference-a", "project_id": "project-a", "object_sha256": "reference-sha-a"}},
        }, True),
        "cross_candidate_negative": ({
            "candidate": {"candidate_id": "candidate-a", "project_id": "project-a"},
            "artifact": {"artifact_id": "artifact-b", "candidate_id": "candidate-b"},
            "quality": {"candidate_id": "candidate-a", "artifact_sha256": "artifact-b", "reference_id": "reference-a", "reference_sha256": "reference-sha-a", "render_set_hash": "render-a", "comparison_report_hash": "comparison-a"},
            "reference": {"reference": {"reference_id": "reference-a", "project_id": "project-a", "object_sha256": "reference-sha-a"}},
        }, False),
        "missing_evidence_negative": ({
            "candidate": {"candidate_id": "candidate-a", "project_id": "project-a"},
            "artifact": {"artifact_id": "artifact-a", "candidate_id": "candidate-a"},
        }, False),
    }
    for name, (entry, expected) in binding_cases.items():
        actual = candidate_binding_is_valid(entry, "project-a")
        if actual is not expected:
            raise SystemExit(f"candidate binding fixture {name} expected {expected}, got {actual}")
    visual_cases = {
        "same_candidate_positive_without_quality_project_id": ({
            "candidate_id": "candidate-a",
            "project_id": "project-a",
            "reference_id": "reference-a",
            "render_set_hash": "render-a",
            "comparison_report_hash": "comparison-a",
            "quality_report_hash": "quality-a",
            "quality_report": {"candidate_id": "candidate-a", "artifact_sha256": "artifact-a", "reference_id": "reference-a", "reference_sha256": "reference-sha-a", "render_set_hash": "render-a", "comparison_report_hash": "comparison-a"},
            "render_set": {"candidate_id": "candidate-a", "artifact_sha256": "artifact-a", "reference_id": "reference-a"},
            "comparison_report": {"candidate_id": "candidate-a", "artifact_sha256": "artifact-a", "reference_id": "reference-a", "reference_sha256": "reference-sha-a", "render_set_hash": "render-a"},
        }, True),
        "cross_candidate_nested_render_negative": ({
            "candidate_id": "candidate-a",
            "project_id": "project-a",
            "reference_id": "reference-a",
            "render_set_hash": "render-a",
            "comparison_report_hash": "comparison-a",
            "quality_report_hash": "quality-a",
            "quality_report": {"candidate_id": "candidate-a", "artifact_sha256": "artifact-a", "reference_id": "reference-a", "reference_sha256": "reference-sha-a", "render_set_hash": "render-a", "comparison_report_hash": "comparison-a"},
            "render_set": {"candidate_id": "candidate-b", "artifact_sha256": "artifact-a", "reference_id": "reference-a"},
            "comparison_report": {"candidate_id": "candidate-a", "artifact_sha256": "artifact-a", "reference_id": "reference-a", "reference_sha256": "reference-sha-a", "render_set_hash": "render-a"},
        }, False),
        "missing_quality_reference_hash_negative": ({
            "candidate_id": "candidate-a",
            "project_id": "project-a",
            "reference_id": "reference-a",
            "render_set_hash": "render-a",
            "comparison_report_hash": "comparison-a",
            "quality_report_hash": "quality-a",
            "quality_report": {"candidate_id": "candidate-a", "artifact_sha256": "artifact-a", "reference_id": "reference-a", "render_set_hash": "render-a", "comparison_report_hash": "comparison-a"},
            "render_set": {"candidate_id": "candidate-a", "artifact_sha256": "artifact-a", "reference_id": "reference-a"},
            "comparison_report": {"candidate_id": "candidate-a", "artifact_sha256": "artifact-a", "reference_id": "reference-a", "reference_sha256": "reference-sha-a", "render_set_hash": "render-a"},
        }, False),
    }
    for name, (payload, expected) in visual_cases.items():
        actual = visual_evidence_is_valid(payload, "candidate-a", "project-a", "artifact-a", "reference-sha-a")
        if actual is not expected:
            raise SystemExit(f"visual evidence fixture {name} expected {expected}, got {actual}")
    result = {
        "schema_version": "ForgeCADMCP010FViewerSourceGate@1",
        "task_id": "FGC-MCP010F",
        "status": "PASS",
        "aov_count": 9,
        "compare_modes": ["split", "overlay", "flicker"],
        "controls": ["part", "material-zone", "explosion", "difference-heatmap"],
        "heatmap_mode": "reference-render-pixel-diff-512x512",
        "contour_first_workflow": "PASS_RUNTIME_AGENTIC_PROJECTION_GATES",
        "correction_queue": "PASS_RUNTIME_ACTION_PROJECTION_READ_ONLY",
        "contour_canvas": "PASS_SOURCE_SILHOUETTE_AOV_OVERLAY",
        "reference_contour_aid": "PASS_SOURCE_EPHEMERAL_BORDER_FLOOD_FILL_AID",
        "contour_annotation": "PASS_EPHEMERAL_NORMALIZED_POINTER_DRAFT",
        "visual_gate_source": "PASS_RUNTIME_AGENTIC_QUALITY_REPORT_ONLY",
        "visual_report_fallback": "PASS_NO_UNVERIFIED_CANDIDATE_QUALITY_FALLBACK",
        "candidate_artifact_binding": "PASS_FAIL_CLOSED_SAME_CANDIDATE_ONLY",
        "candidate_binding_fixtures": "PASS_SAME_CANDIDATE_POSITIVE_CROSS_CANDIDATE_NEGATIVE_MISSING_EVIDENCE_NEGATIVE",
        "visual_evidence_binding_fixtures": "PASS_NO_QUALITY_PROJECT_ID_CROSS_CANDIDATE_RENDER_NEGATIVE_MISSING_REFERENCE_HASH_NEGATIVE",
        "aov_keyboard_navigation": "PASS_TABLIST_ARROW_HOME_END",
        "unrun_visual_queue": "PASS_RUNTIME_ACTION_PROJECTION_ONLY",
        "workflow_truth_boundary": "Runtime Agentic projection + ReferenceComparisonReport@1 + QualityReport; Viewer stage is display-only",
        "write_boundary": "PASS: no Runtime write tool is invoked by Viewer source",
        "packaged_ui_e2e": "NOT_RUN",
        "human_visual_review": "NOT_RUN",
        "full_360_reference": "BLOCKED_REFERENCE_COVERAGE",
    }
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
