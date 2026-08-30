#!/usr/bin/env python3
"""Materialize and restart-read the D1 production Low 4K Hero UV layout.

The helper only sends closed typed requests. ForgeCAD Runtime resolves the
candidate-bound Low GLB/readback, replays the bounded UV Worker twice, and is
the sole CAS/SQLite writer. No candidate is confirmed, versioned, or exported.
"""

from __future__ import annotations

import argparse
import json
import struct
import tempfile
from pathlib import Path
from typing import Any

from probe_mcp010c_codex_cli import canonical_hash
from probe_production_weapon_form_art_repair_execution_d1 import (
    close_client,
    open_client,
    require,
)

PROJECT_ID = "project-0d236b8acdde4f1187b3a46a7d5e4f0f"
CANDIDATE_ID = "candidate-50b6981546f74bca9c2ca774ac5c1b00"
CANDIDATE_STATE = "b70f01f6f60fffe7ce42c92f33b95d8687a70ee3b97d291ccd3a18b70255055c"
LOW_ARTIFACT = "28bba1541e30a7bb4109f737eb548cfa854fa295c5a5079263c1e8f93736596e"
LOW_READBACK_OBJECT = "3e047c4a4ec3af11b03b7c2750ffb18370d6a476b4169203f348951752fb83f4"
LOW_READBACK_CANONICAL = "3143fb0679b5694bd62e08c7a97c9e71e9df979895db52b8f3f2e96b44727a47"
PART_IDS = (
    "receiver-main", "receiver-upper", "receiver-lower", "rear-stock", "rear-cap",
    "grip", "trigger-guard", "underbrace", "top-fin", "top-rail", "bottom-rail",
    "side-light-left", "side-light-right", "muzzle-shroud", "muzzle-emitter",
    "muzzle-core", "energy-ring", "energy-core", "core-housing", "side-panel-a",
    "side-panel-b", "magazine", "rear-light",
)
FPS_PRIORITY = {
    "receiver-main", "receiver-upper", "receiver-lower", "grip", "trigger-guard",
    "underbrace", "top-fin", "top-rail", "side-light-left", "side-light-right",
    "muzzle-shroud", "muzzle-emitter", "muzzle-core", "energy-ring", "energy-core",
    "core-housing", "side-panel-a", "side-panel-b", "magazine",
}
REAR_PRIORITY = {"rear-stock", "rear-cap", "rear-light"}
WRITER_POLICY = "forgecad-runtime-only-state-writer@1"
CANONICALIZATION_POLICY = "canonical-json-sha256-excluding-canonical-sha256@1"


def f32(value: float) -> float:
    return struct.unpack("!f", struct.pack("!f", value))[0]


def weights() -> list[dict[str, Any]]:
    rows = []
    for part_id in PART_IDS:
        if part_id in FPS_PRIORITY:
            first_person, world, hidden = 1.0, 0.9, 0.02
        elif part_id in REAR_PRIORITY:
            first_person, world, hidden = 0.75, 1.0, 0.04
        else:
            first_person, world, hidden = 0.85, 0.95, 0.03
        rows.append({
            "part_id": part_id,
            "first_person": first_person,
            "world": world,
            "hidden": hidden,
        })
    return rows


def prepare_input_hash(request: dict[str, Any]) -> str:
    preimage = dict(request)
    preimage.pop("input_sha256", None)
    preimage.pop("idempotency_key", None)
    normalized = []
    for row in sorted(preimage["visibility_weights"], key=lambda item: item["part_id"]):
        normalized.append({
            "part_id": row["part_id"],
            "first_person": f32(row["first_person"]),
            "world": f32(row["world"]),
            "hidden": f32(row["hidden"]),
        })
    preimage["visibility_weights"] = normalized
    return canonical_hash(preimage)


def get_input_hash(request: dict[str, Any]) -> str:
    preimage = dict(request)
    preimage["input_sha256"] = ""
    return canonical_hash(preimage)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--expected-build-cohort", required=True)
    parser.add_argument("--timeout", type=float, default=360.0)
    return parser.parse_args()


def open_bound_client(args: argparse.Namespace, endpoint: Path):
    runtime, ready_path, ready, client = open_client(
        args.mcp, args.runtime, args.data_root, endpoint, args.timeout
    )
    capabilities = client.tool("capabilities_get")
    require(
        capabilities.get("build_cohort_sha256") == args.expected_build_cohort,
        "Runtime/MCP build cohort differs",
    )
    return runtime, ready_path, ready, client, capabilities


