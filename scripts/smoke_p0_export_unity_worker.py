#!/usr/bin/env python3
"""Smoke test for opt-in async Unity export worker runtime."""

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


def main() -> int:
    manual = _run_manual_work_once_case()
    loop = _run_background_loop_case()
    print(json.dumps({"ok": True, "manual": manual, "loop": loop}, ensure_ascii=False, indent=2))
    return 0


def _run_manual_work_once_case() -> Dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="wushen_p0_export_worker_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(port, library_root, {"WUSHEN_EXPORT_UNITY_ASYNC": "1"})
        try:
            _wait_for_health(base_url, process)
            created = _create_source_weapon(base_url, "p0-export-worker-source")
            detail = _json_request(base_url, f"/api/weapons/{created['weapon_id']}", method="GET")
            model_id = detail["current_model_id"]
            rough_version_id = detail["current_version_id"]
            export_body = _export_body(model_id)
            accepted = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/export-unity",
                method="POST",
                body=export_body,
                idempotency_key="p0-export-worker-key",
            )
            _assert(accepted["status"] == "queued", f"export worker job was not queued: {accepted}")
            replay = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/export-unity",
                method="POST",
                body=export_body,
                idempotency_key="p0-export-worker-key",
            )
            _assert(replay["job_id"] == accepted["job_id"], "queued export idempotency replay returned a different job")
            conflict_status, conflict = _json_request_allow_error(
                base_url,
                f"/api/weapons/{created['weapon_id']}/export-unity",
                method="POST",
                body={**export_body, "include_source_spec": False},
                idempotency_key="p0-export-worker-key",
            )
            _assert(conflict_status == 409 and conflict["error"]["code"] == "IDEMPOTENCY_CONFLICT", "queued export idempotency conflict failed")
            _assert_no_export_outputs(library_root, accepted["job_id"])
            queued_detail = _json_request(base_url, f"/api/weapons/{created['weapon_id']}", method="GET")
            _assert(queued_detail["current_version_id"] == rough_version_id, "queued export changed current version before worker completion")

            work = _json_request(base_url, "/api/runtime/work-once", method="POST")
            _assert(work["claimed"] is True and work["job_type"] == "export_unity", f"work-once did not claim export job: {work}")
            terminal = _json_request(base_url, f"/api/jobs/{accepted['job_id']}", method="GET")
            _assert_export_terminal(library_root, base_url, accepted["job_id"], terminal)
            return {"job_id": accepted["job_id"], "package_asset_id": _asset_id_by_role(terminal, "unity_export_package")}
        finally:
            _stop_process(process)


def _run_background_loop_case() -> Dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="wushen_p0_export_loop_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(
            port,
            library_root,
            {
                "WUSHEN_EXPORT_UNITY_WORKER": "1",
                "WUSHEN_LOCAL_WORKER_INTERVAL_SECONDS": "0.05",
                "WUSHEN_LOCAL_WORKER_ID": "smoke_export_unity_loop",
                "WUSHEN_RECOVER_ON_STARTUP": "0",
            },
        )
        try:
            _wait_for_health(base_url, process)
            created = _create_source_weapon(base_url, "p0-export-loop-source")
            detail = _json_request(base_url, f"/api/weapons/{created['weapon_id']}", method="GET")
            accepted = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/export-unity",
                method="POST",
                body=_export_body(detail["current_model_id"]),
                idempotency_key="p0-export-loop-key",
            )
            _assert(accepted["status"] == "queued", f"export loop job was not queued: {accepted}")
            terminal = _wait_for_terminal_job(base_url, accepted["job_id"])
            _assert_export_terminal(library_root, base_url, accepted["job_id"], terminal)
            return {"job_id": accepted["job_id"], "package_asset_id": _asset_id_by_role(terminal, "unity_export_package")}
        finally:
            _stop_process(process)


