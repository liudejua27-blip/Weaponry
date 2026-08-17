#!/usr/bin/env python3
"""Run one Runtime-owned, hash-bound silhouette Part correction round.

This is an orchestration probe for the F contour loop.  The script prepares
one authored baseline, consolidates its candidate-bound observation, then
submits exactly one typed ``primary_form_repair_prepare`` intent.  Runtime
owns the bounded continuous search, Geometry Worker compilation, strict
readback, Render Worker comparison and same-camera retention decision.  The
probe never edits parameters locally, confirms, exports or writes persistent
user data.
"""

from __future__ import annotations

import argparse
import base64
import copy
import json
import os
import subprocess
import sys
import tempfile
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
    canonical_hash,
    normalize_numeric_representation,
    part_contour_rig_draft,
    view_spec,
)
from probe_mcp010e_raw_stdio import (  # noqa: E402
    png_dimensions,
    robot_detail_program_draft,
    robot_reference_annotations,
)


class ProbeFailure(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ProbeFailure(message)


def read_ponytail_preflight(client: McpClient) -> dict[str, str]:
    """Read the mandatory first-party planning Skill before design calls."""
    result = client.tool(
        "skill_get",
        {"skill_id": "ponytail-preflight", "version": "0.1.0"},
    )
    require(isinstance(result, dict), "ponytail preflight returned no typed result")
    skill = result.get("skill")
    knowledge = result.get("knowledge")
    require(
        isinstance(skill, dict)
        and skill.get("skill_id") == "ponytail-preflight"
        and skill.get("version") == "0.1.0"
        and isinstance(skill.get("canonical_sha256"), str)
        and len(skill["canonical_sha256"]) == 64,
        "ponytail preflight manifest was not verified",
    )
    require(
        isinstance(knowledge, dict)
        and isinstance(knowledge.get("canonical_sha256"), str)
        and len(knowledge["canonical_sha256"]) == 64
        and isinstance(knowledge.get("overview"), str)
        and isinstance(knowledge.get("constraints"), str),
        "ponytail preflight knowledge was not returned",
    )
    return {
        "skill_id": skill["skill_id"],
        "version": skill["version"],
        "skill_manifest_sha256": skill["canonical_sha256"],
        "knowledge_sha256": knowledge["canonical_sha256"],
        "status": "PASS",
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument(
        "--contour-draft",
        type=Path,
        default=Path("docs/evidence/mcp010f/contour-draft-actual-20260812.json"),
    )
    parser.add_argument("--geometry-variant", default="surface-linework")
    parser.add_argument("--part-id", default="chest-shell")
    parser.add_argument(
        "--target-mode",
        choices=("contour", "automatic"),
        default="contour",
        help="Use an explicit Part contour or let Runtime attribute an automatic silhouette boundary through Part-ID evidence.",
    )
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--timeout", type=float, default=90.0)
    return parser.parse_args()


def write_evidence(path: Path | None, value: dict[str, Any]) -> None:
    if path is None:
        return
    root = Path(__file__).resolve().parents[1]
    resolved = path if path.is_absolute() else root / path
    evidence_root = (root / "docs" / "evidence").resolve()
    resolved.resolve().relative_to(evidence_root)
    resolved.parent.mkdir(parents=True, exist_ok=True)
    resolved.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def load_contour(path: Path, part_id: str) -> tuple[list[list[float]], list[dict[str, Any]]]:
    value = json.loads(path.read_text(encoding="utf-8"))
    points = value.get("points") if isinstance(value, dict) else None
    require(isinstance(points, list) and len(points) >= 3, "contour draft has no usable points")
    contour: list[list[float]] = []
    for point in points:
        require(isinstance(point, dict), "contour draft point is not an object")
        x, y = point.get("x"), point.get("y")
        require(isinstance(x, (int, float)) and isinstance(y, (int, float)), "contour point is not numeric")
        require(0.0 <= float(x) <= 1.0 and 0.0 <= float(y) <= 1.0, "contour point is outside normalized image")
        contour.append([float(x), float(y)])
    return contour, [{"part_id": part_id, "start_index": 0, "end_index": len(contour) - 1, "visibility": "observed"}]


def clean_target_landmarks() -> list[dict[str, Any]]:
    landmarks, _ = robot_reference_annotations()
    return [
        {key: item[key] for key in ("landmark_id", "x", "y", "visibility")}
        for item in landmarks
    ]


def build_geometry(client: McpClient, project_id: str, reference_id: str, catalog_hash: str, draft: dict[str, Any]) -> dict[str, Any]:
    hashed = client.tool(
        "geometry_program_hash",
        {"schema_version": "GeometryProgramHashRequest@1", "geometry_program_draft": draft},
    )
    program_hash = hashed.get("canonical_sha256") if isinstance(hashed, dict) else None
    require(isinstance(program_hash, str) and len(program_hash) == 64, "geometry_program_hash did not return a SHA-256")
    geometry = copy.deepcopy(draft)
    geometry["canonical_sha256"] = program_hash
    prepared = client.tool(
        "geometry_prepare",
        {"project_id": project_id, "request": {"typed": "geometry", "reference_id": reference_id, "geometry_program": geometry}},
    )
    candidate = prepared.get("candidate") if isinstance(prepared, dict) else None
    artifact = prepared.get("artifact") if isinstance(prepared, dict) else None
    require(isinstance(candidate, dict) and isinstance(artifact, dict), "geometry_prepare omitted candidate/artifact")
    candidate_id = candidate.get("candidate_id")
    require(isinstance(candidate_id, str) and candidate_id, "geometry_prepare omitted candidate_id")
    return {
        "candidate_id": candidate_id,
        "program_sha256": program_hash,
        "artifact_sha256": artifact.get("object_sha256"),
        "artifact": artifact,
        "geometry": geometry,
        "catalog_sha256": catalog_hash,
    }


def compare_candidate(
    client: McpClient,
    project_id: str,
    reference: dict[str, Any],
    candidate: dict[str, Any],
    width: int,
    height: int,
    camera: dict[str, Any] | None = None,
    target_sha256: str | None = None,
) -> dict[str, Any]:
    landmarks, regions = robot_reference_annotations()
    spec = view_spec(
        reference["reference_id"],
        reference["object_sha256"],
        width,
        height,
        {"landmarks": landmarks, "regions": regions},
    )
    request: dict[str, Any] = {
        "project_id": project_id,
        "candidate_id": candidate["candidate_id"],
        "reference_id": reference["reference_id"],
        "view_spec": spec,
    }
    if camera is not None:
        request["camera"] = camera
    if target_sha256 is not None:
        request["target_sha256"] = target_sha256
    result = client.tool("reference_compare_prepare", request)
    report = result.get("comparison_report") if isinstance(result, dict) else None
    require(isinstance(report, dict), "reference_compare_prepare omitted comparison report")
    metrics = report.get("metrics")
    require(isinstance(metrics, dict), "comparison report omitted metrics")
    return {
        "candidate_id": candidate["candidate_id"],
        "render_set_hash": result.get("render_set_object_sha256"),
        "comparison_report_hash": result.get("comparison_report_object_sha256"),
        "metrics": metrics,
        "status": report.get("status"),
    }


PART_EVIDENCE_ALIASES = {
    "chest-shell": "chest-panel",
    "shoulder-armor-left": "shoulder-armor-left",
    "shoulder-armor-right": "shoulder-armor-right",
    "shin-pair": "shin-left",
}

PART_PARAMETER_PREFIXES = {
    "head-shell": "head",
    "visor": "visor",
    "chest-shell": "chest",
    "chest-vent": "chest-vent",
    "chest-core": "chest-core",
    "neck": "neck",
    "pelvis": "pelvis",
    "shoulder-pair": "shoulder",
    "shoulder-armor-left": "shoulder",
    "shoulder-armor-right": "shoulder",
    "shoulder-armor-pair": "shoulder-armor",
    "upper-arm-pair": "upper-arm",
    "elbow-pair": "elbow",
    "forearm-pair": "forearm",
    "hand-pair": "hand",
    "thigh-pair": "thigh",
    "hip-pair": "hip",
    "shin-pair": "shin",
    "cable-pair": "cable",
    "core-ribs": "core-ribs",
    "amber-sensor": "amber-sensor",
    "visor-edge": "visor-edge",
    "chest-ridge": "chest-ridge",
    "shoulder-trim-pair": "shoulder-trim",
    "forearm-rail-pair": "forearm-rail",
    "hip-flank-pair": "hip-flank",
    "knee-pair": "knee",
    "knee-cap-pair": "knee-cap",
}


def part_parameter_prefix(part_id: str) -> str:
    prefix = PART_PARAMETER_PREFIXES.get(part_id)
    require(isinstance(prefix, str), f"no bounded parameter namespace for Part {part_id}")
    return prefix


def main() -> int:
    args = parse_args()
    source = args.reference.expanduser().resolve()
    require(source.is_file() and not source.is_symlink(), "reference must be a regular file")
    require(args.mcp.is_file() and args.runtime.is_file(), "MCP/Runtime binaries are unavailable")
    require(args.timeout > 0, "timeout must be positive")
    contour: list[list[float]] = []
    parts: list[dict[str, Any]] = []
    if args.target_mode == "contour":
        contour, parts = load_contour(args.contour_draft, args.part_id)
    source_bytes = source.read_bytes()
    width, height = png_dimensions(source_bytes)
    result: dict[str, Any] = {
        "schema_version": "ForgeCADMCP010FPartCorrectionProbe@1",
        "status": "BLOCKED",
        "geometry_variant": args.geometry_variant,
        "part_id": args.part_id,
        "target_mode": args.target_mode,
        "reference_sha256": __import__("hashlib").sha256(source_bytes).hexdigest(),
        "persistent_user_data_touched": False,
    }
    runtime: subprocess.Popen[str] | None = None
    client: McpClient | None = None
    ready: dict[str, Any] | None = None
    preflight: dict[str, str] | None = None
    try:
        mcp_identity = build_identity(args.mcp)
        runtime_identity = build_identity(args.runtime)
        require(
            isinstance(mcp_identity, dict)
            and isinstance(runtime_identity, dict)
            and mcp_identity.get("build_cohort_sha256") == runtime_identity.get("build_cohort_sha256")
            and isinstance(mcp_identity.get("build_cohort_sha256"), str),
            "MCP/Runtime build cohorts do not match",
        )
        result["build_cohort_sha256"] = mcp_identity["build_cohort_sha256"]
        data_root = args.data_root.expanduser().resolve()
        require(not data_root.exists(), "data root must not pre-exist")
        data_root.mkdir(mode=0o700, parents=True)
        (data_root / "ipc").mkdir(mode=0o700)
        ready_path = data_root / "ipc" / "ready.json"
        environment = os.environ.copy()
        for key in ("FORGECAD_RUNTIME_SOCKET", "FORGECAD_RUNTIME_TOKEN", "FORGECAD_RUNTIME_DATA_DIR", "FORGECAD_RUNTIME_COMMAND"):
            environment.pop(key, None)
        environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
        runtime = subprocess.Popen(
            [str(args.runtime), "serve", "--database", str(data_root / "runtime.sqlite"), "--cas-root", str(data_root / "cas"), "--endpoint-dir", str(data_root / "ipc"), "--ready-file", str(ready_path)],
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        ready = wait_for_ready(ready_path, runtime, args.timeout)
        require(isinstance(ready.get("socket_path"), str) and isinstance(ready.get("token"), str), "Runtime handoff is incomplete")
        environment["FORGECAD_RUNTIME_SOCKET"] = ready["socket_path"]
        environment["FORGECAD_RUNTIME_TOKEN"] = ready["token"]
        client = McpClient(args.mcp, environment, args.timeout)
        initialized = client.request("initialize", {"protocolVersion": MCP_PROTOCOL_VERSION, "capabilities": {}, "clientInfo": {"name": "forgecad-mcp010f-part-correction", "version": "1"}})
        require(initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION, "MCP initialize failed")
        client.notify("notifications/initialized")
        preflight = read_ponytail_preflight(client)
        names = {item.get("name") for item in client.request("tools/list").get("result", {}).get("tools", []) if isinstance(item, dict)}
        required = {
            "project_create",
            "reference_import",
            "operator_catalog_get",
            "geometry_program_hash",
            "geometry_prepare",
            "reference_mask_prepare",
            "reference_mask_refine_prepare",
            "camera_fit_prepare",
            "reference_compare_prepare",
            "scene_observe_get",
            "silhouette_rig_hash",
            "primary_form_repair_job_prepare",
            "job_get",
            "job_result_get",
        }
        require(required.issubset(names), "required contour tools are unavailable")
        project = client.tool("project_create", {"name": "MCP010F single Part contour correction", "policy": {"profile": "mvp"}})
        project_id = project.get("project_id")
        require(isinstance(project_id, str) and project_id, "project_create omitted project_id")
        reference = client.tool("reference_import", {"project_id": project_id, "source": {"kind": "inline_content", "mime": "image/png", "content_base64": base64.b64encode(source_bytes).decode("ascii")}, "authorization": {"user_authorized": True, "declaration": "The user supplied and authorized this reference for local ForgeCAD modeling."}}).get("reference")
        require(isinstance(reference, dict) and isinstance(reference.get("reference_id"), str), "reference_import omitted evidence")
        catalog = client.tool("operator_catalog_get")
        catalog_hash = catalog.get("canonical_sha256")
        require(isinstance(catalog_hash, str) and len(catalog_hash) == 64, "operator catalog hash is unavailable")
        draft = robot_detail_program_draft(project_id, catalog_hash, args.geometry_variant, "armor-shell-zones")
        automatic_target = client.tool("reference_mask_prepare", {"project_id": project_id, "reference_id": reference["reference_id"]})
        automatic_target_sha = automatic_target.get("target_sha256")
        require(isinstance(automatic_target_sha, str) and len(automatic_target_sha) == 64, "automatic target is unavailable")
        if args.target_mode == "contour":
            refined_target = client.tool("reference_mask_refine_prepare", {"project_id": project_id, "base_target_sha256": automatic_target_sha, "contour_points": contour, "landmarks": clean_target_landmarks(), "parts": parts})
            refined_target_sha = refined_target.get("target_sha256")
            require(isinstance(refined_target_sha, str) and len(refined_target_sha) == 64, "refined Part target is unavailable")
        else:
            refined_target_sha = automatic_target_sha
        baseline = build_geometry(client, project_id, reference["reference_id"], catalog_hash, draft)
        camera_fit = client.tool(
            "camera_fit_prepare",
            {
                "project_id": project_id,
                "candidate_id": baseline["candidate_id"],
                "target_sha256": refined_target_sha,
                "camera": None,
            },
        )
        require(
            isinstance(camera_fit, dict)
            and camera_fit.get("schema_version") == "CameraFitResult@1"
            and camera_fit.get("candidate_id") == baseline["candidate_id"]
            and camera_fit.get("target_sha256") == refined_target_sha,
            "camera_fit_prepare returned unbound evidence",
        )
        selected_camera = camera_fit.get("selected_camera")
        require(isinstance(selected_camera, dict), "camera_fit_prepare omitted selected camera")
        camera_hash = selected_camera.get("camera_hash")
        camera_canonical = selected_camera.get("canonical_sha256")
        require(
            isinstance(camera_hash, str)
            and len(camera_hash) == 64
            and isinstance(camera_canonical, str)
            and len(camera_canonical) == 64,
            "camera_fit_prepare returned invalid camera hashes",
        )
        camera_ref = {
            "schema_version": "CameraCalibrationRef@1",
            "camera_hash": camera_hash,
            "canonical_sha256": camera_canonical,
        }
        baseline_compare = compare_candidate(
            client,
            project_id,
            reference,
            baseline,
            width,
            height,
            camera=selected_camera,
            target_sha256=refined_target_sha,
        )
        observation = client.tool(
            "scene_observe_get",
            {"project_id": project_id, "candidate_id": baseline["candidate_id"]},
        )
        require(
            isinstance(observation, dict)
            and observation.get("schema_version") == "AgenticSceneObserveResult@1"
            and observation.get("read_only") is True
            and observation.get("project_id") == project_id
            and observation.get("candidate_id") == baseline["candidate_id"]
            and isinstance(observation.get("canonical_sha256"), str)
            and len(observation["canonical_sha256"]) == 64,
            "scene_observe_get returned an unbound canonical observation",
        )

        # The Part Rig is an intent only.  Runtime returns its canonical hash;
        # the probe never derives or mutates a geometry parameter locally.
        part_parameter_prefix(args.part_id)
        rig = part_contour_rig_draft(baseline["candidate_id"], args.part_id)
        rig_hash_result = client.tool(
            "silhouette_rig_hash",
            {
                "schema_version": "SilhouetteRigHashRequest@1",
                "project_id": project_id,
                "candidate_id": baseline["candidate_id"],
                "rig_draft": rig,
            },
        )
        rig_sha256 = rig_hash_result.get("canonical_sha256") if isinstance(rig_hash_result, dict) else None
        require(
            isinstance(rig_sha256, str) and len(rig_sha256) == 64,
            "silhouette_rig_hash did not return a Runtime-owned SHA-256",
        )
        rig["canonical_sha256"] = rig_sha256
        repair_request: dict[str, Any] = {
            "project_id": project_id,
            "candidate_id": baseline["candidate_id"],
            "target_sha256": refined_target_sha,
            "part_id": args.part_id,
            "rig": rig,
            "base_camera": camera_ref,
            "optimizer": {
                "algorithm": "coordinate_descent",
                "max_iterations": 2,
                "max_evaluations": 64,
                "step_fraction": 0.1,
            },
            "base_version_id": None,
            "canonical_sha256": "",
        }
        repair_request["canonical_sha256"] = canonical_hash(
            normalize_numeric_representation(repair_request)
        )
        repair_job = client.tool("primary_form_repair_job_prepare", repair_request)
        require(
            isinstance(repair_job, dict)
            and isinstance(repair_job.get("job_id"), str)
            and repair_job.get("status") in {"queued", "running"},
            "primary_form_repair_job_prepare did not return a queued Runtime job",
        )
        job_id = repair_job["job_id"]
        deadline = time.monotonic() + args.timeout
        terminal_job = repair_job
        while terminal_job.get("status") in {"queued", "running"}:
            if time.monotonic() >= deadline:
                raise ProbeFailure("Primary Form job did not reach a terminal state within the bounded probe window")
            time.sleep(0.25)
            terminal_job = client.tool("job_get", {"job_id": job_id})
            require(isinstance(terminal_job, dict), "job_get returned no typed Runtime job")
        require(
            terminal_job.get("status") == "succeeded",
            f"Primary Form job failed: {terminal_job.get('error_code') or terminal_job.get('status')}",
        )
        job_result = client.tool("job_result_get", {"job_id": job_id})
        require(
            isinstance(job_result, dict)
            and isinstance(job_result.get("result"), dict),
            "job_result_get omitted the Primary Form result",
        )
        repair = job_result["result"]
        require(
            isinstance(repair, dict)
            and repair.get("schema_version") == "PrimaryFormRepairPrepareResult@1"
            and repair.get("project_id") == project_id
            and repair.get("source_candidate_id") == baseline["candidate_id"]
            and repair.get("target_sha256") == refined_target_sha
            and repair.get("part_id") == args.part_id
            and repair.get("version_created") is False
            and repair.get("status") in {"prepared", "no_improvement"},
            "primary_form_repair_prepare returned an invalid bound result",
        )
        fit_result = repair.get("fit_result")
        require(isinstance(fit_result, dict), "Primary Form result omitted fit_result")
        repair_camera = fit_result.get("selected_camera")
        require(isinstance(repair_camera, dict), "Primary Form result omitted selected camera")
        runtime_comparison: dict[str, Any] | None = None
        prepared = repair.get("prepared_candidate")
        visual_evidence = repair.get("visual_evidence")
        if repair.get("status") == "prepared":
            require(isinstance(prepared, dict) and isinstance(visual_evidence, dict), "prepared repair omitted staged evidence")
            prepared_candidate = prepared.get("candidate")
            require(isinstance(prepared_candidate, dict), "prepared repair omitted staged candidate")
            prepared_candidate_id = prepared_candidate.get("candidate_id")
            require(isinstance(prepared_candidate_id, str) and prepared_candidate_id, "prepared repair omitted candidate id")
            require(visual_evidence.get("candidate_id") == prepared_candidate_id, "staged visual evidence drifted from candidate")
            comparison_report = visual_evidence.get("comparison_report")
            require(isinstance(comparison_report, dict), "prepared repair omitted comparison report")
            runtime_comparison = {
                "candidate_id": prepared_candidate_id,
                "render_set_hash": visual_evidence.get("render_set_hash"),
                "comparison_report_hash": visual_evidence.get("comparison_report_hash"),
                "metrics": comparison_report.get("metrics"),
                "status": comparison_report.get("status"),
                "source": "Runtime.primary_form_repair_prepare",
            }
        else:
            require(prepared is None and visual_evidence is None, "no-improvement repair staged a candidate")
        comparisons = [baseline_compare]
        if runtime_comparison is not None:
            comparisons.append(runtime_comparison)
        staged_candidate_id = runtime_comparison.get("candidate_id") if runtime_comparison else None
        result.update({
            "status": "PASS_TRANSPORT_WITH_METRICS",
            "ponytail_preflight": preflight,
            "build_cohort_sha256": result["build_cohort_sha256"],
            "project_id": project_id,
            "reference_id": reference["reference_id"],
            "catalog_sha256": catalog_hash,
            "automatic_target_sha256": automatic_target_sha,
            "refined_part_target_sha256": refined_target_sha,
            "camera_fit": {"status": camera_fit.get("status"), "selected_camera_hash": camera_hash, "selected_camera_canonical_sha256": camera_canonical},
            "scene_observation": observation,
            "rig_sha256": rig_sha256,
            "runtime_search_owner": "forgecad-runtime",
            "primary_form_job": {
                "job_id": job_id,
                "status": terminal_job.get("status"),
                "progress": terminal_job.get("progress"),
                "result_sha256": job_result.get("result_sha256"),
            },
            "primary_form_repair": repair,
            "candidate_comparisons": comparisons,
            "candidate_count": len(comparisons),
            "retention": {
                "baseline_candidate_id": baseline["candidate_id"],
                "staged_candidate_id": staged_candidate_id,
                "status": (
                    "RUNTIME_STAGED_CANDIDATE_REQUIRES_USER_APPROVAL"
                    if staged_candidate_id
                    else "BASELINE_RETAINED_NO_RUNTIME_IMPROVEMENT"
                ),
                "runtime_acceptance": repair.get("acceptance"),
                "policy": "Runtime owns priority-ordered bounded search and same-camera retention; staged candidates remain unconfirmed until user approval",
            },
            "quality_claim": "NO_LIKENESS_PASS_CLAIM; RUNTIME_PRIMARY_FORM_TRANSPORT_ONLY",
        })
    except (GateFailure, OSError, ProbeFailure, ValueError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        result["reason"] = str(error)[:2000]
        if runtime is not None and runtime.stderr is not None:
            # The Runtime stays alive while the MCP client reports a typed
            # failure.  Reading stderr before stopping it blocks forever and
            # hides the actual MCP/Runtime error from the diagnostic receipt.
            if runtime.poll() is None:
                runtime.terminate()
                try:
                    runtime.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    runtime.kill()
                    runtime.wait(timeout=5)
            try:
                stderr = runtime.stderr.read()
            except OSError:
                stderr = ""
            if stderr:
                result["runtime_stderr"] = stderr[-2000:]
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
    write_evidence(args.evidence, result)
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0 if result["status"] == "PASS_TRANSPORT_WITH_METRICS" else 3


if __name__ == "__main__":
    raise SystemExit(main())