def main() -> int:
    args = parse_args()
    repository = Path(__file__).resolve().parents[1]
    output = args.evidence if args.evidence.is_absolute() else repository / args.evidence
    output.resolve().relative_to((repository / "docs" / "evidence").resolve())

    request: dict[str, Any] = {
        "schema_version": "HeroUvDurablePrepareRequest@1",
        "project_id": PROJECT_ID,
        "candidate_id": CANDIDATE_ID,
        "candidate_state_sha256": CANDIDATE_STATE,
        "base_version_id": None,
        "source_low_artifact_id": LOW_ARTIFACT,
        "source_low_artifact_object_sha256": LOW_ARTIFACT,
        "source_low_artifact_sha256": LOW_ARTIFACT,
        "source_low_artifact_readback_object_sha256": LOW_READBACK_OBJECT,
        "source_low_artifact_readback_sha256": LOW_READBACK_CANONICAL,
        "resolution": 4096,
        "padding_texels": 32,
        "min_mip_level": 5,
        "hard_edge_angle_deg": 60.0,
        "stretch_threshold": 32.0,
        "visibility_weights": weights(),
        "idempotency_key": "fps-production-04bi-hero-uv-4096-v1",
        "max_response_bytes": 8_388_608,
        "source_only": True,
        "runtime_write_performed": False,
        "writer_policy": WRITER_POLICY,
        "canonicalization_policy": CANONICALIZATION_POLICY,
        "input_sha256": "",
    }
    request["input_sha256"] = prepare_input_hash(request)

    with tempfile.TemporaryDirectory(prefix="forgecad-04bi-", dir="/tmp") as temporary:
        temporary_root = Path(temporary)
        runtime, ready_path, ready, client, capabilities = open_bound_client(
            args, temporary_root / "prepare"
        )
        try:
            source = client.tool("candidate_get", {"candidate_id": CANDIDATE_ID})
            require(source.get("canonical_sha256") == CANDIDATE_STATE, "candidate state differs")
            prepared = client.tool("hero_uv_durable_prepare", request)
            require(prepared.get("resolution") == 4096, "Hero UV resolution differs")
            require(prepared.get("replay_byte_exact") is True, "Hero UV replay differs")
            require(prepared.get("runtime_write_performed") is True, "Runtime did not persist Hero UV")
            require(prepared.get("production_stage_advanced") is False, "Hero UV advanced production stage")
        finally:
            close_client(runtime, ready_path, ready, client)

        get_request: dict[str, Any] = {
            "schema_version": "HeroUvDurableGetRequest@1",
            "operation": "forgecad.production.hero-uv-durable-get@1",
            "project_id": PROJECT_ID,
            "candidate_id": CANDIDATE_ID,
            "candidate_state_sha256": CANDIDATE_STATE,
            "base_version_id": None,
            "source_low_artifact_id": LOW_ARTIFACT,
            "source_low_artifact_sha256": LOW_ARTIFACT,
            "layout_object_sha256": prepared["layout_object_sha256"],
            "layout_canonical_sha256": prepared["layout_canonical_sha256"],
            "link_id": prepared["link_id"],
            "link_object_sha256": prepared["link_object_sha256"],
            "resolution": prepared["resolution"],
            "padding_texels": prepared["padding_texels"],
            "min_mip_level": prepared["min_mip_level"],
            "hard_edge_angle_deg": prepared["hard_edge_angle_deg"],
            "stretch_threshold": prepared["stretch_threshold"],
            "visibility_weights_sha256": prepared["visibility_weights_sha256"],
            "idempotency_key": prepared["idempotency_key"],
            "source_only": True,
            "writer_policy": WRITER_POLICY,
            "runtime_write_performed": False,
            "persistent_user_data_touched": False,
            "input_sha256": "",
        }
        get_request["input_sha256"] = get_input_hash(get_request)
        runtime, ready_path, ready, client, _ = open_bound_client(
            args, temporary_root / "readback"
        )
        try:
            readback = client.tool("hero_uv_durable_get", get_request)
            require(readback.get("replayed") is True, "Hero UV restart readback did not replay")
            require(readback.get("restart_hash_verified") is True, "Hero UV restart hash differs")
            require(readback.get("layout_object_sha256") == prepared.get("layout_object_sha256"), "layout object differs after restart")
            require(readback.get("link_object_sha256") == prepared.get("link_object_sha256"), "link object differs after restart")
        finally:
            close_client(runtime, ready_path, ready, client)

    layout = prepared["layout"]
    evidence = {
        "schema_version": "ForgeCADWeaponHeroUvDurableEvidence@1",
        "task_id": "FPS-PRODUCTION-04BI-HERO-UV-DURABLE",
        "recorded_at": "2026-08-29",
        "scope": "fictional game and film visual asset only",
        "build_cohort_sha256": capabilities["build_cohort_sha256"],
        "source": {
            "project_id": PROJECT_ID,
            "candidate_id": CANDIDATE_ID,
            "candidate_state_sha256": CANDIDATE_STATE,
            "low_artifact_sha256": LOW_ARTIFACT,
            "low_readback_object_sha256": LOW_READBACK_OBJECT,
            "low_readback_canonical_sha256": LOW_READBACK_CANONICAL,
        },
        "hero_uv": {
            "resolution": prepared["resolution"],
            "padding_texels": prepared["padding_texels"],
            "min_mip_level": prepared["min_mip_level"],
            "hard_edge_angle_deg": prepared["hard_edge_angle_deg"],
            "stretch_threshold": prepared["stretch_threshold"],
            "part_weight_count": len(prepared["visibility_weights"]),
            "island_count": len(layout.get("islands", [])),
            "metrics": layout.get("metrics"),
            "mikk_replay": layout.get("mikk_replay"),
            "layout_object_sha256": prepared["layout_object_sha256"],
            "layout_canonical_sha256": prepared["layout_canonical_sha256"],
            "link_id": prepared["link_id"],
            "link_object_sha256": prepared["link_object_sha256"],
            "visibility_weights_sha256": prepared["visibility_weights_sha256"],
            "worker_build_cohort_sha256": prepared["worker_build_cohort_sha256"],
            "replay_count": prepared["replay_count"],
            "replay_byte_exact": prepared["replay_byte_exact"],
            "restart_hash_verified": readback["restart_hash_verified"],
        },
        "truth": {
            "runtime_durable_hero_uv_4k": "PASS_SOURCE_STRUCTURAL_RESTART_VERIFIED",
            "artist_unwrap_review": "NOT_RUN",
            "formal_high_low_geometric_bake": "NOT_RUN",
            "visual_status": prepared["visual_status"],
            "human_status": prepared["human_status"],
            "engine_status": prepared["engine_status"],
            "promotion_allowed": False,
        },
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(evidence, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
