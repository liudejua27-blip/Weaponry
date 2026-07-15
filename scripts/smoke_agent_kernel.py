#!/usr/bin/env python3
"""Deterministic G1 smoke for the ForgeCAD Agent Thread/Turn/Item kernel."""

from __future__ import annotations

import tempfile
from pathlib import Path

from forgecad_agent.application.agent_kernel import AgentKernelIdempotencyConflict, AgentKernelService
from forgecad_agent.application.agent_models import (
    CreateAgentApprovalRequest,
    CreateAgentThreadRequest,
    ResolveAgentApprovalRequest,
    StartAgentTurnRequest,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner


ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="forgecad-agent-kernel-") as raw:
        root = Path(raw)
        factory = SQLiteConnectionFactory(root / "library.db")
        applied = SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        assert "0019" in applied, applied
        service = AgentKernelService(factory)
        provider_check = service.check_provider()
        assert provider_check.status == "not_configured"
        assert provider_check.network_call_made is False

        request = CreateAgentThreadRequest(
            client_request_id="kernel-thread-1",
            title="冰原探索车设计",
            provider_id="deterministic_kernel",
        )
        thread = service.create_thread(request, "idem-thread-1")
        replay = service.create_thread(request, "idem-thread-1")
        assert thread.thread_id == replay.thread_id

        turn_request = StartAgentTurnRequest(
            client_request_id="kernel-turn-1",
            message="设计一辆双座冰原探索车，完整外观，高离地，哑光车身。",
        )
        turn = service.start_turn(thread.thread_id, turn_request, "idem-turn-1")
        assert turn.status == "completed"
        assert len(turn.items) == 5, [item.item_type for item in turn.items]
        assert any(item.item_type == "tool_result" for item in turn.items)
        plan_item = next(item for item in turn.items if item.item_type == "plan")
        assert len(plan_item.payload["directions"]) == 3
        tool_result = next(item for item in turn.items if item.item_type == "tool_result")
        assert tool_result.payload["result"]["domain_pack_id"] == "pack_vehicle_concept"
        events = service.events(thread.thread_id)
        assert [event.sequence for event in events] == list(range(1, len(events) + 1))
        assert service.events(thread.thread_id, after=2)[0].sequence == 3

        approval = service.create_approval(
            thread.thread_id,
            CreateAgentApprovalRequest(
                client_request_id="kernel-approval-1",
                turn_id=turn.turn_id,
                action="commit_shape_program",
                payload={"candidate_id": "candidate_demo"},
            ),
            "idem-approval-1",
        )
        waiting = service.get_thread(thread.thread_id).turns[-1]
        assert approval.status == "pending"
        assert waiting.status == "waiting_for_approval"
        resolved = service.resolve_approval(
            approval.approval_id,
            ResolveAgentApprovalRequest(
                client_request_id="kernel-resolve-1",
                decision="approved",
            ),
            "idem-resolve-1",
        )
        assert resolved.approval.status == "approved"
        assert resolved.turn.status == "completed"

        try:
            service.create_thread(
                CreateAgentThreadRequest(client_request_id="kernel-conflict", title="不同请求"),
                "idem-thread-1",
            )
        except AgentKernelIdempotencyConflict:
            pass
        else:
            raise AssertionError("reused idempotency key must reject a different request")

    print("agent kernel smoke passed: migration, replay, turn, item, approval, SSE cursor")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
