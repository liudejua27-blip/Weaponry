#!/usr/bin/env python3
"""Smoke test for the local HTTP 3D provider adapter boundary."""

from __future__ import annotations

import base64
import json
import os
import sqlite3
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any, Dict, Optional

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest, Generate3DRequest  # noqa: E402
from wushen_agent.providers.llm import MockLLMProvider  # noqa: E402
from wushen_agent.providers.image import MockComfyUIProvider  # noqa: E402
from wushen_agent.providers.three_d import (  # noqa: E402
    LocalHTTPThreeDConfig,
    LocalHTTPThreeDProvider,
    mock_glb_payload,
    mock_unity_material,
)


class FakeThreeDState:
    next_index = 1
    tasks: Dict[str, Dict[str, Any]] = {}
    calls: list[Dict[str, Any]] = []

    @classmethod
    def reset(cls) -> None:
        cls.next_index = 1
        cls.tasks = {}
        cls.calls = []

    @classmethod
    def submit(cls, payload: Dict[str, Any]) -> str:
        task_id = f"sf3d_task_{cls.next_index:03d}"
        cls.next_index += 1
        cls.tasks[task_id] = {
            "payload": payload,
            "poll_count": 0,
            "cancelled": False,
            "fetched": False,
        }
        cls.calls.append({"op": "submit", "task_id": task_id, "model_id": payload.get("model_id")})
        return task_id

    @classmethod
    def poll(cls, task_id: str) -> Dict[str, Any]:
        task = cls.tasks[task_id]
        task["poll_count"] += 1
        cls.calls.append({"op": "poll", "task_id": task_id, "poll_count": task["poll_count"]})
        if task["cancelled"]:
            return {"provider_task_id": task_id, "status": "cancelled", "progress": 0, "metadata": {"engine": "fake_sf3d"}}
        if task["poll_count"] == 1:
            return {"provider_task_id": task_id, "status": "polling", "progress": 0.45, "metadata": {"engine": "fake_sf3d"}}
        return {"provider_task_id": task_id, "status": "succeeded", "progress": 1, "metadata": {"engine": "fake_sf3d"}}

    @classmethod
    def fetch(cls, task_id: str) -> Dict[str, Any]:
        task = cls.tasks[task_id]
        task["fetched"] = True
        payload = task["payload"]
        weapon_id = str(payload["weapon_id"])
        model_id = str(payload["model_id"])
        cls.calls.append({"op": "fetch", "task_id": task_id})
        return {
            "provider_task_id": task_id,
            "raw_glb_base64": base64.b64encode(mock_glb_payload(weapon_id, model_id=model_id, stage="raw")).decode("ascii"),
            "normalized_glb_base64": base64.b64encode(mock_glb_payload(weapon_id, model_id=model_id, stage="normalized")).decode("ascii"),
            "optimized_glb_base64": base64.b64encode(mock_glb_payload(weapon_id, model_id=model_id, stage="optimized")).decode("ascii"),
            "unity_material_json": mock_unity_material(weapon_id),
            "metrics": {
                "triangle_count": 36,
                "mesh_count": 1,
                "material_count": 1,
                "provider_runtime": "fake_sf3d_http",
            },
            "metadata": {
                "engine": "fake_sf3d",
                "source": "local_http_smoke",
                "non_manufacturing_asset": True,
            },
        }

    @classmethod
    def cancel(cls, task_id: str) -> Dict[str, Any]:
        cls.tasks[task_id]["cancelled"] = True
        cls.calls.append({"op": "cancel", "task_id": task_id})
        return {"provider_task_id": task_id, "status": "cancelled", "metadata": {"engine": "fake_sf3d"}}


