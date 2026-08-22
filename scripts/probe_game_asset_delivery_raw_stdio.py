#!/usr/bin/env python3
"""Exercise automatic typed LOD preview and authored delivery over raw MCP stdio."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_ROOT))

from probe_mcp010b_raw_stdio import (  # noqa: E402
    MCP_PROTOCOL_VERSION,
    GateFailure,
    McpClient,
    build_identity,
    require,
    shutdown_runtime,
    wait_for_ready,
)
from probe_mcp010d_raw_stdio import read_ponytail_preflight  # noqa: E402


def canonical_hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    ).hexdigest()


def program(project_id: str, catalog_sha256: str, segments: int) -> dict[str, Any]:
    parts = [
        ("grip-module", "grip-node", [-0.25, -0.3, 0.0], [0.16, 0.38, 0.16]),
        ("magazine-module", "magazine-node", [0.0, -0.24, 0.0], [0.18, 0.32, 0.14]),
        ("optic-rail", "optic-node", [0.12, 0.25, 0.0], [0.28, 0.08, 0.12]),
        ("energy-core", "energy-node", [0.16, 0.0, 0.0], [0.22, 0.2, 0.18]),
    ]
    nodes = [
        {
            "node_id": node_id,
            "operator_id": "forgecad.geometry.primitive@2",
            "inputs": [],
            "parameters": {
                "shape": "box",
                "size_m": size,
                "position_m": position,
                "rotation_rad": [0.0, 0.0, 0.0],
            },
        }
        for _, node_id, position, size in parts
    ]
    nodes.append(
        {
            "node_id": "barrel-node",
            "operator_id": "forgecad.geometry.primitive@2",
            "inputs": [],
            "parameters": {
                "shape": "cylinder",
                "radius_m": 0.09,
                "height_m": 0.8,
                "radial_segments": segments,
                "position_m": [0.4, 0.0, 0.0],
                "rotation_rad": [0.0, 0.0, 1.5707963267948966],
            },
        }
    )
    part_outputs = [
        {
            "part_id": part_id,
            "input_node_ids": [node_id],
            "material_zone_id": "zone-weapon",
            "solid": True,
        }
        for part_id, node_id, _, _ in parts
    ]
    part_outputs.append(
        {
            "part_id": "barrel-assembly",
            "input_node_ids": ["barrel-node"],
            "material_zone_id": "zone-weapon",
            "solid": True,
        }
    )
    return {
        "schema_version": "GeometryProgram@2",
        "project_id": project_id,
        "representation_plan_sha256": "4" * 64,
        "operator_catalog_sha256": catalog_sha256,
        "units": {
            "length": "meter",
            "angle": "radian",
            "coordinate_system": "right-handed-y-up",
        },
        "budgets": {
            "max_nodes": 5,
            "max_triangles": 2000,
            "max_glb_bytes": 2097152,
            "max_worker_memory_bytes": 536870912,
            "max_runtime_ms": 10000,
        },
        "nodes": nodes,
        "part_outputs": part_outputs,
    }


def anchor_request(project_id: str, delivery_sha256: str) -> dict[str, Any]:
    value = {
        "schema_version": "GameWeaponAnchorPrepareRequest@1",
        "project_id": project_id,
        "delivery_manifest_object_sha256": delivery_sha256,
        "anchor_policy": "weapon-rh-x-forward-y-up-model-space-six-role@1",
        "anchors": [
            {"anchor_id": "weapon-root", "role": "weapon-root", "parent_kind": "synthetic-scene-root", "owner_part_id": None, "local_translation_m": [0.0, 0.0, 0.0], "local_rotation_quat_xyzw": [0.0, 0.0, 0.0, 1.0], "local_scale_xyz": [1.0, 1.0, 1.0]},
            {"anchor_id": "grip-primary", "role": "grip-primary", "parent_kind": "part-node", "owner_part_id": "grip-module", "local_translation_m": [-0.25, -0.3, 0.0], "local_rotation_quat_xyzw": [0.0, 0.0, 0.0, 1.0], "local_scale_xyz": [1.0, 1.0, 1.0]},
            {"anchor_id": "socket-muzzle-vfx", "role": "muzzle-vfx", "parent_kind": "part-node", "owner_part_id": "barrel-assembly", "local_translation_m": [0.8, 0.0, 0.0], "local_rotation_quat_xyzw": [0.0, 0.0, 0.0, 1.0], "local_scale_xyz": [1.0, 1.0, 1.0]},
            {"anchor_id": "socket-magazine-well", "role": "magazine-well", "parent_kind": "part-node", "owner_part_id": "magazine-module", "local_translation_m": [0.0, -0.24, 0.0], "local_rotation_quat_xyzw": [0.0, 0.0, 0.0, 1.0], "local_scale_xyz": [1.0, 1.0, 1.0]},
            {"anchor_id": "socket-sight-primary", "role": "sight-primary", "parent_kind": "part-node", "owner_part_id": "optic-rail", "local_translation_m": [0.12, 0.25, 0.0], "local_rotation_quat_xyzw": [0.0, 0.0, 0.0, 1.0], "local_scale_xyz": [1.0, 1.0, 1.0]},
            {"anchor_id": "socket-energy-core-vfx", "role": "energy-core-vfx", "parent_kind": "part-node", "owner_part_id": "energy-core", "local_translation_m": [0.16, 0.0, 0.0], "local_rotation_quat_xyzw": [0.0, 0.0, 0.0, 1.0], "local_scale_xyz": [1.0, 1.0, 1.0]},
        ],
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def energy_vfx_request(
    project_id: str,
    delivery_sha256: str,
    anchor_sha256: str,
    material_pack_sha256: str,
) -> dict[str, Any]:
    value = {
        "schema_version": "FictionalEnergyVfxPrepareRequest@1",
        "project_id": project_id,
        "delivery_manifest_object_sha256": delivery_sha256,
        "anchor_set_object_sha256": anchor_sha256,
        "material_pack_id": "forgecad-fictional-energy-weapon-2k",
        "material_pack_manifest_sha256": material_pack_sha256,
        "vfx_policy": "fictional-energy-two-effect-time-sampled-emissive-intent@1",
        "effects": [
            {"effect_id": "muzzle-pulse", "anchor_id": "socket-muzzle-vfx", "effect_kind": "muzzle-emissive-pulse", "material_id": "energy-cyan-emissive", "color_linear_rgb": [0.0, 0.82, 1.0], "duration_ticks": 200, "sample_time_ticks": [0, 100, 200], "emissive_strength_samples": [0.0, 8.0, 0.0], "loop_mode": "once", "lod_visibility": [True, True, False]},
            {"effect_id": "energy-core-breathe", "anchor_id": "socket-energy-core-vfx", "effect_kind": "energy-core-emissive-breathe", "material_id": "energy-cyan-emissive", "color_linear_rgb": [0.0, 0.82, 1.0], "duration_ticks": 1000, "sample_time_ticks": [0, 500, 1000], "emissive_strength_samples": [3.0, 6.0, 3.0], "loop_mode": "loop", "lod_visibility": [True, True, True]},
        ],
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def energy_vfx_frame_sample_request(
    project_id: str,
    delivery_sha256: str,
    profile_sha256: str,
    sample_time_ticks: int,
) -> dict[str, Any]:
    value = {
        "schema_version": "FictionalEnergyVfxFrameSampleRequest@1",
        "project_id": project_id,
        "delivery_manifest_object_sha256": delivery_sha256,
        "vfx_profile_object_sha256": profile_sha256,
        "sample_time_ticks": sample_time_ticks,
        "sampling_policy": "integer-tick-linear-once-clamp-loop-modulo-duration@1",
    }
    value["canonical_sha256"] = canonical_hash(value)
    return value


def initialize(client: McpClient, name: str) -> dict[str, str]:
    initialized = client.request(
        "initialize",
        {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": name, "version": "1"},
        },
    )
    require(
        initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION,
        "MCP initialization failed",
    )
    client.notify("notifications/initialized")
    return read_ponytail_preflight(client)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--expected-build-cohort")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--timeout", type=float, default=30.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.mcp.is_file() or not args.runtime.is_file() or args.timeout <= 0:
        raise GateFailure("game delivery source binaries were unavailable")
    if args.expected_build_cohort:
        require(
            re.fullmatch(r"[0-9a-f]{64}", args.expected_build_cohort) is not None,
            "expected build cohort was invalid",
        )
        require(
            build_identity(args.mcp).get("build_cohort_sha256")
            == args.expected_build_cohort
            and build_identity(args.runtime).get("build_cohort_sha256")
            == args.expected_build_cohort,
            "MCP/Runtime build cohort differed",
        )
    data_root = args.data_root.absolute()
    if data_root.exists():
        raise GateFailure("game delivery data root must not pre-exist")
    data_root.mkdir(mode=0o700, parents=True)
    ready_path = data_root / "ipc" / "ready.json"
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
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )
    ready: dict[str, Any] | None = None
    client: McpClient | None = None
    readonly: McpClient | None = None
    receipt: dict[str, Any] | None = None
    try:
        ready = wait_for_ready(ready_path, runtime, args.timeout)
        environment = os.environ.copy()
        for key in (
            "FORGECAD_RUNTIME_COMMAND",
            "FORGECAD_RUNTIME_DATA_DIR",
            "FORGECAD_RUNTIME_READY_FILE",
            "FORGECAD_RUNTIME_STATUS_FILE",
            "FORGECAD_MCP_ENABLE_MCP004_WRITES",
        ):
            environment.pop(key, None)
        environment["FORGECAD_RUNTIME_SOCKET"] = ready["socket_path"]
        environment["FORGECAD_RUNTIME_TOKEN"] = ready["token"]
        readonly_environment = environment.copy()

        readonly = McpClient(args.mcp, readonly_environment, args.timeout)
        initialize(readonly, "forgecad-game-delivery-readonly")
        readonly_tools = readonly.request("tools/list", {}).get("result", {}).get("tools", [])
        derive_tool = next(
            (tool for tool in readonly_tools if tool.get("name") == "game_asset_lod_derive"),
            None,
        )
        require(
            all(
                tool.get("name")
                not in {"game_asset_delivery_prepare", "game_weapon_anchor_prepare", "fictional_energy_vfx_prepare"}
                for tool in readonly_tools
            ),
            "game delivery or weapon anchor write tool leaked into the default tool list",
        )
        anchor_get_tool = next(
            (tool for tool in readonly_tools if tool.get("name") == "game_weapon_anchor_get"),
            None,
        )
        vfx_get_tool = next(
            (tool for tool in readonly_tools if tool.get("name") == "fictional_energy_vfx_get"),
            None,
        )
        vfx_frame_tool = next(
            (tool for tool in readonly_tools if tool.get("name") == "fictional_energy_vfx_frame_sample"),
            None,
        )
        require(
            isinstance(derive_tool, dict)
            and derive_tool.get("annotations", {}).get("readOnlyHint") is True
            and derive_tool.get("inputSchema", {}).get("additionalProperties") is False,
            "automatic LOD derive was not a closed default read tool",
        )
        require(
            isinstance(anchor_get_tool, dict)
            and anchor_get_tool.get("annotations", {}).get("readOnlyHint") is True
            and anchor_get_tool.get("inputSchema", {}).get("additionalProperties") is False,
            "weapon anchor get was not a closed default read tool",
        )
        require(
            isinstance(vfx_get_tool, dict)
            and vfx_get_tool.get("annotations", {}).get("readOnlyHint") is True
            and vfx_get_tool.get("inputSchema", {}).get("additionalProperties") is False,
            "fictional energy VFX get was not a closed default read tool",
        )
        require(
            isinstance(vfx_frame_tool, dict)
            and vfx_frame_tool.get("annotations", {}).get("readOnlyHint") is True
            and vfx_frame_tool.get("inputSchema", {}).get("additionalProperties") is False,
            "fictional energy VFX frame sample was not a closed default read tool",
        )
        readonly.close()

        environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
        client = McpClient(args.mcp, environment, args.timeout)
        preflight = initialize(client, "forgecad-game-delivery-raw-stdio")
        write_tools = client.request("tools/list", {}).get("result", {}).get("tools", [])
        delivery_tool = next(
            (tool for tool in write_tools if tool.get("name") == "game_asset_delivery_prepare"),
            None,
        )
        anchor_prepare_tool = next(
            (tool for tool in write_tools if tool.get("name") == "game_weapon_anchor_prepare"),
            None,
        )
        vfx_prepare_tool = next(
            (tool for tool in write_tools if tool.get("name") == "fictional_energy_vfx_prepare"),
            None,
        )
        require(
            isinstance(delivery_tool, dict)
            and delivery_tool.get("annotations", {}).get("readOnlyHint") is False
            and delivery_tool.get("inputSchema", {}).get("additionalProperties") is False,
            "game delivery write tool was not closed and explicit",
        )
        require(
            isinstance(anchor_prepare_tool, dict)
            and anchor_prepare_tool.get("annotations", {}).get("readOnlyHint") is False
            and anchor_prepare_tool.get("inputSchema", {}).get("additionalProperties") is False,
            "weapon anchor prepare tool was not closed and explicit",
        )
        require(
            isinstance(vfx_prepare_tool, dict)
            and vfx_prepare_tool.get("annotations", {}).get("readOnlyHint") is False
            and vfx_prepare_tool.get("inputSchema", {}).get("additionalProperties") is False,
            "fictional energy VFX prepare tool was not closed and explicit",
        )
        capabilities = client.tool("capabilities_get")
        catalog_sha256 = capabilities.get("operator_catalog_sha256")
        require(isinstance(catalog_sha256, str), "operator catalog hash missing")
        project = client.tool(
            "project_create",
            {"name": "raw authored LOD delivery", "policy": {"profile": "mvp"}},
        )
        project_id = project.get("project_id")
        require(isinstance(project_id, str), "project_create omitted project_id")
        prepared: list[dict[str, Any]] = []
        for segments in (64, 32, 16):
            draft = program(project_id, catalog_sha256, segments)
            hashed = client.tool(
                "geometry_program_hash",
                {
                    "schema_version": "GeometryProgramHashRequest@1",
                    "geometry_program_draft": draft,
                },
            )
            exact = copy.deepcopy(draft)
            exact["canonical_sha256"] = hashed["canonical_sha256"]
            value = client.tool(
                "geometry_prepare",
                {
                    "project_id": project_id,
                    "request": {"typed": "geometry", "geometry_program": exact},
                },
            )
            require(
                value.get("schema_version") == "GeometryPrepareResult@2"
                and value.get("artifact", {}).get("hard_gate_passed") is True,
                "LOD candidate preparation failed",
            )
            prepared.append(value)
        lods = [
            {
                "level": level,
                "candidate_id": value["candidate"]["candidate_id"],
                "candidate_state_sha256": value["candidate"]["canonical_sha256"],
                "artifact_sha256": value["artifact"]["artifact_id"],
                "artifact_readback_sha256": value["artifact"]["canonical_sha256"],
            }
            for level, value in enumerate(prepared)
        ]
        source = prepared[0]
        derive_arguments = {
            "schema_version": "GameAssetLodDeriveRequest@1",
            "project_id": project_id,
            "source_candidate_id": source["candidate"]["candidate_id"],
            "source_candidate_state_sha256": source["candidate"]["canonical_sha256"],
            "source_artifact_sha256": source["artifact"]["artifact_id"],
            "source_artifact_readback_sha256": source["artifact"]["canonical_sha256"],
            "source_geometry_program_sha256": source["artifact"]["program_sha256"],
            "source_operator_catalog_sha256": source["artifact"][
                "operator_catalog_sha256"
            ],
            "source_readback_config_sha256": source["artifact"][
                "readback_config_sha256"
            ],
            "derive_policy": "runtime-owned-typed-segment-lowering-lod1-half-lod2-quarter@1",
        }
        derive_arguments["canonical_sha256"] = canonical_hash(derive_arguments)
        derive_candidates_before = [
            client.tool("candidate_get", {"candidate_id": lod["candidate_id"]})
            for lod in lods
        ]
        derive_versions_before = client.tool("version_list", {"project_id": project_id})
        client.close()
        client = None
        readonly = McpClient(args.mcp, environment, args.timeout)
        initialize(readonly, "forgecad-game-auto-lod-readonly")
        derive_raw = readonly.request(
            "tools/call",
            {"name": "game_asset_lod_derive", "arguments": derive_arguments},
        )
        derive_result = derive_raw.get("result", {})
        derived = derive_result.get("structuredContent")
        require(
            derive_result.get("isError") is not True
            and isinstance(derived, dict)
            and derived.get("schema_version") == "GameAssetLodDeriveResult@1",
            f"automatic LOD raw MCP call failed: {derive_raw}",
        )
        derive_wire_bytes = len(
            json.dumps(derive_raw, separators=(",", ":")).encode("utf-8")
        )
        require(
            derive_wire_bytes <= 1024 * 1024,
            "automatic LOD final MCP response exceeded 1 MiB",
        )
        automatic_lod_triangle_counts = [
            level.get("triangle_count") for level in derived["levels"]
        ]
        require(
            automatic_lod_triangle_counts == [304, 176, 112]
            and derived.get("worker_replay_verified") is True
            and derived.get("runtime_write_performed") is False
            and derived.get("persistent_user_data_touched") is False
            and derived.get("materialization_required") is True,
            f"automatic LOD truth flags differed: {derived}",
        )
        forbidden_derive = copy.deepcopy(derive_arguments)
        forbidden_derive["python"] = "bpy.ops.object.modifier_add(type='DECIMATE')"
        rejected_derive = readonly.request(
            "tools/call",
            {"name": "game_asset_lod_derive", "arguments": forbidden_derive},
        )
        require(
            rejected_derive.get("error", {}).get("data", {}).get("code")
            == "INVALID_TOOL_PARAMS",
            "automatic LOD closed schema accepted a Python field",
        )
        readonly.close()
        readonly = None
        client = McpClient(args.mcp, environment, args.timeout)
        initialize(client, "forgecad-game-delivery-after-auto-lod")
        require(
            [
                client.tool("candidate_get", {"candidate_id": lod["candidate_id"]})
                for lod in lods
            ]
            == derive_candidates_before
            and client.tool("version_list", {"project_id": project_id})
            == derive_versions_before,
            "automatic LOD preview changed candidate or version state",
        )
        arguments = {
            "schema_version": "GameAssetDeliveryPrepareRequest@1",
            "project_id": project_id,
            "lods": lods,
            "animation": None,
            "lod_policy": "authored-three-level-part-stable-progressive-triangles@1",
            "collision_policy": "per-part-aabb-box-from-lod2-visual-geometry@1",
            "readiness_policy": "engine-neutral-gltf2-embedded-assets-stable-names@1",
        }
        arguments["canonical_sha256"] = canonical_hash(arguments)
        candidates_before = [
            client.tool("candidate_get", {"candidate_id": lod["candidate_id"]})
            for lod in lods
        ]
        versions_before = client.tool("version_list", {"project_id": project_id})
        raw = client.request(
            "tools/call",
            {"name": "game_asset_delivery_prepare", "arguments": arguments},
        )
        result = raw.get("result", {})
        value = result.get("structuredContent")
        require(
            result.get("isError") is not True
            and isinstance(value, dict)
            and value.get("schema_version") == "GameAssetDeliveryPrepareResult@1",
            f"game delivery raw MCP call failed: {raw}; arguments={arguments}",
        )
        triangle_counts = [
            level.get("triangle_count") for level in value["lod_receipt"]["levels"]
        ]
        require(triangle_counts == [304, 176, 112], "LOD triangle counts differed")
        require(
            len(value["collision_proxy_set"]["proxies"]) == 5
            and value["collision_proxy_set"]["physical_properties_included"] is False
            and value["candidate_confirmed"] is False
            and value["export_performed"] is False
            and value["actual_engine_roundtrip"] is False,
            "delivery truth flags differed",
        )
        replay = client.tool("game_asset_delivery_prepare", arguments)
        require(replay == value, "delivery replay was not content-identical")
        durable = client.tool(
            "game_asset_delivery_get",
            {
                "schema_version": "GameAssetDeliveryGetRequest@1",
                "project_id": project_id,
                "delivery_manifest_object_sha256": value[
                    "delivery_manifest_object_sha256"
                ],
            },
        )
        require(
            durable.get("schema_version") == "GameAssetDeliveryGetResult@1"
            and durable.get("restart_hash_verified") is True
            and durable.get("link") == value.get("durable_link"),
            "durable game delivery get did not re-verify the exact link",
        )
        require(
            [
                client.tool("candidate_get", {"candidate_id": lod["candidate_id"]})
                for lod in lods
            ]
            == candidates_before
            and client.tool("version_list", {"project_id": project_id}) == versions_before,
            "delivery prepare changed candidate or version state",
        )
        forbidden = copy.deepcopy(arguments)
        forbidden["python"] = "bpy.ops"
        rejected = client.request(
            "tools/call",
            {"name": "game_asset_delivery_prepare", "arguments": forbidden},
        )
        require(
            rejected.get("error", {}).get("data", {}).get("code")
            == "INVALID_TOOL_PARAMS",
            "closed schema accepted a Python field",
        )
        anchor_arguments = anchor_request(
            project_id, value["delivery_manifest_object_sha256"]
        )
        anchor_raw = client.request(
            "tools/call",
            {"name": "game_weapon_anchor_prepare", "arguments": anchor_arguments},
        )
        anchor_result = anchor_raw.get("result", {})
        anchor = anchor_result.get("structuredContent")
        require(
            anchor_result.get("isError") is not True
            and isinstance(anchor, dict)
            and anchor.get("schema_version") == "GameWeaponAnchorPrepareResult@1"
            and anchor.get("anchor_set", {}).get("pivot_status")
            == "not-proven-runtime-pivot"
            and anchor.get("anchor_set", {}).get("node_materialization")
            == "sidecar-only-not-glb-nodes"
            and anchor.get("actual_engine_roundtrip") is False,
            f"weapon anchor raw MCP call failed: {anchor_raw}",
        )
        anchor_replay = client.tool("game_weapon_anchor_prepare", anchor_arguments)
        require(anchor_replay == anchor, "weapon anchor replay was not content-identical")
        anchor_get = client.tool(
            "game_weapon_anchor_get",
            {
                "schema_version": "GameWeaponAnchorGetRequest@1",
                "project_id": project_id,
                "delivery_manifest_object_sha256": value[
                    "delivery_manifest_object_sha256"
                ],
            },
        )
        require(
            anchor_get.get("schema_version") == "GameWeaponAnchorGetResult@1"
            and anchor_get.get("restart_hash_verified") is True
            and anchor_get.get("runtime_write_performed") is False
            and anchor_get.get("link") == anchor.get("durable_link"),
            "weapon anchor durable get did not re-verify the exact link",
        )
        bad_anchor = copy.deepcopy(anchor_arguments)
        bad_anchor["anchors"][2]["local_translation_m"] = [0.4, 0.0, 0.0]
        bad_anchor["canonical_sha256"] = canonical_hash(
            {key: item for key, item in bad_anchor.items() if key != "canonical_sha256"}
        )
        rejected_anchor = client.request(
            "tools/call",
            {"name": "game_weapon_anchor_prepare", "arguments": bad_anchor},
        )
        require(
            rejected_anchor.get("result", {}).get("isError") is True,
            "weapon anchor accepted a muzzle helper away from the +X end",
        )
        weapon_2k_pack = client.tool(
            "material_pack_get", {"pack_id": "forgecad-fictional-energy-weapon-2k"}
        )
        vfx_arguments = energy_vfx_request(
            project_id,
            value["delivery_manifest_object_sha256"],
            anchor["anchor_set_object_sha256"],
            weapon_2k_pack["canonical_sha256"],
        )
        vfx_raw = client.request(
            "tools/call",
            {"name": "fictional_energy_vfx_prepare", "arguments": vfx_arguments},
        )
        vfx_result = vfx_raw.get("result", {})
        vfx = vfx_result.get("structuredContent")
        require(
            vfx_result.get("isError") is not True
            and isinstance(vfx, dict)
            and vfx.get("schema_version") == "FictionalEnergyVfxPrepareResult@1"
            and vfx.get("vfx_profile", {}).get("static_emissive_material_definition_verified") is True
            and vfx.get("vfx_profile", {}).get("emissive_animation_rendered") is False
            and vfx.get("vfx_profile", {}).get("bloom_rendered") is False
            and vfx.get("vfx_profile", {}).get("particles_rendered") is False
            and vfx.get("vfx_profile", {}).get("trails_rendered") is False,
            f"fictional energy VFX raw MCP call failed: {vfx_raw}",
        )
        require(
            client.tool("fictional_energy_vfx_prepare", vfx_arguments) == vfx,
            "fictional energy VFX replay was not content-identical",
        )
        vfx_get = client.tool(
            "fictional_energy_vfx_get",
            {"schema_version": "FictionalEnergyVfxGetRequest@1", "project_id": project_id, "delivery_manifest_object_sha256": value["delivery_manifest_object_sha256"]},
        )
        require(
            vfx_get.get("schema_version") == "FictionalEnergyVfxGetResult@1"
            and vfx_get.get("restart_hash_verified") is True
            and vfx_get.get("runtime_write_performed") is False
            and vfx_get.get("link") == vfx.get("durable_link"),
            "fictional energy VFX durable get did not re-verify the exact link",
        )
        frame_50_arguments = energy_vfx_frame_sample_request(
            project_id,
            value["delivery_manifest_object_sha256"],
            vfx["vfx_profile_object_sha256"],
            50,
        )
        frame_50 = client.tool("fictional_energy_vfx_frame_sample", frame_50_arguments)
        require(
            frame_50.get("schema_version") == "FictionalEnergyVfxFrameSample@1"
            and frame_50.get("interpolation") == "LINEAR"
            and frame_50.get("effects", [])[0].get("resolved_time_ticks") == 50
            and frame_50.get("effects", [])[0].get("emissive_strength") == 4.0
            and frame_50.get("effects", [])[1].get("resolved_time_ticks") == 50
            and frame_50.get("effects", [])[1].get("emissive_strength") == 3.3
            and frame_50.get("glb_material_zone_binding_verified") is False
            and frame_50.get("emissive_animation_rendered") is False
            and frame_50.get("runtime_write_performed") is False,
            f"fictional energy VFX frame sample at 50 ticks failed: {frame_50}",
        )
        frame_1000 = client.tool(
            "fictional_energy_vfx_frame_sample",
            energy_vfx_frame_sample_request(
                project_id,
                value["delivery_manifest_object_sha256"],
                vfx["vfx_profile_object_sha256"],
                1000,
            ),
        )
        require(
            frame_1000.get("effects", [])[0].get("resolved_time_ticks") == 200
            and frame_1000.get("effects", [])[0].get("emissive_strength") == 0.0
            and frame_1000.get("effects", [])[1].get("resolved_time_ticks") == 0
            and frame_1000.get("effects", [])[1].get("emissive_strength") == 3.0,
            f"fictional energy VFX once-clamp or loop-modulo semantics failed: {frame_1000}",
        )
        bad_frame = energy_vfx_frame_sample_request(
            project_id,
            value["delivery_manifest_object_sha256"],
            vfx["vfx_profile_object_sha256"],
            50,
        )
        bad_frame["sample_time_ticks"] = -1
        bad_frame["canonical_sha256"] = canonical_hash(
            {key: item for key, item in bad_frame.items() if key != "canonical_sha256"}
        )
        rejected_frame = client.request(
            "tools/call",
            {"name": "fictional_energy_vfx_frame_sample", "arguments": bad_frame},
        )
        require(
            rejected_frame.get("error", {}).get("data", {}).get("code") == "INVALID_TOOL_PARAMS",
            "fictional energy VFX frame sample accepted a negative tick",
        )
        bad_vfx = copy.deepcopy(vfx_arguments)
        bad_vfx["effects"][0]["emissive_strength_samples"] = [0.0, -1.0, 0.0]
        bad_vfx["canonical_sha256"] = canonical_hash(
            {key: item for key, item in bad_vfx.items() if key != "canonical_sha256"}
        )
        rejected_vfx = client.request(
            "tools/call",
            {"name": "fictional_energy_vfx_prepare", "arguments": bad_vfx},
        )
        require(
            rejected_vfx.get("error", {}).get("data", {}).get("code") == "INVALID_TOOL_PARAMS",
            "fictional energy VFX accepted a negative emissive sample",
        )
        receipt = {
            "schema_version": "ForgeCADGameAssetDeliveryRawStdioProbe@1",
            "gate": "PASS_AUTHORED_LOD_SET_COLLISION_DELIVERY_RAW_STDIO",
            "protocol_version": MCP_PROTOCOL_VERSION,
            "build_cohort_sha256": args.expected_build_cohort,
            "ponytail_preflight": preflight,
            "default_write_tool_visibility": "HIDDEN",
            "explicit_write_opt_in": "PASS",
            "lod_triangle_counts": triangle_counts,
            "part_ids": value["lod_receipt"]["part_ids"],
            "material_zone_ids": value["lod_receipt"]["material_zone_ids"],
            "collision_proxy_count": len(value["collision_proxy_set"]["proxies"]),
            "lod_receipt_object_sha256": value["lod_receipt_object_sha256"],
            "collision_proxy_object_sha256": value["collision_proxy_object_sha256"],
            "readiness_object_sha256": value["readiness_object_sha256"],
            "delivery_manifest_object_sha256": value["delivery_manifest_object_sha256"],
            "durable_link_materialization_status": value["durable_link"][
                "materialization_status"
            ],
            "durable_get": "PASS_CURRENT_RUNTIME_CAS_REVERIFY",
            "runtime_restart_get": "PASS_FOCUSED_RUNTIME_TEST",
            "replay": "PASS_CONTENT_IDENTICAL",
            "candidate_state_unchanged": True,
            "version_state_unchanged": True,
            "forbidden_python": "REJECTED_INVALID_TOOL_PARAMS",
            "wire_bytes": len(json.dumps(raw, separators=(",", ":")).encode("utf-8")),
            "candidate_confirmed": False,
            "export_performed": False,
            "actual_engine_roundtrip": False,
            "threejs": "NOT_RUN_IN_THIS_RAW_PROBE",
            "unity": "NOT_RUN",
            "unreal": "NOT_RUN",
            "godot": "NOT_RUN",
            "automatic_lod_generation": "PASS_RUNTIME_TYPED_PROGRAM_DERIVATION_PREVIEW",
            "automatic_lod_triangle_counts": automatic_lod_triangle_counts,
            "automatic_lod_worker_replay": derived["worker_replay_verified"],
            "automatic_lod_runtime_write_performed": derived[
                "runtime_write_performed"
            ],
            "automatic_lod_materialization_required": derived[
                "materialization_required"
            ],
            "automatic_lod_wire_bytes": derive_wire_bytes,
            "automatic_lod_default_read_invocation": "PASS_WITHOUT_WRITE_OPT_IN",
            "automatic_lod_forbidden_python": "REJECTED_INVALID_TOOL_PARAMS",
            "weapon_anchor_set_object_sha256": anchor["anchor_set_object_sha256"],
            "weapon_anchor_set": anchor["anchor_set"],
            "weapon_anchor_roles": [
                item["role"] for item in anchor["anchor_set"]["anchors"]
            ],
            "weapon_anchor_pivot_status": anchor["anchor_set"]["pivot_status"],
            "weapon_anchor_node_materialization": anchor["anchor_set"][
                "node_materialization"
            ],
            "weapon_anchor_durable_get": "PASS_CURRENT_RUNTIME_CAS_REVERIFY",
            "weapon_anchor_replay": "PASS_CONTENT_IDENTICAL",
            "weapon_anchor_bad_muzzle": "REJECTED_FAIL_CLOSED",
            "weapon_anchor_actual_engine_roundtrip": False,
            "weapon_anchor_wire_bytes": len(
                json.dumps(anchor_raw, separators=(",", ":")).encode("utf-8")
            ),
            "fictional_energy_vfx_profile_object_sha256": vfx["vfx_profile_object_sha256"],
            "fictional_energy_vfx_profile": vfx["vfx_profile"],
            "fictional_energy_vfx_durable_get": "PASS_CURRENT_RUNTIME_CAS_REVERIFY",
            "fictional_energy_vfx_replay": "PASS_CONTENT_IDENTICAL",
            "fictional_energy_vfx_negative_sample": "REJECTED_INVALID_TOOL_PARAMS",
            "fictional_energy_vfx_material_animation_rendered": False,
            "fictional_energy_vfx_bloom_rendered": False,
            "fictional_energy_vfx_particles_rendered": False,
            "fictional_energy_vfx_trails_rendered": False,
            "fictional_energy_vfx_wire_bytes": len(
                json.dumps(vfx_raw, separators=(",", ":")).encode("utf-8")
            ),
            "fictional_energy_vfx_frame_50": frame_50,
            "fictional_energy_vfx_frame_1000": frame_1000,
            "fictional_energy_vfx_frame_sampling": "PASS_LINEAR_ONCE_CLAMP_LOOP_MODULO",
            "fictional_energy_vfx_frame_negative_tick": "REJECTED_INVALID_TOOL_PARAMS",
            "fictional_energy_vfx_frame_runtime_write_performed": False,
            "fictional_energy_vfx_frame_glb_material_zone_binding_verified": False,
            "quality_status": "structural_only",
        }
    finally:
        if readonly is not None:
            readonly.close()
        if client is not None:
            client.close()
        if ready is not None:
            shutdown_runtime(ready, ready_path, runtime)
        elif runtime.poll() is None:
            runtime.kill()
            runtime.wait(timeout=5)
    if receipt is None:
        raise GateFailure("game delivery receipt was not produced")
    encoded = json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.evidence:
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(encoded, encoding="utf-8")
    sys.stdout.write(encoded)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(f"GAME_ASSET_DELIVERY_RAW_STDIO_GATE_FAIL: {error}", file=sys.stderr)
        raise SystemExit(1)
