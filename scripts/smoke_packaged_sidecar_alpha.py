#!/usr/bin/env python3
"""Verify a frozen sidecar's first run, editable GLB export, and recovery."""

from __future__ import annotations

import hashlib
import json
import os
import signal
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from base64 import b64decode
from pathlib import Path
from typing import Any

from forgecad_agent.application.geometry_worker import read_shape_program_glb_facts
from forgecad_agent.application.manifold_csg import MANIFOLD_KERNEL_ID, MANIFOLD_KERNEL_VERSION
from smoke_g825_feature_csg import boolean_program


ROOT = Path(__file__).resolve().parents[1]
SIDECAR = ROOT / "apps/desktop/src-tauri/binaries/wushen-agent-aarch64-apple-darwin"


def main() -> int:
    if not SIDECAR.is_file() or SIDECAR.stat().st_size == 0:
        raise AssertionError("frozen macOS arm64 sidecar is required before the packaged Alpha smoke")
    port = _free_port()
    with tempfile.TemporaryDirectory(prefix="forgecad_packaged_alpha_") as temporary:
        library_root = Path(temporary) / "library"
        environment = _safe_environment(library_root)
        process = _start_sidecar(temporary, environment, port)
        try:
            payload = _wait_for_health(port, process)
            _assert(payload == {"status": "ok", "service": "wushen-agent", "mode": "sqlite_mock"}, "unexpected health payload")
            _assert((library_root / "library.db").is_file(), "first initialization did not create the library")
            asset_version_id, export_glb_sha256 = _create_editable_asset_with_navigation(port, library_root)
        finally:
            _stop_sidecar(process)

        restart_port = _free_port()
        restarted = _start_sidecar(temporary, environment, restart_port)
        restarted_stopped = False
        try:
            try:
                _wait_for_health(restart_port, restarted)
            except AssertionError as error:
                _stop_sidecar(restarted)
                restarted_stopped = True
                stderr = restarted.stderr.read() if restarted.stderr is not None else ""
                raise AssertionError(f"restart health failed: {error}; sidecar stderr: {stderr[-2000:]}") from error
            recovered = _request(restart_port, f"/api/v1/agent/asset-versions/{asset_version_id}")
            _assert(recovered["asset_version_id"] == asset_version_id, "restart did not restore the saved editable asset")
            exported = _request(restart_port, f"/api/v1/agent/asset-versions/{asset_version_id}:export", method="POST")
            recovered_glb = b64decode(exported["glb_base64"])
            _assert(recovered_glb[:4] == b"glTF", "recovered GLB export is invalid")
            _assert(
                hashlib.sha256(recovered_glb).hexdigest() == export_glb_sha256,
                "restart changed the packaged PBR GLB export",
            )
        finally:
            if not restarted_stopped:
                _stop_sidecar(restarted)
    print(json.dumps({
        "ok": True,
        "packaged_sidecar_health": True,
        "empty_library_initialized": True,
        "editable_glb_export": True,
        "packaged_manifold_csg": True,
        "packaged_visual_pbr_readback": True,
        "packaged_undo_redo_export": True,
        "restart_recovery": True,
        "provider_calls": 0,
    }))
    return 0


