#!/usr/bin/env python3
"""M5 HTTP smoke for Unity export package snapshots."""

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
import zipfile
from pathlib import Path
from typing import Any, Dict, Optional

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_m5_export_unity_") as tmp:
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
            created = _json_request(
                base_url,
                "/api/weapons",
                method="POST",
                body={
                    "client_request_id": "m5-export-source",
                    "text": "青玉雷纹长枪，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产",
                    "sketch_asset_id": None,
                    "reference_asset_ids": [],
                    "auto_run": True,
                    "target": {"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
                },
                idempotency_key="m5-export-source-key",
            )
            detail = _json_request(base_url, f"/api/weapons/{created['weapon_id']}", method="GET")
            model_id = detail["current_model_id"]
            _assert(model_id, "created weapon did not expose current_model_id")
            export_body = {
                "client_request_id": "m5-export-unity",
                "model_id": model_id,
                "export_type": "unity_glb",
                "include_source_spec": True,
                "include_quality_reports": True,
            }
            exported = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/export-unity",
                method="POST",
                body=export_body,
                idempotency_key="m5-export-unity-key",
            )
            replay = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/export-unity",
                method="POST",
                body=export_body,
                idempotency_key="m5-export-unity-key",
            )
            _assert(exported["job_id"] == replay["job_id"], "export-unity idempotency replay returned a different job")
            conflict_status, conflict = _json_request_allow_error(
                base_url,
                f"/api/weapons/{created['weapon_id']}/export-unity",
                method="POST",
                body={**export_body, "include_source_spec": False},
                idempotency_key="m5-export-unity-key",
            )
            _assert(conflict_status == 409 and conflict["error"]["code"] == "IDEMPOTENCY_CONFLICT", "export-unity idempotency conflict was not reported")

            export_job = _json_request(base_url, f"/api/jobs/{exported['job_id']}", method="GET")
            _assert(export_job["type"] == "export_unity", "job type mismatch")
            roles = set(export_job["outputs"]["asset_roles"].values())
            _assert("unity_export_package" in roles, "export job did not produce unity_export_package asset")
            package_asset_id = _asset_id_by_role(export_job, "unity_export_package")
            conn = sqlite3.connect(library_root / "library.db")
            conn.row_factory = sqlite3.Row
            package = conn.execute("SELECT object_path FROM asset_files WHERE file_id = ?", (package_asset_id,)).fetchone()
            export_row = conn.execute("SELECT status, manifest_json FROM export_packages WHERE job_id = ?", (exported["job_id"],)).fetchone()
            _assert(export_row and export_row["status"] == "validated", "export_packages row missing or not validated")
            manifest = json.loads(export_row["manifest_json"])
            _assert(manifest["safety_boundary"]["non_manufacturing_asset"] is True, "export manifest missing safety boundary")
            with zipfile.ZipFile(library_root / package["object_path"]) as archive:
                names = archive.namelist()
                _assert(any(name.endswith("/Models/rough_optimized.glb") for name in names), "zip missing optimized GLB")
                _assert(any(name.endswith("/Materials/unity_material.json") for name in names), "zip missing unity material")
                _assert(any(name.endswith("/Specs/weapon_spec.json") for name in names), "zip missing weapon spec")
                _assert(any(name.endswith("/Reports/model_quality_report.json") for name in names), "zip missing quality report")
                _assert(any(name.endswith("/README_WUSHEN.txt") for name in names), "zip missing README")
                _assert(all(not Path(name).is_absolute() and ".." not in Path(name).parts for name in names), "zip contains unsafe paths")

            findings, stats = validate(library_root, library_root / "library.db")
            _assert(stats["blockers"] == 0, f"asset library blockers: {findings}")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "weapon_id": created["weapon_id"],
                        "model_id": model_id,
                        "export_job_id": exported["job_id"],
                        "package_asset_id": package_asset_id,
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
    raise AssertionError(f"missing role {role}")


def _assert(condition: Any, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
