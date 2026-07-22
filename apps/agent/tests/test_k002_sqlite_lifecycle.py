from __future__ import annotations

from pathlib import Path

import pytest

from forgecad_agent.application.k002_port_security import (
    K002PortBoundaryError,
    canonical_json_sha256,
)
from forgecad_agent.application.k002_python_ports import LifecyclePersistencePort
from forgecad_agent.application.k002_sqlite_lifecycle import K002SQLiteLifecycleBackend
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner


MIGRATIONS_DIR = Path(__file__).resolve().parents[3] / "migrations"


def _factory(tmp_path: Path) -> SQLiteConnectionFactory:
    factory = SQLiteConnectionFactory(tmp_path / "forgecad.db")
    SQLiteMigrationRunner(factory, MIGRATIONS_DIR).run()
    return factory


def _key(label: str) -> str:
    return canonical_json_sha256({"test": label})


def _command(
    label: str,
    operation: dict,
    *,
    expected_revision: str | None = None,
    idempotency_key: str | None = None,
) -> dict:
    payload = {
        "schema_version": "LifecyclePersistenceCommand@1",
        "command_id": f"command_{label}",
        "idempotency_key": idempotency_key or _key(label),
        "command": operation,
    }
    if expected_revision is not None:
        payload["expected_revision"] = expected_revision
    return payload


def _thread(thread_id: str, *, project_id: str = "project_k002") -> dict:
    return {
        "thread_id": thread_id,
        "project_id": project_id,
        "title": "K002 lifecycle",
        "status": "idle",
        "summary": "",
        "provider_id": "deepseek",
        "created_at": "2026-07-17T00:00:00Z",
        "updated_at": "2026-07-17T00:00:00Z",
    }


def _turn(thread_id: str, turn_id: str, *, created_at: str) -> dict:
    return {
        "turn_id": turn_id,
        "thread_id": thread_id,
        "request_text": "Generate one bounded concept",
        "status": "running",
        "usage": {},
        "created_at": created_at,
        "updated_at": created_at,
        "items": [],
        "approvals": [],
    }


def _create_thread_and_turn(
    port: LifecyclePersistencePort,
    *,
    suffix: str,
) -> tuple[str, str, str]:
    thread_id = f"thread_{suffix}"
    turn_id = f"turn_{suffix}_1"
    created = port.execute(
        _command(
            f"create_thread_{suffix}",
            {"operation": "create_thread", "thread": _thread(thread_id)},
        )
    )
    started = port.execute(
        _command(
            f"create_turn_{suffix}",
            {
                "operation": "create_turn",
                "thread_id": thread_id,
                "turn": _turn(thread_id, turn_id, created_at="2026-07-17T00:00:01Z"),
            },
            expected_revision=created.revision,
        )
    )
    return thread_id, turn_id, started.revision


def _load(
    port: LifecyclePersistencePort,
    thread_id: str,
    *,
    label: str,
) -> object:
    return port.execute(
        _command(
            label,
            {"operation": "load_thread", "thread_id": thread_id},
        )
    )


