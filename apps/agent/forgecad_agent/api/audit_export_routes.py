from __future__ import annotations

from typing import Annotated, Optional, Union

from fastapi import APIRouter, Header, HTTPException, Query, Response
from fastapi.responses import JSONResponse

from forgecad_agent.application.concept_change_set_audits import (
    ChangeSetAuditExportError,
    ChangeSetAuditExportIdempotencyConflict,
    ConceptChangeSetAuditService,
)
from forgecad_agent.application.concept_models import (
    ChangeSetAuditExportListResponse,
    ChangeSetAuditExportRecord,
    CreateChangeSetAuditExportRequest,
)


def build_audit_export_router(service: ConceptChangeSetAuditService) -> APIRouter:
    router = APIRouter(prefix="/api/v1", tags=["change-set-audit-exports"])

    @router.post(
        "/projects/{project_id}/change-set-audit-exports",
        response_model=ChangeSetAuditExportRecord,
        status_code=201,
    )
    def create_audit_export(
        project_id: str,
        request: CreateChangeSetAuditExportRequest,
        idempotency_key: Annotated[Optional[str], Header(alias="Idempotency-Key")] = None,
    ) -> Union[ChangeSetAuditExportRecord, JSONResponse]:
        key = _require_idempotency_key(idempotency_key)
        try:
            return service.create_export(project_id, request, key)
        except ChangeSetAuditExportIdempotencyConflict as exc:
            return _error_response(409, "IDEMPOTENCY_CONFLICT", str(exc))
        except ChangeSetAuditExportError as exc:
            return _audit_export_error_response(exc)

    @router.get(
        "/projects/{project_id}/change-set-audit-exports",
        response_model=ChangeSetAuditExportListResponse,
    )
    def list_audit_exports(
        project_id: str,
        limit: Annotated[int, Query(ge=1, le=100)] = 50,
    ) -> Union[ChangeSetAuditExportListResponse, JSONResponse]:
        try:
            return service.list_for_project(project_id, limit=limit)
        except ChangeSetAuditExportError as exc:
            return _audit_export_error_response(exc)

    @router.get(
        "/change-set-audit-exports/{audit_export_id}",
        response_model=ChangeSetAuditExportRecord,
    )
    def get_audit_export(
        audit_export_id: str,
    ) -> Union[ChangeSetAuditExportRecord, JSONResponse]:
        try:
            return service.get_export(audit_export_id)
        except ChangeSetAuditExportError as exc:
            return _audit_export_error_response(exc)

    @router.get(
        "/change-set-audit-exports/{audit_export_id}/file",
        response_model=None,
    )
    def download_audit_export(
        audit_export_id: str,
    ) -> Union[Response, JSONResponse]:
        try:
            payload, filename, sha256 = service.read_export(audit_export_id)
        except ChangeSetAuditExportError as exc:
            return _audit_export_error_response(exc)
        return Response(
            content=payload,
            media_type="application/zip",
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


def _audit_export_error_response(exc: ChangeSetAuditExportError) -> JSONResponse:
    if exc.code in {"PROJECT_NOT_FOUND", "AUDIT_EXPORT_NOT_FOUND"}:
        status_code = 404
    elif exc.code == "AUDIT_EXPORT_LIMIT_EXCEEDED":
        status_code = 409
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
