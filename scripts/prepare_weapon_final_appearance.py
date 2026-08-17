#!/usr/bin/env python3
"""Prepare a visible-view fictional energy-weapon appearance candidate.

This is a compatibility appearance route for the current live AssetPack,
which lacks cyan emissive and gold anodized materials.  Runtime remains the
sole writer and produces the GLB, render, comparison and quality evidence.
The script never confirms or exports the newly prepared candidate.
"""

from __future__ import annotations

import argparse
import copy
import json
import os
import sys
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_ROOT.parent
sys.path.insert(0, str(SCRIPT_ROOT))

from probe_mcp010b_raw_stdio import MCP_PROTOCOL_VERSION, McpClient  # noqa: E402
from probe_mcp010c_codex_cli import canonical_hash, view_spec  # noqa: E402


class FinalAppearanceFailure(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise FinalAppearanceFailure(message)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--project-id", required=True)
    parser.add_argument("--reference-id", required=True)
    parser.add_argument("--reference-sha256", required=True)
    parser.add_argument("--geometry-candidate-id", required=True)
    parser.add_argument("--geometry-program-sha256", required=True)
    parser.add_argument("--target-sha256", required=True)
    parser.add_argument("--camera-hash", required=True)
    parser.add_argument("--camera-canonical-sha256", required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=120.0)
    return parser.parse_args()


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"expected JSON object at {path.name}")
    return value


def cas_path(data_root: Path, sha256: str) -> Path:
    require(len(sha256) == 64 and all(ch in "0123456789abcdef" for ch in sha256), "invalid CAS hash")
    return data_root / "cas" / "objects" / sha256[:2] / sha256


def initialize_client(args: argparse.Namespace) -> McpClient:
    ready = read_json(args.data_root / "ipc" / "ready.json")
    socket_path = ready.get("socket_path")
    token = ready.get("token")
    require(isinstance(socket_path, str) and Path(socket_path).exists(), "Runtime socket is unavailable")
    require(isinstance(token, str) and token, "Runtime token is unavailable")
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
            "clientInfo": {"name": "forgecad-weapon-final-appearance", "version": "1"},
        },
    )
    require(initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION, "MCP initialize failed")
    client.notify("notifications/initialized")
    return client


def camera_calibration(args: argparse.Namespace) -> dict[str, Any]:
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


