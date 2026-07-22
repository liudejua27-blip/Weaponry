"""Fail-closed Python compatibility ports for the K002 Rust Agent runtime.

These ports are intentionally not wired into the legacy FastAPI factory.  Rust
is the lifecycle decision maker.  Python may only execute a versioned,
idempotent persistence command through the injected sole writer, or one
code-owned candidate/read-only Product Tool against ephemeral run state.
"""

from __future__ import annotations

import copy
import os
import re
import threading
import time
from collections import OrderedDict
from collections.abc import Mapping
from dataclasses import dataclass, field
from typing import Any, Protocol

from jsonschema import Draft202012Validator
from pydantic import ValidationError

from .agent_action_loop import (
    AgentActionContext,
    AgentActionLoopError,
    ProductToolDefinition,
    ProductToolRegistry,
)
from .k002_port_contracts import (
    AppliedPersistenceOutcome,
    ArchiveThreadPersistenceOperation,
    AppendItemPersistenceOperation,
    CreateApprovalPersistenceOperation,
    CreateThreadPersistenceOperation,
    CreateTurnPersistenceOperation,
    LifecyclePersistenceCommand,
    LifecyclePersistenceResult,
    ProductToolExecutionRequest,
    ProductToolExecutionResult,
    ReplayItemsPersistenceOperation,
    ResolveApprovalPersistenceOperation,
    SetTurnTerminalPersistenceOperation,
    ValidatedProductToolPayload,
)
from .k002_port_security import (
    K002PortBoundaryError,
    canonical_json_sha256,
    ensure_isolated_environment,
    reject_machine_locations,
    validate_bounded_json,
)
from .product_tool_registry import forgecad_product_tool_registry


_PERSISTENCE_FORBIDDEN_KEYS = frozenset(
    {
        "provider_key",
        "api_key",
        "authorization",
        "bearer_token",
        "access_token",
        "client_secret",
        "provider_url",
        "base_url",
        "endpoint_url",
        "reasoning",
        "reasoning_content",
        "raw_reasoning",
        "hidden_reasoning",
        "chain_of_thought",
        "database_path",
        "sqlite_path",
        "object_store_path",
        "object_path",
        "filesystem_path",
        "snapshot_write_token",
        "asset_write_token",
    }
)
_PRODUCT_TOOL_FORBIDDEN_KEYS = _PERSISTENCE_FORBIDDEN_KEYS | frozenset(
    {
        "thread_id",
        "session",
        "session_id",
        "history",
        "messages",
        "provider",
        "provider_id",
        "provider_name",
        "model",
        "url",
        "file_path",
        "db_path",
        "repository_path",
        "snapshot_token",
        "asset_token",
        "agent_item",
        "item_write",
        "asset_write",
        "snapshot_write",
        "changeset_write",
        "version_write",
    }
)
_EXPECTED_OUTCOME_BY_OPERATION = {
    "load_thread": "thread_loaded",
    "list_threads": "threads_listed",
    "create_thread": "applied",
    "archive_thread": "applied",
    "create_turn": "applied",
    "append_item": "applied",
    "create_approval": "applied",
    "resolve_approval": "applied",
    "set_turn_terminal": "applied",
    "replay_items": "items_replayed",
}
_STABLE_ID = re.compile(r"^[A-Za-z0-9_.\-]{1,160}$")
_UNSPECIFIED = object()


class LifecyclePersistenceBackend(Protocol):
    """The one Python lifecycle writer retained until K003."""

    def execute(
        self,
        command: LifecyclePersistenceCommand,
    ) -> LifecyclePersistenceResult | Mapping[str, Any]:
        ...


