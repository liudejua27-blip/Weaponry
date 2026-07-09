#!/usr/bin/env python3
"""Smoke test for the Wushen local 3D runtime wrapper process."""

from __future__ import annotations

import json
import os
import signal
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

from wushen_agent.asset_store import SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest, Generate3DRequest  # noqa: E402
from wushen_agent.providers.image import MockComfyUIProvider  # noqa: E402
from wushen_agent.providers.llm import MockLLMProvider  # noqa: E402
from wushen_agent.providers.three_d import LocalHTTPThreeDConfig, LocalHTTPThreeDProvider  # noqa: E402


def main() -> int:
    previous_async = os.environ.get("WUSHEN_GENERATE3D_ASYNC")
    os.environ["WUSHEN_GENERATE3D_ASYNC"] = "1"
    runtime: Optional[subprocess.Popen[str]] = None
    try:
        with tempfile.TemporaryDirectory(prefix="wushen_p0_local_3d_runtime_") as tmp:
            port = _free_port()
            base_url = f"http://127.0.0.1:{port}"
            runtime = _start_runtime(base_url=base_url, port=port, tmp=Path(tmp))
            _wait_for_health(base_url)
            store = SQLiteAssetStore(
                library_root=Path(tmp) / "WushenForgeLibrary",
                migrations_dir=ROOT / "migrations",
                llm_provider=MockLLMProvider(),
                image_provider=MockComfyUIProvider(),
                three_d_provider=LocalHTTPThreeDProvider(
                    LocalHTTPThreeDConfig(
                        base_url=base_url,
                        provider_id="local_http_3d_runtime",
                        timeout_seconds=5,
                        poll_interval_seconds=0.01,
                        max_wait_seconds=2,
                        retry_attempts=1,
                    )
                ),
            )
            source = _create_source(store)
            completed_provider_task_id = _run_wait_then_fetch_case(store, source, base_url)
            cancelled_provider_task_id = _run_cancel_case(store, source, base_url)
            findings, stats = validate(Path(tmp) / "WushenForgeLibrary", Path(tmp) / "WushenForgeLibrary" / "library.db")
            _assert(stats["blockers"] == 0, f"asset library blockers after local runtime wrapper smoke: {findings}")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "runtime": "wushen_local_3d_runtime",
                        "backend": "mock",
                        "base_url": base_url,
                        "completed_provider_task_id": completed_provider_task_id,
                        "cancelled_provider_task_id": cancelled_provider_task_id,
                        "asset_count": stats["asset_files"],
                    },
                    indent=2,
                    ensure_ascii=False,
                )
            )
            return 0
    finally:
        if previous_async is None:
            os.environ.pop("WUSHEN_GENERATE3D_ASYNC", None)
        else:
            os.environ["WUSHEN_GENERATE3D_ASYNC"] = previous_async
        if runtime is not None:
            _terminate_process(runtime)


