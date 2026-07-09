#!/usr/bin/env python3
"""P1 smoke for controlled local asset reveal actions."""

from __future__ import annotations

import json
import os
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, Optional


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_p1_asset_reveal_") as tmp:
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
                    "client_request_id": "p1-reveal-source",
                    "text": "鎏金龙纹斩马刀，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产",
                    "sketch_asset_id": None,
                    "reference_asset_ids": [],
                    "auto_run": True,
                    "target": {"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
                },
                idempotency_key="p1-reveal-source-key",
            )
            detail = _json_request(base_url, f"/api/weapons/{created['weapon_id']}", method="GET")
            model_id = detail["current_model_id"]
            _assert(model_id, "created weapon did not expose current_model_id")
            exported = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/export-unity",
                method="POST",
                body={
                    "client_request_id": "p1-reveal-export",
                    "model_id": model_id,
                    "export_type": "unity_glb",
                    "include_source_spec": True,
                    "include_quality_reports": True,
                },
                idempotency_key="p1-reveal-export-key",
            )
            export_job = _json_request(base_url, f"/api/jobs/{exported['job_id']}", method="GET")
            package_asset_id = _asset_id_by_role(export_job, "unity_export_package")
            reveal = _json_request(base_url, f"/api/assets/{package_asset_id}/reveal?dry_run=true", method="POST")
            _assert(reveal["asset_id"] == package_asset_id, "reveal returned a different asset id")
            _assert(reveal["role"] == "unity_export_package", "reveal role mismatch")
            _assert(reveal["dry_run"] is True and reveal["opened"] is False, "dry-run reveal should not open the file manager")
            _assert(Path(reveal["filename"]).name == reveal["filename"], "reveal leaked a path in filename")
            _assert("path" not in reveal and "absolute_path" not in reveal, "reveal response leaked a local path")

            missing_status, missing = _json_request_allow_error(base_url, "/api/assets/file_missing/reveal?dry_run=true", method="POST")
            _assert(missing_status == 404 and missing["error"]["code"] == "ASSET_FILE_MISSING", "missing asset reveal did not return a controlled 404")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "weapon_id": created["weapon_id"],
                        "model_id": model_id,
                        "package_asset_id": package_asset_id,
                        "target": reveal["target"],
                        "filename": reveal["filename"],
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
    payload = json.dumps(body).encode("utf-8") if body is not None else None
    headers = {"Content-Type": "application/json"} if body is not None else {}
    if idempotency_key:
        headers["Idempotency-Key"] = idempotency_key
    request = urllib.request.Request(base_url + path, data=payload, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        return exc.code, json.loads(exc.read().decode("utf-8"))


def _asset_id_by_role(job: Dict[str, Any], role: str) -> str:
    for asset_id, asset_role in job["outputs"]["asset_roles"].items():
        if asset_role == role:
            return asset_id
    raise AssertionError(f"job did not expose asset role {role}")


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
