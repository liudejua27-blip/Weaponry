"""Bounded, single-turn Product Tool lifecycle for ForgeCAD.

The loop owns orchestration only.  Geometry operation truth remains in
ShapeProgramRuntimeManifest@1 and permanent changes remain behind the existing
Snapshot/ChangeSet approval path.  Provider hidden reasoning is carried only in
the short-lived in-memory message list and is never included in an observer
event or result returned for persistence.
"""

from __future__ import annotations

import hashlib
import json
import threading
import time
from dataclasses import dataclass, field
from typing import Any, Callable, Literal, Mapping, Optional, Protocol

from jsonschema import Draft202012Validator
from pydantic import Field

from .concept_models import StrictApiModel


ApprovalPolicy = Literal["read_only", "candidate_only", "user_confirmation_required"]
ToolFailureCategory = Literal[
    "schema",
    "permission",
    "unsupported",
    "conflict",
    "cancelled",
    "timeout",
    "provider",
    "execution",
]


class AgentActionLoopError(RuntimeError):
    def __init__(
        self,
        code: str,
        message: str,
        *,
        category: ToolFailureCategory,
        recoverable: bool = False,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.category = category
        self.recoverable = recoverable


class ProviderToolCall(StrictApiModel):
    call_id: str = Field(pattern=r"^[A-Za-z0-9_\-]{1,160}$")
    name: str = Field(pattern=r"^[a-z][a-z0-9_]{1,63}$")
    arguments_json: str = Field(min_length=2, max_length=200_000)


class ProviderActionStep(StrictApiModel):
    content: str = Field(default="", max_length=50_000)
    reasoning_content: str = Field(default="", max_length=200_000)
    tool_calls: list[ProviderToolCall] = Field(default_factory=list, max_length=12)
    total_tokens: int | None = Field(default=None, ge=0)


class AgentActionToolEvent(StrictApiModel):
    schema_version: Literal["AgentActionToolEvent@1"] = "AgentActionToolEvent@1"
    event_kind: Literal["tool_call", "tool_result"]
    parent_turn_id: str = Field(min_length=1, max_length=160)
    tool_call_id: str = Field(pattern=r"^[A-Za-z0-9_\-]{1,160}$")
    tool_id: str = Field(pattern=r"^forgecad\.[a-z0-9_.\-]+\.v1$")
    tool_name: str = Field(pattern=r"^[a-z][a-z0-9_]{1,63}$")
    status: Literal["pending", "completed", "failed", "cancelled"]
    duration_ms: int = Field(ge=0)
    idempotency_key: str = Field(pattern=r"^[a-f0-9]{64}$")
    approval_policy: ApprovalPolicy
    arguments: dict[str, Any] | None = None
    result: dict[str, Any] | None = None
    failure_category: ToolFailureCategory | None = None
    error_code: str | None = Field(default=None, max_length=120)
    message: str | None = Field(default=None, max_length=500)


class ProductToolManifest(StrictApiModel):
    tool_id: str = Field(pattern=r"^forgecad\.[a-z0-9_.\-]+\.v1$")
    name: str = Field(pattern=r"^[a-z][a-z0-9_]{1,63}$")
    description: str = Field(min_length=1, max_length=500)
    input_schema: dict[str, Any]
    output_schema: dict[str, Any]
    approval_policy: ApprovalPolicy


class ProductToolRegistryManifest(StrictApiModel):
    schema_version: Literal["ForgeCADProductToolRegistry@1"] = "ForgeCADProductToolRegistry@1"
    tools: list[ProductToolManifest] = Field(min_length=1, max_length=32)


@dataclass(frozen=True)
class ProductToolDefinition:
    tool_id: str
    name: str
    description: str
    input_schema: dict[str, Any]
    output_schema: dict[str, Any]
    approval_policy: ApprovalPolicy
    handler: Callable[[dict[str, Any], "AgentActionContext"], dict[str, Any]]

    def provider_contract(self) -> dict[str, Any]:
        return {
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.input_schema,
            },
        }

    def public_manifest(self) -> dict[str, Any]:
        return {
            "tool_id": self.tool_id,
            "name": self.name,
            "description": self.description,
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "approval_policy": self.approval_policy,
        }


