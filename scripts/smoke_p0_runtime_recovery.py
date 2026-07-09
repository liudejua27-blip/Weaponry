#!/usr/bin/env python3
"""Smoke test for P0 runtime recovery metadata and SSE cursor contract."""

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


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_p0_runtime_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(base_url, port, library_root)
        try:
            _wait_for_health(base_url, process)
            job = _create_weapon(base_url, "p0-runtime-create")
            detail = _json_request(base_url, f"/api/jobs/{job['job_id']}", method="GET")
            runtime = _json_request(base_url, f"/api/jobs/{job['job_id']}/runtime", method="GET")
            _assert(runtime["job_id"] == job["job_id"], "runtime job id mismatch")
            _assert(runtime["provider_tasks"], "provider task registry was empty")
            _assert(runtime["checkpoints"], "job checkpoints were empty")
            _assert(
                any(task["step"] == "rough3d_submit" and task["provider_task_id"] for task in runtime["provider_tasks"]),
                "rough3d provider task was not persisted",
            )
            _assert(
                any(checkpoint["step"] == "rough3d_submit" and checkpoint["status"] == "completed" for checkpoint in runtime["checkpoints"]),
                "rough3d checkpoint was not completed",
            )

            cursor_text = _read_text(base_url, f"{job['event_stream_url']}?after=evt_missing")
            _assert("INVALID_EVENT_CURSOR" in cursor_text, "unknown SSE cursor did not emit INVALID_EVENT_CURSOR")

            db_path = library_root / "library.db"
            waiting_job = _create_weapon(base_url, "p0-runtime-cancel")
            _mark_job_waiting_with_provider_task(db_path, waiting_job["job_id"], "rough3d_submit")
            cancel = _json_request(base_url, f"/api/jobs/{waiting_job['job_id']}/cancel", method="POST")
            _assert(cancel["status"] == "cancelled", f"cancel did not return cancelled: {cancel}")
            cancel_runtime = _json_request(base_url, f"/api/jobs/{waiting_job['job_id']}/runtime", method="GET")
            _assert(
                any(task["status"] in {"cancel_requested", "cancelled"} for task in cancel_runtime["provider_tasks"]),
                "cancel did not mark provider task cancel requested or cancelled",
            )
            _assert(
                any(checkpoint["status"] == "cancelled" for checkpoint in cancel_runtime["checkpoints"]),
                "cancel did not mark checkpoint cancelled",
            )

            interrupted = _create_weapon(base_url, "p0-runtime-recover")
            _mark_job_interrupted(db_path, interrupted["job_id"], "rough3d_submit")
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()

        process = _start_agent(base_url, port, library_root)
        try:
            _wait_for_health(base_url, process)
            recovered = _json_request(base_url, "/api/runtime/recover", method="POST")
            recovered_ids = {item["job_id"] for item in recovered["items"]}
            _assert(interrupted["job_id"] in recovered_ids or recovered["recovered_count"] == 0, f"unexpected recovery response: {recovered}")
            recovered_detail = _json_request(base_url, f"/api/jobs/{interrupted['job_id']}", method="GET")
            _assert(recovered_detail["status"] == "waiting_user", "startup recovery did not pause interrupted job")
            _assert(
                any(event["status"] == "waiting_user" for event in recovered_detail["events"]),
                "recovery event was not appended",
            )
            recovered_runtime = _json_request(base_url, f"/api/jobs/{interrupted['job_id']}/runtime", method="GET")
            _assert(recovered_runtime["resumable"] is True, "recovered job was not marked resumable")

            with sqlite3.connect(db_path) as conn:
                versions = {row[0] for row in conn.execute("SELECT version FROM schema_migrations").fetchall()}
            _assert("0006" in versions, "migration 0006 was not applied")

            print(
                json.dumps(
                    {
                        "ok": True,
                        "runtime_job_id": job["job_id"],
                        "cancel_job_id": waiting_job["job_id"],
                        "recovered_job_id": interrupted["job_id"],
                        "migrations": sorted(versions),
                    },
                    indent=2,
                    ensure_ascii=False,
                )
            )
            return 0
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()


def _start_agent(base_url: str, port: int, library_root: Path) -> subprocess.Popen:
    env = os.environ.copy()
    env["WUSHEN_LIBRARY_ROOT"] = str(library_root)
    env["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
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


def _mark_job_waiting_with_provider_task(db_path: Path, job_id: str, step_name: str) -> None:
    now = _now()
    with sqlite3.connect(db_path) as conn:
        conn.row_factory = sqlite3.Row
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'waiting_provider', current_step = ?, updated_at = ?, finished_at = NULL
            WHERE job_id = ?
            """,
            (step_name, now, job_id),
        )
        conn.execute(
            """
            UPDATE job_steps
            SET status = 'waiting_provider',
                provider_task_id = 'mock_waiting_provider_task',
                resumable_after_restart = 1,
                cancel_state = 'none',
                finished_at = NULL
            WHERE job_id = ? AND step_name = ?
            """,
            (job_id, step_name),
        )
        conn.execute(
            """
            INSERT INTO provider_tasks (
              task_record_id, job_id, step_name, attempt, provider_kind, provider_id,
              provider_task_id, status, last_seen_at, metadata_json, created_at, updated_at
            )
            VALUES (?, ?, ?, 1, 'three_d', 'mock_3d', 'mock_waiting_provider_task',
                    'polling', ?, '{}', ?, ?)
            """,
            (f"ptask_{job_id}", job_id, step_name, now, now, now),
        )


def _mark_job_interrupted(db_path: Path, job_id: str, step_name: str) -> None:
    now = _now()
    with sqlite3.connect(db_path) as conn:
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'running', current_step = ?, provider_task_id = ?, updated_at = ?, finished_at = NULL
            WHERE job_id = ?
            """,
            (step_name, f"mock_restart_{job_id}", now, job_id),
        )
        conn.execute(
            """
            UPDATE job_steps
            SET status = 'running',
                provider_task_id = ?,
                resumable_after_restart = 1,
                finished_at = NULL
            WHERE job_id = ? AND step_name = ?
            """,
            (f"mock_restart_{job_id}", job_id, step_name),
        )
        conn.execute(
            """
            INSERT INTO provider_tasks (
              task_record_id, job_id, step_name, attempt, provider_kind, provider_id,
              provider_task_id, status, last_seen_at, metadata_json, created_at, updated_at
            )
            VALUES (?, ?, ?, 1, 'three_d', 'mock_3d', ?, 'polling', ?, '{}', ?, ?)
            """,
            (f"ptask_restart_{job_id}", job_id, step_name, f"mock_restart_{job_id}", now, now, now),
        )


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
    with urllib.request.urlopen(request, timeout=15) as response:
        return json.loads(response.read().decode("utf-8"))


def _read_text(base_url: str, path: str) -> str:
    with urllib.request.urlopen(base_url + path, timeout=15) as response:
        return response.read().decode("utf-8")


def _now() -> str:
    return time.strftime("%Y-%m-%dT%H:%M:%S+00:00", time.gmtime())


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
