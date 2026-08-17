#!/usr/bin/env python3
"""Add bounded secondary structure to an existing reference-fitted candidate.

The script is deliberately a prepare-only quality loop: every stage is hashed
by Runtime, compiled by Geometry Worker, strict-read back, and compared with
the same confirmed silhouette target and camera.  It never confirms or exports
a candidate and it stops promoting stages when the numeric silhouette gate
regresses.
"""

from __future__ import annotations

import argparse
import copy
import json
import math
import os
import sys
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_ROOT.parent
sys.path.insert(0, str(SCRIPT_ROOT))

from probe_mcp010b_raw_stdio import MCP_PROTOCOL_VERSION, McpClient  # noqa: E402
from probe_mcp010c_codex_cli import canonical_hash, view_spec  # noqa: E402


class SecondaryStructureFailure(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SecondaryStructureFailure(message)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--project-id", required=True)
    parser.add_argument("--reference-id", required=True)
    parser.add_argument("--reference-sha256", required=True)
    parser.add_argument("--base-candidate-id", required=True)
    parser.add_argument("--base-program-sha256", required=True)
    parser.add_argument("--target-sha256", required=True)
    parser.add_argument("--camera-hash", required=True)
    parser.add_argument("--camera-canonical-sha256", required=True)
    parser.add_argument("--width", type=int, default=1000)
    parser.add_argument("--height", type=int, default=220)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=120.0)
    return parser.parse_args()


def cas_path(data_root: Path, sha256: str) -> Path:
    require(len(sha256) == 64 and all(c in "0123456789abcdef" for c in sha256), "invalid CAS hash")
    return data_root / "cas" / "objects" / sha256[:2] / sha256


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"expected JSON object at {path.name}")
    return value


def initialize_client(args: argparse.Namespace) -> McpClient:
    ready = read_json(args.data_root / "ipc" / "ready.json")
    socket_path = ready.get("socket_path")
    token = ready.get("token")
    require(isinstance(socket_path, str) and Path(socket_path).exists(), "live Runtime socket is unavailable")
    require(isinstance(token, str) and token, "live Runtime token is unavailable")
    environment = os.environ.copy()
    environment["FORGECAD_RUNTIME_SOCKET"] = socket_path
    environment["FORGECAD_RUNTIME_TOKEN"] = token
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    client = McpClient(args.mcp, environment, args.timeout)
    initialized = client.request(
        "initialize",
        {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "forgecad-secondary-structure-loop", "version": "1"},
        },
    )
    require(initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION, "MCP initialize failed")
    client.notify("notifications/initialized")
    return client


def skill_preflight(client: McpClient) -> dict[str, Any]:
    ponytail = client.tool("skill_get", {"skill_id": "ponytail-preflight", "version": "0.1.0"})
    detail = client.tool("skill_get", {"skill_id": "hard-surface-detail", "version": "0.2.0"})
    ponytail_skill = ponytail.get("skill", {})
    detail_skill = detail.get("skill", {})
    require(
        ponytail_skill.get("skill_id") == "ponytail-preflight"
        and ponytail_skill.get("version") == "0.1.0"
        and isinstance(ponytail_skill.get("canonical_sha256"), str),
        "ponytail preflight manifest is invalid",
    )
    require(
        detail_skill.get("skill_id") == "hard-surface-detail"
        and detail_skill.get("version") == "0.2.0"
        and isinstance(detail_skill.get("canonical_sha256"), str),
        "hard-surface detail manifest is invalid",
    )
    return {
        "ponytail_manifest_sha256": ponytail_skill["canonical_sha256"],
        "hard_surface_detail_manifest_sha256": detail_skill["canonical_sha256"],
    }


def camera_ref(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "schema_version": "CameraCalibrationRef@1",
        "camera_hash": args.camera_hash,
        "canonical_sha256": args.camera_canonical_sha256,
    }


