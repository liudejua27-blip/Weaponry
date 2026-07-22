from __future__ import annotations

import asyncio
import os
from collections.abc import Mapping
from typing import Any

from fastapi import FastAPI
from fastapi.responses import JSONResponse

from forgecad_agent.api.factory import LocalApiSettings, create_local_api
from forgecad_agent.api.restricted_geometry_routes import (
    build_restricted_geometry_router,
)
from forgecad_agent.application.restricted_geometry_executor import (
    RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV,
    RestrictedGeometryExecutor,
)


def create_app(
    *,
    environment: Mapping[str, str] | None = None,
) -> FastAPI:
    """Create the production Python facet.

    Environment variables can configure only the restricted geometry
    capability.  In particular, no packaged or production entrypoint can
    select the historical product-state writer through an environment switch.
    Tests that still need the migration oracle must call
    :func:`create_test_only_legacy_product_core_app` directly.
    """

    values = dict(os.environ if environment is None else environment)
    # Do not forward a legacy library root, migrations directory, Provider
    # configuration, or arbitrary host environment into the Python executor.
    # The only location is the audited read-only bundle root; the only secret
    # is the one-process Rust capability.
    restricted_environment = {
        name: values[name]
        for name in (
            RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV,
            "FORGECAD_RUNTIME_RESOURCE_ROOT",
        )
        if values.get(name)
    }
    return create_restricted_geometry_app(environment=restricted_environment)


def create_test_only_legacy_product_core_app(
    *,
    environment: Mapping[str, str] | None = None,
) -> FastAPI:
    """Create the historical full app for an explicit test-process oracle.

    This function is deliberately absent from the production/frozen command
    line.  Its explicit name and direct call are the capability boundary; no
    environment variable can redirect :func:`create_app` to it.
    """

    values = dict(os.environ if environment is None else environment)
    return _create_test_only_legacy_product_core(values)


def create_restricted_geometry_app(
    *,
    environment: Mapping[str, str] | None = None,
) -> FastAPI:
    """Create the production Python facet without any persistence product core."""

    values = dict(os.environ if environment is None else environment)
    app = create_local_api(
        LocalApiSettings.from_env(
            title="ForgeCAD Restricted Geometry Executor",
            version="1.0.0",
            environ=values,
        )
    )
    executor = RestrictedGeometryExecutor(environment=values)
    app.include_router(build_restricted_geometry_router(executor, environment=values))
    app.state.restricted_geometry_executor = executor
    app.state.product_state_owner = "rust_forgecad_core"
    app.state.persistent_state_writer = False

    @app.get("/api/health")
    async def restricted_geometry_health() -> dict[str, Any]:
        return {
            "status": "ok",
            "service": "forgecad-restricted-geometry-executor",
            "mode": "restricted_geometry_executor",
            "schema_version": "RestrictedGeometryExecutorHealth@1",
            "python_role": "restricted_geometry_executor",
            "database_access": False,
            "object_store_access": False,
            "provider_access": False,
            "snapshot_write": False,
            "persistent_state_writer": False,
        }

    # Exact internal geometry routes were registered above.  Everything else
    # under the former Python product namespace is a stable tombstone rather
    # than a hidden legacy handler or accidental 404 fallback.
    @app.api_route(
        "/api/v1/{legacy_path:path}",
        methods=["GET", "POST", "PUT", "PATCH", "DELETE"],
        include_in_schema=False,
    )
    async def rust_owned_product_tombstone(legacy_path: str) -> JSONResponse:
        del legacy_path
        return JSONResponse(
            status_code=410,
            content={
                "error": {
                    "code": "PRODUCT_STATE_RUST_OWNED",
                    "message": (
                        "Project, Agent lifecycle and product state are owned by "
                        "the Rust ForgeCAD core; this Python process only executes "
                        "restricted geometry."
                    ),
                    "recoverable": False,
                    "details": {},
                }
            },
        )

    return app


