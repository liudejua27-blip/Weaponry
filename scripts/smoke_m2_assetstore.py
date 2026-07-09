#!/usr/bin/env python3
"""M2 smoke test for SQLite AssetStore-backed Agent API."""

from __future__ import annotations

import json
import os
import socket
import sqlite3
import struct
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

from wushen_agent.spec_validation import validate_quality_report  # noqa: E402


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_m2_") as tmp:
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
            body = {
                "client_request_id": "smoke-m2-001",
                "text": "赤金国风龙纹长剑，3渲2，逼真外观，但仅作为虚构 Unity 游戏资产",
                "sketch_asset_id": None,
                "reference_asset_ids": [],
                "auto_run": True,
                "target": {"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
            }
            first = _json_request(base_url, "/api/weapons", method="POST", body=body, idempotency_key="smoke-key-001")
            second = _json_request(base_url, "/api/weapons", method="POST", body=body, idempotency_key="smoke-key-001")
            _assert(first["job_id"] == second["job_id"], "idempotency replay returned a different job_id")
            _assert(first["weapon_id"] == second["weapon_id"], "idempotency replay returned a different weapon_id")

            conflict_body = dict(body, text="同一个 key 但不同请求体")
            status, conflict = _json_request_allow_error(
                base_url, "/api/weapons", method="POST", body=conflict_body, idempotency_key="smoke-key-001"
            )
            _assert(status == 409, f"expected 409 idempotency conflict, got {status}: {conflict}")
            _assert(conflict["error"]["code"] == "IDEMPOTENCY_CONFLICT", "conflict response did not use error envelope")

            job = _json_request(base_url, f"/api/jobs/{first['job_id']}", method="GET")
            _assert(len(job["events"]) >= 5, "job detail did not include persisted events")
            event_stream = _read_text(base_url, first["event_stream_url"])
            _assert("event: job.event" in event_stream, "SSE stream did not include job.event frames")
            _assert("request_guard" in event_stream and "finalize_job" in event_stream, "SSE stream missed expected steps")
            last_event_id = job["events"][0]["id"]
            resumed = _read_text(base_url, f"{first['event_stream_url']}?after={last_event_id}")
            _assert(last_event_id not in resumed, "SSE resume repeated the last consumed event")

            db_path = library_root / "library.db"
            _assert(db_path.exists(), "library.db was not created")
            _check_db_rows(library_root, db_path, first["job_id"])
            findings, stats = validate(library_root, db_path)
            _assert(stats["blockers"] == 0, f"asset library blockers: {findings}")

            print(
                json.dumps(
                    {
                        "ok": True,
                        "library_root": str(library_root),
                        "db": str(db_path),
                        "weapon_id": first["weapon_id"],
                        "job_id": first["job_id"],
                        "event_count": len(job["events"]),
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


def _read_text(base_url: str, path: str) -> str:
    with urllib.request.urlopen(f"{base_url}{path}", timeout=10) as response:
        return response.read().decode("utf-8")


def _check_db_rows(library_root: Path, db_path: Path, job_id: str) -> None:
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    expected = {
        "weapons": 1,
        "generation_jobs": 1,
        "job_steps": 7,
        "weapon_versions": 1,
        "weapon_specs": 1,
        "asset_files": 8,
        "agent_events": 7,
        "models_3d": 1,
    }
    for table, minimum in expected.items():
        count = conn.execute(f"SELECT COUNT(*) AS count FROM {table}").fetchone()["count"]
        _assert(count >= minimum, f"{table} expected at least {minimum} rows, got {count}")
    fk_violations = conn.execute("PRAGMA foreign_key_check").fetchall()
    _assert(not fk_violations, f"foreign key violations: {fk_violations}")
    _check_create_weapon_traceability(library_root, conn, job_id)


def _check_create_weapon_traceability(library_root: Path, conn: sqlite3.Connection, job_id: str) -> None:
    assets = conn.execute(
        """
        SELECT file_id, role, weapon_id, version_id, object_path, sha256, width, height, metadata_json
        FROM asset_files
        WHERE job_id = ? AND soft_deleted_at IS NULL
        """,
        (job_id,),
    ).fetchall()
    by_role = {}
    for asset in assets:
        by_role.setdefault(asset["role"], []).append(asset)
    required_roles = {
        "weapon_spec",
        "prompt",
        "negative_prompt",
        "comfyui_workflow",
        "concept_image",
        "quality_report",
        "rough_raw_glb",
        "unity_material_json",
    }
    missing = sorted(required_roles - set(by_role))
    _assert(not missing, f"missing create_weapon asset roles: {missing}")

    concept = by_role["concept_image"][0]
    workflow = by_role["comfyui_workflow"][0]
    report_asset = by_role["quality_report"][0]
    concept_meta = json.loads(concept["metadata_json"])
    workflow_meta = json.loads(workflow["metadata_json"])
    _assert(concept["width"] and concept["height"], "concept image did not persist width/height")
    _assert(concept_meta["workflow_asset_id"] == workflow["file_id"], "concept image metadata did not reference workflow asset")
    _assert(concept_meta["prompt_asset_id"] == by_role["prompt"][0]["file_id"], "concept image metadata did not reference prompt asset")
    _assert(workflow_meta["provider_task_id"], "workflow metadata did not include provider_task_id")
    _assert(workflow_meta["workflow_sha256"] == workflow["sha256"], "workflow metadata sha256 did not match asset sha256")
    _assert(workflow_meta["checkpoint_name"], "workflow metadata did not include checkpoint_name")
    sampler = workflow_meta["generation_provenance"]["sampler"]
    _assert(sampler["sampler_name"] == "euler", "workflow metadata did not include sampler_name")
    _assert(sampler["scheduler"] == "normal", "workflow metadata did not include scheduler")
    _assert(sampler["steps"] == 24, "workflow metadata did not include steps")
    _assert(sampler["cfg"] == 7, "workflow metadata did not include cfg")

    report = json.loads((library_root / report_asset["object_path"]).read_text(encoding="utf-8"))
    validate_quality_report(report, provider_id="smoke_m2")
    _assert(report["target_type"] == "concept_image", "quality report target_type was not concept_image")
    _assert(report["target_id"] == concept["file_id"], "quality report did not target the concept image")
    _assert(report["status"] == "passed", "quality report did not pass")
    _assert_valid_glb(library_root / by_role["rough_raw_glb"][0]["object_path"])

    events = conn.execute(
        """
        SELECT seq, step, artifact_asset_id, metadata_json
        FROM agent_events
        WHERE job_id = ?
        ORDER BY seq ASC
        """,
        (job_id,),
    ).fetchall()
    expected_steps = [
        "request_guard",
        "weapon_spec_planner",
        "prompt_builder",
        "image_submit",
        "image_quality_check",
        "rough3d_submit",
        "finalize_job",
    ]
    _assert([event["step"] for event in events] == expected_steps, "agent event sequence changed")
    event_by_step = {event["step"]: event for event in events}
    _assert(event_by_step["weapon_spec_planner"]["artifact_asset_id"] == by_role["weapon_spec"][0]["file_id"], "spec event artifact mismatch")
    _assert(event_by_step["prompt_builder"]["artifact_asset_id"] == by_role["prompt"][0]["file_id"], "prompt event artifact mismatch")
    _assert(event_by_step["image_submit"]["artifact_asset_id"] == concept["file_id"], "image event artifact mismatch")
    _assert(event_by_step["image_quality_check"]["artifact_asset_id"] == report_asset["file_id"], "quality event artifact mismatch")
    quality_meta = json.loads(event_by_step["image_quality_check"]["metadata_json"])
    _assert(quality_meta["target_asset_id"] == concept["file_id"], "quality event did not reference concept image")
    rough_meta = json.loads(event_by_step["rough3d_submit"]["metadata_json"])
    _assert(rough_meta["gated_by"] == report_asset["file_id"], "rough 3D event was not gated by quality report")


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def _assert_valid_glb(path: Path) -> None:
    payload = path.read_bytes()
    _assert(payload[:4] == b"glTF", "rough_raw_glb did not use GLB magic")
    version, total_length = struct.unpack("<II", payload[4:12])
    _assert(version == 2, "rough_raw_glb was not GLB version 2")
    _assert(total_length == len(payload), "rough_raw_glb length header mismatch")
    json_length, json_type = struct.unpack("<I4s", payload[12:20])
    _assert(json_type == b"JSON", "rough_raw_glb first chunk was not JSON")
    gltf = json.loads(payload[20:20 + json_length].decode("utf-8"))
    _assert(gltf["asset"]["version"] == "2.0", "rough_raw_glb JSON asset version missing")
    _assert(gltf["meshes"][0]["primitives"][0]["attributes"]["POSITION"] == 0, "rough_raw_glb POSITION accessor missing")
    bin_header = 20 + json_length
    bin_length, bin_type = struct.unpack("<I4s", payload[bin_header:bin_header + 8])
    _assert(bin_type == b"BIN\x00", "rough_raw_glb second chunk was not BIN")
    _assert(bin_length > 0, "rough_raw_glb BIN chunk was empty")


if __name__ == "__main__":
    sys.exit(main())
