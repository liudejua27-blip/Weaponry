"""Loopback-only K003 Rust-to-Python restricted geometry protocol."""

from __future__ import annotations

import json
import secrets
from collections.abc import Mapping
from typing import Any, Literal, TypeVar

from fastapi import APIRouter, Request
from fastapi.responses import JSONResponse
from pydantic import ValidationError
from starlette.concurrency import run_in_threadpool

from forgecad_agent.application.restricted_geometry_executor import (
    MAX_RESTRICTED_GEOMETRY_REQUEST_BYTES,
    RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV,
    RESTRICTED_GEOMETRY_PROTOCOL_VERSION,
    RestrictedGeometryBoundaryError,
    RestrictedGeometryApiModel,
    RestrictedGeometryCancellationRequest,
    RestrictedGeometryCancellationResult,
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutionResult,
    RestrictedGeometryExecutor,
    validate_restricted_geometry_payload,
)


RESTRICTED_GEOMETRY_INTERNAL_PREFIX = "/api/v1/internal/geometry"
RESTRICTED_GEOMETRY_CAPABILITY_HEADER = (
    "X-ForgeCAD-Restricted-Geometry-Capability"
)

_UNCONFIGURED_COMPARISON_TOKEN = "0" * 64
_LOOPBACK_HOSTS = frozenset({"127.0.0.1", "::1", "localhost", "testclient"})
_ModelT = TypeVar("_ModelT", bound=RestrictedGeometryApiModel)


class RestrictedGeometryCapabilityOwnership(RestrictedGeometryApiModel):
    schema_version: Literal["RestrictedGeometryCapabilityOwnership@1"] = (
        "RestrictedGeometryCapabilityOwnership@1"
    )
    protocol_version: Literal["forgecad.restricted-geometry/1"] = (
        RESTRICTED_GEOMETRY_PROTOCOL_VERSION
    )
    capability_owner: Literal["rust_forgecad_core"] = "rust_forgecad_core"
    python_role: Literal["restricted_geometry_executor"] = (
        "restricted_geometry_executor"
    )
    database_access: Literal[False] = False
    object_store_access: Literal[False] = False
    provider_access: Literal[False] = False
    thread_session_access: Literal[False] = False
    snapshot_write: Literal[False] = False
    accepts_caller_glb: Literal[False] = False
    persistent_artifacts: Literal[False] = False
    actions: tuple[Literal["compile_readback", "render"], ...] = (
        "compile_readback",
        "render",
    )


def build_restricted_geometry_router(
    executor: RestrictedGeometryExecutor,
    *,
    environment: Mapping[str, str],
) -> APIRouter:
    """Build a hidden router whose one-process capability is frozen at startup."""

    configured_capability = environment.get(
        RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV
    )
    capability_available = _valid_capability(configured_capability)
    comparison_capability = (
        configured_capability
        if capability_available and configured_capability is not None
        else _UNCONFIGURED_COMPARISON_TOKEN
    )
    router = APIRouter(
        prefix=RESTRICTED_GEOMETRY_INTERNAL_PREFIX,
        include_in_schema=False,
    )

    @router.get("/capability/ownership")
    async def capability_ownership(request: Request) -> Any:
        denied = _authorize(
            request,
            capability_available=capability_available,
            comparison_capability=comparison_capability,
        )
        if denied is not None:
            return denied
        return RestrictedGeometryCapabilityOwnership()

    @router.post("/execute")
    async def execute_geometry(request: Request) -> Any:
        denied = _authorize(
            request,
            capability_available=capability_available,
            comparison_capability=comparison_capability,
        )
        if denied is not None:
            return denied
        body = await _parse_body(request, RestrictedGeometryExecutionRequest)
        if isinstance(body, JSONResponse):
            return body
        try:
            result = await run_in_threadpool(executor.execute, body)
        except RestrictedGeometryBoundaryError as exc:
            return _boundary_error(exc)
        except Exception:
            return _error(
                503,
                "GEOMETRY_EXECUTOR_INTERNAL_ERROR",
                "The restricted geometry executor failed without exposing internal values.",
                recoverable=True,
            )
        return RestrictedGeometryExecutionResult.model_validate(result)

    @router.post("/cancel")
    async def cancel_geometry(request: Request) -> Any:
        denied = _authorize(
            request,
            capability_available=capability_available,
            comparison_capability=comparison_capability,
        )
        if denied is not None:
            return denied
        body = await _parse_body(request, RestrictedGeometryCancellationRequest)
        if isinstance(body, JSONResponse):
            return body
        try:
            result = await run_in_threadpool(executor.cancel, body)
        except RestrictedGeometryBoundaryError as exc:
            return _boundary_error(exc)
        except Exception:
            return _error(
                503,
                "GEOMETRY_EXECUTOR_INTERNAL_ERROR",
                "The restricted geometry executor failed without exposing internal values.",
                recoverable=True,
            )
        return RestrictedGeometryCancellationResult.model_validate(result)

    return router


