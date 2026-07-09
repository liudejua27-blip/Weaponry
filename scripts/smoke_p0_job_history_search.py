#!/usr/bin/env python3
"""Smoke test for P0 job history search and action audit APIs."""

from __future__ import annotations

import json
import os
import socket
import sqlite3
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any, Dict, Optional


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_p0_job_history_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        env = os.environ.copy()
        env["WUSHEN_LIBRARY_ROOT"] = str(library_root)
        env["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
        env["WUSHEN_RECOVER_ON_STARTUP"] = "0"

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
            success = _create_weapon(base_url, "history-success")
            failed_filter = _create_weapon(base_url, "history-failed-filter")
            failed_action = _create_weapon(base_url, "history-failed-action")
            waiting_cancel = _create_weapon(base_url, "history-waiting-cancel")
            interrupted = _create_weapon(base_url, "history-interrupted")

            db_path = library_root / "library.db"
            _set_job_timestamp(db_path, success["job_id"], "2026-07-05T08:00:00+00:00")
            _mark_job_failed(db_path, failed_filter["job_id"], "rough3d_submit", "2026-07-05T08:04:00+00:00")
            _mark_job_failed(db_path, failed_action["job_id"], "rough3d_submit", "2026-07-05T08:03:00+00:00")
            _mark_job_waiting(db_path, waiting_cancel["job_id"], "image_submit", "2026-07-05T08:02:00+00:00")
            _mark_job_running(db_path, interrupted["job_id"], "rough3d_submit", "2026-07-05T08:01:00+00:00")

            first_page = _json_request(base_url, "/api/jobs?limit=2", method="GET")
            _assert(len(first_page["items"]) == 2, f"expected 2 jobs on first page: {first_page}")
            _assert(first_page["items"][0]["job_id"] == failed_filter["job_id"], f"default order was not updated_at desc: {first_page}")
            _assert(first_page.get("next_cursor"), "first page did not return a cursor")
            second_page = _json_request(base_url, f"/api/jobs?limit=2&cursor={urllib.parse.quote(first_page['next_cursor'])}", method="GET")
            first_ids = {item["job_id"] for item in first_page["items"]}
            second_ids = {item["job_id"] for item in second_page["items"]}
            _assert(not first_ids & second_ids, f"pagination returned duplicate ids: {first_ids & second_ids}")

            failed_jobs = _json_request(base_url, "/api/jobs?status=failed", method="GET")
            _assert(
                {item["job_id"] for item in failed_jobs["items"]} == {failed_filter["job_id"], failed_action["job_id"]},
                f"status=failed mismatch: {failed_jobs}",
            )
            timeout_jobs = _json_request(base_url, "/api/jobs?error_code=PROVIDER_TIMEOUT", method="GET")
            _assert(len(timeout_jobs["items"]) == 2, f"error_code filter mismatch: {timeout_jobs}")
            query_hit = _json_request(base_url, f"/api/jobs?query={failed_filter['job_id'][-8:]}", method="GET")
            _assert(query_hit["items"] and query_hit["items"][0]["job_id"] == failed_filter["job_id"], f"query did not hit job id: {query_hit}")
            query_miss = _json_request(base_url, "/api/jobs?query=no_such_wushen_job", method="GET")
            _assert(query_miss["items"] == [], f"query miss should be empty: {query_miss}")

            detail = _json_request(base_url, f"/api/jobs/{failed_filter['job_id']}", method="GET")
            _assert(detail["error"]["code"] == "PROVIDER_TIMEOUT", f"JobDetail.error not populated: {detail}")

            retry = _json_request(base_url, f"/api/jobs/{failed_action['job_id']}/retry-from/rough3d_submit", method="POST")
            _assert(retry["status"] == "retrying", f"retry-from failed: {retry}")
            cancel = _json_request(base_url, f"/api/jobs/{waiting_cancel['job_id']}/cancel", method="POST")
            _assert(cancel["status"] == "cancelled", f"cancel failed: {cancel}")

            action_list = _json_request(base_url, f"/api/jobs/{failed_action['job_id']}/actions", method="GET")
            _assert(action_list["items"], f"job actions list empty: {action_list}")
            retry_action = action_list["items"][0]
            _assert(retry_action["action_type"] == "retry_from_step", f"wrong action type: {retry_action}")
            _assert(retry_action["event_id"] == retry["event_id"], f"action event id mismatch: {retry_action}")
            _assert(retry_action["previous_job_status"] == "failed", f"action previous status mismatch: {retry_action}")
            retry_summary = _json_request(base_url, f"/api/jobs?query={failed_action['job_id']}", method="GET")["items"][0]
            _assert(retry_summary["action_count"] >= 1 and retry_summary["status"] == "retrying", f"summary did not reflect action: {retry_summary}")

            recovered = _json_request(base_url, "/api/runtime/recover", method="POST")
            _assert(recovered["recovered_count"] >= 1, f"expected at least one recovered job: {recovered}")
            _assert(
                any(item["job_id"] == interrupted["job_id"] for item in recovered["items"]),
                f"recovery did not include interrupted job: {recovered}",
            )
            recovered_again = _json_request(base_url, "/api/runtime/recover", method="POST")
            _assert(recovered_again["recovered_count"] == 0, f"recover should not repeat same waiting_user job: {recovered_again}")
            waiting_user_jobs = _json_request(base_url, "/api/jobs?status=waiting_user", method="GET")
            _assert(
                any(item["job_id"] == interrupted["job_id"] for item in waiting_user_jobs["items"]),
                f"waiting_user filter missing recovered job: {waiting_user_jobs}",
            )

            print(json.dumps(
                {
                    "ok": True,
                    "failed_job_id": failed_filter["job_id"],
                    "retry_job_id": failed_action["job_id"],
                    "cancel_job_id": waiting_cancel["job_id"],
                    "recovered_job_id": interrupted["job_id"],
                    "first_page": [item["job_id"] for item in first_page["items"]],
                    "action_id": retry_action["action_id"],
                },
                indent=2,
                ensure_ascii=False,
            ))
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
        "text": f"{client_request_id} 赤金国风龙纹长剑，3渲2，逼真外观，仅作为虚构 Unity 游戏资产",
        "sketch_asset_id": None,
        "reference_asset_ids": [],
        "auto_run": True,
        "target": {"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
    }
    return _json_request(base_url, "/api/weapons", method="POST", body=body, idempotency_key=client_request_id)


def _set_job_timestamp(db_path: Path, job_id: str, updated_at: str) -> None:
    with sqlite3.connect(db_path) as conn:
        conn.execute("UPDATE generation_jobs SET updated_at = ? WHERE job_id = ?", (updated_at, job_id))


def _mark_job_failed(db_path: Path, job_id: str, step_name: str, updated_at: str) -> None:
    with sqlite3.connect(db_path) as conn:
        conn.row_factory = sqlite3.Row
        weapon_id = conn.execute("SELECT weapon_id FROM generation_jobs WHERE job_id = ?", (job_id,)).fetchone()["weapon_id"]
        next_seq = conn.execute("SELECT COALESCE(MAX(seq), 0) + 1 FROM agent_events WHERE job_id = ?", (job_id,)).fetchone()[0]
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'failed', current_step = ?, error_code = 'PROVIDER_TIMEOUT',
                error_message = 'Synthetic provider timeout for history search smoke.',
                updated_at = ?, finished_at = ?
            WHERE job_id = ?
            """,
            (step_name, updated_at, updated_at, job_id),
        )
        conn.execute(
            """
            UPDATE job_steps
            SET status = 'failed', error_code = 'PROVIDER_TIMEOUT',
                error_message = 'synthetic failure', finished_at = ?
            WHERE job_id = ? AND step_name = ?
            """,
            (updated_at, job_id, step_name),
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
                f"evt_{job_id}_{next_seq:04d}",
                job_id,
                next_seq,
                weapon_id,
                step_name,
                "Synthetic provider timeout for history search smoke.",
                json.dumps({"progress": 0.9, "error_code": "PROVIDER_TIMEOUT"}),
                updated_at,
            ),
        )


def _mark_job_waiting(db_path: Path, job_id: str, step_name: str, updated_at: str) -> None:
    with sqlite3.connect(db_path) as conn:
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'waiting_provider', current_step = ?, updated_at = ?, finished_at = NULL
            WHERE job_id = ?
            """,
            (step_name, updated_at, job_id),
        )
        conn.execute(
            "UPDATE job_steps SET status = 'waiting_provider', finished_at = NULL WHERE job_id = ? AND step_name = ?",
            (job_id, step_name),
        )


def _mark_job_running(db_path: Path, job_id: str, step_name: str, updated_at: str) -> None:
    with sqlite3.connect(db_path) as conn:
        conn.execute(
            """
            UPDATE generation_jobs
            SET status = 'running', current_step = ?, updated_at = ?, finished_at = NULL
            WHERE job_id = ?
            """,
            (step_name, updated_at, job_id),
        )
        conn.execute(
            "UPDATE job_steps SET status = 'running', finished_at = NULL, resumable_after_restart = 1 WHERE job_id = ? AND step_name = ?",
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


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
