"""Capability-scoped Python geometry executor for the K003 Rust core.

The Rust core owns every persistent product object.  This module is only a
bounded, short-lived compiler/readback/renderer facet: it receives a validated
ShapeProgram (including canonical ProfileSketch/ProfileSectionSet payloads),
returns immutable bytes and facts, and never receives a database path, object
store root, Provider credential, Thread/session identity, or Snapshot write
authority.

Execution happens in a disposable child process.  Cancellation and timeout
therefore have a real process boundary, and a terminal tombstone is checked
before a result is promoted into the in-memory artifact cache.  The cache is
ephemeral and opaque; it exists only so a follow-up render call does not need
to accept arbitrary GLB bytes from a caller.
"""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import math
import multiprocessing
import os
import re
import secrets
import threading
import time
from collections import OrderedDict
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from typing import Any, Literal, Optional

from pydantic import BaseModel, ConfigDict, Field, model_validator


RESTRICTED_GEOMETRY_PROTOCOL_VERSION = "forgecad.restricted-geometry/1"
RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV = (
    "FORGECAD_RESTRICTED_GEOMETRY_CAPABILITY_TOKEN"
)

MAX_RESTRICTED_GEOMETRY_REQUEST_BYTES = 2 * 1024 * 1024
MAX_RESTRICTED_GEOMETRY_RESULT_BYTES = 96 * 1024 * 1024
MAX_RESTRICTED_GEOMETRY_ACTIVE_EXECUTIONS = 2
MAX_RESTRICTED_GEOMETRY_TERMINAL_RECORDS = 24
MAX_RESTRICTED_GEOMETRY_TERMINAL_RESULT_BYTES = 192 * 1024 * 1024
MAX_RESTRICTED_GEOMETRY_CANCELLATION_TOMBSTONES = 128
MAX_RESTRICTED_GEOMETRY_ARTIFACTS = 16
MAX_RESTRICTED_GEOMETRY_ARTIFACT_BYTES = 128 * 1024 * 1024
RESTRICTED_GEOMETRY_ARTIFACT_TTL_SECONDS = 300.0

_ID_PATTERN = r"^[A-Za-z0-9_.\-]{1,160}$"
_SHA256_PATTERN = r"^[a-f0-9]{64}$"
_ARTIFACT_HANDLE_PATTERN = r"^geomart_[A-Za-z0-9_\-]{32,160}$"
_SURFACE_LAYER_INPUT_FIELDS = frozenset(
    {"schema_version", "lowering", "lowering_sha256"}
)
_SECRET_VALUE_PREFIXES = ("Bearer ", "sk-")
_FORBIDDEN_CONTEXT_KEYS = frozenset(
    {
        "authorization",
        "api_key",
        "provider_key",
        "provider_secret",
        "credential",
        "credentials",
        "database_path",
        "db_path",
        "sqlite_path",
        "object_store_path",
        "object_store_root",
        "library_root",
        "thread_id",
        "session_id",
        "snapshot",
        "snapshot_id",
        "snapshot_revision",
        "snapshot_write_token",
        "write_token",
        "file_path",
        "glb_base64",
        "url",
        "uri",
        "code",
        "script",
        "shell",
        "command",
        "python",
        "javascript",
        "mechanical_concept_plan",
        "concept_plan",
        "plan",
        "mechanical_style_token",
        "style_token",
        "style_recipe",
        "component_recipe",
        "recipe",
    }
)
_ALLOWED_EXECUTOR_ENVIRONMENT_NAMES = frozenset(
    {
        RESTRICTED_GEOMETRY_CAPABILITY_TOKEN_ENV,
        "FORGECAD_RUNTIME_RESOURCE_ROOT",
    }
)


