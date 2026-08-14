#!/usr/bin/env python3
"""Run one bounded, hash-bound silhouette Part correction round.

This is an orchestration probe for the F contour loop.  Runtime remains the
source of masks, Part-ID evidence, adjustment proposals and comparison
metrics; this script only copies a bounded proposal into a local typed draft
and asks Runtime to compile/read/compare each candidate.  It never confirms,
exports or writes persistent user data.
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
    silhouette_rig_draft,
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
) -> dict[str, Any]:
    landmarks, regions = robot_reference_annotations()
    spec = view_spec(
        reference["reference_id"],
        reference["object_sha256"],
        width,
        height,
        {"landmarks": landmarks, "regions": regions},
    )
    result = client.tool(
        "reference_compare_prepare",
        {
            "project_id": project_id,
            "candidate_id": candidate["candidate_id"],
            "reference_id": reference["reference_id"],
            "view_spec": spec,
        },
    )
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


def non_regressing_single_part_candidate(
    baseline: dict[str, Any], candidate: dict[str, Any]
) -> bool:
    strict_improvement = False
    for metric_name, direction in (
        ("silhouette_iou", "min"),
        ("boundary_f1_4px", "min"),
        ("bbox_edge_error", "max"),
        ("centroid_error", "max"),
    ):
        baseline_value = baseline.get("metrics", {}).get(metric_name)
        candidate_value = candidate.get("metrics", {}).get(metric_name)
        if not isinstance(baseline_value, (int, float)) or not isinstance(candidate_value, (int, float)):
            return False
        if direction == "min":
            if candidate_value + 1e-12 < baseline_value:
                return False
            strict_improvement = strict_improvement or candidate_value > baseline_value + 1e-12
        else:
            if candidate_value - 1e-12 > baseline_value:
                return False
            strict_improvement = strict_improvement or candidate_value < baseline_value - 1e-12
    return strict_improvement


def adjustment_map(result: dict[str, Any]) -> dict[str, float]:
    rows = result.get("adjustments")
    require(isinstance(rows, list), "part_contour_fit_prepare omitted adjustments")
    output: dict[str, float] = {}
    for row in rows:
        if isinstance(row, dict) and isinstance(row.get("parameter_id"), str) and isinstance(row.get("delta"), (int, float)):
            output[row["parameter_id"]] = float(row["delta"])
    return output


PART_NODE_IDS = {
    "chest-shell": "chest-panel",
    "shoulder-armor-left": "shoulder-armor-left",
    "shoulder-armor-right": "shoulder-armor-right",
    "shin-pair": "shin-left",
}

PART_PARAMETER_PREFIXES = {
    "chest-shell": "chest",
    "shoulder-armor-left": "shoulder",
    "shoulder-armor-right": "shoulder",
    "shin-pair": "shin",
}


def part_parameter_prefix(part_id: str) -> str:
    prefix = PART_PARAMETER_PREFIXES.get(part_id)
    require(isinstance(prefix, str), f"no bounded parameter namespace for Part {part_id}")
    return prefix


def apply_part_adjustment(draft: dict[str, Any], part_id: str, parameter_id: str, delta: float) -> dict[str, Any]:
    """Apply only one bounded semantic adjustment to the local typed draft."""
    result = copy.deepcopy(draft)
    nodes = result.get("nodes")
    require(isinstance(nodes, list), "GeometryProgram nodes are missing")
    node_id = PART_NODE_IDS.get(part_id)
    require(isinstance(node_id, str), f"no deterministic patch route for Part {part_id}")
    node = next((item for item in nodes if isinstance(item, dict) and item.get("node_id") == node_id), None)
    require(isinstance(node, dict), f"no deterministic patch route for Part {part_id}")
    parameters = node.get("parameters")
    require(isinstance(parameters, dict), "Part parameters are missing")
    prefix = part_parameter_prefix(part_id)
    semantic = parameter_id.removeprefix(f"{prefix}-")
    shape = parameters.get("shape")
    if shape == "panel":
        size = parameters.get("size_m")
        position = parameters.get("position_m")
        require(isinstance(size, list) and len(size) == 3, "panel size is missing")
        require(isinstance(position, list) and len(position) == 3, "panel position is missing")
        if semantic == "width":
            size[0] = float(size[0]) * (1.0 + max(-0.25, min(0.25, delta)))
        elif semantic == "height":
            size[1] = float(size[1]) * (1.0 + max(-0.25, min(0.25, delta)))
        elif semantic == "offset-x":
            position[0] = float(position[0]) + float(delta) * float(size[0])
        elif semantic == "offset-y":
            position[1] = float(position[1]) + float(delta) * float(size[1])
        else:
            raise ProbeFailure(f"unsupported {part_id} adjustment {parameter_id}")
    elif shape in {"profile-extrude", "profile-loft"}:
        # Keep the patch intentionally conservative.  Profile coordinates are
        # scaled in the visible local plane; depth is never inferred from one
        # image and therefore is not exposed by this probe.
        if semantic == "width":
            factor = 1.0 + max(-0.25, min(0.25, delta))
            if shape == "profile-extrude":
                points = parameters.get("profile")
                require(isinstance(points, list), "profile points are missing")
                for point in points:
                    point[0] = float(point[0]) * factor
            else:
                profiles = parameters.get("profiles")
                require(isinstance(profiles, list), "loft profiles are missing")
                for profile in profiles:
                    for point in profile.get("points", []):
                        point[0] = float(point[0]) * factor
        else:
            raise ProbeFailure(f"unsupported profile adjustment {parameter_id}")
    else:
        raise ProbeFailure(f"unsupported Part shape {shape}")
    return result


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
        required = {"project_create", "reference_import", "operator_catalog_get", "geometry_program_hash", "geometry_prepare", "reference_mask_prepare", "reference_mask_refine_prepare", "reference_compare_prepare", "part_contour_fit_prepare", "silhouette_part_error_get", "silhouette_candidate_compare"}
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
        baseline_compare = compare_candidate(client, project_id, reference, baseline, width, height)
        part_error = None
        if args.target_mode == "contour":
            part_error = client.tool(
                "silhouette_part_error_get",
                {
                    "project_id": project_id,
                    "candidate_id": baseline["candidate_id"],
                    "target_sha256": refined_target_sha,
                },
            )
            require(
                isinstance(part_error, dict)
                and part_error.get("schema_version") == "SilhouettePartErrorResult@1"
                and isinstance(part_error.get("parts"), list)
                and any(
                    row.get("part_id") == args.part_id
                    for row in part_error["parts"]
                    if isinstance(row, dict)
                ),
                "silhouette_part_error_get omitted the selected Part row",
            )
        rig = silhouette_rig_draft(baseline["candidate_id"])
        parameter_prefix = part_parameter_prefix(args.part_id)
        rig["parameters"] = [
            {"parameter_id": f"{parameter_prefix}-width", "part_id": args.part_id, "semantic": "width", "value": 1.0, "min": 0.75, "max": 1.25, "step": 0.04, "unit": "ratio"},
            {"parameter_id": f"{parameter_prefix}-height", "part_id": args.part_id, "semantic": "height", "value": 1.0, "min": 0.75, "max": 1.25, "step": 0.04, "unit": "ratio"},
            {"parameter_id": f"{parameter_prefix}-offset-x", "part_id": args.part_id, "semantic": "offset_x", "value": 0.0, "min": -0.25, "max": 0.25, "step": 0.02, "unit": "ratio"},
            {"parameter_id": f"{parameter_prefix}-offset-y", "part_id": args.part_id, "semantic": "offset_y", "value": 0.0, "min": -0.25, "max": 0.25, "step": 0.02, "unit": "ratio"},
        ]
        rig["canonical_sha256"] = ""
        rig["canonical_sha256"] = canonical_hash(rig)
        part_fit = client.tool("part_contour_fit_prepare", {"project_id": project_id, "candidate_id": baseline["candidate_id"], "target_sha256": refined_target_sha, "part_id": args.part_id, "rig": rig})
        adjustments = adjustment_map(part_fit)
        candidates = [baseline]
        comparisons = [baseline_compare]
        # Use the proposal as a direction, then probe a small local line
        # search.  Trying only the clamped endpoint can overshoot a visible
        # Part; three fractions are still bounded and keep the round cheap.
        priority = {"height": 0, "width": 1, "scale": 2, "offset_x": 3, "offset_y": 4}
        ordered = sorted(
            adjustments.items(),
            key=lambda item: (priority.get(item[0].removeprefix(f"{parameter_prefix}-"), 9), -abs(item[1])),
        )
        proposals: list[tuple[str, float]] = []
        if ordered:
            primary_id, primary_delta = ordered[0]
            for fraction in (0.4, 0.7, 1.0):
                proposals.append((primary_id, primary_delta * fraction))
            for parameter_id, delta in ordered[1:]:
                if abs(delta) >= 1e-9:
                    proposals.append((parameter_id, delta))
                    break
        for parameter_id, delta in proposals[:4]:
            if abs(delta) < 1e-9:
                continue
            patched = apply_part_adjustment(draft, args.part_id, parameter_id, delta)
            candidate = build_geometry(client, project_id, reference["reference_id"], catalog_hash, patched)
            comparison = compare_candidate(client, project_id, reference, candidate, width, height)
            candidates.append(candidate)
            comparisons.append(comparison)
            if len(candidates) >= 5:
                break
        if len(candidates) >= 2:
            winner = client.tool("silhouette_candidate_compare", {"project_id": project_id, "target_sha256": automatic_target_sha, "candidate_ids": [item["candidate_id"] for item in candidates]})
        else:
            winner = None
        result.update({
            "status": "PASS_TRANSPORT_WITH_METRICS",
            "ponytail_preflight": preflight,
            "build_cohort_sha256": result["build_cohort_sha256"],
            "project_id": project_id,
            "reference_id": reference["reference_id"],
            "catalog_sha256": catalog_hash,
            "automatic_target_sha256": automatic_target_sha,
            "refined_part_target_sha256": refined_target_sha,
            "part_fit": {"status": part_fit.get("status"), "metrics": part_fit.get("metrics"), "adjustments": part_fit.get("adjustments")},
            "part_error": part_error,
            "candidate_comparisons": comparisons,
            "winner": winner,
            "candidate_count": len(candidates),
            "retention": {
                "baseline_candidate_id": baseline["candidate_id"],
                "accepted_candidate_id": next(
                    (
                        row["candidate_id"]
                        for row in comparisons[1:]
                        if non_regressing_single_part_candidate(baseline_compare, row)
                    ),
                    baseline["candidate_id"],
                ),
                "status": "CANDIDATE_ACCEPTED" if any(
                    non_regressing_single_part_candidate(baseline_compare, row)
                    for row in comparisons[1:]
                ) else "BASELINE_RETAINED_NO_NON_REGRESSING_SINGLE_PART_CANDIDATE",
                "policy": "preserve authored baseline unless IoU, Boundary F1, bbox edge error, and centroid error all avoid regression",
            },
            "quality_claim": "NO_LIKENESS_PASS_CLAIM; SINGLE_PART_CORRECTION_TRANSPORT_ONLY",
        })
    except (GateFailure, OSError, ProbeFailure, ValueError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        result["reason"] = str(error)[:2000]
        if runtime is not None and runtime.stderr is not None:
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
