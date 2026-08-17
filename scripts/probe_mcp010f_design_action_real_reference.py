#!/usr/bin/env python3
"""Run one real-reference DesignActionRun with a single-Part proposal.

The probe exercises the Runtime-owned ActionRun path through MCP stdio:
preflight -> typed geometry -> fixed reference comparison -> durable session
-> one-Part RepairIntent proposal -> compile -> strict readback -> render /
compare -> quality gate.  The source candidate remains immutable and the
proposal is reviewable only; this probe never confirms, versions, or exports.
"""

from __future__ import annotations

import argparse
import base64
import copy
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
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
from probe_mcp010c_codex_cli import (  # noqa: E402
    bind_reference_canvas_authoring_context,
    normalize_numeric_representation,
    reference_canvas_authoring_context,
    view_spec,
)
from probe_mcp010e_raw_stdio import (  # noqa: E402
    canonical_hash,
    png_dimensions,
    robot_detail_program_draft,
    robot_reference_annotations,
)
from probe_mcp010f_optimization_real_reference import (  # noqa: E402
    COARSE_EVALUATIONS,
    EXPECTED_EVALUATIONS,
    FINAL_TOP_K,
    MID_TOP_K,
    chest_rig,
    resolve_camera,
)
from probe_mcp010f_part_correction import read_ponytail_preflight  # noqa: E402


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
    resolved.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def build_geometry(
    client: McpClient,
    project_id: str,
    reference_id: str,
    catalog_sha256: str,
    draft: dict[str, Any],
) -> dict[str, Any]:
    hashed = tool_value(
        client,
        "geometry_program_hash",
        {
            "schema_version": "GeometryProgramHashRequest@1",
            "geometry_program_draft": draft,
        },
    )
    program_sha256 = hashed.get("canonical_sha256")
    require(isinstance(program_sha256, str) and len(program_sha256) == 64, "geometry hash was unavailable")
    program = copy.deepcopy(draft)
    program["canonical_sha256"] = program_sha256
    prepared = tool_value(
        client,
        "geometry_prepare",
        {
            "project_id": project_id,
            "request": {
                "typed": "geometry",
                "reference_id": reference_id,
                "geometry_program": program,
            },
        },
    )
    candidate = prepared.get("candidate")
    artifact = prepared.get("artifact")
    require(isinstance(candidate, dict) and isinstance(artifact, dict), "geometry prepare omitted candidate/artifact")
    candidate_id = candidate.get("candidate_id")
    candidate_state_sha256 = candidate.get("canonical_sha256")
    artifact_sha256 = artifact.get("object_sha256") or artifact.get("artifact_id")
    require(isinstance(candidate_id, str) and candidate_id, "geometry prepare omitted candidate_id")
    require(isinstance(candidate_state_sha256, str) and len(candidate_state_sha256) == 64, "candidate state hash was unavailable")
    require(isinstance(artifact_sha256, str) and len(artifact_sha256) == 64, "artifact hash was unavailable")
    return {
        "candidate": candidate,
        "candidate_id": candidate_id,
        "candidate_state_sha256": candidate_state_sha256,
        "artifact_sha256": artifact_sha256,
        "program": program,
        "program_sha256": program_sha256,
        "catalog_sha256": catalog_sha256,
    }


def patch_chest_shell(draft: dict[str, Any], factor: float) -> dict[str, Any]:
    result = copy.deepcopy(draft)
    nodes = result.get("nodes")
    require(isinstance(nodes, list), "GeometryProgram nodes were unavailable")
    node = next(
        (item for item in nodes if isinstance(item, dict) and item.get("node_id") == "chest-panel"),
        None,
    )
    require(isinstance(node, dict), "chest-shell source node was unavailable")
    parameters = node.get("parameters")
    require(isinstance(parameters, dict), "chest-shell parameters were unavailable")
    size = parameters.get("size_m")
    require(isinstance(size, list) and len(size) == 3, "chest-shell size was unavailable")
    size[0] = float(size[0]) * factor
    return result


SURFACE_CHEST_CONTROL_POINT_BEFORE = 0.12
SURFACE_CHEST_CONTROL_POINT_AFTER = 0.20


def surface_backed_chest_shell(draft: dict[str, Any]) -> dict[str, Any]:
    """Replace only the authored chest-shell root with a typed surface shell.

    The rest of the robot detail fixture, semantic Part id, material-zone
    binding, camera and comparison contract remain unchanged.  Control points
    are intentionally in the program's world coordinate system because the
    bounded Worker applies position/rotation after evaluating the patch.
    """
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
    control_points = [
        [-0.83, 1.42, -0.02],
        [-0.276666666667, 1.42, 0.12],
        [0.276666666667, 1.42, 0.12],
        [0.83, 1.42, -0.02],
        [-0.83, 1.793333333333, -0.02],
        [-0.276666666667, 1.793333333333, SURFACE_CHEST_CONTROL_POINT_BEFORE],
        [0.276666666667, 1.793333333333, SURFACE_CHEST_CONTROL_POINT_BEFORE],
        [0.83, 1.793333333333, -0.02],
        [-0.83, 2.166666666667, -0.02],
        [-0.276666666667, 2.166666666667, SURFACE_CHEST_CONTROL_POINT_BEFORE],
        [0.276666666667, 2.166666666667, SURFACE_CHEST_CONTROL_POINT_BEFORE],
        [0.83, 2.166666666667, -0.02],
        [-0.83, 2.54, -0.02],
        [-0.276666666667, 2.54, 0.12],
        [0.276666666667, 2.54, 0.12],
        [0.83, 2.54, -0.02],
    ]
    node["node_id"] = new_node_id
    node["operator_id"] = "forgecad.geometry.surface-shell@1"
    node["parameters"] = {
        "shape": "surface-shell",
        "control_points": control_points,
        "u_segments": 8,
        "v_segments": 8,
        "thickness_m": 0.68,
        "position_m": [0.0, 0.0, 0.0],
        "rotation_rad": [0.0, 0.0, 0.0],
    }
    for item in nodes:
        if not isinstance(item, dict):
            continue
        inputs = item.get("inputs")
        if isinstance(inputs, list):
            item["inputs"] = [new_node_id if value == old_node_id else value for value in inputs]
    matched_output = False
    for item in outputs:
        if not isinstance(item, dict):
            continue
        input_node_ids = item.get("input_node_ids")
        if isinstance(input_node_ids, list) and old_node_id in input_node_ids:
            item["input_node_ids"] = [new_node_id if value == old_node_id else value for value in input_node_ids]
            if item.get("part_id") == "chest-shell":
                matched_output = True
    require(matched_output, "chest-shell Part output was not bound to the surface node")
    return result