class RestrictedGeometryBoundaryError(RuntimeError):
    """Stable, value-free error safe to return over the internal port."""

    def __init__(
        self,
        code: str,
        message: str,
        *,
        status_code: int = 400,
        recoverable: bool = False,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.status_code = status_code
        self.recoverable = recoverable


class RestrictedGeometryApiModel(BaseModel):
    model_config = ConfigDict(extra="forbid", allow_inf_nan=False)


class RestrictedGeometryExplodedPart(RestrictedGeometryApiModel):
    part_id: str = Field(pattern=_ID_PATTERN)
    translation_mm: list[float] = Field(min_length=3, max_length=3)


class RestrictedGeometryRenderOptions(RestrictedGeometryApiModel):
    width: int = Field(default=640, ge=64, le=2048)
    height: int = Field(default=640, ge=64, le=2048)
    exploded_parts: list[RestrictedGeometryExplodedPart] = Field(
        default_factory=list,
        max_length=512,
    )

    @model_validator(mode="after")
    def validate_part_ids(self) -> "RestrictedGeometryRenderOptions":
        part_ids = [item.part_id for item in self.exploded_parts]
        if len(part_ids) != len(set(part_ids)):
            raise ValueError("exploded part ids must be unique")
        return self


class RestrictedGeometryExecutionRequest(RestrictedGeometryApiModel):
    schema_version: Literal["RestrictedGeometryExecutionRequest@1"] = (
        "RestrictedGeometryExecutionRequest@1"
    )
    protocol_version: Literal["forgecad.restricted-geometry/1"] = (
        RESTRICTED_GEOMETRY_PROTOCOL_VERSION
    )
    execution_id: str = Field(pattern=_ID_PATTERN)
    idempotency_key: str = Field(pattern=_ID_PATTERN)
    cancellation_id: str = Field(pattern=_ID_PATTERN)
    cancellation_token: str = Field(pattern=_ID_PATTERN)
    action: Literal["compile_readback", "render"]
    # Production concept compilation is on-demand and may expand a reviewed
    # multi-part arm before returning one GLB. Keep this below the Rust
    # Product Tool hard ceiling while allowing a cold macOS worker to finish.
    timeout_ms: int = Field(default=30_000, ge=50, le=240_000)
    artifact_profile_id: Optional[
        Literal["interactive_preview", "production_concept"]
    ] = None
    shape_program: Optional[dict[str, Any]] = None
    # Rust Core owns the persisted ShapeProgram identity.  These optional
    # fields let the native caller seal its canonical bytes without forcing
    # legacy geometry-only probes to adopt a second protocol version at once.
    # When present they are an inseparable pair and are verified before a
    # disposable compiler process is started.
    shape_program_canonical_json: Optional[str] = Field(
        default=None,
        max_length=MAX_RESTRICTED_GEOMETRY_REQUEST_BYTES,
    )
    shape_program_sha256: Optional[str] = Field(
        default=None,
        pattern=_SHA256_PATTERN,
    )
    profile_sketch: Optional[dict[str, Any]] = None
    section_set: Optional[dict[str, Any]] = None
    surface_adornment_programs: list[dict[str, Any]] = Field(
        default_factory=list,
        max_length=32,
    )
    # A sealed Rust lowering, never a second Python authoring surface.
    surface_layer_input: Optional[dict[str, Any]] = None
    artifact_handle: Optional[str] = Field(
        default=None,
        pattern=_ARTIFACT_HANDLE_PATTERN,
    )
    render: Optional[RestrictedGeometryRenderOptions] = None

    @model_validator(mode="after")
    def validate_action_payload(self) -> "RestrictedGeometryExecutionRequest":
        if self.action == "compile_readback":
            if self.artifact_profile_id is None or self.shape_program is None:
                raise ValueError(
                    "compile_readback requires artifact_profile_id and shape_program"
                )
            if (
                self.artifact_handle is not None
                or self.render is not None
            ):
                raise ValueError("compile_readback cannot carry render artifact fields")
            if (self.shape_program_canonical_json is None) != (
                self.shape_program_sha256 is None
            ):
                raise ValueError(
                    "compile_readback Rust ShapeProgram seal must include canonical JSON and SHA-256"
                )
            if self.shape_program_canonical_json is not None:
                _validate_shape_program_seal(
                    self.shape_program,
                    self.shape_program_canonical_json,
                    self.shape_program_sha256 or "",
                )
            from forgecad_agent.application.visual_texture_sets import (
                normalize_surface_adornment_program,
            )

            normalized = [
                normalize_surface_adornment_program(item)
                for item in self.surface_adornment_programs
            ]
            if [item["program_id"] for item in normalized] != [
                item["program_id"] for item in self.surface_adornment_programs
            ]:
                raise ValueError("surface adornment programs must preserve canonical identity")
            if self.surface_layer_input is not None:
                sealed_surface = _normalize_surface_layer_input(self.surface_layer_input)
                if normalized != sealed_surface["lowering"]["adornments"]:
                    raise ValueError(
                        "surface layer input must carry the exact Rust-lowered A005 adornment list"
                    )
        else:
            if (
                self.artifact_handle is None
                or self.shape_program_sha256 is None
                or self.render is None
            ):
                raise ValueError(
                    "render requires artifact_handle, shape_program_sha256 and render options"
                )
            if (
                self.artifact_profile_id is not None
                or self.shape_program is not None
                or self.shape_program_canonical_json is not None
                or self.profile_sketch is not None
                or self.section_set is not None
                or self.surface_adornment_programs
                or self.surface_layer_input is not None
            ):
                raise ValueError("render accepts only an opaque compiled artifact handle")
        return self


class RestrictedGeometryCancellationRequest(RestrictedGeometryApiModel):
    schema_version: Literal["RestrictedGeometryCancellationRequest@1"] = (
        "RestrictedGeometryCancellationRequest@1"
    )
    protocol_version: Literal["forgecad.restricted-geometry/1"] = (
        RESTRICTED_GEOMETRY_PROTOCOL_VERSION
    )
    cancellation_id: str = Field(pattern=_ID_PATTERN)
    cancellation_token: str = Field(pattern=_ID_PATTERN)


class RestrictedGeometryCancellationResult(RestrictedGeometryApiModel):
    schema_version: Literal["RestrictedGeometryCancellationResult@1"] = (
        "RestrictedGeometryCancellationResult@1"
    )
    cancellation_id: str = Field(pattern=_ID_PATTERN)
    accepted: Literal[True] = True
    tombstoned: Literal[True] = True


class RestrictedGeometryExecutionResult(RestrictedGeometryApiModel):
    schema_version: Literal["RestrictedGeometryExecutionResult@1"] = (
        "RestrictedGeometryExecutionResult@1"
    )
    protocol_version: Literal["forgecad.restricted-geometry/1"] = (
        RESTRICTED_GEOMETRY_PROTOCOL_VERSION
    )
    execution_id: str = Field(pattern=_ID_PATTERN)
    action: Literal["compile_readback", "render"]
    artifact_handle: str = Field(pattern=_ARTIFACT_HANDLE_PATTERN)
    artifact_profile_id: Literal["interactive_preview", "production_concept"]
    artifact_profile_sha256: str = Field(pattern=_SHA256_PATTERN)
    shape_program_sha256: str = Field(pattern=_SHA256_PATTERN)
    glb_sha256: str = Field(pattern=_SHA256_PATTERN)
    glb_byte_size: int = Field(ge=20, le=64 * 1024 * 1024)
    triangle_count: int = Field(ge=1)
    bounds_mm: list[float] = Field(min_length=3, max_length=3)
    readback: Optional[dict[str, Any]] = None
    glb_base64: Optional[str] = None
    render_views: Optional[dict[str, str]] = None
    render_view_sha256: Optional[dict[str, str]] = None
    renderer_id: Optional[Literal["forgecad-agent-software-raster@1"]] = None
    exploded_part_ids: list[str] = Field(default_factory=list, max_length=512)
    exploded_unavailable_reason: Optional[str] = Field(default=None, max_length=500)

    @model_validator(mode="after")
    def validate_result_payload(self) -> "RestrictedGeometryExecutionResult":
        if self.action == "compile_readback":
            if self.readback is None or self.glb_base64 is None:
                raise ValueError("compile_readback result requires GLB bytes and readback")
            if self.render_views is not None or self.render_view_sha256 is not None:
                raise ValueError("compile_readback result cannot contain render views")
            if self.renderer_id is not None:
                raise ValueError("compile_readback result cannot name a renderer")
        else:
            if self.render_views is None or self.render_view_sha256 is None:
                raise ValueError("render result requires views and hashes")
            if set(self.render_views) != set(self.render_view_sha256):
                raise ValueError("render view bytes and hashes must align")
            if self.readback is not None or self.glb_base64 is not None:
                raise ValueError("render result cannot repeat compiled GLB bytes")
            if self.renderer_id is None:
                raise ValueError("render result requires a code-owned renderer identity")
        return self


@dataclass
class _ArtifactRecord:
    handle: str
    artifact_profile_id: Literal["interactive_preview", "production_concept"]
    artifact_profile_sha256: str
    shape_program_sha256: str
    glb_sha256: str
    glb_bytes: bytes
    triangle_count: int
    bounds_mm: list[float]
    created_monotonic: float
    expires_monotonic: float


@dataclass
class _ExecutionRecord:
    execution_id: str
    idempotency_key: str
    fingerprint: str
    cancellation_id: str
    cancellation_token: str
    status: Literal["running", "completed", "failed", "cancelled", "timed_out"]
    created_monotonic: float
    cancel_event: Any | None = None
    process: Any | None = None
    result: RestrictedGeometryExecutionResult | None = None
    retained_result_bytes: int = 0
    error: RestrictedGeometryBoundaryError | None = None


WorkerTarget = Callable[[Any, Any, dict[str, Any], Optional[str]], None]


class RestrictedGeometryExecutor:
    """Bounded, idempotent process supervisor with ephemeral artifacts only."""

    def __init__(
        self,
        *,
        environment: Mapping[str, str] | None = None,
        worker_target: WorkerTarget | None = None,
        artifact_ttl_seconds: float = RESTRICTED_GEOMETRY_ARTIFACT_TTL_SECONDS,
    ) -> None:
        values = dict(os.environ if environment is None else environment)
        validate_restricted_geometry_environment(values)
        self._resource_root = values.get("FORGECAD_RUNTIME_RESOURCE_ROOT")
        self._worker_target = worker_target or _restricted_geometry_worker_entry
        self._artifact_ttl_seconds = artifact_ttl_seconds
        self._context = multiprocessing.get_context("spawn")
        self._lock = threading.RLock()
        self._records: OrderedDict[str, _ExecutionRecord] = OrderedDict()
        self._idempotency_index: dict[str, str] = {}
        self._cancellation_tombstones: OrderedDict[str, str] = OrderedDict()
        self._artifacts: OrderedDict[str, _ArtifactRecord] = OrderedDict()

    def execute(
        self,
        request: RestrictedGeometryExecutionRequest,
    ) -> RestrictedGeometryExecutionResult:
        request_payload = request.model_dump(mode="json", exclude_none=True)
        validate_restricted_geometry_payload(request_payload)
        if request.action == "compile_readback":
            # Validate the complete Schema/G819/Profile boundary before any
            # worker process starts.  The child repeats this check after its
            # environment is cleared, so this is not a trust hand-off.
            from forgecad_agent.application.geometry_worker import (
                assert_shape_program_runtime_compatible,
            )

            try:
                assert_shape_program_runtime_compatible(request.shape_program or {})
                (
                    canonical_profile_sketch,
                    canonical_section_set,
                ) = _validate_restricted_geometry_companions(
                    request.shape_program or {},
                    profile_sketch=request.profile_sketch,
                    section_set=request.section_set,
                )
            except Exception as exc:
                raise RestrictedGeometryBoundaryError(
                    _stable_geometry_error_code(exc),
                    "The ShapeProgram did not pass the restricted Schema/G819 boundary.",
                    status_code=422,
                ) from None

        fingerprint = _canonical_json_sha256(request_payload)
        with self._lock:
            replay = self._prepare_execution(request, fingerprint)
            if replay is not None:
                return replay
            record = self._records[request.execution_id]
            if request.action == "render":
                artifact = self._artifact_for_render(request)
                worker_payload = {
                    "action": "render",
                    "artifact_handle": artifact.handle,
                    "artifact_profile_id": artifact.artifact_profile_id,
                    "artifact_profile_sha256": artifact.artifact_profile_sha256,
                    "shape_program_sha256": artifact.shape_program_sha256,
                    "glb_sha256": artifact.glb_sha256,
                    "glb_bytes": artifact.glb_bytes,
                    "triangle_count": artifact.triangle_count,
                    "bounds_mm": artifact.bounds_mm,
                    "render": request.render.model_dump(mode="json") if request.render else {},
                }
            else:
                sealed_surface = (
                    _normalize_surface_layer_input(request.surface_layer_input)
                    if request.surface_layer_input is not None
                    else None
                )
                worker_payload = {
                    "action": "compile_readback",
                    "artifact_profile_id": request.artifact_profile_id,
                    "shape_program": request.shape_program,
                    "surface_adornment_programs": request.surface_adornment_programs,
                }
                if sealed_surface is not None:
                    worker_payload["surface_layer_lowering"] = sealed_surface["lowering"]
                if canonical_profile_sketch is not None:
                    worker_payload["profile_sketch"] = canonical_profile_sketch
                if canonical_section_set is not None:
                    worker_payload["section_set"] = canonical_section_set

        return self._run_worker(request, record, worker_payload)

    def cancel(
        self,
        request: RestrictedGeometryCancellationRequest,
    ) -> RestrictedGeometryCancellationResult:
        with self._lock:
            existing_token = self._cancellation_tombstones.get(request.cancellation_id)
            if existing_token is not None and not secrets.compare_digest(
                existing_token,
                request.cancellation_token,
            ):
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_CANCELLATION_TOKEN_MISMATCH",
                    "The geometry cancellation token did not match the registered execution.",
                    status_code=409,
                )
            matching_records = [
                record
                for record in self._records.values()
                if record.cancellation_id == request.cancellation_id
            ]
            if any(
                not secrets.compare_digest(
                    record.cancellation_token,
                    request.cancellation_token,
                )
                for record in matching_records
            ):
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_CANCELLATION_TOKEN_MISMATCH",
                    "The geometry cancellation token did not match the registered execution.",
                    status_code=409,
                )
            self._cancellation_tombstones[request.cancellation_id] = (
                request.cancellation_token
            )
            self._cancellation_tombstones.move_to_end(request.cancellation_id)
            for record in matching_records:
                if record.cancel_event is not None:
                    record.cancel_event.set()
            self._prune_cancellation_tombstones_locked()
        return RestrictedGeometryCancellationResult(
            cancellation_id=request.cancellation_id
        )

    def _prepare_execution(
        self,
        request: RestrictedGeometryExecutionRequest,
        fingerprint: str,
    ) -> RestrictedGeometryExecutionResult | None:
        self._prune_locked()
        known_cancellation_token = self._cancellation_tombstones.get(
            request.cancellation_id
        )
        if known_cancellation_token is not None and not secrets.compare_digest(
            known_cancellation_token,
            request.cancellation_token,
        ):
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_CANCELLATION_ID_CONFLICT",
                "The geometry cancellation id was reused with a different token.",
                status_code=409,
            )
        if any(
            record.cancellation_id == request.cancellation_id
            and not secrets.compare_digest(
                record.cancellation_token,
                request.cancellation_token,
            )
            for record in self._records.values()
        ):
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_CANCELLATION_ID_CONFLICT",
                "The geometry cancellation id was reused with a different token.",
                status_code=409,
            )
        known_execution = self._records.get(request.execution_id)
        known_by_idempotency = self._idempotency_index.get(request.idempotency_key)
        if known_by_idempotency is not None and known_by_idempotency != request.execution_id:
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_IDEMPOTENCY_CONFLICT",
                "The geometry idempotency key belongs to a different execution.",
                status_code=409,
            )
        if known_execution is not None:
            if (
                known_execution.idempotency_key != request.idempotency_key
                or known_execution.fingerprint != fingerprint
            ):
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_EXECUTION_ID_CONFLICT",
                    "The geometry execution id was reused with different input.",
                    status_code=409,
                )
            if known_execution.status == "running":
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_EXECUTION_IN_PROGRESS",
                    "The geometry execution is already running.",
                    status_code=409,
                    recoverable=True,
                )
            if known_execution.result is not None:
                return known_execution.result
            if known_execution.error is not None:
                raise known_execution.error
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_EXECUTION_TERMINAL_INVALID",
                "The geometry execution reached an invalid terminal state.",
                status_code=500,
            )

        if self._tombstone_matches(request.cancellation_id, request.cancellation_token):
            error = RestrictedGeometryBoundaryError(
                "GEOMETRY_EXECUTION_CANCELLED",
                "The geometry execution was cancelled before it started.",
                status_code=409,
                recoverable=True,
            )
            record = _ExecutionRecord(
                execution_id=request.execution_id,
                idempotency_key=request.idempotency_key,
                fingerprint=fingerprint,
                cancellation_id=request.cancellation_id,
                cancellation_token=request.cancellation_token,
                status="cancelled",
                created_monotonic=time.monotonic(),
                error=error,
            )
            self._records[request.execution_id] = record
            self._idempotency_index[request.idempotency_key] = request.execution_id
            raise error

        active_count = sum(
            record.status == "running" for record in self._records.values()
        )
        if active_count >= MAX_RESTRICTED_GEOMETRY_ACTIVE_EXECUTIONS:
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_EXECUTOR_BACKPRESSURE",
                "The restricted geometry executor is at its bounded concurrency limit.",
                status_code=503,
                recoverable=True,
            )
        record = _ExecutionRecord(
            execution_id=request.execution_id,
            idempotency_key=request.idempotency_key,
            fingerprint=fingerprint,
            cancellation_id=request.cancellation_id,
            cancellation_token=request.cancellation_token,
            status="running",
            created_monotonic=time.monotonic(),
        )
        self._records[request.execution_id] = record
        self._idempotency_index[request.idempotency_key] = request.execution_id
        return None

    def _run_worker(
        self,
        request: RestrictedGeometryExecutionRequest,
        record: _ExecutionRecord,
        worker_payload: dict[str, Any],
    ) -> RestrictedGeometryExecutionResult:
        parent_pipe, child_pipe = self._context.Pipe(duplex=False)
        cancel_event = self._context.Event()
        process = self._context.Process(
            target=self._worker_target,
            args=(child_pipe, cancel_event, worker_payload, self._resource_root),
            name="forgecad-restricted-geometry",
        )
        process.daemon = False
        with self._lock:
            record.cancel_event = cancel_event
            record.process = process
        try:
            process.start()
        except Exception:
            child_pipe.close()
            parent_pipe.close()
            error = RestrictedGeometryBoundaryError(
                "GEOMETRY_EXECUTOR_START_FAILED",
                "The restricted geometry worker could not start.",
                status_code=503,
                recoverable=True,
            )
            self._finish_error(record, error)
            raise error from None
        child_pipe.close()
        deadline = time.monotonic() + request.timeout_ms / 1000.0
        message: dict[str, Any] | None = None
        terminal_error: RestrictedGeometryBoundaryError | None = None
        try:
            while True:
                with self._lock:
                    cancelled = self._tombstone_matches(
                        request.cancellation_id,
                        request.cancellation_token,
                    )
                if cancelled:
                    cancel_event.set()
                    terminal_error = RestrictedGeometryBoundaryError(
                        "GEOMETRY_EXECUTION_CANCELLED",
                        "The geometry execution was cancelled.",
                        status_code=409,
                        recoverable=True,
                    )
                    break
                if time.monotonic() >= deadline:
                    cancel_event.set()
                    terminal_error = RestrictedGeometryBoundaryError(
                        "GEOMETRY_EXECUTION_TIMEOUT",
                        "The geometry execution exceeded its bounded deadline.",
                        status_code=504,
                        recoverable=True,
                    )
                    break
                if parent_pipe.poll(0.01):
                    try:
                        candidate = parent_pipe.recv()
                    except EOFError:
                        candidate = None
                    if isinstance(candidate, dict):
                        message = candidate
                    break
                if process.exitcode is not None:
                    break

            if terminal_error is not None:
                _stop_process(process, cancel_event)
            else:
                process.join(timeout=0.25)
                if process.is_alive():
                    _stop_process(process, cancel_event)

            # Cancellation may race the final pipe read.  The tombstone is
            # authoritative and is checked before any result/artifact is made
            # visible, so late bytes can never be promoted.
            with self._lock:
                if self._tombstone_matches(
                    request.cancellation_id,
                    request.cancellation_token,
                ):
                    terminal_error = RestrictedGeometryBoundaryError(
                        "GEOMETRY_EXECUTION_CANCELLED",
                        "The late geometry result was discarded after cancellation.",
                        status_code=409,
                        recoverable=True,
                    )

            if terminal_error is not None:
                self._finish_error(record, terminal_error)
                raise terminal_error
            if message is None:
                error = RestrictedGeometryBoundaryError(
                    "GEOMETRY_EXECUTOR_CRASHED",
                    "The restricted geometry worker exited without a result.",
                    status_code=503,
                    recoverable=True,
                )
                self._finish_error(record, error)
                raise error
            if message.get("ok") is not True:
                error_payload = message.get("error")
                error = RestrictedGeometryBoundaryError(
                    str(error_payload.get("code", "GEOMETRY_EXECUTION_FAILED"))
                    if isinstance(error_payload, dict)
                    else "GEOMETRY_EXECUTION_FAILED",
                    "The restricted geometry worker rejected the bounded input.",
                    status_code=422,
                )
                self._finish_error(record, error)
                raise error

            payload = message.get("result")
            if not isinstance(payload, dict):
                error = RestrictedGeometryBoundaryError(
                    "GEOMETRY_EXECUTOR_RESULT_INVALID",
                    "The restricted geometry worker returned an invalid result.",
                    status_code=503,
                )
                self._finish_error(record, error)
                raise error
            # Result promotion and the final cancellation check form one
            # linearized state transition.  A cancellation already accepted
            # by the executor wins; a cancellation arriving after this lock
            # observes a completed execution and cannot revoke visible bytes.
            with self._lock:
                if self._tombstone_matches(
                    request.cancellation_id,
                    request.cancellation_token,
                ):
                    error = RestrictedGeometryBoundaryError(
                        "GEOMETRY_EXECUTION_CANCELLED",
                        "The late geometry result was discarded after cancellation.",
                        status_code=409,
                        recoverable=True,
                    )
                    self._finish_error(record, error)
                    raise error
                try:
                    result = self._promote_result(request, payload)
                except RestrictedGeometryBoundaryError as exc:
                    self._finish_error(record, exc)
                    raise
                except Exception:
                    error = RestrictedGeometryBoundaryError(
                        "GEOMETRY_EXECUTOR_RESULT_INVALID",
                        "The restricted geometry worker returned an invalid bounded result.",
                        status_code=503,
                    )
                    self._finish_error(record, error)
                    raise error from None
                record.status = "completed"
                record.result = result
                record.retained_result_bytes = _retained_result_size(result)
                record.process = None
                record.cancel_event = None
                self._records.move_to_end(record.execution_id)
                self._prune_locked()
            return result
        finally:
            parent_pipe.close()
            if process.is_alive():
                _stop_process(process, cancel_event)

    def _promote_result(
        self,
        request: RestrictedGeometryExecutionRequest,
        payload: dict[str, Any],
    ) -> RestrictedGeometryExecutionResult:
        if request.action == "compile_readback":
            glb = payload.pop("glb_bytes", None)
            readback = payload.get("readback")
            if not isinstance(glb, bytes) or not isinstance(readback, dict):
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_EXECUTOR_RESULT_INVALID",
                    "The compile result did not contain bounded GLB/readback bytes.",
                    status_code=503,
                )
            if len(glb) > 64 * 1024 * 1024:
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_EXECUTOR_RESULT_TOO_LARGE",
                    "The compiled GLB exceeded the restricted result budget.",
                    status_code=413,
                )
            from forgecad_agent.application.geometry_models import (
                GeometryCompileReadback,
            )

            validated_readback = GeometryCompileReadback.model_validate(readback)
            readback = validated_readback.model_dump(mode="json")
            # The worker proves what it actually compiled using its local
            # deterministic JSON encoder.  A Rust-origin request may then
            # replace only the identity field with the already validated
            # Rust-owned seal; geometry bytes and all readback facts remain
            # independently checked below.
            worker_shape_program_sha256 = _canonical_json_sha256(
                request.shape_program
            )
            if readback.get("shape_program_sha256") != worker_shape_program_sha256:
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_READBACK_IDENTITY_MISMATCH",
                    "The compiler did not read back the requested ShapeProgram.",
                    status_code=503,
                )
            expected_shape_program_sha256 = worker_shape_program_sha256
            if request.shape_program_canonical_json is not None:
                try:
                    _validate_shape_program_seal(
                        request.shape_program,
                        request.shape_program_canonical_json,
                        request.shape_program_sha256 or "",
                    )
                except ValueError as error:
                    raise RestrictedGeometryBoundaryError(
                        "GEOMETRY_EXECUTOR_REQUEST_IDENTITY_MISMATCH",
                        "The Rust-owned ShapeProgram seal was invalid.",
                        status_code=400,
                    ) from error
                expected_shape_program_sha256 = request.shape_program_sha256 or ""
                readback["shape_program_sha256"] = expected_shape_program_sha256
            surface_provenance = readback.get("surface_provenance", [])
            readback["surface_provenance_present"] = bool(surface_provenance)
            readback["closed_manifold"] = bool(surface_provenance) and all(
                item.get("closed") is True
                and item.get("boundary_edge_count") == 0
                and item.get("non_manifold_edge_count") == 0
                and item.get("degenerate_triangle_count") == 0
                for item in surface_provenance
                if isinstance(item, dict)
            ) and all(isinstance(item, dict) for item in surface_provenance)
            glb_sha256 = hashlib.sha256(glb).hexdigest()
            if (
                readback.get("glb_sha256") != glb_sha256
                or readback.get("glb_byte_size") != len(glb)
                or readback.get("shape_program_sha256")
                != expected_shape_program_sha256
            ):
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_READBACK_IDENTITY_MISMATCH",
                    "The GLB bytes did not match their compile readback identity.",
                    status_code=503,
                )
            profile = readback.get("artifact_profile")
            if not isinstance(profile, dict):
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_READBACK_IDENTITY_MISMATCH",
                    "The compile readback omitted its artifact profile.",
                    status_code=503,
                )
            if profile.get("artifact_profile_id") != request.artifact_profile_id:
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_READBACK_IDENTITY_MISMATCH",
                    "The compile readback did not match the requested artifact profile.",
                    status_code=503,
                )
            handle = "geomart_" + secrets.token_urlsafe(32)
            now = time.monotonic()
            artifact = _ArtifactRecord(
                handle=handle,
                artifact_profile_id=profile["artifact_profile_id"],
                artifact_profile_sha256=profile["profile_sha256"],
                shape_program_sha256=readback["shape_program_sha256"],
                glb_sha256=glb_sha256,
                glb_bytes=glb,
                triangle_count=readback["triangle_count"],
                bounds_mm=list(readback["bounds_mm"]),
                created_monotonic=now,
                expires_monotonic=now + self._artifact_ttl_seconds,
            )
            result = RestrictedGeometryExecutionResult(
                execution_id=request.execution_id,
                action="compile_readback",
                artifact_handle=handle,
                artifact_profile_id=artifact.artifact_profile_id,
                artifact_profile_sha256=artifact.artifact_profile_sha256,
                shape_program_sha256=artifact.shape_program_sha256,
                glb_sha256=artifact.glb_sha256,
                glb_byte_size=len(glb),
                triangle_count=artifact.triangle_count,
                bounds_mm=artifact.bounds_mm,
                readback=readback,
                glb_base64=base64.b64encode(glb).decode("ascii"),
            )
            self._store_artifact_locked(artifact)
            return result

        artifact = self._artifact_for_render(request)
        (
            render_views,
            render_hashes,
            exploded_part_ids,
            exploded_unavailable_reason,
        ) = _validate_render_result_payload(
            payload,
            request.render or RestrictedGeometryRenderOptions(),
        )
        return RestrictedGeometryExecutionResult(
            execution_id=request.execution_id,
            action="render",
            artifact_handle=artifact.handle,
            artifact_profile_id=artifact.artifact_profile_id,
            artifact_profile_sha256=artifact.artifact_profile_sha256,
            shape_program_sha256=artifact.shape_program_sha256,
            glb_sha256=artifact.glb_sha256,
            glb_byte_size=len(artifact.glb_bytes),
            triangle_count=artifact.triangle_count,
            bounds_mm=artifact.bounds_mm,
            render_views=render_views,
            render_view_sha256=render_hashes,
            renderer_id="forgecad-agent-software-raster@1",
            exploded_part_ids=exploded_part_ids,
            exploded_unavailable_reason=exploded_unavailable_reason,
        )

    def _artifact_for_render(
        self,
        request: RestrictedGeometryExecutionRequest,
    ) -> _ArtifactRecord:
        self._prune_artifacts_locked()
        artifact = self._artifacts.get(request.artifact_handle or "")
        if artifact is None:
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_ARTIFACT_HANDLE_UNAVAILABLE",
                "The short-lived geometry artifact handle is unavailable; recompile the same IR.",
                status_code=409,
                recoverable=True,
            )
        if not secrets.compare_digest(
            artifact.shape_program_sha256,
            request.shape_program_sha256 or "",
        ):
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_ARTIFACT_IDENTITY_MISMATCH",
                "The geometry artifact handle did not match the expected ShapeProgram hash.",
                status_code=409,
            )
        artifact.expires_monotonic = time.monotonic() + self._artifact_ttl_seconds
        self._artifacts.move_to_end(artifact.handle)
        return artifact

    def _finish_error(
        self,
        record: _ExecutionRecord,
        error: RestrictedGeometryBoundaryError,
    ) -> None:
        with self._lock:
            if error.code == "GEOMETRY_EXECUTION_CANCELLED":
                record.status = "cancelled"
            elif error.code == "GEOMETRY_EXECUTION_TIMEOUT":
                record.status = "timed_out"
            else:
                record.status = "failed"
            record.error = error
            record.process = None
            record.cancel_event = None
            self._records.move_to_end(record.execution_id)
            self._prune_locked()

    def _tombstone_matches(self, cancellation_id: str, token: str) -> bool:
        known = self._cancellation_tombstones.get(cancellation_id)
        return known is not None and secrets.compare_digest(known, token)

    def _store_artifact_locked(self, artifact: _ArtifactRecord) -> None:
        self._prune_artifacts_locked()
        self._artifacts[artifact.handle] = artifact
        self._artifacts.move_to_end(artifact.handle)
        while (
            len(self._artifacts) > MAX_RESTRICTED_GEOMETRY_ARTIFACTS
            or sum(len(item.glb_bytes) for item in self._artifacts.values())
            > MAX_RESTRICTED_GEOMETRY_ARTIFACT_BYTES
        ):
            self._artifacts.popitem(last=False)

    def _prune_artifacts_locked(self) -> None:
        now = time.monotonic()
        expired = [
            handle
            for handle, artifact in self._artifacts.items()
            if artifact.expires_monotonic <= now
        ]
        for handle in expired:
            self._artifacts.pop(handle, None)

    def _prune_locked(self) -> None:
        self._prune_artifacts_locked()
        terminal_ids = [
            execution_id
            for execution_id, record in self._records.items()
            if record.status != "running"
        ]
        retained_result_bytes = sum(
            self._records[execution_id].retained_result_bytes
            for execution_id in terminal_ids
        )
        while terminal_ids and (
            len(terminal_ids) > MAX_RESTRICTED_GEOMETRY_TERMINAL_RECORDS
            or retained_result_bytes
            > MAX_RESTRICTED_GEOMETRY_TERMINAL_RESULT_BYTES
        ):
            execution_id = terminal_ids.pop(0)
            record = self._records.pop(execution_id)
            retained_result_bytes -= record.retained_result_bytes
            self._idempotency_index.pop(record.idempotency_key, None)
        self._prune_cancellation_tombstones_locked()

    def _prune_cancellation_tombstones_locked(self) -> None:
        protected = {
            record.cancellation_id
            for record in self._records.values()
            if record.status == "running"
        }
        while (
            len(self._cancellation_tombstones)
            > MAX_RESTRICTED_GEOMETRY_CANCELLATION_TOMBSTONES
        ):
            removable = next(
                (
                    cancellation_id
                    for cancellation_id in self._cancellation_tombstones
                    if cancellation_id not in protected
                ),
                None,
            )
            if removable is None:
                break
            self._cancellation_tombstones.pop(removable, None)


