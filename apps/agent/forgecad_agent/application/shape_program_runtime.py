"""Single-source runtime operation contract for restricted ShapeProgram@1.

The JSON manifest is the authority.  JSON Schema consumes its operation names
through ``scripts/generate_schema_types.py``; Pydantic, semantic validation,
the geometry worker and quality entry points load this module instead of
maintaining private allow-lists.
"""

from __future__ import annotations

import json
from functools import lru_cache
from typing import Any, Literal, Mapping

from pydantic import BaseModel, ConfigDict, Field, model_validator

from forgecad_agent.runtime_paths import runtime_resource_root


MANIFEST_SCHEMA_VERSION = "ShapeProgramRuntimeManifest@1"
_MANIFEST_PATH = runtime_resource_root() / "packages" / "concept-spec" / "fixtures" / "shape-program-runtime-manifest.json"
_SHAPE_PROGRAM_SCHEMA_PATH = runtime_resource_root() / "packages" / "concept-spec" / "schemas" / "shape-program.schema.json"


class RuntimeOperationManifestError(ValueError):
    """The packaged declarative contract itself is corrupt or stale."""


class UnsupportedRuntimeOperationError(ValueError):
    """A ShapeProgram cannot enter any runtime path without a declared executor."""

    code = "UNSUPPORTED_RUNTIME_OPERATION"

    def __init__(self, *, operation_id: str, op: str, reason: str) -> None:
        self.operation_id = operation_id
        self.op = op
        self.reason = reason
        super().__init__(f"{self.code}: {operation_id or '<missing-operation-id>'} ({op or '<missing-op>'}): {reason}")


class RuntimeOperationSpec(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)

    op: str = Field(pattern=r"^[a-z][a-z0-9_]{1,63}$")
    executor: str = Field(pattern=r"^[a-z][a-z0-9_]{1,63}$")
    output_kind: str = Field(pattern=r"^(mesh|profile)$")
    legacy_quality_estimate: Literal["box", "cylinder", "none"]


class ShapeProgramRuntimeManifest(BaseModel):
    model_config = ConfigDict(extra="forbid", frozen=True)

    schema_version: str
    operations: tuple[RuntimeOperationSpec, ...] = Field(min_length=1, max_length=64)

    @model_validator(mode="after")
    def _validate_contract(self) -> "ShapeProgramRuntimeManifest":
        if self.schema_version != MANIFEST_SCHEMA_VERSION:
            raise ValueError("unsupported ShapeProgram runtime manifest version")
        names = [item.op for item in self.operations]
        executors = [item.executor for item in self.operations]
        if len(names) != len(set(names)):
            raise ValueError("runtime operation names must be unique")
        if len(executors) != len(set(executors)):
            raise ValueError("runtime executor names must be unique")
        return self


@lru_cache(maxsize=1)
def runtime_manifest() -> ShapeProgramRuntimeManifest:
    try:
        raw = json.loads(_MANIFEST_PATH.read_text(encoding="utf-8"))
        return ShapeProgramRuntimeManifest.model_validate(raw)
    except (OSError, ValueError) as exc:
        raise RuntimeOperationManifestError(f"invalid ShapeProgram runtime manifest: {exc}") from exc


def runtime_operation_specs() -> dict[str, RuntimeOperationSpec]:
    return {item.op: item for item in runtime_manifest().operations}


def runtime_operation_names() -> tuple[str, ...]:
    return tuple(item.op for item in runtime_manifest().operations)


def runtime_executor_ids() -> frozenset[str]:
    return frozenset(item.executor for item in runtime_manifest().operations)


def assert_schema_consumes_runtime_manifest() -> None:
    """Fail closed when the generated JSON Schema no longer reflects manifest truth."""

    try:
        schema = json.loads(_SHAPE_PROGRAM_SCHEMA_PATH.read_text(encoding="utf-8"))
        declared = schema["properties"]["operations"]["items"]["properties"]["op"]["enum"]
    except (OSError, KeyError, TypeError, ValueError) as exc:
        raise RuntimeOperationManifestError(f"ShapeProgram schema cannot consume runtime manifest: {exc}") from exc
    if tuple(declared) != runtime_operation_names():
        raise RuntimeOperationManifestError(
            "ShapeProgram schema operation enum is stale; run contracts:types:generate"
        )


def assert_declared_runtime_operations(program: Mapping[str, Any]) -> None:
    """Reject unknown operations before normal Schema or semantic validation."""

    operations = program.get("operations")
    if not isinstance(operations, list):
        return
    declared = runtime_operation_specs()
    for index, operation in enumerate(operations):
        if not isinstance(operation, Mapping):
            continue
        operation_id = str(operation.get("operation_id", ""))
        op = operation.get("op")
        if not isinstance(op, str) or op not in declared:
            label = op if isinstance(op, str) else "<invalid-op>"
            raise UnsupportedRuntimeOperationError(
                operation_id=operation_id or f"operations[{index}]",
                op=label,
                reason="operation is not declared by ShapeProgramRuntimeManifest@1",
            )


def assert_worker_executor_coverage(executor_ids: frozenset[str]) -> None:
    """Ensure a packaged worker has an executor for every declared operation."""

    missing = sorted(runtime_executor_ids() - executor_ids)
    if missing:
        raise UnsupportedRuntimeOperationError(
            operation_id="runtime_manifest",
            op=missing[0],
            reason="declared runtime executor is unavailable in this worker",
        )
