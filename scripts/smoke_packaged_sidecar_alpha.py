#!/usr/bin/env python3
"""Verify a frozen sidecar's first run, editable GLB export, and recovery."""

from __future__ import annotations

import json
import os
import socket
import sqlite3
import subprocess
import tempfile
import time
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
            asset_version_id = _create_and_export_editable_asset(port, library_root)
        finally:
            _stop_sidecar(process)

        restart_port = _free_port()
        restarted = _start_sidecar(temporary, environment, restart_port)
        try:
            _wait_for_health(restart_port, restarted)
            recovered = _request(restart_port, f"/api/v1/agent/asset-versions/{asset_version_id}")
            _assert(recovered["asset_version_id"] == asset_version_id, "restart did not restore the saved editable asset")
            exported = _request(restart_port, f"/api/v1/agent/asset-versions/{asset_version_id}:export", method="POST")
            _assert(b64decode(exported["glb_base64"])[:4] == b"glTF", "recovered GLB export is invalid")
        finally:
            _stop_sidecar(restarted)
    print(json.dumps({
        "ok": True,
        "packaged_sidecar_health": True,
        "empty_library_initialized": True,
        "editable_glb_export": True,
        "packaged_manifold_csg": True,
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
    )


def _stop_sidecar(process: subprocess.Popen[str]) -> None:
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=10)


def _create_and_export_editable_asset(port: int, library_root: Path) -> str:
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
    tool_results = [item["payload"].get("result") for item in turn["items"] if item["item_type"] == "tool_result"]
    _assert(len(tool_results) == 1 and isinstance(tool_results[0], dict), f"explicit robot-arm request did not produce one plan: {turn}")
    plan = tool_results[0]
    direction_id = plan["directions"][0]["direction_id"]
    built = _request(
        port,
        "/api/v1/agent/blockouts",
        method="POST",
        idempotency_key="p002-build",
        body={"client_request_id": "p002-build", "plan": plan, "direction_id": direction_id},
    )
    segmented = _request(
        port,
        "/api/v1/agent/blockouts:segment",
        method="POST",
        idempotency_key="p002-segment",
        body={"client_request_id": "p002-segment", "plan": plan, "direction_id": direction_id, "artifact_id": built["artifact_id"]},
    )
    committed = _request(
        port,
        "/api/v1/agent/blockouts:commit",
        method="POST",
        idempotency_key="p002-commit",
        body={"client_request_id": "p002-commit", "project_id": project_id, "artifact_id": segmented["artifact_id"], "summary": "P002 本机 Alpha 可编辑概念资产"},
    )
    asset_version_id = committed["asset_version_id"]
    asset = _request(port, f"/api/v1/agent/asset-versions/{asset_version_id}")
    target = next(item for item in asset["parts"] if item["editable_parameter_bindings"])
    tool = next(item for item in asset["parts"] if item["part_id"] != target["part_id"])
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
    return str(asset_version_id)


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


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as socket_handle:
        socket_handle.bind(("127.0.0.1", 0))
        return int(socket_handle.getsockname()[1])


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
