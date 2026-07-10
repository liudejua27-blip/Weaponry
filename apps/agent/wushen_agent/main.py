from __future__ import annotations

import asyncio
import os

from fastapi import FastAPI
from forgecad_agent.api import (
    LocalApiSettings,
    build_concept_project_router,
    create_local_api,
)
from forgecad_agent.application import ConceptProjectService

from .api.asset_routes import build_asset_router
from .api.errors import register_error_handlers
from .api.job_routes import build_job_router
from .api.system_routes import build_system_router
from .api.weapon_routes import build_weapon_router
from .asset_store import SQLiteAssetStore
from .worker import local_worker_enabled, run_local_worker_loop, stop_worker_task


def create_app() -> FastAPI:
    app = create_local_api(
        LocalApiSettings.from_env(title="Wushen Forge Agent", version="0.1.0")
    )
    store = SQLiteAssetStore.from_env()
    concept_projects = ConceptProjectService(store.connection_factory)
    register_error_handlers(app)
    app.include_router(build_asset_router(store))
    app.include_router(build_job_router(store))
    app.include_router(build_system_router(store))
    app.include_router(build_weapon_router(store))
    app.include_router(build_concept_project_router(concept_projects))

    @app.on_event("startup")
    async def recover_interrupted_jobs_on_startup() -> None:
        worker_enabled = local_worker_enabled()
        if os.environ.get("WUSHEN_RECOVER_ON_STARTUP", "1").strip() != "0":
            store.recover_interrupted_jobs(reason="startup", include_queued=not worker_enabled)
        if worker_enabled:
            app.state.local_worker_task = asyncio.create_task(
                run_local_worker_loop(
                    store,
                    runner_id=os.environ.get(
                        "WUSHEN_LOCAL_WORKER_ID",
                        os.environ.get("WUSHEN_GENERATE3D_WORKER_ID", "local_asset_worker"),
                    ),
                )
            )

    @app.on_event("shutdown")
    async def stop_local_worker_on_shutdown() -> None:
        task = getattr(app.state, "local_worker_task", None)
        if task is not None:
            await stop_worker_task(task)

    return app


app = create_app()
