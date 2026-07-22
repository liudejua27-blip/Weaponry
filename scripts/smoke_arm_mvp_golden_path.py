#!/usr/bin/env python3
"""Execute the smallest honest robotic-arm MVP path.

This is an aggregate gate, not an alternate implementation.  Its primary
evidence is one release-shaped packaged app run followed by a fresh-process
resume against the same Rust-owned library.  That path covers V003 synthesis,
C106 production geometry, A005 confirmation, Snapshot, export and recovery
under one project/turn/version lineage.  A second real packaged WebView run
proves the user-facing F026/C106/A005/single-renderer path without fixture,
proxy or accessibility substitution.
"""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = "ForgeCADArmMvpGoldenPath@1"
SERVICE_ROOT = "recipe_c106_arm_service_display"
GOLDEN_BRIEF = "流线三关节维护机械臂，固定基座、双连杆、旋转腕部和夹爪"
PACKAGED_SCHEMA = "ForgeCADArmMvpPackagedProtocolProof@1"
RESUME_SCHEMA = "ForgeCADArmMvpPackagedResumeProof@1"
WEBVIEW_SCHEMA = "ForgeCADArmWebViewQa@1"
EXPECTED_PROVIDER = {
    "source_kind": "offline_deterministic",
    "internal_subrequests": 8,
    "action_loop_steps": 8,
    "product_tool_calls": 7,
    "external_network_calls": 0,
    "credential_reads": 0,
}
C106_PRODUCTION_TRIANGLE_MIN = 12_000
C106_PRODUCTION_TRIANGLE_MAX = 24_000


class GateFailure(RuntimeError):
    pass


def run(label: str, command: list[str]) -> str:
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=1_200,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise GateFailure(f"ARM_MVP_{label}_UNAVAILABLE") from exc
    if completed.returncode:
        detail = f"{completed.stdout}\n{completed.stderr}"[-5_000:]
        raise GateFailure(f"ARM_MVP_{label}_FAILED: {detail}")
    return completed.stdout


def final_json(stdout: str, *, label: str) -> dict[str, Any]:
    # Composed gates emit either one compact JSON line or one pretty-printed
    # JSON document after npm/build chatter. Only a column-zero opening brace
    # can start the top-level report; nested objects stay indented. Parsing the
    # full remaining suffix prevents accepting a nested child by accident.
    lines = stdout.splitlines()
    for index in range(len(lines) - 1, -1, -1):
        if not lines[index].startswith("{"):
            continue
        candidate = "\n".join(lines[index:]).strip()
        try:
            value = json.loads(candidate)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    raise GateFailure(f"ARM_MVP_{label}_REPORT_MISSING")