def normalize_action_input_numbers(value: Any) -> Any:
    """Match Runtime's bounded numeric compatibility digest for ActionRun."""
    if isinstance(value, bool) or value is None:
        return value
    if isinstance(value, (int, float)):
        return round(float(value), 12)
    if isinstance(value, list):
        return [normalize_action_input_numbers(item) for item in value]
    if isinstance(value, dict):
        return {key: normalize_action_input_numbers(item) for key, item in value.items()}
    return value


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--expected-build-cohort")
    parser.add_argument("--timeout", type=float, default=120.0)
    parser.add_argument(
        "--with-optimization",
        action="store_true",
        help="After the Repair proposal, launch one separate ActionRun-bound CADFit child Job.",
    )
    parser.add_argument(
        "--with-orchestrator",
        action="store_true",
        help="After the single action, run one real-reference ordered stage batch and replay its CAS checkpoint.",
    )
    parser.add_argument(
        "--runtime-parameter-patch",
        action="store_true",
        help="Submit only RuntimeParameterPatch@1; Runtime materializes the bound GeometryProgram from action.parameter_changes.",
    )
    parser.add_argument(
        "--runtime-action-auto-patch",
        action="store_true",
        help="Submit only typed action.parameter_changes plus ReferenceViewSpec; Runtime derives the constrained parameter patch without a proposal payload.",
    )
    parser.add_argument(
        "--surface-backed-chest-shell",
        action="store_true",
        help="Use a real surface-shell@1 chest Part and exercise surface-control-points-v1 through RuntimeParameterPatch@1.",
    )
    parser.add_argument(
        "--with-repair-intent-run",
        action="store_true",
        help="After the legacy ActionRun proposal, replay the same CAS-bound RepairIntent through repair_intent_run_prepare.",
    )
    args = parser.parse_args()
    require(
        not (
            (args.runtime_parameter_patch or args.runtime_action_auto_patch)
            and args.with_optimization
        ),
        "Runtime parameter patch probes are intentionally isolated from CADFit extensions",
    )
    require(
        not (args.runtime_parameter_patch and args.with_orchestrator),
        "The caller-owned RuntimeParameterPatch probe is isolated from the stage orchestrator",
    )
    require(
        not args.runtime_parameter_patch or not args.runtime_action_auto_patch,
        "RuntimeParameterPatch and automatic ActionRun patch modes are mutually exclusive",
    )
    require(
        not args.surface_backed_chest_shell
        or args.runtime_parameter_patch
        or args.runtime_action_auto_patch,
        "surface-backed chest probe requires an isolated Runtime parameter patch path",
    )
    require(
        not args.with_repair_intent_run
        or not (args.runtime_parameter_patch or args.runtime_action_auto_patch or args.with_optimization or args.with_orchestrator),
        "repair_intent_run_prepare probe is isolated from parameter-patch, CADFit and stage-batch extensions",
    )

    source = args.reference.expanduser().resolve()
    require(source.is_file() and not source.is_symlink(), "reference must be a regular file")
    require(args.mcp.is_file() and args.runtime.is_file(), "MCP/Runtime binaries are unavailable")
    require(args.timeout > 0, "timeout must be positive")
    data_root = args.data_root.expanduser().resolve()
    require(not data_root.exists(), "isolated ActionRun data root must not pre-exist")
    data_root.mkdir(mode=0o700, parents=True)

    reference_bytes = source.read_bytes()
    reference_sha256 = hashlib.sha256(reference_bytes).hexdigest()
    width, height = png_dimensions(reference_bytes)
    mcp_identity = build_identity(args.mcp)
    runtime_identity = build_identity(args.runtime)
    build_cohort = mcp_identity.get("build_cohort_sha256")
    geometry_worker_binary = shutil.which("forgecad-geometry-worker")
    render_worker_binary = shutil.which("forgecad-render-worker")
    require(geometry_worker_binary is not None, "Geometry Worker binary is unavailable on PATH")
    require(render_worker_binary is not None, "Render Worker binary is unavailable on PATH")
    build_cohorts = {
        "mcp": build_cohort,
        "runtime": runtime_identity.get("build_cohort_sha256"),
        "geometry_worker": build_identity(Path(geometry_worker_binary)).get("build_cohort_sha256"),
        "render_worker": build_identity(Path(render_worker_binary)).get("build_cohort_sha256"),
    }
    if args.expected_build_cohort:
        require(all(value == args.expected_build_cohort for value in build_cohorts.values()), "ActionRun build cohort mismatch")

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
    repair_client: McpClient | None = None
    optimization_client: McpClient | None = None
    ready: dict[str, Any] | None = None
    receipt: dict[str, Any] = {
        "schema_version": "ForgeCADMCP010FDesignActionRealReferenceProbe@1",
        "task_id": "FGC-MCP010F",
        "status": "BLOCKED",
        "reference_sha256": reference_sha256,
        "expected_build_cohort_sha256": args.expected_build_cohort,
        "build_cohorts": build_cohorts,
        "image_bytes_recorded": False,
        "persistent_user_data_touched": False,
        "candidate_confirmed": False,
        "version_count": 0,
        "execution_mode": (
            "runtime-owned-surface-control-point-patch-v1"
            if args.surface_backed_chest_shell
            else (
                "runtime-owned-parameter-patch-v1"
                if args.runtime_parameter_patch
                else (
                    "runtime-owned-auto-parameter-patch-v1"
                    if args.runtime_action_auto_patch
                    else "caller-owned-full-geometry-program-v1"
                )
            )
        ),
        "surface_backed_chest_shell": args.surface_backed_chest_shell,
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
        require(isinstance(socket_path, str) and isinstance(token, str), "Runtime handoff was incomplete")
        environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
        environment["FORGECAD_RUNTIME_TOKEN"] = token

        client = McpClient(args.mcp, environment, max(args.timeout, 30.0))
        initialized = client.request(
            "initialize",
            {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "mcp010f-design-action-real-reference", "version": "1"},
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
            "reference_compare_prepare",
            "scene_observe_get",
            "session_create_or_resume",
            "session_get",
            "design_action_run_prepare",
            "design_action_run_get",
            "candidate_get",
            "version_list",
        }
        if args.with_repair_intent_run:
            required_tools.add("repair_intent_run_prepare")
        if args.with_optimization:
            required_tools.update(
                {
                    "reference_mask_prepare",
                    "camera_fit_prepare",
                    "silhouette_fit_prepare",
                    "silhouette_rig_hash",
                    "optimization_job_get",
                    "design_action_optimization_proposal_prepare",
                }
            )
        if args.with_orchestrator:
            required_tools.add("design_stage_run_prepare")
        require(required_tools.issubset(names), "real-reference ActionRun tools are unavailable")

        project = tool_value(
            client,
            "project_create",
            {"name": "MCP010F real-reference DesignActionRun", "policy": {"profile": "mvp"}},
        )
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
        require(isinstance(catalog_sha256, str) and len(catalog_sha256) == 64, "operator catalog hash was unavailable")
        baseline_draft = robot_detail_program_draft(project_id, catalog_sha256, "surface-linework", "armor-shell-zones")
        if args.surface_backed_chest_shell:
            baseline_draft = surface_backed_chest_shell(baseline_draft)
        baseline = build_geometry(client, project_id, reference_id, catalog_sha256, baseline_draft)

        landmarks, regions = robot_reference_annotations()
        reference_view_spec = view_spec(
            reference_id,
            reference_sha256,
            width,
            height,
            {
                "landmarks": landmarks,
                "regions": regions,
            },
        )
        prefit_target_sha256: str | None = None
        prefit_camera: dict[str, Any] | None = None
        if args.with_optimization:
            target_landmarks = [
                {key: item[key] for key in ("landmark_id", "x", "y", "visibility")}
                for item in landmarks
            ]
            # reference_mask_prepare requires the imported reference id; keep
            # this pre-fit ordering explicit before the session locks camera.
            prefit_target = tool_value(
                client,
                "reference_mask_prepare",
                {
                    "project_id": project_id,
                    "reference_id": reference_id,
                    "landmarks": target_landmarks,
                    "parts": [],
                },
            )
            prefit_target_sha256 = prefit_target.get("target_sha256")
            require(
                isinstance(prefit_target_sha256, str) and len(prefit_target_sha256) == 64,
                "ActionRun pre-fit target was not hash-bound",
            )
            prefit_camera_result = tool_value(
                client,
                "camera_fit_prepare",
                {
                    "project_id": project_id,
                    "candidate_id": baseline["candidate_id"],
                    "target_sha256": prefit_target_sha256,
                    "camera": None,
                },
            )
            prefit_camera = resolve_camera(prefit_camera_result)
            prefit_camera_hash = prefit_camera.get("camera_hash")
            require(
                isinstance(prefit_camera_hash, str) and len(prefit_camera_hash) == 64,
                "ActionRun pre-fit camera was not hash-bound",
            )
            prefit_rig = chest_rig(baseline["candidate_id"])
            prefit_rig_hash = tool_value(
                client,
                "silhouette_rig_hash",
                {
                    "schema_version": "SilhouetteRigHashRequest@1",
                    "project_id": project_id,
                    "candidate_id": baseline["candidate_id"],
                    "rig_draft": {key: value for key, value in prefit_rig.items() if key != "canonical_sha256"},
                },
            ).get("canonical_sha256")
            require(isinstance(prefit_rig_hash, str) and len(prefit_rig_hash) == 64, "ActionRun pre-fit Rig hash was unavailable")
            prefit_rig["canonical_sha256"] = prefit_rig_hash
            prefit_fit_request: dict[str, Any] = {
                "project_id": project_id,
                "candidate_id": baseline["candidate_id"],
                "target_sha256": prefit_target_sha256,
                "rig": prefit_rig,
                "base_camera": {
                    "schema_version": "CameraCalibrationRef@1",
                    "camera_hash": prefit_camera_hash,
                    "canonical_sha256": prefit_camera.get("canonical_sha256"),
                },
                "optimizer": {
                    "algorithm": "coordinate_descent",
                    "max_iterations": 2,
                    "max_evaluations": 24,
                    "step_fraction": 0.1,
                },
                "canonical_sha256": "",
            }
            prefit_fit_request["canonical_sha256"] = canonical_hash(prefit_fit_request)
            prefit_fit_result = tool_value(client, "silhouette_fit_prepare", prefit_fit_request)
            prefit_camera = resolve_camera(prefit_fit_result)
            require(
                isinstance(prefit_camera.get("camera_hash"), str)
                and len(prefit_camera["camera_hash"]) == 64
                and isinstance(prefit_camera.get("canonical_sha256"), str)
                and len(prefit_camera["canonical_sha256"]) == 64,
                "ActionRun pre-fit silhouette camera was not hash-bound",
            )
        baseline_compare_request: dict[str, Any] = {
            "project_id": project_id,
            "candidate_id": baseline["candidate_id"],
            "reference_id": reference_id,
            "view_spec": reference_view_spec,
        }
        if prefit_camera is not None:
            baseline_compare_request["camera"] = {
                "schema_version": "CameraCalibrationRef@1",
                "camera_hash": prefit_camera["camera_hash"],
                "canonical_sha256": prefit_camera["canonical_sha256"],
            }
            baseline_compare_request["target_sha256"] = prefit_target_sha256
        baseline_compare = tool_value(
            client,
            "reference_compare_prepare",
            baseline_compare_request,
        )
        camera = baseline_compare.get("camera")
        require(isinstance(camera, dict), "reference comparison did not return a camera")
        camera_hash = camera.get("camera_hash")
        require(isinstance(camera_hash, str) and len(camera_hash) == 64, "comparison camera was not hash-bound")
        baseline_quality = baseline_compare.get("quality_report") or {}
        baseline_quality_id = baseline_quality.get("quality_report_id")
        baseline_quality_sha256 = baseline_compare.get("quality_report_object_sha256")
        require(isinstance(baseline_quality_id, str) and isinstance(baseline_quality_sha256, str), "baseline quality evidence was incomplete")
        camera_canonical_sha256 = camera.get("canonical_sha256")
        require(
            isinstance(camera_canonical_sha256, str) and len(camera_canonical_sha256) == 64,
            "comparison camera canonical hash was unavailable",
        )

        observation = tool_value(
            client,
            "scene_observe_get",
            {"project_id": project_id, "candidate_id": baseline["candidate_id"]},
        )
        observation_sha256 = observation.get("canonical_sha256")
        require(isinstance(observation_sha256, str) and len(observation_sha256) == 64, "scene observation hash was unavailable")
        # The durable session must bind to a stable Runtime-owned evidence
        # object.  The complete observation projection is a derived read model
        # and may change when the Runtime refreshes lineage; the quality report
        # is the same evidence hash used by the in-process ActionRun fixture.
        session_evidence_sha256 = baseline_quality_sha256

        target_result = tool_value(
            client,
            "reference_mask_prepare",
            {
                "project_id": project_id,
                "reference_id": reference_id,
                "landmarks": [
                    {key: item[key] for key in ("landmark_id", "x", "y", "visibility")}
                    for item in landmarks
                ],
                "parts": [],
                "user_confirmed": False,
            },
        )
        target_sha256 = target_result.get("target_sha256")
        mask_sha256 = target_result.get("mask_sha256")
        require(
            isinstance(target_sha256, str)
            and len(target_sha256) == 64
            and isinstance(mask_sha256, str)
            and len(mask_sha256) == 64,
            "explicit authoring target/mask hashes were unavailable",
        )
        authoring_context = reference_canvas_authoring_context(
            project_id,
            reference_id,
            reference_sha256,
            width,
            height,
            {"landmarks": landmarks, "regions": regions},
        )
        # Keep the actual authoring payload in the same numeric representation
        # used by authoring_canonical_hash.  Otherwise an integral 0.0/1.0 in
        # the wire JSON can hash differently from the normalized value used
        # to populate ReferenceCanvas/DesignSpec canonical fields.
        authoring_context = normalize_numeric_representation(authoring_context)
        bind_reference_canvas_authoring_context(
            authoring_context,
            target_sha256=target_sha256,
            camera_hash=camera_hash,
            camera_canonical_sha256=camera_canonical_sha256,
            evidence_sha256=session_evidence_sha256,
            view_bindings={
                "three-quarter-user-reference": {
                    "target_sha256": target_sha256,
                    "mask_sha256": mask_sha256,
                    "camera_hash": camera_hash,
                    "camera_canonical_sha256": camera_canonical_sha256,
                    "evidence_sha256": session_evidence_sha256,
                }
            },
        )

        session_id = "design-session-real-action-run"
        session_result = tool_value(
            client,
            "session_create_or_resume",
            {
                "session_id": session_id,
                "project_id": project_id,
                "candidate_id": baseline["candidate_id"],
                "idempotency_key": "real-action-session-idempotency",
                "reference_id": reference_id,
                "design_spec_id": "design-spec-real-codex",
                "reference_canvas_id": "reference-canvas-real-codex",
                "camera_hash": camera_hash,
                "evidence_sha256": session_evidence_sha256,
                "approved": True,
                "approval_receipt_id": "real-action-session-approval",
                "approval_summary": "Create an isolated ActionRun design session",
                "approval_expires_at": "2030-01-01T00:00:00Z",
                "authoring_context": authoring_context,
            },
        )
        session = session_result.get("session") or {}
        require(session.get("session_id") == session_id, "session did not bind the requested id")
        session_observation_sha256 = session.get("observation_sha256")
        require(
            isinstance(session_observation_sha256, str) and len(session_observation_sha256) == 64,
            "durable session observation hash was unavailable",
        )
        requested_stage = session.get("current_stage")
        require(requested_stage == "primary-form", f"ActionRun stage was not primary-form: {requested_stage}")
        session_read = tool_value(
            client,
            "session_get",
            {"session_id": session_id, "project_id": project_id, "candidate_id": baseline["candidate_id"]},
        )
        require(session_read.get("session", {}).get("current_stage") == requested_stage, "session readback stage drifted")

        parameter_changes = (
            [
                {
                    "parameter_id": "control-point-5-z",
                    "before": SURFACE_CHEST_CONTROL_POINT_BEFORE,
                    "after": SURFACE_CHEST_CONTROL_POINT_AFTER,
                    "minimum": -10.0,
                    "maximum": 10.0,
                    "unit": "meter",
                }
            ]
            if args.surface_backed_chest_shell
            else [
                {
                    "parameter_id": "chest-width",
                    "before": 1.0,
                    "after": 1.04,
                    "minimum": 0.75,
                    "maximum": 1.25,
                    "unit": "ratio",
                }
            ]
        )
        action = {
            "action_id": (
                "action-real-chest-surface-control-point"
                if args.surface_backed_chest_shell
                else "action-real-chest-width"
            ),
            "action_kind": "primary-form-adjustment",
            "scope_kind": "part",
            "target_id": "chest-shell",
            "operator_id": (
                "forgecad.geometry.surface-shell@1"
                if args.surface_backed_chest_shell
                else "forgecad.geometry.panel@1"
            ),
            "parameter_changes": parameter_changes,
            "bounded": True,
            "description": (
                "Try one bounded visible chest-shell surface control-point correction"
                if args.surface_backed_chest_shell
                else "Try one bounded visible chest-shell width correction"
            ),
        }
        if args.runtime_action_auto_patch:
            proposal = None
        elif args.runtime_parameter_patch:
            proposal = {
                "parameter_patch": {
                    "schema_version": "RuntimeParameterPatch@1",
                    "strategy": (
                        "surface-control-points-v1"
                        if args.surface_backed_chest_shell
                        else "primitive-dimensions-v1"
                    ),
                },
                "view_spec": reference_view_spec,
                "camera": camera,
            }
        else:
            proposed_draft = patch_chest_shell(baseline_draft, 1.04)
            proposed_hash_result = tool_value(
                client,
                "geometry_program_hash",
                {
                    "schema_version": "GeometryProgramHashRequest@1",
                    "geometry_program_draft": proposed_draft,
                },
            )
            proposed_program_sha256 = proposed_hash_result.get("canonical_sha256")
            require(isinstance(proposed_program_sha256, str) and len(proposed_program_sha256) == 64, "proposal program hash was unavailable")
            proposed_program = copy.deepcopy(proposed_draft)
            proposed_program["canonical_sha256"] = proposed_program_sha256

            repair_intent: dict[str, Any] = {
                "schema_version": "RepairIntent@1",
                "intent_id": "repair-intent-real-chest-width",
                "session_id": session_id,
                "project_id": project_id,
                "candidate_id": baseline["candidate_id"],
                "candidate_state_sha256": baseline["candidate_state_sha256"],
                "reference_id": reference_id,
                "reference_sha256": reference_sha256,
                "camera_hash": camera_hash,
                "observation_sha256": session_observation_sha256,
                "source_evidence_sha256": session_evidence_sha256,
                "source_critic_report_id": baseline_quality_id,
                "source_critic_report_sha256": baseline_quality_sha256,
                "stage": requested_stage,
                "scope": {"kind": "part", "part_id": "chest-shell"},
                "action": {
                    "action_kind": "bounded-repair",
                    "kit_id": "forgecad.kit.housing@1",
                    "operator_id": "forgecad.geometry.panel@1",
                    "operation": "adjust-parameter",
                    "parameter_changes": parameter_changes,
                    "bounded": True,
                    "description": "Prepare a bounded chest-shell width repair proposal",
                },
                "precondition": {
                    "failed_gate_id": "visible-view",
                    "quality_status": baseline_quality.get("visual_status", "QUALITY_TARGET_NOT_MET"),
                    "current_candidate_state_sha256": baseline["candidate_state_sha256"],
                    "evidence_sha256": session_evidence_sha256,
                    "status": "failed",
                },
                "recompute": {
                    "steps": ["compile", "readback", "render", "compare"],
                    "must_rebind_reference": True,
                    "must_rebind_camera": True,
                    "confirm_allowed": False,
                },
                "rollback": {
                    "relation": "none",
                    "target_checkpoint_id": None,
                    "target_checkpoint_sha256": None,
                    "target_version_id": None,
                    "target_version_sha256": None,
                    "on_failure": "keep-current",
                    "reason": None,
                },
                "status": "approved",
                "approval_required": True,
                "runtime_write": False,
                "canonical_sha256": "",
            }
            repair_intent["canonical_sha256"] = canonical_hash(repair_intent)
            proposal = {
                "repair_intent": repair_intent,
                "geometry_program": proposed_program,
                "view_spec": reference_view_spec,
                "camera": camera,
            }
        run_id = (
            "design-action-run-real-chest-surface-control-point"
            if args.surface_backed_chest_shell
            else "design-action-run-real-chest-width"
        )
        input_binding = {
            "project_id": project_id,
            "session_id": session_id,
            "candidate_id": baseline["candidate_id"],
            "run_id": run_id,
            "action": action,
            "requested_stage": requested_stage,
            "observation_sha256": session_observation_sha256,
        }
        if proposal is None:
            input_binding["view_spec"] = reference_view_spec
        else:
            input_binding["proposal"] = proposal
        input_sha256 = canonical_hash(normalize_action_input_numbers(input_binding))
        approval = {
            "approved": True,
            "approval_receipt_id": "real-design-action-approval",
            "approval_summary": "Approve one isolated chest-shell proposal and return reviewable evidence only",
            "approval_expires_at": "2030-01-01T00:00:00Z",
            "approval_session_id": session_id,
            "idempotency_key": "real-design-action-idempotency",
        }
        action_request = {
            "project_id": project_id,
            "session_id": session_id,
            "candidate_id": baseline["candidate_id"],
            "run_id": run_id,
            "action": action,
            "input_sha256": input_sha256,
            "requested_stage": requested_stage,
            "observation_sha256": session_observation_sha256,
            **approval,
        }
        if proposal is None:
            action_request["view_spec"] = reference_view_spec
        else:
            action_request["proposal"] = proposal
        action_result = tool_value(client, "design_action_run_prepare", action_request)
        require(action_result.get("schema_version") == "DesignActionRun@1", "ActionRun result schema mismatch")
        require(action_result.get("run_id") == run_id, "ActionRun id binding was lost")
        require(action_result.get("runtime_write") is False, "ActionRun reported Runtime write")
        require(action_result.get("persistent_user_data_touched") is False, "ActionRun touched persistent user data")
        if action_result.get("status") == "completed":
            require(action_result.get("completed_stage") == "evaluate", "ActionRun did not reach evaluate")
            require(all((action_result.get("stage_results") or {}).get(stage, {}).get("status") == "completed" for stage in ("prepare", "compile", "readback", "render", "evaluate")), "ActionRun stage chain was incomplete")
            proposal_summary = action_result.get("proposal")
            require(isinstance(proposal_summary, dict), "ActionRun omitted proposal summary")
            require(proposal_summary.get("candidate_id") != baseline["candidate_id"], "proposal reused the source candidate")
            require(proposal_summary.get("confirm_allowed") is False, "proposal unexpectedly unlocked confirm")
            require(isinstance(proposal_summary.get("intent_sha256"), str), "ActionRun omitted the Runtime-generated or supplied RepairIntent hash")
            require(proposal_summary.get("visual_status") in {"QUALITY_TARGET_NOT_MET", "PARTIAL_VISIBLE_VIEW_PASS", "BLOCKED_REFERENCE_COVERAGE"}, "proposal quality status was invalid")
        else:
            require(
                (
                    args.runtime_parameter_patch
                    or args.runtime_action_auto_patch
                    or args.with_optimization
                    or args.with_orchestrator
                    or args.with_repair_intent_run
                )
                and action_result.get("status") == "blocked",
                f"ActionRun did not complete: {json.dumps(action_result, ensure_ascii=False, sort_keys=True)[:6000]}",
            )
            proposal_summary = None

        repair_intent_run_receipt: dict[str, Any] | None = None
        if args.with_repair_intent_run:
            # The MCP adapter binds a session to the first ActionRun's
            # run_id. Use a second preflighted stdio session for the wrapper
            # so its distinct run_id cannot be mistaken for a cross-scope
            # request; both sessions still share this isolated Runtime.
            repair_client = McpClient(args.mcp, environment, max(args.timeout, 30.0))
            repair_initialized = repair_client.request(
                "initialize",
                {
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "mcp010f-repair-intent-run", "version": "1"},
                },
            )
            require(
                repair_initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION,
                "RepairIntent MCP initialize failed",
            )
            repair_client.notify("notifications/initialized")
            read_ponytail_preflight(repair_client)
            repair_run_client = repair_client
            intent_source = "action-run"
            checkpoint_id: str | None = None
            if (
                action_result.get("status") == "completed"
                and isinstance(proposal_summary, dict)
                and isinstance(proposal, dict)
                and isinstance(proposal.get("repair_intent"), dict)
            ):
                intent_sha256 = proposal_summary.get("intent_sha256")
                intent_object_sha256 = proposal_summary.get("intent_object_sha256")
                require(
                    isinstance(intent_sha256, str)
                    and len(intent_sha256) == 64
                    and isinstance(intent_object_sha256, str)
                    and len(intent_object_sha256) == 64,
                    "ActionRun did not expose both CAS-bound RepairIntent hashes",
                )
                wrapper_action = copy.deepcopy(action)
                wrapper_action["action_id"] = "repair-intent-run-real-chest-width"
                wrapper_action["action_kind"] = "bounded-repair"
                wrapper_proposal = copy.deepcopy(proposal)
                wrapper_proposal.pop("repair_intent", None)
                wrapper_run_id = "repair-intent-run-real-chest-width"
            else:
                # A real reference commonly fails the visual gate before the
                # legacy ActionRun emits a proposal summary. Exercise the new
                # CAS-bound wrapper independently by first persisting the
                # Runtime-owned failed checkpoint and deriving its approved
                # RepairIntent. This still runs the same compile/readback/
                # render/compare path and must remain blocked/review-only.
                intent_source = "checkpoint-restore"
                checkpoint_result = tool_value(
                    client,
                    "checkpoint_prepare",
                    {
                        "session_id": session_id,
                        "project_id": project_id,
                        "candidate_id": baseline["candidate_id"],
                        "visual_state": "fail",
                        "evidence_sha256": session_evidence_sha256,
                        "stage": requested_stage,
                        "checkpoint_type": "stage-fail",
                        "candidate_state_sha256": baseline["candidate_state_sha256"],
                        "artifact_sha256": baseline["artifact_sha256"],
                        "reference_id": reference_id,
                        "reference_sha256": reference_sha256,
                        "camera_hash": camera_hash,
                        "idempotency_key": "real-repair-intent-run-failed-checkpoint",
                        "approved": True,
                        "approval_receipt_id": "real-repair-intent-run-checkpoint-approval",
                        "approval_summary": "Persist the isolated failed visual checkpoint before RepairIntent replay",
                        "approval_expires_at": "2030-01-01T00:00:00Z",
                    },
                )
                checkpoint = checkpoint_result.get("checkpoint")
                require(isinstance(checkpoint, dict), "RepairIntent wrapper checkpoint was unavailable")
                checkpoint_id = checkpoint.get("checkpoint_id")
                checkpoint_sha256 = checkpoint.get("canonical_sha256")
                require(
                    isinstance(checkpoint_id, str)
                    and isinstance(checkpoint_sha256, str)
                    and len(checkpoint_sha256) == 64,
                    "RepairIntent wrapper checkpoint binding was incomplete",
                )
                restore_result = tool_value(
                    client,
                    "checkpoint_restore_prepare",
                    {
                        "checkpoint_id": checkpoint_id,
                        "checkpoint_sha256": checkpoint_sha256,
                        "session_id": session_id,
                        "project_id": project_id,
                        "candidate_id": baseline["candidate_id"],
                        "visual_state": "fail",
                        "idempotency_key": "real-repair-intent-run-checkpoint-restore",
                        "approved": True,
                        "approval_receipt_id": "real-repair-intent-run-restore-approval",
                        "approval_summary": "Prepare the isolated CAS-bound RepairIntent wrapper",
                        "approval_expires_at": "2030-01-01T00:00:00Z",
                    },
                )
                restore_intent = restore_result.get("intent")
                require(isinstance(restore_intent, dict), "checkpoint restore did not return a RepairIntent")
                intent_sha256 = restore_intent.get("canonical_sha256")
                intent_object_sha256 = restore_result.get("intent_object_sha256")
                require(
                    isinstance(intent_sha256, str)
                    and len(intent_sha256) == 64
                    and isinstance(intent_object_sha256, str)
                    and len(intent_object_sha256) == 64,
                    "checkpoint restore did not expose both CAS-bound RepairIntent hashes",
                )
                intent_action = restore_intent.get("action")
                require(isinstance(intent_action, dict), "checkpoint RepairIntent action was unavailable")
                wrapper_action = {
                    "action_id": "repair-intent-run-real-checkpoint-restore",
                    "action_kind": "bounded-repair",
                    "scope_kind": "part",
                    "target_id": "scene",
                    "operator_id": intent_action.get("operator_id", "forgecad.geometry.primitive@2"),
                    "parameter_changes": copy.deepcopy(intent_action.get("parameter_changes")),
                    "bounded": True,
                    "description": "Replay one failed-checkpoint RepairIntent without mutating source state",
                }
                require(
                    isinstance(wrapper_action["parameter_changes"], list)
                    and wrapper_action["parameter_changes"],
                    "checkpoint RepairIntent parameter changes were unavailable",
                )
                wrapper_proposal = {
                    "geometry_program": copy.deepcopy(baseline["program"]),
                    "view_spec": copy.deepcopy(reference_view_spec),
                    "camera": copy.deepcopy(camera),
                }
                wrapper_run_id = "repair-intent-run-real-checkpoint-restore"
            wrapper_input_binding = {
                "project_id": project_id,
                "session_id": session_id,
                "candidate_id": baseline["candidate_id"],
                "run_id": wrapper_run_id,
                "intent_sha256": intent_sha256,
                "intent_object_sha256": intent_object_sha256,
                "observation_sha256": session_observation_sha256,
                "source_evidence_sha256": session_evidence_sha256,
                "reference_sha256": reference_sha256,
                "action": wrapper_action,
                "proposal": wrapper_proposal,
                "requested_stage": requested_stage,
            }
            wrapper_input_sha256 = canonical_hash(
                normalize_action_input_numbers(wrapper_input_binding)
            )
            wrapper_request = {
                **wrapper_input_binding,
                "input_sha256": wrapper_input_sha256,
                "approved": True,
                "approval_receipt_id": "real-repair-intent-run-approval",
                "approval_summary": "Run one CAS-bound bounded RepairIntent and return staged evidence only",
                "approval_expires_at": "2030-01-01T00:00:00Z",
                "approval_session_id": session_id,
                "idempotency_key": "real-repair-intent-run-idempotency",
            }
            repair_intent_run = tool_value(
                repair_run_client,
                "repair_intent_run_prepare",
                wrapper_request,
            )
            require(
                repair_intent_run.get("schema_version") == "RepairIntentRunResult@1"
                and repair_intent_run.get("run_id") == wrapper_run_id
                and repair_intent_run.get("input_sha256") == wrapper_input_sha256
                and repair_intent_run.get("intent_sha256") == intent_sha256
                and repair_intent_run.get("intent_object_sha256") == intent_object_sha256
                and repair_intent_run.get("confirm_allowed") is False
                and repair_intent_run.get("source_candidate_unchanged") is True
                and repair_intent_run.get("active_design_state_mutated") is False
                and repair_intent_run.get("runtime_write") is False
                and repair_intent_run.get("persistent_user_data_touched") is False,
                "repair_intent_run_prepare returned an invalid source-bound result: "
                + json.dumps(repair_intent_run, ensure_ascii=False, sort_keys=True)[:8000],
            )
            nested_action_run = repair_intent_run.get("action_run")
            require(
                isinstance(nested_action_run, dict)
                and nested_action_run.get("schema_version") == "DesignActionRun@1"
                and nested_action_run.get("runtime_write") is False
                and nested_action_run.get("persistent_user_data_touched") is False,
                "repair_intent_run_prepare omitted its bounded ActionRun receipt",
            )
            repair_intent_run_replay = tool_value(
                repair_run_client,
                "repair_intent_run_prepare",
                wrapper_request,
            )
            require(
                repair_intent_run_replay.get("canonical_sha256")
                == repair_intent_run.get("canonical_sha256"),
                "repair_intent_run_prepare replay changed its immutable result receipt",
            )
            source_after_repair_intent_run = tool_value(
                repair_run_client,
                "candidate_get",
                {"candidate_id": baseline["candidate_id"]},
            )
            source_after_repair_intent_run = (
                source_after_repair_intent_run.get("candidate")
                if isinstance(source_after_repair_intent_run.get("candidate"), dict)
                else source_after_repair_intent_run
            )
            require(
                source_after_repair_intent_run.get("canonical_sha256")
                == baseline["candidate_state_sha256"],
                "repair_intent_run_prepare mutated the source candidate",
            )
            repair_intent_run_receipt = {
                "run_id": wrapper_run_id,
                "run_sha256": repair_intent_run.get("canonical_sha256"),
                "replay_sha256": repair_intent_run_replay.get("canonical_sha256"),
                "input_sha256": wrapper_input_sha256,
                "intent_sha256": intent_sha256,
                "intent_object_sha256": intent_object_sha256,
                "action_run_sha256": nested_action_run.get("canonical_sha256"),
                "action_run_status": nested_action_run.get("status"),
                "quality_status": repair_intent_run.get("quality_status"),
                "status": repair_intent_run.get("status"),
                "intent_source": intent_source,
                "checkpoint_id": checkpoint_id,
                "source_candidate_unchanged": True,
                "confirm_allowed": False,
                "persistent_user_data_touched": False,
                "quality_claim": "NO_LIKENESS_PASS_CLAIM; CAS_BOUND_REPAIR_INTENT_RUN_TRANSPORT_ONLY",
            }

        replay = tool_value(client, "design_action_run_prepare", action_request)
        require(replay.get("canonical_sha256") == action_result.get("canonical_sha256"), "ActionRun replay changed its receipt hash")
        readback = tool_value(
            client,
            "design_action_run_get",
            {"project_id": project_id, "session_id": session_id, "candidate_id": baseline["candidate_id"], "run_id": run_id},
        )
        require(readback.get("canonical_sha256") == action_result.get("canonical_sha256"), "ActionRun get did not round-trip the receipt")

        source_after = tool_value(client, "candidate_get", {"candidate_id": baseline["candidate_id"]})
        source_after = source_after.get("candidate") if isinstance(source_after.get("candidate"), dict) else source_after
        require(source_after.get("canonical_sha256") == baseline["candidate_state_sha256"], "source candidate changed during proposal execution")
        versions = client.tool("version_list", {"project_id": project_id})
        require(isinstance(versions, (dict, list)), "version_list did not return JSON")
        if isinstance(versions, list):
            version_values = versions
        else:
            version_values = versions.get("versions") if isinstance(versions.get("versions"), list) else versions.get("items", [])
        require(isinstance(version_values, list) and len(version_values) == 0, "ActionRun unexpectedly created a version")

        orchestrator_receipt: dict[str, Any] | None = None
        if args.with_orchestrator:
            batch_id = "design-stage-batch-real-reference"
            batch_run_id = "design-stage-batch-real-action"
            batch_action = copy.deepcopy(action)
            batch_action["action_id"] = "action-real-stage-batch"
            batch_entry: dict[str, Any] = {
                "run_id": batch_run_id,
                "action": batch_action,
            }
            if args.runtime_action_auto_patch:
                # This is the regression for the newly closed boundary:
                # Stage Batch receives no caller-authored proposal, but must
                # forward the candidate-bound ReferenceViewSpec to its child
                # ActionRun so Runtime can materialize RuntimeParameterPatch.
                batch_entry["view_spec"] = copy.deepcopy(reference_view_spec)
            else:
                batch_entry["proposal"] = copy.deepcopy(proposal)
            batch_actions = [batch_entry]
            batch_input_binding = {
                "project_id": project_id,
                "session_id": session_id,
                "candidate_id": baseline["candidate_id"],
                "batch_id": batch_id,
                "requested_stage": requested_stage,
                "actions": batch_actions,
                "observation_sha256": session_observation_sha256,
            }
            batch_request = {
                **batch_input_binding,
                "input_sha256": canonical_hash(normalize_action_input_numbers(batch_input_binding)),
                "approved": True,
                "approval_receipt_id": "real-stage-batch-approval",
                "approval_summary": "Run one bounded real-reference stage batch and return its checkpoint",
                "approval_expires_at": "2030-01-01T00:00:00Z",
                "approval_session_id": session_id,
                "idempotency_key": "real-stage-batch-idempotency",
            }
            batch_result = tool_value(client, "design_stage_run_prepare", batch_request)
            require(
                batch_result.get("schema_version") == "DesignActionBatchResult@1"
                and batch_result.get("batch_id") == batch_id
                and batch_result.get("job_id") == batch_id
                and batch_result.get("status") == "blocked"
                and batch_result.get("job_status") == "failed"
                and batch_result.get("next_action_index") == 0
                and batch_result.get("runtime_write") is False
                and batch_result.get("persistent_user_data_touched") is False,
                "real-reference stage orchestrator did not stop at the quality gate: "
                + json.dumps(batch_result, ensure_ascii=False, sort_keys=True)[:8000],
            )
            batch_replay = tool_value(client, "design_stage_run_prepare", batch_request)
            require(
                batch_replay.get("canonical_sha256") == batch_result.get("canonical_sha256"),
                "real-reference stage orchestrator replay changed its checkpoint receipt",
            )
            batch_action_runs = batch_result.get("action_runs")
            require(
                isinstance(batch_action_runs, list)
                and len(batch_action_runs) == 1
                and isinstance(batch_action_runs[0], dict),
                "real-reference stage orchestrator omitted its child ActionRun",
            )
            batch_child = batch_action_runs[0]
            if args.runtime_action_auto_patch:
                child_input_binding = {
                    "project_id": project_id,
                    "session_id": session_id,
                    "candidate_id": baseline["candidate_id"],
                    "run_id": batch_run_id,
                    "action": batch_action,
                    "requested_stage": requested_stage,
                    "observation_sha256": session_observation_sha256,
                    "view_spec": reference_view_spec,
                }
                expected_child_input_sha256 = canonical_hash(child_input_binding)
                require(
                    batch_child.get("input_sha256") == expected_child_input_sha256,
                    "stage batch did not bind view_spec into the child ActionRun input hash",
                )
                require(
                    "proposal" not in batch_entry
                    and isinstance(batch_child.get("proposal"), dict)
                    and batch_child["proposal"].get("candidate_id") != baseline["candidate_id"],
                    "stage batch did not materialize a Runtime-owned review candidate from view_spec",
                )
            source_after_batch = tool_value(client, "candidate_get", {"candidate_id": baseline["candidate_id"]})
            source_after_batch = (
                source_after_batch.get("candidate")
                if isinstance(source_after_batch.get("candidate"), dict)
                else source_after_batch
            )
            require(
                source_after_batch.get("canonical_sha256") == baseline["candidate_state_sha256"],
                "real-reference stage orchestrator mutated the source candidate",
            )
            orchestrator_receipt = {
                "batch_id": batch_id,
                "batch_sha256": batch_result.get("canonical_sha256"),
                "replay_sha256": batch_replay.get("canonical_sha256"),
                "job_status": batch_result.get("job_status"),
                "status": batch_result.get("status"),
                "completed_count": batch_result.get("completed_count"),
                "next_action_index": batch_result.get("next_action_index"),
                "execution_mode": batch_result.get("execution_mode"),
                "action_run_sha256": batch_child.get("canonical_sha256"),
                "action_run_input_sha256": batch_child.get("input_sha256"),
                "action_run_status": batch_child.get("status"),
                "action_run_quality_status": batch_child.get("quality_status"),
                "view_spec_forwarded": args.runtime_action_auto_patch,
                "caller_supplied_full_proposal": not args.runtime_action_auto_patch,
                "proposal_candidate_id": (batch_child.get("proposal") or {}).get("candidate_id"),
                "source_candidate_unchanged": True,
                "version_count": len(version_values),
                "quality_claim": "NO_LIKENESS_PASS_CLAIM; REAL_REFERENCE_STAGE_ORCHESTRATOR_GATE_ONLY",
            }

        optimization_receipt: dict[str, Any] | None = None
        if args.with_optimization:
            optimization_client = McpClient(args.mcp, environment, max(args.timeout, 30.0))
            optimization_initialized = optimization_client.request(
                "initialize",
                {
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "mcp010f-action-cadfit-handoff", "version": "1"},
                },
            )
            require(
                optimization_initialized.get("result", {}).get("protocolVersion")
                == MCP_PROTOCOL_VERSION,
                "MCP initialize failed for the ActionRun-bound CADFit session",
            )
            optimization_client.notify("notifications/initialized")
            optimization_preflight = read_ponytail_preflight(optimization_client)
            target_landmarks = [
                {key: item[key] for key in ("landmark_id", "x", "y", "visibility")}
                for item in landmarks
            ]
            target_result = tool_value(
                optimization_client,
                "reference_mask_prepare",
                {
                    "project_id": project_id,
                    "reference_id": reference_id,
                    "landmarks": target_landmarks,
                    "parts": [],
                },
            )
            target_sha256 = target_result.get("target_sha256")
            require(
                isinstance(target_sha256, str) and len(target_sha256) == 64,
                "ActionRun-bound optimization target was not hash-bound",
            )
            optimization_camera_result = tool_value(
                optimization_client,
                "camera_fit_prepare",
                {
                    "project_id": project_id,
                    "candidate_id": baseline["candidate_id"],
                    "target_sha256": target_sha256,
                    "camera": None,
                },
            )
            optimization_camera = resolve_camera(optimization_camera_result)
            optimization_camera_hash = optimization_camera.get("camera_hash")
            require(
                isinstance(optimization_camera_hash, str)
                and len(optimization_camera_hash) == 64,
                "ActionRun-bound optimization camera was not hash-bound",
            )

            rig = chest_rig(baseline["candidate_id"])
            rig_hash = tool_value(
                optimization_client,
                "silhouette_rig_hash",
                {
                    "schema_version": "SilhouetteRigHashRequest@1",
                    "project_id": project_id,
                    "candidate_id": baseline["candidate_id"],
                    "rig_draft": {key: value for key, value in rig.items() if key != "canonical_sha256"},
                },
            ).get("canonical_sha256")
            require(
                isinstance(rig_hash, str) and len(rig_hash) == 64,
                "ActionRun-bound optimization Rig hash was unavailable",
            )
            rig["canonical_sha256"] = rig_hash
            optimization_camera_ref = {
                "schema_version": "CameraCalibrationRef@1",
                "camera_hash": optimization_camera_hash,
                "canonical_sha256": optimization_camera.get("canonical_sha256"),
            }
            silhouette_fit_request: dict[str, Any] = {
                "project_id": project_id,
                "candidate_id": baseline["candidate_id"],
                "target_sha256": target_sha256,
                "rig": rig,
                "base_camera": optimization_camera_ref,
                "optimizer": {
                    "algorithm": "coordinate_descent",
                    "max_iterations": 2,
                    "max_evaluations": 24,
                    "step_fraction": 0.1,
                },
                "canonical_sha256": "",
            }
            silhouette_fit_request["canonical_sha256"] = canonical_hash(silhouette_fit_request)
            silhouette_fit_result = tool_value(
                optimization_client,
                "silhouette_fit_prepare",
                silhouette_fit_request,
            )
            optimization_camera = resolve_camera(silhouette_fit_result)
            optimization_camera_hash = optimization_camera.get("camera_hash")
            require(
                isinstance(optimization_camera_hash, str)
                and len(optimization_camera_hash) == 64
                and isinstance(optimization_camera.get("canonical_sha256"), str)
                and len(optimization_camera["canonical_sha256"]) == 64,
                "ActionRun-bound silhouette fit camera was not hash-bound",
            )
            require(
                optimization_camera.get("schema_version") == "CameraCalibration@1"
                and optimization_camera.get("projection") == "perspective"
                and optimization_camera.get("coordinate_system") == "right-handed-y-up-meter"
                and optimization_camera.get("resolution") == {"width": 512, "height": 512}
                and isinstance(optimization_camera.get("transform"), dict),
                "ActionRun-bound silhouette fit camera fixed fields were invalid: "
                + json.dumps(
                    {
                        "schema_version": optimization_camera.get("schema_version"),
                        "projection": optimization_camera.get("projection"),
                        "coordinate_system": optimization_camera.get("coordinate_system"),
                        "resolution": optimization_camera.get("resolution"),
                        "near_m": optimization_camera.get("near_m"),
                        "far_m": optimization_camera.get("far_m"),
                        "transform_keys": sorted((optimization_camera.get("transform") or {}).keys()),
                    },
                    ensure_ascii=False,
                    sort_keys=True,
                ),
            )

            optimization_run_id = "design-action-run-real-chest-cadfit"
            optimization_job_id = "design-action-cadfit-real-chest-job"
            optimization_intent: dict[str, Any] = {
                "schema_version": "OptimizationIntent@1",
                "intent_id": "design-action-real-chest-cadfit-intent",
                "action_run_id": optimization_run_id,
                "job_id": optimization_job_id,
                "project_id": project_id,
                "candidate_id": baseline["candidate_id"],
                "reference_id": reference_id,
                "reference_sha256": reference_sha256,
                "program_sha256": baseline["program_sha256"],
                "target_sha256": target_sha256,
                "camera": optimization_camera,
                "camera_hash": optimization_camera_hash,
                "part_id": "chest-shell",
                "stage": requested_stage,
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
            optimization_intent["canonical_sha256"] = canonical_hash(optimization_intent)
            optimization_action = {
                "action_id": "action-real-chest-cadfit",
                "action_kind": "primary-form-adjustment",
                "scope_kind": "part",
                "target_id": "chest-shell",
                "operator_id": "forgecad.geometry.panel@1",
                "parameter_changes": parameter_changes,
                "bounded": True,
                "description": "Launch one bounded CADFit search for the chest-shell Part",
            }
            optimization_input_binding = {
                "project_id": project_id,
                "session_id": session_id,
                "candidate_id": baseline["candidate_id"],
                "run_id": optimization_run_id,
                "action": optimization_action,
                "requested_stage": requested_stage,
                "optimization_intent": optimization_intent,
            }
            optimization_input_sha256 = canonical_hash(
                normalize_action_input_numbers(optimization_input_binding)
            )
            optimization_approval = {
                "approved": True,
                "approval_receipt_id": "real-design-action-cadfit-approval",
                "approval_summary": "Approve one isolated ActionRun-bound CADFit search and return a proposal only",
                "approval_expires_at": "2030-01-01T00:00:00Z",
                "approval_session_id": session_id,
                "idempotency_key": "real-design-action-cadfit-idempotency",
            }
            optimization_request = {
                "project_id": project_id,
                "session_id": session_id,
                "candidate_id": baseline["candidate_id"],
                "run_id": optimization_run_id,
                "action": optimization_action,
                "input_sha256": optimization_input_sha256,
                "requested_stage": requested_stage,
                **optimization_approval,
                "optimization_intent": optimization_intent,
            }
            optimization_action_result = tool_value(
                optimization_client,
                "design_action_run_prepare",
                optimization_request,
            )
            require(
                optimization_action_result.get("schema_version") == "DesignActionRun@1"
                and optimization_action_result.get("run_id") == optimization_run_id
                and optimization_action_result.get("status") == "completed"
                and optimization_action_result.get("runtime_write") is False
                and optimization_action_result.get("persistent_user_data_touched") is False,
                "ActionRun-bound CADFit parent did not complete as a review-only run: "
                + json.dumps(optimization_action_result, ensure_ascii=False, sort_keys=True)[:8000],
            )
            child_job_id = optimization_action_result.get("optimization_job_id")
            child_intent_sha256 = optimization_action_result.get("optimization_intent_sha256")
            require(
                isinstance(child_job_id, str) and child_job_id == optimization_job_id,
                "ActionRun did not expose the bound CADFit child job id",
            )
            require(
                isinstance(child_intent_sha256, str) and len(child_intent_sha256) == 64,
                "ActionRun did not expose the bound OptimizationIntent hash",
            )
            optimization_replay = tool_value(
                optimization_client,
                "design_action_run_prepare",
                optimization_request,
            )
            require(
                optimization_replay.get("canonical_sha256")
                == optimization_action_result.get("canonical_sha256"),
                "ActionRun-bound CADFit replay changed the immutable parent receipt",
            )

            latest_job = tool_value(
                optimization_client,
                "optimization_job_get",
                {
                    "project_id": project_id,
                    "candidate_id": baseline["candidate_id"],
                    "job_id": child_job_id,
                },
            )
            deadline = time.monotonic() + args.timeout
            while time.monotonic() < deadline:
                job = latest_job.get("job") or {}
                if job.get("status") in {"succeeded", "failed", "cancelled"}:
                    break
                time.sleep(0.25)
                latest_job = tool_value(
                    optimization_client,
                    "optimization_job_get",
                    {
                        "project_id": project_id,
                        "candidate_id": baseline["candidate_id"],
                        "job_id": child_job_id,
                    },
                )
            child_job = latest_job.get("job") or {}
            child_result = latest_job.get("result") or {}
            require(
                child_job.get("status") == "succeeded"
                and child_result.get("status") == "succeeded",
                f"ActionRun-bound CADFit child did not succeed: {child_job.get('status')}",
            )
            require(
                child_result.get("evaluations_count") == EXPECTED_EVALUATIONS
                and child_result.get("fidelity_counts")
                == {"coarse": COARSE_EVALUATIONS, "mid": MID_TOP_K, "final": FINAL_TOP_K + 1}
                and child_result.get("next_stage") == "done"
                and child_result.get("checkpoint_sequence") == EXPECTED_EVALUATIONS,
                "ActionRun-bound CADFit child did not finish its bounded 32/4/2-plus-baseline search",
            )
            require(
                child_result.get("proposal_status") in {"proposed", "blocked-no-improvement"}
                and child_result.get("best_evaluation_fidelity") == "final",
                "ActionRun-bound CADFit child crossed the proposal boundary incorrectly",
            )
            proposal_view_binding = {
                "project_id": project_id,
                "session_id": session_id,
                "candidate_id": baseline["candidate_id"],
                "run_id": optimization_run_id,
                "job_id": child_job_id,
                "view_spec": reference_view_spec,
                "idempotency_key": "real-design-action-cadfit-proposal-idempotency",
            }
            proposal_request = {
                **proposal_view_binding,
                "input_sha256": canonical_hash(proposal_view_binding),
                "approved": True,
                "approval_receipt_id": "real-design-action-cadfit-proposal-approval",
                "approval_summary": "Materialize only a strict-improvement CADFit proposal for explicit visual review",
                "approval_expires_at": "2030-01-01T00:00:00Z",
                "approval_session_id": session_id,
            }
            optimization_proposal_result = tool_value(
                optimization_client,
                "design_action_optimization_proposal_prepare",
                proposal_request,
            )
            require(
                optimization_proposal_result.get("schema_version")
                == "OptimizationProposalPrepareResult@1"
                and optimization_proposal_result.get("candidate_id")
                == baseline["candidate_id"]
                and optimization_proposal_result.get("run_id") == optimization_run_id
                and optimization_proposal_result.get("confirm_allowed") is False
                and optimization_proposal_result.get("source_candidate_unchanged") is True
                and optimization_proposal_result.get("version_created") is False,
                "ActionRun-bound CADFit proposal continuation was not fail-closed: "
                + json.dumps(optimization_proposal_result, ensure_ascii=False, sort_keys=True)[:8000],
            )
            if child_result.get("proposal_status") == "blocked-no-improvement":
                require(
                    optimization_proposal_result.get("status") == "blocked"
                    and optimization_proposal_result.get("proposal_candidate_id") is None
                    and optimization_proposal_result.get("reason_code")
                    == "optimization-no-strict-improvement",
                    "CADFit no-improvement result unexpectedly materialized a candidate",
                )
            else:
                require(
                    optimization_proposal_result.get("status") == "proposed"
                    and isinstance(optimization_proposal_result.get("proposal_candidate_id"), str)
                    and optimization_proposal_result.get("visual_status")
                    in {"QUALITY_TARGET_NOT_MET", "PARTIAL_VISIBLE_VIEW_PASS"},
                    "CADFit strict proposal did not produce a reviewable candidate",
                )
            optimization_proposal_replay = tool_value(
                optimization_client,
                "design_action_optimization_proposal_prepare",
                proposal_request,
            )
            require(
                optimization_proposal_replay.get("canonical_sha256")
                == optimization_proposal_result.get("canonical_sha256"),
                "ActionRun-bound CADFit proposal continuation was not idempotent",
            )
            optimizer_readback = tool_value(
                optimization_client,
                "design_action_run_get",
                {
                    "project_id": project_id,
                    "session_id": session_id,
                    "candidate_id": baseline["candidate_id"],
                    "run_id": optimization_run_id,
                },
            )
            require(
                optimizer_readback.get("canonical_sha256")
                == optimization_action_result.get("canonical_sha256"),
                "ActionRun-bound CADFit parent did not round-trip from Runtime storage",
            )
            source_after_cadfit = tool_value(
                optimization_client,
                "candidate_get",
                {"candidate_id": baseline["candidate_id"]},
            )
            source_after_cadfit = (
                source_after_cadfit.get("candidate")
                if isinstance(source_after_cadfit.get("candidate"), dict)
                else source_after_cadfit
            )
            require(
                source_after_cadfit.get("canonical_sha256") == baseline["candidate_state_sha256"],
                "ActionRun-bound CADFit mutated the source candidate",
            )
            versions_after_cadfit = optimization_client.tool("version_list", {"project_id": project_id})
            if isinstance(versions_after_cadfit, list):
                version_values_after_cadfit = versions_after_cadfit
            else:
                version_values_after_cadfit = (
                    versions_after_cadfit.get("versions")
                    if isinstance(versions_after_cadfit.get("versions"), list)
                    else versions_after_cadfit.get("items", [])
                )
            require(
                isinstance(version_values_after_cadfit, list) and len(version_values_after_cadfit) == 0,
                "ActionRun-bound CADFit unexpectedly created a version",
            )
            optimization_receipt = {
                "run_id": optimization_run_id,
                "run_sha256": optimization_action_result.get("canonical_sha256"),
                "replay_sha256": optimization_replay.get("canonical_sha256"),
                "optimization_job_id": child_job_id,
                "optimization_intent_sha256": child_intent_sha256,
                "optimization_job_status": child_job.get("status"),
                "optimization_result_status": child_result.get("status"),
                "search_strategy": child_result.get("search_strategy"),
                "evaluations_count": child_result.get("evaluations_count"),
                "fidelity_counts": child_result.get("fidelity_counts"),
                "checkpoint_sequence": child_result.get("checkpoint_sequence"),
                "next_stage": child_result.get("next_stage"),
                "proposal_status": child_result.get("proposal_status"),
                "strict_improvement": child_result.get("strict_improvement"),
                "baseline_loss": child_result.get("baseline_loss"),
                "best_loss": child_result.get("best_loss"),
                "best_evaluation_id": child_result.get("best_evaluation_id"),
                "best_evaluation_fidelity": child_result.get("best_evaluation_fidelity"),
                "optimization_camera_hash": optimization_camera_hash,
                "comparison_camera_hash": camera_hash,
                "camera_binding_status": (
                    "SAME_CAMERA"
                    if optimization_camera_hash == camera_hash
                    else "NOT_SAME_AS_COMPARISON_CAMERA"
                ),
                "evaluation_object_sha256s": child_result.get("evaluation_object_sha256s"),
                "source_candidate_unchanged": True,
                "version_count": len(version_values_after_cadfit),
                "preflight": optimization_preflight,
                "quality_claim": "NO_LIKENESS_PASS_CLAIM; ACTION_RUN_BOUND_CADFIT_HANDOFF_ONLY",
                "proposal_continuation": {
                    "status": optimization_proposal_result.get("status"),
                    "reason_code": optimization_proposal_result.get("reason_code"),
                    "proposal_job_id": optimization_proposal_result.get("proposal_job_id"),
                    "proposal_candidate_id": optimization_proposal_result.get("proposal_candidate_id"),
                    "proposal_candidate_state_sha256": optimization_proposal_result.get("proposal_candidate_state_sha256"),
                    "visual_status": optimization_proposal_result.get("visual_status"),
                    "visual_gate_passed": optimization_proposal_result.get("visual_gate_passed"),
                    "confirm_allowed": optimization_proposal_result.get("confirm_allowed"),
                    "repair_apply_status": optimization_proposal_result.get("repair_apply_status"),
                    "replay_sha256": optimization_proposal_replay.get("canonical_sha256"),
                },
            }

        receipt.update(
            {
                "status": (
                    "PASS_REPAIR_INTENT_RUN_REAL_REFERENCE"
                    if args.with_repair_intent_run
                    else (
                    "PASS_ACTION_RUN_RUNTIME_AUTO_PARAMETER_PATCH_STAGE_BATCH"
                    if args.with_orchestrator and args.runtime_action_auto_patch
                    and action_result.get("status") == "completed"
                    else (
                        "PASS_ACTION_RUN_CADFIT_HANDOFF"
                    if args.with_optimization
                    else (
                        "PASS_ACTION_RUN_RUNTIME_AUTO_PARAMETER_PATCH"
                        if args.runtime_action_auto_patch
                        and action_result.get("status") == "completed"
                        else (
                            "BLOCKED_RUNTIME_AUTO_PARAMETER_PATCH"
                            if args.runtime_action_auto_patch
                            else (
                                "PASS_ACTION_RUN_RUNTIME_PARAMETER_PATCH"
                                if args.runtime_parameter_patch
                                and action_result.get("status") == "completed"
                                else (
                                    "BLOCKED_RUNTIME_PARAMETER_PATCH"
                                    if args.runtime_parameter_patch
                                    else "PASS_ACTION_RUN_REAL_REFERENCE"
                                )
                            )
                        )
                    )
                    )
                    )
                ),
                "build_cohort_sha256": build_cohort,
                "build_cohorts": build_cohorts,
                "project_id": project_id,
                "reference_id": reference_id,
                "catalog_sha256": catalog_sha256,
                "source_candidate_id": baseline["candidate_id"],
                "source_candidate_state_sha256": baseline["candidate_state_sha256"],
                "source_program_sha256": baseline["program_sha256"],
                "source_artifact_sha256": baseline["artifact_sha256"],
                "source_quality_report_id": baseline_quality_id,
                "source_quality_report_sha256": baseline_quality_sha256,
                "source_visual_status": baseline_quality.get("visual_status"),
                "repair_probe_status": action_result.get("status"),
                "repair_probe_gate": ((action_result.get("stage_results") or {}).get("evaluate") or {}).get("reason"),
                "camera_hash": camera_hash,
                "session_id": session_id,
                "session_evidence_sha256": session_evidence_sha256,
                "run_id": run_id,
                "run_sha256": action_result.get("canonical_sha256"),
                "run_status": action_result.get("status"),
                "completed_stage": action_result.get("completed_stage"),
                "stage_results": action_result.get("stage_results"),
                "proposal": proposal_summary,
                "runtime_parameter_patch": (
                    {
                        "schema_version": "RuntimeParameterPatch@1",
                        "strategy": (
                            "surface-control-points-v1"
                            if args.surface_backed_chest_shell
                            else "primitive-dimensions-v1"
                        ),
                        "surface_backed_chest_shell": args.surface_backed_chest_shell,
                        "parameter_ids": [item["parameter_id"] for item in parameter_changes],
                        "action_changes_authoritative": True,
                        "runtime_materialized_geometry_program": proposal_summary is not None,
                        "caller_supplied_full_proposal": proposal is not None,
                        "source_candidate_unchanged": True,
                    }
                    if args.runtime_parameter_patch or args.runtime_action_auto_patch
                    else None
                ),
                "replay_sha256": replay.get("canonical_sha256"),
                "version_count": len(version_values),
                "scope": "real authorized reference, one chest-shell Part, isolated Runtime/MCP/Worker process",
                "quality_claim": "NO_LIKENESS_PASS_CLAIM; ACTION_RUN_PROPOSAL_TRANSPORT_AND_GATE_ONLY",
                "human_review": "NOT_RUN",
                "hq_360": "BLOCKED_REFERENCE_COVERAGE",
                "preflight": preflight,
            }
        )
        if repair_intent_run_receipt is not None:
            receipt["repair_intent_run"] = repair_intent_run_receipt
            receipt["quality_claim"] = "NO_LIKENESS_PASS_CLAIM; CAS_BOUND_REPAIR_INTENT_RUN_TRANSPORT_ONLY"
        if optimization_receipt is not None:
            receipt["optimization"] = optimization_receipt
            receipt["quality_claim"] = "NO_LIKENESS_PASS_CLAIM; ACTION_RUN_BOUND_CADFIT_HANDOFF_ONLY"
        if orchestrator_receipt is not None:
            receipt["orchestrator"] = orchestrator_receipt
    except (GateFailure, OSError, ProbeFailure, ValueError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        detail = str(error)
        if runtime is not None and runtime.poll() is not None and runtime.stderr is not None:
            try:
                stderr = runtime.stderr.read(4000).strip()
            except OSError:
                stderr = ""
            if stderr:
                detail = f"{detail}; runtime_stderr={stderr}"
        receipt["reason"] = detail[:2000]
    finally:
        if optimization_client is not None:
            try:
                optimization_client.close()
            except BaseException:
                pass
        if repair_client is not None:
            try:
                repair_client.close()
            except BaseException:
                pass
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
    return 0 if receipt["status"] in {
        "PASS_REPAIR_INTENT_RUN_REAL_REFERENCE",
        "PASS_ACTION_RUN_REAL_REFERENCE",
        "PASS_ACTION_RUN_CADFIT_HANDOFF",
        "PASS_ACTION_RUN_RUNTIME_PARAMETER_PATCH",
        "PASS_ACTION_RUN_RUNTIME_AUTO_PARAMETER_PATCH",
        "PASS_ACTION_RUN_RUNTIME_AUTO_PARAMETER_PATCH_STAGE_BATCH",
    } else 3


if __name__ == "__main__":
    raise SystemExit(main())
