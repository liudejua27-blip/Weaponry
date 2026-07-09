#!/usr/bin/env python3
"""Smoke test for opt-in always-on generate-3d worker loop."""

from __future__ import annotations

import json
import os
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path
from typing import Any, Dict, Optional

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_p0_worker_loop_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(port, library_root)
        try:
            _wait_for_health(base_url, process)
            created = _create_weapon(base_url, "p0-worker-loop-source")
            source_job = _json_request(base_url, f"/api/jobs/{created['job_id']}", method="GET")
            source_version_id = source_job["outputs"]["current_version_id"]
            source_image_id = _asset_id_by_role(source_job, "concept_image")
            accepted = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/generate-3d",
                method="POST",
                body={
                    "client_request_id": "p0-worker-loop-generate",
                    "source_version_id": source_version_id,
                    "source_image_asset_id": source_image_id,
                    "provider_id": "mock_3d",
                    "target_format": "glb",
                    "style": "stylized_toon_weapon",
                    "orientation_policy": {"forward_axis": "+Z", "long_axis": "+Y", "pivot": "grip_center"},
                    "scale_policy": "normalized_game_asset_scale",
                    "build_unity_export": True,
                },
                idempotency_key="p0-worker-loop-key",
            )
            _assert(accepted["status"] == "queued", f"worker-loop job was not queued: {accepted}")
            terminal = _wait_for_terminal_job(base_url, accepted["job_id"])
            _assert(terminal["status"] == "succeeded", f"worker-loop job did not succeed: {terminal['status']}")
            _assert(terminal["current_step"] == "finalize_job", "worker-loop job did not finish at finalize_job")
            seqs = [event["seq"] for event in terminal["events"]]
            _assert(seqs == list(range(1, len(seqs) + 1)), f"worker-loop event seqs are not contiguous: {seqs}")
            succeeded_steps = {event["step"] for event in terminal["events"] if event["status"] == "succeeded"}
            expected_steps = {"rough3d_plan", "rough3d_submit", "model_qc_optimize", "asset_commit_model", "finalize_job"}
            _assert(expected_steps.issubset(succeeded_steps), f"worker-loop steps missing: {expected_steps - succeeded_steps}")
            _assert(terminal["outputs"]["current_version_id"] != source_version_id, "worker-loop job did not create a child version")
            roles = set(terminal["outputs"]["asset_roles"].values())
            expected_roles = {"other", "rough_raw_glb", "rough_normalized_glb", "rough_optimized_glb", "unity_material_json", "quality_report"}
            _assert(expected_roles.issubset(roles), f"worker-loop roles missing: {expected_roles - roles}")
            runtime = _json_request(base_url, f"/api/jobs/{accepted['job_id']}/runtime", method="GET")
            rough_task = next(task for task in runtime["provider_tasks"] if task["step"] == "rough3d_submit")
            _assert(rough_task["status"] == "succeeded", "worker-loop provider task was not succeeded")
            _assert(rough_task["provider_task_id"], "worker-loop provider task id missing")
            rough_checkpoint = next(item for item in runtime["checkpoints"] if item["step"] == "rough3d_submit")
            _assert(rough_checkpoint["status"] == "completed", "worker-loop checkpoint was not completed")
            with sqlite3.connect(library_root / "library.db") as conn:
                job_row = conn.execute(
                    "SELECT runner_id, lease_expires_at FROM generation_jobs WHERE job_id = ?",
                    (accepted["job_id"],),
                ).fetchone()
                version_count = conn.execute(
                    "SELECT COUNT(*) FROM weapon_versions WHERE job_id = ? AND version_type = 'rough_3d' AND status = 'committed'",
                    (accepted["job_id"],),
                ).fetchone()[0]
                model_count = conn.execute("SELECT COUNT(*) FROM models_3d WHERE job_id = ?", (accepted["job_id"],)).fetchone()[0]
            _assert(job_row[0] is None and job_row[1] is None, "worker-loop lease was not cleared")
            _assert(version_count == 1, "worker-loop committed duplicate rough_3d versions")
            _assert(model_count == 1, "worker-loop created duplicate models")
            findings, stats = validate(library_root, library_root / "library.db")
            _assert(stats["blockers"] == 0, f"asset library blockers after worker-loop: {findings}")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "job_id": accepted["job_id"],
                        "rough3d_version_id": terminal["outputs"]["current_version_id"],
                        "asset_count": stats["asset_files"],
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
            return 0
        finally:
            _stop_process_and_assert_port_free(process, port)


def _start_agent(port: int, library_root: Path) -> subprocess.Popen:
    env = os.environ.copy()
    env["WUSHEN_LIBRARY_ROOT"] = str(library_root)
    env["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
    env["WUSHEN_GENERATE3D_WORKER"] = "1"
    env["WUSHEN_GENERATE3D_WORKER_INTERVAL_SECONDS"] = "0.05"
    env["WUSHEN_GENERATE3D_WORKER_ID"] = "smoke_generate3d_loop"
    env["WUSHEN_RECOVER_ON_STARTUP"] = "0"
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


def _create_weapon(base_url: str, client_request_id: str) -> Dict[str, Any]:
    body = {
        "client_request_id": client_request_id,
        "text": "青玉雷纹长枪，3渲2国风神兵，高拟真外观，仅作为虚构 Unity 游戏资产",
        "sketch_asset_id": None,
        "reference_asset_ids": [],
        "auto_run": True,
        "target": {"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
    }
    return _json_request(base_url, "/api/weapons", method="POST", body=body, idempotency_key=client_request_id)


def _wait_for_terminal_job(base_url: str, job_id: str) -> Dict[str, Any]:
    deadline = time.time() + 20
    while time.time() < deadline:
        job = _json_request(base_url, f"/api/jobs/{job_id}", method="GET")
        if job["status"] in {"succeeded", "failed", "cancelled", "partial_succeeded"}:
            return job
        time.sleep(0.2)
    raise TimeoutError(f"Job did not reach terminal status: {job_id}")


def _asset_id_by_role(job: Dict[str, Any], role: str) -> str:
    for asset_id, asset_role in job["outputs"]["asset_roles"].items():
        if asset_role == role:
            return asset_id
    raise AssertionError(f"Missing asset role {role}")


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _stop_process_and_assert_port_free(process: subprocess.Popen, port: int) -> None:
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)
    deadline = time.time() + 5
    while time.time() < deadline:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            if sock.connect_ex(("127.0.0.1", port)) != 0:
                return
        time.sleep(0.1)
    raise AssertionError(f"Agent process stopped but port {port} still accepts connections")


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


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
