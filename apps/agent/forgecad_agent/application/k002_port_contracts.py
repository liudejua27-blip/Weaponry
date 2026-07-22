"""Exact Python mirrors of the K002 Rust compatibility-port wire DTOs.

Rust owns lifecycle and Product Tool decisions.  These strict models mirror
``persistence.rs`` and ``product_tool.rs`` so Python can only persist or execute
one already-sealed, versioned command while it remains the K002 compatibility
writer/executor.
"""

from __future__ import annotations

from typing import Annotated, Any, Literal, Optional, Union

from pydantic import Field, model_validator

from .agent_models import (
    AgentApproval,
    AgentItem,
    AgentThreadDetail,
    AgentThreadSummary,
    AgentTurn,
)
from .concept_models import StrictApiModel


_STABLE_ID_PATTERN = r"^[A-Za-z0-9_.\-]{1,160}$"
_SHA256_PATTERN = r"^[a-f0-9]{64}$"
_TOOL_ID_PATTERN = r"^forgecad\.[A-Za-z0-9_.\-]+\.v1$"
_MUTATING_PERSISTENCE_OPERATIONS = frozenset(
    {
        "create_thread",
        "archive_thread",
        "create_turn",
        "append_item",
        "create_approval",
        "resolve_approval",
        "set_turn_terminal",
    }
)
_CAS_REQUIRED_PERSISTENCE_OPERATIONS = _MUTATING_PERSISTENCE_OPERATIONS - {
    "create_thread"
}


class LoadThreadPersistenceOperation(StrictApiModel):
    operation: Literal["load_thread"]
    thread_id: str = Field(pattern=_STABLE_ID_PATTERN)


class ListThreadsPersistenceOperation(StrictApiModel):
    operation: Literal["list_threads"]
    project_id: Optional[str] = Field(default=None, pattern=_STABLE_ID_PATTERN)
    include_archived: bool = False
    limit: int = Field(ge=1, le=200)


class CreateThreadPersistenceOperation(StrictApiModel):
    operation: Literal["create_thread"]
    thread: AgentThreadSummary


class ArchiveThreadPersistenceOperation(StrictApiModel):
    operation: Literal["archive_thread"]
    thread: AgentThreadSummary

    @model_validator(mode="after")
    def validate_archived_thread(self) -> "ArchiveThreadPersistenceOperation":
        if self.thread.status != "archived":
            raise ValueError("archive_thread requires an archived Thread summary")
        return self


class CreateTurnPersistenceOperation(StrictApiModel):
    operation: Literal["create_turn"]
    thread_id: str = Field(pattern=_STABLE_ID_PATTERN)
    turn: AgentTurn

    @model_validator(mode="after")
    def validate_new_turn(self) -> "CreateTurnPersistenceOperation":
        if self.turn.thread_id != self.thread_id:
            raise ValueError("create_turn must preserve thread_id")
        if self.turn.items or self.turn.approvals:
            raise ValueError("create_turn must start with empty items and approvals")
        return self


class AppendItemPersistenceOperation(StrictApiModel):
    operation: Literal["append_item"]
    item: AgentItem
    expected_previous_sequence: int = Field(ge=0)

    @model_validator(mode="after")
    def validate_sequence_cas(self) -> "AppendItemPersistenceOperation":
        if self.item.sequence != self.expected_previous_sequence + 1:
            raise ValueError("append_item must immediately follow expected_previous_sequence")
        return self


class CreateApprovalPersistenceOperation(StrictApiModel):
    operation: Literal["create_approval"]
    approval: AgentApproval

    @model_validator(mode="after")
    def validate_pending_approval(self) -> "CreateApprovalPersistenceOperation":
        if self.approval.status != "pending" or self.approval.resolved_at is not None:
            raise ValueError("create_approval requires a pending unresolved approval")
        return self


class ResolveApprovalPersistenceOperation(StrictApiModel):
    operation: Literal["resolve_approval"]
    approval: AgentApproval

    @model_validator(mode="after")
    def validate_resolved_approval(self) -> "ResolveApprovalPersistenceOperation":
        if self.approval.status == "pending" or self.approval.resolved_at is None:
            raise ValueError("resolve_approval requires a resolved approval and timestamp")
        return self


class SetTurnTerminalPersistenceOperation(StrictApiModel):
    operation: Literal["set_turn_terminal"]
    turn: AgentTurn

    @model_validator(mode="after")
    def validate_terminal_turn(self) -> "SetTurnTerminalPersistenceOperation":
        if self.turn.status not in {"completed", "failed", "cancelled"}:
            raise ValueError("set_turn_terminal requires a terminal Turn status")
        return self


