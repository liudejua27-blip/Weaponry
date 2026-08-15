#!/usr/bin/env python3
"""Run CADFit OptimizationJob against one real, user-authorized reference.

The probe intentionally keeps the optimization scope to one visible Part.  It
creates an isolated project, imports the authorized PNG into that temporary
CAS, obtains a Runtime-owned target/camera/Rig, runs the bounded silhouette
camera fit, and only then starts the asynchronous coarse -> mid -> final job.
It never confirms, versions, exports, or writes a user project.  The receipt
records hashes and status only; it does not retain the reference bytes.
"""

from __future__ import annotations

import base64
import copy
import json
import os
import shutil
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
    wait_for_ready,
)
from probe_mcp010c_codex_cli import (  # noqa: E402
    canonical_hash,
    silhouette_rig_draft,
    view_spec,
)
from probe_mcp010e_raw_stdio import (  # noqa: E402
    png_dimensions,
    robot_detail_program_draft,
    robot_reference_annotations,
)
from probe_mcp010f_part_correction import read_ponytail_preflight  # noqa: E402


COARSE_EVALUATIONS = 32
MID_TOP_K = 4
FINAL_TOP_K = 2
FINAL_CONTROLS = 1
EXPECTED_EVALUATIONS = COARSE_EVALUATIONS + MID_TOP_K + FINAL_TOP_K + FINAL_CONTROLS