def _safe_environment(library_root: Path) -> dict[str, str]:
    environment = os.environ.copy()
    for name in ("FORGECAD_AGENT_PROVIDER", "FORGECAD_AGENT_BASE_URL", "FORGECAD_AGENT_MODEL", "FORGECAD_AGENT_API_KEY"):
        environment.pop(name, None)
    environment["WUSHEN_LIBRARY_ROOT"] = str(library_root)
    environment["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"
    environment["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
    return environment


def _start_sidecar(temporary: str, environment: dict[str, str], port: int) -> subprocess.Popen[str]:
    return subprocess.Popen(
        [str(SIDECAR), "agent", "serve", "--host", "127.0.0.1", "--port", str(port)],
        cwd=temporary,
        env=environment,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        # Manifold uses a bounded child process.  A frozen-server smoke must
        # own and stop that whole process group before its restart assertion.
        start_new_session=True,
    )


def _stop_sidecar(process: subprocess.Popen[str]) -> None:
    if process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=10)


def _create_editable_asset_with_navigation(port: int, library_root: Path) -> tuple[str, str]:
    project = _request(
        port,
        "/api/v1/projects",
        method="POST",
        idempotency_key="p002-project",
        body={
            "client_request_id": "p002-project",
            "name": "P002 本机 Alpha 概念道具",
            "intended_uses": ["game_asset", "film_prop", "non_functional_display"],
            "style": {"keywords": ["fictional", "mechanical", "concept"], "palette": ["graphite"], "detail_density": 0.35},
            "proportions": {"overall_length_mm": 210, "body_height_mm": 50, "grip_angle_deg": 10},
            "constraints": {"symmetry": "mostly_symmetric", "max_triangle_count": 100000},
            "assumptions": ["虚构非功能概念道具；不用于真实制造、使用或性能判断。"],
        },
    )
    project_id = project["project_id"]
    thread = _request(
        port,
        "/api/v1/agent/threads",
        method="POST",
        idempotency_key="p002-thread",
        body={"client_request_id": "p002-thread", "project_id": project_id, "title": "本机 Alpha 概念", "provider_id": "deterministic_kernel"},
    )
    turn = _request(
        port,
        f"/api/v1/agent/threads/{thread['thread_id']}/turns",
        method="POST",
        idempotency_key="p002-turn",
        body={"client_request_id": "p002-turn", "message": "设计一台三关节机械臂。"},
    )
    plan = _extract_plan_from_turn(turn)
    direction_id = plan["directions"][0]["direction_id"]
    built = _request(
        port,
        "/api/v1/agent/blockouts",
        method="POST",
        idempotency_key="p002-build",
        body={
            "client_request_id": "p002-build",
            "plan": plan,
            "direction_id": direction_id,
            "presentation_profile": "showcase",
        },
    )
    segmented = _request(
        port,
        "/api/v1/agent/blockouts:segment",
        method="POST",
        idempotency_key="p002-segment",
        body={
            "client_request_id": "p002-segment",
            "plan": plan,
            "direction_id": direction_id,
            "artifact_id": built["artifact_id"],
            "presentation_profile": "showcase",
        },
    )
    committed = _request(
        port,
        "/api/v1/agent/blockouts:commit",
        method="POST",
        idempotency_key="p002-commit",
        body={"client_request_id": "p002-commit", "project_id": project_id, "artifact_id": segmented["artifact_id"], "summary": "P002 本机 Alpha 可编辑概念资产"},
    )
    initial_asset_version_id = committed["asset_version_id"]
    initial_export = _request(port, f"/api/v1/agent/asset-versions/{initial_asset_version_id}:export", method="POST")
    initial_glb = b64decode(initial_export["glb_base64"])
    _assert_packaged_visual_pbr(initial_glb)
    initial_glb_sha256 = hashlib.sha256(initial_glb).hexdigest()
    asset = _request(port, f"/api/v1/agent/asset-versions/{initial_asset_version_id}")
    target = next(item for item in asset["parts"] if item["editable_parameter_bindings"])
    binding = target["editable_parameter_bindings"][0]
    candidate_value = float(binding["min"]) + float(binding["step"])
    if abs(candidate_value - float(binding["default"])) <= 1e-9:
        candidate_value += float(binding["step"])
    _assert(candidate_value <= float(binding["max"]), "packaged fixture has no editable parameter value distinct from its default")
    proposed = _request(
        port,
        f"/api/v1/agent/asset-versions/{initial_asset_version_id}/change-sets",
        method="POST",
        idempotency_key="p002-propose-edit",
        body={
            "client_request_id": "p002-propose-edit",
            "summary": "P002 packaged undo redo PBR export fixture",
            "operations": [{
                "operation_id": "op_p002_adjust_parameter",
                "op": "set_part_parameter",
                "part_id": target["part_id"],
                "path": binding["path"],
                "value": candidate_value,
            }],
        },
    )
    previewed = _request(
        port,
        f"/api/v1/agent/change-sets/{proposed['change_set_id']}:preview",
        method="POST",
        idempotency_key="p002-preview-edit",
    )
    _assert(previewed["status"] == "previewed", "packaged editable PBR preview did not enter previewed state")
    confirmed = _request(
        port,
        f"/api/v1/agent/change-sets/{proposed['change_set_id']}:confirm",
        method="POST",
        idempotency_key="p002-confirm-edit",
    )
    edited_asset_version_id = confirmed["asset_version"]["asset_version_id"]
    edited_export = _request(port, f"/api/v1/agent/asset-versions/{edited_asset_version_id}:export", method="POST")
    edited_glb = b64decode(edited_export["glb_base64"])
    _assert_packaged_visual_pbr(edited_glb)
    edited_glb_sha256 = hashlib.sha256(edited_glb).hexdigest()
    _assert(edited_glb_sha256 != initial_glb_sha256, "packaged parameter edit did not change its GLB export")

    active = _request(port, f"/api/v1/projects/{project_id}/active-design")
    undone = _request(
        port,
        f"/api/v1/projects/{project_id}/active-design:undo",
        method="POST",
        idempotency_key="p002-undo",
        body={"client_request_id": "p002-undo", "snapshot_revision": active["revision"]},
    )
    undone_asset_version_id = undone["active_design"]["asset_version_id"]
    _assert(undone_asset_version_id not in {initial_asset_version_id, edited_asset_version_id}, "undo must create an immutable navigation child")
    undone_export = _request(port, f"/api/v1/agent/asset-versions/{undone_asset_version_id}:export", method="POST")
    undone_glb = b64decode(undone_export["glb_base64"])
    _assert_packaged_visual_pbr(undone_glb)
    _assert(hashlib.sha256(undone_glb).hexdigest() == initial_glb_sha256, "packaged undo did not restore the initial PBR GLB")

    redone = _request(
        port,
        f"/api/v1/projects/{project_id}/active-design:redo",
        method="POST",
        idempotency_key="p002-redo",
        body={"client_request_id": "p002-redo", "snapshot_revision": undone["revision"]},
    )
    asset_version_id = redone["active_design"]["asset_version_id"]
    _assert(asset_version_id not in {initial_asset_version_id, edited_asset_version_id, undone_asset_version_id}, "redo must create an immutable navigation child")
    redone_export = _request(port, f"/api/v1/agent/asset-versions/{asset_version_id}:export", method="POST")
    redone_glb = b64decode(redone_export["glb_base64"])
    _assert_packaged_visual_pbr(redone_glb)
    _assert(hashlib.sha256(redone_glb).hexdigest() == edited_glb_sha256, "packaged redo did not restore the edited PBR GLB")

    current_asset = _request(port, f"/api/v1/agent/asset-versions/{asset_version_id}")
    target = next(item for item in current_asset["parts"] if item["part_id"] == target["part_id"])
    tool = next(item for item in current_asset["parts"] if item["part_id"] != target["part_id"])
    program = boolean_program("subtract", suffix="packaged")
    program["operations"][0]["args"].update({
        "part_role": target["role"],
        "position": list(target["position_mm"]),
        "size": list(target["size_mm"]),
    })
    program["operations"][1]["args"].update({
        "part_role": tool["role"],
        "position": list(target["position_mm"]),
        "size": [
            max(10, target["size_mm"][0] * 0.25),
            target["size_mm"][1] * 1.2,
            target["size_mm"][2] * 0.4,
        ],
    })
    program["operations"][2]["args"]["part_role"] = target["role"]
    program["outputs"][0]["part_role"] = target["role"]
    canonical = json.dumps(program, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    with sqlite3.connect(library_root / "library.db") as connection:
        connection.execute(
            "UPDATE agent_asset_versions SET shape_program_json = ? WHERE asset_version_id = ?",
            (canonical, asset_version_id),
        )
        connection.commit()
    exported = _request(port, f"/api/v1/agent/asset-versions/{asset_version_id}:export", method="POST")
    glb = b64decode(exported["glb_base64"])
    _assert(glb[:4] == b"glTF", "editable GLB export is invalid")
    facts = read_shape_program_glb_facts(glb)
    csg = facts.feature_history[-1]
    _assert(csg["kernel_id"] == MANIFOLD_KERNEL_ID, "packaged export did not use the Manifold kernel")
    _assert(csg["kernel_version"] == MANIFOLD_KERNEL_VERSION, "packaged export used an unexpected kernel version")
    _assert(csg["result_closed"] is True, "packaged CSG result is not closed")
    _assert("boolean_cut" in csg["surface_roles"], "packaged CSG cut provenance is missing")
    _assert_packaged_visual_pbr(glb)
    return str(asset_version_id), hashlib.sha256(glb).hexdigest()


def _assert_packaged_visual_pbr(glb: bytes) -> None:
    facts = read_shape_program_glb_facts(glb)
    _assert(facts.visual_environment["environment_id"] == "env_forgecad_room_studio_v1", "packaged PBR environment is missing")
    _assert(facts.visual_environment["color_workflow"] == "linear_srgb", "packaged PBR color workflow diverged")
    _assert(facts.visual_environment["tone_mapping"] == "aces_filmic", "packaged PBR tone mapping diverged")
    _assert(facts.visual_environment["contact_shadows"] is True, "packaged PBR contact shadows are missing")
    _assert(facts.visual_texture_sets, "packaged PBR texture readback is empty")
    for texture_set in facts.visual_texture_sets:
        _assert(
            {item["texture_role"] for item in texture_set["maps"]}
            == {"base_color", "metallic_roughness", "normal", "occlusion", "emissive"},
            "packaged PBR texture channels are incomplete",
        )
        _assert(all(item["fallback"] == "none" for item in texture_set["maps"]), "packaged PBR unexpectedly fell back")


def _request(
    port: int,
    path: str,
    *,
    method: str = "GET",
    body: dict[str, Any] | None = None,
    idempotency_key: str | None = None,
) -> dict[str, Any]:
    data = json.dumps(body).encode("utf-8") if body is not None else None
    headers = {"Accept": "application/json"}
    if data is not None:
        headers["Content-Type"] = "application/json"
    if idempotency_key is not None:
        headers["Idempotency-Key"] = idempotency_key
    request = urllib.request.Request(f"http://127.0.0.1:{port}{path}", data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", errors="replace")
        raise AssertionError(f"request failed for {method} {path}: {error}; response: {detail}") from error
    except Exception as error:
        raise AssertionError(f"request failed for {method} {path}: {error}") from error


def _wait_for_health(port: int, process: subprocess.Popen[str]) -> dict[str, str]:
    endpoint = f"http://127.0.0.1:{port}/api/health"
    for _ in range(300):
        if process.poll() is not None:
            stderr = process.stderr.read() if process.stderr else ""
            raise AssertionError(f"packaged sidecar exited before health check: {stderr[-2000:]}")
        try:
            with urllib.request.urlopen(endpoint, timeout=0.25) as response:
                return json.loads(response.read().decode("utf-8"))
        except OSError:
            time.sleep(0.1)
    raise AssertionError("packaged sidecar did not become healthy within 30 seconds")


def _extract_plan_from_turn(turn: dict[str, Any]) -> dict[str, Any]:
    """Read the one plan result from either supported packaged Turn contract."""
    plan_results = [
        item["payload"]["result"]
        for item in turn.get("items", [])
        if item.get("item_type") == "tool_result"
        and isinstance(item.get("payload"), dict)
        and item["payload"].get("tool_name", item["payload"].get("tool")) == "plan_complete_concept"
        and isinstance(item["payload"].get("result"), dict)
    ]
    _assert(len(plan_results) == 1, f"explicit robot-arm request did not produce one plan result: {turn}")
    plan = plan_results[0].get("plan", plan_results[0])
    _assert(isinstance(plan, dict), f"explicit robot-arm plan result is missing its plan payload: {turn}")
    return plan


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as socket_handle:
        socket_handle.bind(("127.0.0.1", 0))
        return int(socket_handle.getsockname()[1])


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
