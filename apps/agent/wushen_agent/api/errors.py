from __future__ import annotations

from typing import Any, Optional

from fastapi import FastAPI, HTTPException, Request
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse


def register_error_handlers(app: FastAPI) -> None:
    @app.exception_handler(HTTPException)
    async def http_error_handler(_request: Request, exc: HTTPException) -> JSONResponse:
        detail = exc.detail if isinstance(exc.detail, str) else "Request failed."
        code = "INVALID_REQUEST"
        if exc.status_code == 404:
            code = "JOB_NOT_FOUND"
        elif exc.status_code == 409:
            code = "IDEMPOTENCY_CONFLICT"
        return error_response(exc.status_code, code, detail)

    @app.exception_handler(RequestValidationError)
    async def validation_error_handler(
        _request: Request,
        exc: RequestValidationError,
    ) -> JSONResponse:
        return error_response(
            422,
            "INVALID_REQUEST",
            "Request validation failed.",
            details={"errors": _serializable_validation_errors(exc.errors())},
        )


def provider_error_status(code: str) -> int:
    if code == "PROVIDER_UNCONFIGURED":
        return 400
    if code == "PROVIDER_AUTH_FAILED":
        return 401
    if code == "RATE_LIMITED":
        return 429
    if code in {"INVALID_LLM_JSON", "PROVIDER_BAD_OUTPUT"}:
        return 502
    return 504


def asset_store_error_status(code: str) -> int:
    if code in {
        "WEAPON_NOT_FOUND",
        "VERSION_NOT_FOUND",
        "ASSET_FILE_MISSING",
        "INVALID_INTERPRETATION_ID",
    }:
        return 404
    if code in {
        "MASK_EMPTY",
        "MASK_SIZE_MISMATCH",
        "INVALID_REQUEST",
        "REVEAL_UNSUPPORTED",
        "INVALID_INTERPRETATION_CANDIDATE",
    }:
        return 400
    if code == "INVALID_EVENT_CURSOR":
        return 400
    if code in {"JOB_ACTION_CONFLICT", "INTERPRETATION_NOT_CONFIRMED"}:
        return 409
    if code == "PROVIDER_UNCONFIGURED":
        return 400
    if code in {"PROVIDER_BAD_OUTPUT", "GLB_INVALID", "QUALITY_CHECK_FAILED"}:
        return 502
    if code == "PROVIDER_TIMEOUT":
        return 504
    return 500


def error_response(
    status_code: int,
    code: str,
    message: str,
    details: Optional[dict[str, object]] = None,
    recoverable: Optional[bool] = None,
) -> JSONResponse:
    return JSONResponse(
        status_code=status_code,
        content={
            "error": {
                "code": code,
                "message": message,
                "recoverable": (
                    recoverable
                    if recoverable is not None
                    else status_code >= 500 or status_code in {409, 429}
                ),
                "details": details or {},
            }
        },
    )


def _serializable_validation_errors(
    errors: list[dict[str, Any]],
) -> list[dict[str, Any]]:
    sanitized: list[dict[str, Any]] = []
    for error in errors:
        item = dict(error)
        if "ctx" in item:
            item["ctx"] = {
                str(key): str(value)
                for key, value in dict(item["ctx"] or {}).items()
            }
        if "input" in item and not isinstance(
            item["input"],
            (str, int, float, bool, list, dict, type(None)),
        ):
            item["input"] = str(item["input"])
        sanitized.append(item)
    return sanitized