def _start_agent(port: int, library_root: Path, extra_env: Dict[str, str]) -> subprocess.Popen:
    env = os.environ.copy()
    env["WUSHEN_LIBRARY_ROOT"] = str(library_root)
    env["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
    env.update(extra_env)
    return subprocess.Popen(
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


def _create_source_weapon(base_url: str, client_request_id: str) -> Dict[str, Any]:
    return _json_request(
        base_url,
        "/api/weapons",
        method="POST",
        body={
            "client_request_id": client_request_id,
            "text": "青玉雷纹长枪，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产",
            "sketch_asset_id": None,
            "reference_asset_ids": [],
            "auto_run": True,
            "target": {"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
        },
        idempotency_key=client_request_id,
    )


def _export_body(model_id: str) -> Dict[str, Any]:
    return {
        "client_request_id": "p0-export-unity-worker",
        "model_id": model_id,
        "export_type": "unity_glb",
        "include_source_spec": True,
        "include_quality_reports": True,
    }


def _assert_no_export_outputs(library_root: Path, job_id: str) -> None:
    with sqlite3.connect(library_root / "library.db") as conn:
        version_count = conn.execute("SELECT COUNT(*) FROM weapon_versions WHERE job_id = ? AND version_type = 'export'", (job_id,)).fetchone()[0]
        package_count = conn.execute("SELECT COUNT(*) FROM export_packages WHERE job_id = ?", (job_id,)).fetchone()[0]
        asset_count = conn.execute("SELECT COUNT(*) FROM asset_files WHERE job_id = ? AND role = 'unity_export_package'", (job_id,)).fetchone()[0]
    _assert(version_count == 0, "queued export created an export version too early")
    _assert(package_count == 0, "queued export created an export package row too early")
    _assert(asset_count == 0, "queued export created a ZIP asset too early")


def _assert_export_terminal(library_root: Path, base_url: str, job_id: str, job: Dict[str, Any]) -> None:
    _assert(job["status"] == "succeeded", f"export worker job did not succeed: {job['status']}")
    _assert(job["current_step"] == "finalize_job", "export worker job did not finish at finalize_job")
    seqs = [event["seq"] for event in job["events"]]
    _assert(seqs == list(range(1, len(seqs) + 1)), f"export worker event seqs are not contiguous: {seqs}")
    succeeded_steps = [event["step"] for event in job["events"] if event["status"] == "succeeded"]
    _assert(succeeded_steps == ["export_plan", "export_manifest", "export_package", "finalize_job"], f"unexpected export succeeded steps: {succeeded_steps}")
    package_asset_id = _asset_id_by_role(job, "unity_export_package")
    runtime = _json_request(base_url, f"/api/jobs/{job_id}/runtime", method="GET")
    completed = {checkpoint["step"]: checkpoint for checkpoint in runtime["checkpoints"] if checkpoint["status"] == "completed"}
    for step in ["export_plan", "export_manifest", "export_package", "finalize_job"]:
        _assert(step in completed, f"missing completed export checkpoint: {step}")
    with sqlite3.connect(library_root / "library.db") as conn:
        conn.row_factory = sqlite3.Row
        job_row = conn.execute("SELECT runner_id, lease_expires_at FROM generation_jobs WHERE job_id = ?", (job_id,)).fetchone()
        version_count = conn.execute("SELECT COUNT(*) FROM weapon_versions WHERE job_id = ? AND version_type = 'export' AND status = 'committed'", (job_id,)).fetchone()[0]
        package_count = conn.execute("SELECT COUNT(*) FROM export_packages WHERE job_id = ?", (job_id,)).fetchone()[0]
        asset_count = conn.execute("SELECT COUNT(*) FROM asset_files WHERE job_id = ? AND role = 'unity_export_package'", (job_id,)).fetchone()[0]
        package = conn.execute("SELECT object_path FROM asset_files WHERE file_id = ?", (package_asset_id,)).fetchone()
    _assert(job_row["runner_id"] is None and job_row["lease_expires_at"] is None, "export worker lease was not cleared")
    _assert(version_count == 1, "export worker committed duplicate export versions")
    _assert(package_count == 1, "export worker created duplicate export package rows")
    _assert(asset_count == 1, "export worker created duplicate ZIP assets")
    with zipfile.ZipFile(library_root / package["object_path"]) as archive:
        names = archive.namelist()
        _assert(any(name.endswith("/Models/rough_optimized.glb") for name in names), "zip missing optimized GLB")
        _assert(any(name.endswith("/Materials/unity_material.json") for name in names), "zip missing unity material")
        _assert(any(name.endswith("/README_WUSHEN.txt") for name in names), "zip missing README")
        _assert(all(not Path(name).is_absolute() and ".." not in Path(name).parts for name in names), "zip contains unsafe paths")
    findings, stats = validate(library_root, library_root / "library.db")
    _assert(stats["blockers"] == 0, f"asset library blockers after export worker: {findings}")


def _asset_id_by_role(job: Dict[str, Any], role: str) -> str:
    for asset_id, asset_role in job["outputs"]["asset_roles"].items():
        if asset_role == role:
            return asset_id
    raise AssertionError(f"Missing asset role {role}")


def _wait_for_terminal_job(base_url: str, job_id: str) -> Dict[str, Any]:
    deadline = time.time() + 20
    while time.time() < deadline:
        job = _json_request(base_url, f"/api/jobs/{job_id}", method="GET")
        if job["status"] in {"succeeded", "failed", "cancelled", "partial_succeeded"}:
            return job
        time.sleep(0.2)
    raise TimeoutError(f"Job did not reach terminal status: {job_id}")


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
    base_url: str,
    path: str,
    *,
    method: str,
    body: Optional[Dict[str, Any]] = None,
    idempotency_key: Optional[str] = None,
) -> Dict[str, Any]:
    payload = json.dumps(body).encode("utf-8") if body is not None else None
    headers = {"Content-Type": "application/json"}
    if idempotency_key:
        headers["Idempotency-Key"] = idempotency_key
    request = urllib.request.Request(base_url + path, data=payload, headers=headers, method=method)
    with urllib.request.urlopen(request, timeout=20) as response:
        return json.loads(response.read().decode("utf-8"))


def _json_request_allow_error(
    base_url: str,
    path: str,
    *,
    method: str,
    body: Dict[str, Any],
    idempotency_key: Optional[str] = None,
) -> tuple[int, Dict[str, Any]]:
    try:
        return 200, _json_request(base_url, path, method=method, body=body, idempotency_key=idempotency_key)
    except urllib.error.HTTPError as exc:
        return exc.code, json.loads(exc.read().decode("utf-8"))


def _stop_process(process: subprocess.Popen) -> None:
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