class LifecyclePersistencePort:
    """Validate a sealed Rust lifecycle command before the sole Python writer."""

    def __init__(
        self,
        backend: LifecyclePersistenceBackend,
        *,
        environment: Mapping[str, str] | None = None,
    ) -> None:
        self._backend = backend
        self._environment = os.environ if environment is None else environment

    def execute(
        self,
        command: LifecyclePersistenceCommand | Mapping[str, Any],
    ) -> LifecyclePersistenceResult:
        ensure_isolated_environment(self._environment)
        parsed = _parse_persistence_command(command)
        validate_bounded_json(
            parsed.model_dump(mode="json", exclude_none=True),
            forbidden_keys=_PERSISTENCE_FORBIDDEN_KEYS,
            boundary_name="LifecyclePersistenceCommand@1",
        )

        try:
            raw_result = self._backend.execute(parsed)
        except K002PortBoundaryError:
            raise
        except Exception as exc:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_BACKEND_FAILED",
                "The sole Python lifecycle writer failed without changing ownership.",
                recoverable=True,
            ) from exc

        result = _parse_persistence_result(raw_result)
        validate_bounded_json(
            result.model_dump(mode="json", exclude_none=True),
            forbidden_keys=_PERSISTENCE_FORBIDDEN_KEYS,
            boundary_name="LifecyclePersistenceResult@1",
        )
        self._validate_result_binding(parsed, result)
        return result

    @staticmethod
    def _validate_result_binding(
        command: LifecyclePersistenceCommand,
        result: LifecyclePersistenceResult,
    ) -> None:
        if result.command_id != command.command_id:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_COMMAND_ID_DRIFT",
                "The lifecycle writer returned a result for another sealed command.",
            )
        expected_outcome = _EXPECTED_OUTCOME_BY_OPERATION[command.operation_name]
        if result.result.outcome != expected_outcome:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_OUTCOME_DRIFT",
                "The lifecycle writer returned an outcome incompatible with the command.",
            )
        if (
            command.mutates_persistence
            and command.expected_revision is not None
            and not result.replayed
            and result.revision == command.expected_revision
        ):
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_REVISION_NOT_ADVANCED",
                "A newly applied lifecycle mutation must advance its opaque revision.",
            )
        _validate_persistence_identity(command, result)


@dataclass
class _CachedProductToolResult:
    idempotency_key: str
    result: ProductToolExecutionResult


@dataclass
class _ExecutorRun:
    execution_id: str
    turn_id: str
    cancellation_id: str
    cancellation_token: str
    cancel_event: threading.Event = field(default_factory=threading.Event)
    generation: int = 0
    calls_started: int = 0
    state: dict[str, Any] = field(default_factory=dict)
    in_flight: set[str] = field(default_factory=set)
    cached: dict[str, _CachedProductToolResult] = field(default_factory=dict)
    idempotency_to_call: dict[str, str] = field(default_factory=dict)
    touched_at: float = field(default_factory=time.monotonic)


