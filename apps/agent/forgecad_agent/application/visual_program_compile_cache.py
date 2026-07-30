"""Bounded ephemeral compile cache for hash-sealed ShapeProgram artifacts.

This cache is derived acceleration only. Rust remains the source/session owner;
the cache never persists product state and every hit re-verifies retained GLB
bytes plus ShapeProgram lineage. Artifact-handle liveness remains the executor's
responsibility and is proven by an actual render in the VP204 Gate.
"""

from __future__ import annotations

import base64
import hashlib
import json
from collections import OrderedDict
from dataclasses import dataclass
from typing import Any

from forgecad_agent.application.restricted_geometry_executor import (
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutionResult,
    RestrictedGeometryExecutor,
)


@dataclass(frozen=True)
class VisualProgramCompileCacheEvidence:
    cache_key_sha256: str
    shape_program_sha256: str
    glb_sha256: str
    hit: bool
    entry_count: int
    retained_bytes: int


class VisualProgramCompileCache:
    def __init__(self, executor: RestrictedGeometryExecutor, *, max_entries: int = 8, max_bytes: int = 64 * 1024 * 1024) -> None:
        if not 1 <= max_entries <= 32 or not 1_000_000 <= max_bytes <= 256 * 1024 * 1024:
            raise ValueError("VP204_COMPILE_CACHE_BOUNDS_INVALID")
        self._executor = executor
        self._max_entries = max_entries
        self._max_bytes = max_bytes
        self._entries: OrderedDict[str, RestrictedGeometryExecutionResult] = OrderedDict()
        self._retained_bytes = 0

    @property
    def entry_count(self) -> int:
        return len(self._entries)

    @property
    def retained_bytes(self) -> int:
        return self._retained_bytes

    @staticmethod
    def canonical_shape_program(shape_program: dict[str, Any]) -> tuple[str, str]:
        canonical = json.dumps(shape_program, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False)
        return canonical, hashlib.sha256(canonical.encode("utf-8")).hexdigest()

    @staticmethod
    def cache_key(shape_program_sha256: str, artifact_profile_id: str) -> str:
        payload = json.dumps(
            {
                "artifact_profile_id": artifact_profile_id,
                "compiler_boundary": "forgecad.restricted-geometry/1",
                "shape_program_sha256": shape_program_sha256,
            },
            sort_keys=True,
            separators=(",", ":"),
        )
        return hashlib.sha256(payload.encode("utf-8")).hexdigest()

    def compile(
        self,
        *,
        execution_id: str,
        idempotency_key: str,
        cancellation_id: str,
        cancellation_token: str,
        shape_program: dict[str, Any],
        artifact_profile_id: str = "interactive_preview",
        timeout_ms: int = 120_000,
    ) -> tuple[RestrictedGeometryExecutionResult, VisualProgramCompileCacheEvidence]:
        canonical, shape_sha256 = self.canonical_shape_program(shape_program)
        key = self.cache_key(shape_sha256, artifact_profile_id)
        cached = self._entries.get(key)
        if cached is not None:
            self._verify(cached, shape_sha256)
            self._entries.move_to_end(key)
            return cached.model_copy(deep=True), self._evidence(key, cached, True)

        request = RestrictedGeometryExecutionRequest.model_validate(
            {
                "schema_version": "RestrictedGeometryExecutionRequest@1",
                "protocol_version": "forgecad.restricted-geometry/1",
                "execution_id": execution_id,
                "idempotency_key": idempotency_key,
                "cancellation_id": cancellation_id,
                "cancellation_token": cancellation_token,
                "action": "compile_readback",
                "timeout_ms": timeout_ms,
                "artifact_profile_id": artifact_profile_id,
                "shape_program": shape_program,
                "shape_program_canonical_json": canonical,
                "shape_program_sha256": shape_sha256,
            }
        )
        result = self._executor.execute(request)
        self._verify(result, shape_sha256)
        if result.glb_byte_size <= self._max_bytes:
            self._entries[key] = result.model_copy(deep=True)
            self._retained_bytes += result.glb_byte_size
            self._evict()
        return result, self._evidence(key, result, False)

    def _verify(self, result: RestrictedGeometryExecutionResult, shape_sha256: str) -> None:
        if result.action != "compile_readback" or result.shape_program_sha256 != shape_sha256 or result.glb_base64 is None:
            raise ValueError("VP204_COMPILE_CACHE_LINEAGE_INVALID")
        glb = base64.b64decode(result.glb_base64, validate=True)
        if len(glb) != result.glb_byte_size or hashlib.sha256(glb).hexdigest() != result.glb_sha256:
            raise ValueError("VP204_COMPILE_CACHE_ARTIFACT_CORRUPT")

    def _evict(self) -> None:
        while len(self._entries) > self._max_entries or self._retained_bytes > self._max_bytes:
            _key, result = self._entries.popitem(last=False)
            self._retained_bytes -= result.glb_byte_size

    def _evidence(self, key: str, result: RestrictedGeometryExecutionResult, hit: bool) -> VisualProgramCompileCacheEvidence:
        return VisualProgramCompileCacheEvidence(
            cache_key_sha256=key,
            shape_program_sha256=result.shape_program_sha256,
            glb_sha256=result.glb_sha256,
            hit=hit,
            entry_count=len(self._entries),
            retained_bytes=self._retained_bytes,
        )
