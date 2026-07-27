#!/usr/bin/env python3
"""Verify the active Python runtime cannot enter the historical product API.

This is a route/import smoke, not a product-success fixture.  The explicit
legacy factory remains available to migration and test-oracle callers; the
default app must expose only the restricted geometry facet and Rust-owned
tombstones for the former product namespace.
"""

from __future__ import annotations

import asyncio
import json
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "apps/agent"))

from forgecad_agent.application.app_server_compat import InProcessAsgiClient  # noqa: E402
from wushen_agent.main import create_app  # noqa: E402


class BoundaryFailure(RuntimeError):
    pass


def require(condition: bool, code: str) -> None:
    if not condition:
        raise BoundaryFailure(code)


async def run() -> dict[str, object]:
    app = create_app(
        environment={
            "FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE": "1",
            "FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE": "1",
            "WUSHEN_LIBRARY_ROOT": "/must-not-reach-the-default-python-app",
            "FORGECAD_AGENT_API_KEY": "must-not-reach-the-default-python-app",
        }
    )
    route_paths = {getattr(route, "path", "") for route in app.routes}
    for legacy_path in (
        "/api/v1/projects",
        "/api/v1/module-graphs",
        "/api/v1/concept-versions",
        "/api/v1/agent/threads",
    ):
        require(legacy_path not in route_paths, f"ACTIVE_LEGACY_ROUTE_REGISTERED:{legacy_path}")
    require(
        "/api/v1/{legacy_path:path}" in route_paths,
        "RUST_OWNED_PRODUCT_TOMBSTONE_MISSING",
    )
    require(app.state.product_state_owner == "rust_forgecad_core", "PYTHON_STATE_OWNER_INVALID")
    require(app.state.persistent_state_writer is False, "PYTHON_PERSISTENCE_WRITER_ENABLED")

    client = InProcessAsgiClient(app)
    checks: dict[str, int] = {}
    for method, path in (
        ("POST", "/api/v1/projects"),
        ("POST", "/api/v1/agent/threads"),
        ("GET", "/api/v1/module-graphs/legacy"),
        ("POST", "/api/v1/internal/k002/lifecycle/execute"),
    ):
        response = await client.request(method, path, body=b"{}")
        checks[f"{method} {path}"] = response.status
        require(response.status == 410, f"LEGACY_ROUTE_NOT_TOMBSTONED:{method}:{path}")
        payload = json.loads(response.body.decode("utf-8"))
        require(
            payload.get("error", {}).get("code") == "PRODUCT_STATE_RUST_OWNED",
            f"LEGACY_TOMBSTONE_CODE_INVALID:{method}:{path}",
        )

    health = await client.request("GET", "/api/health")
    health_payload = json.loads(health.body.decode("utf-8"))
    require(health.status == 200, "RESTRICTED_HEALTH_FAILED")
    require(health_payload.get("mode") == "restricted_geometry_executor", "PYTHON_MODE_INVALID")
    require(health_payload.get("database_access") is False, "PYTHON_DATABASE_ACCESS_ENABLED")
    require(health_payload.get("provider_access") is False, "PYTHON_PROVIDER_ACCESS_ENABLED")
    require(health_payload.get("snapshot_write") is False, "PYTHON_SNAPSHOT_WRITE_ENABLED")
    loaded = sorted(
        name
        for name in sys.modules
        if name.startswith(("forgecad_agent", "wushen_agent"))
    )
    forbidden_modules = {
        "forgecad_agent.api.concept_routes",
        "forgecad_agent.api.module_routes",
        "forgecad_agent.application.product_core",
        "forgecad_agent.application.sqlite_asset_store",
    }
    require(not forbidden_modules.intersection(loaded), "LEGACY_PRODUCT_MODULE_IMPORTED")
    return {
        "schema_version": "ForgeCADTaskDRuntimeBoundarySmoke@1",
        "status": "pass",
        "python_role": health_payload.get("python_role"),
        "product_state_owner": app.state.product_state_owner,
        "legacy_routes": checks,
        "legacy_factory": "explicit_test_only_direct_call",
        "loaded_module_count": len(loaded),
        "legacy_modules_loaded": sorted(forbidden_modules.intersection(loaded)),
    }


def main() -> int:
    print(json.dumps(asyncio.run(run()), ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BoundaryFailure as error:
        print(str(error))
        raise SystemExit(1)
