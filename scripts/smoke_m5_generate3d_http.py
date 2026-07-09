#!/usr/bin/env python3
"""M5 HTTP smoke for explicit generate-3d job creation."""

from __future__ import annotations

import json
import os
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, Optional

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_m5_generate3d_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        env = os.environ.copy()
        env["WUSHEN_LIBRARY_ROOT"] = str(library_root)
        env["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
        process = subprocess.Popen(
            [
                sys.executable,
                "-m",
                "uvicorn",
                "wushen_agent.main:create_app",
                "--factory",
                "--host",
                "127.0.0.1",
                "--port",
                str(port),
            ],
            cwd=ROOT,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        try:
            _wait_for_health(base_url, process)
            create_body = {
                "client_request_id": "m5-generate3d-source",
                "text": "青玉雷纹长枪，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产",
                "sketch_asset_id": None,
                "reference_asset_ids": [],
                "auto_run": True,
                "target": {"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
            }
            created = _json_request(base_url, "/api/weapons", method="POST", body=create_body, idempotency_key="m5-generate3d-source-key")
            source_job = _json_request(base_url, f"/api/jobs/{created['job_id']}", method="GET")
            source_version_id = source_job["outputs"]["current_version_id"]
            source_image_id = _asset_id_by_role(source_job, "concept_image")
            generate_body = {
                "client_request_id": "m5-generate3d",
                "source_version_id": source_version_id,
                "source_image_asset_id": source_image_id,
                "provider_id": "mock_3d",
                "target_format": "glb",
                "style": "stylized_toon_weapon",
                "orientation_policy": {"forward_axis": "+Z", "long_axis": "+Y", "pivot": "grip_center"},
                "scale_policy": "normalized_game_asset_scale",
                "build_unity_export": True,
            }
            generated = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/generate-3d",
                method="POST",
                body=generate_body,
                idempotency_key="m5-generate3d-key",
            )
            replay = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/generate-3d",
                method="POST",
                body=generate_body,
                idempotency_key="m5-generate3d-key",
            )
            _assert(generated["job_id"] == replay["job_id"], "generate-3d idempotency replay returned a different job")
            conflict_status, conflict = _json_request_allow_error(
                base_url,
                f"/api/weapons/{created['weapon_id']}/generate-3d",
                method="POST",
                body={**generate_body, "source_image_asset_id": "file_different"},
                idempotency_key="m5-generate3d-key",
            )
            _assert(conflict_status == 409 and conflict["error"]["code"] == "IDEMPOTENCY_CONFLICT", "generate-3d idempotency conflict was not reported")

            generate_job = _json_request(base_url, f"/api/jobs/{generated['job_id']}", method="GET")
            _assert(generate_job["type"] == "generate_3d", "job type mismatch")
            _assert(generate_job["outputs"]["current_version_id"] != source_version_id, "generate-3d did not create a child rough_3d version")
            roles = set(generate_job["outputs"]["asset_roles"].values())
            expected_roles = {"other", "rough_raw_glb", "rough_normalized_glb", "rough_optimized_glb", "unity_material_json", "quality_report"}
            _assert(expected_roles.issubset(roles), f"generate-3d roles missing: {expected_roles - roles}")
            detail = _json_request(base_url, f"/api/weapons/{created['weapon_id']}", method="GET")
            _assert(detail["current_version_id"] == generate_job["outputs"]["current_version_id"], "weapon current version did not move to generated 3D version")
            version = next(item for item in detail["versions"] if item["version_id"] == detail["current_version_id"])
            _assert(version["parent_version_id"] == source_version_id, "rough_3d version parent mismatch")
            _assert(version["version_type"] == "rough_3d", "generated version was not rough_3d")
            listed = _json_request(base_url, "/api/weapons", method="GET")
            matching = [item for item in listed["items"] if item["weapon_id"] == created["weapon_id"]]
            _assert(len(matching) == 1, "listWeapons returned duplicate rows after multiple models")

            findings, stats = validate(library_root, library_root / "library.db")
            _assert(stats["blockers"] == 0, f"asset library blockers: {findings}")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "weapon_id": created["weapon_id"],
                        "source_version_id": source_version_id,
                        "rough3d_version_id": generate_job["outputs"]["current_version_id"],
                        "generate_job_id": generated["job_id"],
                        "asset_count": stats["asset_files"],
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
            return 0
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _wait_for_health(base_url: str, process: subprocess.Popen) -> None:
    deadline = time.time() + 15
    while time.time() < deadline:
        if process.poll() is not None:
            output = process.stdout.read() if process.stdout else ""
            raise RuntimeError(f"Agent exited before health check:\n{output}")
        try:
            health = _json_request(base_url, "/api/health", method="GET")
            if health.get("status") == "ok":
                return
        except Exception:
            time.sleep(0.2)
    raise TimeoutError("Agent health check timed out")


def _json_request(
    base_url: str, path: str, *, method: str, body: Optional[Dict[str, Any]] = None, idempotency_key: Optional[str] = None
) -> Dict[str, Any]:
    status, data = _json_request_allow_error(base_url, path, method=method, body=body, idempotency_key=idempotency_key)
    _assert(200 <= status < 300, f"{method} {path} failed with {status}: {data}")
    return data


def _json_request_allow_error(
    base_url: str, path: str, *, method: str, body: Optional[Dict[str, Any]] = None, idempotency_key: Optional[str] = None
) -> tuple[int, Dict[str, Any]]:
    payload = None if body is None else json.dumps(body).encode("utf-8")
    request = urllib.request.Request(f"{base_url}{path}", data=payload, method=method)
    request.add_header("Content-Type", "application/json")
    if idempotency_key:
        request.add_header("Idempotency-Key", idempotency_key)
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        return exc.code, json.loads(exc.read().decode("utf-8"))


def _asset_id_by_role(job: Dict[str, Any], role: str) -> str:
    for asset_id, asset_role in job["outputs"]["asset_roles"].items():
        if asset_role == role:
            return asset_id
    raise AssertionError(f"Missing asset role {role}")


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