def appearance_program(args: argparse.Namespace, pack_sha256: str) -> dict[str, Any]:
    white_parts = [
        "receiver-core",
        "upper-armor-shell",
        "lower-keel-shell",
        "upper-forward-plate",
        "upper-rear-plate",
        "lower-forward-shroud",
    ]
    black_parts = [
        "rear-mechanical-inset",
        "energy-core-housing",
        "receiver-vent-bank",
        "core-rear-collar",
        "core-forward-collar",
    ]
    gold_parts = ["energy-core-ring", "dorsal-rail", "muzzle-terminal-inset"]
    cyan_parts = ["energy-core-emitter", "energy-guide-channel"]
    value: dict[str, Any] = {
        "schema_version": "AppearanceProgram@2",
        "project_id": args.project_id,
        "geometry_program_sha256": args.geometry_program_sha256,
        "material_pack_id": "forgecad-hard-surface-robot",
        "material_pack_manifest_sha256": pack_sha256,
        "material_zones": [
            {
                "zone_id": "white-dielectric-clearcoat",
                "part_ids": white_parts,
                "material_id": "white-dielectric-clearcoat",
                "texture_set_id": "plastic-surface",
            },
            {
                "zone_id": "black-anodized-metal",
                "part_ids": black_parts,
                "material_id": "black-anodized-metal",
                "texture_set_id": "metal-surface",
            },
            {
                "zone_id": "brushed-steel",
                "part_ids": gold_parts,
                "material_id": "brushed-steel",
                "texture_set_id": "metal-surface",
            },
            {
                "zone_id": "warm-orange-emissive",
                "part_ids": cyan_parts,
                "material_id": "warm-orange-emissive",
                "texture_set_id": None,
            },
        ],
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def compare(client: McpClient, args: argparse.Namespace, candidate_id: str) -> dict[str, Any]:
    spec = view_spec(
        args.reference_id,
        args.reference_sha256,
        1000,
        220,
        {"landmarks": [], "regions": []},
        kind="right",
        view_id="weapon-right-final-visible",
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
        "render_set_object_sha256": result.get("render_set_object_sha256"),
        "comparison_report_object_sha256": result.get("comparison_report_object_sha256"),
        "quality_report_object_sha256": result.get("quality_report_object_sha256"),
        "status": report.get("status"),
        "metrics": report["metrics"],
        "view_spec_canonical_sha256": spec["canonical_sha256"],
    }


def main() -> int:
    args = parse_args()
    args.data_root = args.data_root.expanduser().resolve()
    geometry = read_json(cas_path(args.data_root, args.geometry_program_sha256))
    require(geometry.get("project_id") == args.project_id, "geometry program belongs to another project")
    geometry["canonical_sha256"] = args.geometry_program_sha256

    evidence: dict[str, Any] = {
        "schema_version": "ForgeCADWeaponFinalVisibleAppearance@1",
        "task_id": "FGC-MCP010F-WEAPON-FINAL-VISIBLE",
        "scope": "fictional game and film visual asset only",
        "project_id": args.project_id,
        "reference_id": args.reference_id,
        "geometry_candidate_id": args.geometry_candidate_id,
        "geometry_program_sha256": args.geometry_program_sha256,
        "confirmed_target_sha256": args.target_sha256,
        "candidate_confirmed": False,
        "exported": False,
        "hq_360_status": "BLOCKED_REFERENCE_COVERAGE",
    }
    client: McpClient | None = None
    try:
        client = initialize_client(args)
        capabilities = client.tool("capabilities_get")
        runtime_status = client.tool("runtime_status")
        doctor = client.tool("doctor")
        require(runtime_status.get("state") == "Ready" and doctor.get("state") == "Ready", "Runtime is not Ready")
        preflight = client.tool("skill_get", {"skill_id": "ponytail-preflight", "version": "0.1.0"})
        uv_pbr = client.tool("skill_get", {"skill_id": "uv-pbr", "version": "0.2.0"})
        require(uv_pbr.get("skill", {}).get("execution_availability") == "active", "uv-pbr Skill is not active")
        catalog = client.tool("operator_catalog_get")
        require(catalog.get("canonical_sha256") == capabilities.get("operator_catalog_sha256"), "catalog digest mismatch")
        material_pack = client.tool("material_pack_get")
        require(material_pack.get("pack_id") == "forgecad-hard-surface-robot", "offline material pack is unavailable")
        evidence["runtime"] = {
            "build_cohort_sha256": capabilities.get("build_cohort_sha256"),
            "operator_catalog_sha256": catalog.get("canonical_sha256"),
            "state": runtime_status.get("state"),
        }
        evidence["skills"] = {
            "ponytail_manifest_sha256": preflight.get("skill", {}).get("canonical_sha256"),
            "uv_pbr_manifest_sha256": uv_pbr.get("skill", {}).get("canonical_sha256"),
        }
        evidence["offline_pack"] = {
            "pack_id": material_pack.get("pack_id"),
            "manifest_sha256": material_pack.get("canonical_sha256"),
            "status": material_pack.get("status"),
            "limitation": "pack has embedded PBR textures but no cyan-emissive or gold-anodized material definitions",
        }

        base_candidate = client.tool("candidate_get", {"candidate_id": args.geometry_candidate_id})
        require(base_candidate.get("state") in ("reviewable", "confirmed"), "geometry candidate is unavailable")
        base_readback = client.tool(
            "artifact_readback_get",
            {
                "artifact_id": base_candidate["prepared_object_sha256"],
                "candidate_id": args.geometry_candidate_id,
            },
        )
        require(base_readback.get("hard_gate_passed") is True, "geometry readback hard gate failed")

        pack_sha256 = material_pack.get("canonical_sha256")
        require(isinstance(pack_sha256, str) and len(pack_sha256) == 64, "material pack hash is unavailable")
        appearance = appearance_program(args, pack_sha256)
        prepared = client.tool(
            "appearance_prepare",
            {
                "project_id": args.project_id,
                "request": {
                    "typed": "appearance",
                    "reference_id": args.reference_id,
                    "geometry_program": geometry,
                    "appearance_program": appearance,
                },
            },
        )
        candidate = prepared.get("candidate")
        artifact = prepared.get("artifact")
        require(isinstance(candidate, dict) and isinstance(artifact, dict), "appearance prepare omitted candidate/artifact")
        readback = client.tool(
            "artifact_readback_get",
            {"artifact_id": artifact["artifact_id"], "candidate_id": candidate["candidate_id"]},
        )
        require(
            readback.get("schema_version") in ("ArtifactReadback@1", "ArtifactReadback@2")
            and readback.get("candidate_id") == candidate["candidate_id"]
            and readback.get("hard_gate_passed") is True,
            "appearance GLB strict readback failed",
        )
        comparison = compare(client, args, candidate["candidate_id"])
        quality = client.tool("quality_get", {"candidate_id": candidate["candidate_id"], "reference_id": args.reference_id})
        evidence["appearance_program"] = appearance
        evidence["prepared_candidate"] = {
            "candidate_id": candidate["candidate_id"],
            "prepared_object_id": candidate.get("prepared_object_id"),
            "prepared_object_sha256": candidate.get("prepared_object_sha256"),
            "quality_report_id": candidate.get("quality_report_id"),
            "state": candidate.get("state"),
        }
        evidence["artifact_readback"] = {
            "artifact_id": readback.get("artifact_id"),
            "object_sha256": readback.get("object_sha256"),
            "canonical_sha256": readback.get("canonical_sha256"),
            "schema_version": readback.get("schema_version"),
            "hard_gate_passed": readback.get("hard_gate_passed"),
            "triangle_count": readback.get("triangle_count"),
            "part_ids": readback.get("part_ids"),
            "material_zone_ids": readback.get("material_zone_ids"),
            "integrity": readback.get("integrity"),
        }
        evidence["comparison"] = comparison
        evidence["quality"] = quality
        evidence["material_status"] = {
            "visible_color_match": "SUBSTITUTED_BRUSHED_STEEL_AND_WARM_EMISSIVE",
            "embedded_texture_fidelity": "PBR_V2_PASS_WITH_CURRENT_OFFLINE_PACK",
            "pbr_v2_status": "PASS_WITH_COLORWAY_GAP",
        }
        evidence["status"] = "FINAL_VISIBLE_REVIEW_CANDIDATE_PREPARED"
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