def camera_calibration(args: argparse.Namespace) -> dict[str, Any]:
    """Rehydrate the complete Runtime-owned calibration after an MCP restart.

    CameraCalibrationRef is session-scoped in the current alpha Runtime.  The
    complete canonical calibration remains CAS-backed and can be copied
    byte-for-byte across MCP sessions without changing the camera.
    """
    return {
        "schema_version": "CameraCalibration@2",
        "camera_hash": args.camera_hash,
        "projection": "orthographic",
        "transform": {
            "position_m": [0.6, 1.36, 20],
            "target_m": [0.6, 1.36, 0],
            "up": [0, 1, 0],
        },
        "fov_y_degrees": None,
        "ortho_scale": 9.0,
        "near_m": 0.05,
        "far_m": 100,
        "resolution": {"width": 512, "height": 512},
        "coordinate_system": "right-handed-y-up-meter",
        "renderer_revision": "forgecad-renderer-2",
        "canonical_sha256": args.camera_canonical_sha256,
    }


def compare(client: McpClient, args: argparse.Namespace, candidate_id: str) -> dict[str, Any]:
    spec = view_spec(
        args.reference_id,
        args.reference_sha256,
        args.width,
        args.height,
        {"landmarks": [], "regions": []},
        kind="right",
        view_id="weapon-right-confirmed-contour",
    )
    result = client.tool(
        "reference_compare_prepare",
        {
            "project_id": args.project_id,
            "candidate_id": candidate_id,
            "reference_id": args.reference_id,
            "view_spec": spec,
            "camera": camera_calibration(args),
            "target_sha256": args.target_sha256,
        },
    )
    report = result.get("comparison_report")
    require(isinstance(report, dict) and isinstance(report.get("metrics"), dict), "comparison omitted metrics")
    return {
        "candidate_id": candidate_id,
        "render_set_object_sha256": result.get("render_set_object_sha256"),
        "comparison_report_object_sha256": result.get("comparison_report_object_sha256"),
        "quality_report_object_sha256": result.get("quality_report_object_sha256"),
        "status": report.get("status"),
        "metrics": report["metrics"],
        "view_spec_canonical_sha256": spec["canonical_sha256"],
    }


def prepare(client: McpClient, args: argparse.Namespace, draft: dict[str, Any]) -> dict[str, Any]:
    draft.pop("canonical_sha256", None)
    hashed = client.tool(
        "geometry_program_hash",
        {"schema_version": "GeometryProgramHashRequest@1", "geometry_program_draft": draft},
    )
    program_sha = hashed.get("canonical_sha256")
    require(isinstance(program_sha, str) and len(program_sha) == 64, "Runtime did not hash geometry program")
    program = copy.deepcopy(draft)
    program["canonical_sha256"] = program_sha
    prepared = client.tool(
        "geometry_prepare",
        {
            "project_id": args.project_id,
            "request": {
                "typed": "geometry",
                "reference_id": args.reference_id,
                "geometry_program": program,
            },
        },
    )
    candidate = prepared.get("candidate")
    artifact = prepared.get("artifact")
    require(isinstance(candidate, dict) and isinstance(artifact, dict), "geometry prepare omitted candidate/artifact")
    readback = client.tool(
        "artifact_readback_get",
        {"artifact_id": artifact["artifact_id"], "candidate_id": candidate["candidate_id"]},
    )
    require(
        readback.get("schema_version") == "ArtifactReadback@2"
        and readback.get("artifact_id") == artifact["artifact_id"]
        and readback.get("candidate_id") == candidate["candidate_id"]
        and readback.get("canonical_sha256") == artifact.get("canonical_sha256"),
        "strict GLB readback failed",
    )
    return {
        "candidate_id": candidate["candidate_id"],
        "program_sha256": program_sha,
        "artifact_id": artifact["artifact_id"],
        "artifact_sha256": artifact["object_sha256"],
        "triangle_count": readback.get("triangle_count"),
        "part_count": len(readback.get("part_ids", [])),
        "part_ids": readback.get("part_ids", []),
        "material_zone_count": len(readback.get("material_zone_ids", [])),
        "material_zone_ids": readback.get("material_zone_ids", []),
        "strict_glb_readback": "PASS",
    }


