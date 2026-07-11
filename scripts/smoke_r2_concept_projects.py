#!/usr/bin/env python3
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
from typing import Any, Dict, Optional


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad_r2_projects_") as temporary_directory:
        library_root = Path(temporary_directory) / "ForgeCADLibrary"
        port = _free_port()
        base_url = f"http://127.0.0.1:{port}"
        process = _start_agent(library_root, port)
        try:
            _wait_for_health(base_url, process)
            empty = _json_request(base_url, "/api/v1/projects", method="GET")
            _assert(empty["items"] == [], "fresh project list was not empty")

            create_body = _create_body()
            created = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=create_body,
                idempotency_key="r2-project-create-1",
            )
            project_id = created["project_id"]
            version_1 = created["current_version_id"]
            _assert(project_id.startswith("prj_"), "project id prefix mismatch")
            _assert(version_1.startswith("ver_"), "initial version id prefix mismatch")
            _assert(created["current_spec"]["project_id"] == project_id, "spec project trace mismatch")
            _assert(created["profile"]["non_functional_only"] is True, "profile boundary mismatch")
            _assert(len(created["versions"]) == 1, "initial project did not contain one version")

            replay = _json_request(
                base_url,
                "/api/v1/projects",
                method="POST",
                body=create_body,
                idempotency_key="r2-project-create-1",
            )
            _assert(replay["project_id"] == project_id, "create idempotency replay mismatch")
            conflict_status, conflict = _json_request_allow_error(
                base_url,
                "/api/v1/projects",
                method="POST",
                body={**create_body, "name": "不同的项目"},
                idempotency_key="r2-project-create-1",
            )
            _assert(
                conflict_status == 409 and conflict["error"]["code"] == "IDEMPOTENCY_CONFLICT",
                "create idempotency conflict was not rejected",
            )

            listed = _json_request(base_url, "/api/v1/projects", method="GET")
            _assert(len(listed["items"]) == 1, "project list count mismatch")
            detail = _json_request(base_url, f"/api/v1/projects/{project_id}", method="GET")
            _assert(detail["current_version_id"] == version_1, "project detail version mismatch")

            next_spec = json.loads(json.dumps(detail["current_spec"]))
            next_spec["proportions"]["overall_length_mm"] = 242
            next_spec["style"]["detail_density"] = 0.74
            append_body = {
                "client_request_id": "r2-project-version-2",
                "parent_version_id": version_1,
                "summary": "调整整体比例与细节密度。",
                "spec": next_spec,
            }
            appended = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body=append_body,
                idempotency_key="r2-project-version-2",
            )
            version_2 = appended["current_version_id"]
            _assert(version_2 != version_1, "append did not create a child version")
            _assert(len(appended["versions"]) == 2, "append did not preserve both versions")
            _assert(appended["versions"][1]["parent_version_id"] == version_1, "version parent mismatch")
            _assert(
                appended["current_spec"]["proportions"]["overall_length_mm"] == 242,
                "current spec was not updated",
            )
            version_detail = _json_request(
                base_url,
                f"/api/v1/versions/{version_2}",
                method="GET",
            )
            _assert(version_detail["project_id"] == project_id, "version project trace mismatch")
            _assert(
                version_detail["spec"]["proportions"]["overall_length_mm"] == 242,
                "version detail did not return its immutable spec",
            )
            append_replay = _json_request(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body=append_body,
                idempotency_key="r2-project-version-2",
            )
            _assert(append_replay["current_version_id"] == version_2, "append replay mismatch")

            missing_parent_status, missing_parent = _json_request_allow_error(
                base_url,
                f"/api/v1/projects/{project_id}/versions",
                method="POST",
                body={**append_body, "parent_version_id": "ver_missing"},
                idempotency_key="r2-project-version-missing-parent",
            )
            _assert(
                missing_parent_status == 404 and missing_parent["error"]["code"] == "VERSION_NOT_FOUND",
                "missing parent version was not rejected",
            )
        finally:
            _stop_agent(process)

        database_path = library_root / "library.db"
        _assert_database_state(database_path, project_id, version_1, version_2)

        restart_port = _free_port()
        restart_url = f"http://127.0.0.1:{restart_port}"
        restarted_process = _start_agent(library_root, restart_port)
        try:
            _wait_for_health(restart_url, restarted_process)
            restored = _json_request(
                restart_url,
                f"/api/v1/projects/{project_id}",
                method="GET",
            )
            _assert(restored["current_version_id"] == version_2, "restart did not restore current version")
            _assert(len(restored["versions"]) == 2, "restart did not restore version history")
        finally:
            _stop_agent(restarted_process)

        print(
            json.dumps(
                {
                    "ok": True,
                    "project_id": project_id,
                    "version_1": version_1,
                    "version_2": version_2,
                    "migration_count": _table_count(database_path, "schema_migrations"),
                    "legacy_graph_dependencies": 0,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    return 0


def _create_body() -> Dict[str, Any]:
    return {
        "client_request_id": "r2-project-create-1",
        "profile_id": "profile_weapon_concept_v1",
        "name": "寒地巡逻 S1",
        "intended_uses": ["game_asset", "film_prop", "non_functional_display"],
        "style": {
            "keywords": ["寒地", "工业", "紧凑", "硬表面"],
            "palette": ["graphite", "gunmetal", "signal_red"],
            "detail_density": 0.68,
        },
        "proportions": {
            "overall_length_mm": 230,
            "body_height_mm": 54,
            "grip_angle_deg": 15,
        },
        "required_slots": ["core", "front", "rear", "grip"],
        "optional_slots": ["top", "left", "right", "bottom", "side_panels"],
        "constraints": {
            "symmetry": "mostly_symmetric",
            "max_triangle_count": 180000,
        },
        "assumptions": ["非功能性概念模型，不用于真实制造或使用"],
    }


def _assert_database_state(
    database_path: Path,
    project_id: str,
    version_1: str,
    version_2: str,
) -> None:
    _assert(_table_count(database_path, "domain_profiles") == 1, "domain profile count mismatch")
    _assert(_table_count(database_path, "projects") == 1, "project count mismatch")
    _assert(_table_count(database_path, "project_versions") == 2, "version count mismatch")
    with sqlite3.connect(database_path) as connection:
        first_spec = connection.execute(
            "SELECT spec_json FROM project_versions WHERE version_id = ?",
            (version_1,),
        ).fetchone()[0]
        second_spec = connection.execute(
            "SELECT spec_json FROM project_versions WHERE version_id = ?",
            (version_2,),
        ).fetchone()[0]
        _assert(first_spec != second_spec, "child version overwrote or duplicated the parent spec")
        current = connection.execute(
            "SELECT current_version_id FROM projects WHERE project_id = ?",
            (project_id,),
        ).fetchone()[0]
        _assert(current == version_2, "database current_version_id mismatch")
        forbidden_targets = {"creative_weapon_graphs", "skill_graphs", "weapons", "weapon_versions"}
        for table in ("projects", "project_versions", "module_assets", "module_graphs"):
            foreign_targets = {
                row[2]
                for row in connection.execute(f"PRAGMA foreign_key_list({table})").fetchall()
            }
            _assert(
                not (foreign_targets & forbidden_targets),
                f"{table} references a forbidden legacy domain table",
            )


def _table_count(database_path: Path, table: str) -> int:
    with sqlite3.connect(database_path) as connection:
        return int(connection.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0])


def _start_agent(library_root: Path, port: int) -> subprocess.Popen:
    environment = os.environ.copy()
    environment["WUSHEN_LIBRARY_ROOT"] = str(library_root)
    environment["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
    environment["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
    environment.setdefault("FORGECAD_CONCEPT_WORKER_ENABLED", "0")
    environment["FORGECAD_CONCEPT_PLANNER_PROVIDER"] = "deterministic_rules"
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
        env=environment,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )


def _stop_agent(process: subprocess.Popen) -> None:
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as socket_handle:
        socket_handle.bind(("127.0.0.1", 0))
        return int(socket_handle.getsockname()[1])


def _wait_for_health(base_url: str, process: subprocess.Popen) -> None:
    deadline = time.time() + 15
    while time.time() < deadline:
        if process.poll() is not None:
            output = process.stdout.read() if process.stdout else ""
            raise RuntimeError(f"Agent exited before health check:\n{output}")
        try:
            response = _json_request(base_url, "/api/health", method="GET")
            if response.get("status") == "ok":
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
    status, response = _json_request_allow_error(
        base_url,
        path,
        method=method,
        body=body,
        idempotency_key=idempotency_key,
    )
    _assert(200 <= status < 300, f"{method} {path} failed with {status}: {response}")
    return response


def _json_request_allow_error(
    base_url: str,
    path: str,
    *,
    method: str,
    body: Optional[Dict[str, Any]] = None,
    idempotency_key: Optional[str] = None,
) -> tuple[int, Dict[str, Any]]:
    payload = None if body is None else json.dumps(body, ensure_ascii=False).encode("utf-8")
    request = urllib.request.Request(f"{base_url}{path}", data=payload, method=method)
    request.add_header("Content-Type", "application/json")
    if idempotency_key:
        request.add_header("Idempotency-Key", idempotency_key)
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        return exc.code, json.loads(exc.read().decode("utf-8"))


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    raise SystemExit(main())
