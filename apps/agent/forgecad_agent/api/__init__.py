"""Lazy public API exports.

K003 starts a restricted geometry-only Python facet.  Importing one small
route must not eagerly import every legacy product route and service, while
the explicit test oracle still needs the historical ``from forgecad_agent.api
import ...`` surface.  PEP 562 lazy attributes preserve that public surface
without widening the production import graph.
"""

from __future__ import annotations

from importlib import import_module
from typing import Any


_EXPORT_MODULES = {
    "LocalApiSettings": ".factory",
    "create_local_api": ".factory",
    "build_concept_project_router": ".concept_routes",
    "build_module_router": ".module_routes",
    "build_change_set_router": ".change_set_routes",
    "build_quality_router": ".quality_routes",
    "build_brief_router": ".brief_routes",
    "build_concept_job_router": ".concept_job_routes",
    "build_export_router": ".export_routes",
    "build_audit_export_router": ".audit_export_routes",
    "build_agent_router": ".agent_routes",
    "build_agent_asset_router": ".agent_asset_routes",
    "build_active_design_router": ".active_design_routes",
}

__all__ = list(_EXPORT_MODULES)


def __getattr__(name: str) -> Any:
    module_name = _EXPORT_MODULES.get(name)
    if module_name is None:
        raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
    value = getattr(import_module(module_name, __name__), name)
    globals()[name] = value
    return value


def __dir__() -> list[str]:
    return sorted({*globals(), *__all__})