def metric(metrics: dict[str, Any], key: str) -> float:
    value = metrics.get(key)
    require(isinstance(value, (int, float)), f"missing metric {key}")
    return float(value)


def passes_no_regression(baseline: dict[str, Any], trial: dict[str, Any]) -> tuple[bool, dict[str, float]]:
    base = baseline["metrics"]
    new = trial["metrics"]
    deltas = {
        "silhouette_iou": metric(new, "silhouette_iou") - metric(base, "silhouette_iou"),
        "boundary_f1_4px": metric(new, "boundary_f1_4px") - metric(base, "boundary_f1_4px"),
        "bbox_edge_error": metric(new, "bbox_edge_error") - metric(base, "bbox_edge_error"),
        "centroid_error": metric(new, "centroid_error") - metric(base, "centroid_error"),
    }
    passed = (
        deltas["silhouette_iou"] >= -0.005
        and deltas["boundary_f1_4px"] >= -0.01
        and deltas["bbox_edge_error"] <= 0.005
        and deltas["centroid_error"] <= 0.005
    )
    return passed, deltas


def node(node_id: str, operator_id: str, parameters: dict[str, Any]) -> dict[str, Any]:
    return {"node_id": node_id, "operator_id": operator_id, "inputs": [], "parameters": parameters}


def profile_node(node_id: str, profile: list[list[float]], depth: float, z: float) -> dict[str, Any]:
    return node(
        node_id,
        "forgecad.geometry.profile-extrude@1",
        {
            "shape": "profile-extrude",
            "profile": profile,
            "depth_m": depth,
            "position_m": [0, 0, z],
            "rotation_rad": [0, 0, 0],
        },
    )


def cylinder_node(node_id: str, radius: float, depth: float, position: list[float]) -> dict[str, Any]:
    return node(
        node_id,
        "forgecad.geometry.primitive@2",
        {
            "shape": "cylinder",
            "radius_m": radius,
            "height_m": depth,
            "radial_segments": 48,
            "position_m": position,
            "rotation_rad": [math.pi / 2, 0, 0],
        },
    )


def add_part(draft: dict[str, Any], part_id: str, node_ids: list[str], material_zone_id: str) -> None:
    draft["part_outputs"].append(
        {
            "part_id": part_id,
            "input_node_ids": node_ids,
            "material_zone_id": material_zone_id,
            "solid": True,
        }
    )


def armor_layer(draft: dict[str, Any]) -> None:
    draft["nodes"].extend(
        [
            profile_node(
                "upper-armor-spline",
                [[-3.45, 1.56], [-2.75, 1.72], [-1.55, 1.86], [-0.35, 2.02], [0.85, 1.94], [2.25, 1.80], [3.72, 1.65], [3.28, 1.56], [1.72, 1.58], [0.25, 1.68], [-1.10, 1.54], [-2.55, 1.45]],
                0.12,
                0.51,
            ),
            profile_node(
                "lower-keel-spline",
                [[-2.82, 1.05], [-2.22, 0.90], [-1.58, 0.72], [-1.20, 1.05], [-0.20, 1.10], [1.00, 1.20], [2.45, 1.25], [3.42, 1.32], [2.82, 1.42], [1.48, 1.38], [0.02, 1.28], [-0.98, 1.28], [-1.60, 1.18], [-2.42, 1.20]],
                0.11,
                0.52,
            ),
            profile_node(
                "rear-mechanical-inset",
                [[-3.42, 0.92], [-3.05, 0.74], [-2.52, 0.75], [-1.82, 0.92], [-1.52, 1.08], [-2.05, 1.30], [-2.82, 1.41], [-3.36, 1.28]],
                0.08,
                0.59,
            ),
        ]
    )
    add_part(draft, "upper-armor-shell", ["upper-armor-spline"], "white-dielectric-clearcoat")
    add_part(draft, "lower-keel-shell", ["lower-keel-spline"], "white-dielectric-clearcoat")
    add_part(draft, "rear-mechanical-inset", ["rear-mechanical-inset"], "black-anodized-metal")