class ReplayItemsPersistenceOperation(StrictApiModel):
    operation: Literal["replay_items"]
    thread_id: str = Field(pattern=_STABLE_ID_PATTERN)
    after_sequence: int = Field(default=0, ge=0)
    limit: int = Field(ge=1, le=200)


LifecyclePersistenceOperation = Annotated[
    Union[
        LoadThreadPersistenceOperation,
        ListThreadsPersistenceOperation,
        CreateThreadPersistenceOperation,
        ArchiveThreadPersistenceOperation,
        CreateTurnPersistenceOperation,
        AppendItemPersistenceOperation,
        CreateApprovalPersistenceOperation,
        ResolveApprovalPersistenceOperation,
        SetTurnTerminalPersistenceOperation,
        ReplayItemsPersistenceOperation,
    ],
    Field(discriminator="operation"),
]


class LifecyclePersistenceCommand(StrictApiModel):
    schema_version: Literal["LifecyclePersistenceCommand@1"] = (
        "LifecyclePersistenceCommand@1"
    )
    command_id: str = Field(pattern=_STABLE_ID_PATTERN)
    idempotency_key: str = Field(pattern=_SHA256_PATTERN)
    expected_revision: Optional[str] = Field(default=None, min_length=1, max_length=256)
    command: LifecyclePersistenceOperation

    @model_validator(mode="after")
    def validate_expected_state(self) -> "LifecyclePersistenceCommand":
        if not _printable_ascii(self.expected_revision):
            raise ValueError("expected_revision must be bounded printable ASCII")
        if self.operation_name in _CAS_REQUIRED_PERSISTENCE_OPERATIONS:
            if self.expected_revision is None:
                raise ValueError("mutating an existing lifecycle aggregate requires CAS revision")
        return self

    @property
    def operation_name(self) -> str:
        return self.command.operation

    @property
    def mutates_persistence(self) -> bool:
        return self.operation_name in _MUTATING_PERSISTENCE_OPERATIONS


class AppliedPersistenceOutcome(StrictApiModel):
    outcome: Literal["applied"]
    thread_id: str = Field(pattern=_STABLE_ID_PATTERN)
    turn_id: Optional[str] = Field(default=None, pattern=_STABLE_ID_PATTERN)
    item_id: Optional[str] = Field(default=None, pattern=_STABLE_ID_PATTERN)
    approval_id: Optional[str] = Field(default=None, pattern=_STABLE_ID_PATTERN)
    sequence: Optional[int] = Field(default=None, ge=1)

    @model_validator(mode="after")
    def validate_applied_identity(self) -> "AppliedPersistenceOutcome":
        if self.item_id is not None and self.turn_id is None:
            raise ValueError("applied item identity requires turn_id")
        if self.approval_id is not None and (self.turn_id is None or self.item_id is None):
            raise ValueError("applied approval identity requires turn_id and item_id")
        if (self.sequence is None) != (self.item_id is None):
            raise ValueError("applied item identity requires one positive sequence")
        return self


class ThreadLoadedPersistenceOutcome(StrictApiModel):
    outcome: Literal["thread_loaded"]
    thread: Optional[AgentThreadDetail] = None


class ThreadsListedPersistenceOutcome(StrictApiModel):
    outcome: Literal["threads_listed"]
    threads: list[AgentThreadSummary] = Field(max_length=200)


class ItemsReplayedPersistenceOutcome(StrictApiModel):
    outcome: Literal["items_replayed"]
    thread_id: str = Field(pattern=_STABLE_ID_PATTERN)
    items: list[AgentItem] = Field(max_length=200)
    next_sequence: Optional[int] = Field(default=None, ge=1)

    @model_validator(mode="after")
    def validate_replay_identity(self) -> "ItemsReplayedPersistenceOutcome":
        previous = 0
        for item in self.items:
            if (
                item.thread_id != self.thread_id
                or item.sequence <= previous
            ):
                raise ValueError(
                    "replayed items must preserve thread identity and sequence"
                )
            previous = item.sequence
        if self.next_sequence is not None and self.next_sequence <= previous:
            raise ValueError("next_sequence must follow the last returned item")
        return self


LifecyclePersistenceOutcome = Annotated[
    Union[
        AppliedPersistenceOutcome,
        ThreadLoadedPersistenceOutcome,
        ThreadsListedPersistenceOutcome,
        ItemsReplayedPersistenceOutcome,
    ],
    Field(discriminator="outcome"),
]