class ProductToolExecutorPort:
    """Execute only immutable-registry tools with no lifecycle/provider authority."""

    def __init__(
        self,
        registry: ProductToolRegistry | None = None,
        *,
        environment: Mapping[str, str] | None = None,
        max_active_runs: int = 64,
        max_tool_calls_per_run: int = 12,
        max_wall_seconds: float = 60.0,
        max_cancel_tombstones: int = 256,
    ) -> None:
        if not 1 <= max_active_runs <= 128:
            raise ValueError("max_active_runs must be between 1 and 128")
        if not 1 <= max_tool_calls_per_run <= 12:
            raise ValueError("max_tool_calls_per_run must be between 1 and 12")
        if not 0.1 <= max_wall_seconds <= 300:
            raise ValueError("max_wall_seconds must be between 0.1 and 300")
        if not 1 <= max_cancel_tombstones <= 1024:
            raise ValueError("max_cancel_tombstones must be between 1 and 1024")
        self._registry = registry or forgecad_product_tool_registry()
        self._environment = os.environ if environment is None else environment
        self._max_active_runs = max_active_runs
        self._max_tool_calls_per_run = max_tool_calls_per_run
        self._max_wall_seconds = max_wall_seconds
        self._max_cancel_tombstones = max_cancel_tombstones
        self._lock = threading.RLock()
        self._runs: OrderedDict[str, _ExecutorRun] = OrderedDict()
        self._run_by_cancellation_id: dict[str, str] = {}
        self._cancel_tombstones: OrderedDict[str, str] = OrderedDict()

    def execute(
        self,
        request: ProductToolExecutionRequest | Mapping[str, Any],
    ) -> ProductToolExecutionResult:
        ensure_isolated_environment(self._environment)
        parsed = _parse_product_tool_request(request)
        arguments = parsed.validated_arguments.value
        validate_bounded_json(
            arguments,
            forbidden_keys=_PRODUCT_TOOL_FORBIDDEN_KEYS,
            boundary_name="ProductToolExecutionRequest@1.validated_arguments",
        )
        reject_machine_locations(
            arguments,
            boundary_name="ProductToolExecutionRequest@1.validated_arguments",
        )
        tool = self._require_sealed_tool(parsed)
        self._require_idempotency_binding(parsed)

        with self._lock:
            run = self._require_run(parsed)
            cached = run.cached.get(parsed.call_id)
            if cached is not None:
                if cached.idempotency_key != parsed.idempotency_key:
                    raise K002PortBoundaryError(
                        "K002_PRODUCT_TOOL_CALL_ID_CONFLICT",
                        "A Product Tool call ID cannot be reused with another decision.",
                    )
                return cached.result.model_copy(deep=True)
            previous_call = run.idempotency_to_call.get(parsed.idempotency_key)
            if previous_call is not None and previous_call != parsed.call_id:
                raise K002PortBoundaryError(
                    "K002_PRODUCT_TOOL_IDEMPOTENCY_CONFLICT",
                    "A Product Tool idempotency key cannot identify two calls.",
                )
            if parsed.call_id in run.in_flight:
                raise K002PortBoundaryError(
                    "K002_PRODUCT_TOOL_CALL_IN_FLIGHT",
                    "The same Product Tool call is already in flight.",
                    recoverable=True,
                )
            if run.cancel_event.is_set():
                result = self._cancelled_result(parsed, 0)
                self._cache_result(run, parsed, result)
                return result.model_copy(deep=True)
            if run.calls_started >= self._max_tool_calls_per_run:
                raise K002PortBoundaryError(
                    "K002_PRODUCT_TOOL_CALL_LIMIT",
                    "The bounded Product Tool executor run exceeded its call limit.",
                )
            run.calls_started += 1
            run.in_flight.add(parsed.call_id)
            run.idempotency_to_call[parsed.idempotency_key] = parsed.call_id
            run.touched_at = time.monotonic()
            generation = run.generation
            local_state = copy.deepcopy(run.state)

        started_at = time.monotonic()
        context = AgentActionContext(
            parent_turn_id=parsed.turn_id,
            state=local_state,
            cancel_event=run.cancel_event,
        )
        status = "completed"
        failure_category = None
        error_code = None
        message = None
        output: dict[str, Any] | None = None
        try:
            context.check_cancelled()
            handler_arguments = self._arguments_for_legacy_handler(tool, arguments)
            output = self._invoke_tool(tool, handler_arguments, context)
            output = self._output_from_legacy_handler(tool, output)
            self._validate_tool_output(tool, output)
        except AgentActionLoopError as exc:
            status = "cancelled" if exc.category == "cancelled" else "failed"
            failure_category = "execution" if exc.category == "provider" else exc.category
            error_code = exc.code if _STABLE_ID.fullmatch(exc.code) else "PRODUCT_TOOL_FAILED"
            message = _safe_failure_message(str(exc), "Product Tool execution failed.")
            output = None
        except K002PortBoundaryError:
            status = "failed"
            failure_category = "permission"
            error_code = "PRODUCT_TOOL_OUTPUT_BOUNDARY_REJECTED"
            message = "Product Tool output crossed the restricted executor boundary."
            output = None
        except Exception:
            status = "failed"
            failure_category = "execution"
            error_code = "PRODUCT_TOOL_EXECUTION_FAILED"
            message = "Product Tool execution failed inside the restricted Python executor."
            output = None

        duration_ms = max(0, int((time.monotonic() - started_at) * 1000))
        if status == "completed" and duration_ms > int(self._max_wall_seconds * 1000):
            status = "failed"
            failure_category = "timeout"
            error_code = "PRODUCT_TOOL_TIMEOUT"
            message = "Product Tool exceeded the restricted executor wall-time budget."
            output = None

        with self._lock:
            run.in_flight.discard(parsed.call_id)
            run.touched_at = time.monotonic()
            late_cancelled = run.cancel_event.is_set() or run.generation != generation
            if late_cancelled:
                result = self._cancelled_result(parsed, duration_ms)
            elif status == "completed" and output is not None:
                run.state = context.state
                result = ProductToolExecutionResult(
                    execution_id=parsed.execution_id,
                    turn_id=parsed.turn_id,
                    call_id=parsed.call_id,
                    tool_id=parsed.tool_id,
                    cancellation_id=parsed.cancellation_id,
                    status="completed",
                    validated_output=ValidatedProductToolPayload(
                        schema_id=f"{parsed.tool_id}:output",
                        schema_sha256=canonical_json_sha256(tool.output_schema),
                        value=output,
                    ),
                    duration_ms=duration_ms,
                    permanent_side_effects=0,
                )
            else:
                result = ProductToolExecutionResult(
                    execution_id=parsed.execution_id,
                    turn_id=parsed.turn_id,
                    call_id=parsed.call_id,
                    tool_id=parsed.tool_id,
                    cancellation_id=parsed.cancellation_id,
                    status=status,
                    failure_category=failure_category,
                    error_code=error_code,
                    message=message,
                    duration_ms=duration_ms,
                    permanent_side_effects=0,
                )
            self._cache_result(run, parsed, result)
            return result.model_copy(deep=True)

    def cancel(self, *, cancellation_id: str, cancellation_token: str) -> bool:
        """Cancel a run or retain a bounded tombstone for cancel-before-start."""

        ensure_isolated_environment(self._environment)
        if not _STABLE_ID.fullmatch(cancellation_id) or not _STABLE_ID.fullmatch(
            cancellation_token
        ):
            raise K002PortBoundaryError(
                "K002_CANCELLATION_ID_INVALID",
                "Cancellation identity must be one bounded opaque stable ID pair.",
            )
        with self._lock:
            execution_id = self._run_by_cancellation_id.get(cancellation_id)
            if execution_id is not None:
                run = self._runs[execution_id]
                if run.cancellation_token != cancellation_token:
                    raise K002PortBoundaryError(
                        "K002_CANCELLATION_TOKEN_MISMATCH",
                        "Cancellation token does not own this executor run.",
                    )
                if not run.cancel_event.is_set():
                    run.generation += 1
                    run.cancel_event.set()
                run.touched_at = time.monotonic()
                return True

            prior_token = self._cancel_tombstones.get(cancellation_id)
            if prior_token is not None and prior_token != cancellation_token:
                raise K002PortBoundaryError(
                    "K002_CANCELLATION_TOKEN_MISMATCH",
                    "Cancellation token conflicts with an earlier cancellation.",
                )
            self._cancel_tombstones[cancellation_id] = cancellation_token
            self._cancel_tombstones.move_to_end(cancellation_id)
            while len(self._cancel_tombstones) > self._max_cancel_tombstones:
                self._cancel_tombstones.popitem(last=False)
            return prior_token is not None

    def _require_sealed_tool(
        self,
        request: ProductToolExecutionRequest,
    ) -> ProductToolDefinition:
        try:
            tool = self._registry.require(request.tool_name)
        except AgentActionLoopError as exc:
            raise K002PortBoundaryError(
                "K002_PRODUCT_TOOL_NOT_CODE_OWNED",
                "Product Tool is not present in the immutable code-owned registry.",
            ) from exc
        if tool.tool_id != request.tool_id:
            raise K002PortBoundaryError(
                "K002_PRODUCT_TOOL_IDENTITY_DRIFT",
                "Product Tool name and stable ID do not identify the same registry entry.",
            )
        if tool.approval_policy != request.approval_policy:
            raise K002PortBoundaryError(
                "K002_PRODUCT_TOOL_POLICY_DRIFT",
                "Rust approval policy does not match the immutable Product Tool registry.",
            )
        if tool.approval_policy == "user_confirmation_required":
            raise K002PortBoundaryError(
                "K002_PERMANENT_PRODUCT_TOOL_FORBIDDEN",
                "Permanent mutation tools cannot run in the K002 Python executor.",
            )
        boundary_schema = self._boundary_input_schema(tool)
        expected_hash = canonical_json_sha256(boundary_schema)
        if request.validated_arguments.schema_sha256 != expected_hash:
            raise K002PortBoundaryError(
                "K002_PRODUCT_TOOL_INPUT_SCHEMA_DRIFT",
                "Rust validation did not bind arguments to the shipped input Schema.",
            )
        errors = sorted(
            Draft202012Validator(boundary_schema).iter_errors(
                request.validated_arguments.value
            ),
            key=lambda item: list(item.path),
        )
        if errors:
            raise K002PortBoundaryError(
                "K002_PRODUCT_TOOL_INPUT_SCHEMA_INVALID",
                "Product Tool arguments failed the immutable registry Schema.",
            )
        return tool

    @staticmethod
    def _boundary_input_schema(tool: ProductToolDefinition) -> dict[str, Any]:
        schema = copy.deepcopy(tool.input_schema)
        if tool.tool_id != "forgecad.plan.complete_concept.v1":
            return schema
        plan_schema = schema.get("properties", {}).get("plan")
        if not isinstance(plan_schema, dict):
            raise K002PortBoundaryError(
                "K002_PRODUCT_TOOL_REGISTRY_INVALID",
                "The complete-concept Product Tool Schema is not structurally valid.",
            )
        properties = plan_schema.get("properties")
        required = plan_schema.get("required")
        if not isinstance(properties, dict) or not isinstance(required, list):
            raise K002PortBoundaryError(
                "K002_PRODUCT_TOOL_REGISTRY_INVALID",
                "The complete-concept Product Tool Schema cannot be authority-scrubbed.",
            )
        properties.pop("provider_id", None)
        properties.pop("model", None)
        plan_schema["required"] = [name for name in required if name != "provider_id"]
        return schema

    @staticmethod
    def _arguments_for_legacy_handler(
        tool: ProductToolDefinition,
        arguments: dict[str, Any],
    ) -> dict[str, Any]:
        prepared = copy.deepcopy(arguments)
        if tool.tool_id == "forgecad.plan.complete_concept.v1":
            plan = prepared.get("plan")
            if not isinstance(plan, dict):
                raise K002PortBoundaryError(
                    "K002_PRODUCT_TOOL_INPUT_SCHEMA_INVALID",
                    "The complete-concept Product Tool requires one plan object.",
                )
            plan["provider_id"] = "rust_app_server"
            plan["model"] = None
        return prepared

    @staticmethod
    def _output_from_legacy_handler(
        tool: ProductToolDefinition,
        output: dict[str, Any],
    ) -> dict[str, Any]:
        sanitized = copy.deepcopy(output)
        if tool.tool_id == "forgecad.plan.complete_concept.v1":
            plan = sanitized.get("plan")
            if isinstance(plan, dict):
                plan.pop("provider_id", None)
                plan.pop("model", None)
        return sanitized

    @staticmethod
    def _require_idempotency_binding(request: ProductToolExecutionRequest) -> None:
        expected = canonical_json_sha256(
            {
                "turn_id": request.turn_id,
                "call_id": request.call_id,
                "tool_id": request.tool_id,
                "arguments": request.validated_arguments.value,
            }
        )
        if request.idempotency_key != expected:
            raise K002PortBoundaryError(
                "K002_PRODUCT_TOOL_IDEMPOTENCY_DRIFT",
                "Product Tool idempotency key is not bound to the sealed Rust decision.",
            )

    def _require_run(self, request: ProductToolExecutionRequest) -> _ExecutorRun:
        run = self._runs.get(request.execution_id)
        if run is not None:
            if (
                run.turn_id != request.turn_id
                or run.cancellation_id != request.cancellation_id
                or run.cancellation_token != request.cancellation_token
            ):
                raise K002PortBoundaryError(
                    "K002_EXECUTOR_RUN_IDENTITY_DRIFT",
                    "Opaque executor run identity cannot be rebound to another Turn or cancellation.",
                )
            self._runs.move_to_end(request.execution_id)
            return run

        self._prune_runs_for_capacity()
        prior_execution = self._run_by_cancellation_id.get(request.cancellation_id)
        if prior_execution is not None:
            raise K002PortBoundaryError(
                "K002_CANCELLATION_ID_CONFLICT",
                "Cancellation ID is already bound to another executor run.",
            )
        run = _ExecutorRun(
            execution_id=request.execution_id,
            turn_id=request.turn_id,
            cancellation_id=request.cancellation_id,
            cancellation_token=request.cancellation_token,
        )
        tombstone = self._cancel_tombstones.get(request.cancellation_id)
        if tombstone is not None:
            if tombstone != request.cancellation_token:
                raise K002PortBoundaryError(
                    "K002_CANCELLATION_TOKEN_MISMATCH",
                    "Cancellation token conflicts with cancel-before-start state.",
                )
            run.generation += 1
            run.cancel_event.set()
        self._runs[request.execution_id] = run
        self._run_by_cancellation_id[request.cancellation_id] = request.execution_id
        return run

    def _prune_runs_for_capacity(self) -> None:
        while len(self._runs) >= self._max_active_runs:
            removable = next(
                (
                    execution_id
                    for execution_id, candidate in self._runs.items()
                    if not candidate.in_flight
                ),
                None,
            )
            if removable is None:
                raise K002PortBoundaryError(
                    "K002_EXECUTOR_BACKPRESSURE",
                    "All bounded Product Tool executor runs are active.",
                    recoverable=True,
                )
            removed = self._runs.pop(removable)
            self._run_by_cancellation_id.pop(removed.cancellation_id, None)

    @staticmethod
    def _cache_result(
        run: _ExecutorRun,
        request: ProductToolExecutionRequest,
        result: ProductToolExecutionResult,
    ) -> None:
        run.cached[request.call_id] = _CachedProductToolResult(
            idempotency_key=request.idempotency_key,
            result=result.model_copy(deep=True),
        )

    @staticmethod
    def _cancelled_result(
        request: ProductToolExecutionRequest,
        duration_ms: int,
    ) -> ProductToolExecutionResult:
        return ProductToolExecutionResult(
            execution_id=request.execution_id,
            turn_id=request.turn_id,
            call_id=request.call_id,
            tool_id=request.tool_id,
            cancellation_id=request.cancellation_id,
            status="cancelled",
            failure_category="cancelled",
            error_code="PRODUCT_TOOL_CANCELLED",
            message="Product Tool result was cancelled; any late result was discarded.",
            duration_ms=duration_ms,
            permanent_side_effects=0,
        )

    @staticmethod
    def _validate_tool_output(tool: ProductToolDefinition, output: Any) -> None:
        if not isinstance(output, dict):
            raise K002PortBoundaryError(
                "K002_PRODUCT_TOOL_OUTPUT_NOT_OBJECT",
                "Product Tool output must be a bounded JSON object.",
            )
        validate_bounded_json(
            output,
            forbidden_keys=_PRODUCT_TOOL_FORBIDDEN_KEYS,
            boundary_name="ProductToolExecutionResult@1.validated_output",
        )
        reject_machine_locations(
            output,
            boundary_name="ProductToolExecutionResult@1.validated_output",
        )
        errors = sorted(
            Draft202012Validator(tool.output_schema).iter_errors(output),
            key=lambda item: list(item.path),
        )
        if errors:
            raise K002PortBoundaryError(
                "K002_PRODUCT_TOOL_OUTPUT_SCHEMA_INVALID",
                "Product Tool output failed the immutable registry Schema.",
            )

    def _invoke_tool(
        self,
        tool: ProductToolDefinition,
        arguments: dict[str, Any],
        context: AgentActionContext,
    ) -> dict[str, Any]:
        return tool.handler(arguments, context)