def _create_test_only_legacy_product_core(
    environment: Mapping[str, str],
) -> FastAPI:
    """Create the historical full app only for explicit test-oracle processes."""

    # The restricted-geometry capability belongs exclusively to the Rust ->
    # geometry channel.  Even the explicit legacy browser oracle must not pass
    # it into the K002 lifecycle/product-tool ports, whose environment policy
    # intentionally rejects unrelated secrets.
    legacy_environment = dict(environment)
    legacy_environment.pop(RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV, None)

    # Imports that can construct or reach SQLite, the object store, legacy
    # providers, lifecycle persistence and product tools stay entirely behind
    # the explicit direct test factory. The default module path above never
    # imports or constructs SQLiteAssetStore.
    from forgecad_agent.api import (
        build_active_design_router,
        build_agent_asset_router,
        build_agent_router,
        build_audit_export_router,
        build_brief_router,
        build_change_set_router,
        build_concept_job_router,
        build_concept_project_router,
        build_export_router,
        build_module_router,
        build_quality_router,
    )
    from forgecad_agent.api.app_server_compat_routes import (
        build_app_server_compat_router,
    )
    from forgecad_agent.api.k002_internal_routes import build_k002_internal_router
    from forgecad_agent.application import (
        ActiveDesignService,
        AgentAssetEditingService,
        AgentKernelService,
        ConceptBriefService,
        ConceptChangeSetAuditService,
        ConceptChangeSetService,
        ConceptExportService,
        ConceptJobService,
        ConceptModuleService,
        ConceptProjectService,
        ConceptQualityService,
        ConceptWorkbenchBootstrapService,
        MaterialTextureService,
        concept_planner_from_env,
    )
    from forgecad_agent.application.app_server_compat import (
        AppServerCompatibilityAdapter,
    )
    from forgecad_agent.application.k002_python_ports import (
        LifecyclePersistencePort,
        ProductToolExecutorPort,
    )
    from forgecad_agent.application.k002_sqlite_lifecycle import (
        K002SQLiteLifecycleBackend,
    )

    from .api.asset_routes import build_asset_router
    from .api.errors import register_error_handlers
    from .api.job_routes import build_job_router
    from .api.system_routes import build_system_router
    from .api.weapon_routes import build_weapon_router
    from .asset_store import SQLiteAssetStore
    from .worker import (
        local_worker_enabled,
        run_local_worker_loop,
        stop_worker_task,
    )

    app = create_local_api(
        LocalApiSettings.from_env(
            title="Wushen Forge Agent Test Oracle",
            version="0.1.0",
            environ=legacy_environment,
        )
    )
    store = SQLiteAssetStore.from_env()
    concept_planner = concept_planner_from_env()
    concept_modules = ConceptModuleService(
        store.connection_factory,
        store.object_store,
    )
    concept_projects = ConceptProjectService(store.connection_factory)
    concept_bootstrap = ConceptWorkbenchBootstrapService(
        concept_projects,
        concept_modules,
    )
    concept_change_sets = ConceptChangeSetService(
        store.connection_factory,
        concept_planner,
    )
    concept_quality = ConceptQualityService(
        store.connection_factory,
        store.object_store,
    )
    concept_briefs = ConceptBriefService(
        store.connection_factory,
        concept_planner,
    )
    concept_jobs = ConceptJobService(store.connection_factory)
    concept_exports = ConceptExportService(
        store.connection_factory,
        store.object_store,
    )
    material_textures = MaterialTextureService(
        store.connection_factory,
        store.object_store,
    )
    change_set_audits = ConceptChangeSetAuditService(
        store.connection_factory,
        store.object_store,
    )
    register_error_handlers(app)
    app.include_router(build_asset_router(store))
    app.include_router(build_job_router(store))
    app.include_router(build_system_router(store))
    app.include_router(build_weapon_router(store))
    app.include_router(build_concept_project_router(concept_projects, concept_bootstrap))
    app.include_router(build_module_router(concept_modules))
    app.include_router(build_change_set_router(concept_change_sets))
    app.include_router(build_quality_router(concept_quality, concept_jobs))
    app.include_router(build_brief_router(concept_briefs))
    app.include_router(build_concept_job_router(concept_jobs, concept_quality))
    app.include_router(build_export_router(concept_exports))
    app.include_router(build_audit_export_router(change_set_audits))
    app.include_router(
        build_agent_router(
            AgentKernelService(store.connection_factory),
            material_textures,
            allow_test_only_legacy_lifecycle=True,
        )
    )
    app.include_router(
        build_agent_asset_router(
            AgentAssetEditingService(
                store.connection_factory,
                store.object_store,
            )
        )
    )
    app.include_router(build_active_design_router(ActiveDesignService(store.connection_factory)))
    app_server_compat = AppServerCompatibilityAdapter(app)
    app.include_router(build_app_server_compat_router(app_server_compat))
    app.state.app_server_compat = app_server_compat
    k002_lifecycle = LifecyclePersistencePort(
        K002SQLiteLifecycleBackend(store.connection_factory),
        environment=legacy_environment,
    )
    k002_product_tools = ProductToolExecutorPort(environment=legacy_environment)
    app.include_router(
        build_k002_internal_router(
            k002_lifecycle,
            k002_product_tools,
            environment=legacy_environment,
        )
    )
    app.state.k002_lifecycle = k002_lifecycle
    app.state.k002_product_tools = k002_product_tools
    app.state.test_only_legacy_product_core = True

    @app.on_event("startup")
    async def recover_interrupted_jobs_on_startup() -> None:
        worker_enabled = local_worker_enabled()
        if os.environ.get("WUSHEN_RECOVER_ON_STARTUP", "1").strip() != "0":
            store.recover_interrupted_jobs(
                reason="startup",
                include_queued=not worker_enabled,
            )
        if os.environ.get("FORGECAD_CONCEPT_RECOVER_ON_STARTUP", "1").strip() != "0":
            concept_jobs.recover_interrupted_work(force=True)
        if worker_enabled:
            app.state.local_worker_task = asyncio.create_task(
                run_local_worker_loop(
                    store,
                    runner_id=os.environ.get(
                        "WUSHEN_LOCAL_WORKER_ID",
                        os.environ.get(
                            "WUSHEN_GENERATE3D_WORKER_ID",
                            "local_asset_worker",
                        ),
                    ),
                )
            )
        if os.environ.get("FORGECAD_CONCEPT_WORKER_ENABLED", "1").strip() != "0":

            async def run_concept_worker_loop() -> None:
                while True:
                    completed = await asyncio.to_thread(
                        concept_jobs.run_next_quality_inspection,
                        concept_quality,
                        runner_id=os.environ.get(
                            "FORGECAD_CONCEPT_WORKER_ID",
                            "local_concept_worker",
                        ),
                    )
                    await asyncio.sleep(0.05 if completed is not None else 0.25)

            app.state.concept_worker_task = asyncio.create_task(run_concept_worker_loop())

    @app.on_event("shutdown")
    async def stop_local_worker_on_shutdown() -> None:
        await app_server_compat.close_all()
        task = getattr(app.state, "local_worker_task", None)
        if task is not None:
            await stop_worker_task(task)
        concept_task = getattr(app.state, "concept_worker_task", None)
        if concept_task is not None:
            await stop_worker_task(concept_task)

    return app


app = create_app()
