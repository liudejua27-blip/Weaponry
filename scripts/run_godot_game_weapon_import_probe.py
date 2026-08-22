#!/usr/bin/env python3
"""Run the fixed external Godot 4.7.2 GLB import/readback evidence gate.

This harness is deliberately outside ForgeCAD Runtime truth. It creates an isolated
temporary Godot project, imports only the hash-bound GLBs emitted by the ignored
Runtime probes, and emits a sanitized structural receipt. It never writes Runtime
SQLite/CAS and never persists Godot's imported cache.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
GDSCRIPT = ROOT / "scripts/check_godot_game_weapon_import_probe.gd"
THREEJS_CHECKER = ROOT / "scripts/check_threejs_game_weapon_animated_glb_socket_probe.mjs"
EXPECTED_GODOT_SHA256 = "c7cccbf8fb143e34e02fd6521e09be2c2b974f0d5db080b19071c9c570718ccf"
EXPECTED_GODOT_VERSION = "4.7.2.stable.official.ed1daf0bf"
EXPECTED_TEAM_ID = "6K46PWY5DM"
EXPECTED_IDENTIFIER = "org.godotengine.godot"
SOCKET_NAMES = [
    "forgecad-anchor-grip-primary",
    "forgecad-anchor-socket-energy-core-vfx",
    "forgecad-anchor-socket-magazine-well",
    "forgecad-anchor-socket-muzzle-vfx",
    "forgecad-anchor-socket-sight-primary",
    "forgecad-anchor-weapon-root",
]
FOLLOW_NAMES = [
    "forgecad-anchor-socket-energy-core-vfx",
    "forgecad-anchor-socket-magazine-well",
]
LIMITATIONS = [
    "no-ballistics",
    "no-damage-or-hitbox",
    "no-physics-simulation",
    "no-manufacturing-or-operation",
    "no-commercial-engine-roundtrip",
    "no-visual-quality-pass",
    "static-and-animated-assets-come-from-independent-runtime-source-cohorts",
    "godot-headless-import-is-structural-only",
]


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def sha_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def numeric_array_near(left: list[Any], right: list[Any], tolerance: float = 1e-6) -> bool:
    return len(left) == len(right) and all(abs(float(a) - float(b)) <= tolerance for a, b in zip(left, right))


def quaternion_near(left: list[Any], right: list[Any]) -> bool:
    return numeric_array_near(left, right) or numeric_array_near(left, [-float(value) for value in right])


def load_object(path: Path, label: str) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    require(isinstance(value, dict), f"{label} must be a JSON object")
    return value


def decode_exact(value: str, expected_sha256: str, label: str) -> bytes:
    require(isinstance(value, str) and value, f"{label} base64 is missing")
    try:
        data = base64.b64decode(value, validate=True)
    except Exception as exc:  # pragma: no cover - fail-closed diagnostic
        raise RuntimeError(f"{label} base64 is invalid") from exc
    require(sha_bytes(data) == expected_sha256, f"{label} bytes do not match Runtime hash")
    require(data[:4] == b"glTF", f"{label} is not a binary glTF container")
    return data


def run_checked(
    argv: list[str], *, cwd: Path, env: dict[str, str], timeout: int, label: str
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
        check=False,
    )
    if result.returncode != 0:
        diagnostic = (result.stderr + "\n" + result.stdout)[-4000:].replace(str(cwd), "<isolated-project>")
        raise RuntimeError(f"{label} failed with exit code {result.returncode}: {diagnostic}")
    return result


def check_godot_binary(godot: Path) -> None:
    require(godot.is_file(), "Godot executable does not exist")
    require(sha_file(godot) == EXPECTED_GODOT_SHA256, "Godot executable SHA-256 is not pinned 4.7.2 build")
    version = subprocess.run(
        [str(godot), "--version"], text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=20
    )
    require(version.returncode == 0, "Godot --version failed")
    require(version.stdout.strip() == EXPECTED_GODOT_VERSION, "Godot version/build cohort differs")
    if sys.platform == "darwin":
        verified = subprocess.run(
            ["/usr/bin/codesign", "--verify", "--deep", "--strict", str(godot)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )
        require(verified.returncode == 0, "Godot code signature verification failed")
        detail = subprocess.run(
            ["/usr/bin/codesign", "-dvv", str(godot)],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )
        signature = detail.stdout + detail.stderr
        require(detail.returncode == 0, "Godot signature metadata readback failed")
        require(f"Identifier={EXPECTED_IDENTIFIER}" in signature, "Godot signing identifier differs")
        require(f"TeamIdentifier={EXPECTED_TEAM_ID}" in signature, "Godot signing team differs")


def socket_expectations(static_probe: dict[str, Any]) -> list[dict[str, Any]]:
    levels = static_probe.get("levels")
    require(isinstance(levels, list) and len(levels) == 3, "static probe must contain exact three LOD readbacks")
    rows = levels[0].get("socket_nodes")
    require(isinstance(rows, list) and len(rows) == 6, "static probe must contain exact six sockets")
    result = []
    for row in rows:
        result.append(
            {
                "node_name": row["node_name"],
                "parent_node_name": row["parent_node_name"],
                "local_translation_m": row["local_translation_m"],
                "local_rotation_quat_xyzw": row["local_rotation_quat_xyzw"],
                "local_scale_xyz": row["local_scale_xyz"],
            }
        )
    result.sort(key=lambda row: row["node_name"])
    require([row["node_name"] for row in result] == SOCKET_NAMES, "socket name inventory differs")
    return result


def parse_godot_report(stdout: str) -> dict[str, Any]:
    for line in reversed(stdout.splitlines()):
        line = line.strip()
        if line.startswith("{"):
            value = json.loads(line)
            if value.get("schema_version") == "GodotGameWeaponImportProbeReport@1":
                return value
    raise RuntimeError("Godot probe emitted no typed JSON report")


def sanitize_scene(
    raw: dict[str, Any], artifact_sha256: str, expected_sockets: dict[str, dict[str, Any]]
) -> dict[str, Any]:
    role = raw["role"]
    require(raw.get("load") == "PASS", f"Godot failed to load {role}")
    require(raw.get("mesh_count") == 5, f"Godot {role} mesh count differs")
    raw_sockets = raw.get("sockets")
    require(isinstance(raw_sockets, list) and len(raw_sockets) == 6, f"Godot {role} socket count differs")
    nodes = []
    for socket in sorted(raw_sockets, key=lambda item: item["name"]):
        expected = expected_sockets[socket["name"]]
        node = {
            "node_name": socket["name"],
            "parent_node_name": socket["parent"],
            "node_kind": socket["class"],
            "local_translation_m": socket["position"],
            "local_rotation_quat_xyzw": socket["quaternion_xyzw"],
            "local_scale_xyz": socket["scale"],
            "non_rendering": socket["non_rendering"],
            "parent_local_trs_exact": (
                socket["parent"] == expected["parent_node_name"]
                and numeric_array_near(socket["position"], expected["local_translation_m"])
                and quaternion_near(socket["quaternion_xyzw"], expected["local_rotation_quat_xyzw"])
                and numeric_array_near(socket["scale"], expected["local_scale_xyz"])
            ),
        }
        require(node["node_kind"] == "Node3D" and node["non_rendering"] is True, f"Godot {role} socket is renderable")
        require(node["parent_local_trs_exact"] is True, f"Godot {role} socket parent/local TRS differs")
        nodes.append(node)
    require([node["node_name"] for node in nodes] == SOCKET_NAMES, f"Godot {role} socket inventory differs")
    material_names = sorted(set(raw.get("material_names", [])))
    require(material_names, f"Godot {role} material inventory is empty")
    raw_animation = raw.get("animation", {})
    animated = role == "animated"
    if animated:
        require(raw_animation.get("source_gltf_channel_count") == 10, "source glTF animation channel count differs")
        require(raw_animation.get("godot_optimized_track_count") == 2, "Godot optimized track count differs")
        require(raw_animation.get("semantic_sampling_exact") is True, "cross-loader semantic sampling differs")
        follow = raw_animation.get("half_duration_follow_names")
        require(follow == FOLLOW_NAMES, "animated socket follower inventory differs")
        animation_core = {
            "animation_status": "rigid-gltf-animation",
            "source_gltf_channel_count": 10,
            "godot_optimized_track_count": 2,
            "cross_loader_semantic_sampling_exact": True,
            "half_duration_follow_names": FOLLOW_NAMES,
        }
        animation_projection_sha256: str | None = sha_bytes(canonical_bytes(animation_core))
    else:
        require(raw_animation.get("animation_player_count") == 0, f"Godot {role} unexpectedly imported animation")
        animation_core = {
            "animation_status": "absent",
            "source_gltf_channel_count": 0,
            "godot_optimized_track_count": 0,
            "cross_loader_semantic_sampling_exact": True,
            "half_duration_follow_names": [],
        }
        animation_projection_sha256 = None
    animation = {**animation_core, "animation_projection_sha256": animation_projection_sha256}
    mesh_projection = {"mesh_count": 5, "triangle_count": raw["triangle_count"]}
    material_projection = {"material_count": len(material_names), "material_names": material_names}
    socket_inventory_sha = sha_bytes(canonical_bytes(nodes))
    trs_projection_sha = sha_bytes(
        canonical_bytes(
            [
                {
                    "node_name": node["node_name"],
                    "parent_node_name": node["parent_node_name"],
                    "translation": node["local_translation_m"],
                    "rotation": node["local_rotation_quat_xyzw"],
                    "scale": node["local_scale_xyz"],
                }
                for node in nodes
            ]
        )
    )
    projection = {
        "scene_role": role,
        "scene_artifact_sha256": artifact_sha256,
        "mesh_count": 5,
        "triangle_count": raw["triangle_count"],
        "material_count": len(material_names),
        "material_names": material_names,
        "material_signature_sha256": sha_bytes(canonical_bytes(material_names)),
        "mesh_projection_sha256": sha_bytes(canonical_bytes(mesh_projection)),
        "material_projection_sha256": sha_bytes(canonical_bytes(material_projection)),
        "socket_node_count": 6,
        "socket_node_inventory_sha256": socket_inventory_sha,
        "socket_nodes": nodes,
        "scale_trs_projection_sha256": trs_projection_sha,
        "animation": animation,
    }
    projection["scene_projection_sha256"] = sha_bytes(canonical_bytes(projection))
    return projection


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--godot", required=True, type=Path)
    parser.add_argument("--static-probe", required=True, type=Path)
    parser.add_argument("--animated-probe", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    check_godot_binary(args.godot.resolve())
    static_probe = load_object(args.static_probe.resolve(), "static Runtime probe")
    animated_probe = load_object(args.animated_probe.resolve(), "animated Runtime probe")
    require(static_probe.get("schema_version") == "GameWeaponGlbSocketThreeJsProbe@1", "static Runtime probe schema differs")
    require(animated_probe.get("schema_version") == "GameWeaponAnimatedGlbSocketThreeJsProbe@1", "animated Runtime probe schema differs")
    require(static_probe.get("restart_hash_verified") is True, "static Runtime restart readback is not verified")
    require(animated_probe.get("restart_hash_verified") is True, "animated Runtime restart readback is not verified")
    levels = sorted(static_probe["levels"], key=lambda level: level["lod_level"])
    require([level["lod_level"] for level in levels] == [0, 1, 2], "static LOD levels differ")
    static_hashes = [level["derived_artifact_sha256"] for level in levels]
    require(len(static_probe.get("lod_glb_base64s", [])) == 3, "static Runtime probe has no exact three GLBs")
    static_glbs = [
        decode_exact(value, expected, f"static LOD{index}")
        for index, (value, expected) in enumerate(zip(static_probe["lod_glb_base64s"], static_hashes))
    ]
    animated_receipt = animated_probe.get("receipt", {})
    animated_hash = animated_receipt.get("derived_animated_socket_artifact_sha256")
    animated_glb = decode_exact(
        animated_probe.get("derived_animated_socket_glb_base64"), animated_hash, "animated socket GLB"
    )
    expected_socket_rows = socket_expectations(static_probe)
    expected_sockets = {row["node_name"]: row for row in expected_socket_rows}

    node = shutil.which("node")
    require(node is not None, "Node.js is required for cross-loader semantic samples")
    three = subprocess.run(
        [node, str(THREEJS_CHECKER), str(args.animated_probe.resolve())],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=60,
        check=False,
    )
    require(three.returncode == 0, f"Three.js semantic sample probe failed with exit code {three.returncode}")
    three_report = json.loads(three.stdout)
    require(three_report.get("animation_track_count") == 10, "Three.js source channel projection differs")
    require(three_report.get("animated_socket_names") == FOLLOW_NAMES, "Three.js socket follower projection differs")

    harness_sources = {
        "python_sha256": sha_file(Path(__file__).resolve()),
        "gdscript_sha256": sha_file(GDSCRIPT),
        "threejs_checker_sha256": sha_file(THREEJS_CHECKER),
    }
    harness_sha = sha_bytes(canonical_bytes(harness_sources))
    binding = {
        "static_delivery": static_probe["delivery_manifest_object_sha256"],
        "static_socket": static_probe["socket_materialization_key_sha256"],
        "static_lods": static_hashes,
        "animated_delivery": animated_receipt["delivery_manifest_object_sha256"],
        "animated_socket": animated_probe["animated_socket_materialization_key_sha256"],
        "animated_artifact": animated_hash,
        "collision": static_probe["collision_proxy_set"]["canonical_sha256"],
        "godot": EXPECTED_GODOT_SHA256,
        "harness": harness_sha,
    }
    import_key = sha_bytes(canonical_bytes(binding))

    with tempfile.TemporaryDirectory(prefix="forgecad-godot-import-probe-") as temp_name:
        temp = Path(temp_name)
        project = temp / "project"
        assets = project / "assets"
        isolated_home = temp / "home"
        isolated_tmp = temp / "tmp"
        assets.mkdir(parents=True)
        isolated_home.mkdir()
        isolated_tmp.mkdir()
        (project / "project.godot").write_text(
            '[application]\nconfig/name="ForgeCAD Godot Import Evidence"\n[rendering]\nrenderer/rendering_method="gl_compatibility"\n',
            encoding="utf-8",
        )
        for index, data in enumerate(static_glbs):
            (assets / f"weapon-lod{index}.glb").write_bytes(data)
        (assets / "weapon-animated.glb").write_bytes(animated_glb)
        (assets / "socket-expectations.json").write_bytes(canonical_bytes(expected_socket_rows))
        (assets / "collision-proxy-set.json").write_bytes(canonical_bytes(static_probe["collision_proxy_set"]))
        (assets / "socket-animation-samples.json").write_bytes(canonical_bytes(three_report))
        shutil.copyfile(GDSCRIPT, project / "probe.gd")
        env = dict(os.environ)
        env.update({"HOME": str(isolated_home), "TMPDIR": str(isolated_tmp)})
        import_run = run_checked(
            [str(args.godot.resolve()), "--headless", "--recovery-mode", "--path", str(project), "--import"],
            cwd=project,
            env=env,
            timeout=180,
            label="Godot headless import",
        )
        imported = project / ".godot/imported"
        inventory = []
        require(imported.is_dir(), "Godot did not create imported resource cache")
        for path in sorted(imported.rglob("*")):
            if path.is_file():
                inventory.append({"relative_name": path.relative_to(imported).as_posix(), "sha256": sha_file(path), "size": path.stat().st_size})
        require(inventory, "Godot imported resource inventory is empty")
        probe_run = run_checked(
            [str(args.godot.resolve()), "--headless", "--path", str(project), "--script", "res://probe.gd"],
            cwd=project,
            env=env,
            timeout=120,
            label="Godot imported PackedScene readback",
        )
        raw_report = parse_godot_report(probe_run.stdout)
        require(raw_report.get("failures") == [], "Godot structural probe reported failures")
        require(raw_report.get("actual_godot_headless_import") is True, "Godot import did not pass")
        require(raw_report.get("commercial_engine_roundtrip") is False, "commercial engine boundary drifted")
        report_sanitized = dict(raw_report)
        report_sanitized["scenes"] = [
            {key: value for key, value in scene.items() if key != "resource_path"}
            for scene in raw_report["scenes"]
        ]
        godot_report_sha = sha_bytes(canonical_bytes(report_sanitized))

    raw_scenes = raw_report["scenes"]
    require([scene["role"] for scene in raw_scenes] == ["lod0", "lod1", "lod2", "animated"], "Godot scene order differs")
    scene_hashes = [*static_hashes, animated_hash]
    scenes = [
        sanitize_scene(raw, artifact, expected_sockets)
        for raw, artifact in zip(raw_scenes, scene_hashes)
    ]
    lod_triangles = [scene["triangle_count"] for scene in scenes[:3]]
    require(lod_triangles[0] > lod_triangles[1] > lod_triangles[2], "Godot LOD triangles are not strictly decreasing")
    require(len({scene["material_signature_sha256"] for scene in scenes[:3]}) == 1, "Godot LOD material signatures differ")
    collision = raw_report["collision"]
    require(collision.get("source_proxy_count") == 5, "collision source proxy count differs")
    require(collision.get("godot_collision_shape_count") == 5, "Godot collision shape count differs")
    require(collision.get("aabb_sidecar_readback") == "PASS", "Godot collision sidecar readback differs")
    require({row["shape"] for row in collision.get("rows", [])} == {"BoxShape3D"}, "Godot collision shape kind differs")

    receipt: dict[str, Any] = {
        "schema_version": "GodotGameWeaponImportReceipt@1",
        "godot_game_weapon_import_key_sha256": import_key,
        "static_project_id": static_probe["collision_proxy_set"]["project_id"],
        "static_delivery_manifest_object_sha256": static_probe["delivery_manifest_object_sha256"],
        "static_socket_materialization_key_sha256": static_probe["socket_materialization_key_sha256"],
        "static_lod0_derived_artifact_sha256": static_hashes[0],
        "static_lod1_derived_artifact_sha256": static_hashes[1],
        "static_lod2_derived_artifact_sha256": static_hashes[2],
        "animated_socket_materialization_key_sha256": animated_probe["animated_socket_materialization_key_sha256"],
        "animated_project_id": animated_receipt["project_id"],
        "animated_delivery_manifest_object_sha256": animated_receipt["delivery_manifest_object_sha256"],
        "animated_derived_artifact_sha256": animated_hash,
        "collision_proxy_set_canonical_sha256": static_probe["collision_proxy_set"]["canonical_sha256"],
        "godot_binary_sha256": EXPECTED_GODOT_SHA256,
        "godot_version": EXPECTED_GODOT_VERSION,
        "godot_build_sha256": EXPECTED_GODOT_SHA256,
        "probe_harness_sha256": harness_sha,
        "harness_policy": "first-party-fixed-godot-headless-import-probe@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-canonical-sha256-and-created-at@1",
        "godot_process_exit_code": 0,
        "godot_report_sha256": godot_report_sha,
        "scene_count": 4,
        "scene_projections": scenes,
        "lod_triangle_counts": lod_triangles,
        "lod_triangles_strictly_decreasing": True,
        "lod_material_signatures_exact": True,
        "source_gltf_channel_count": 10,
        "godot_optimized_track_count": 2,
        "cross_loader_semantic_sampling_exact": True,
        "half_duration_follow_names": FOLLOW_NAMES,
        "collision_proxy_count": 5,
        "godot_collision_shape_count": 5,
        "collision_shape_kind": "BoxShape3D",
        "collision_aabb_sidecar_readback_exact": True,
        "collision_physics_simulation": "NOT_RUN",
        "hitbox_semantics": False,
        "actual_godot_headless_import": True,
        "actual_engine_roundtrip": True,
        "commercial_engine_roundtrip": False,
        "unity_status": "NOT_RUN",
        "unreal_status": "NOT_RUN",
        "candidate_confirmed": False,
        "export_performed": False,
        "human_review": "NOT_RUN",
        "visual_quality_status": "NOT_RUN",
        "quality_status": "structural_only",
        "semantic_scope": "fictional-nonfunctional-game-visual-authoring-only@1",
        "functional_semantics": False,
        "limitations": LIMITATIONS,
        "created_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    }
    canonical_projection = {key: value for key, value in receipt.items() if key != "created_at"}
    receipt["canonical_sha256"] = sha_bytes(canonical_bytes(canonical_projection))
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(canonical_bytes(receipt) + b"\n")
    print(
        json.dumps(
            {
                "status": "PASS_EXTERNAL_GODOT_IMPORT_STRUCTURAL",
                "receipt_sha256": sha_file(args.output),
                "canonical_sha256": receipt["canonical_sha256"],
                "scene_count": 4,
                "lod_triangle_counts": lod_triangles,
                "godot_version": EXPECTED_GODOT_VERSION,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, OSError, ValueError, KeyError, subprocess.TimeoutExpired) as exc:
        print(f"Godot import probe failed: {exc}", file=sys.stderr)
        raise SystemExit(1)