def require_hash(value: object, *, label: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise GateFailure(f"ARM_MVP_{label}_HASH_INVALID")
    return value


def require_object(value: object, *, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise GateFailure(f"ARM_MVP_{label}_OBJECT_INVALID")
    return value


def require_string(value: object, *, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise GateFailure(f"ARM_MVP_{label}_STRING_INVALID")
    return value


def require_int_at_least(value: object, minimum: int, *, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < minimum:
        raise GateFailure(f"ARM_MVP_{label}_INTEGER_INVALID")
    return value


def require_int_range(value: object, minimum: int, maximum: int, *, label: str) -> int:
    if (
        not isinstance(value, int)
        or isinstance(value, bool)
        or not minimum <= value <= maximum
    ):
        raise GateFailure(f"ARM_MVP_{label}_INTEGER_INVALID")
    return value


def packaged_lineage(report: dict[str, Any]) -> dict[str, Any]:
    """Validate the phase-1 proof before exposing any IDs to the aggregate."""
    if (
        report.get("schema_version") != PACKAGED_SCHEMA
        or report.get("status") != "pass"
        or report.get("brief") != GOLDEN_BRIEF
    ):
        raise GateFailure("ARM_MVP_PACKAGED_PHASE1_REPORT_INVALID")
    project_id = require_string(report.get("project_id"), label="PACKAGED_PROJECT_ID")
    thread_id = require_string(report.get("thread_id"), label="PACKAGED_THREAD_ID")
    turn_id = require_string(report.get("turn_id"), label="PACKAGED_TURN_ID")
    v1_id = require_string(report.get("v1_asset_version_id"), label="PACKAGED_V1_ID")
    if report.get("root_recipe_id") != SERVICE_ROOT:
        raise GateFailure("ARM_MVP_PACKAGED_ROOT_RECIPE_INVALID")
    preview = require_object(report.get("preview"), label="PACKAGED_PREVIEW")
    if preview.get("artifact_profile_id") != "production_concept":
        raise GateFailure("ARM_MVP_PACKAGED_PREVIEW_PROFILE_INVALID")
    preview_id = require_string(preview.get("preview_id"), label="PACKAGED_PREVIEW_ID")
    preview_sha = require_hash(preview.get("glb_sha256"), label="PACKAGED_PREVIEW_GLB")
    preview_triangles = require_int_range(
        preview.get("triangle_count"),
        C106_PRODUCTION_TRIANGLE_MIN,
        C106_PRODUCTION_TRIANGLE_MAX,
        label="PACKAGED_PREVIEW_TRIANGLES",
    )
    a005 = require_object(report.get("a005"), label="PACKAGED_A005")
    change_set_id = require_string(a005.get("change_set_id"), label="PACKAGED_A005_CHANGESET")
    v2_id = require_string(a005.get("v2_asset_version_id"), label="PACKAGED_V2_ID")
    if a005.get("parent_asset_version_id") != v1_id:
        raise GateFailure("ARM_MVP_PACKAGED_A005_PARENT_DRIFT")
    surface_adornment_count = require_int_at_least(
        a005.get("surface_adornment_count"), 1, label="PACKAGED_A005_ADORNMENTS"
    )
    active = require_object(report.get("active_design"), label="PACKAGED_ACTIVE_DESIGN")
    if active.get("asset_version_id") != v2_id:
        raise GateFailure("ARM_MVP_PACKAGED_ACTIVE_VERSION_DRIFT")
    snapshot_revision = require_int_at_least(
        active.get("snapshot_revision"), 1, label="PACKAGED_SNAPSHOT_REVISION"
    )
    export = require_object(report.get("export"), label="PACKAGED_EXPORT")
    export_sha = require_hash(export.get("glb_sha256"), label="PACKAGED_EXPORT_GLB")
    export_triangles = require_int_range(
        export.get("triangle_count"),
        C106_PRODUCTION_TRIANGLE_MIN,
        C106_PRODUCTION_TRIANGLE_MAX,
        label="PACKAGED_EXPORT_TRIANGLES",
    )
    export_bytes = require_int_at_least(
        export.get("glb_byte_size"), 1, label="PACKAGED_EXPORT_BYTES"
    )
    if (
        export.get("asset_version_id") != v2_id
        or export.get("x_forgecad_glb_sha256") != export_sha
        or export_triangles != preview_triangles
    ):
        raise GateFailure("ARM_MVP_PACKAGED_EXPORT_DRIFT")
    provider = require_object(report.get("provider"), label="PACKAGED_PROVIDER")
    if provider != EXPECTED_PROVIDER:
        raise GateFailure("ARM_MVP_PACKAGED_PROVIDER_POLICY_INVALID")
    return {
        "project_id": project_id,
        "thread_id": thread_id,
        "turn_id": turn_id,
        "preview_id": preview_id,
        "preview_glb_sha256": preview_sha,
        "preview_triangle_count": preview_triangles,
        "v1_asset_version_id": v1_id,
        "a005_change_set_id": change_set_id,
        "v2_asset_version_id": v2_id,
        "surface_adornment_count": surface_adornment_count,
        "snapshot_revision": snapshot_revision,
        "export_glb_sha256": export_sha,
        "export_glb_byte_size": export_bytes,
        "export_triangle_count": export_triangles,
        "provider": provider,
    }


def packaged_resume(report: dict[str, Any], phase1: dict[str, Any]) -> dict[str, Any]:
    """Require a new packaged process to re-read exactly the phase-1 head."""
    if report.get("schema_version") != RESUME_SCHEMA or report.get("status") != "pass":
        raise GateFailure("ARM_MVP_PACKAGED_RESUME_REPORT_INVALID")
    if (
        report.get("project_id") != phase1["project_id"]
        or report.get("expected_asset_version_id") != phase1["v2_asset_version_id"]
    ):
        raise GateFailure("ARM_MVP_PACKAGED_RESUME_IDENTITY_DRIFT")
    active = require_object(report.get("active_design"), label="PACKAGED_RESUME_ACTIVE_DESIGN")
    if (
        active.get("asset_version_id") != phase1["v2_asset_version_id"]
        or active.get("snapshot_revision") != phase1["snapshot_revision"]
    ):
        raise GateFailure("ARM_MVP_PACKAGED_RESUME_SNAPSHOT_DRIFT")
    export = require_object(report.get("export"), label="PACKAGED_RESUME_EXPORT")
    if (
        export.get("asset_version_id") != phase1["v2_asset_version_id"]
        or export.get("glb_sha256") != phase1["export_glb_sha256"]
        or export.get("x_forgecad_glb_sha256") != phase1["export_glb_sha256"]
        or export.get("glb_byte_size") != phase1["export_glb_byte_size"]
        or export.get("triangle_count") != phase1["export_triangle_count"]
    ):
        raise GateFailure("ARM_MVP_PACKAGED_RESUME_EXPORT_DRIFT")
    return {
        "active_asset_version_id": phase1["v2_asset_version_id"],
        "snapshot_revision": phase1["snapshot_revision"],
        "export_glb_sha256": phase1["export_glb_sha256"],
        "export_glb_byte_size": phase1["export_glb_byte_size"],
        "export_triangle_count": phase1["export_triangle_count"],
    }


def packaged_webview_proof(report: dict[str, Any]) -> dict[str, Any]:
    if (
        report.get("schema_version") != WEBVIEW_SCHEMA
        or report.get("ok") is not True
        or report.get("real_packaged_webview") is not True
        or report.get("fixture_or_proxy_used") is not False
        or report.get("accessibility_api_used") is not False
        or report.get("provider_network_calls") != 0
        or report.get("credential_reads") != 0
        or report.get("single_renderer") is not True
        or report.get("c106_single_result_confirmed") is not True
        or report.get("a005_v2_confirmed") is not True
        or report.get("r007b_preview_seen") is not True
        or report.get("r007b_v3_confirmed") is not True
        or report.get("v3_glb_download_confirmed") is not True
        or report.get("visual_fidelity_validated") is not False
        or report.get("snapshot_restart_restored") is not True
    ):
        raise GateFailure("ARM_MVP_PACKAGED_WEBVIEW_REPORT_INVALID")
    initial = require_object(report.get("initial"), label="PACKAGED_WEBVIEW_INITIAL")
    restart = require_object(report.get("restart"), label="PACKAGED_WEBVIEW_RESTART")
    project_id = require_string(initial.get("project_id"), label="PACKAGED_WEBVIEW_PROJECT")
    v1_id = require_string(initial.get("v1_asset_version_id"), label="PACKAGED_WEBVIEW_V1")
    v2_id = require_string(initial.get("v2_asset_version_id"), label="PACKAGED_WEBVIEW_V2")
    v3_id = require_string(initial.get("v3_asset_version_id"), label="PACKAGED_WEBVIEW_V3")
    preview_sha = require_hash(
        initial.get("preview_artifact_sha256"), label="PACKAGED_WEBVIEW_PREVIEW"
    )
    snapshot_revision = require_int_at_least(
        initial.get("snapshot_revision"), 1, label="PACKAGED_WEBVIEW_SNAPSHOT"
    )
    glb_capture = require_object(
        initial.get("v3_production_glb"), label="PACKAGED_WEBVIEW_V3_GLB_CAPTURE"
    )
    screenshot_capture = require_object(
        initial.get("v3_viewport_screenshot"), label="PACKAGED_WEBVIEW_V3_SCREENSHOT_CAPTURE"
    )
    if (
        glb_capture.get("relative_path") != "qa-artifacts/arm-webview/initial/v3_production_glb.glb"
        or require_hash(glb_capture.get("sha256"), label="PACKAGED_WEBVIEW_V3_GLB_CAPTURE") == preview_sha
        or require_int_range(
            glb_capture.get("triangle_count"),
            C106_PRODUCTION_TRIANGLE_MIN,
            C106_PRODUCTION_TRIANGLE_MAX,
            label="PACKAGED_WEBVIEW_V3_GLB_TRIANGLES",
        ) < C106_PRODUCTION_TRIANGLE_MIN
        or require_int_at_least(
            glb_capture.get("complete_pbr_material_count"),
            1,
            label="PACKAGED_WEBVIEW_V3_PBR",
        ) < 1
        or require_int_at_least(glb_capture.get("byte_size"), 1, label="PACKAGED_WEBVIEW_V3_GLB_BYTES") < 1
        or screenshot_capture.get("relative_path") != "qa-artifacts/arm-webview/initial/v3_viewport_png.png"
        or not isinstance(screenshot_capture.get("sha256"), str)
        or require_hash(screenshot_capture.get("sha256"), label="PACKAGED_WEBVIEW_V3_SCREENSHOT") == preview_sha
        or require_int_at_least(screenshot_capture.get("byte_size"), 1, label="PACKAGED_WEBVIEW_V3_SCREENSHOT_BYTES") < 1
        or require_int_at_least(screenshot_capture.get("width"), 320, label="PACKAGED_WEBVIEW_V3_SCREENSHOT_WIDTH") < 320
        or require_int_at_least(screenshot_capture.get("height"), 240, label="PACKAGED_WEBVIEW_V3_SCREENSHOT_HEIGHT") < 240
    ):
        raise GateFailure("ARM_MVP_PACKAGED_WEBVIEW_CAPTURE_INVALID")
    captures = require_object(report.get("captures"), label="PACKAGED_WEBVIEW_CAPTURE_FILES")
    copied_glb = require_object(captures.get("v3_production_glb"), label="PACKAGED_WEBVIEW_CAPTURED_GLB")
    copied_screenshot = require_object(captures.get("v3_viewport_screenshot"), label="PACKAGED_WEBVIEW_CAPTURED_SCREENSHOT")
    if (
        copied_glb.get("sha256") != glb_capture.get("sha256")
        or copied_glb.get("byte_size") != glb_capture.get("byte_size")
        or copied_screenshot.get("sha256") != screenshot_capture.get("sha256")
        or copied_screenshot.get("byte_size") != screenshot_capture.get("byte_size")
        or not isinstance(copied_glb.get("path"), str)
        or not isinstance(copied_screenshot.get("path"), str)
    ):
        raise GateFailure("ARM_MVP_PACKAGED_WEBVIEW_CAPTURE_COPY_DRIFT")
    if (
        v1_id == v2_id
        or v2_id == v3_id
        or initial.get("renderer_generation") != 1
        or initial.get("active_webgl_contexts") != 1
        or initial.get("production_glb_render_source") != "glb_pbr"
        or initial.get("a005_preview_seen") is not True
        or initial.get("r007b_preview_seen") is not True
        or initial.get("r007b_v3_confirmed") is not True
        or initial.get("v3_glb_download_confirmed") is not True
        or restart.get("project_id") != project_id
        or restart.get("v3_asset_version_id") != v3_id
        or restart.get("snapshot_revision") != snapshot_revision
        or restart.get("renderer_generation") != 1
        or restart.get("active_webgl_contexts") != 1
        or restart.get("production_glb_render_source") != "glb_pbr"
        or restart.get("r007b_preview_seen") is not False
        or restart.get("r007b_v3_confirmed") is not False
        or restart.get("v3_glb_download_confirmed") is not False
        or restart.get("restart_hydrated") is not True
    ):
        raise GateFailure("ARM_MVP_PACKAGED_WEBVIEW_LINEAGE_DRIFT")
    return {
        "project_id": project_id,
        "turn_id": require_string(
            initial.get("turn_id"), label="PACKAGED_WEBVIEW_TURN"
        ),
        "preview_id": require_string(
            initial.get("preview_id"), label="PACKAGED_WEBVIEW_PREVIEW_ID"
        ),
        "preview_artifact_sha256": preview_sha,
        "v1_asset_version_id": v1_id,
        "v2_asset_version_id": v2_id,
        "v3_asset_version_id": v3_id,
        "snapshot_revision": snapshot_revision,
        "renderer_generation": 1,
        "active_webgl_contexts": 1,
        "production_glb_render_source": "glb_pbr",
        "r007b_preview_seen": True,
        "r007b_v3_confirmed": True,
        "v3_glb_download_confirmed": True,
        "v3_production_glb": {
            "sha256": glb_capture["sha256"],
            "byte_size": glb_capture["byte_size"],
            "triangle_count": glb_capture["triangle_count"],
            "complete_pbr_material_count": glb_capture["complete_pbr_material_count"],
            "path": copied_glb["path"],
        },
        "v3_viewport_screenshot": {
            "sha256": screenshot_capture["sha256"],
            "byte_size": screenshot_capture["byte_size"],
            "width": screenshot_capture["width"],
            "height": screenshot_capture["height"],
            "path": copied_screenshot["path"],
        },
        "visual_fidelity_validated": False,
        "restart_hydrated": True,
    }


def main() -> int:
    packaged = final_json(
        run("PACKAGED", ["script/build_and_run.sh", "--mvp-arm-verify"]),
        label="PACKAGED",
    )
    phase1 = packaged_lineage(require_object(packaged.get("phase1"), label="PACKAGED_PHASE1"))
    resumed = packaged_resume(
        require_object(packaged.get("resume"), label="PACKAGED_RESUME"), phase1
    )
    packaged_webview = packaged_webview_proof(
        final_json(
            run("PACKAGED_WEBVIEW", ["npm", "run", "arm:packaged-webview-qa"]),
            label="PACKAGED_WEBVIEW",
        )
    )
    print(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "status": "pass",
                "golden_brief": GOLDEN_BRIEF,
                "root_recipe_id": SERVICE_ROOT,
                "steps": {
                    "single_v003_synthesis": {
                        "project_id": phase1["project_id"],
                        "thread_id": phase1["thread_id"],
                        "turn_id": phase1["turn_id"],
                        "preview_id": phase1["preview_id"],
                        "asset_version_id": phase1["v1_asset_version_id"],
                        "preview_glb_sha256": phase1["preview_glb_sha256"],
                    },
                    "c106_production_geometry": {
                        "artifact_profile_id": "production_concept",
                        "glb_sha256": phase1["preview_glb_sha256"],
                        "triangle_count": phase1["preview_triangle_count"],
                    },
                    "packaged_workbench_single_renderer": {
                        "evidence_scope": "real_packaged_webview_separate_packaged_c106_lineage",
                        **packaged_webview,
                    },
                    "a005_preview_confirm": {
                        "change_set_id": phase1["a005_change_set_id"],
                        "parent_asset_version_id": phase1["v1_asset_version_id"],
                        "asset_version_id": phase1["v2_asset_version_id"],
                        "surface_adornment_count": phase1["surface_adornment_count"],
                    },
                    "packaged_restart_recovery": {
                        **resumed,
                    },
                    "glb_export": {
                        "asset_version_id": phase1["v2_asset_version_id"],
                        "glb_sha256": phase1["export_glb_sha256"],
                        "glb_byte_size": phase1["export_glb_byte_size"],
                        "triangle_count": phase1["export_triangle_count"],
                    },
                },
                "provider_boundary": phase1["provider"],
                "visual_fidelity_validated": False,
                "formal_eligible": False,
                "m108b_status": "blocked",
                "limits": [
                    "The packaged two-launch proof verifies one Rust-owned V003 → C106 → A005 → Snapshot → export lineage and its restart readback.",
                    "The packaged probe verifies the deterministic C106 service-display root recipe from persisted AssemblyGraph provenance.",
                    "The real packaged WebView proof verifies F026, one decoded PBR canvas, C106 single-result confirmation, A005 V2, R007B V3, visible V3 export, same-Blob GLB readback and a Rust-saved V3 viewport screenshot without fixture, proxy or accessibility substitution.",
                    "The packaged protocol/export lineage and packaged WebView lineage are separate deterministic runs; both use the same frozen app and bundled sidecar source.",
                    "This is an automated engineering closure, not M108B human visual approval, semantic image scoring, or a production-release claim.",
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
        print(
            json.dumps(
                {
                    "schema_version": SCHEMA_VERSION,
                    "status": "fail",
                    "error": str(error),
                    "formal_eligible": False,
                },
                ensure_ascii=False,
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        raise SystemExit(1) from None
