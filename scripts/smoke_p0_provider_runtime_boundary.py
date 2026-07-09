#!/usr/bin/env python3
"""Smoke test for the generate-3D provider submit/poll/fetch/cancel boundary."""

from __future__ import annotations

import json
import os
import sqlite3
import sys
import tempfile
from pathlib import Path
from typing import Any, Dict

from check_asset_library import validate
from wushen_agent.asset_store import SQLiteAssetStore
from wushen_agent.models import CreateWeaponRequest, Generate3DRequest


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="wushen_p0_provider_boundary_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        os.environ["WUSHEN_LIBRARY_ROOT"] = str(library_root)
        os.environ["WUSHEN_MIGRATIONS_DIR"] = str(ROOT / "migrations")
        os.environ["WUSHEN_GENERATE3D_ASYNC"] = "1"
        os.environ["WUSHEN_MOCK_3D_POLL_SEQUENCE"] = "polling,succeeded"

        store = SQLiteAssetStore.from_env()
        source = _create_source_weapon(store, "p0-provider-source")
        source_version_id = source["source_version_id"]
        source_image_id = source["source_image_id"]
        weapon_id = source["weapon_id"]

        waiting = store.generate_3d(
            weapon_id,
            _generate_body("p0-provider-wait", source_version_id, source_image_id),
            "p0-provider-wait-key",
        )
        first_work = store.run_worker_once("p0_provider_smoke")
        _assert(first_work.claimed is True and first_work.status == "waiting_provider", f"first work did not wait: {first_work}")
        waiting_job = store.get_job(waiting.job_id)
        _assert(waiting_job.status == "waiting_provider", f"job was not waiting_provider: {waiting_job.status}")
        _assert(any(event.step == "rough3d_submit" and event.status == "waiting_provider" for event in waiting_job.events), "waiting event missing")

        db_path = library_root / "library.db"
        with sqlite3.connect(db_path) as conn:
            conn.row_factory = sqlite3.Row
            committed_rough_versions = conn.execute(
                """
                SELECT COUNT(*)
                FROM weapon_versions
                WHERE job_id = ? AND version_type = 'rough_3d' AND status = 'committed'
                """,
                (waiting.job_id,),
            ).fetchone()[0]
            _assert(committed_rough_versions == 0, "waiting job committed a rough_3d version")
            _assert(_count(conn, "models_3d", waiting.job_id) == 0, "waiting job created model rows")
            _assert(_rough_asset_count(conn, waiting.job_id) == 0, "waiting job wrote rough assets before fetch")
            task = _provider_task(conn, waiting.job_id)
            _assert(task["provider_task_id"].startswith("mock_3d_model_"), f"provider task id was not persisted: {task['provider_task_id']}")
            _assert(task["status"] == "polling", f"provider task was not polling: {task['status']}")
            task_meta = json.loads(task["metadata_json"] or "{}")
            _assert(task_meta["phase"] == "polling" and task_meta["last_provider_state"] == "polling", f"poll metadata mismatch: {task_meta}")
            checkpoint = _checkpoint(conn, waiting.job_id)
            checkpoint_state = json.loads(checkpoint["state_json"] or "{}")
            _assert(checkpoint["status"] == "ready" and checkpoint_state["phase"] == "polling", f"checkpoint not resumable polling: {checkpoint_state}")

        second_work = store.run_worker_once("p0_provider_smoke")
        _assert(second_work.claimed is True and second_work.status == "succeeded", f"second work did not succeed: {second_work}")
        completed = store.get_job(waiting.job_id)
        _assert(completed.status == "succeeded", f"completed status mismatch: {completed.status}")
        succeeded_steps = [event.step for event in completed.events if event.status == "succeeded"]
        _assert(
            succeeded_steps == ["rough3d_plan", "rough3d_submit", "model_qc_optimize", "asset_commit_model", "finalize_job"],
            f"unexpected succeeded event order: {succeeded_steps}",
        )
        with sqlite3.connect(db_path) as conn:
            conn.row_factory = sqlite3.Row
            _assert(_count(conn, "models_3d", waiting.job_id) == 1, "completed job did not create one model")
            _assert(_rough_asset_count(conn, waiting.job_id) == 4, "completed job did not write rough GLB/material assets")
            task = _provider_task(conn, waiting.job_id)
            _assert(task["status"] == "succeeded", f"provider task was not succeeded: {task['status']}")
            task_meta = json.loads(task["metadata_json"] or "{}")
            _assert(task_meta["phase"] == "fetched", f"provider task did not record fetched phase: {task_meta}")
            checkpoint = _checkpoint(conn, waiting.job_id)
            checkpoint_state = json.loads(checkpoint["state_json"] or "{}")
            _assert(checkpoint["status"] == "completed" and checkpoint_state["phase"] == "completed", f"checkpoint not completed: {checkpoint_state}")

        cancelled = store.generate_3d(
            weapon_id,
            _generate_body("p0-provider-cancel", source_version_id, source_image_id),
            "p0-provider-cancel-key",
        )
        cancel_first_work = store.run_worker_once("p0_provider_smoke")
        _assert(cancel_first_work.status == "waiting_provider", f"cancel fixture did not reach waiting_provider: {cancel_first_work}")
        cancel = store.cancel_job(cancelled.job_id)
        _assert(cancel.status == "cancelled", f"cancel action did not cancel job: {cancel}")
        no_work = store.run_worker_once("p0_provider_smoke")
        _assert(no_work.claimed is False, f"cancelled provider job should not be claimed again: {no_work}")
        with sqlite3.connect(db_path) as conn:
            conn.row_factory = sqlite3.Row
            task = _provider_task(conn, cancelled.job_id)
            _assert(task["status"] == "cancelled", f"provider task was not cancelled: {task['status']}")
            _assert(_count(conn, "models_3d", cancelled.job_id) == 0, "cancelled job created a model")
            _assert(_rough_asset_count(conn, cancelled.job_id) == 0, "cancelled job wrote rough assets")

        findings, stats = validate(library_root, db_path)
        _assert(stats["blockers"] == 0, f"asset library blockers after provider boundary smoke: {findings}")
        print(
            json.dumps(
                {
                    "ok": True,
                    "completed_job_id": waiting.job_id,
                    "cancelled_job_id": cancelled.job_id,
                    "asset_count": stats["asset_files"],
                },
                indent=2,
                ensure_ascii=False,
            )
        )
        return 0


