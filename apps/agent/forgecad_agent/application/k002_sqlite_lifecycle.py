"""SQLite implementation of the transitional K002 lifecycle persistence port.

Rust remains the sole lifecycle decision maker; this backend only persists the
sealed ``LifecyclePersistenceCommand@1`` into the existing Agent Kernel tables.
Every call runs in one ``BEGIN IMMEDIATE`` transaction, compares a canonical
full-thread hash for CAS, and records command-hash-bound idempotency in the same
transaction.  It does not own Provider, budget, Project, Snapshot, or asset
state and is intentionally not wired into FastAPI or the sidecar factory here.
"""

from __future__ import annotations

import json
import sqlite3
from datetime import datetime, timezone
from typing import Any, Optional

from .agent_models import AgentApproval, AgentItem, AgentThreadDetail, AgentThreadSummary, AgentTurn
from .k002_port_contracts import (
    AppendItemPersistenceOperation,
    AppliedPersistenceOutcome,
    ArchiveThreadPersistenceOperation,
    CreateApprovalPersistenceOperation,
    CreateThreadPersistenceOperation,
    CreateTurnPersistenceOperation,
    ItemsReplayedPersistenceOutcome,
    LifecyclePersistenceCommand,
    LifecyclePersistenceResult,
    ListThreadsPersistenceOperation,
    LoadThreadPersistenceOperation,
    ReplayItemsPersistenceOperation,
    ResolveApprovalPersistenceOperation,
    SetTurnTerminalPersistenceOperation,
    ThreadLoadedPersistenceOutcome,
    ThreadsListedPersistenceOutcome,
)
from .k002_port_security import K002PortBoundaryError, canonical_json_sha256
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteUnitOfWork


_IDEMPOTENCY_SCOPE = "K002 LifecyclePersistenceCommand@1"
_TERMINAL_TURN_STATUSES = {"completed", "failed", "cancelled"}


