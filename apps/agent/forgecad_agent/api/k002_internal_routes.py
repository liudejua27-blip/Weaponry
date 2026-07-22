"""Loopback-only K002 Rust-to-Python compatibility ports.

These endpoints are deliberately absent from OpenAPI and are not a product
API.  The Rust app-server owns lifecycle and Provider decisions; this router
only exposes the transitional Python lifecycle writer and bounded Product Tool
executor to the colocated desktop sidecar.
"""

from __future__ import annotations

import json
import os
import secrets
from collections.abc import Mapping
from typing import Any, Literal, TypeVar

from fastapi import APIRouter, Request
from fastapi.responses import JSONResponse
from pydantic import Field, ValidationError
from starlette.concurrency import run_in_threadpool

from forgecad_agent.application.concept_models import StrictApiModel
from forgecad_agent.application.k002_port_contracts import (
    LifecyclePersistenceCommand,
    LifecyclePersistenceResult,
    ProductToolExecutionRequest,
    ProductToolExecutionResult,
)
from forgecad_agent.application.k002_port_security import (
    K002PortBoundaryError,
    MAX_PORT_JSON_BYTES,
)
from forgecad_agent.application.k002_python_ports import (
    LifecyclePersistencePort,
    ProductToolExecutorPort,
)


K002_INTERNAL_CAPABILITY_TOKEN_ENV = "FORGECAD_K002_INTERNAL_CAPABILITY_TOKEN"
K002_INTERNAL_CAPABILITY_HEADER = "X-ForgeCAD-K002-Internal-Capability"
K002_INTERNAL_PREFIX = "/api/v1/internal/k002"

_MAX_CAPABILITY_TOKEN_CHARS = 256
_UNCONFIGURED_COMPARISON_TOKEN = "0" * 64
_LOOPBACK_HOSTS = frozenset({"127.0.0.1", "::1", "localhost", "testclient"})
_ModelT = TypeVar("_ModelT", bound=StrictApiModel)


class ProductToolCancellationRequest(StrictApiModel):
    schema_version: Literal["ProductToolCancellationRequest@1"] = (
        "ProductToolCancellationRequest@1"
    )
    cancellation_id: str = Field(pattern=r"^[A-Za-z0-9_.\-]{1,160}$")
    cancellation_token: str = Field(pattern=r"^[A-Za-z0-9_.\-]{1,160}$")


class ProductToolCancellationResult(StrictApiModel):
    schema_version: Literal["ProductToolCancellationResult@1"] = (
        "ProductToolCancellationResult@1"
    )
    cancellation_id: str = Field(pattern=r"^[A-Za-z0-9_.\-]{1,160}$")
    accepted: Literal[True] = True


class K002InternalCapabilityOwnership(StrictApiModel):
    """Fixed, non-secret identity returned after capability authorization."""

    schema_version: Literal["K002InternalCapabilityOwnership@1"] = (
        "K002InternalCapabilityOwnership@1"
    )
    capability_owner: Literal["rust_app_server"] = "rust_app_server"
    port_owner: Literal["python_compatibility_service"] = (
        "python_compatibility_service"
    )
    lifecycle_owner: Literal["rust_app_server"] = "rust_app_server"