class FakeThreeDHandler(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API.
        if self.path == "/v1/rough-models":
            payload = self._read_json()
            _assert(payload["schema_version"] == "WushenThreeDProviderRequest@1", "request schema version mismatch")
            _assert(payload["target_format"] == "glb", "target format mismatch")
            _assert(payload["output_contract"]["non_manufacturing_asset"] is True, "non-manufacturing boundary missing")
            _assert(base64.b64decode(payload["source_image"]["data_base64"], validate=True), "source image payload missing")
            task_id = FakeThreeDState.submit(payload)
            self._send_json(
                {
                    "provider_task_id": task_id,
                    "status": "polling",
                    "metadata": {"engine": "fake_sf3d", "adapter_contract": "local_http"},
                }
            )
            return
        if self.path.startswith("/v1/rough-models/") and self.path.endswith("/cancel"):
            task_id = self.path.split("/")[3]
            if task_id not in FakeThreeDState.tasks:
                self.send_error(404)
                return
            self._send_json(FakeThreeDState.cancel(task_id))
            return
        self.send_error(404)

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API.
        if not self.path.startswith("/v1/rough-models/"):
            self.send_error(404)
            return
        parts = self.path.split("/")
        task_id = parts[3]
        if task_id not in FakeThreeDState.tasks:
            self.send_error(404)
            return
        if len(parts) == 5 and parts[4] == "result":
            self._send_json(FakeThreeDState.fetch(task_id))
            return
        self._send_json(FakeThreeDState.poll(task_id))

    def log_message(self, _format: str, *_args: Any) -> None:
        return

    def _read_json(self) -> Dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        return json.loads(self.rfile.read(length).decode("utf-8"))

    def _send_json(self, payload: Dict[str, Any]) -> None:
        data = json.dumps(payload).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


def main() -> int:
    FakeThreeDState.reset()
    server = ThreadingHTTPServer(("127.0.0.1", 0), FakeThreeDHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    base_url = f"http://127.0.0.1:{server.server_port}"
    previous_async = os.environ.get("WUSHEN_GENERATE3D_ASYNC")
    os.environ["WUSHEN_GENERATE3D_ASYNC"] = "1"
    try:
        with tempfile.TemporaryDirectory(prefix="wushen_p0_local_http_3d_") as tmp:
            store = SQLiteAssetStore(
                library_root=Path(tmp) / "WushenForgeLibrary",
                migrations_dir=ROOT / "migrations",
                llm_provider=MockLLMProvider(),
                image_provider=MockComfyUIProvider(),
                three_d_provider=LocalHTTPThreeDProvider(
                    LocalHTTPThreeDConfig(
                        base_url=base_url,
                        provider_id="local_http_3d",
                        timeout_seconds=5,
                        poll_interval_seconds=0.01,
                        max_wait_seconds=1,
                        retry_attempts=1,
                    )
                ),
            )
            source = _create_source(store)
            _run_wait_then_fetch_case(store, source)
            _run_cancel_case(store, source)
            findings, stats = validate(Path(tmp) / "WushenForgeLibrary", Path(tmp) / "WushenForgeLibrary" / "library.db")
            _assert(stats["blockers"] == 0, f"asset library blockers after local HTTP 3D smoke: {findings}")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "provider": "local_http_3d",
                        "calls": FakeThreeDState.calls,
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
        server.shutdown()
        server.server_close()
        thread.join(timeout=2)


def _create_source(store: SQLiteAssetStore) -> Dict[str, str]:
    job = store.create_weapon(
        CreateWeaponRequest(
            client_request_id="local-http-3d-source",
            text="赤金国风龙纹长剑，3渲2，逼真外观，仅作为虚构 Unity 游戏资产",
        ),
        "local-http-3d-source-key",
    )
    source_image_id = next(asset_id for asset_id, role in job.outputs["asset_roles"].items() if role == "concept_image")
    return {
        "weapon_id": job.weapon_id,
        "source_version_id": job.outputs["current_version_id"],
        "source_image_id": source_image_id,
        "library_root": str(store.library_root),
    }


def _run_wait_then_fetch_case(store: SQLiteAssetStore, source: Dict[str, str]) -> None:
    job = store.generate_3d(
        source["weapon_id"],
        _generate_request("local-http-3d-complete", source),
        "local-http-3d-complete-key",
    )
    first = store.run_worker_once("local_http_3d_smoke")
    _assert(first.status == "waiting_provider", f"first worker step should wait: {first}")
    _assert(_ops_for_job(source, job.job_id, "submit") == 1, "submit count mismatch after first worker step")
    _assert(_ops_for_job(source, job.job_id, "poll") == 1, "poll count mismatch after first worker step")
    _assert(_ops_for_job(source, job.job_id, "fetch") == 0, "fetch should not happen while provider is polling")
    _assert(_rough_asset_count(source, job.job_id) == 0, "waiting provider job wrote rough assets")

    second = store.run_worker_once("local_http_3d_smoke")
    _assert(second.status == "succeeded", f"second worker step should succeed: {second}")
    _assert(_ops_for_job(source, job.job_id, "submit") == 1, "worker re-submitted provider task")
    _assert(_ops_for_job(source, job.job_id, "poll") == 2, "worker did not poll persisted provider task")
    _assert(_ops_for_job(source, job.job_id, "fetch") == 1, "worker did not fetch exactly once")
    _assert(_rough_asset_count(source, job.job_id) == 4, "completed provider job did not write rough assets")


def _run_cancel_case(store: SQLiteAssetStore, source: Dict[str, str]) -> None:
    job = store.generate_3d(
        source["weapon_id"],
        _generate_request("local-http-3d-cancel", source),
        "local-http-3d-cancel-key",
    )
    first = store.run_worker_once("local_http_3d_smoke")
    _assert(first.status == "waiting_provider", f"cancel fixture should wait: {first}")
    cancel = store.cancel_job(job.job_id)
    _assert(cancel.status == "cancelled", f"cancel response mismatch: {cancel}")
    _assert(_ops_for_job(source, job.job_id, "cancel") == 1, "provider cancel was not called exactly once")
    no_work = store.run_worker_once("local_http_3d_smoke")
    _assert(no_work.claimed is False, f"cancelled job should not be claimed: {no_work}")
    _assert(_ops_for_job(source, job.job_id, "fetch") == 0, "cancelled provider job was fetched")
    _assert(_rough_asset_count(source, job.job_id) == 0, "cancelled provider job wrote rough assets")


def _generate_request(client_request_id: str, source: Dict[str, str]) -> Generate3DRequest:
    return Generate3DRequest(
        client_request_id=client_request_id,
        source_version_id=source["source_version_id"],
        source_image_asset_id=source["source_image_id"],
        provider_id="local_http_3d",
        target_format="glb",
        style="stylized_toon_weapon",
        orientation_policy={"forward_axis": "+Z", "long_axis": "+Y", "pivot": "grip_center"},
        scale_policy="normalized_game_asset_scale",
        build_unity_export=True,
    )


def _ops_for_job(source: Dict[str, str], job_id: str, op: str) -> int:
    task_id = _task_for_job(source, job_id)
    return len(
        [
            call
            for call in FakeThreeDState.calls
            if call["op"] == op and task_id == call["task_id"]
        ]
    )


def _task_for_job(source: Dict[str, str], job_id: str) -> Optional[str]:
    db_path = Path(source["library_root"]) / "library.db"
    with sqlite3.connect(db_path) as conn:
        row = conn.execute(
            "SELECT provider_task_id FROM provider_tasks WHERE job_id = ? ORDER BY updated_at DESC LIMIT 1",
            (job_id,),
        ).fetchone()
        return str(row[0]) if row else None


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


def _assert(condition: Any, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
