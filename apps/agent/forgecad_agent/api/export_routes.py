from __future__ import annotations

from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, HTTPException, Response
from fastapi.responses import JSONResponse

from forgecad_agent.application.concept_exports import (
    ConceptExportError,
    ConceptExportIdempotencyConflict,
    ConceptExportService,
)
from forgecad_agent.application.concept_models import (
    ConceptExportRecord,
    CreateConceptExportRequest,
)


def build_export_router(service: ConceptExportService) -> APIRouter:
    router = APIRouter(prefix="/api/v1", tags=["concept-exports"])

    @router.post(
        "/versions/{version_id}/exports",
        response_model=ConceptExportRecord,
        status_code=201,
    )
    def create_export(
        version_id: str,
        request: CreateConceptExportRequest,
        idempotency_key: Annotated[
            Optional[str],
            Header(alias="Idempotency-Key"),
        ] = None,
    ) -> Union[ConceptExportRecord, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.create_export(version_id, request, key)
        except ConceptExportIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ConceptExportError as exc:
            return _export_error_response(exc)

    @router.get("/exports/{export_id}", response_model=ConceptExportRecord)
    def get_export(export_id: str) -> Union[ConceptExportRecord, JSONResponse]:
        try:
            return service.get_export(export_id)
        except ConceptExportError as exc:
            return _export_error_response(exc)

    @router.get("/exports/{export_id}/file", response_model=None)
    def download_export(export_id: str) -> Union[Response, JSONResponse]:
        try:
            payload, filename, sha256 = service.read_export(export_id)
        except ConceptExportError as exc:
            return _export_error_response(exc)
        return Response(
            content=payload,
            media_type="application/zip",
            headers={
                "Content-Disposition": f'attachment; filename="{filename}"',
                "X-Content-SHA256": sha256,
            },
        )

    @router.get("/exports/{export_id}/combined.glb", response_model=None)
    def download_combined_glb(export_id: str) -> Union[Response, JSONResponse]:
        try:
            payload, filename, sha256 = service.read_combined_glb(export_id)
        except ConceptExportError as exc:
            return _export_error_response(exc)
        return Response(
            content=payload,
            media_type="model/gltf-binary",
            headers={
                "Content-Disposition": f'attachment; filename="{filename}"',
                "X-Content-SHA256": sha256,
            },
        )

    @router.get("/exports/{export_id}/combined.obj", response_model=None)
    def download_combined_obj(export_id: str) -> Union[Response, JSONResponse]:
        try:
            payload, filename, sha256 = service.read_combined_obj(export_id)
        except ConceptExportError as exc:
            return _export_error_response(exc)
        return Response(
            content=payload,
            media_type="model/obj",
            headers={
                "Content-Disposition": f'attachment; filename="{filename}"',
                "X-Content-SHA256": sha256,
            },
        )

    @router.get("/exports/{export_id}/combined.mtl", response_model=None)
    def download_combined_mtl(export_id: str) -> Union[Response, JSONResponse]:
        try:
            payload, filename, sha256 = service.read_combined_mtl(export_id)
        except ConceptExportError as exc:
            return _export_error_response(exc)
        return Response(
            content=payload,
            media_type="model/mtl",
            headers={
                "Content-Disposition": f'attachment; filename="{filename}"',
                "X-Content-SHA256": sha256,
            },
        )

    @router.get("/exports/{export_id}/preview.png", response_model=None)
    def download_preview_png(export_id: str) -> Union[Response, JSONResponse]:
        try:
            payload, filename, sha256 = service.read_preview_png(export_id)
        except ConceptExportError as exc:
            return _export_error_response(exc)
        return Response(
            content=payload,
            media_type="image/png",
            headers={
                "Content-Disposition": f'attachment; filename="{filename}"',
                "X-Content-SHA256": sha256,
            },
        )

    @router.get("/exports/{export_id}/exploded.png", response_model=None)
    def download_exploded_png(export_id: str) -> Union[Response, JSONResponse]:
        try:
            payload, filename, sha256 = service.read_exploded_png(export_id)
        except ConceptExportError as exc:
            return _export_error_response(exc)
        return Response(
            content=payload,
            media_type="image/png",
            headers={
                "Content-Disposition": f'attachment; filename="{filename}"',
                "X-Content-SHA256": sha256,
            },
        )

    return router


def _require_idempotency_key(value: Optional[str]) -> str:
    if not value:
        raise HTTPException(status_code=400, detail="Idempotency-Key is required.")
    return value


def _export_error_response(exc: ConceptExportError) -> JSONResponse:
    if exc.code in {
        "VERSION_NOT_FOUND",
        "MODULE_GRAPH_NOT_FOUND",
        "MODULE_NOT_FOUND",
        "MODULE_ASSET_NOT_FOUND",
        "EXPORT_NOT_FOUND",
    }:
        status_code = 404
    elif exc.code in {"EXPORT_SOURCE_UNAVAILABLE", "EXPORT_PACKAGE_UNAVAILABLE"}:
        status_code = 503
    elif exc.code == "INVALID_REQUEST":
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
