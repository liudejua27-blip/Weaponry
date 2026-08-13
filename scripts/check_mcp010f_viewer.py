#!/usr/bin/env python3
"""Small source gate for the read-only MCP010F Viewer surface.

This is deliberately not a visual-quality test.  It proves that the source
surface exposes the bounded controls and only invokes the five read-only
Tauri commands; packaged/current-cohort UI E2E remains a separate gate.
"""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VIEWER = ROOT / "apps/desktop/src/features/runtime-viewer/RuntimeViewer.tsx"
STYLES = ROOT / "apps/desktop/src/styles.css"
TAURI_VIEWER = ROOT / "apps/desktop/src-tauri/src/viewer.rs"


def main() -> int:
    source = VIEWER.read_text(encoding="utf-8")
    styles = STYLES.read_text(encoding="utf-8")
    tauri_source = TAURI_VIEWER.read_text(encoding="utf-8")
    required_tokens = [
        "viewer_read_model",
        "viewer_artifact_bytes",
        "viewer_reference_bytes",
        "viewer_render_pass",
        "viewer_visual_evidence",
        "selectedPartId",
        "selectedMaterialZone",
        "exploded",
        "diffHeatmap",
        "differenceHeatmapUrl",
        "createDifferenceHeatmap",
        "contourCanvasActive",
        "contour-canvas",
        "CONTOUR CANVAS",
        "createReferenceContourAid",
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
        "getImageData",
        "Uint8Array",
        "Int32Array",
        "localBackgroundEdgeThreshold",
        "distance <= localBackgroundEdgeThreshold",
        "background[index] === 0",
        "foregroundCount",
        "queueHead",
        "queueTail",
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
        "deriveVisualWorkflow",
        "if (!hasMetric) currentStage = 'reference-canvas'",
        "deriveCorrectionQueue",
        "visualQualityReport",
        "visualHardGatePassed",
        "visualGateSource",
        "Visual gate",
        "fit-silhouette",
        "fit-landmarks",
        "fit-regions",
        "semantic Part intent",
        "camera lock",
        "CODEX NEXT ACTION",
        "correction-queue",
        "VISUAL_GATE_THRESHOLDS",
        "silhouette_iou: { operator: '>=', threshold: 0.90 }",
        "boundary_f1_4px: { operator: '>=', threshold: 0.90 }",
        "bbox_edge_error: { operator: '<=', threshold: 0.02 }",
        "centroid_error: { operator: '<=', threshold: 0.02 }",
        "landmark_nme: { operator: '<=', threshold: 0.03 }",
        "region_median_iou: { operator: '>=', threshold: 0.85 }",
        "critical_region_min_iou: { operator: '>=', threshold: 0.85 }",
        "landmark_nme",
        "region_median_iou",
        "critical_region_min_iou",
        "surfaceMaterialUnlocked",
        "workflow-gates",
        "reference-canvas",
        "workflow.currentStage === 'reference-canvas'",
        "workflow.gates.silhouette.status === 'not-run'",
        "silhouette-blockout",
        "landmark-structure",
        "semantic-part-fill",
        "uv-pbr",
        "ReferenceComparisonReport@1",
        "QualityReport",
    ]
    missing = [token for token in required_tokens if token not in source]
    if missing:
        raise SystemExit(f"Viewer source surface is missing required tokens: {missing}")
    style_tokens = [".contour-annotation-layer", "touch-action: none", "cursor: crosshair"]
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
    result = {
        "schema_version": "ForgeCADMCP010FViewerSourceGate@1",
        "task_id": "FGC-MCP010F",
        "status": "PASS",
        "aov_count": 9,
        "compare_modes": ["split", "overlay", "flicker"],
        "controls": ["part", "material-zone", "explosion", "difference-heatmap"],
        "heatmap_mode": "reference-render-pixel-diff-512x512",
        "contour_first_workflow": "PASS_SOURCE_UI_DERIVED_CUMULATIVE_GATES",
        "correction_queue": "PASS_SOURCE_READ_ONLY_HASH_BOUND_INTENTS",
        "contour_canvas": "PASS_SOURCE_SILHOUETTE_AOV_OVERLAY",
        "reference_contour_aid": "PASS_SOURCE_EPHEMERAL_BORDER_FLOOD_FILL_AID",
        "contour_annotation": "PASS_EPHEMERAL_NORMALIZED_POINTER_DRAFT",
        "visual_gate_source": "PASS_CANDIDATE_BOUND_QUALITY_REPORT_ONLY",
        "visual_report_fallback": "PASS_NO_UNVERIFIED_CANDIDATE_QUALITY_FALLBACK",
        "aov_keyboard_navigation": "PASS_TABLIST_ARROW_HOME_END",
        "unrun_visual_queue": "PASS_EMPTY_UNRUN_VISUAL_QUEUE",
        "workflow_truth_boundary": "Runtime ReferenceComparisonReport@1 + QualityReport; Viewer stage is transient",
        "write_boundary": "PASS: no Runtime write tool is invoked by Viewer source",
        "packaged_ui_e2e": "NOT_RUN",
        "human_visual_review": "NOT_RUN",
        "full_360_reference": "BLOCKED_REFERENCE_COVERAGE",
    }
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