class K002SQLiteLifecycleBackend:
    """The sole SQLite lifecycle writer used behind ``LifecyclePersistencePort``."""

    def __init__(self, connection_factory: SQLiteConnectionFactory) -> None:
        self.connection_factory = connection_factory

    def execute(self, command: LifecyclePersistenceCommand) -> LifecyclePersistenceResult:
        parsed = LifecyclePersistenceCommand.model_validate(command)
        command_json = parsed.model_dump(mode="json", exclude_none=True)
        # `command_id` is transport correlation, not business identity. A
        # reconnect may legitimately resend the same sealed idempotency key
        # under a fresh JSON-RPC command ID; the stored outcome must then be
        # rebound to the current command rather than treated as a conflict.
        command_json.pop("command_id", None)
        command_hash = canonical_json_sha256(command_json)
        try:
            with SQLiteUnitOfWork(self.connection_factory) as unit:
                connection = unit.require_connection()
                connection.execute("BEGIN IMMEDIATE")
                replay = unit.idempotency.get(_IDEMPOTENCY_SCOPE, parsed.idempotency_key)
                if replay is not None:
                    if replay.request_hash != command_hash:
                        raise K002PortBoundaryError(
                            "K002_PERSISTENCE_IDEMPOTENCY_CONFLICT",
                            "Lifecycle idempotency key was reused for another sealed command.",
                        )
                    stored = LifecyclePersistenceResult.model_validate_json(replay.response_json)
                    return stored.model_copy(
                        update={"command_id": parsed.command_id, "replayed": True},
                        deep=True,
                    )

                result = self._apply(unit, parsed)
                unit.idempotency.add(
                    scope=_IDEMPOTENCY_SCOPE,
                    key=parsed.idempotency_key,
                    request_hash=command_hash,
                    response_json=_canonical_json(result.model_dump(mode="json", exclude_none=True)),
                    created_at=_utc_now(),
                )
                return result
        except K002PortBoundaryError:
            raise
        except sqlite3.IntegrityError as exc:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_SQLITE_CONFLICT",
                "SQLite rejected a conflicting sealed lifecycle command.",
            ) from exc

    def _apply(
        self,
        unit: SQLiteUnitOfWork,
        command: LifecyclePersistenceCommand,
    ) -> LifecyclePersistenceResult:
        operation = command.command
        if isinstance(operation, LoadThreadPersistenceOperation):
            thread = _thread_detail(unit, operation.thread_id)
            self._require_optional_read_revision(command, _thread_revision(thread))
            return _result(
                command,
                revision=_thread_revision(thread),
                outcome=ThreadLoadedPersistenceOutcome(
                    outcome="thread_loaded",
                    thread=thread,
                ),
            )
        if isinstance(operation, ListThreadsPersistenceOperation):
            threads = _list_thread_summaries(unit, operation)
            revision = _collection_revision([item.model_dump(mode="json") for item in threads])
            self._require_optional_read_revision(command, revision)
            return _result(
                command,
                revision=revision,
                outcome=ThreadsListedPersistenceOutcome(
                    outcome="threads_listed",
                    threads=threads,
                ),
            )
        if isinstance(operation, CreateThreadPersistenceOperation):
            return self._create_thread(unit, command, operation)
        if isinstance(operation, ArchiveThreadPersistenceOperation):
            return self._archive_thread(unit, command, operation)
        if isinstance(operation, CreateTurnPersistenceOperation):
            return self._create_turn(unit, command, operation)
        if isinstance(operation, AppendItemPersistenceOperation):
            return self._append_item(unit, command, operation)
        if isinstance(operation, CreateApprovalPersistenceOperation):
            return self._create_approval(unit, command, operation)
        if isinstance(operation, ResolveApprovalPersistenceOperation):
            return self._resolve_approval(unit, command, operation)
        if isinstance(operation, SetTurnTerminalPersistenceOperation):
            return self._set_turn_terminal(unit, command, operation)
        if isinstance(operation, ReplayItemsPersistenceOperation):
            return self._replay_items(unit, command, operation)
        raise K002PortBoundaryError(
            "K002_PERSISTENCE_OPERATION_UNSUPPORTED",
            "Lifecycle operation is not implemented by the SQLite compatibility backend.",
        )

    @staticmethod
    def _require_optional_read_revision(
        command: LifecyclePersistenceCommand,
        actual_revision: str,
    ) -> None:
        if (
            command.expected_revision is not None
            and command.expected_revision != actual_revision
        ):
            raise _stale_revision()

    def _create_thread(
        self,
        unit: SQLiteUnitOfWork,
        command: LifecyclePersistenceCommand,
        operation: CreateThreadPersistenceOperation,
    ) -> LifecyclePersistenceResult:
        thread = operation.thread
        current = _thread_detail(unit, thread.thread_id)
        if current is not None:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_THREAD_EXISTS",
                "Sealed create_thread targets an existing lifecycle Thread.",
            )
        if command.expected_revision is not None:
            if command.expected_revision != _thread_revision(None):
                raise _stale_revision()
        if thread.status != "idle" or thread.last_turn_id is not None:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_CREATE_THREAD_STATE_INVALID",
                "A persisted lifecycle Thread must start idle without a last Turn.",
            )
        unit.require_connection().execute(
            """
            INSERT INTO agent_threads (
              thread_id, project_id, title, status, summary, provider_id,
              created_at, updated_at, last_turn_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                thread.thread_id,
                thread.project_id,
                thread.title,
                thread.status,
                thread.summary,
                thread.provider_id,
                thread.created_at,
                thread.updated_at,
                thread.last_turn_id,
            ),
        )
        return _applied_result(command, _require_thread_detail(unit, thread.thread_id))

    def _create_turn(
        self,
        unit: SQLiteUnitOfWork,
        command: LifecyclePersistenceCommand,
        operation: CreateTurnPersistenceOperation,
    ) -> LifecyclePersistenceResult:
        current = self._require_cas(unit, command, operation.thread_id)
        turn = operation.turn
        if turn.status != "running":
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_CREATE_TURN_STATE_INVALID",
                "The Rust lifecycle must persist a new Turn in running state.",
            )
        if any(item.status not in _TERMINAL_TURN_STATUSES for item in current.turns):
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_TURN_ALREADY_ACTIVE",
                "A lifecycle Thread can have only one non-terminal Turn.",
            )
        unit.agent_kernel.add_turn(
            turn_id=turn.turn_id,
            thread_id=turn.thread_id,
            request_text=turn.request_text,
            status=turn.status,
            created_at=turn.created_at,
        )
        unit.agent_kernel.update_turn(
            turn_id=turn.turn_id,
            status=turn.status,
            updated_at=turn.updated_at,
            error_code=turn.error_code,
            error_message=turn.error_message,
            usage=turn.usage,
        )
        unit.agent_kernel.update_thread(
            thread_id=turn.thread_id,
            status="active",
            summary=None,
            last_turn_id=turn.turn_id,
            updated_at=turn.updated_at,
        )
        return _applied_result(
            command,
            _require_thread_detail(unit, turn.thread_id),
            turn_id=turn.turn_id,
        )

    def _archive_thread(
        self,
        unit: SQLiteUnitOfWork,
        command: LifecyclePersistenceCommand,
        operation: ArchiveThreadPersistenceOperation,
    ) -> LifecyclePersistenceResult:
        current = self._require_cas(unit, command, operation.thread.thread_id)
        target = operation.thread
        expected = current.model_copy(
            update={"status": "archived", "updated_at": target.updated_at},
            deep=True,
        )
        expected_summary = AgentThreadSummary.model_validate(
            expected.model_dump(mode="json", exclude={"turns"})
        )
        if expected_summary != target:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_ARCHIVE_IDENTITY_DRIFT",
                "Archive Thread may update only status and updated_at.",
            )
        unit.require_connection().execute(
            """
            UPDATE agent_threads
            SET status = 'archived', updated_at = ?
            WHERE thread_id = ?
            """,
            (target.updated_at, target.thread_id),
        )
        return _applied_result(command, _require_thread_detail(unit, target.thread_id))

    def _append_item(
        self,
        unit: SQLiteUnitOfWork,
        command: LifecyclePersistenceCommand,
        operation: AppendItemPersistenceOperation,
    ) -> LifecyclePersistenceResult:
        item = operation.item
        self._require_cas(unit, command, item.thread_id)
        actual_previous = unit.agent_kernel.next_sequence(item.thread_id) - 1
        if actual_previous != operation.expected_previous_sequence:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_ITEM_SEQUENCE_CONFLICT",
                "Append Item sequence is stale for the thread-scoped event stream.",
            )
        turn_row = unit.agent_kernel.get_turn(item.turn_id)
        if turn_row is None or str(turn_row["thread_id"]) != item.thread_id:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_TURN_NOT_FOUND",
                "Append Item targets no Turn in the sealed Thread.",
            )
        if str(turn_row["status"]) in _TERMINAL_TURN_STATUSES:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_TURN_TERMINAL",
                "A terminal Turn cannot accept a new lifecycle Item.",
            )
        unit.agent_kernel.add_item(
            item_id=item.item_id,
            thread_id=item.thread_id,
            turn_id=item.turn_id,
            sequence=item.sequence,
            item_type=item.item_type,
            status=item.status,
            payload=item.payload,
            created_at=item.created_at,
        )
        turn_status = (
            "waiting_for_clarification"
            if item.item_type == "clarification"
            else str(turn_row["status"])
        )
        unit.agent_kernel.update_turn(
            turn_id=item.turn_id,
            status=turn_status,
            updated_at=item.created_at,
        )
        unit.agent_kernel.update_thread(
            thread_id=item.thread_id,
            status="active",
            summary=None,
            last_turn_id=item.turn_id,
            updated_at=item.created_at,
        )
        return _applied_result(
            command,
            _require_thread_detail(unit, item.thread_id),
            turn_id=item.turn_id,
            item_id=item.item_id,
            sequence=item.sequence,
        )

    def _create_approval(
        self,
        unit: SQLiteUnitOfWork,
        command: LifecyclePersistenceCommand,
        operation: CreateApprovalPersistenceOperation,
    ) -> LifecyclePersistenceResult:
        approval = operation.approval
        self._require_cas(unit, command, approval.thread_id)
        item_row = _get_item_row(unit, approval.item_id)
        if (
            item_row is None
            or str(item_row["thread_id"]) != approval.thread_id
            or str(item_row["turn_id"]) != approval.turn_id
            or str(item_row["item_type"]) != "approval_request"
            or str(item_row["status"]) != "pending"
        ):
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_APPROVAL_ITEM_INVALID",
                "Create Approval must reference its pending approval_request Item.",
            )
        unit.agent_kernel.add_approval(
            approval_id=approval.approval_id,
            thread_id=approval.thread_id,
            turn_id=approval.turn_id,
            item_id=approval.item_id,
            action=approval.action,
            payload=approval.payload,
            created_at=approval.created_at,
        )
        unit.agent_kernel.update_turn(
            turn_id=approval.turn_id,
            status="waiting_for_approval",
            updated_at=approval.created_at,
        )
        unit.agent_kernel.update_thread(
            thread_id=approval.thread_id,
            status="active",
            summary=None,
            last_turn_id=approval.turn_id,
            updated_at=approval.created_at,
        )
        return _applied_result(
            command,
            _require_thread_detail(unit, approval.thread_id),
            turn_id=approval.turn_id,
            item_id=approval.item_id,
            approval_id=approval.approval_id,
            sequence=int(item_row["sequence"]),
        )

    def _resolve_approval(
        self,
        unit: SQLiteUnitOfWork,
        command: LifecyclePersistenceCommand,
        operation: ResolveApprovalPersistenceOperation,
    ) -> LifecyclePersistenceResult:
        approval = operation.approval
        self._require_cas(unit, command, approval.thread_id)
        existing = unit.agent_kernel.get_approval(approval.approval_id)
        if existing is None or str(existing["status"]) != "pending":
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_APPROVAL_NOT_PENDING",
                "Resolve Approval requires the current pending approval.",
            )
        immutable_existing = {
            "approval_id": str(existing["approval_id"]),
            "thread_id": str(existing["thread_id"]),
            "turn_id": str(existing["turn_id"]),
            "item_id": str(existing["item_id"]),
            "action": str(existing["action"]),
            "payload": json.loads(str(existing["payload_json"])),
            "created_at": str(existing["created_at"]),
        }
        immutable_command = approval.model_dump(mode="json", exclude={"status", "resolved_at"})
        if immutable_existing != immutable_command:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_APPROVAL_IDENTITY_DRIFT",
                "Resolved Approval changed immutable persisted identity.",
            )
        assert approval.resolved_at is not None
        unit.agent_kernel.resolve_approval(
            approval.approval_id,
            status=approval.status,
            resolved_at=approval.resolved_at,
        )
        item_status = "completed" if approval.status == "approved" else "cancelled"
        turn_status = "running" if approval.status == "approved" else "cancelled"
        unit.agent_kernel.update_item_status(approval.item_id, status=item_status)
        unit.agent_kernel.update_turn(
            turn_id=approval.turn_id,
            status=turn_status,
            updated_at=approval.resolved_at,
            error_code=None,
            error_message=None,
        )
        unit.agent_kernel.update_thread(
            thread_id=approval.thread_id,
            status="active" if approval.status == "approved" else "idle",
            summary=None,
            last_turn_id=approval.turn_id,
            updated_at=approval.resolved_at,
        )
        item_row = _get_item_row(unit, approval.item_id)
        assert item_row is not None
        return _applied_result(
            command,
            _require_thread_detail(unit, approval.thread_id),
            turn_id=approval.turn_id,
            item_id=approval.item_id,
            approval_id=approval.approval_id,
            sequence=int(item_row["sequence"]),
        )

    def _set_turn_terminal(
        self,
        unit: SQLiteUnitOfWork,
        command: LifecyclePersistenceCommand,
        operation: SetTurnTerminalPersistenceOperation,
    ) -> LifecyclePersistenceResult:
        target = operation.turn
        current_thread = self._require_cas(unit, command, target.thread_id)
        current = next((item for item in current_thread.turns if item.turn_id == target.turn_id), None)
        if current is None:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_TURN_NOT_FOUND",
                "Set terminal targets no Turn in the sealed Thread.",
            )
        if not _valid_terminal_transition(current.status, target.status):
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_TURN_TRANSITION_INVALID",
                "Set terminal does not match a Rust-owned lifecycle transition.",
            )
        expected = current.model_copy(
            update={
                "status": target.status,
                "error_code": target.error_code,
                "error_message": target.error_message,
                "usage": target.usage,
                "updated_at": target.updated_at,
            },
            deep=True,
        )
        if expected.model_dump(mode="json") != target.model_dump(mode="json"):
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_TURN_IDENTITY_DRIFT",
                "Set terminal may update only status, error, usage and updated_at.",
            )
        if target.status == "completed" and any(
            approval.status == "pending" for approval in current.approvals
        ):
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_APPROVAL_PENDING",
                "A Turn with pending approval cannot complete.",
            )
        unit.agent_kernel.update_turn(
            turn_id=target.turn_id,
            status=target.status,
            updated_at=target.updated_at,
            error_code=target.error_code,
            error_message=target.error_message,
            usage=target.usage,
        )
        thread_status = "error" if target.status == "failed" else "idle"
        unit.agent_kernel.update_thread(
            thread_id=target.thread_id,
            status=thread_status,
            summary=None,
            last_turn_id=target.turn_id,
            updated_at=target.updated_at,
        )
        return _applied_result(
            command,
            _require_thread_detail(unit, target.thread_id),
            turn_id=target.turn_id,
        )

    def _replay_items(
        self,
        unit: SQLiteUnitOfWork,
        command: LifecyclePersistenceCommand,
        operation: ReplayItemsPersistenceOperation,
    ) -> LifecyclePersistenceResult:
        thread = _thread_detail(unit, operation.thread_id)
        revision = _thread_revision(thread)
        self._require_optional_read_revision(command, revision)
        if thread is None:
            items: list[AgentItem] = []
            next_sequence = None
        else:
            rows = unit.agent_kernel.list_items(
                operation.thread_id,
                after=operation.after_sequence,
            )[: operation.limit + 1]
            next_sequence = int(rows[-1]["sequence"]) if len(rows) > operation.limit else None
            items = [_item_from_row(row) for row in rows[: operation.limit]]
        return _result(
            command,
            revision=revision,
            outcome=ItemsReplayedPersistenceOutcome(
                outcome="items_replayed",
                thread_id=operation.thread_id,
                items=items,
                next_sequence=next_sequence,
            ),
        )

    @staticmethod
    def _require_cas(
        unit: SQLiteUnitOfWork,
        command: LifecyclePersistenceCommand,
        thread_id: str,
    ) -> AgentThreadDetail:
        current = _thread_detail(unit, thread_id)
        if current is None:
            raise K002PortBoundaryError(
                "K002_PERSISTENCE_THREAD_NOT_FOUND",
                "Lifecycle mutation targets no persisted Thread.",
            )
        if command.expected_revision != _thread_revision(current):
            raise _stale_revision()
        return current


def _list_thread_summaries(
    unit: SQLiteUnitOfWork,
    operation: ListThreadsPersistenceOperation,
) -> list[AgentThreadSummary]:
    clauses: list[str] = []
    parameters: list[Any] = []
    if operation.project_id is not None:
        clauses.append("project_id = ?")
        parameters.append(operation.project_id)
    if not operation.include_archived:
        clauses.append("status != 'archived'")
    where = f"WHERE {' AND '.join(clauses)}" if clauses else ""
    parameters.append(operation.limit)
    rows = unit.require_connection().execute(
        f"""
        SELECT thread_id, project_id, title, status, summary, provider_id,
               created_at, updated_at, last_turn_id
        FROM agent_threads
        {where}
        ORDER BY updated_at DESC, thread_id DESC
        LIMIT ?
        """,
        parameters,
    ).fetchall()
    return [AgentThreadSummary(**dict(row)) for row in rows]


def _thread_detail(
    unit: SQLiteUnitOfWork,
    thread_id: str,
) -> Optional[AgentThreadDetail]:
    row = unit.agent_kernel.get_thread(thread_id)
    if row is None:
        return None
    summary = AgentThreadSummary(**dict(row))
    all_items = [_item_from_row(item) for item in unit.agent_kernel.list_items(thread_id)]
    all_approvals = [
        _approval_from_row(approval) for approval in unit.agent_kernel.list_approvals(thread_id)
    ]
    turns: list[AgentTurn] = []
    for turn_row in unit.agent_kernel.list_turns(thread_id):
        turn_id = str(turn_row["turn_id"])
        turns.append(
            AgentTurn(
                turn_id=turn_id,
                thread_id=str(turn_row["thread_id"]),
                request_text=str(turn_row["request_text"]),
                status=str(turn_row["status"]),
                error_code=turn_row["error_code"],
                error_message=turn_row["error_message"],
                usage=json.loads(str(turn_row["usage_json"])),
                created_at=str(turn_row["created_at"]),
                updated_at=str(turn_row["updated_at"]),
                items=[item for item in all_items if item.turn_id == turn_id],
                approvals=[item for item in all_approvals if item.turn_id == turn_id],
            )
        )
    return AgentThreadDetail(**summary.model_dump(mode="json"), turns=turns)


def _require_thread_detail(unit: SQLiteUnitOfWork, thread_id: str) -> AgentThreadDetail:
    thread = _thread_detail(unit, thread_id)
    if thread is None:
        raise K002PortBoundaryError(
            "K002_PERSISTENCE_THREAD_NOT_FOUND",
            "Persisted lifecycle Thread could not be read back.",
        )
    return thread


def _item_from_row(row: sqlite3.Row) -> AgentItem:
    return AgentItem(
        item_id=str(row["item_id"]),
        thread_id=str(row["thread_id"]),
        turn_id=str(row["turn_id"]),
        sequence=int(row["sequence"]),
        item_type=str(row["item_type"]),
        status=str(row["status"]),
        payload=json.loads(str(row["payload_json"])),
        created_at=str(row["created_at"]),
    )


def _approval_from_row(row: sqlite3.Row) -> AgentApproval:
    return AgentApproval(
        approval_id=str(row["approval_id"]),
        thread_id=str(row["thread_id"]),
        turn_id=str(row["turn_id"]),
        item_id=str(row["item_id"]),
        action=str(row["action"]),
        status=str(row["status"]),
        payload=json.loads(str(row["payload_json"])),
        created_at=str(row["created_at"]),
        resolved_at=row["resolved_at"],
    )


def _get_item_row(unit: SQLiteUnitOfWork, item_id: str) -> Optional[sqlite3.Row]:
    return unit.require_connection().execute(
        """
        SELECT item_id, thread_id, turn_id, sequence, item_type, status,
               payload_json, created_at
        FROM agent_items WHERE item_id = ?
        """,
        (item_id,),
    ).fetchone()


def _thread_revision(thread: Optional[AgentThreadDetail]) -> str:
    payload = None if thread is None else thread.model_dump(mode="json")
    return f"sha256:{canonical_json_sha256(payload)}"


def _collection_revision(payload: Any) -> str:
    return f"sha256:{canonical_json_sha256(payload)}"


def _applied_result(
    command: LifecyclePersistenceCommand,
    thread: AgentThreadDetail,
    *,
    turn_id: str | None = None,
    item_id: str | None = None,
    approval_id: str | None = None,
    sequence: int | None = None,
) -> LifecyclePersistenceResult:
    return _result(
        command,
        revision=_thread_revision(thread),
        outcome=AppliedPersistenceOutcome(
            outcome="applied",
            thread_id=thread.thread_id,
            turn_id=turn_id,
            item_id=item_id,
            approval_id=approval_id,
            sequence=sequence,
        ),
    )


def _result(
    command: LifecyclePersistenceCommand,
    *,
    revision: str,
    outcome: Any,
) -> LifecyclePersistenceResult:
    return LifecyclePersistenceResult(
        command_id=command.command_id,
        revision=revision,
        replayed=False,
        result=outcome,
    )


def _stale_revision() -> K002PortBoundaryError:
    return K002PortBoundaryError(
        "K002_PERSISTENCE_CAS_CONFLICT",
        "Lifecycle expected_revision does not match the canonical persisted Thread.",
        recoverable=True,
    )


def _valid_terminal_transition(current: str, target: str) -> bool:
    return (current, target) in {
        ("queued", "cancelled"),
        ("running", "completed"),
        ("running", "failed"),
        ("running", "cancelled"),
        ("waiting_for_approval", "failed"),
        ("waiting_for_approval", "cancelled"),
        ("waiting_for_clarification", "cancelled"),
    }


def _canonical_json(value: Any) -> str:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    )


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
