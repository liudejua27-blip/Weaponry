#!/usr/bin/env python3
"""Manual smoke for a real local ComfyUI server.

This script is intentionally not part of m3:gate. It requires a running ComfyUI
server and a workflow template that matches the user's installed checkpoints.
"""

from __future__ import annotations

import json
import os
import sqlite3
import sys
import tempfile
import urllib.request
from pathlib import Path

from check_asset_library import validate


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps" / "agent"))

from wushen_agent.asset_store import SQLiteAssetStore  # noqa: E402
from wushen_agent.models import CreateWeaponRequest  # noqa: E402


def main() -> int:
    base_url = os.environ.get("WUSHEN_COMFYUI_BASE_URL", "http://127.0.0.1:8188").rstrip("/")
    _check_comfyui_health(base_url)

    with tempfile.TemporaryDirectory(prefix="wushen_m3_real_comfyui_") as tmp:
        library_root = Path(tmp) / "WushenForgeLibrary"
        previous = _set_env(
            {
                "WUSHEN_LIBRARY_ROOT": str(library_root),
                "WUSHEN_MIGRATIONS_DIR": str(ROOT / "migrations"),
                "WUSHEN_IMAGE_PROVIDER": "comfyui",
                "WUSHEN_COMFYUI_BASE_URL": base_url,
            }
        )
        try:
            store = SQLiteAssetStore.from_env()
            request = CreateWeaponRequest(
                client_request_id="m3-real-comfyui-smoke",
                text=os.environ.get(
                    "WUSHEN_COMFYUI_SMOKE_PROMPT",
                    "赤金龙纹长剑，3渲2国风神兵，逼真外观，仅作为虚构 Unity 游戏资产",
                ),
            )
            job = store.create_weapon(request, idempotency_key="m3-real-comfyui-smoke-key")
            db_path = library_root / "library.db"
            findings, stats = validate(library_root, db_path)
            if stats["blockers"]:
                raise AssertionError(f"asset library blockers: {findings}")
            print(
                json.dumps(
                    {
                        "ok": True,
                        "base_url": base_url,
                        "library_root": str(library_root),
                        "db": str(db_path),
                        "weapon_id": job.weapon_id,
                        "job_id": job.job_id,
                        "asset_count": stats["asset_files"],
                        "concept": _concept_summary(db_path, job.job_id),
                    },
                    ensure_ascii=False,
                    indent=2,
                )
            )
        finally:
            _restore_env(previous)
    return 0


def _check_comfyui_health(base_url: str) -> None:
    try:
        with urllib.request.urlopen(f"{base_url}/system_stats", timeout=10) as response:
            data = json.loads(response.read().decode("utf-8"))
    except Exception as exc:  # noqa: BLE001 - manual smoke should print clear preflight failure.
        raise RuntimeError(f"ComfyUI is not reachable at {base_url}. Start ComfyUI or set WUSHEN_COMFYUI_BASE_URL.") from exc
    if not isinstance(data, dict):
        raise RuntimeError(f"ComfyUI /system_stats returned unexpected payload: {data!r}")


def _concept_summary(db_path: Path, job_id: str) -> dict[str, str]:
    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    row = conn.execute(
        """
        SELECT file_id, logical_path, sha256, mime_type
        FROM asset_files
        WHERE job_id = ? AND role = 'concept_image'
        LIMIT 1
        """,
        (job_id,),
    ).fetchone()
    if row is None:
        raise AssertionError("manual ComfyUI smoke did not produce concept_image asset")
    return {
        "file_id": row["file_id"],
        "logical_path": row["logical_path"],
        "sha256": row["sha256"],
        "mime_type": row["mime_type"],
    }


def _set_env(values: dict[str, str]) -> dict[str, str | None]:
    previous = {key: os.environ.get(key) for key in values}
    os.environ.update(values)
    return previous


def _restore_env(previous: dict[str, str | None]) -> None:
    for key, value in previous.items():
        if value is None:
            os.environ.pop(key, None)
        else:
            os.environ[key] = value


if __name__ == "__main__":
    sys.exit(main())