class ProductToolRegistry:
    """Immutable code-owned registry; models cannot add or replace tools."""

    schema_version = "ForgeCADProductToolRegistry@1"

    def __init__(self, tools: tuple[ProductToolDefinition, ...]) -> None:
        names = [tool.name for tool in tools]
        ids = [tool.tool_id for tool in tools]
        if len(names) != len(set(names)) or len(ids) != len(set(ids)):
            raise AgentActionLoopError(
                "DUPLICATE_PRODUCT_TOOL",
                "Product Tool Registry contains a duplicate stable name or ID.",
                category="conflict",
            )
        for tool in tools:
            Draft202012Validator.check_schema(tool.input_schema)
            Draft202012Validator.check_schema(tool.output_schema)
            if tool.approval_policy == "user_confirmation_required":
                raise AgentActionLoopError(
                    "PERMANENT_TOOL_REGISTRATION_FORBIDDEN",
                    "Permanent mutation tools cannot run inside the candidate Action Loop.",
                    category="permission",
                )
        self._tools = tools
        self._by_name = {tool.name: tool for tool in tools}

    def require(self, name: str) -> ProductToolDefinition:
        tool = self._by_name.get(name)
        if tool is None:
            raise AgentActionLoopError(
                "PRODUCT_TOOL_NOT_ALLOWED",
                f"Tool {name!r} is not in ForgeCADProductToolRegistry@1.",
                category="permission",
            )
        return tool

    def provider_contracts(self) -> list[dict[str, Any]]:
        return [tool.provider_contract() for tool in self._tools]

    def public_manifest(self) -> ProductToolRegistryManifest:
        return ProductToolRegistryManifest.model_validate({
            "schema_version": self.schema_version,
            "tools": [tool.public_manifest() for tool in self._tools],
        })


@dataclass
class AgentActionContext:
    parent_turn_id: str
    state: dict[str, Any] = field(default_factory=dict)
    cancel_event: Optional[threading.Event] = None
    expected_snapshot_fingerprint: str | None = None
    current_snapshot_fingerprint: str | None = None

    def check_cancelled(self) -> None:
        if self.cancel_event is not None and self.cancel_event.is_set():
            raise AgentActionLoopError(
                "AGENT_ACTION_CANCELLED",
                "Agent action was cancelled; saved assets were not changed.",
                category="cancelled",
            )


@dataclass(frozen=True)
class AgentActionLoopConfig:
    max_tool_calls: int = 12
    max_wall_seconds: float = 60.0
    max_total_tokens: int = 100_000


@dataclass(frozen=True)
class AgentActionLoopResult:
    final_content: str
    tool_call_count: int
    total_provider_tokens: int
    state: Mapping[str, Any]


class ProviderActionCallback(Protocol):
    def __call__(
        self,
        messages: list[dict[str, Any]],
        tools: list[dict[str, Any]],
        cancel_event: Optional[threading.Event],
    ) -> ProviderActionStep:
        ...