class ProbeFailure(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProbeFailure(message)


def tool_value(client: McpClient, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
    value = client.tool(name, arguments)
    require(isinstance(value, dict), f"{name} did not return a typed object")
    return value


def write_evidence(path: Path | None, value: dict[str, Any]) -> None:
    if path is None:
        return
    root = Path(__file__).resolve().parents[1]
    resolved = path if path.is_absolute() else root / path
    evidence_root = (root / "docs" / "evidence").resolve()
    resolved.resolve().relative_to(evidence_root)
    resolved.parent.mkdir(parents=True, exist_ok=True)
    resolved.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def read_cas_json(data_root: Path, object_sha256: str) -> dict[str, Any]:
    """Read one JSON object from this probe's isolated CAS without exposing it."""
    require(isinstance(object_sha256, str) and len(object_sha256) == 64, "CAS object hash is invalid")
    object_path = data_root / "cas" / "objects" / object_sha256[:2] / object_sha256
    require(object_path.is_file() and not object_path.is_symlink(), "expected CAS object is missing")
    value = json.loads(object_path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), "expected CAS JSON object is not an object")
    return value


def resolve_camera(camera_result: dict[str, Any]) -> dict[str, Any]:
    selected = camera_result.get("selected_camera") or camera_result.get("camera")
    require(isinstance(selected, dict), "camera fit omitted selected camera")
    required = {
        "schema_version",
        "camera_hash",
        "projection",
        "transform",
        "fov_y_degrees",
        "near_m",
        "far_m",
        "resolution",
        "coordinate_system",
        "renderer_revision",
        "canonical_sha256",
    }
    if required.issubset(selected):
        return selected
    selected_hash = selected.get("camera_hash")
    selected_canonical = selected.get("canonical_sha256")
    for row in camera_result.get("candidates", []):
        candidate = row.get("camera") if isinstance(row, dict) else None
        if not isinstance(candidate, dict):
            continue
        if (
            isinstance(selected_hash, str)
            and candidate.get("camera_hash") == selected_hash
        ) or (
            isinstance(selected_canonical, str)
            and candidate.get("canonical_sha256") == selected_canonical
        ):
            require(required.issubset(candidate), "camera candidate was not a complete calibration")
            return candidate
    raise ProbeFailure("camera fit returned only an unresolvable camera reference")


def chest_rig(candidate_id: str) -> dict[str, Any]:
    source = silhouette_rig_draft(candidate_id)
    parameters = [
        item
        for item in source["parameters"]
        if isinstance(item, dict) and item.get("part_id") == "chest-shell"
    ]
    # Keep the optimizer's one-Part contract while exposing the two image-plane
    # offsets that the typed Runtime projection can apply to a panel sink.
    parameters.extend(
        [
            {
                "parameter_id": "chest-offset-x",
                "part_id": "chest-shell",
                "semantic": "offset_x",
                "value": 0.0,
                "min": -0.25,
                "max": 0.25,
                "step": 0.02,
                "unit": "meter",
            },
            {
                "parameter_id": "chest-offset-y",
                "part_id": "chest-shell",
                "semantic": "offset_y",
                "value": 0.0,
                "min": -0.25,
                "max": 0.25,
                "step": 0.02,
                "unit": "meter",
            },
        ]
    )
    return {
        "schema_version": "SilhouetteRig@1",
        "rig_id": "robot-chest-cadfit-rig",
        "candidate_id": candidate_id,
        "parameters": parameters,
        "canonical_sha256": "",
    }


def surface_backed_chest_shell(draft: dict[str, Any]) -> dict[str, Any]:
    """Replace the authored chest root with a typed, bounded surface shell."""
    result = copy.deepcopy(draft)
    nodes = result.get("nodes")
    outputs = result.get("part_outputs")
    require(isinstance(nodes, list), "GeometryProgram nodes were unavailable")
    require(isinstance(outputs, list), "GeometryProgram part outputs were unavailable")
    node = next(
        (item for item in nodes if isinstance(item, dict) and item.get("node_id") == "chest-panel"),
        None,
    )
    require(isinstance(node, dict), "chest-shell source node was unavailable")
    old_node_id = "chest-panel"
    new_node_id = "chest-shell-surface"
    node["node_id"] = new_node_id
    node["operator_id"] = "forgecad.geometry.surface-shell@1"
    node["parameters"] = {
        "shape": "surface-shell",
        "control_points": [
            [-0.83, 1.42, -0.02],
            [-0.276666666667, 1.42, 0.12],
            [0.276666666667, 1.42, 0.12],
            [0.83, 1.42, -0.02],
            [-0.83, 1.793333333333, -0.02],
            [-0.276666666667, 1.793333333333, 0.12],
            [0.276666666667, 1.793333333333, 0.12],
            [0.83, 1.793333333333, -0.02],
            [-0.83, 2.166666666667, -0.02],
            [-0.276666666667, 2.166666666667, 0.12],
            [0.276666666667, 2.166666666667, 0.12],
            [0.83, 2.166666666667, -0.02],
            [-0.83, 2.54, -0.02],
            [-0.276666666667, 2.54, 0.12],
            [0.276666666667, 2.54, 0.12],
            [0.83, 2.54, -0.02],
        ],
        "u_segments": 8,
        "v_segments": 8,
        "thickness_m": 0.68,
        "position_m": [0.0, 0.0, 0.0],
        "rotation_rad": [0.0, 0.0, 0.0],
    }
    for item in nodes:
        if isinstance(item, dict) and isinstance(item.get("inputs"), list):
            item["inputs"] = [new_node_id if value == old_node_id else value for value in item["inputs"]]
    matched_output = False
    for item in outputs:
        if not isinstance(item, dict) or not isinstance(item.get("input_node_ids"), list):
            continue
        if old_node_id in item["input_node_ids"]:
            item["input_node_ids"] = [new_node_id if value == old_node_id else value for value in item["input_node_ids"]]
            matched_output = matched_output or item.get("part_id") == "chest-shell"
    require(matched_output, "chest-shell Part output was not bound to the surface node")
    return result


def surface_chest_rig(candidate_id: str) -> dict[str, Any]:
    """Expose paired/mirrored surface controls as one Part-bound Rig."""
    parameters = [
        {"parameter_id": "control-point-4-x", "part_id": "chest-shell", "semantic": "surface_control_point", "control_point_index": 4, "axis": "x", "value": -0.83, "min": -1.05, "max": -0.60, "step": 0.03, "unit": "meter"},
        {"parameter_id": "control-point-7-x", "part_id": "chest-shell", "semantic": "surface_control_point", "control_point_index": 7, "axis": "x", "value": 0.83, "min": 0.60, "max": 1.05, "step": 0.03, "unit": "meter"},
        {"parameter_id": "control-point-8-x", "part_id": "chest-shell", "semantic": "surface_control_point", "control_point_index": 8, "axis": "x", "value": -0.83, "min": -1.05, "max": -0.60, "step": 0.03, "unit": "meter"},
        {"parameter_id": "control-point-11-x", "part_id": "chest-shell", "semantic": "surface_control_point", "control_point_index": 11, "axis": "x", "value": 0.83, "min": 0.60, "max": 1.05, "step": 0.03, "unit": "meter"},
        {"parameter_id": "control-point-12-x", "part_id": "chest-shell", "semantic": "surface_control_point", "control_point_index": 12, "axis": "x", "value": -0.83, "min": -1.05, "max": -0.60, "step": 0.03, "unit": "meter"},
        {"parameter_id": "control-point-15-x", "part_id": "chest-shell", "semantic": "surface_control_point", "control_point_index": 15, "axis": "x", "value": 0.83, "min": 0.60, "max": 1.05, "step": 0.03, "unit": "meter"},
        {"parameter_id": "control-point-5-z", "part_id": "chest-shell", "semantic": "surface_control_point", "control_point_index": 5, "axis": "z", "value": 0.12, "min": 0.00, "max": 0.28, "step": 0.02, "unit": "meter"},
        {"parameter_id": "control-point-6-z", "part_id": "chest-shell", "semantic": "surface_control_point", "control_point_index": 6, "axis": "z", "value": 0.12, "min": 0.00, "max": 0.28, "step": 0.02, "unit": "meter"},
    ]
    return {
        "schema_version": "SilhouetteRig@1",
        "rig_id": "robot-chest-surface-cadfit-rig",
        "candidate_id": candidate_id,
        "parameters": parameters,
        "canonical_sha256": "",
    }


def main() -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument("--expected-build-cohort")
    parser.add_argument(
        "--with-boolean-residual",
        action="store_true",
        help="Bind one Runtime critic/Part-error residual to CADFit and prove the Manifold Boolean node survives evaluation.",
    )
    parser.add_argument(
        "--surface-backed-chest-shell",
        action="store_true",
        help="Use a real surface-shell@1 chest Part and search paired surface control points through OptimizationJob.",
    )
    args = parser.parse_args()

    source = args.reference.expanduser().resolve()
    require(source.is_file() and not source.is_symlink(), "reference must be a regular file")
    require(args.mcp.is_file() and args.runtime.is_file(), "MCP/Runtime binaries are unavailable")
    require(args.timeout > 0, "timeout must be positive")
    data_root = args.data_root.expanduser().resolve()
    require(not data_root.exists(), "isolated optimization data root must not pre-exist")
    data_root.mkdir(mode=0o700, parents=True)

    reference_bytes = source.read_bytes()
    import hashlib

    reference_sha256 = hashlib.sha256(reference_bytes).hexdigest()
    mcp_identity = build_identity(args.mcp)
    runtime_identity = build_identity(args.runtime)
    worker_identities: dict[str, dict[str, Any]] = {}
    for component in ("forgecad-geometry-worker", "forgecad-render-worker"):
        executable = shutil.which(component, path=os.environ.get("PATH"))
        if executable is None:
            worker_identities[component] = {"build_cohort_sha256": None}
        else:
            worker_identities[component] = build_identity(Path(executable))
    build_cohorts = {
        "mcp": mcp_identity.get("build_cohort_sha256"),
        "runtime": runtime_identity.get("build_cohort_sha256"),
        "geometry_worker": worker_identities["forgecad-geometry-worker"].get("build_cohort_sha256"),
        "render_worker": worker_identities["forgecad-render-worker"].get("build_cohort_sha256"),
    }
    if args.expected_build_cohort:
        require(mcp_identity.get("build_cohort_sha256") == args.expected_build_cohort, "MCP build cohort mismatch")
        require(runtime_identity.get("build_cohort_sha256") == args.expected_build_cohort, "Runtime build cohort mismatch")
        require(
            all(value == args.expected_build_cohort for value in build_cohorts.values()),
            "MCP/Runtime/Geometry Worker/Render Worker build cohort mismatch",
        )

    ready_path = data_root / "ipc" / "ready.json"
    environment = os.environ.copy()
    for key in (
        "FORGECAD_RUNTIME_SOCKET",
        "FORGECAD_RUNTIME_TOKEN",
        "FORGECAD_RUNTIME_DATA_DIR",
        "FORGECAD_RUNTIME_COMMAND",
    ):
        environment.pop(key, None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    runtime: subprocess.Popen[str] | None = None
    client: McpClient | None = None
    ready: dict[str, Any] | None = None
    receipt: dict[str, Any] = {
        "schema_version": "ForgeCADMCP010FOptimizationRealReferenceProbe@1",
        "task_id": "FGC-MCP010F",
        "status": "BLOCKED",
        "expected_build_cohort_sha256": args.expected_build_cohort,
        "build_cohorts": build_cohorts,
        "reference_sha256": reference_sha256,
        "image_bytes_recorded": False,
        "persistent_user_data_touched": False,
        "candidate_confirmed": False,
        "version_count": 0,
    }
    try:
        runtime = subprocess.Popen(
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
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
        )
        ready = wait_for_ready(ready_path, runtime, args.timeout)
        socket_path = ready.get("socket_path")
        token = ready.get("token")
        require(isinstance(socket_path, str) and isinstance(token, str), "Runtime ready handoff was incomplete")
        environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
        environment["FORGECAD_RUNTIME_TOKEN"] = token
        client = McpClient(args.mcp, environment, max(args.timeout, 30.0))
        initialized = client.request(
            "initialize",
            {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "mcp010f-optimization-real-reference", "version": "1"},
            },
        )
        require(initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION, "MCP initialize failed")
        client.notify("notifications/initialized")
        preflight = read_ponytail_preflight(client)
        names = {
            item.get("name")
            for item in client.request("tools/list").get("result", {}).get("tools", [])
            if isinstance(item, dict)
        }
        required_tools = {
            "project_create",
            "reference_import",
            "operator_catalog_get",
            "geometry_program_hash",
            "geometry_prepare",
            "reference_mask_prepare",
            "scene_observe_get",
            "camera_fit_prepare",
            "silhouette_rig_hash",
            "silhouette_fit_prepare",
            "optimization_job_prepare",
            "optimization_job_get",
        }
        if args.with_boolean_residual:
            required_tools.update(
                {
                    "reference_compare_prepare",
                    "reference_mask_refine_prepare",
                    "silhouette_target_get",
                    "critic_report_get",
                    "silhouette_part_error_get",
                    "silhouette_evaluation_objective_prepare",
                }
            )
        require(required_tools.issubset(names), "real-reference CADFit tools are unavailable")

        project = tool_value(client, "project_create", {"name": "MCP010F real-reference CADFit", "policy": {"profile": "mvp"}})
        project_id = project.get("project_id")
        require(isinstance(project_id, str) and project_id, "project_create omitted project_id")
        reference_result = tool_value(
            client,
            "reference_import",
            {
                "project_id": project_id,
                "source": {
                    "kind": "inline_content",
                    "mime": "image/png",
                    "content_base64": base64.b64encode(reference_bytes).decode("ascii"),
                },
                "authorization": {
                    "user_authorized": True,
                    "declaration": "The user supplied and authorized this reference for local ForgeCAD modeling.",
                },
            },
        )
        reference = reference_result.get("reference") or {}
        reference_id = reference.get("reference_id")
        imported_sha256 = reference.get("object_sha256")
        require(isinstance(reference_id, str) and isinstance(imported_sha256, str), "reference evidence was incomplete")
        require(imported_sha256 == reference_sha256, "reference import changed the source hash")

        catalog = tool_value(client, "operator_catalog_get", {})
        catalog_sha256 = catalog.get("canonical_sha256")
        require(isinstance(catalog_sha256, str) and len(catalog_sha256) == 64, "operator catalog hash unavailable")
        draft = robot_detail_program_draft(project_id, catalog_sha256, "surface-linework", "armor-shell-zones")
        if args.surface_backed_chest_shell:
            draft = surface_backed_chest_shell(draft)
        hashed = tool_value(
            client,
            "geometry_program_hash",
            {"schema_version": "GeometryProgramHashRequest@1", "geometry_program_draft": draft},
        )
        program_sha256 = hashed.get("canonical_sha256")
        require(isinstance(program_sha256, str) and len(program_sha256) == 64, "geometry program hash unavailable")
        program = copy.deepcopy(draft)
        program["canonical_sha256"] = program_sha256
        prepared = tool_value(
            client,
            "geometry_prepare",
            {
                "project_id": project_id,
                "request": {"typed": "geometry", "reference_id": reference_id, "geometry_program": program},
            },
        )
        candidate = prepared.get("candidate") or {}
        candidate_id = candidate.get("candidate_id")
        artifact = prepared.get("artifact") or {}
        artifact_sha256 = artifact.get("object_sha256") or artifact.get("artifact_id")
        require(isinstance(candidate_id, str) and isinstance(artifact_sha256, str), "geometry candidate evidence was incomplete")

        landmarks, regions = robot_reference_annotations()
        target_landmarks = [
            {key: item[key] for key in ("landmark_id", "x", "y", "visibility")}
            for item in landmarks
        ]
        target = tool_value(
            client,
            "reference_mask_prepare",
            {
                "project_id": project_id,
                "reference_id": reference_id,
                "landmarks": target_landmarks,
                "parts": [],
            },
        )
        target_sha256 = target.get("target_sha256")
        require(isinstance(target_sha256, str) and len(target_sha256) == 64, "real reference target was not hash-bound")
        global_target_sha256 = target_sha256

        residual_target_sha256: str | None = None
        comparison_report_sha256: str | None = None
        comparison_metrics: dict[str, Any] | None = None
        comparison_camera_hash: str | None = None
        comparison_render_set_hash: str | None = None
        critic_report_sha256: str | None = None
        part_error_sha256: str | None = None
        part_region: dict[str, Any] | None = None
        residual: dict[str, Any] | None = None
        evaluation_objective_sha256: str | None = None
        if args.with_boolean_residual:
            # Keep the automatically derived whole-image mask as the global
            # gate, then bind the residual lane to an image-derived chest ROI.
            # This is an explicit bounded region, not a semantic segmentation
            # claim; it prevents a full-body mask from being mislabeled as the
            # chest Part while the global silhouette gate remains unchanged.
            chest_region = next(
                (region for region in regions if region.get("region_id") == "chest-armor"),
                None,
            )
            require(isinstance(chest_region, dict), "chest-armor reference region is unavailable")
            part_region = {
                key: chest_region[key]
                for key in ("region_id", "x", "y", "width", "height")
            }
            target_readback = tool_value(
                client,
                "silhouette_target_get",
                {"target_sha256": target_sha256},
            )
            contour_points = target_readback.get("contour_points")
            require(isinstance(contour_points, list) and len(contour_points) >= 3, "automatic target contour is unavailable")
            refined_target = tool_value(
                client,
                "reference_mask_refine_prepare",
                {
                    "project_id": project_id,
                    "base_target_sha256": target_sha256,
                    "contour_points": contour_points,
                    "landmarks": target_landmarks,
                    "parts": [
                        {
                            "part_id": "chest-shell",
                            "start_index": 0,
                            "end_index": len(contour_points) - 1,
                            "visibility": "observed",
                            "region": part_region,
                        }
                    ],
                },
            )
            residual_target_sha256 = refined_target.get("target_sha256")
            require(
                isinstance(residual_target_sha256, str) and len(residual_target_sha256) == 64,
                "Part-bound silhouette target was not hash-bound",
            )
            refined_readback = tool_value(
                client,
                "silhouette_target_get",
                {"target_sha256": residual_target_sha256},
            )
            refined_parts = refined_readback.get("parts")
            require(
                isinstance(refined_parts, list)
                and len(refined_parts) == 1
                and refined_parts[0].get("part_id") == "chest-shell"
                and refined_parts[0].get("region") == part_region,
                "refined target did not preserve the image-derived chest ROI binding",
            )
            target_sha256 = residual_target_sha256

            width, height = png_dimensions(reference_bytes)
            comparison = tool_value(
                client,
                "reference_compare_prepare",
                {
                    "project_id": project_id,
                    "candidate_id": candidate_id,
                    "reference_id": reference_id,
                    "view_spec": view_spec(
                        reference_id,
                        reference_sha256,
                        width,
                        height,
                        {"landmarks": landmarks, "regions": regions},
                    ),
                },
            )
            comparison_report = comparison.get("comparison_report") or {}
            require(isinstance(comparison_report, dict), "reference comparison report is unavailable")
            comparison_report_sha256 = comparison.get("comparison_report_object_sha256")
            require(
                isinstance(comparison_report_sha256, str) and len(comparison_report_sha256) == 64,
                "reference comparison report hash is unavailable",
            )
            comparison_metrics = comparison_report.get("metrics")
            require(isinstance(comparison_metrics, dict), "reference comparison metrics are unavailable")
            comparison_camera_hash = comparison_report.get("camera_hash")
            comparison_render_set_hash = comparison_report.get("render_set_hash")
            require(
                isinstance(comparison_camera_hash, str)
                and len(comparison_camera_hash) == 64
                and isinstance(comparison_render_set_hash, str)
                and len(comparison_render_set_hash) == 64,
                "reference comparison camera/render binding is incomplete",
            )
            critic = tool_value(
                client,
                "critic_report_get",
                {
                    "project_id": project_id,
                    "candidate_id": candidate_id,
                    "target_sha256": target_sha256,
                },
            )
            critic_report_sha256 = critic.get("canonical_sha256")
            require(
                isinstance(critic_report_sha256, str) and len(critic_report_sha256) == 64,
                "Runtime critic projection hash is unavailable",
            )
            visual_surface = critic.get("visual_surface")
            require(isinstance(visual_surface, dict), "Runtime critic visual-surface projection is unavailable")
            visual_surface_sha256 = visual_surface.get("surface_signal_canonical_sha256")
            require(
                visual_surface.get("surface_signal_status") == "ready"
                and isinstance(visual_surface_sha256, str)
                and len(visual_surface_sha256) == 64,
                "Runtime critic surface-signal hash is unavailable",
            )
            part_error = tool_value(
                client,
                "silhouette_part_error_get",
                {
                    "project_id": project_id,
                    "candidate_id": candidate_id,
                    "target_sha256": target_sha256,
                },
            )
            part_error_sha256 = part_error.get("canonical_sha256")
            require(
                isinstance(part_error_sha256, str) and len(part_error_sha256) == 64,
                "Runtime Part-error projection hash is unavailable",
            )
            require(
                any(
                    row.get("part_id") == "chest-shell"
                    for row in part_error.get("parts", [])
                    if isinstance(row, dict)
                ),
                "Part-error projection omitted chest-shell",
            )
            residual = {
                "schema_version": "OptimizationResidual@1",
                "part_id": "chest-shell",
                "node_id": "residual-chest-sphere",
                "operation": "union",
                "parameters": {
                    "shape": "sphere",
                    "radius_m": 0.13,
                    "longitude_segments": 16,
                    "latitude_segments": 8,
                    "position_m": [0.0, 1.98, 0.08],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
                "source_critic_report_sha256": critic_report_sha256,
                "source_part_error_sha256": part_error_sha256,
                "source_visual_surface_sha256": visual_surface_sha256,
                "canonical_sha256": "",
            }
            residual["canonical_sha256"] = canonical_hash(residual)

        observation = tool_value(client, "scene_observe_get", {"project_id": project_id, "candidate_id": candidate_id})
        require(
            observation.get("schema_version") == "AgenticSceneObserveResult@1"
            and observation.get("read_only") is True
            and observation.get("project_id") == project_id
            and observation.get("candidate_id") == candidate_id,
            "Agentic observation was not candidate-bound and read-only",
        )
        observation_sha256 = observation.get("canonical_sha256")
        require(isinstance(observation_sha256, str) and len(observation_sha256) == 64, "Agentic observation hash unavailable")

        camera_result = tool_value(
            client,
            "camera_fit_prepare",
            {"project_id": project_id, "candidate_id": candidate_id, "target_sha256": target_sha256, "camera": None},
        )
        camera = resolve_camera(camera_result)
        camera_hash = camera.get("camera_hash")
        require(isinstance(camera_hash, str) and len(camera_hash) == 64, "camera fit did not return a complete hash-bound camera")
        initial_camera = copy.deepcopy(camera)
        initial_camera_hash = camera_hash
        initial_camera_canonical_sha256 = initial_camera.get("canonical_sha256")
        require(
            isinstance(initial_camera_canonical_sha256, str) and len(initial_camera_canonical_sha256) == 64,
            "camera fit did not return a complete canonical camera hash",
        )

        rig = surface_chest_rig(candidate_id) if args.surface_backed_chest_shell else chest_rig(candidate_id)
        rig_hash = tool_value(
            client,
            "silhouette_rig_hash",
            {
                "schema_version": "SilhouetteRigHashRequest@1",
                "project_id": project_id,
                "candidate_id": candidate_id,
                "rig_draft": {key: value for key, value in rig.items() if key != "canonical_sha256"},
            },
        ).get("canonical_sha256")
        require(isinstance(rig_hash, str) and len(rig_hash) == 64, "Runtime Rig hash unavailable")
        rig["canonical_sha256"] = rig_hash

        # Lock the camera against the same candidate/target/Rig context that
        # the optimizer will use.  The compact ref is intentional: Runtime
        # resolves it to the exact calibration it produced, so this probe
        # cannot accidentally hash a model-rounded copy of the camera.
        camera_ref = {
            "schema_version": "CameraCalibrationRef@1",
            "camera_hash": initial_camera_hash,
            "canonical_sha256": initial_camera_canonical_sha256,
        }
        silhouette_fit_request: dict[str, Any] = {
            "project_id": project_id,
            "candidate_id": candidate_id,
            "target_sha256": target_sha256,
            "rig": rig,
            "base_camera": camera_ref,
            "optimizer": {
                "algorithm": "coordinate_descent",
                "max_iterations": 2,
                "max_evaluations": 24,
                "step_fraction": 0.1,
            },
            "canonical_sha256": "",
        }
        silhouette_fit_request["canonical_sha256"] = canonical_hash(silhouette_fit_request)
        silhouette_fit_intent_sha256 = silhouette_fit_request["canonical_sha256"]
        silhouette_fit_result = tool_value(client, "silhouette_fit_prepare", silhouette_fit_request)
        require(
            silhouette_fit_result.get("schema_version") == "SilhouetteFitResult@1"
            and silhouette_fit_result.get("project_id") == project_id
            and silhouette_fit_result.get("candidate_id") == candidate_id
            and silhouette_fit_result.get("target_sha256") == target_sha256,
            "silhouette fit result was not candidate/target bound",
        )
        silhouette_fit_result_sha256 = silhouette_fit_result.get("canonical_sha256")
        require(
            isinstance(silhouette_fit_result_sha256, str) and len(silhouette_fit_result_sha256) == 64,
            "silhouette fit result hash unavailable",
        )
        camera = resolve_camera(silhouette_fit_result)
        camera_hash = camera.get("camera_hash")
        silhouette_fit_camera_canonical_sha256 = camera.get("canonical_sha256")
        require(
            isinstance(camera_hash, str)
            and len(camera_hash) == 64
            and isinstance(silhouette_fit_camera_canonical_sha256, str)
            and len(silhouette_fit_camera_canonical_sha256) == 64,
            "silhouette fit did not return a complete selected camera",
        )
        # Send only the Runtime-owned compact reference across the next MCP
        # boundary.  The complete calibration is retained locally for the
        # OptimizationIntent, but a Python JSON round-trip can spell f64
        # values differently from Rust and make its canonical_sha256 stale.
        # reference_compare_prepare and the unified objective both resolve
        # this ref against the candidate/target camera cache, so no caller
        # supplied float payload becomes geometry truth.
        fit_camera_ref = {
            "schema_version": "CameraCalibrationRef@1",
            "camera_hash": camera_hash,
            "canonical_sha256": silhouette_fit_camera_canonical_sha256,
        }

        if args.with_boolean_residual:
            # Rebind the comparison after silhouette-fit has selected the
            # optimizer camera. The earlier setup comparison is intentionally
            # retained in CAS, but it must not feed the Critic/PartError/
            # residual chain because it used the pre-fit framing.
            rebound_comparison = tool_value(
                client,
                "reference_compare_prepare",
                {
                    "project_id": project_id,
                    "candidate_id": candidate_id,
                    "reference_id": reference_id,
                    "target_sha256": target_sha256,
                    "camera": fit_camera_ref,
                    "view_spec": view_spec(
                        reference_id,
                        reference_sha256,
                        width,
                        height,
                        {"landmarks": landmarks, "regions": regions},
                    ),
                },
            )
            rebound_report = rebound_comparison.get("comparison_report") or {}
            require(isinstance(rebound_report, dict), "camera-rebound comparison report is unavailable")
            comparison_report_sha256 = rebound_comparison.get("comparison_report_object_sha256")
            comparison_metrics = rebound_report.get("metrics")
            comparison_camera_hash = rebound_report.get("camera_hash")
            comparison_render_set_hash = rebound_report.get("render_set_hash")
            require(
                isinstance(comparison_report_sha256, str)
                and len(comparison_report_sha256) == 64
                and isinstance(comparison_metrics, dict)
                and comparison_camera_hash == camera_hash
                and isinstance(comparison_render_set_hash, str)
                and len(comparison_render_set_hash) == 64,
                "camera-rebound comparison was not bound to the silhouette-fit camera",
            )
            critic = tool_value(
                client,
                "critic_report_get",
                {
                    "project_id": project_id,
                    "candidate_id": candidate_id,
                    "target_sha256": target_sha256,
                },
            )
            critic_report_sha256 = critic.get("canonical_sha256")
            visual_surface = critic.get("visual_surface")
            visual_surface_sha256 = (
                visual_surface.get("surface_signal_canonical_sha256")
                if isinstance(visual_surface, dict)
                else None
            )
            require(
                isinstance(critic_report_sha256, str)
                and len(critic_report_sha256) == 64
                and isinstance(visual_surface, dict)
                and visual_surface.get("surface_signal_status") == "ready"
                and isinstance(visual_surface_sha256, str)
                and len(visual_surface_sha256) == 64,
                "camera-rebound Critic surface signal is unavailable",
            )
            part_error = tool_value(
                client,
                "silhouette_part_error_get",
                {
                    "project_id": project_id,
                    "candidate_id": candidate_id,
                    "target_sha256": target_sha256,
                },
            )
            part_error_sha256 = part_error.get("canonical_sha256")
            require(
                isinstance(part_error_sha256, str)
                and len(part_error_sha256) == 64
                and any(
                    row.get("part_id") == "chest-shell"
                    for row in part_error.get("parts", [])
                    if isinstance(row, dict)
                ),
                "camera-rebound PartError evidence is unavailable",
            )
            require(isinstance(residual, dict), "camera-rebound residual is unavailable")
            residual["source_critic_report_sha256"] = critic_report_sha256
            residual["source_part_error_sha256"] = part_error_sha256
            residual["source_visual_surface_sha256"] = visual_surface_sha256
            residual["canonical_sha256"] = ""
            residual["canonical_sha256"] = canonical_hash(residual)

        if args.with_boolean_residual:
            require(isinstance(part_error_sha256, str), "unified objective requires PartError evidence")
            objective_prepared = tool_value(
                client,
                "silhouette_evaluation_objective_prepare",
                {
                    "project_id": project_id,
                    "baseline_candidate_id": candidate_id,
                    "global_target_sha256": global_target_sha256,
                    "part_target_sha256": target_sha256,
                    "part_id": "chest-shell",
                    "source_part_error_sha256": part_error_sha256,
                    "camera": fit_camera_ref,
                },
            )
            require(
                objective_prepared.get("schema_version") == "SilhouetteEvaluationObjectivePrepareResult@1",
                "unified evaluation objective prepare schema mismatch",
            )
            evaluation_objective_sha256 = objective_prepared.get("objective_sha256")
            require(
                isinstance(evaluation_objective_sha256, str) and len(evaluation_objective_sha256) == 64,
                "unified evaluation objective hash was unavailable",
            )

        intent: dict[str, Any] = {
            "schema_version": "OptimizationIntent@1",
            "intent_id": "mcp010f-real-reference-cadfit-intent",
            "job_id": "mcp010f-real-reference-cadfit-job",
            "project_id": project_id,
            "candidate_id": candidate_id,
            "reference_id": reference_id,
            "reference_sha256": reference_sha256,
            "program_sha256": program_sha256,
            "target_sha256": global_target_sha256,
            "camera": camera,
            "camera_hash": camera_hash,
            "part_id": "chest-shell",
            "stage": "primary-form",
            "rig": rig,
            "fidelity": {
                "coarse_resolution": 128,
                "mid_resolution": 256,
                "final_resolution": 512,
                "coarse_evaluations": COARSE_EVALUATIONS,
                "mid_top_k": MID_TOP_K,
                "final_top_k": FINAL_TOP_K,
            },
            "budget": {
                "max_evaluations": 42,
                "max_runtime_ms": 120000,
                "max_output_triangles": 250000,
                "max_worker_memory_bytes": 536870912,
            },
            "objective": {
                "silhouette_iou": 0.35,
                "boundary_f1_4px": 0.30,
                "landmark_coverage": 0.10,
                "landmark_nme": 0.10,
                "part_region": 0.10,
                "program_complexity": 0.05,
            },
            "canonical_sha256": "",
        }
        if evaluation_objective_sha256 is not None:
            intent["evaluation_objective_sha256"] = evaluation_objective_sha256
        if residual is not None:
            intent["residual"] = residual
        intent["canonical_sha256"] = canonical_hash(intent)
        approval = {
            "approved": True,
            "approval_receipt_id": "mcp010f-real-reference-cadfit-approval",
            "approval_summary": "Run isolated one-Part CADFit search and return a proposal only",
            "approval_expires_at": "9999999999",
            "approval_session_id": "mcp010f-real-reference-cadfit-session",
            "idempotency_key": "mcp010f-real-reference-cadfit-idempotency",
        }
        initial = tool_value(
            client,
            "optimization_job_prepare",
            {"project_id": project_id, "candidate_id": candidate_id, "intent": intent, **approval},
        )
        require(initial.get("schema_version") == "OptimizationJobResult@1", "real CADFit prepare schema mismatch")
        latest = initial
        deadline = time.monotonic() + args.timeout
        while time.monotonic() < deadline:
            job = latest.get("job") or {}
            if job.get("status") in {"succeeded", "failed", "cancelled"}:
                break
            time.sleep(0.25)
            latest = tool_value(
                client,
                "optimization_job_get",
                {"project_id": project_id, "candidate_id": candidate_id, "job_id": intent["job_id"]},
            )
        job = latest.get("job") or {}
        result = latest.get("result") or {}
        require(job.get("status") == "succeeded", f"real CADFit job did not succeed: {job.get('status')}")
        require(result.get("status") == "succeeded", "real CADFit result was not succeeded")
        require(
            result.get("search_strategy") == "seed-then-adaptive-trust-region-v5-surface-control-groups-final-top-k-plus-baseline",
            "real CADFit result did not use the residual-family proposal lane",
        )
        require(result.get("evaluations_count") == EXPECTED_EVALUATIONS, "real CADFit evaluation count was incomplete")
        require(
            result.get("fidelity_counts") == {"coarse": COARSE_EVALUATIONS, "mid": MID_TOP_K, "final": FINAL_TOP_K + 1},
            "real CADFit fidelity counts drifted",
        )
        evaluation_hashes = result.get("evaluation_object_sha256s")
        require(
            isinstance(evaluation_hashes, list)
            and len(evaluation_hashes) == EXPECTED_EVALUATIONS
            and all(isinstance(value, str) and len(value) == 64 for value in evaluation_hashes),
            "real CADFit checkpoint chain was incomplete",
        )
        require(result.get("next_stage") == "done" and result.get("checkpoint_sequence") == EXPECTED_EVALUATIONS, "real CADFit checkpoint did not finish")
        require(result.get("best_evaluation_fidelity") == "final", "real CADFit best-so-far escaped the highest completed fidelity")
        require(result.get("proposal_status") in {"proposed", "blocked-no-improvement"}, "real CADFit proposal boundary was invalid")
        require(result.get("strict_improvement") is False or result.get("proposal_status") == "proposed", "real CADFit improvement was not tied to proposal status")
        if evaluation_objective_sha256 is not None:
            require(
                result.get("evaluation_objective_sha256") == evaluation_objective_sha256
                and result.get("promotion_policy") == "silhouette-evaluation-objective-v1"
                and result.get("promotion_status") in {"ready", "blocked_global_or_part_objective"},
                "OptimizationJob did not read back the unified evaluation objective policy",
            )
        if result.get("proposal_status") == "proposed":
            require(isinstance(result.get("proposal_program_object_sha256"), str) and len(result["proposal_program_object_sha256"]) == 64, "real CADFit proposal program hash was not separated from best-so-far")
            require(isinstance(result.get("proposal_artifact_sha256"), str) and len(result["proposal_artifact_sha256"]) == 64, "real CADFit proposal artifact hash was not separated from best-so-far")

        boolean_node_ids: list[str] = []
        baseline_boolean_node_ids: list[str] = []
        boolean_lane_candidate_indices: list[int] = []
        boolean_lane_node_ids: list[str] = []
        if args.with_boolean_residual:
            best_program_sha256 = result.get("best_program_object_sha256")
            best_program = read_cas_json(data_root, best_program_sha256)
            boolean_node_ids = [
                node.get("node_id")
                for node in best_program.get("nodes", [])
                if isinstance(node, dict)
                and node.get("operator_id") == "forgecad.geometry.boolean@1"
                and isinstance(node.get("node_id"), str)
            ]
            require(
                boolean_node_ids in ([], ["residual-chest-sphere-boolean"]),
                "best CADFit program contained an unexpected Boolean node",
            )
            candidate_program_objects = result.get("candidate_program_object_sha256s")
            require(
                isinstance(candidate_program_objects, list)
                and len(candidate_program_objects) == COARSE_EVALUATIONS
                and all(isinstance(value, str) and len(value) == 64 for value in candidate_program_objects),
                "CADFit candidate program lane was not checkpoint-bound",
            )
            baseline_program = read_cas_json(data_root, candidate_program_objects[0])
            baseline_boolean_node_ids = [
                node.get("node_id")
                for node in baseline_program.get("nodes", [])
                if isinstance(node, dict)
                and node.get("operator_id") == "forgecad.geometry.boolean@1"
                and isinstance(node.get("node_id"), str)
            ]
            require(
                baseline_boolean_node_ids == [],
                "CADFit candidate zero was not the unmodified baseline",
            )
            for candidate_index, object_sha256 in enumerate(candidate_program_objects):
                program = read_cas_json(data_root, object_sha256)
                node_ids = [
                    node.get("node_id")
                    for node in program.get("nodes", [])
                    if isinstance(node, dict)
                    and node.get("operator_id") == "forgecad.geometry.boolean@1"
                    and isinstance(node.get("node_id"), str)
                ]
                if node_ids:
                    boolean_lane_candidate_indices.append(candidate_index)
                    boolean_lane_node_ids.extend(node_ids)
            boolean_lane_node_ids = sorted(set(boolean_lane_node_ids))
            require(
                boolean_lane_candidate_indices
                and all(index > 0 for index in boolean_lane_candidate_indices)
                and boolean_lane_node_ids == ["residual-chest-sphere-boolean"],
                "CADFit residual lane did not produce a non-baseline typed Boolean candidate",
            )

        surface_multi_control_point_candidate_indices: list[int] = []
        surface_changed_control_point_counts: dict[str, int] = {}
        if args.surface_backed_chest_shell:
            candidate_program_objects = result.get("candidate_program_object_sha256s")
            require(
                isinstance(candidate_program_objects, list)
                and len(candidate_program_objects) == COARSE_EVALUATIONS
                and all(isinstance(value, str) and len(value) == 64 for value in candidate_program_objects),
                "surface CADFit candidate program lane was not checkpoint-bound",
            )
            baseline_surface_program = read_cas_json(data_root, candidate_program_objects[0])
            baseline_surface_node = next(
                (
                    node
                    for node in baseline_surface_program.get("nodes", [])
                    if isinstance(node, dict) and node.get("operator_id") == "forgecad.geometry.surface-shell@1"
                ),
                None,
            )
            require(isinstance(baseline_surface_node, dict), "surface CADFit baseline node was unavailable")
            baseline_points = baseline_surface_node.get("parameters", {}).get("control_points")
            require(isinstance(baseline_points, list) and len(baseline_points) == 16, "surface CADFit baseline controls were unavailable")
            for candidate_index, object_sha256 in enumerate(candidate_program_objects):
                program = read_cas_json(data_root, object_sha256)
                surface_node = next(
                    (
                        node
                        for node in program.get("nodes", [])
                        if isinstance(node, dict) and node.get("operator_id") == "forgecad.geometry.surface-shell@1"
                    ),
                    None,
                )
                if not isinstance(surface_node, dict):
                    continue
                points = surface_node.get("parameters", {}).get("control_points")
                if not isinstance(points, list) or len(points) != len(baseline_points):
                    continue
                changed = [
                    index
                    for index, (before, after) in enumerate(zip(baseline_points, points))
                    if isinstance(before, list)
                    and isinstance(after, list)
                    and len(before) == 3
                    and len(after) == 3
                    and any(abs(float(left) - float(right)) > 1.0e-9 for left, right in zip(before, after))
                ]
                surface_changed_control_point_counts[str(candidate_index)] = len(changed)
                if len(changed) >= 2:
                    surface_multi_control_point_candidate_indices.append(candidate_index)
            require(
                surface_multi_control_point_candidate_indices,
                "surface CADFit search did not materialize a multi-control-point candidate",
            )

        receipt.update(
            {
                "status": "PASS",
                "search_strategy": result.get("search_strategy"),
                "job_kind": "design_optimization",
                "job_status": job.get("status"),
                "result_status": result.get("status"),
                "evaluations_count": result.get("evaluations_count"),
                "fidelity_counts": result.get("fidelity_counts"),
                "checkpoint_sequence": result.get("checkpoint_sequence"),
                "next_stage": result.get("next_stage"),
                "proposal_status": result.get("proposal_status"),
                "strict_improvement": result.get("strict_improvement"),
                "baseline_loss": result.get("baseline_loss"),
                "best_loss": result.get("best_loss"),
                "best_evaluation_id": result.get("best_evaluation_id"),
                "best_evaluation_fidelity": result.get("best_evaluation_fidelity"),
                "best_program_object_sha256": result.get("best_program_object_sha256"),
                "best_artifact_sha256": result.get("best_artifact_sha256"),
                "proposal_program_object_sha256": result.get("proposal_program_object_sha256"),
                "proposal_artifact_object_sha256": result.get("proposal_artifact_sha256"),
                "evaluation_object_sha256s": evaluation_hashes,
                "project_id": project_id,
                "candidate_id": candidate_id,
                "target_sha256": target_sha256,
                "global_target_sha256": global_target_sha256,
                "evaluation_objective_sha256": evaluation_objective_sha256,
                "promotion_policy": result.get("promotion_policy"),
                "promotion_status": result.get("promotion_status"),
                "camera_hash": camera_hash,
                "camera_source": "silhouette_fit_prepare",
                "camera_binding_status": (
                    "PASS_SILHOUETTE_FIT_TO_OPTIMIZATION"
                    if camera_hash != initial_camera_hash
                    else "PASS_SILHOUETTE_FIT_RETAINED_INITIAL"
                ),
                "initial_camera_hash": initial_camera_hash,
                "initial_camera_canonical_sha256": initial_camera_canonical_sha256,
                "silhouette_fit_intent_sha256": silhouette_fit_intent_sha256,
                "silhouette_fit_result_sha256": silhouette_fit_result_sha256,
                "silhouette_fit_camera_hash": camera_hash,
                "silhouette_fit_camera_canonical_sha256": silhouette_fit_camera_canonical_sha256,
                "silhouette_fit_status": silhouette_fit_result.get("status"),
                "rig_sha256": rig_hash,
                "rig_parameter_count": len(rig.get("parameters", [])),
                "rig_parameter_ids": [item.get("parameter_id") for item in rig.get("parameters", []) if isinstance(item, dict)],
                "surface_backed_chest_shell": args.surface_backed_chest_shell,
                "surface_multi_control_point_candidate_indices": surface_multi_control_point_candidate_indices,
                "surface_changed_control_point_counts": surface_changed_control_point_counts,
                "program_sha256": program_sha256,
                "artifact_sha256": artifact_sha256,
                "agentic_observation_sha256": observation_sha256,
                "ponytail_preflight": preflight,
                "scope": "real reference, chest-shell only, isolated Runtime/MCP/Worker process",
                "quality_claim": "NO_LIKENESS_PASS_CLAIM; CADFIT_PROPOSAL_TRANSPORT_ONLY",
            }
        )
        if args.with_boolean_residual:
            receipt.update(
                {
                    "boolean_residual_mode": True,
                    "boolean_operator": "forgecad.geometry.boolean@1",
                    "boolean_backend": "product-owned-Manifold-C-ABI",
                    "boolean_node_ids": boolean_node_ids,
                    "baseline_boolean_node_ids": baseline_boolean_node_ids,
                    "boolean_lane_candidate_indices": boolean_lane_candidate_indices,
                    "boolean_lane_node_ids": boolean_lane_node_ids,
                    "residual_target_sha256": residual_target_sha256,
                    "evaluation_objective_sha256": evaluation_objective_sha256,
                    "comparison_report_sha256": comparison_report_sha256,
                    "comparison_status": comparison_report.get("status"),
                    "comparison_metrics": comparison_metrics,
                    "comparison_camera_hash": comparison_camera_hash,
                    "comparison_render_set_hash": comparison_render_set_hash,
                    "critic_report_sha256": critic_report_sha256,
                    "surface_signal_status": "ready",
                    "surface_signal_canonical_sha256": visual_surface_sha256,
                    "part_error_sha256": part_error_sha256,
                    "part_target_binding": "PASS_IMAGE_DERIVED_REGION_BOUNDED_REFERENCE_MASK",
                    "part_target_region": part_region,
                    "residual": residual,
                    "quality_claim": "NO_LIKENESS_PASS_CLAIM; MANIFOLD_BOOLEAN_RESIDUAL_AND_CADFIT_GATE_ONLY",
                }
            )
    except (GateFailure, OSError, ProbeFailure, ValueError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        receipt["reason"] = str(error)[:2000]
    finally:
        if client is not None:
            try:
                client.close()
            except BaseException:
                pass
        if ready is not None and runtime is not None:
            try:
                shutdown_runtime(ready, ready_path, runtime)
            except BaseException:
                pass
        if runtime is not None and runtime.poll() is None:
            runtime.terminate()
            try:
                runtime.wait(timeout=5)
            except subprocess.TimeoutExpired:
                runtime.kill()
                runtime.wait(timeout=5)
    write_evidence(args.evidence, receipt)
    print(json.dumps(receipt, ensure_ascii=False, sort_keys=True))
    return 0 if receipt["status"] == "PASS" else 3


if __name__ == "__main__":
    raise SystemExit(main())