def test_sqlite_backend_exact_replay_conflict_cas_and_restart(tmp_path: Path) -> None:
    factory = _factory(tmp_path)
    port = LifecyclePersistencePort(K002SQLiteLifecycleBackend(factory), environment={})
    create_command = _command(
        "create_thread_replay",
        {"operation": "create_thread", "thread": _thread("thread_replay")},
    )

    created = port.execute(create_command)
    reconnect_command = dict(create_command)
    reconnect_command["command_id"] = "command_create_thread_reconnect"
    replayed = port.execute(reconnect_command)

    assert created.replayed is False
    assert replayed.replayed is True
    assert replayed.command_id == reconnect_command["command_id"]
    assert replayed.revision == created.revision
    assert replayed.result == created.result

    conflicting = _command(
        "different_command_id",
        {
            "operation": "create_thread",
            "thread": _thread("thread_other"),
        },
        idempotency_key=create_command["idempotency_key"],
    )
    with pytest.raises(K002PortBoundaryError) as conflict:
        port.execute(conflicting)
    assert conflict.value.code == "K002_PERSISTENCE_IDEMPOTENCY_CONFLICT"

    turn = _turn("thread_replay", "turn_replay_1", created_at="2026-07-17T00:00:01Z")
    stale_key = _key("stale_then_retry")
    stale = _command(
        "stale_create_turn",
        {"operation": "create_turn", "thread_id": "thread_replay", "turn": turn},
        expected_revision="sha256:" + "0" * 64,
        idempotency_key=stale_key,
    )
    with pytest.raises(K002PortBoundaryError) as stale_error:
        port.execute(stale)
    assert stale_error.value.code == "K002_PERSISTENCE_CAS_CONFLICT"

    retried = _command(
        "stale_create_turn",
        {"operation": "create_turn", "thread_id": "thread_replay", "turn": turn},
        expected_revision=created.revision,
        idempotency_key=stale_key,
    )
    started = port.execute(retried)
    assert started.replayed is False

    restarted = LifecyclePersistencePort(K002SQLiteLifecycleBackend(factory), environment={})
    loaded = _load(restarted, "thread_replay", label="load_after_restart")
    assert loaded.revision == started.revision
    assert loaded.result.thread.turns[0].status == "running"


def test_sqlite_backend_thread_scoped_items_load_list_and_restart_replay(
    tmp_path: Path,
) -> None:
    factory = _factory(tmp_path)
    port = LifecyclePersistencePort(K002SQLiteLifecycleBackend(factory), environment={})
    thread_id, first_turn_id, revision = _create_thread_and_turn(port, suffix="stream")

    first_item = {
        "item_id": "item_stream_1",
        "thread_id": thread_id,
        "turn_id": first_turn_id,
        "sequence": 1,
        "item_type": "user_message",
        "status": "completed",
        "payload": {"text": "first"},
        "created_at": "2026-07-17T00:00:02Z",
    }
    appended = port.execute(
        _command(
            "append_stream_1",
            {
                "operation": "append_item",
                "item": first_item,
                "expected_previous_sequence": 0,
            },
            expected_revision=revision,
        )
    )
    loaded = _load(port, thread_id, label="load_stream_before_terminal")
    current_turn = loaded.result.thread.turns[0].model_dump(mode="json")
    current_turn.update(
        {
            "status": "completed",
            "usage": {"input_tokens": 4, "output_tokens": 2},
            "updated_at": "2026-07-17T00:00:03Z",
        }
    )
    terminal = port.execute(
        _command(
            "terminal_stream_1",
            {"operation": "set_turn_terminal", "turn": current_turn},
            expected_revision=loaded.revision,
        )
    )

    second_turn_id = "turn_stream_2"
    second_turn = port.execute(
        _command(
            "create_turn_stream_2",
            {
                "operation": "create_turn",
                "thread_id": thread_id,
                "turn": _turn(
                    thread_id,
                    second_turn_id,
                    created_at="2026-07-17T00:00:04Z",
                ),
            },
            expected_revision=terminal.revision,
        )
    )
    second_item = {
        "item_id": "item_stream_2",
        "thread_id": thread_id,
        "turn_id": second_turn_id,
        "sequence": 2,
        "item_type": "tool_call",
        "status": "completed",
        "payload": {"tool_id": "forgecad.domain.inference.v1"},
        "created_at": "2026-07-17T00:00:05Z",
    }
    port.execute(
        _command(
            "append_stream_2",
            {
                "operation": "append_item",
                "item": second_item,
                "expected_previous_sequence": 1,
            },
            expected_revision=second_turn.revision,
        )
    )

    page_one = port.execute(
        _command(
            "replay_stream_page_1",
            {
                "operation": "replay_items",
                "thread_id": thread_id,
                "after_sequence": 0,
                "limit": 1,
            },
        )
    )
    assert [item.sequence for item in page_one.result.items] == [1]
    assert page_one.result.next_sequence == 2
    page_two = port.execute(
        _command(
            "replay_stream_page_2",
            {
                "operation": "replay_items",
                "thread_id": thread_id,
                "after_sequence": 1,
                "limit": 10,
            },
        )
    )
    assert [item.sequence for item in page_two.result.items] == [2]
    assert page_two.result.items[0].turn_id == second_turn_id

    listed = port.execute(
        _command(
            "list_stream_project",
            {
                "operation": "list_threads",
                "project_id": "project_k002",
                "include_archived": False,
                "limit": 20,
            },
        )
    )
    assert [thread.thread_id for thread in listed.result.threads] == [thread_id]

    restarted = LifecyclePersistencePort(K002SQLiteLifecycleBackend(factory), environment={})
    restart_page = restarted.execute(
        _command(
            "replay_stream_after_restart",
            {
                "operation": "replay_items",
                "thread_id": thread_id,
                "after_sequence": 0,
                "limit": 20,
            },
        )
    )
    assert [item.model_dump(mode="json") for item in restart_page.result.items] == [
        first_item,
        second_item,
    ]
    assert restart_page.revision == page_two.revision
    assert appended.revision != restart_page.revision