def _parse_persistence_command(
    value: LifecyclePersistenceCommand | Mapping[str, Any],
) -> LifecyclePersistenceCommand:
    try:
        return LifecyclePersistenceCommand.model_validate(value)
    except ValidationError as exc:
        raise K002PortBoundaryError(
            "K002_PERSISTENCE_COMMAND_INVALID",
            "Lifecycle persistence command does not match LifecyclePersistenceCommand@1.",
        ) from exc


def _parse_persistence_result(
    value: LifecyclePersistenceResult | Mapping[str, Any],
) -> LifecyclePersistenceResult:
    try:
        return LifecyclePersistenceResult.model_validate(value)
    except ValidationError as exc:
        raise K002PortBoundaryError(
            "K002_PERSISTENCE_RESULT_INVALID",
            "Lifecycle writer result does not match LifecyclePersistenceResult@1.",
        ) from exc


def _parse_product_tool_request(
    value: ProductToolExecutionRequest | Mapping[str, Any],
) -> ProductToolExecutionRequest:
    try:
        return ProductToolExecutionRequest.model_validate(value)
    except ValidationError as exc:
        raise K002PortBoundaryError(
            "K002_PRODUCT_TOOL_REQUEST_INVALID",
            "Product Tool request does not match ProductToolExecutionRequest@1.",
        ) from exc