def _validate_restricted_geometry_companions(
    shape_program: Mapping[str, Any],
    *,
    profile_sketch: Mapping[str, Any] | None,
    section_set: Mapping[str, Any] | None,
) -> tuple[Optional[dict[str, Any]], Optional[dict[str, Any]]]:
    """Validate optional profile contracts and bind them to canonical IR inputs.

    Companions never add geometry or execution authority.  They are accepted
    only when the already-expanded ShapeProgram contains the exact canonical
    payload and digest, making them a read-only Rust/Python contract witness.
    """

    from forgecad_agent.application.profile_contracts import (
        canonical_profile_payload,
    )

    profile_inputs = shape_program.get("profile_inputs", [])
    if not isinstance(profile_inputs, list):
        profile_inputs = []

    def canonical_bound_companion(
        value: Mapping[str, Any] | None,
        *,
        expected_kind: Literal["profile_sketch", "profile_section_set"],
    ) -> Optional[dict[str, Any]]:
        if value is None:
            return None
        normalized, _canonical, digest = canonical_profile_payload(value)
        if not any(
            isinstance(item, Mapping)
            and item.get("input_kind") == expected_kind
            and item.get("contract_version") == normalized.get("schema_version")
            and item.get("input_sha256") == digest
            and item.get("canonical_payload") == normalized
            for item in profile_inputs
        ):
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_PROFILE_COMPANION_UNBOUND",
                "A profile companion did not match an expanded canonical ShapeProgram input.",
                status_code=422,
            )
        return normalized

    return (
        canonical_bound_companion(
            profile_sketch,
            expected_kind="profile_sketch",
        ),
        canonical_bound_companion(
            section_set,
            expected_kind="profile_section_set",
        ),
    )


