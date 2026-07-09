#!/usr/bin/env python3
"""Smoke test for persisted P0 job actions and public event ordering."""

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
from typing import Any, Dict, Optional, Tuple


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_p0_actions_") as tmp:
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
            retry_job = _create_weapon(base_url, "p0-actions-retry")
            job = _json_request(base_url, f"/api/jobs/{retry_job['job_id']}", method="GET")
            seqs = [event["seq"] for event in job["events"]]
            _assert(seqs == list(range(1, len(seqs) + 1)), f"job events are not contiguous seqs: {seqs}")

            status, error = _json_request_allow_error(base_url, f"/api/jobs/{retry_job['job_id']}/cancel", method="POST")
            _assert(status == 409, f"succeeded job cancel must return 409, got {status}: {error}")

            db_path = library_root / "library.db"
            failed_event_id = _mark_job_failed(db_path, retry_job["job_id"], "rough3d_submit")
            status, error = _json_request_allow_error(base_url, f"/api/jobs/{retry_job['job_id']}/retry-from/not_a_step", method="POST")
            _assert(status == 400, f"unknown retry step must return 400, got {status}: {error}")

            retry = _json_request(base_url, f"/api/jobs/{retry_job['job_id']}/retry-from/rough3d_submit", method="POST")
            _assert(retry["status"] == "retrying", f"retry-from returned wrong status: {retry}")
            _assert(retry["previous_status"] == "failed", f"retry previous_status was not failed: {retry}")
            _assert(retry["retry_from"] == "rough3d_submit", f"retry_from missing: {retry}")
            retry_detail = _json_request(base_url, f"/api/jobs/{retry_job['job_id']}", method="GET")
            _assert(retry_detail["status"] == "retrying", "retry did not persist generation_jobs.status")
            _assert(retry_detail["current_step"] == "rough3d_submit", "retry did not persist current_step")
            _assert(retry["event_id"] in {event["id"] for event in retry_detail["events"]}, "retry event was not persisted")
            resumed = _read_text(base_url, f"{retry_job['event_stream_url']}?after={failed_event_id}")
            _assert(str(retry["event_id"]) in resumed, "SSE after failed event did not replay retry event")

            cancel_job = _create_weapon(base_url, "p0-actions-cancel")
            _mark_job_waiting(db_path, cancel_job["job_id"], "image_submit")
            cancel = _json_request(base_url, f"/api/jobs/{cancel_job['job_id']}/cancel", method="POST")
            _assert(cancel["status"] == "cancelled", f"cancel returned wrong status: {cancel}")
            cancel_detail = _json_request(base_url, f"/api/jobs/{cancel_job['job_id']}", method="GET")
            _assert(cancel_detail["status"] == "cancelled", "cancel did not persist generation_jobs.status")
            _assert(cancel["event_id"] in {event["id"] for event in cancel_detail["events"]}, "cancel event was not persisted")

            with sqlite3.connect(db_path) as conn:
                action_count = conn.execute("SELECT COUNT(*) FROM job_actions WHERE status IN ('accepted', 'noop')").fetchone()[0]
            _assert(action_count >= 2, "job_actions did not record accepted actions")

            print(
                json.dumps(
                    {
                        "ok": True,
                        "retry_job_id": retry_job["job_id"],
                        "retry_event_id": retry["event_id"],
                        "cancel_job_id": cancel_job["job_id"],
                        "cancel_event_id": cancel["event_id"],
                        "action_count": action_count,
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


def _create_weapon(base_url: str, client_request_id: str) -> Dict[str, Any]:
    body = {
        "client_request_id": client_request_id,
        "text": "赤金国风龙纹长剑，3渲2，逼真外观，仅作为虚构 Unity 游戏资产",
        "sketch_asset_id": None,
        "reference_asset_ids": [],
        "auto_run": True,
        "target": {"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
    }
    return _json_request(base_url, "/api/weapons", method="POST", body=body, idempotency_key=client_request_id)


def _mark_job_failed(db_path: Path, job_id: str, step_name: str) -> str:
    now = _now()
    with sqlite3.connect(db_path) as conn:
        conn.row_factory = sqlite3.Row
        weapon_id = conn.execute("SELECT weapon_id FROM generation_jobs WHERE job_id = ?", (job_id,)).fetchone()["weapon_id"]
        next_seq = conn.execute("SELECT COALESCE(MAX(seq), 0) + 1 FROM agent_events WHERE job_id = ?", (job_id,)).fetchone()[0]
        event_id = f"evt_{job_id}_{next_seq:04d}"
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'failed', current_step = ?, error_code = 'PROVIDER_TIMEOUT',
                error_message = 'Synthetic provider timeout for P0 action smoke.',
                updated_at = ?, finished_at = ?
            WHERE job_id = ?
            """,
            (step_name, now, now, job_id),
        )
        conn.execute(
            "UPDATE job_steps SET status = 'failed', error_code = 'PROVIDER_TIMEOUT', error_message = 'synthetic failure', finished_at = ? WHERE job_id = ? AND step_name = ?",
            (now, job_id, step_name),
        )
        conn.execute(
            """
            INSERT INTO agent_events (
              event_id, job_id, seq, weapon_id, step, level, status, message,
              artifact_asset_id, metadata_json, created_at
            )
            VALUES (?, ?, ?, ?, ?, 'error', 'failed', ?, NULL, ?, ?)
            """,
            (
                event_id,
                job_id,
                next_seq,
                weapon_id,
                step_name,
                "Synthetic failure for P0 action smoke.",
                json.dumps({"progress": 0.9, "error_code": "PROVIDER_TIMEOUT"}),
                now,
            ),
        )
        return event_id


def _mark_job_waiting(db_path: Path, job_id: str, step_name: str) -> None:
    now = _now()
    with sqlite3.connect(db_path) as conn:
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'waiting_provider', current_step = ?, updated_at = ?, finished_at = NULL
            WHERE job_id = ?
            """,
            (step_name, now, job_id),
        )
        conn.execute(
            "UPDATE job_steps SET status = 'waiting_provider', finished_at = NULL WHERE job_id = ? AND step_name = ?",
            (job_id, step_name),
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


def _json_request_allow_error(base_url: str, path: str, *, method: str) -> Tuple[int, Dict[str, Any]]:
    request = urllib.request.Request(base_url + path, method=method)
    try:
        with urllib.request.urlopen(request, timeout=15) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        return exc.code, json.loads(exc.read().decode("utf-8"))


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
