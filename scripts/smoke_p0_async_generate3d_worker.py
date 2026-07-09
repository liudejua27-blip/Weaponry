#!/usr/bin/env python3
"""Smoke test for opt-in async generate-3d worker foundation."""

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

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_p0_async_3d_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(port, library_root, recover_on_startup=False)
        try:
            _wait_for_health(base_url, process)
            created = _create_weapon(base_url, "p0-async-source")
            source_job = _json_request(base_url, f"/api/jobs/{created['job_id']}", method="GET")
            source_version_id = source_job["outputs"]["current_version_id"]
            source_image_id = _asset_id_by_role(source_job, "concept_image")
            generate_body = _generate_body("p0-async-generate", source_version_id, source_image_id)

            accepted = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/generate-3d",
                method="POST",
                body=generate_body,
                idempotency_key="p0-async-generate-key",
            )
            _assert(accepted["status"] == "queued", f"async generate-3d did not return queued: {accepted}")
            queued_job = _json_request(base_url, f"/api/jobs/{accepted['job_id']}", method="GET")
            _assert(queued_job["status"] == "queued", f"queued job status mismatch: {queued_job['status']}")
            _assert(queued_job["outputs"]["current_version_id"] is None, "queued job already has output version")
            _assert(_seqs(queued_job) == list(range(1, len(queued_job["events"]) + 1)), "queued event seqs were not contiguous")
            detail = _json_request(base_url, f"/api/weapons/{created['weapon_id']}", method="GET")
            _assert(detail["current_version_id"] == source_version_id, "queued job moved current version before worker")
            _assert(not _rough_versions_for_job(detail, accepted["job_id"]), "queued job created a rough_3d version before worker")

            db_path = library_root / "library.db"
            with sqlite3.connect(db_path) as conn:
                job_count = conn.execute(
                    "SELECT COUNT(*) FROM generation_jobs WHERE idempotency_scope = ? AND idempotency_key = ?",
                    (f"POST /api/weapons/{created['weapon_id']}/generate-3d", "p0-async-generate-key"),
                ).fetchone()[0]
                model_count = conn.execute("SELECT COUNT(*) FROM models_3d WHERE job_id = ?", (accepted["job_id"],)).fetchone()[0]
                rough_asset_count = conn.execute(
                    """
                    SELECT COUNT(*) FROM asset_files
                    WHERE job_id = ? AND role IN ('rough_raw_glb', 'rough_normalized_glb', 'rough_optimized_glb', 'unity_material_json')
                    """,
                    (accepted["job_id"],),
                ).fetchone()[0]
            _assert(job_count == 1, "queued idempotency scope created duplicate job rows")
            _assert(model_count == 0, "queued job created model row before worker")
            _assert(rough_asset_count == 0, "queued job created rough assets before worker")

            replay = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/generate-3d",
                method="POST",
                body=generate_body,
                idempotency_key="p0-async-generate-key",
            )
            _assert(replay["job_id"] == accepted["job_id"] and replay["status"] == "queued", "queued idempotency replay mismatch")
            conflict_status, conflict = _json_request_allow_error(
                base_url,
                f"/api/weapons/{created['weapon_id']}/generate-3d",
                method="POST",
                body={**generate_body, "source_image_asset_id": "file_different"},
                idempotency_key="p0-async-generate-key",
            )
            _assert(conflict_status == 409 and conflict["error"]["code"] == "IDEMPOTENCY_CONFLICT", "async conflict was not reported")

            work = _json_request(base_url, "/api/runtime/work-once", method="POST")
            _assert(work["claimed"] is True and work["job_id"] == accepted["job_id"], f"worker did not claim queued job: {work}")
            _assert(work["status"] == "succeeded", f"worker did not complete job: {work}")

            completed = _json_request(base_url, f"/api/jobs/{accepted['job_id']}", method="GET")
            _assert(completed["status"] == "succeeded", f"completed job status mismatch: {completed['status']}")
            _assert(completed["current_step"] == "finalize_job", "completed job did not end at finalize_job")
            _assert(completed["outputs"]["current_version_id"] != source_version_id, "worker did not create a child version")
            roles = set(completed["outputs"]["asset_roles"].values())
            expected_roles = {"other", "rough_raw_glb", "rough_normalized_glb", "rough_optimized_glb", "unity_material_json", "quality_report"}
            _assert(expected_roles.issubset(roles), f"worker job missing roles: {expected_roles - roles}")
            _assert(_seqs(completed) == list(range(1, len(completed["events"]) + 1)), "completed event seqs were not contiguous")
            succeeded_steps = [event["step"] for event in completed["events"] if event["status"] == "succeeded"]
            _assert(
                succeeded_steps == ["rough3d_plan", "rough3d_submit", "model_qc_optimize", "asset_commit_model", "finalize_job"],
                f"unexpected succeeded worker steps: {succeeded_steps}",
            )
            runtime = _json_request(base_url, f"/api/jobs/{accepted['job_id']}/runtime", method="GET")
            rough_task = next(task for task in runtime["provider_tasks"] if task["step"] == "rough3d_submit")
            _assert(rough_task["provider_kind"] == "three_d" and rough_task["provider_id"] == "mock_3d", "rough3d provider task metadata mismatch")
            _assert(rough_task["provider_task_id"], "rough3d provider task id missing")
            _assert(rough_task["status"] == "succeeded", "rough3d provider task was not succeeded")
            rough_checkpoint = next(item for item in runtime["checkpoints"] if item["step"] == "rough3d_submit")
            _assert(rough_checkpoint["status"] == "completed", "rough3d checkpoint was not completed")
            _assert(rough_checkpoint["resume_policy"] == "skip_completed", "rough3d checkpoint resume policy mismatch")

            detail = _json_request(base_url, f"/api/weapons/{created['weapon_id']}", method="GET")
            current = next(item for item in detail["versions"] if item["version_id"] == completed["outputs"]["current_version_id"])
            _assert(current["parent_version_id"] == source_version_id and current["version_type"] == "rough_3d", "worker version parent/type mismatch")
            with sqlite3.connect(db_path) as conn:
                committed_count = conn.execute(
                    "SELECT COUNT(*) FROM weapon_versions WHERE job_id = ? AND version_type = 'rough_3d' AND status = 'committed'",
                    (accepted["job_id"],),
                ).fetchone()[0]
                model_count = conn.execute("SELECT COUNT(*) FROM models_3d WHERE job_id = ?", (accepted["job_id"],)).fetchone()[0]
                runner_row = conn.execute(
                    "SELECT runner_id, lease_expires_at FROM generation_jobs WHERE job_id = ?",
                    (accepted["job_id"],),
                ).fetchone()
            _assert(committed_count == 1, "worker committed duplicate rough_3d versions")
            _assert(model_count == 1, "worker created duplicate model rows")
            _assert(runner_row[0] is None and runner_row[1] is None, "worker lease was not cleared")
            findings, stats = validate(library_root, db_path)
            _assert(stats["blockers"] == 0, f"asset library blockers after worker completion: {findings}")

            replay_after = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/generate-3d",
                method="POST",
                body=generate_body,
                idempotency_key="p0-async-generate-key",
            )
            _assert(replay_after["job_id"] == accepted["job_id"] and replay_after["status"] == "succeeded", "completed replay mismatch")

            recovery_job = _json_request(
                base_url,
                f"/api/weapons/{created['weapon_id']}/generate-3d",
                method="POST",
                body=_generate_body("p0-async-recover", source_version_id, source_image_id),
                idempotency_key="p0-async-recover-key",
            )
            _assert(recovery_job["status"] == "queued", "recovery fixture job was not queued")
        finally:
            _stop_process(process)

        process = _start_agent(port, library_root, recover_on_startup=False)
        try:
            _wait_for_health(base_url, process)
            before_recovery = _json_request(base_url, f"/api/jobs/{recovery_job['job_id']}", method="GET")
            _assert(before_recovery["status"] == "queued", "queued job changed before manual recovery")
            recovered = _json_request(base_url, "/api/runtime/recover", method="POST")
            item = next(item for item in recovered["items"] if item["job_id"] == recovery_job["job_id"])
            _assert(item["previous_status"] == "queued" and item["status"] == "waiting_user", f"recovery item mismatch: {item}")
            recovered_detail = _json_request(base_url, f"/api/jobs/{recovery_job['job_id']}", method="GET")
            _assert(recovered_detail["status"] == "waiting_user", "manual recovery did not pause job")
            recovered_runtime = _json_request(base_url, f"/api/jobs/{recovery_job['job_id']}/runtime", method="GET")
            _assert(recovered_runtime["resumable"] is True, "manual recovery job was not resumable")
            retry = _json_request(base_url, f"/api/jobs/{recovery_job['job_id']}/retry-from/{item['resume_from_step']}", method="POST")
            _assert(retry["status"] == "retrying" and retry["previous_status"] == "waiting_user", f"retry after recovery mismatch: {retry}")
            recovery_work = _json_request(base_url, "/api/runtime/work-once", method="POST")
            _assert(recovery_work["claimed"] is True and recovery_work["status"] == "succeeded", f"recovered worker did not complete: {recovery_work}")
            final_recovered = _json_request(base_url, f"/api/jobs/{recovery_job['job_id']}", method="GET")
            _assert(final_recovered["status"] == "succeeded", "recovered retry job did not succeed")
            with sqlite3.connect(library_root / "library.db") as conn:
                rough_count = conn.execute(
                    "SELECT COUNT(*) FROM weapon_versions WHERE job_id = ? AND version_type = 'rough_3d' AND status = 'committed'",
                    (recovery_job["job_id"],),
                ).fetchone()[0]
            _assert(rough_count == 1, "recovered job committed duplicate rough_3d versions")
            findings, stats = validate(library_root, library_root / "library.db")
            _assert(stats["blockers"] == 0, f"asset library blockers after recovery worker: {findings}")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "worker_job_id": accepted["job_id"],
                        "recovered_job_id": recovery_job["job_id"],
                        "asset_count": stats["asset_files"],
                    },
                    indent=2,
                    ensure_ascii=False,
                )
            )
            return 0
        finally:
            _stop_process(process)