def _validate_render_result_payload(
    payload: dict[str, Any],
    render: RestrictedGeometryRenderOptions,
) -> tuple[dict[str, str], dict[str, str], list[str], Optional[str]]:
    render_views = payload.get("render_views")
    render_hashes = payload.get("render_view_sha256")
    if not isinstance(render_views, dict) or not isinstance(render_hashes, dict):
        raise RestrictedGeometryBoundaryError(
            "GEOMETRY_EXECUTOR_RESULT_INVALID",
            "The render result did not contain bounded PNG views.",
            status_code=503,
        )
    if not all(
        isinstance(key, str) and isinstance(value, str)
        for key, value in render_views.items()
    ) or not all(
        isinstance(key, str) and isinstance(value, str)
        for key, value in render_hashes.items()
    ):
        raise RestrictedGeometryBoundaryError(
            "GEOMETRY_EXECUTOR_RESULT_INVALID",
            "The render result did not contain string PNG identities.",
            status_code=503,
        )
    base_view_ids = {"iso", "front", "side", "top"}
    actual_view_ids = set(render_views)
    if actual_view_ids not in (base_view_ids, base_view_ids | {"exploded_iso"}):
        raise RestrictedGeometryBoundaryError(
            "GEOMETRY_EXECUTOR_RESULT_INVALID",
            "The render result did not contain the exact restricted view set.",
            status_code=503,
        )
    if actual_view_ids != set(render_hashes):
        raise RestrictedGeometryBoundaryError(
            "GEOMETRY_EXECUTOR_RESULT_INVALID",
            "The rendered PNG bytes and hashes did not align.",
            status_code=503,
        )
    encoded_size = sum(len(value) for value in render_views.values())
    if encoded_size > MAX_RESTRICTED_GEOMETRY_RESULT_BYTES:
        raise RestrictedGeometryBoundaryError(
            "GEOMETRY_EXECUTOR_RESULT_TOO_LARGE",
            "The rendered PNG set exceeded the restricted result budget.",
            status_code=413,
        )
    for view_id, encoded in render_views.items():
        expected_hash = render_hashes[view_id]
        if not isinstance(expected_hash, str) or not re.fullmatch(
            _SHA256_PATTERN,
            expected_hash,
        ):
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_EXECUTOR_RESULT_INVALID",
                "The rendered PNG identity was invalid.",
                status_code=503,
            )
        try:
            png = base64.b64decode(encoded, validate=True)
        except (binascii.Error, ValueError, TypeError):
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_EXECUTOR_RESULT_INVALID",
                "The rendered PNG payload was invalid.",
                status_code=503,
            ) from None
        if (
            not png.startswith(b"\x89PNG\r\n\x1a\n")
            or not secrets.compare_digest(hashlib.sha256(png).hexdigest(), expected_hash)
        ):
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_EXECUTOR_RESULT_INVALID",
                "The rendered PNG bytes did not match their identity.",
                status_code=503,
            )

    raw_exploded_part_ids = payload.get("exploded_part_ids", [])
    if not isinstance(raw_exploded_part_ids, list) or not all(
        isinstance(value, str) for value in raw_exploded_part_ids
    ):
        raise RestrictedGeometryBoundaryError(
            "GEOMETRY_EXECUTOR_RESULT_INVALID",
            "The exploded-view part identity set was invalid.",
            status_code=503,
        )
    exploded_part_ids = list(raw_exploded_part_ids)
    requested_part_ids = [item.part_id for item in render.exploded_parts]
    exploded_rendered = "exploded_iso" in actual_view_ids
    if exploded_rendered:
        if exploded_part_ids != requested_part_ids or not exploded_part_ids:
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_EXECUTOR_RESULT_INVALID",
                "The exploded view did not match the requested stable parts.",
                status_code=503,
            )
    elif exploded_part_ids:
        raise RestrictedGeometryBoundaryError(
            "GEOMETRY_EXECUTOR_RESULT_INVALID",
            "The render result claimed exploded parts without an exploded view.",
            status_code=503,
        )

    exploded_unavailable_reason = payload.get("exploded_unavailable_reason")
    if exploded_unavailable_reason is not None and (
        not isinstance(exploded_unavailable_reason, str)
        or not 1 <= len(exploded_unavailable_reason) <= 500
    ):
        raise RestrictedGeometryBoundaryError(
            "GEOMETRY_EXECUTOR_RESULT_INVALID",
            "The exploded-view availability result was invalid.",
            status_code=503,
        )
    if exploded_rendered and exploded_unavailable_reason is not None:
        raise RestrictedGeometryBoundaryError(
            "GEOMETRY_EXECUTOR_RESULT_INVALID",
            "A rendered exploded view cannot also be unavailable.",
            status_code=503,
        )
    if not requested_part_ids and exploded_unavailable_reason is not None:
        raise RestrictedGeometryBoundaryError(
            "GEOMETRY_EXECUTOR_RESULT_INVALID",
            "An unrequested exploded view cannot report an unavailable reason.",
            status_code=503,
        )
    return (
        render_views,
        render_hashes,
        exploded_part_ids,
        exploded_unavailable_reason,
    )