def test_sqlite_backend_approval_derives_turn_item_and_thread_state(tmp_path: Path) -> None:
    factory = _factory(tmp_path)
    port = LifecyclePersistencePort(K002SQLiteLifecycleBackend(factory), environment={})
    thread_id, turn_id, revision = _create_thread_and_turn(port, suffix="approval")
    approval_item = {
        "item_id": "item_approval_1",
        "thread_id": thread_id,
        "turn_id": turn_id,
        "sequence": 1,
        "item_type": "approval_request",
        "status": "pending",
        "payload": {"action": "confirm_preview"},
        "created_at": "2026-07-17T00:00:02Z",
    }
    appended = port.execute(
        _command(
            "append_approval_item",
            {
                "operation": "append_item",
                "item": approval_item,
                "expected_previous_sequence": 0,
            },
            expected_revision=revision,
        )
    )
    pending_approval = {
        "approval_id": "approval_k002_1",
        "thread_id": thread_id,
        "turn_id": turn_id,
        "item_id": approval_item["item_id"],
        "action": "confirm_preview",
        "status": "pending",
        "payload": {"permanent_side_effects": 0},
        "created_at": "2026-07-17T00:00:03Z",
    }
    created = port.execute(
        _command(
            "create_approval",
            {"operation": "create_approval", "approval": pending_approval},
            expected_revision=appended.revision,
        )
    )
    pending = _load(port, thread_id, label="load_pending_approval")
    assert pending.result.thread.status == "active"
    assert pending.result.thread.turns[0].status == "waiting_for_approval"

    resolved_approval = dict(pending_approval)
    resolved_approval.update(
        {"status": "approved", "resolved_at": "2026-07-17T00:00:04Z"}
    )
    resolved_command = _command(
        "resolve_approval",
        {"operation": "resolve_approval", "approval": resolved_approval},
        expected_revision=created.revision,
    )
    resolved = port.execute(resolved_command)
    assert resolved.result.sequence == 1

    final = _load(port, thread_id, label="load_resolved_approval")
    turn = final.result.thread.turns[0]
    assert final.result.thread.status == "active"
    assert turn.status == "running"
    assert turn.items[0].status == "completed"
    assert turn.approvals[0].status == "approved"

    restarted = LifecyclePersistencePort(K002SQLiteLifecycleBackend(factory), environment={})
    replay = restarted.execute(resolved_command)
    assert replay.replayed is True
    assert replay.revision == resolved.revision