def _start_agent(port: int, library_root: Path, *, recover_on_startup: bool) -> subprocess.Popen:
    env = os.environ.copy()
    env["WUSHEN_LIBRARY_ROOT"] = str(library_root)
    env["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
    env["WUSHEN_GENERATE3D_ASYNC"] = "1"
    env["WUSHEN_RECOVER_ON_STARTUP"] = "1" if recover_on_startup else "0"
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


def _stop_process(process: subprocess.Popen) -> None:
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


def _generate_body(client_request_id: str, source_version_id: str, source_image_id: str) -> Dict[str, Any]:
    return {
        "client_request_id": client_request_id,
        "source_version_id": source_version_id,
        "source_image_asset_id": source_image_id,
        "provider_id": "mock_3d",
        "target_format": "glb",
        "style": "stylized_toon_weapon",
        "orientation_policy": {"forward_axis": "+Z", "long_axis": "+Y", "pivot": "grip_center"},
        "scale_policy": "normalized_game_asset_scale",
        "build_unity_export": True,
    }


def _asset_id_by_role(job: Dict[str, Any], role: str) -> str:
    for asset_id, asset_role in job["outputs"]["asset_roles"].items():
        if asset_role == role:
            return asset_id
    raise AssertionError(f"Missing asset role {role}")


def _rough_versions_for_job(detail: Dict[str, Any], job_id: str) -> list[Dict[str, Any]]:
    return [item for item in detail["versions"] if item["job_id"] == job_id and item["version_type"] == "rough_3d"]


def _seqs(job: Dict[str, Any]) -> list[int]:
    return [event["seq"] for event in job["events"]]


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


def _json_request_allow_error(base_url: str, path: str, *, method: str, body: Dict[str, Any], idempotency_key: str) -> Tuple[int, Dict[str, Any]]:
    payload = json.dumps(body).encode("utf-8")
    request = urllib.request.Request(
        base_url + path,
        data=payload,
        headers={"Content-Type": "application/json", "Idempotency-Key": idempotency_key},
        method=method,
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        return exc.code, json.loads(exc.read().decode("utf-8"))


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