def _retained_result_size(result: RestrictedGeometryExecutionResult) -> int:
    retained = 4096
    if result.glb_base64 is not None:
        retained += len(result.glb_base64)
    if result.render_views is not None:
        retained += sum(len(value) for value in result.render_views.values())
    return retained


def validate_restricted_geometry_environment(environment: Mapping[str, str]) -> None:
    """Reject product persistence and Provider authority at process creation."""

    for name, value in environment.items():
        if value and name not in _ALLOWED_EXECUTOR_ENVIRONMENT_NAMES:
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_EXECUTOR_ENVIRONMENT_FORBIDDEN",
                "The restricted geometry process cannot receive persistence or Provider configuration.",
                status_code=503,
            )


def validate_restricted_geometry_payload(value: Any) -> None:
    """Reject authority fields, machine locations and secret-like values recursively."""

    stack = [value]
    visited = 0
    while stack:
        current = stack.pop()
        visited += 1
        if visited > 100_000:
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_REQUEST_TOO_COMPLEX",
                "The restricted geometry request exceeded its JSON node budget.",
                status_code=413,
            )
        if isinstance(current, Mapping):
            for key, child in current.items():
                if not isinstance(key, str):
                    raise RestrictedGeometryBoundaryError(
                        "GEOMETRY_REQUEST_INVALID",
                        "Restricted geometry object keys must be strings.",
                    )
                if key.casefold() in _FORBIDDEN_CONTEXT_KEYS:
                    raise RestrictedGeometryBoundaryError(
                        "GEOMETRY_CONTEXT_FORBIDDEN",
                        "The restricted geometry request contained product or execution authority.",
                    )
                stack.append(child)
        elif isinstance(current, list):
            stack.extend(current)
        elif isinstance(current, str):
            if (
                current.startswith("/")
                or current.startswith("~/")
                or current.startswith("file://")
                or current.startswith("http://")
                or current.startswith("https://")
                or (
                    len(current) >= 3
                    and current[1] == ":"
                    and current[2] in {"/", "\\"}
                )
            ):
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_MACHINE_LOCATION_FORBIDDEN",
                    "The restricted geometry request cannot contain machine or network locations.",
                )
            if any(current.startswith(prefix) for prefix in _SECRET_VALUE_PREFIXES):
                raise RestrictedGeometryBoundaryError(
                    "GEOMETRY_PROVIDER_SECRET_FORBIDDEN",
                    "The restricted geometry request cannot contain Provider secret material.",
                )
        elif isinstance(current, float) and not math.isfinite(current):
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_REQUEST_INVALID",
                "The restricted geometry request must contain finite JSON values.",
            )
        elif current is None or isinstance(current, (bool, int, float)):
            continue
        else:
            raise RestrictedGeometryBoundaryError(
                "GEOMETRY_REQUEST_INVALID",
                "The restricted geometry request must contain finite JSON values.",
            )


