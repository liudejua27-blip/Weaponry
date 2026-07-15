#!/usr/bin/env python3
"""Focused D003 smoke for persisted domain clarification items."""

from __future__ import annotations

import tempfile
from pathlib import Path
from shutil import copy2

from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import CreateAgentThreadRequest, StartAgentTurnRequest
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner


ROOT = Path(__file__).resolve().parents[1]


def _count(factory: SQLiteConnectionFactory, table: str) -> int:
    connection = factory.connect()
    try:
        return int(connection.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0])
    finally:
        connection.close()


def main() -> int:
    _migration_upgrade_preserves_kernel_rows()
    with tempfile.TemporaryDirectory(prefix="forgecad-d003-clarification-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        service = AgentKernelService(factory)
        thread = service.create_thread(
            CreateAgentThreadRequest(client_request_id="d003-thread"),
            "d003-thread",
        )
        before = {table: _count(factory, table) for table in ("agent_blockout_candidates", "agent_asset_versions")}
        turn = service.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="d003-turn", message="我想做一个海洋生物雕塑"),
            "d003-turn",
        )
        assert turn.status == "waiting_for_clarification"
        clarification = next(item for item in turn.items if item.item_type == "clarification")
        assert clarification.payload["kind"] == "domain"
        assert clarification.payload["status"] == "unsupported"
        assert isinstance(clarification.payload["question"], str)
        assert len(clarification.payload["options"]) == 4
        assert all(option["prompt"] for option in clarification.payload["options"])
        replay = service.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="d003-turn", message="我想做一个海洋生物雕塑"),
            "d003-turn",
        )
        assert replay.turn_id == turn.turn_id
        continued = service.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(
                client_request_id="d003-continue",
                message="设计一台能飞的无人机载具\n我想设计一架飞机或未来航空器，先做完整外观概念。",
                clarification_domain_pack_id="pack_aircraft_concept",
            ),
            "d003-continue",
        )
        assert continued.status == "completed"
        plan_item = next(item for item in continued.items if item.item_type == "plan")
        assert plan_item.payload["domain_pack_id"] == "pack_aircraft_concept"
        assert before == {table: _count(factory, table) for table in before}

    print("D003 clarification smoke passed: one question, four choices, idempotent replay, zero asset writes")
    return 0


def _migration_upgrade_preserves_kernel_rows() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad-d003-migration-") as raw:
        root = Path(raw)
        old_migrations = root / "migrations-before-d003"
        old_migrations.mkdir()
        for migration in (ROOT / "migrations").glob("*.sql"):
            version = int(migration.name.split("_", 1)[0])
            if version < 27:
                copy2(migration, old_migrations / migration.name)
        factory = SQLiteConnectionFactory(root / "library.db")
        SQLiteMigrationRunner(factory, old_migrations).run()
        connection = factory.connect()
        try:
            connection.execute(
                "INSERT INTO agent_threads(thread_id, title, status, created_at, updated_at, last_turn_id) VALUES (?, ?, ?, ?, ?, ?)",
                ("thread_d003_old", "D003 migration fixture", "idle", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z", "turn_d003_old"),
            )
            connection.execute(
                "INSERT INTO agent_turns(turn_id, thread_id, request_text, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
                ("turn_d003_old", "thread_d003_old", "设计一辆未来汽车", "completed", "2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"),
            )
            connection.execute(
                "INSERT INTO agent_items(item_id, thread_id, turn_id, sequence, item_type, status, payload_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                ("item_d003_old", "thread_d003_old", "turn_d003_old", 1, "approval_request", "completed", "{}", "2026-01-01T00:00:00Z"),
            )
            connection.execute(
                "INSERT INTO agent_approvals(approval_id, thread_id, turn_id, item_id, action, status, payload_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                ("approval_d003_old", "thread_d003_old", "turn_d003_old", "item_d003_old", "commit_shape_program", "pending", "{}", "2026-01-01T00:00:00Z"),
            )
            connection.commit()
        finally:
            connection.close()
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        restored = AgentKernelService(factory).get_thread("thread_d003_old")
        assert restored.turns[0].turn_id == "turn_d003_old"
        assert restored.turns[0].items
        assert restored.turns[0].approvals[0].approval_id == "approval_d003_old"
        connection = factory.connect()
        try:
            assert connection.execute("PRAGMA foreign_key_check").fetchall() == []
        finally:
            connection.close()


if __name__ == "__main__":
    raise SystemExit(main())
