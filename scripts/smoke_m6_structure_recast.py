#!/usr/bin/env python3
"""M6 smoke for structure interpretation and Creative Recast confirmation."""

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
sys.path.insert(0, str(ROOT / "apps" / "agent"))


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_m6_recast_") as tmp:
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
            create_body = {
                "client_request_id": "m6-create-source",
                "text": "防弹裤神炮，可以是裤子、护甲、炮台、位移机关，3渲2国风神兵，仅作为虚构 Unity 游戏资产",
                "sketch_asset_id": None,
                "reference_asset_ids": [],
                "auto_run": True,
                "target": {"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
            }
            created = _json_request(base_url, "/api/weapons", method="POST", body=create_body, idempotency_key="m6-create-key")
            weapon_id = created["weapon_id"]
            interp_body = {
                "client_request_id": "m6-interpret",
                "source_object": "防弹裤",
                "raw_description": create_body["text"],
                "desired_style": "3渲2国风神兵，高拟真虚构 Unity 资产",
                "freedom_level": "strange",
                "mythology_level": "guofeng_divine",
                "gameplay_complexity": "multi_stage",
                "asset_priority": "lowpoly_first",
            }
            interpretation = _json_request(
                base_url,
                f"/api/weapons/{weapon_id}/interpretation",
                method="POST",
                body=interp_body,
                idempotency_key="m6-interpret-key",
            )
            replay = _json_request(
                base_url,
                f"/api/weapons/{weapon_id}/interpretation",
                method="POST",
                body=interp_body,
                idempotency_key="m6-interpret-key",
            )
            _assert(replay["interpretation_id"] == interpretation["interpretation_id"], "interpretation idempotency replay mismatch")
            _assert(interpretation["status"] == "ready", "interpretation was not ready")
            _assert(interpretation["candidate_count"] in {2, 3}, "candidate count was not 2~3")
            candidates = interpretation["candidates"]
            _assert(len(candidates) == interpretation["candidate_count"], "candidate_count did not match candidates length")
            axes = {tuple(candidate["combat_affordances"]) for candidate in candidates}
            _assert(len(axes) >= 2, "candidates did not produce distinct affordance directions")
            for candidate in candidates:
                for key in ["anchor_points", "protected_regions", "skill_anchor_points", "risk_tags", "structure_graph"]:
                    _assert(candidate.get(key), f"candidate missing {key}")

            bad_status, bad_payload = _json_request_allow_error(
                base_url,
                f"/api/weapons/{weapon_id}/recast/confirm",
                method="POST",
                body={
                    "client_request_id": "m6-bad-confirm",
                    "interpretation_id": interpretation["interpretation_id"],
                    "selected_candidate_id": "cand_missing",
                    "selected_candidate_rank": 1,
                    "recast_mode": "stylized_artifact",
                },
                idempotency_key="m6-bad-confirm-key",
            )
            _assert(bad_status == 400 and bad_payload["error"]["code"] == "INVALID_INTERPRETATION_CANDIDATE", "invalid candidate was not rejected")

            selected = candidates[1]
            confirm_body = {
                "client_request_id": "m6-confirm",
                "interpretation_id": interpretation["interpretation_id"],
                "selected_candidate_id": selected["candidate_id"],
                "selected_candidate_rank": selected["rank"],
                "recast_mode": "stylized_artifact",
                "recast_choice_text": selected["recast_summary"],
            }
            confirmed = _json_request(
                base_url,
                f"/api/weapons/{weapon_id}/recast/confirm",
                method="POST",
                body=confirm_body,
                idempotency_key="m6-confirm-key",
            )
            _assert(confirmed["status"] == "confirmed", "confirm status mismatch")
            _assert(confirmed["creative_graph_id"].startswith("cg_"), "creative graph id prefix mismatch")
            _assert(confirmed["skill_graph_id"].startswith("sg_"), "skill graph id prefix mismatch")
            _assert(len(confirmed["skill_graph"]["skills"]) == 6, "SkillGraph did not contain 6 skills")
            _assert(confirmed["creative_graph"]["selected_candidate_id"] == selected["candidate_id"], "creative graph selected candidate mismatch")

            graph = _json_request(base_url, f"/api/weapons/{weapon_id}/creative-graph", method="GET")
            _assert(graph["creative_graph_id"] == confirmed["creative_graph_id"], "creative graph lookup mismatch")
            _assert(graph["skill_graph_id"] == confirmed["skill_graph_id"], "skill graph lookup mismatch")
            _assert(_db_count(library_root / "library.db", "structure_interpretations") == 1, "structure_interpretations row missing")
            _assert(_db_count(library_root / "library.db", "creative_weapon_graphs") == 1, "creative graph row missing")
            _assert(_db_count(library_root / "library.db", "skill_graphs") == 1, "skill graph row missing")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "weapon_id": weapon_id,
                        "interpretation_id": interpretation["interpretation_id"],
                        "candidate_count": interpretation["candidate_count"],
                        "creative_graph_id": confirmed["creative_graph_id"],
                        "skill_graph_id": confirmed["skill_graph_id"],
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


def _db_count(db_path: Path, table: str) -> int:
    with sqlite3.connect(db_path) as conn:
        return int(conn.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0])


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
    status, data = _json_request_allow_error(base_url, path, method=method, body=body, idempotency_key=idempotency_key)
    _assert(200 <= status < 300, f"{method} {path} failed with {status}: {data}")
    return data


def _json_request_allow_error(
    base_url: str,
    path: str,
    *,
    method: str,
    body: Optional[Dict[str, Any]] = None,
    idempotency_key: Optional[str] = None,
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


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