def _restricted_geometry_worker_entry(
    pipe: Any,
    cancel_event: Any,
    payload: dict[str, Any],
    resource_root: Optional[str],
) -> None:
    """Disposable child entry; clear inherited authority before geometry imports."""

    try:
        sanitize_restricted_geometry_child_environment(resource_root)
        result = _execute_worker_payload(payload, cancel_event.is_set)
        pipe.send({"ok": True, "result": result})
    except BaseException as exc:  # child returns only a stable class, never values
        pipe.send(
            {
                "ok": False,
                "error": {
                    "code": _stable_geometry_error_code(exc),
                },
            }
        )
    finally:
        pipe.close()


def sanitize_restricted_geometry_child_environment(
    resource_root: Optional[str],
) -> None:
    """Retain only the audited read-only bundle location in a worker child."""

    os.environ.clear()
    if resource_root:
        os.environ["FORGECAD_RUNTIME_RESOURCE_ROOT"] = resource_root


def _execute_worker_payload(
    payload: dict[str, Any],
    cancel_check: Callable[[], bool],
) -> dict[str, Any]:
    if cancel_check():
        raise InterruptedError("cancelled")
    action = payload.get("action")
    if action == "compile_readback":
        from forgecad_agent.application.geometry_worker import compile_shape_program

        surface_layer_lowering = payload.get("surface_layer_lowering")
        if surface_layer_lowering is not None:
            _compile_retained_surface_layer_pbr(
                surface_layer_lowering,
                artifact_profile_id=payload["artifact_profile_id"],
                surface_adornment_programs=payload.get("surface_adornment_programs", ()),
            )

        _validate_restricted_geometry_companions(
            payload["shape_program"],
            profile_sketch=payload.get("profile_sketch"),
            section_set=payload.get("section_set"),
        )
        compiled = compile_shape_program(
            payload["shape_program"],
            artifact_profile_id=payload["artifact_profile_id"],
            surface_adornment_programs=payload.get("surface_adornment_programs", ()),
            surface_layer_lowering=surface_layer_lowering,
            cancel_check=cancel_check,
        )
        if cancel_check():
            raise InterruptedError("cancelled")
        return {
            "glb_bytes": compiled.glb_bytes,
            "readback": compiled.readback.model_dump(mode="json"),
        }
    if action == "render":
        from forgecad_agent.application.agent_rendering import (
            ExplodedPartOffset,
            render_agent_views,
        )
        from forgecad_agent.application.geometry_worker import (
            read_shape_program_glb_facts,
        )

        glb = payload["glb_bytes"]
        if hashlib.sha256(glb).hexdigest() != payload["glb_sha256"]:
            raise ValueError("compiled artifact hash mismatch")
        facts = read_shape_program_glb_facts(glb)
        if (
            facts.triangle_count != payload["triangle_count"]
            or facts.bounds_mm != payload["bounds_mm"]
            or facts.artifact_profile.get("artifact_profile_id")
            != payload["artifact_profile_id"]
            or facts.artifact_profile.get("profile_sha256")
            != payload["artifact_profile_sha256"]
        ):
            raise ValueError("compiled artifact readback identity mismatch")
        render = payload["render"]
        exploded_parts = tuple(
            ExplodedPartOffset(
                part_id=item["part_id"],
                offset=tuple(float(value) for value in item["translation_mm"]),
            )
            for item in render.get("exploded_parts", [])
        )
        rendered = render_agent_views(
            glb,
            width=int(render["width"]),
            height=int(render["height"]),
            exploded_parts=exploded_parts,
        )
        if cancel_check():
            raise InterruptedError("cancelled")
        view_bytes = {
            view_id: base64.b64encode(value).decode("ascii")
            for view_id, value in rendered.views.items()
        }
        return {
            "render_views": view_bytes,
            "render_view_sha256": {
                view_id: hashlib.sha256(value).hexdigest()
                for view_id, value in rendered.views.items()
            },
            "exploded_part_ids": list(rendered.exploded_part_ids),
            "exploded_unavailable_reason": rendered.exploded_unavailable_reason,
        }
    raise ValueError("unsupported restricted geometry action")