def test_sqlite_backend_terminal_fields_archive_and_restart_visibility(tmp_path: Path) -> None:
    factory = _factory(tmp_path)
    port = LifecyclePersistencePort(K002SQLiteLifecycleBackend(factory), environment={})
    thread_id, turn_id, revision = _create_thread_and_turn(port, suffix="archive")
    loaded = _load(port, thread_id, label="load_before_failed_terminal")
    target_turn = loaded.result.thread.turns[0].model_dump(mode="json")
    terminal_usage = {
        "provider_requests": 1,
        "input_tokens": 7,
        "output_tokens": 0,
        "network_call_made": True,
        "outcome": "failed",
        "error_code": "PROVIDER_TIMEOUT",
        "redacted_trace": {
            "schema_version": "RedactedAgentTrace@1",
            "execution_id": "execution_timeout",
            "context_digest": "a" * 64,
            "entries": [
                {
                    "sequence": 1,
                    "phase": "provider",
                    "event": "failed",
                    "elapsed_ms": 100,
                    "error_code": "PROVIDER_TIMEOUT",
                    "provider_failure_category": "timeout",
                    "input_tokens": 7,
                    "output_tokens": 0,
                    "estimated_cost_microusd": 1,
                    "network_call_made": True,
                }
            ],
        },
    }
    target_turn.update(
        {
            "status": "failed",
            "error_code": "PROVIDER_TIMEOUT",
            "error_message": "Provider timed out.",
            "usage": terminal_usage,
            "updated_at": "2026-07-17T00:00:02Z",
        }
    )
    terminal = port.execute(
        _command(
            "set_failed_terminal",
            {"operation": "set_turn_terminal", "turn": target_turn},
            expected_revision=revision,
        )
    )
    failed = _load(port, thread_id, label="load_failed_terminal")
    assert failed.result.thread.status == "error"
    assert failed.result.thread.turns[0].error_code == "PROVIDER_TIMEOUT"
    assert failed.result.thread.turns[0].usage == terminal_usage

    archived_summary = failed.result.thread.model_dump(mode="json", exclude={"turns"})
    archived_summary.update(
        {"status": "archived", "updated_at": "2026-07-17T00:00:03Z"}
    )
    archived = port.execute(
        _command(
            "archive_thread",
            {"operation": "archive_thread", "thread": archived_summary},
            expected_revision=terminal.revision,
        )
    )
    assert archived.result.thread_id == thread_id

    restarted = LifecyclePersistencePort(K002SQLiteLifecycleBackend(factory), environment={})
    hidden = restarted.execute(
        _command(
            "list_without_archived",
            {
                "operation": "list_threads",
                "project_id": "project_k002",
                "include_archived": False,
                "limit": 20,
            },
        )
    )
    assert hidden.result.threads == []
    visible = restarted.execute(
        _command(
            "list_with_archived",
            {
                "operation": "list_threads",
                "project_id": "project_k002",
                "include_archived": True,
                "limit": 20,
            },
        )
    )
    assert visible.result.threads[0].status == "archived"
    assert visible.result.threads[0].last_turn_id == turn_id
    restarted_detail = _load(restarted, thread_id, label="load_archived_trace_restart")
    assert restarted_detail.result.thread.turns[0].usage == terminal_usage


def test_lifecycle_port_rejects_reasoning_secrets_and_provider_environment_before_sqlite(
    tmp_path: Path,
) -> None:
    factory = _factory(tmp_path)
    backend = K002SQLiteLifecycleBackend(factory)
    port = LifecyclePersistencePort(backend, environment={})
    thread_id, turn_id, revision = _create_thread_and_turn(port, suffix="security")
    forbidden_item = {
        "item_id": "item_security_1",
        "thread_id": thread_id,
        "turn_id": turn_id,
        "sequence": 1,
        "item_type": "assistant_message",
        "status": "completed",
        "payload": {"nested": {"reasoning_content": "private"}},
        "created_at": "2026-07-17T00:00:02Z",
    }
    with pytest.raises(K002PortBoundaryError):
        port.execute(
            _command(
                "forbidden_reasoning",
                {
                    "operation": "append_item",
                    "item": forbidden_item,
                    "expected_previous_sequence": 0,
                },
                expected_revision=revision,
            )
        )
    assert _load(port, thread_id, label="load_after_forbidden").result.thread.turns[0].items == []

    secret_port = LifecyclePersistencePort(
        backend,
        environment={"FORGECAD_AGENT_API_KEY": "must-not-enter-python"},
    )
    with pytest.raises(K002PortBoundaryError) as error:
        _load(secret_port, thread_id, label="load_with_secret_environment")
    assert error.value.code == "K002_PROVIDER_ENVIRONMENT_FORBIDDEN"