def _validate_persistence_identity(
    command: LifecyclePersistenceCommand,
    result: LifecyclePersistenceResult,
) -> None:
    operation = command.command
    outcome = result.result
    if isinstance(operation, CreateThreadPersistenceOperation):
        _require_applied_identity(outcome, thread_id=operation.thread.thread_id)
    elif isinstance(operation, ArchiveThreadPersistenceOperation):
        _require_applied_identity(outcome, thread_id=operation.thread.thread_id)
    elif isinstance(operation, CreateTurnPersistenceOperation):
        _require_applied_identity(
            outcome,
            thread_id=operation.thread_id,
            turn_id=operation.turn.turn_id,
        )
    elif isinstance(operation, AppendItemPersistenceOperation):
        _require_applied_identity(
            outcome,
            thread_id=operation.item.thread_id,
            turn_id=operation.item.turn_id,
            item_id=operation.item.item_id,
            sequence=operation.item.sequence,
        )
    elif isinstance(
        operation,
        (CreateApprovalPersistenceOperation, ResolveApprovalPersistenceOperation),
    ):
        _require_applied_identity(
            outcome,
            thread_id=operation.approval.thread_id,
            turn_id=operation.approval.turn_id,
            item_id=operation.approval.item_id,
            approval_id=operation.approval.approval_id,
            sequence=_UNSPECIFIED,
        )
    elif isinstance(operation, SetTurnTerminalPersistenceOperation):
        _require_applied_identity(
            outcome,
            thread_id=operation.turn.thread_id,
            turn_id=operation.turn.turn_id,
        )
    elif operation.operation == "load_thread":
        if outcome.thread is not None and outcome.thread.thread_id != operation.thread_id:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_IDENTITY_DRIFT",
                "Loaded lifecycle Thread identity does not match the sealed command.",
            )
    elif operation.operation == "list_threads" and operation.project_id is not None:
        if any(thread.project_id != operation.project_id for thread in outcome.threads):
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_IDENTITY_DRIFT",
                "Listed lifecycle Thread escaped the sealed project filter.",
            )
    elif isinstance(operation, ReplayItemsPersistenceOperation):
        if outcome.thread_id != operation.thread_id:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_IDENTITY_DRIFT",
                "Replayed lifecycle Item identity does not match the sealed command.",
            )


def _require_applied_identity(
    outcome: Any,
    *,
    thread_id: str,
    turn_id: str | None = None,
    item_id: str | None = None,
    approval_id: str | None = None,
    sequence: int | None | object = None,
) -> None:
    identity_drift = not isinstance(outcome, AppliedPersistenceOutcome)
    if not identity_drift:
        identity_drift = any(
            (
                outcome.thread_id != thread_id,
                outcome.turn_id != turn_id,
                outcome.item_id != item_id,
                outcome.approval_id != approval_id,
                sequence is not _UNSPECIFIED and outcome.sequence != sequence,
            )
        )
    if identity_drift:
        raise K002PortBoundaryError(
            "K002_PERSISTENCE_IDENTITY_DRIFT",
            "Applied lifecycle identity does not match the sealed Rust command.",
        )


def _safe_failure_message(value: str, fallback: str) -> str:
    message = value[:500]
    try:
        validate_bounded_json(
            {"message": message},
            forbidden_keys=_PRODUCT_TOOL_FORBIDDEN_KEYS,
            boundary_name="ProductToolExecutionResult@1.message",
        )
    except K002PortBoundaryError:
        return fallback
    return message or fallback
