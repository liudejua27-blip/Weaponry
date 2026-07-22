from __future__ import annotations

from pathlib import Path

from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import CreateAgentThreadRequest, StartAgentTurnRequest
from forgecad_agent.application.conversation import compile_provider_conversation
from forgecad_agent.application.mechanical_planner import (
    DeterministicMechanicalPlanner,
    MechanicalPlannerConfig,
    MechanicalPlannerTelemetry,
)
from forgecad_agent.infrastructure.db import SQLiteUnitOfWork
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner


ROOT = Path(__file__).resolve().parents[3]


def test_compiler_preserves_history_and_appends_snapshot_digest():
    conversation = compile_provider_conversation(
        prior_items=[
            {"sequence": 1, "item_type": "user_message", "payload": {"text": "设计冰原探索车"}},
            {"sequence": 2, "item_type": "assistant_message", "payload": {"text": "我准备了三个方向。"}},
        ],
        current_request="把座舱更低一些",
        memory_summary=None,
        snapshot={
            "project_id": "prj_test",
            "revision": 4,
            "active_design": {"asset_version_id": "assetver_4"},
            "selected_part_id": "part_cabin",
        },
    )
    assert conversation.messages[0] == {"role": "user", "content": "设计冰原探索车"}
    assert conversation.messages[1] == {"role": "assistant", "content": "我准备了三个方向。"}
    assert '"asset_version_id":"assetver_4"' in conversation.messages[-1]["content"]
    assert '"request":"把座舱更低一些"' in conversation.messages[-1]["content"]


class CapturingPlanner(DeterministicMechanicalPlanner):
    provider_id = "capturing_test_planner"

    def __init__(self) -> None:
        super().__init__()
        self.contexts = []

    def plan_complete_concept(self, *, brief, pack, project_id, conversation=None):  # type: ignore[no-untyped-def]
        self.contexts.append(conversation)
        self.last_call_telemetry = MechanicalPlannerTelemetry(
            latency_ms=8,
            input_tokens=120,
            output_tokens=24,
            total_tokens=144,
            prompt_cache_hit_tokens=90,
            prompt_cache_miss_tokens=30,
        )
        return super().plan_complete_concept(
            brief=brief, pack=pack, project_id=project_id, conversation=conversation
        )


def test_kernel_sends_prior_conversation_and_records_deepseek_usage(tmp_path):
    factory = SQLiteConnectionFactory(tmp_path / "library.db")
    SQLiteMigrationRunner(factory, ROOT / "migrations").run()
    planner = CapturingPlanner()
    service = AgentKernelService(factory, planner=planner)
    thread = service.create_thread(
        CreateAgentThreadRequest(client_request_id="thread", title="车辆"), "idem-thread"
    )
    first = service.start_turn(
        thread.thread_id,
        StartAgentTurnRequest(client_request_id="turn-1", message="设计一辆冰原探索车，完整外观。"),
        "idem-turn-1",
    )
    second = service.start_turn(
        thread.thread_id,
        StartAgentTurnRequest(client_request_id="turn-2", message="把座舱压低，保留完整外观。"),
        "idem-turn-2",
    )
    assert first.status == second.status == "completed"
    context = planner.contexts[-1]
    assert context is not None
    assert any(message["content"] == "设计一辆冰原探索车，完整外观。" for message in context.messages)
    assert any("我已生成一个完整外观意图" in message["content"] for message in context.messages)
    assert second.usage["prompt_cache_hit_tokens"] == 90
    assert second.usage["prompt_cache_miss_tokens"] == 30
    assert second.usage["usage_status"] == "reported"


def test_deepseek_usage_settles_daily_reservation(tmp_path):
    factory = SQLiteConnectionFactory(tmp_path / "library.db")
    SQLiteMigrationRunner(factory, ROOT / "migrations").run()
    planner = CapturingPlanner()
    planner.config = MechanicalPlannerConfig(
        base_url="https://api.deepseek.com", model="deepseek-v4-pro", api_key="not-used"
    )
    service = AgentKernelService(factory, planner=planner)
    thread = service.create_thread(
        CreateAgentThreadRequest(client_request_id="thread", title="车辆"), "idem-thread"
    )
    turn = service.start_turn(
        thread.thread_id,
        StartAgentTurnRequest(client_request_id="turn", message="设计一辆冰原探索车，完整外观。"),
        "idem-turn",
    )
    assert turn.usage["estimated_cost_cny"] == 0.000236
    assert turn.usage["budget_reservation_cny"] == 1.2816
    with SQLiteUnitOfWork(factory) as unit:
        row = unit.require_connection().execute(
            "SELECT spent_micros, reserved_micros, unmetered_turns FROM agent_provider_daily_budgets"
        ).fetchone()
    assert tuple(row) == (236, 0, 0)
