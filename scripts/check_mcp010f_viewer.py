#!/usr/bin/env python3
"""Small source gate for the read-only MCP010F Viewer surface.

This is deliberately not a visual-quality test. It proves that the source
surface exposes the bounded controls and only invokes read-only Tauri
commands; packaged/current-cohort UI E2E remains a separate gate.
"""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VIEWER = ROOT / "apps/desktop/src/features/runtime-viewer/RuntimeViewer.tsx"
APP = ROOT / "apps/desktop/src/App.tsx"
TAURI_MAIN = ROOT / "apps/desktop/src-tauri/src/main.rs"
STYLES = ROOT / "apps/desktop/src/styles.css"
TAURI_VIEWER = ROOT / "apps/desktop/src-tauri/src/viewer.rs"
COMPARE_WORKER = ROOT / "apps/desktop/src/features/runtime-viewer/compare-worker.ts"
AGENTIC_DESIGN = ROOT / "apps/desktop/src/features/runtime-viewer/agentic-design.ts"
MECHANICAL_ANIMATION = ROOT / "apps/desktop/src/features/runtime-viewer/mechanical-animation.ts"
PROVENANCE_GRAPH = ROOT / "apps/desktop/src/features/runtime-viewer/provenance-graph.ts"
AUTHORING_MESH = ROOT / "apps/desktop/src/features/runtime-viewer/authoring-mesh.ts"