class AgentActionLoop:
    def __init__(self, registry: ProductToolRegistry, config: AgentActionLoopConfig | None = None) -> None:
        self.registry = registry
        self.config = config or AgentActionLoopConfig()
        if not 1 <= self.config.max_tool_calls <= 12:
            raise ValueError("max_tool_calls must be between 1 and 12")
        if not 1 <= self.config.max_wall_seconds <= 300:
            raise ValueError("max_wall_seconds must be between 1 and 300")
        if not 1_000 <= self.config.max_total_tokens <= 1_000_000:
            raise ValueError("max_total_tokens must be between 1000 and 1000000")

    def run(
        self,
        *,
        messages: list[dict[str, Any]],
        provider: ProviderActionCallback,
        context: AgentActionContext,
        observer: Callable[[AgentActionToolEvent], None] | None = None,
    ) -> AgentActionLoopResult:
        started_at = time.monotonic()
        seen_call_ids: set[str] = set()
        call_count = 0
        total_provider_tokens = 0
        ephemeral_messages = [dict(message) for message in messages]
        if (
            context.expected_snapshot_fingerprint is not None
            and context.expected_snapshot_fingerprint != context.current_snapshot_fingerprint
        ):
            raise AgentActionLoopError(
                "STALE_ACTIVE_DESIGN_SNAPSHOT",
                "ActiveDesignSnapshot changed before the Action Loop started.",
                category="conflict",
            )
        while True:
            self._check_limits(started_at, context)
            try:
                step = provider(ephemeral_messages, self.registry.provider_contracts(), context.cancel_event)
            except AgentActionLoopError:
                raise
            except Exception as exc:
                raise AgentActionLoopError(
                    "PROVIDER_ACTION_DISCONNECTED",
                    "Provider disconnected during the Agent Action Loop.",
                    category="provider",
                    recoverable=True,
                ) from exc
            self._check_limits(started_at, context)
            if step.total_tokens is not None:
                total_provider_tokens += step.total_tokens
                if total_provider_tokens > self.config.max_total_tokens:
                    raise AgentActionLoopError(
                        "ACTION_LOOP_TOKEN_LIMIT",
                        "Agent Action Loop exceeded its total Provider token limit.",
                        category="permission",
                    )
            assistant_message: dict[str, Any] = {
                "role": "assistant",
                "content": step.content or None,
            }
            if step.reasoning_content:
                # DeepSeek requires this on the next sub-request.  It remains in
                # this local list only and is never exposed through observer.
                assistant_message["reasoning_content"] = step.reasoning_content
            if step.tool_calls:
                assistant_message["tool_calls"] = [
                    {
                        "id": call.call_id,
                        "type": "function",
                        "function": {"name": call.name, "arguments": call.arguments_json},
                    }
                    for call in step.tool_calls
                ]
            ephemeral_messages.append(assistant_message)
            if not step.tool_calls:
                if "plan" not in context.state:
                    raise AgentActionLoopError(
                        "ACTION_LOOP_PLAN_MISSING",
                        "Provider stopped before producing a validated concept plan.",
                        category="execution",
                    )
                return AgentActionLoopResult(
                    final_content=step.content,
                    tool_call_count=call_count,
                    total_provider_tokens=total_provider_tokens,
                    state=dict(context.state),
                )
            for call in step.tool_calls:
                self._check_limits(started_at, context)
                if call_count >= self.config.max_tool_calls:
                    raise AgentActionLoopError(
                        "ACTION_LOOP_CALL_LIMIT",
                        "Agent Action Loop exceeded the maximum of 12 product tool calls.",
                        category="permission",
                    )
                if call.call_id in seen_call_ids:
                    raise AgentActionLoopError(
                        "DUPLICATE_PROVIDER_TOOL_CALL_ID",
                        "Provider reused a tool call ID within one Turn.",
                        category="conflict",
                    )
                seen_call_ids.add(call.call_id)
                call_count += 1
                tool = self.registry.require(call.name)
                arguments = self._decode_arguments(call)
                self._validate(tool.input_schema, arguments, "PRODUCT_TOOL_INPUT_SCHEMA_INVALID")
                idempotency_key = _hash_json(
                    {
                        "turn_id": context.parent_turn_id,
                        "call_id": call.call_id,
                        "tool_id": tool.tool_id,
                        "arguments": arguments,
                    }
                )
                self._notify(
                    observer,
                    AgentActionToolEvent(
                        event_kind="tool_call",
                        parent_turn_id=context.parent_turn_id,
                        tool_call_id=call.call_id,
                        tool_id=tool.tool_id,
                        tool_name=tool.name,
                        status="completed",
                        duration_ms=0,
                        idempotency_key=idempotency_key,
                        approval_policy=tool.approval_policy,
                        arguments=arguments,
                    ),
                )
                tool_started = time.monotonic()
                try:
                    context.check_cancelled()
                    result = tool.handler(arguments, context)
                    self._validate(tool.output_schema, result, "PRODUCT_TOOL_OUTPUT_SCHEMA_INVALID")
                    status: Literal["completed", "failed", "cancelled"] = "completed"
                    failure_category = None
                    error_code = None
                    message = None
                except AgentActionLoopError as exc:
                    status = "cancelled" if exc.category == "cancelled" else "failed"
                    result = None
                    failure_category = exc.category
                    error_code = exc.code
                    message = str(exc)
                    self._notify_result(
                        observer,
                        context,
                        call,
                        tool,
                        idempotency_key,
                        tool_started,
                        status,
                        result,
                        failure_category,
                        error_code,
                        message,
                    )
                    raise
                except Exception as exc:
                    status = "failed"
                    result = None
                    failure_category = "execution"
                    error_code = "PRODUCT_TOOL_EXECUTION_FAILED"
                    message = str(exc)[:500] or "Product tool failed."
                    self._notify_result(
                        observer,
                        context,
                        call,
                        tool,
                        idempotency_key,
                        tool_started,
                        status,
                        result,
                        failure_category,
                        error_code,
                        message,
                    )
                    raise AgentActionLoopError(
                        error_code,
                        "Product tool execution failed.",
                        category="execution",
                    ) from exc
                self._notify_result(
                    observer,
                    context,
                    call,
                    tool,
                    idempotency_key,
                    tool_started,
                    status,
                    result,
                    failure_category,
                    error_code,
                    message,
                )
                ephemeral_messages.append(
                    {
                        "role": "tool",
                        "tool_call_id": call.call_id,
                        "content": json.dumps(result, ensure_ascii=False, sort_keys=True, separators=(",", ":")),
                    }
                )

    def _check_limits(self, started_at: float, context: AgentActionContext) -> None:
        context.check_cancelled()
        if time.monotonic() - started_at > self.config.max_wall_seconds:
            raise AgentActionLoopError(
                "ACTION_LOOP_TIMEOUT",
                "Agent Action Loop exceeded its wall-time limit; saved assets were not changed.",
                category="timeout",
                recoverable=True,
            )

    @staticmethod
    def _decode_arguments(call: ProviderToolCall) -> dict[str, Any]:
        try:
            value = json.loads(call.arguments_json)
        except json.JSONDecodeError as exc:
            raise AgentActionLoopError(
                "PRODUCT_TOOL_ARGUMENTS_INVALID_JSON",
                "Product tool arguments were not valid JSON.",
                category="schema",
            ) from exc
        if not isinstance(value, dict):
            raise AgentActionLoopError(
                "PRODUCT_TOOL_ARGUMENTS_NOT_OBJECT",
                "Product tool arguments must be a JSON object.",
                category="schema",
            )
        return value

    @staticmethod
    def _validate(schema: Mapping[str, Any], value: Any, code: str) -> None:
        errors = sorted(Draft202012Validator(schema).iter_errors(value), key=lambda item: list(item.path))
        if errors:
            location = ".".join(str(part) for part in errors[0].path) or "$"
            raise AgentActionLoopError(
                code,
                f"Product tool Schema rejected {location}: {errors[0].message}",
                category="schema",
            )

    @staticmethod
    def _notify(observer: Callable[[AgentActionToolEvent], None] | None, event: AgentActionToolEvent) -> None:
        if observer is None:
            return
        try:
            observer(event)
        except Exception:
            # Persistence telemetry cannot change tool execution outcome.
            return

    def _notify_result(
        self,
        observer: Callable[[AgentActionToolEvent], None] | None,
        context: AgentActionContext,
        call: ProviderToolCall,
        tool: ProductToolDefinition,
        idempotency_key: str,
        started_at: float,
        status: Literal["completed", "failed", "cancelled"],
        result: dict[str, Any] | None,
        failure_category: ToolFailureCategory | None,
        error_code: str | None,
        message: str | None,
    ) -> None:
        self._notify(
            observer,
            AgentActionToolEvent(
                event_kind="tool_result",
                parent_turn_id=context.parent_turn_id,
                tool_call_id=call.call_id,
                tool_id=tool.tool_id,
                tool_name=tool.name,
                status=status,
                duration_ms=max(0, int((time.monotonic() - started_at) * 1000)),
                idempotency_key=idempotency_key,
                approval_policy=tool.approval_policy,
                result=result,
                failure_category=failure_category,
                error_code=error_code,
                message=message,
            ),
        )


def _hash_json(value: Any) -> str:
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()