def _create_source_weapon(store: SQLiteAssetStore, client_request_id: str) -> Dict[str, str]:
    job = store.create_weapon(
        CreateWeaponRequest(
            client_request_id=client_request_id,
            text="赤金国风龙纹长剑，3渲2，逼真外观，仅作为虚构 Unity 游戏资产",
            sketch_asset_id=None,
            reference_asset_ids=[],
            auto_run=True,
            target={"phase": "concept_to_rough_3d", "engine": "unity", "output_format": "glb"},
        ),
        client_request_id,
    )
    source_image_id = next(asset_id for asset_id, role in job.outputs["asset_roles"].items() if role == "concept_image")
    return {
        "weapon_id": job.weapon_id,
        "source_version_id": job.outputs["current_version_id"],
        "source_image_id": source_image_id,
    }


def _generate_body(client_request_id: str, source_version_id: str, source_image_id: str) -> Generate3DRequest:
    return Generate3DRequest(
        client_request_id=client_request_id,
        source_version_id=source_version_id,
        source_image_asset_id=source_image_id,
        provider_id="mock_3d",
        target_format="glb",
        style="stylized_toon_weapon",
        orientation_policy={"forward_axis": "+Z", "long_axis": "+Y", "pivot": "grip_center"},
        scale_policy="normalized_game_asset_scale",
        build_unity_export=True,
    )


def _provider_task(conn: sqlite3.Connection, job_id: str) -> sqlite3.Row:
    row = conn.execute(
        """
        SELECT provider_task_id, status, metadata_json
        FROM provider_tasks
        WHERE job_id = ? AND step_name = 'rough3d_submit'
        ORDER BY updated_at DESC
        LIMIT 1
        """,
        (job_id,),
    ).fetchone()
    _assert(row is not None, "provider task missing")
    return row


def _checkpoint(conn: sqlite3.Connection, job_id: str) -> sqlite3.Row:
    row = conn.execute(
        """
        SELECT status, resume_policy, state_json
        FROM job_checkpoints
        WHERE job_id = ? AND step_name = 'rough3d_submit'
        ORDER BY updated_at DESC
        LIMIT 1
        """,
        (job_id,),
    ).fetchone()
    _assert(row is not None, "checkpoint missing")
    return row


def _count(conn: sqlite3.Connection, table: str, job_id: str) -> int:
    return int(conn.execute(f"SELECT COUNT(*) FROM {table} WHERE job_id = ?", (job_id,)).fetchone()[0])


def _rough_asset_count(conn: sqlite3.Connection, job_id: str) -> int:
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


def _assert(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


if __name__ == "__main__":
    sys.exit(main())