def energy_core_layer(draft: dict[str, Any]) -> None:
    draft["nodes"].extend(
        [
            cylinder_node("energy-core-outer", 0.50, 0.14, [-0.48, 1.55, 0.62]),
            cylinder_node("energy-core-ring", 0.37, 0.10, [-0.48, 1.55, 0.75]),
            cylinder_node("energy-core-hub", 0.22, 0.08, [-0.48, 1.55, 0.85]),
        ]
    )
    add_part(draft, "energy-core-housing", ["energy-core-outer"], "black-anodized-metal")
    add_part(draft, "energy-core-ring", ["energy-core-ring"], "brushed-steel")
    add_part(draft, "energy-core-emitter", ["energy-core-hub"], "warm-orange-emissive")


def linework_layer(draft: dict[str, Any]) -> None:
    draft["nodes"].extend(
        [
            profile_node(
                "energy-guide-channel",
                [[-0.18, 1.43], [0.65, 1.43], [1.70, 1.47], [2.88, 1.47], [4.25, 1.50], [4.05, 1.58], [2.85, 1.60], [1.65, 1.57], [0.62, 1.54], [-0.18, 1.53]],
                0.06,
                0.92,
            ),
            node(
                "receiver-vent-bank",
                "forgecad.geometry.vent-array@1",
                {
                    "shape": "vent-array",
                    "width_m": 0.92,
                    "height_m": 0.24,
                    "depth_m": 0.06,
                    "slot_count": 5,
                    "slot_width_m": 0.08,
                    "slot_spacing_m": 0.07,
                    "position_m": [1.15, 1.76, 0.83],
                    "rotation_rad": [0, 0, 0],
                },
            ),
            profile_node(
                "dorsal-rail",
                [[-1.65, 1.94], [-0.80, 2.08], [0.45, 2.12], [1.65, 2.02], [2.42, 1.91], [1.55, 1.91], [0.35, 2.00], [-0.72, 1.96]],
                0.08,
                0.72,
            ),
        ]
    )
    add_part(draft, "energy-guide-channel", ["energy-guide-channel"], "warm-orange-emissive")
    add_part(draft, "receiver-vent-bank", ["receiver-vent-bank"], "black-anodized-metal")
    add_part(draft, "dorsal-rail", ["dorsal-rail"], "brushed-steel")


def panel_relief_layer(draft: dict[str, Any]) -> None:
    draft["nodes"].extend(
        [
            profile_node(
                "upper-forward-plate",
                [[0.02, 1.66], [1.05, 1.78], [2.42, 1.71], [3.94, 1.60], [3.18, 1.55], [1.72, 1.60], [0.55, 1.55]],
                0.055,
                1.02,
            ),
            profile_node(
                "upper-rear-plate",
                [[-3.22, 1.48], [-2.52, 1.62], [-1.42, 1.70], [-0.82, 1.83], [-0.86, 1.56], [-2.10, 1.42]],
                0.055,
                0.97,
            ),
            profile_node(
                "lower-forward-shroud",
                [[0.12, 1.17], [1.42, 1.20], [3.72, 1.31], [3.03, 1.42], [1.20, 1.38], [0.22, 1.30]],
                0.055,
                1.01,
            ),
            profile_node(
                "core-rear-collar",
                [[-1.42, 1.04], [-0.88, 1.08], [-0.70, 1.34], [-0.91, 1.58], [-1.48, 1.39]],
                0.06,
                0.69,
            ),
            profile_node(
                "core-forward-collar",
                [[-0.12, 1.08], [0.48, 1.14], [0.66, 1.41], [0.20, 1.54], [-0.06, 1.40]],
                0.06,
                0.70,
            ),
            profile_node(
                "muzzle-terminal-inset",
                [[3.96, 1.43], [4.52, 1.48], [4.61, 1.55], [4.48, 1.63], [3.98, 1.66]],
                0.055,
                0.98,
            ),
        ]
    )
    add_part(draft, "upper-forward-plate", ["upper-forward-plate"], "white-dielectric-clearcoat")
    add_part(draft, "upper-rear-plate", ["upper-rear-plate"], "white-dielectric-clearcoat")
    add_part(draft, "lower-forward-shroud", ["lower-forward-shroud"], "white-dielectric-clearcoat")
    add_part(draft, "core-rear-collar", ["core-rear-collar"], "black-anodized-metal")
    add_part(draft, "core-forward-collar", ["core-forward-collar"], "black-anodized-metal")
    add_part(draft, "muzzle-terminal-inset", ["muzzle-terminal-inset"], "brushed-steel")