def _normalize_surface_layer_input(value: object) -> dict[str, object]:
    """Accept only the exact Rust-generated retained-surface DTO.

    The loopback capability authenticates the native caller; this additional
    hash check prevents a stale retained payload from being paired with an
    unrelated A005 list. It has no authoring, fallback or repair path.
    """

    if not isinstance(value, Mapping) or set(value) != _SURFACE_LAYER_INPUT_FIELDS:
        raise ValueError("surface layer input must contain exactly the sealed Rust DTO fields")
    if value.get("schema_version") != "RestrictedSurfaceLayerInput@1":
        raise ValueError("surface layer input schema version is invalid")
    from forgecad_agent.application.surface_layer_pbr import (
        normalize_surface_layer_lowering,
        surface_layer_lowering_sha256,
    )

    lowering = normalize_surface_layer_lowering(value.get("lowering"))
    lowering_sha256 = value.get("lowering_sha256")
    if (
        not isinstance(lowering_sha256, str)
        or len(lowering_sha256) != 64
        or any(character not in "0123456789abcdef" for character in lowering_sha256)
        or lowering_sha256 != surface_layer_lowering_sha256(lowering)
    ):
        raise ValueError("surface layer input seal does not match the exact lowering")
    return {
        "schema_version": "RestrictedSurfaceLayerInput@1",
        "lowering": lowering,
        "lowering_sha256": lowering_sha256,
    }


