#!/usr/bin/env python3
"""Run the MCP010C fixed-render/visual-review loop in an isolated Runtime.

This is a source-built contract and transport gate, not a visual likeness
claim. It deliberately uses a tiny synthetic reference and a typed V2
candidate, then proves that the MCP image content block is backed by the
candidate-bound RenderSet CAS object.
"""

from __future__ import annotations

import copy
import base64
import hashlib
import json
import os
import re
import socket
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
from probe_mcp010b_raw_stdio import (  # noqa: E402
    GateFailure,
    MCP_PROTOCOL_VERSION,
    McpClient,
    build_identity,
    shutdown_runtime,
    v2_program_draft,
    wait_for_ready,
    write_receipt,
)


def canonical_hash(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateFailure(message)


def robot_v2_program_draft(project_id: str, catalog_hash: str) -> dict[str, Any]:
    """Conservative visible three-quarter blockout for the supplied robot image.

    This is authored data, not an image-to-mesh shortcut: it uses only the
    live primitive@2 catalog and leaves hidden/rear detail unknown.
    """
    nodes: list[dict[str, Any]] = []
    outputs: list[dict[str, Any]] = []

    def add(node_id: str, shape: str, parameters: dict[str, Any], *, material_zone_id: str = "zone-white-shell", inputs: list[str] | None = None, emit_output: bool = True) -> None:
        nodes.append({"node_id": node_id, "operator_id": "forgecad.geometry.primitive@2", "inputs": [], "parameters": {"shape": shape, **parameters}})
        if emit_output:
            outputs.append({"part_id": node_id, "input_node_ids": inputs or [node_id], "material_zone_id": material_zone_id, "solid": True})

    def box(node_id: str, size: list[float], position: list[float], rotation: list[float], **kwargs: Any) -> None:
        add(node_id, "box", {"size_m": size, "position_m": position, "rotation_rad": rotation}, **kwargs)

    def ellipsoid(node_id: str, radii: list[float], position: list[float], rotation: list[float], **kwargs: Any) -> None:
        add(node_id, "ellipsoid", {"radii_m": radii, "longitude_segments": 20, "latitude_segments": 12, "position_m": position, "rotation_rad": rotation}, **kwargs)

    def cylinder(node_id: str, radius: float, height: float, position: list[float], rotation: list[float], **kwargs: Any) -> None:
        add(node_id, "cylinder", {"radius_m": radius, "height_m": height, "radial_segments": 20, "position_m": position, "rotation_rad": rotation}, **kwargs)

    def sphere(node_id: str, radius: float, position: list[float], **kwargs: Any) -> None:
        add(node_id, "sphere", {"radius_m": radius, "longitude_segments": 20, "latitude_segments": 12, "position_m": position, "rotation_rad": [0.0, 0.0, 0.0]}, **kwargs)

    ellipsoid("head-shell", [0.44, 0.50, 0.42], [0.0, 3.02, 0.0], [0.0, 0.12, 0.0])
    box("visor", [0.62, 0.27, 0.14], [0.0, 3.01, 0.38], [0.0, -0.18, 0.0], material_zone_id="zone-black-mechanical")
    cylinder("neck", 0.22, 0.38, [0.0, 2.54, 0.0], [0.0, 0.0, 0.0], material_zone_id="zone-black-mechanical")
    box("chest-shell", [1.28, 0.96, 0.54], [0.0, 2.03, 0.0], [0.0, 0.0, 0.0], inputs=["chest-shell", "chest-panel"])
    add("chest-panel", "box", {"size_m": [0.64, 0.24, 0.10], "position_m": [0.0, 2.17, 0.36], "rotation_rad": [0.0, 0.0, 0.0]}, emit_output=False)
    ellipsoid("chest-core", [0.42, 0.54, 0.31], [0.0, 1.95, 0.34], [0.0, 0.0, 0.0], material_zone_id="zone-black-mechanical")
    box("chest-light", [0.09, 0.16, 0.04], [0.0, 2.02, 0.49], [0.0, 0.0, 0.0], material_zone_id="zone-amber-emissive")
    sphere("shoulder-left", 0.32, [-0.82, 2.17, 0.0])
    sphere("shoulder-right", 0.32, [0.82, 2.17, 0.0])
    box("upper-arm-left", [0.32, 0.72, 0.34], [-0.98, 1.70, 0.02], [0.0, 0.0, -0.12])
    box("upper-arm-right", [0.32, 0.72, 0.34], [0.98, 1.70, 0.02], [0.0, 0.0, 0.12])
    cylinder("elbow-left", 0.17, 0.27, [-0.99, 1.28, 0.08], [1.5708, 0.0, 0.0], material_zone_id="zone-black-mechanical")
    cylinder("elbow-right", 0.17, 0.27, [0.99, 1.28, 0.08], [1.5708, 0.0, 0.0], material_zone_id="zone-black-mechanical")
    box("forearm-left", [0.28, 0.68, 0.30], [-1.0, 0.94, 0.12], [0.0, 0.0, -0.06])
    box("forearm-right", [0.28, 0.68, 0.30], [1.0, 0.94, 0.12], [0.0, 0.0, 0.06])
    ellipsoid("hand-left", [0.22, 0.30, 0.20], [-1.0, 0.48, 0.18], [0.0, 0.0, 0.0], material_zone_id="zone-black-mechanical")
    ellipsoid("hand-right", [0.22, 0.30, 0.20], [1.0, 0.48, 0.18], [0.0, 0.0, 0.0], material_zone_id="zone-black-mechanical")
    box("pelvis", [0.98, 0.56, 0.50], [0.0, 1.24, 0.0], [0.0, 0.0, 0.0])
    cylinder("hip-left", 0.22, 0.30, [-0.38, 0.92, 0.0], [1.5708, 0.0, 0.0], material_zone_id="zone-black-mechanical")
    cylinder("hip-right", 0.22, 0.30, [0.38, 0.92, 0.0], [1.5708, 0.0, 0.0], material_zone_id="zone-black-mechanical")
    box("thigh-left", [0.38, 0.84, 0.40], [-0.37, 0.55, 0.0], [0.0, 0.0, 0.04])
    box("thigh-right", [0.38, 0.84, 0.40], [0.37, 0.55, 0.0], [0.0, 0.0, -0.04])
    sphere("knee-left", 0.22, [-0.37, 0.08, 0.08], material_zone_id="zone-black-mechanical")
    sphere("knee-right", 0.22, [0.37, 0.08, 0.08], material_zone_id="zone-black-mechanical")
    box("shin-left", [0.34, 0.70, 0.36], [-0.37, -0.34, 0.10], [0.0, 0.0, 0.0])
    box("shin-right", [0.34, 0.70, 0.36], [0.37, -0.34, 0.10], [0.0, 0.0, 0.0])
    box("foot-left", [0.38, 0.24, 0.60], [-0.37, -0.78, 0.18], [0.0, 0.0, 0.0])
    box("foot-right", [0.38, 0.24, 0.60], [0.37, -0.78, 0.18], [0.0, 0.0, 0.0])
    return {
        "schema_version": "GeometryProgram@2",
        "project_id": project_id,
        "representation_plan_sha256": "b" * 64,
        "operator_catalog_sha256": catalog_hash,
        "units": {"length": "meter", "angle": "radian", "coordinate_system": "right-handed-y-up"},
        "budgets": {"max_nodes": 64, "max_triangles": 50000, "max_glb_bytes": 8 * 1024 * 1024, "max_worker_memory_bytes": 536870912, "max_runtime_ms": 10000},
        "nodes": nodes,
        "part_outputs": outputs,
    }


def parse_args() -> Any:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--expected-build-cohort")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument(
        "--reference",
        type=Path,
        help="Optional user-authorized PNG reference. Without it the probe uses the 1x1 synthetic fixture.",
    )
    parser.add_argument(
        "--render-dir",
        type=Path,
        help="Optional temporary directory where returned AOV PNGs are decoded for inspection.",
    )
    parser.add_argument(
        "--human-review",
        action="store_true",
        help="Submit the fixed synthetic human-review fixture; never use this for an unscored user image.",
    )
    parser.add_argument(
        "--determinism-repeats",
        type=int,
        default=1,
        help="Repeat the same candidate-bound comparison this many times and require identical hashes (1-5).",
    )
    parser.add_argument(
        "--export-restart",
        action="store_true",
        help="Synthetic structural-only check: confirm before visual comparison, export, restart Runtime, and replay the export without hash drift.",
    )
    parser.add_argument("--timeout", type=float, default=30.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    require(args.mcp.is_file() and args.runtime.is_file(), "source MCP010C binaries were unavailable")
    require(1 <= args.determinism_repeats <= 5, "determinism-repeats must be between 1 and 5")
    require(
        not args.export_restart or args.reference is None,
        "export-restart is limited to the synthetic structural fixture; do not confirm a real user reference here",
    )
    if args.expected_build_cohort:
        require(re.fullmatch(r"[0-9a-f]{64}", args.expected_build_cohort) is not None, "invalid build cohort")
        mcp_identity = build_identity(args.mcp)
        runtime_identity = build_identity(args.runtime)
        require(mcp_identity.get("build_cohort_sha256") == args.expected_build_cohort, "MCP cohort mismatch")
        require(runtime_identity.get("build_cohort_sha256") == args.expected_build_cohort, "Runtime cohort mismatch")
    else:
        mcp_identity = runtime_identity = None

    data_root = args.data_root.resolve()
    require(not data_root.exists(), "isolated C data root must not pre-exist")
    data_root.mkdir(mode=0o700, parents=True)
    reference_bytes = bytes.fromhex(
        "89504e470d0a1a0a0000000d4948445200000001000000010804000000b51c0c020000000b4944415478da6364f80f00010501012718e3660000000049454e44ae426082"
    )
    reference_mime = "image/png"
    reference_width = 1
    reference_height = 1
    reference_declaration = "MCP010C isolated synthetic reference"
    if args.reference is not None:
        reference_path = args.reference.expanduser()
        require(reference_path.is_file(), "authorized reference file was unavailable")
        reference_bytes = reference_path.read_bytes()
        require(len(reference_bytes) <= 8 * 1024 * 1024, "authorized reference exceeded 8 MiB")
        require(reference_bytes.startswith(b"\x89PNG\r\n\x1a\n"), "real-reference probe currently accepts PNG only")
        require(len(reference_bytes) >= 24, "PNG reference header was truncated")
        reference_width = int.from_bytes(reference_bytes[16:20], "big")
        reference_height = int.from_bytes(reference_bytes[20:24], "big")
        require(1 <= reference_width <= 8192 and 1 <= reference_height <= 8192, "PNG reference dimensions were outside Runtime bounds")
        reference_mime = "image/png"
        reference_declaration = "User authorized the supplied robot reference for ForgeCAD modeling"
    ready_path = data_root / "ipc" / "ready.json"

    def start_runtime() -> subprocess.Popen[str]:
        return subprocess.Popen(
            [
                str(args.runtime),
                "serve",
                "--database",
                str(data_root / "runtime.sqlite"),
                "--cas-root",
                str(data_root / "cas"),
                "--endpoint-dir",
                str(data_root / "ipc"),
                "--ready-file",
                str(ready_path),
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
        )

    runtime = start_runtime()
    client: McpClient | None = None
    ready: dict[str, Any] | None = None
    try:
        ready = wait_for_ready(ready_path, runtime, args.timeout)
        socket_path = ready.get("socket_path")
        token = ready.get("token")
        require(isinstance(socket_path, str) and isinstance(token, str), "ready handoff was incomplete")
        environment = os.environ.copy()
        for key in ("FORGECAD_RUNTIME_COMMAND", "FORGECAD_RUNTIME_DATA_DIR", "FORGECAD_RUNTIME_READY_FILE", "FORGECAD_RUNTIME_STATUS_FILE"):
            environment.pop(key, None)
        environment.pop("FORGECAD_RUNTIME_SOCKET", None)
        environment.pop("FORGECAD_RUNTIME_TOKEN", None)
        if args.export_restart:
            # Keep the adapter on the ready-file backend so a restarted
            # Runtime may publish a fresh authenticated token and socket.
            environment["FORGECAD_RUNTIME_READY_FILE"] = str(ready_path)
        else:
            environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
            environment["FORGECAD_RUNTIME_TOKEN"] = token
        environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
        client = McpClient(args.mcp, environment, args.timeout)
        initialized = client.request(
            "initialize",
            {"protocolVersion": MCP_PROTOCOL_VERSION, "capabilities": {}, "clientInfo": {"name": "mcp010c-raw-stdio", "version": "1"}},
        )
        require(initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION, "MCP initialize failed")
        client.notify("notifications/initialized")

        listed = client.request("tools/list")
        tools = listed.get("result", {}).get("tools")
        require(isinstance(tools, list) and len(tools) == 36, "C source tool manifest did not expose 20 read + 16 write tools")
        render_tool = next((tool for tool in tools if tool.get("name") == "render_pass_get"), None)
        require(isinstance(render_tool, dict) and render_tool.get("annotations", {}).get("readOnlyHint") is True, "render_pass_get was not read-only")

        project = client.tool("project_create", {"name": "MCP010C isolated visual loop", "policy": {"profile": "mvp"}})
        project_id = project.get("project_id") if isinstance(project, dict) else None
        require(isinstance(project_id, str) and project_id, "project_create omitted project_id")
        reference_result = client.tool(
            "reference_import",
            {
                "project_id": project_id,
                "source": {"kind": "inline_content", "mime": reference_mime, "content_base64": base64.b64encode(reference_bytes).decode("ascii")},
                "authorization": {"user_authorized": True, "declaration": reference_declaration},
            },
        )
        reference = reference_result.get("reference") if isinstance(reference_result, dict) else None
        require(isinstance(reference, dict), "reference_import omitted evidence")
        reference_id = reference.get("reference_id")
        reference_sha = reference.get("object_sha256")
        require(isinstance(reference_id, str) and isinstance(reference_sha, str), "reference evidence binding was incomplete")

        catalog = client.tool("operator_catalog_get")
        catalog_hash = catalog.get("canonical_sha256") if isinstance(catalog, dict) else None
        require(isinstance(catalog_hash, str) and len(catalog_hash) == 64, "operator catalog hash unavailable")
        draft = robot_v2_program_draft(project_id, catalog_hash) if args.reference else v2_program_draft(project_id, catalog_hash)
        hashed = client.tool("geometry_program_hash", {"schema_version": "GeometryProgramHashRequest@1", "geometry_program_draft": draft})
        program_hash = hashed.get("canonical_sha256") if isinstance(hashed, dict) else None
        require(isinstance(program_hash, str) and len(program_hash) == 64, "V2 program hash unavailable")
        program = copy.deepcopy(draft)
        program["canonical_sha256"] = program_hash
        prepared = client.tool("geometry_prepare", {"project_id": project_id, "request": {"typed": "geometry", "reference_id": reference_id, "geometry_program": program}})
        candidate = prepared.get("candidate") if isinstance(prepared, dict) else None
        require(isinstance(candidate, dict), "geometry_prepare omitted candidate")
        candidate_id = candidate.get("candidate_id")
        require(isinstance(candidate_id, str), "geometry candidate id unavailable")
        version_id: str | None = None
        if args.export_restart:
            confirmed = client.tool(
                "candidate_confirm",
                {
                    "project_id": project_id,
                    "candidate_id": candidate_id,
                    "base_version_id": None,
                    "prepared_object_id": candidate.get("prepared_object_id"),
                    "prepared_object_sha256": candidate.get("prepared_object_sha256"),
                    "quality_report_id": candidate.get("quality_report_id"),
                    "approval_receipt_id": "mcp010c-export-restart-confirm",
                    "approval_summary": "Confirm synthetic structural export/restart fixture",
                    "approval_session_id": "mcp010c-export-restart-session",
                    "approval_expires_at": "9999999999",
                    "idempotency_key": "mcp010c-export-restart-confirm-once",
                },
            )
            version_id = confirmed.get("version_id") if isinstance(confirmed, dict) else None
            require(isinstance(version_id, str) and version_id, "export-restart confirmation omitted version_id")

        view_spec: dict[str, Any] = {
            "schema_version": "ReferenceViewSpec@1",
            "reference_id": reference_id,
            "reference_sha256": reference_sha,
            "view_id": "three-quarter-user-reference" if args.reference else "three-quarter-isolated",
            "source_view": "three-quarter",
            "image": {"width": reference_width, "height": reference_height, "rotation_degrees": 0.0, "crop": {"x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0}},
            "landmarks": [],
            "regions": [],
            "canonical_sha256": "",
        }
        view_spec["canonical_sha256"] = canonical_hash(view_spec)
        comparison_request = {"project_id": project_id, "candidate_id": candidate_id, "reference_id": reference_id, "view_spec": view_spec}
        comparison = client.tool("reference_compare_prepare", comparison_request)
        render_set = comparison.get("render_set") if isinstance(comparison, dict) else None
        require(isinstance(render_set, dict) and render_set.get("schema_version") == "RenderSet@2", "reference_compare_prepare omitted RenderSet@2")
        require(render_set.get("passes") == ["beauty", "silhouette", "depth", "normal", "ao", "part-id", "material-id", "wireframe", "uv-stretch"], "RenderSet did not contain the fixed nine AOV order")
        render_set_hash = comparison.get("render_set_object_sha256")
        comparison_hash = comparison.get("comparison_report_object_sha256")
        quality_report_object_sha256 = comparison.get("quality_report_object_sha256")
        require(
            isinstance(render_set_hash, str)
            and isinstance(comparison_hash, str)
            and isinstance(quality_report_object_sha256, str),
            "visual evidence CAS hashes were omitted",
        )
        baseline_pass_artifacts = render_set.get("pass_artifacts")
        require(isinstance(baseline_pass_artifacts, dict), "RenderSet pass artifacts were omitted")
        for repeat_index in range(1, args.determinism_repeats):
            repeated = client.tool("reference_compare_prepare", comparison_request)
            repeated_render_set = repeated.get("render_set") if isinstance(repeated, dict) else None
            require(
                repeated.get("render_set_object_sha256") == render_set_hash
                and repeated.get("comparison_report_object_sha256") == comparison_hash,
                f"deterministic comparison repeat {repeat_index + 1} changed a CAS hash",
            )
            require(
                isinstance(repeated_render_set, dict)
                and repeated_render_set.get("pass_artifacts") == baseline_pass_artifacts,
                f"deterministic comparison repeat {repeat_index + 1} changed a pass artifact",
            )

        image_response = client.request("tools/call", {"name": "render_pass_get", "arguments": {"render_set_hash": render_set_hash, "pass": "beauty"}})
        result = image_response.get("result")
        content = result.get("content") if isinstance(result, dict) else None
        image = next((item for item in content or [] if isinstance(item, dict) and item.get("type") == "image"), None)
        structured = result.get("structuredContent") if isinstance(result, dict) else None
        require(isinstance(image, dict) and image.get("mimeType") == "image/png" and isinstance(image.get("data"), str) and image.get("data"), "render_pass_get did not return an MCP image block")
        require(isinstance(structured, dict) and "png_base64" not in structured, "render_pass_get leaked duplicate base64 into structuredContent")

        if args.render_dir:
            render_dir = args.render_dir.expanduser()
            render_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
            for pass_name in render_set["passes"]:
                pass_response = client.request("tools/call", {"name": "render_pass_get", "arguments": {"render_set_hash": render_set_hash, "pass": pass_name}})
                pass_result = pass_response.get("result") if isinstance(pass_response, dict) else None
                pass_content = pass_result.get("content") if isinstance(pass_result, dict) else None
                pass_image = next((item for item in pass_content or [] if isinstance(item, dict) and item.get("type") == "image"), None)
                require(isinstance(pass_image, dict) and isinstance(pass_image.get("data"), str) and pass_image.get("data"), f"render_pass_get missing image block for {pass_name}")
                (render_dir / f"{pass_name.replace('-', '_')}.png").write_bytes(base64.b64decode(pass_image["data"], validate=True))

        issues = []
        if args.reference:
            issues = [{"issue_id": "primitive-blockout", "pass": "silhouette", "region_id": "whole-body", "claim": "Primitive-only candidate is a structural blockout and does not yet reproduce the panel, vent, cable and joint detail visible in the reference.", "confidence": 0.98, "visibility": "observed", "action": "Keep this candidate as comparison evidence; activate supported hard-surface detail operators in a later MCP010D goal before claiming likeness."}]
        review = client.tool("visual_review_submit", {"candidate_id": candidate_id, "reference_id": reference_id, "render_set_hash": render_set_hash, "comparison_report_hash": comparison_hash, "round": 1, "stage": "silhouette", "issues": issues, "status": "needs_revision"})
        require(review.get("review", {}).get("schema_version") == "VisualReviewReport@1", "visual_review_submit did not persist VisualReviewReport@1")
        human_status = "NOT_RUN"
        if args.human_review:
            require(not args.reference, "--human-review cannot be combined with a real user reference")
            human = client.tool("human_visual_review_submit", {"candidate_id": candidate_id, "reference_id": reference_id, "render_set_hash": render_set_hash, "comparison_report_hash": comparison_hash, "scores": {"likeness": 3, "geometry_detail": 3, "material_fidelity": 2, "editability": 5}, "approved": False})
            require(human.get("receipt", {}).get("schema_version") == "HumanVisualReviewReceipt@1", "human_visual_review_submit did not persist a receipt")
            human_status = "PASS"
        quality = client.tool("quality_get", {"candidate_id": candidate_id, "reference_id": reference_id})
        require(quality.get("schema_version") == "QualityReport@2" and quality.get("render_set_hash") == render_set_hash and quality.get("comparison_report_hash") == comparison_hash, "quality_get did not return candidate-bound C evidence")
        wrong = client.tool_error("render_pass_get", {"render_set_hash": "0" * 64, "pass": "beauty"})
        require(wrong.get("schema_version") == "RuntimeError@1", "wrong RenderSet hash did not fail closed")

        export_restart_receipt: dict[str, Any] | None = None
        if args.export_restart:
            require(isinstance(version_id, str), "export-restart version was unavailable")
            export_prepared = client.tool(
                "export_prepare",
                {
                    "project_id": project_id,
                    "version_id": version_id,
                    "format": "glb",
                    "profile": "mvp-glb",
                    "request": {"reason": "MCP010C source export/restart hash fixture"},
                },
            )
            export_manifest = export_prepared.get("manifest") if isinstance(export_prepared, dict) else None
            require(isinstance(export_manifest, dict), "export_prepare omitted manifest")
            export_request = {
                "project_id": project_id,
                "export_id": export_manifest.get("export_id"),
                "version_id": version_id,
                "format": "glb",
                "profile": "mvp-glb",
                "approval_receipt_id": "mcp010c-source-export-approval",
                "approval_summary": "Approve synthetic structural export/restart fixture",
                "approval_session_id": "mcp010c-export-restart-session",
                "approval_expires_at": "9999999999",
                "idempotency_key": "mcp010c-source-export-once",
            }
            exported = client.tool("export_confirm", export_request)
            export_output_sha256 = exported.get("output_sha256") if isinstance(exported, dict) else None
            require(isinstance(export_output_sha256, str) and len(export_output_sha256) == 64, "export_confirm omitted output hash")
            export_manifest_sha256 = exported.get("manifest_sha256") if isinstance(exported, dict) else None
            require(isinstance(export_manifest_sha256, str) and len(export_manifest_sha256) == 64, "export_confirm omitted manifest hash")

            # Close only the Runtime; keep the MCP stdio session alive. The
            # ready-file backend must discover the fresh token/socket on the
            # next call, proving that visual evidence and export remain usable
            # after a real process restart.
            shutdown_runtime(ready, ready_path, runtime)
            ready = None
            runtime = start_runtime()
            ready = wait_for_ready(ready_path, runtime, args.timeout)
            restarted_status = client.tool("runtime_status", {})
            require(restarted_status.get("state") == "Ready", "Runtime was not Ready after restart")
            quality_after_restart = client.tool("quality_get", {"candidate_id": candidate_id, "reference_id": reference_id})
            require(
                quality_after_restart.get("artifact_sha256") == quality.get("artifact_sha256")
                and quality_after_restart.get("render_set_hash") == render_set_hash
                and quality_after_restart.get("comparison_report_hash") == comparison_hash,
                "quality hashes drifted after Runtime restart",
            )
            restarted_pass = client.request(
                "tools/call",
                {"name": "render_pass_get", "arguments": {"render_set_hash": render_set_hash, "pass": "beauty"}},
            )
            restarted_result = restarted_pass.get("result") if isinstance(restarted_pass, dict) else None
            restarted_structured = restarted_result.get("structuredContent") if isinstance(restarted_result, dict) else None
            require(
                isinstance(restarted_structured, dict)
                and restarted_structured.get("sha256") == baseline_pass_artifacts.get("beauty", {}).get("sha256"),
                "beauty pass hash drifted after Runtime restart",
            )
            export_replay = client.tool("export_confirm", export_request)
            require(
                export_replay.get("replayed") is True
                and export_replay.get("output_sha256") == export_output_sha256
                and export_replay.get("manifest_sha256") == export_manifest_sha256,
                "export hash drifted after Runtime restart",
            )
            export_restart_receipt = {
                "status": "PASS_WITH_QUALITY_TARGET_NOT_MET",
                "version_id": version_id,
                "candidate_id": candidate_id,
                "artifact_sha256": quality.get("artifact_sha256"),
                "reference_sha256": reference_sha,
                "render_set_sha256": render_set_hash,
                "comparison_report_sha256": comparison_hash,
                "quality_report_sha256": quality_report_object_sha256,
                "export_manifest_sha256": export_manifest_sha256,
                "export_output_sha256": export_output_sha256,
                "restart_runtime_status": restarted_status.get("state"),
                "beauty_pass_sha256": baseline_pass_artifacts.get("beauty", {}).get("sha256"),
                "quality_visual_status": quality.get("visual_status"),
                "persistent_user_data_touched": False,
                "structural_visual_claim": "NOT_CLAIMED",
            }

        receipt = {
            "schema_version": "ForgeCADMCP010CRawStdioProbe@1",
            "task_id": "FGC-MCP010C",
            "status": "PASS",
            "protocol_version": MCP_PROTOCOL_VERSION,
            "tool_count": 36,
            "fixed_renderer": "512x512-perspective-zbuffer-deterministic",
            "aov_count": 9,
            "aov_order": render_set["passes"],
            "render_pass_mcp_image_block": "PASS",
            "reference_mask": "local-border-flood-fill-morphology",
            "reference_mask_revision": "mask-2",
            "reference_compare": "PASS",
            "visual_review": "PASS",
            "human_review_receipt": human_status,
            "quality_binding": "PASS",
            "structural_visual_claim": "NOT_CLAIMED",
            "hq_360": "BLOCKED_REFERENCE_COVERAGE",
            "reference_source": "user_authorized_png" if args.reference else "synthetic_png",
            "reference_sha256": reference_sha,
            "reference_width": reference_width,
            "reference_height": reference_height,
            "candidate_id": candidate_id,
            "artifact_sha256": prepared.get("artifact", {}).get("object_sha256"),
            "render_set_sha256": render_set_hash,
            "comparison_report_sha256": comparison_hash,
            "pass_artifact_sha256": {
                pass_name: artifact.get("sha256")
                for pass_name, artifact in baseline_pass_artifacts.items()
                if isinstance(artifact, dict) and isinstance(artifact.get("sha256"), str)
            },
            "comparison_metrics": comparison.get("comparison_report", {}).get("metrics") if isinstance(comparison.get("comparison_report"), dict) else None,
            "quality_visual_status": quality.get("visual_status"),
            "quality_hard_gate_passed": quality.get("hard_gate_passed"),
            "render_passes_saved": 9 if args.render_dir else 1,
            "determinism_repeat_count": args.determinism_repeats,
            "determinism_hashes": "PASS" if args.determinism_repeats > 1 else "NOT_RUN",
            "expected_build_cohort_sha256": args.expected_build_cohort,
            "mcp_build_cohort_sha256": mcp_identity.get("build_cohort_sha256") if mcp_identity else None,
            "runtime_build_cohort_sha256": runtime_identity.get("build_cohort_sha256") if runtime_identity else None,
            "persistent_user_data_touched": False,
        }
        if export_restart_receipt is not None:
            receipt["export_restart_hash_evidence"] = export_restart_receipt
    finally:
        cleanup_error: BaseException | None = None
        if client is not None:
            try:
                client.close()
            except BaseException as error:
                cleanup_error = error
        if ready is not None:
            try:
                shutdown_runtime(ready, ready_path, runtime)
            except BaseException as error:
                if cleanup_error is None:
                    cleanup_error = error
        elif runtime.poll() is None:
            runtime.kill()
            runtime.wait(timeout=5)
        if cleanup_error is not None and sys.exc_info()[0] is None:
            raise cleanup_error
    write_receipt(args.evidence, receipt)
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(json.dumps({"schema_version": "ForgeCADMCP010CRawStdioProbe@1", "task_id": "FGC-MCP010C", "status": "FAIL", "reason": str(error)[:2000], "persistent_user_data_touched": False}, sort_keys=True))
        raise SystemExit(1)