def _authorize(
    request: Request,
    *,
    capability_available: bool,
    comparison_capability: str,
) -> JSONResponse | None:
    host = request.client.host if request.client is not None else ""
    if host not in _LOOPBACK_HOSTS:
        return _error(
            403,
            "GEOMETRY_EXECUTOR_LOOPBACK_REQUIRED",
            "The restricted geometry executor is available only to the local Rust core.",
        )

    candidates = request.headers.getlist(RESTRICTED_GEOMETRY_CAPABILITY_HEADER)
    supplied = candidates[0] if len(candidates) == 1 else ""
    supplied_valid = _valid_capability(supplied)
    comparison_value = (
        supplied if supplied_valid else _UNCONFIGURED_COMPARISON_TOKEN
    )
    matched = secrets.compare_digest(comparison_value, comparison_capability)
    if not capability_available:
        return _error(
            503,
            "GEOMETRY_EXECUTOR_CAPABILITY_UNAVAILABLE",
            "The restricted geometry capability was not configured at process startup.",
            recoverable=True,
        )
    if len(candidates) != 1 or not supplied_valid or not matched:
        return _error(
            403,
            "GEOMETRY_EXECUTOR_CAPABILITY_REJECTED",
            "The restricted geometry capability was rejected.",
        )
    return None


def _valid_capability(value: str | None) -> bool:
    return bool(
        value
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


async def _parse_body(request: Request, model: type[_ModelT]) -> _ModelT | JSONResponse:
    media_type = request.headers.get("content-type", "").split(";", 1)[0].strip().casefold()
    if media_type != "application/json":
        return _error(
            415,
            "GEOMETRY_REQUEST_MEDIA_TYPE_INVALID",
            "The restricted geometry protocol accepts only application/json.",
        )
    raw = await request.body()
    if len(raw) > MAX_RESTRICTED_GEOMETRY_REQUEST_BYTES:
        return _error(
            413,
            "GEOMETRY_REQUEST_TOO_LARGE",
            "The restricted geometry request exceeded its bounded payload size.",
        )
    try:
        value = json.loads(raw.decode("utf-8"))
        validate_restricted_geometry_payload(value)
        return model.model_validate(value)
    except RestrictedGeometryBoundaryError as exc:
        return _boundary_error(exc)
    except (UnicodeDecodeError, json.JSONDecodeError, ValidationError, TypeError, ValueError):
        return _error(
            400,
            "GEOMETRY_REQUEST_INVALID",
            "The request did not match the strict restricted geometry contract.",
        )


def _boundary_error(error: RestrictedGeometryBoundaryError) -> JSONResponse:
    return _error(
        error.status_code,
        error.code,
        str(error),
        recoverable=error.recoverable,
    )


def _error(
    status_code: int,
    code: str,
    message: str,
    *,
    recoverable: bool = False,
) -> JSONResponse:
    return JSONResponse(
        status_code=status_code,
        content={
            "error": {
                "code": code,
                "message": message,
                "recoverable": recoverable,
                "details": {},
            }
        },
    )