def main() -> int:
    source = VIEWER.read_text(encoding="utf-8")
    app_source = APP.read_text(encoding="utf-8")
    tauri_main_source = TAURI_MAIN.read_text(encoding="utf-8")
    styles = STYLES.read_text(encoding="utf-8")
    tauri_source = TAURI_VIEWER.read_text(encoding="utf-8")
    agentic_source = AGENTIC_DESIGN.read_text(encoding="utf-8")
    mechanical_animation_source = MECHANICAL_ANIMATION.read_text(encoding="utf-8")
    provenance_graph_source = PROVENANCE_GRAPH.read_text(encoding="utf-8")
    authoring_mesh_source = AUTHORING_MESH.read_text(encoding="utf-8")
    normalizer_check = subprocess.run(
        [
            "node",
            "--no-warnings",
            "--experimental-strip-types",
            "-e",
            (
                f"import({json.dumps(MECHANICAL_ANIMATION.as_uri())}).then(m => {{"
                "const r=m.mechanicalAnimationNormalizerSelfCheck();"
                "console.log(JSON.stringify(r));"
                "if(!r.passed) process.exit(1)"
                "})"
            ),
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    normalizer_result = json.loads(normalizer_check.stdout)
    required_normalizer_checks = {
        "inventory-ready",
        "link-hierarchy-ready",
        "candidate-mismatch-fail-closed",
        "provenance-hash-mismatch-fail-closed",
        "frame-preview-ready",
        "frame-tick-mismatch-fail-closed",
        "partial-part-delta-fail-closed",
        "part-owner-map-ready",
        "duplicate-part-owner-fail-closed",
        "nested-part-owner-fail-closed",
        "unknown-part-owner-fail-closed",
        "nonidentity-part-owner-fail-closed",
        "bone-part-owner-fail-closed",
        "skinned-part-owner-fail-closed",
        "embedded-animation-fail-closed",
    }
    if set(normalizer_result.get("checks", [])) != required_normalizer_checks:
        raise SystemExit(f"Mechanical animation normalizer fixtures drifted: {normalizer_result}")
    mechanical_animation_race_check = subprocess.run(
        [
            "node",
            "--no-warnings",
            "--experimental-strip-types",
            "-e",
            (
                f"import({json.dumps(MECHANICAL_ANIMATION.as_uri())}).then(async m => {{"
                "const r=await m.mechanicalAnimationFrameDeferredResponseSelfCheck();"
                "console.log(JSON.stringify(r));"
                "if(!r.passed) process.exit(1)"
                "})"
            ),
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    mechanical_animation_race_result = json.loads(mechanical_animation_race_check.stdout)
    required_mechanical_animation_race_checks = {
        "stale-success-rejected",
        "stale-finally-rejected",
        "latest-success-wins",
        "stale-error-rejected",
        "next-latest-wins",
    }
    if set(mechanical_animation_race_result.get("checks", [])) != required_mechanical_animation_race_checks:
        raise SystemExit(f"Mechanical animation deferred response fixtures drifted: {mechanical_animation_race_result}")
    provenance_normalizer_check = subprocess.run(
        [
            "node",
            "--no-warnings",
            "--experimental-strip-types",
            "-e",
            (
                f"import({json.dumps(PROVENANCE_GRAPH.as_uri())}).then(m => {{"
                "const r=m.provenanceGraphNormalizerSelfCheck();"
                "console.log(JSON.stringify(r));"
                "if(!r.passed) process.exit(1)"
                "})"
            ),
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    provenance_normalizer_result = json.loads(provenance_normalizer_check.stdout)
    required_provenance_checks = {
        "positive-ready",
        "cross-candidate-fail-closed",
        "stale-state-fail-closed",
        "artifact-mismatch-fail-closed",
        "dangling-edge-fail-closed",
        "duplicate-edge-fail-closed",
        "cycle-fail-closed",
        "modifier-history-omission-explicit",
        "modifier-history-unsupported-fail-closed",
    }
    if set(provenance_normalizer_result.get("checks", [])) != required_provenance_checks:
        raise SystemExit(f"Provenance graph normalizer fixtures drifted: {provenance_normalizer_result}")
    provenance_race_check = subprocess.run(
        [
            "node",
            "--no-warnings",
            "--experimental-strip-types",
            "-e",
            (
                f"import({json.dumps(PROVENANCE_GRAPH.as_uri())}).then(async m => {{"
                "const r=await m.provenanceGraphDeferredResponseSelfCheck();"
                "console.log(JSON.stringify(r));"
                "if(!r.passed) process.exit(1)"
                "})"
            ),
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    provenance_race_result = json.loads(provenance_race_check.stdout)
    required_provenance_race_checks = {
        "stale-success-rejected",
        "stale-finally-rejected",
        "latest-success-wins",
        "stale-error-rejected",
        "next-latest-wins",
    }
    if set(provenance_race_result.get("checks", [])) != required_provenance_race_checks:
        raise SystemExit(f"Provenance graph deferred response fixtures drifted: {provenance_race_result}")
    for token in (
        "MAX_NODES = 64",
        "MAX_EDGES = 128",
        "persistentUserDataTouched: false",
        "isAcyclic",
        "PROVENANCE_GRAPH_TOPOLOGY_INVALID",
        "PROVENANCE_GRAPH_MODIFIER_APPLY_HISTORY_UNSUPPORTED",
        "UNSUPPORTED_MODIFIER_APPLY_PROJECTION_KEYS",
        "TODO(MCP010F): consume Modifier Apply history",
    ):
        if token not in provenance_graph_source:
            raise SystemExit(f"Provenance graph fail-closed normalizer is missing: {token}")
    for token in ("window.localStorage", "globalThis.localStorage", "fetch("):
        if token in provenance_graph_source:
            raise SystemExit(f"Provenance graph normalizer must remain ephemeral and offline: {token}")
    for token in (
        "AuthoringMesh@1",
        "candidate-program-artifact-readback-bound@1",
        "non-bijective-derived-only@1",
        "cross_version_stable: false",
        "runtime_write_performed: false",
        "persistent_user_data_touched: false",
        "normalizeAuthoringMesh",
        "isCurrentAuthoringMeshResponse",
        "AUTHORING_MESH_BINDING_MISMATCH",
    ):
        if token not in authoring_mesh_source:
            raise SystemExit(f"AuthoringMesh fail-closed normalizer is missing: {token}")
    for token in ("window.localStorage", "globalThis.localStorage", "fetch("):
        if token in authoring_mesh_source:
            raise SystemExit(f"AuthoringMesh normalizer must remain ephemeral and offline: {token}")
    for token in (
        "artifactReadbackSha256 !== normalizedClip.artifactReadbackSha256",
        "geometryCandidateEvidenceSha256 !== normalizedClip.geometryCandidateEvidenceSha256",
        "programSha256 !== normalizedClip.programSha256",
        "operatorCatalogSha256 !== normalizedClip.operatorCatalogSha256",
        "readbackConfigSha256 !== normalizedClip.readbackConfigSha256",
    ):
        if token not in mechanical_animation_source:
            raise SystemExit(f"Mechanical animation nested provenance binding is missing: {token}")
    for token in (
        "validateMechanicalAnimationPartOwners",
        "GLB_PART_OWNER_MAPPING_INVALID",
        "GLB_EMBEDDED_ANIMATION_UNSUPPORTED",
        "GLB_SKIN_OR_BONE_UNSUPPORTED",
    ):
        if token not in mechanical_animation_source:
            raise SystemExit(f"Mechanical animation Part-owner gate is missing: {token}")
    for token in (
        "GLB_NODE_PART_OWNER_MISSING",
        "metadataPartId !== partId",
        "parentWorldInverse",
        "localEnd.sub(localOrigin)",
    ):
        if token not in source:
            raise SystemExit(f"Mechanical animation GLB ownership/transform composition is missing: {token}")
    required_tokens = [
        "viewer_read_model",
        "viewer_read_model_summary",
        "viewer_artifact_bytes",
        "viewer_reference_bytes",
        "viewer_render_pass",
        "viewer_visual_evidence",
        "viewer_mechanical_animation_inventory",
        "viewer_mechanical_animation_clip",
        "viewer_mechanical_animation_frame_preview",
        "viewer_provenance_graph",
        "viewer_authoring_mesh",
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
        "reference-contour-aid",
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
        "临时轮廓草图",
        "runtime_write: false",
        "setPointerCapture",
        "OrbitControls",
        "Raycaster",
        "pickViewportObjectFromPointer",
        "mouseButtons",
        "RIGHT: THREE.MOUSE.ROTATE",
        "MIDDLE: THREE.MOUSE.PAN",
        "右键拖动：旋转视角",
        "中键拖动：平移视角",
        "aria-keyshortcuts",
        "Shift 加左键框选",
        "ResizeObserver",
        "disposeObjectResources",
        "renderer.dispose()",
        "selectedCandidateId",
        "AUTO_LATEST_CANDIDATE",
        "selectedObjectIds",
        "replaceViewportSelection",
        "viewportHoverFrameRef",
        "requestAnimationFrame",
        'role="treeitem"',
        "sceneTreeFilter",
        "candidateSortOrder",
        "compareZoom",
        "comparePan",
        "measureMode",
        "exportCompareSnapshot",
        "compare-parameters",
        "dataUrlToBlob",
        "createImageBitmap",
        "OffscreenCanvas",
        "COMPARE_WORKER_DEBOUNCE_MS",
        "error-console",
        "refreshCurrentCandidate",
        "secondaryActionLabel",
        "切换自动候选",
        "pbrStatus",
        "PBR 材质区",
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
        "normalizeViewerProvenanceGraph",
        "candidateStateSha256",
        "provenanceGraphRequestRef",
        "Provenance Graph",
        "role=\"tree\"",
        "complete-or-fail",
    ]
    missing = [token for token in required_tokens if token not in source]
    required_token_variants = [
        ("CONTOUR CANVAS", "轮廓画布"),
        ("REFERENCE CONTOUR AID", "参考轮廓引导"),
        ("复制 hash-bound 轮廓点集", "复制哈希绑定轮廓点集"),
        ("Visual gate", "可见性门"),
    ]
    missing.extend(
        f"{canonical} (or localized {localized})"
        for canonical, localized in required_token_variants
        if canonical not in source and localized not in source
    )
    if missing:
        raise SystemExit(f"Viewer source surface is missing required tokens: {missing}")
    if "<RuntimeViewer" not in app_source or "from './features/runtime-viewer/RuntimeViewer'" not in app_source:
        raise SystemExit("Desktop App must mount the Runtime Viewer as its only product entry surface")
    expected_runtime_commands = sorted({
        "viewer_read_model",
        "viewer_read_model_summary",
        "viewer_artifact_bytes",
        "viewer_reference_bytes",
        "viewer_render_pass",
        "viewer_visual_evidence",
        "viewer_agentic_projection",
        "viewer_agentic_session",
        "viewer_mechanical_animation_inventory",
        "viewer_mechanical_animation_clip",
        "viewer_mechanical_animation_frame_preview",
        "viewer_provenance_graph",
        "viewer_authoring_mesh",
    })
    actual_runtime_commands = sorted(set(re.findall(
        r"runtimeInvoke(?:<[^>]+>)?\(\s*['\"]([^'\"]+)", source
    )))
    if actual_runtime_commands != expected_runtime_commands:
        raise SystemExit(
            "Viewer invokes a Runtime command outside its read-only allowlist: "
            f"{actual_runtime_commands}"
        )
    actual_tauri_commands = sorted(set(re.findall(r"\bviewer_[a-z_]+\b", tauri_main_source)))
    if actual_tauri_commands != expected_runtime_commands:
        raise SystemExit(
            "Tauri Viewer command registration drifted from the read-only allowlist: "
            f"{actual_tauri_commands}"
        )
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
    style_tokens = [
        ".contour-annotation-layer",
        "touch-action: none",
        "cursor: crosshair",
        ".compare-parameters",
        ".error-console",
        ".status-icon",
        ".viewport-crosshair",
        ".runtime-shell .viewport-hints",
    ]
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
    snapshot_start = source.find("const exportCompareSnapshot = async () => {")
    snapshot_end = source.find("  const measuredPixels", snapshot_start)
    if snapshot_start < 0 or snapshot_end < 0:
        raise SystemExit("Viewer local compare snapshot boundary is missing")
    snapshot_source = source[snapshot_start:snapshot_end]
    local_snapshot_forbidden = [
        "runtimeInvoke",
        "tauriInvoke",
        "fetch(",
        "candidate_confirm",
        "export_confirm",
        "writeFile",
    ]
    leaked_local_snapshot_tokens = [token for token in local_snapshot_forbidden if token in snapshot_source]
    if leaked_local_snapshot_tokens:
        raise SystemExit(
            "Viewer compare snapshot must remain a local transient download: "
            f"{leaked_local_snapshot_tokens}"
        )
    if "read-only IPC client" not in tauri_source or "read_model" not in tauri_source:
        raise SystemExit("Viewer Tauri bridge is missing its read-only projection boundary")
    if '"preview_policy":"single-tick-transient-double-worker-replay@1"' not in tauri_source:
        raise SystemExit("Viewer mechanical animation frame bridge drifted from the closed Runtime preview policy")
    worker_source = COMPARE_WORKER.read_text(encoding="utf-8")
    worker_tokens = ["createDifferenceImage", "createContourImage", "decodeBlobToBuffer", "createImageBitmap", "OffscreenCanvas", "onmessage", "postMessage"]
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
    if "if (binding.visualEvidenceBound) return actual === expected" not in agentic_source:
        raise SystemExit("Agentic Viewer binding must require exact evidence hashes when visual evidence is bound")
    if "actual === null || Boolean(expected && actual === expected)" in agentic_source:
        raise SystemExit("Agentic Viewer binding must not accept missing Runtime evidence hashes")
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
        "agentic_evidence_binding": "PASS_FAIL_CLOSED_MISSING_OR_DRIFTED_HASH",
        "candidate_artifact_binding": "PASS_FAIL_CLOSED_SAME_CANDIDATE_ONLY",
        "candidate_binding_fixtures": "PASS_SAME_CANDIDATE_POSITIVE_CROSS_CANDIDATE_NEGATIVE_MISSING_EVIDENCE_NEGATIVE",
        "visual_evidence_binding_fixtures": "PASS_NO_QUALITY_PROJECT_ID_CROSS_CANDIDATE_RENDER_NEGATIVE_MISSING_REFERENCE_HASH_NEGATIVE",
        "aov_keyboard_navigation": "PASS_TABLIST_ARROW_HOME_END",
        "unrun_visual_queue": "PASS_RUNTIME_ACTION_PROJECTION_ONLY",
        "workflow_truth_boundary": "Runtime Agentic projection + ReferenceComparisonReport@1 + QualityReport; Viewer stage is display-only",
        "write_boundary": "PASS: no Runtime write tool is invoked by Viewer source",
        "mechanical_animation_normalizer_fixtures": "PASS_EXECUTED_PROVENANCE_FRAME_AND_PART_OWNER_NEGATIVES",
        "mechanical_animation_deferred_response_fixtures": "PASS_EXECUTED_STALE_SUCCESS_ERROR_FINALLY_REJECTED_LATEST_WINS",
        "provenance_graph_normalizer_fixtures": "PASS_EXECUTED_EXACT_STATE_DANGLING_DUPLICATE_CYCLE_NEGATIVES",
        "provenance_graph_deferred_response_fixtures": "PASS_EXECUTED_STALE_SUCCESS_ERROR_FINALLY_REJECTED_LATEST_WINS",
        "provenance_graph_policy": "PASS_COMPLETE_OR_FAIL_64_NODES_128_EDGES_READ_ONLY",
        "modifier_apply_history_projection": "FAIL_CLOSED_UNTIL_RUNTIME_BOUND_INTERFACE",
        "packaged_ui_e2e": "NOT_RUN",
        "human_visual_review": "NOT_RUN",
        "full_360_reference": "BLOCKED_REFERENCE_COVERAGE",
    }
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