def main() -> int:
    args = parse_args()
    data_root = args.data_root.expanduser().resolve()
    base_program = read_json(cas_path(data_root, args.base_program_sha256))
    require(base_program.get("project_id") == args.project_id, "base program belongs to another project")
    base_program.pop("canonical_sha256", None)
    base_program["budgets"]["max_nodes"] = 32

    evidence: dict[str, Any] = {
        "schema_version": "ForgeCADWeaponSecondaryStructureLoop@1",
        "task_id": "FGC-MCP010F-WEAPON-SECONDARY-STRUCTURE",
        "scope": "fictional game and film visual asset only",
        "project_id": args.project_id,
        "reference_id": args.reference_id,
        "confirmed_target_sha256": args.target_sha256,
        "base_candidate_id": args.base_candidate_id,
        "base_program_sha256": args.base_program_sha256,
        "camera": camera_ref(args),
        "candidate_confirmed": False,
        "exported": False,
        "pbr_unlocked": False,
        "stages": [],
    }
    client: McpClient | None = None
    try:
        client = initialize_client(args)
        evidence["skills"] = skill_preflight(client)
        catalog = client.tool("operator_catalog_get")
        require(catalog.get("canonical_sha256") == base_program.get("operator_catalog_sha256"), "operator catalog drifted")
        evidence["operator_catalog_sha256"] = catalog["canonical_sha256"]

        baseline = compare(client, args, args.base_candidate_id)
        evidence["confirmed_contour_baseline"] = baseline
        retained = baseline
        retained_program = base_program

        for stage_id, mutator in (
            ("armor-layering", armor_layer),
            ("energy-core", energy_core_layer),
            ("surface-linework", linework_layer),
            ("panel-relief", panel_relief_layer),
        ):
            trial_program = copy.deepcopy(retained_program)
            mutator(trial_program)
            prepared = prepare(client, args, trial_program)
            comparison = compare(client, args, prepared["candidate_id"])
            passed, deltas = passes_no_regression(retained, comparison)
            stage = {
                "stage_id": stage_id,
                "prepared": prepared,
                "comparison": comparison,
                "metric_deltas_vs_retained": deltas,
                "no_regression_gate": "PASS" if passed else "FAIL",
                "retained": passed,
            }
            evidence["stages"].append(stage)
            if not passed:
                evidence["status"] = "STOPPED_ON_SILHOUETTE_REGRESSION"
                break
            retained = comparison
            retained_program = trial_program
        else:
            evidence["status"] = "SECONDARY_STRUCTURE_NUMERIC_GATE_PASS"

        retained_stage = next((stage for stage in reversed(evidence["stages"]) if stage["retained"]), None)
        evidence["selected_review_candidate"] = retained_stage["prepared"] if retained_stage else {
            "candidate_id": args.base_candidate_id,
            "program_sha256": args.base_program_sha256,
        }
        evidence["selected_review_comparison"] = retained
        evidence["visual_review_required"] = True
        evidence["pbr_unlock_condition"] = "human-visible secondary structure review plus no-regression evidence"
    finally:
        if client is not None:
            client.close()

    output = args.evidence if args.evidence.is_absolute() else REPO_ROOT / args.evidence
    output.resolve().relative_to((REPO_ROOT / "docs" / "evidence").resolve())
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(evidence, ensure_ascii=False, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
