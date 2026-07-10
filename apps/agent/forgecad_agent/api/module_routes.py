from __future__ import annotations

from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, HTTPException, Query
from fastapi.responses import JSONResponse

from forgecad_agent.application.concept_models import (
    ModuleAssetListResponse,
    ModuleAssetRecord,
    ModuleGraphRecord,
    ModuleGraphValidationResponse,
    RegisterModuleAssetRequest,
    ValidateModuleGraphRequest,
)
from forgecad_agent.application.concept_modules import (
    ConceptModuleError,
    ConceptModuleIdempotencyConflict,
    ConceptModuleService,
)
from forgecad_agent.domain.concepts.models import ModuleCategory


def build_module_router(service: ConceptModuleService) -> APIRouter:
    router = APIRouter(prefix="/api/v1", tags=["concept-modules"])

    @router.post("/module-assets", response_model=ModuleAssetRecord, status_code=201)
    def register_module_asset(
        request: RegisterModuleAssetRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[ModuleAssetRecord, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.register_module(request, key)
        except ConceptModuleIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptModuleError as exc:
            return _module_error_response(exc)

    @router.get("/module-assets", response_model=ModuleAssetListResponse)
    def list_module_assets(
        pack_id: Optional[str] = Query(default=None),
        category: Optional[ModuleCategory] = Query(default=None),
    ) -> ModuleAssetListResponse:
        return service.list_modules(pack_id=pack_id, category=category)

    @router.post(
        "/module-graphs/{graph_id}/validate",
        response_model=ModuleGraphValidationResponse,
    )
    def validate_module_graph(
        graph_id: str,
        request: ValidateModuleGraphRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[ModuleGraphValidationResponse, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.validate_graph(graph_id, request, key)
        except ConceptModuleIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptModuleError as exc:
            return _module_error_response(exc)

    @router.get("/module-graphs/{graph_id}", response_model=ModuleGraphRecord)
    def get_module_graph(
        graph_id: str,
    ) -> Union[ModuleGraphRecord, JSONResponse]:
        try:
            return service.get_graph(graph_id)
        except ConceptModuleError as exc:
            return _module_error_response(exc)

    return router


def _require_idempotency_key(value: Optional[str]) -> str:
    if not value:
        raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
    return value


def _module_error_response(exc: ConceptModuleError) -> JSONResponse:
    if exc.code in {
        "PROJECT_NOT_FOUND",
        "DOMAIN_PROFILE_NOT_FOUND",
        "MODULE_GRAPH_NOT_FOUND",
    }:
        status_code = 404
    elif exc.code in {
        "MODULE_ALREADY_EXISTS",
        "MODULE_GRAPH_CONFLICT",
        "MODULE_HASH_MISMATCH",
    }:
        status_code = 409
    elif exc.code in {"INVALID_REQUEST", "INVALID_GLB"}:
        status_code = 400
    else:
        status_code = 500
    return _error_response(status_code, exc.code, str(exc))


def _error_response(status_code: int, code: str, message: str) -> JSONResponse:
    return JSONResponse(
        status_code=status_code,
        content={
            "error": {
                "code": code,
                "message": message,
                "recoverable": status_code >= 500 or status_code == 409,
                "details": {},
            }
        },
    )