def _start_runtime(*, base_url: str, port: int, tmp: Path) -> subprocess.Popen[str]:
    process = subprocess.Popen(
        [
            sys.executable,
            str(ROOT / "scripts" / "wushen_local_3d_runtime.py"),
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
            "--backend",
            "mock",
            "--work-dir",
            str(tmp / "runtime-work"),
            "--mock-delay-seconds",
            "0.75",
            "--task-timeout-seconds",
            "10",
        ],
        cwd=str(ROOT),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    deadline = time.time() + 5
    while time.time() < deadline:
        if process.poll() is not None:
            stderr = process.stderr.read() if process.stderr else ""
            raise RuntimeError(f"local 3D runtime exited before listening at {base_url}: {stderr}")
        try:
            _http_json(f"{base_url}/health")
            return process
        except (urllib.error.URLError, TimeoutError, RuntimeError):
            time.sleep(0.05)
    raise RuntimeError(f"local 3D runtime did not become healthy at {base_url}")


def _wait_for_health(base_url: str) -> None:
    health = _http_json(f"{base_url}/health")
    _assert(health.get("status") == "ok", f"runtime health mismatch: {health}")
    _assert(health.get("backend") == "mock", f"runtime backend mismatch: {health}")


def _create_source(store: SQLiteAssetStore) -> Dict[str, str]:
    job = store.create_weapon(
        CreateWeaponRequest(
            client_request_id="local-3d-runtime-source",
            text="赤金国风龙纹长剑，3渲2，逼真外观，仅作为虚构 Unity 游戏资产",
        ),
        "local-3d-runtime-source-key",
    )
    source_image_id = next(asset_id for asset_id, role in job.outputs["asset_roles"].items() if role == "concept_image")
    return {
        "weapon_id": job.weapon_id,
        "source_version_id": job.outputs["current_version_id"],
        "source_image_id": source_image_id,
        "library_root": str(store.library_root),
    }


def _run_wait_then_fetch_case(store: SQLiteAssetStore, source: Dict[str, str], base_url: str) -> str:
    job = store.generate_3d(
        source["weapon_id"],
        _generate_request("local-3d-runtime-complete", source),
        "local-3d-runtime-complete-key",
    )
    first = store.run_worker_once("local_3d_runtime_smoke")
    _assert(first.status == "waiting_provider", f"first worker step should wait: {first}")
    provider_task_id = _task_for_job(source, job.job_id)
    _assert(provider_task_id, "provider task id missing after first worker step")
    status = _http_json(f"{base_url}/v1/rough-models/{provider_task_id}")
    _assert(status.get("status") in {"submitted", "polling"}, f"runtime task should still be in progress: {status}")
    _assert(_rough_asset_count(source, job.job_id) == 0, "waiting provider job wrote rough assets")

    _wait_for_task_status(base_url, provider_task_id, "succeeded")
    second = store.run_worker_once("local_3d_runtime_smoke")
    _assert(second.status == "succeeded", f"second worker step should succeed: {second}")
    _assert(_rough_asset_count(source, job.job_id) == 4, "completed provider job did not write rough assets")
    return provider_task_id


def _run_cancel_case(store: SQLiteAssetStore, source: Dict[str, str], base_url: str) -> str:
    job = store.generate_3d(
        source["weapon_id"],
        _generate_request("local-3d-runtime-cancel", source),
        "local-3d-runtime-cancel-key",
    )
    first = store.run_worker_once("local_3d_runtime_smoke")
    _assert(first.status == "waiting_provider", f"cancel fixture should wait: {first}")
    provider_task_id = _task_for_job(source, job.job_id)
    _assert(provider_task_id, "provider task id missing before cancel")
    cancel = store.cancel_job(job.job_id)
    _assert(cancel.status == "cancelled", f"cancel response mismatch: {cancel}")
    _wait_for_task_status(base_url, provider_task_id, "cancelled")
    no_work = store.run_worker_once("local_3d_runtime_smoke")
    _assert(no_work.claimed is False, f"cancelled job should not be claimed: {no_work}")
    _assert(_rough_asset_count(source, job.job_id) == 0, "cancelled provider job wrote rough assets")
    return provider_task_id


def _generate_request(client_request_id: str, source: Dict[str, str]) -> Generate3DRequest:
    return Generate3DRequest(
        client_request_id=client_request_id,
        source_version_id=source["source_version_id"],
        source_image_asset_id=source["source_image_id"],
        provider_id="local_http_3d_runtime",
        target_format="glb",
        style="stylized_toon_weapon",
        orientation_policy={"forward_axis": "+Z", "long_axis": "+Y", "pivot": "grip_center"},
        scale_policy="normalized_game_asset_scale",
        build_unity_export=True,
    )


def _task_for_job(source: Dict[str, str], job_id: str) -> str:
    db_path = Path(source["library_root"]) / "library.db"
    with sqlite3.connect(db_path) as conn:
        row = conn.execute(
            "SELECT provider_task_id FROM provider_tasks WHERE job_id = ? ORDER BY updated_at DESC LIMIT 1",
            (job_id,),
        ).fetchone()
        return str(row[0]) if row and row[0] else ""


def _rough_asset_count(source: Dict[str, str], job_id: str) -> int:
    db_path = Path(source["library_root"]) / "library.db"
    with sqlite3.connect(db_path) as conn:
        return int(
            conn.execute(
                """
                SELECT COUNT(*)
                FROM asset_files
                WHERE job_id = ?
                  AND role IN ('rough_raw_glb', 'rough_normalized_glb', 'rough_optimized_glb', 'unity_material_json')
                """,
                (job_id,),
            ).fetchone()[0]
        )


def _wait_for_task_status(base_url: str, provider_task_id: str, expected: str) -> None:
    deadline = time.time() + 5
    last_status: Optional[Dict[str, Any]] = None
    while time.time() < deadline:
        last_status = _http_json(f"{base_url}/v1/rough-models/{provider_task_id}")
        if last_status.get("status") == expected:
            return
        time.sleep(0.05)
    raise AssertionError(f"task {provider_task_id} did not reach {expected}: {last_status}")


def _http_json(url: str) -> Dict[str, Any]:
    with urllib.request.urlopen(url, timeout=1) as response:
        payload = json.loads(response.read().decode("utf-8"))
    if not isinstance(payload, dict):
        raise RuntimeError(f"HTTP response was not a JSON object: {url}")
    return payload


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _terminate_process(process: subprocess.Popen[str]) -> None:
    try:
        if process.poll() is not None:
            return
        if os.name == "posix":
            os.killpg(process.pid, signal.SIGTERM)
        else:
            process.terminate()
        process.wait(timeout=5)
    except Exception:
        try:
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGKILL)
            else:
                process.kill()
        except Exception:
            return


def _assert(condition: Any, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