def build_k002_internal_router(
    lifecycle: LifecyclePersistencePort,
    product_tools: ProductToolExecutorPort,
    *,
    environment: Mapping[str, str] | None = None,
) -> APIRouter:
    """Build the hidden internal router and snapshot its startup capability.

    The capability is intentionally read once while the process is creating
    the application.  Adding or changing the environment variable later does
    not silently enable the port; the sidecar must be restarted with a fresh
    shared capability.
    """

    values = os.environ if environment is None else environment
    configured_capability = values.get(K002_INTERNAL_CAPABILITY_TOKEN_ENV)
    capability_available = _valid_capability(configured_capability)
    comparison_capability = (
        configured_capability
        if capability_available and configured_capability is not None
        else _UNCONFIGURED_COMPARISON_TOKEN
    )

    router = APIRouter(
        prefix=K002_INTERNAL_PREFIX,
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
        # This handshake deliberately does not parse a body or touch either
        # injected Python port.  It proves only that the Rust supervisor and
        # this loopback compatibility service share the startup capability.
        return K002InternalCapabilityOwnership()

    @router.post("/lifecycle/execute")
    async def execute_lifecycle(request: Request) -> Any:
        denied = _authorize(
            request,
            capability_available=capability_available,
            comparison_capability=comparison_capability,
        )
        if denied is not None:
            return denied
        body = await _parse_body(request, LifecyclePersistenceCommand)
        if isinstance(body, JSONResponse):
            return body
        try:
            result = await run_in_threadpool(lifecycle.execute, body)
        except K002PortBoundaryError as exc:
            return _port_error(exc)
        except Exception:
            return _internal_error()
        return LifecyclePersistenceResult.model_validate(result)

    @router.post("/product-tools/execute")
    async def execute_product_tool(request: Request) -> Any:
        denied = _authorize(
            request,
            capability_available=capability_available,
            comparison_capability=comparison_capability,
        )
        if denied is not None:
            return denied
        body = await _parse_body(request, ProductToolExecutionRequest)
        if isinstance(body, JSONResponse):
            return body
        try:
            result = await run_in_threadpool(product_tools.execute, body)
        except K002PortBoundaryError as exc:
            return _port_error(exc)
        except Exception:
            return _internal_error()
        return ProductToolExecutionResult.model_validate(result)

    @router.post("/product-tools/cancel")
    async def cancel_product_tools(request: Request) -> Any:
        denied = _authorize(
            request,
            capability_available=capability_available,
            comparison_capability=comparison_capability,
        )
        if denied is not None:
            return denied
        body = await _parse_body(request, ProductToolCancellationRequest)
        if isinstance(body, JSONResponse):
            return body
        try:
            await run_in_threadpool(
                product_tools.cancel,
                cancellation_id=body.cancellation_id,
                cancellation_token=body.cancellation_token,
            )
        except K002PortBoundaryError as exc:
            return _port_error(exc)
        except Exception:
            return _internal_error()
        return ProductToolCancellationResult(cancellation_id=body.cancellation_id)

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
            "K002_INTERNAL_LOOPBACK_REQUIRED",
            "The K002 compatibility port is available only to the local sidecar.",
        )

    candidates = request.headers.getlist(K002_INTERNAL_CAPABILITY_HEADER)
    supplied = candidates[0] if len(candidates) == 1 else ""
    supplied_valid = _valid_capability(supplied)
    # Always perform one constant-time comparison, including unavailable and
    # malformed cases.  Invalid values are replaced before compare_digest so
    # non-ASCII header input cannot raise or be reflected in an error.
    comparison_value = supplied if supplied_valid else _UNCONFIGURED_COMPARISON_TOKEN
    matched = secrets.compare_digest(comparison_value, comparison_capability)

    if not capability_available:
        return _error(
            503,
            "K002_INTERNAL_PORT_UNAVAILABLE",
            "The K002 compatibility port was not enabled when the sidecar started.",
            recoverable=True,
        )
    if len(candidates) != 1 or not supplied_valid or not matched:
        return _error(
            403,
            "K002_INTERNAL_CAPABILITY_REJECTED",
            "The K002 compatibility capability was rejected.",
        )
    return None


def _valid_capability(value: str | None) -> bool:
    return bool(
        value
        and len(value) <= _MAX_CAPABILITY_TOKEN_CHARS
        and value.isascii()
        and all(33 <= ord(character) <= 126 for character in value)
    )


async def _parse_body(
    request: Request,
    model: type[_ModelT],
) -> _ModelT | JSONResponse:
    raw = await request.body()
    if len(raw) > MAX_PORT_JSON_BYTES:
        return _error(
            413,
            "K002_INTERNAL_PAYLOAD_TOO_LARGE",
            "The K002 compatibility payload exceeded its bounded size.",
        )
    try:
        value = json.loads(raw.decode("utf-8"))
        return model.model_validate(value)
    except (UnicodeDecodeError, json.JSONDecodeError, ValidationError, TypeError, ValueError):
        return _error(
            400,
            "K002_INTERNAL_REQUEST_INVALID",
            "The K002 compatibility request did not match its strict contract.",
        )


def _port_error(error: K002PortBoundaryError) -> JSONResponse:
    if error.code in {
        "K002_PERSISTENCE_CAS_CONFLICT",
        "K002_PERSISTENCE_IDEMPOTENCY_CONFLICT",
        "K002_PERSISTENCE_SQLITE_CONFLICT",
        "K002_PRODUCT_TOOL_CALL_ID_CONFLICT",
        "K002_PRODUCT_TOOL_IDEMPOTENCY_CONFLICT",
        "K002_PRODUCT_TOOL_CALL_IN_FLIGHT",
        "K002_EXECUTOR_RUN_IDENTITY_DRIFT",
        "K002_CANCELLATION_ID_CONFLICT",
        "K002_CANCELLATION_TOKEN_MISMATCH",
    }:
        status_code = 409
    elif error.code in {
        "K002_PORT_JSON_TOO_LARGE",
        "K002_PORT_STRING_TOO_LARGE",
    }:
        status_code = 413
    elif error.code in {
        "K002_PERSISTENCE_BACKEND_FAILED",
        "K002_EXECUTOR_BACKPRESSURE",
    }:
        status_code = 503
    elif error.code == "K002_PROVIDER_ENVIRONMENT_FORBIDDEN":
        status_code = 403
    else:
        status_code = 400
    return _error(
        status_code,
        error.code,
        str(error),
        recoverable=error.recoverable,
    )


def _internal_error() -> JSONResponse:
    return _error(
        500,
        "K002_INTERNAL_PORT_FAILED",
        "The K002 compatibility port failed without exposing internal state.",
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
            }
        },
    )