def _compile_retained_surface_layer_pbr(
    lowering: object,
    *,
    artifact_profile_id: object,
    surface_adornment_programs: object,
) -> None:
    """Compile all five retained PBR maps before the existing GLB compiler.

    This preflight makes texture generation fail before geometry compilation.
    The exact same sealed lowering is then handed to the GLB writer, which
    binds the retained five-map texture set to its existing Rust-selected
    material zone and proves that provenance during readback.
    """

    if artifact_profile_id not in {"interactive_preview", "production_concept"}:
        raise ValueError("surface layer artifact profile is invalid")
    from forgecad_agent.application.surface_layer_pbr import (
        normalize_surface_layer_lowering,
        surface_layer_visual_texture_png_bytes,
        surface_layer_visual_texture_set,
    )
    from forgecad_agent.application.visual_texture_sets import (
        normalize_surface_adornment_program,
    )

    normalized = normalize_surface_layer_lowering(lowering)
    supplied = [
        normalize_surface_adornment_program(item)
        for item in surface_adornment_programs
    ]
    if supplied != normalized["adornments"]:
        raise ValueError("surface layer retained compiler lost its Rust-lowered A005 binding")
    texture_set = surface_layer_visual_texture_set(
        normalized,
        artifact_profile_id=artifact_profile_id,
    )
    for texture_map in texture_set.maps:
        texture_bytes = surface_layer_visual_texture_png_bytes(
            normalized,
            artifact_profile_id=artifact_profile_id,
            texture_role=texture_map.texture_role,
        )
        if hashlib.sha256(texture_bytes).hexdigest() != texture_map.sha256:
            raise ValueError("surface layer retained compiler hash mismatch")


def _stable_geometry_error_code(error: BaseException) -> str:
    if isinstance(error, InterruptedError):
        return "GEOMETRY_EXECUTION_CANCELLED"
    code = getattr(error, "code", None)
    if isinstance(code, str) and 1 <= len(code) <= 120:
        return code
    name = type(error).__name__
    if name == "UnsupportedRuntimeOperationError":
        return "UNSUPPORTED_RUNTIME_OPERATION"
    if name == "ShapeProgramValidationError":
        return "SHAPE_PROGRAM_INVALID"
    if name == "ProfileContractValidationError":
        return "PROFILE_CONTRACT_INVALID"
    if name == "AgentRenderError":
        return "GEOMETRY_RENDER_FAILED"
    # Keep packaged worker failures diagnosable without exposing exception
    # messages, paths, request bodies or other runtime values. Exception
    # class names are bounded implementation identifiers and make frozen
    # worker/import failures distinguishable from malformed ShapePrograms.
    normalized_name = "".join(
        character if character.isalnum() else "_" for character in name.upper()
    ).strip("_")
    if normalized_name:
        return f"GEOMETRY_WORKER_{normalized_name}"[:120]
    return "GEOMETRY_EXECUTION_FAILED"


def _stop_process(process: Any, cancel_event: Any) -> None:
    cancel_event.set()
    process.join(timeout=0.2)
    if process.is_alive():
        process.terminate()
        process.join(timeout=0.5)
    if process.is_alive() and hasattr(process, "kill"):
        process.kill()
        process.join(timeout=0.5)


def _canonical_json_sha256(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _json_values_identical(left: Any, right: Any) -> bool:
    """Compare JSON trees without Python's bool/int or int/float coercion."""

    if type(left) is not type(right):
        return False
    if isinstance(left, dict):
        return left.keys() == right.keys() and all(
            _json_values_identical(left[key], right[key]) for key in left
        )
    if isinstance(left, list):
        return len(left) == len(right) and all(
            _json_values_identical(left_item, right_item)
            for left_item, right_item in zip(left, right)
        )
    if isinstance(left, float):
        if not math.isfinite(left) or not math.isfinite(right):
            return False
        if left != right:
            return False
        if left == 0.0:
            return math.copysign(1.0, left) == math.copysign(1.0, right)
        return True
    return left == right


def _validate_shape_program_seal(
    shape_program: Mapping[str, Any],
    canonical_json: str,
    expected_sha256: str,
) -> None:
    actual_sha256 = hashlib.sha256(canonical_json.encode("utf-8")).hexdigest()
    if actual_sha256 != expected_sha256:
        raise ValueError("Rust ShapeProgram canonical JSON hash does not match")
    try:
        sealed_shape_program = json.loads(canonical_json)
    except (json.JSONDecodeError, TypeError) as error:
        raise ValueError("Rust ShapeProgram canonical JSON is invalid") from error
    if not _json_values_identical(shape_program, sealed_shape_program):
        raise ValueError("Rust ShapeProgram seal does not match the request value")
