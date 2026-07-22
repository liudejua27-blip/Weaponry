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
        assert len(turn.items) == 16, [item.item_type for item in turn.items]
        provider_result = next(
            item
            for item in turn.items
            if item.item_type == "tool_result" and item.payload.get("tool") == "provider_gateway"
        )
        provider_trace = provider_result.payload["provider_execution_trace"]
        assert provider_trace["phase"] == "completed"
        assert provider_trace["network_call_made"] is False
        assert provider_trace["attempt"] == 1
        plan_item = next(item for item in turn.items if item.item_type == "plan")
        assert len(plan_item.payload["directions"]) == 1
        action_calls = [
            item.payload["tool_name"]
            for item in turn.items
            if item.item_type == "tool_call" and item.payload.get("schema_version") == "AgentActionToolEvent@1"
        ]
        assert action_calls == [
            "plan_complete_concept",
            "build_candidate_geometry",
            "compile_readback_candidate",
            "render_candidate_views",
            "evaluate_candidate",
            "prepare_candidate_preview",
        ], action_calls
        preview_result = next(
            item
            for item in turn.items
            if item.item_type == "tool_result" and item.payload.get("tool_name") == "prepare_candidate_preview"
        )
        assert preview_result.payload["result"]["permanent_side_effects"] == 0
        assert preview_result.payload["result"]["requires_user_confirmation"] is True
        assert all("reasoning_content" not in str(item.payload) for item in turn.items)
        events = service.events(thread.thread_id)
        assert [event.sequence for event in events] == list(range(1, len(events) + 1))
        assert service.events(thread.thread_id, after=2)[0].sequence == 3
        restarted = AgentKernelService(factory).get_thread(thread.thread_id)
        assert restarted.turns[-1].status == "completed"
        assert len(restarted.turns[-1].items) == len(turn.items)

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