class LifecyclePersistenceResult(StrictApiModel):
    schema_version: Literal["LifecyclePersistenceResult@1"] = "LifecyclePersistenceResult@1"
    command_id: str = Field(pattern=_STABLE_ID_PATTERN)
    revision: str = Field(min_length=1, max_length=256)
    replayed: bool
    result: LifecyclePersistenceOutcome

    @model_validator(mode="after")
    def validate_revision(self) -> "LifecyclePersistenceResult":
        if not _printable_ascii(self.revision):
            raise ValueError("revision must be bounded printable ASCII")
        return self


class ValidatedProductToolPayload(StrictApiModel):
    schema_id: str = Field(min_length=1, max_length=240)
    schema_sha256: str = Field(pattern=_SHA256_PATTERN)
    value: dict[str, Any]


ProductToolApprovalPolicy = Literal[
    "read_only",
    "candidate_only",
    "user_confirmation_required",
]
ProductToolExecutionStatus = Literal["completed", "failed", "cancelled", "rejected"]
ProductToolFailureCategory = Literal[
    "schema",
    "permission",
    "unsupported",
    "conflict",
    "cancelled",
    "timeout",
    "provider",
    "execution",
]


class ProductToolExecutionRequest(StrictApiModel):
    schema_version: Literal["ProductToolExecutionRequest@1"] = (
        "ProductToolExecutionRequest@1"
    )
    execution_id: str = Field(pattern=_STABLE_ID_PATTERN)
    turn_id: str = Field(pattern=_STABLE_ID_PATTERN)
    call_id: str = Field(pattern=_STABLE_ID_PATTERN)
    tool_id: str = Field(pattern=_TOOL_ID_PATTERN)
    tool_name: str = Field(pattern=r"^[a-z][a-z0-9_]{1,63}$")
    registry_schema_version: Literal["ForgeCADProductToolRegistry@1"]
    idempotency_key: str = Field(pattern=_SHA256_PATTERN)
    validated_arguments: ValidatedProductToolPayload
    approval_policy: ProductToolApprovalPolicy
    cancellation_id: str = Field(pattern=_STABLE_ID_PATTERN)
    cancellation_token: str = Field(pattern=_STABLE_ID_PATTERN)

    @model_validator(mode="after")
    def validate_input_schema_identity(self) -> "ProductToolExecutionRequest":
        if self.validated_arguments.schema_id != f"{self.tool_id}:input":
            raise ValueError("validated_arguments.schema_id must be bound to tool_id input")
        return self


class ProductToolExecutionResult(StrictApiModel):
    schema_version: Literal["ProductToolExecutionResult@1"] = "ProductToolExecutionResult@1"
    execution_id: str = Field(pattern=_STABLE_ID_PATTERN)
    turn_id: str = Field(pattern=_STABLE_ID_PATTERN)
    call_id: str = Field(pattern=_STABLE_ID_PATTERN)
    tool_id: str = Field(pattern=_TOOL_ID_PATTERN)
    cancellation_id: str = Field(pattern=_STABLE_ID_PATTERN)
    status: ProductToolExecutionStatus
    validated_output: Optional[ValidatedProductToolPayload] = None
    failure_category: Optional[ProductToolFailureCategory] = None
    error_code: Optional[str] = Field(default=None, pattern=_STABLE_ID_PATTERN)
    message: Optional[str] = Field(default=None, max_length=500)
    duration_ms: int = Field(ge=0)
    permanent_side_effects: Literal[0] = 0

    @model_validator(mode="after")
    def validate_status_payload(self) -> "ProductToolExecutionResult":
        if type(self.permanent_side_effects) is not int or self.permanent_side_effects != 0:
            raise ValueError("permanent_side_effects must be the integer zero")
        if self.status == "completed":
            if (
                self.validated_output is None
                or self.failure_category is not None
                or self.error_code is not None
            ):
                raise ValueError(
                    "completed tool execution requires validated_output and no failure fields"
                )
            if self.validated_output.schema_id != f"{self.tool_id}:output":
                raise ValueError("validated_output.schema_id must be bound to tool_id output")
        elif self.validated_output is not None or self.failure_category is None:
            raise ValueError(
                "non-completed tool execution requires failure_category and no output"
            )
        return self


def _printable_ascii(value: str | None) -> bool:
    if value is None:
        return True
    return value.isascii() and all(character.isprintable() for character in value)
