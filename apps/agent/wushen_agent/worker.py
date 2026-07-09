from __future__ import annotations

import asyncio
import contextlib
import logging
import os
from typing import Optional

from .asset_store import SQLiteAssetStore

LOGGER = logging.getLogger(__name__)


def generate3d_worker_enabled() -> bool:
    return os.environ.get("WUSHEN_GENERATE3D_WORKER", "0").strip() == "1"


def export_unity_worker_enabled() -> bool:
    return os.environ.get("WUSHEN_EXPORT_UNITY_WORKER", "0").strip() == "1"


def local_worker_enabled() -> bool:
    return os.environ.get("WUSHEN_RUNTIME_WORKER", "0").strip() == "1" or generate3d_worker_enabled() or export_unity_worker_enabled()


def local_worker_interval_seconds() -> float:
    raw = os.environ.get("WUSHEN_LOCAL_WORKER_INTERVAL_SECONDS", os.environ.get("WUSHEN_GENERATE3D_WORKER_INTERVAL_SECONDS", "0.25")).strip()
    try:
        return max(0.05, float(raw))
    except ValueError:
        return 0.25


async def run_local_worker_loop(
    store: SQLiteAssetStore,
    *,
    runner_id: str = "local_asset_worker",
    interval_seconds: Optional[float] = None,
) -> None:
    interval = local_worker_interval_seconds() if interval_seconds is None else interval_seconds
    while True:
        try:
            response = await asyncio.to_thread(store.run_worker_once, runner_id)
        except Exception:
            LOGGER.exception("local worker loop iteration failed")
            await asyncio.sleep(interval)
            continue
        if not response.claimed:
            await asyncio.sleep(interval)


async def run_generate3d_worker_loop(
    store: SQLiteAssetStore,
    *,
    runner_id: str = "local_generate3d_worker",
    interval_seconds: Optional[float] = None,
) -> None:
    await run_local_worker_loop(store, runner_id=runner_id, interval_seconds=interval_seconds)


async def stop_worker_task(task: asyncio.Task[None]) -> None:
    task.cancel()
    with contextlib.suppress(asyncio.CancelledError):
        await task
