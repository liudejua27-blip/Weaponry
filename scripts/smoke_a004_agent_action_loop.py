#!/usr/bin/env python3
"""FGC-A004 smoke: bounded ForgeCAD Product Tool Action Loop."""

from __future__ import annotations

import json
import tempfile
import threading
import time
import urllib.request
from pathlib import Path
from typing import Any

from forgecad_agent.application.agent_action_loop import (
    AgentActionContext,
    AgentActionLoop,
    AgentActionLoopConfig,
    AgentActionLoopError,
    ProductToolRegistry,
    ProviderActionStep,
    ProviderToolCall,
)
from forgecad_agent.application.agent_kernel import AgentKernelError, AgentKernelService
from forgecad_agent.application.agent_models import CreateAgentThreadRequest, StartAgentTurnRequest
from forgecad_agent.application.domain_packs import domain_pack_by_id
from forgecad_agent.application.geometry_worker import build_blockout
from forgecad_agent.application.mechanical_planner import (
    DeterministicMechanicalPlanner,
    MechanicalPlannerConfig,
    MechanicalPlannerError,
    OpenAICompatibleMechanicalPlanner,
)
from forgecad_agent.application.product_tool_registry import forgecad_product_tool_registry
from forgecad_agent.application.provider_gateway import ProviderConnectionState
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner


ROOT = Path(__file__).resolve().parents[1]


class _JsonResponse:
    headers = {"Content-Type": "application/json"}

    def __init__(self, payload: dict[str, Any]) -> None:
        self.payload = payload

    def __enter__(self) -> "_JsonResponse":
        return self

    def __exit__(self, *args: object) -> None:
        return None

    def read(self) -> bytes:
        return json.dumps(self.payload, ensure_ascii=False).encode("utf-8")


def _step(call_id: str, name: str, arguments: dict[str, Any], secret: str) -> dict[str, Any]:
    return {
        "choices": [
            {
                "message": {
                    "content": "",
                    "reasoning_content": secret,
                    "tool_calls": [
                        {
                            "id": call_id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": json.dumps(arguments, ensure_ascii=False),
                            },
                        }
                    ],
                }
            }
        ],
        "usage": {"prompt_tokens": 10, "completion_tokens": 4, "total_tokens": 14},
    }


def _expect_error(code: str, callback: Any) -> AgentActionLoopError:
    try:
        callback()
    except AgentActionLoopError as exc:
        assert exc.code == code, (exc.code, code)
        return exc
    raise AssertionError(f"expected {code}")


def _direct_failure_boundaries(plan: Any) -> None:
    registry = forgecad_product_tool_registry()

    def one(call: ProviderToolCall) -> Any:
        used = False

        def provider(messages: list[dict[str, Any]], tools: list[dict[str, Any]], event: Any) -> ProviderActionStep:
            nonlocal used
            del messages, tools, event
            if used:
                return ProviderActionStep(content="done")
            used = True
            return ProviderActionStep(tool_calls=[call])

        return provider

    _expect_error(
        "PRODUCT_TOOL_INPUT_SCHEMA_INVALID",
        lambda: AgentActionLoop(registry).run(
            messages=[],
            provider=one(ProviderToolCall(call_id="bad_schema", name="plan_complete_concept", arguments_json="{}")),
            context=AgentActionContext(parent_turn_id="turn_schema"),
        ),
    )

    program = build_blockout(plan, plan.directions[0].direction_id).shape_program
    program = json.loads(json.dumps(program))
    program["operations"][0]["op"] = "shell"
    unsupported = _expect_error(
        "UNSUPPORTED_RUNTIME_OPERATION",
        lambda: AgentActionLoop(registry).run(
            messages=[],
            provider=one(
                ProviderToolCall(
                    call_id="bad_operation",
                    name="validate_shape_program",
                    arguments_json=json.dumps({"shape_program": program}),
                )
            ),
            context=AgentActionContext(parent_turn_id="turn_g819"),
        ),
    )
    assert unsupported.category == "unsupported"

    counter = 0

    def excessive(messages: list[dict[str, Any]], tools: list[dict[str, Any]], event: Any) -> ProviderActionStep:
        nonlocal counter
        del messages, tools, event
        counter += 1
        return ProviderActionStep(
            tool_calls=[
                ProviderToolCall(
                    call_id=f"limit_{counter}",
                    name="infer_product_domain",
                    arguments_json=json.dumps({"brief": "设计一辆汽车"}),
                )
            ]
        )

    _expect_error(
        "ACTION_LOOP_CALL_LIMIT",
        lambda: AgentActionLoop(registry, AgentActionLoopConfig(max_tool_calls=2)).run(
            messages=[],
            provider=excessive,
            context=AgentActionContext(parent_turn_id="turn_limit"),
        ),
    )
    _expect_error(
        "ACTION_LOOP_TOKEN_LIMIT",
        lambda: AgentActionLoop(registry, AgentActionLoopConfig(max_total_tokens=1_000)).run(
            messages=[],
            provider=lambda messages, tools, event: ProviderActionStep(
                content="over budget",
                total_tokens=1_001,
            ),
            context=AgentActionContext(parent_turn_id="turn_token_limit"),
        ),
    )

    duplicate_steps = [
        ProviderActionStep(
            tool_calls=[ProviderToolCall(call_id="same", name="infer_product_domain", arguments_json='{"brief":"设计汽车"}')]
        ),
        ProviderActionStep(
            tool_calls=[ProviderToolCall(call_id="same", name="infer_product_domain", arguments_json='{"brief":"设计汽车"}')]
        ),
    ]
    _expect_error(
        "DUPLICATE_PROVIDER_TOOL_CALL_ID",
        lambda: AgentActionLoop(registry).run(
            messages=[],
            provider=lambda messages, tools, event: duplicate_steps.pop(0),
            context=AgentActionContext(parent_turn_id="turn_duplicate"),
        ),
    )

    cancelled = threading.Event()
    cancelled.set()
    _expect_error(
        "AGENT_ACTION_CANCELLED",
        lambda: AgentActionLoop(registry).run(
            messages=[],
            provider=lambda messages, tools, event: ProviderActionStep(content="never"),
            context=AgentActionContext(parent_turn_id="turn_cancel", cancel_event=cancelled),
        ),
    )

    def slow(messages: list[dict[str, Any]], tools: list[dict[str, Any]], event: Any) -> ProviderActionStep:
        del messages, tools, event
        time.sleep(1.02)
        return ProviderActionStep(content="late")

    _expect_error(
        "ACTION_LOOP_TIMEOUT",
        lambda: AgentActionLoop(registry, AgentActionLoopConfig(max_wall_seconds=1)).run(
            messages=[],
            provider=slow,
            context=AgentActionContext(parent_turn_id="turn_timeout"),
        ),
    )
    _expect_error(
        "PROVIDER_ACTION_DISCONNECTED",
        lambda: AgentActionLoop(registry).run(
            messages=[],
            provider=lambda messages, tools, event: (_ for _ in ()).throw(OSError("disconnect")),
            context=AgentActionContext(parent_turn_id="turn_disconnect"),
        ),
    )
    _expect_error(
        "STALE_ACTIVE_DESIGN_SNAPSHOT",
        lambda: AgentActionLoop(registry).run(
            messages=[],
            provider=lambda messages, tools, event: ProviderActionStep(content="never"),
            context=AgentActionContext(
                parent_turn_id="turn_stale",
                expected_snapshot_fingerprint="snapshot_a",
                current_snapshot_fingerprint="snapshot_b",
            ),
        ),
    )
    duplicate_tool = registry.require("infer_product_domain")
    _expect_error(
        "DUPLICATE_PRODUCT_TOOL",
        lambda: ProductToolRegistry((duplicate_tool, duplicate_tool)),
    )


def _deepseek_roundtrip(plan: Any) -> None:
    test_secret = "".join(("not", "-", "a", "-", "real", "-", "key"))
    responses = [
        _step("deepseek_plan", "plan_complete_concept", {"plan": plan.model_dump(mode="json")}, "private-plan"),
        _step(
            "deepseek_build",
            "build_candidate_geometry",
            {"direction_id": plan.directions[0].direction_id, "presentation_profile": "quick_sketch"},
            "private-build",
        ),
        _step("deepseek_readback", "compile_readback_candidate", {}, "private-readback"),
        _step("deepseek_render", "render_candidate_views", {}, "private-render"),
        _step("deepseek_evaluate", "evaluate_candidate", {}, "private-evaluate"),
        _step("deepseek_preview", "prepare_candidate_preview", {}, "private-preview"),
        {
            "choices": [{"message": {"content": "候选已完成真实回读与四视图检查。", "reasoning_content": "private-final"}}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 4, "total_tokens": 14},
        },
    ]
    requests: list[dict[str, Any]] = []
    original = urllib.request.urlopen

    def fake_urlopen(request: Any, timeout: float) -> _JsonResponse:
        assert timeout == 10
        payload = json.loads(request.data.decode("utf-8"))
        requests.append(payload)
        if len(requests) > 1:
            previous_secrets = [
                message.get("reasoning_content")
                for message in payload["messages"]
                if message.get("role") == "assistant" and message.get("reasoning_content")
            ]
            assert previous_secrets, "DeepSeek reasoning_content was not round-tripped"
        return _JsonResponse(responses.pop(0))

    urllib.request.urlopen = fake_urlopen
    events: list[Any] = []
    try:
        planner = OpenAICompatibleMechanicalPlanner(
            MechanicalPlannerConfig(
                base_url="https://api.deepseek.com",
                model="deepseek-chat",
                api_key=test_secret,
                timeout_seconds=10,
            )
        )
        result = planner.plan_complete_concept(
            brief=plan.brief,
            pack=domain_pack_by_id(plan.domain_pack_id),
            project_id="project_a004",
            action_observer=events.append,
        )
    finally:
        urllib.request.urlopen = original
    assert result.provider_id == "openai_compatible_mechanical_planner"
    assert len(requests) == 7
    assert all(len(request["tools"]) == 13 for request in requests)
    tool_names = [event.tool_name for event in events if event.event_kind == "tool_call"]
    assert tool_names == [
        "plan_complete_concept",
        "build_candidate_geometry",
        "compile_readback_candidate",
        "render_candidate_views",
        "evaluate_candidate",
        "prepare_candidate_preview",
    ]
    serialized = json.dumps([event.model_dump(mode="json") for event in events], ensure_ascii=False)
    assert "private-plan" not in serialized and "private-final" not in serialized
    assert events[-1].result["permanent_side_effects"] == 0

    probe_requests: list[dict[str, Any]] = []

    def fake_probe(request: Any, timeout: float) -> _JsonResponse:
        assert timeout == 10
        probe_requests.append(json.loads(request.data.decode("utf-8")))
        return _JsonResponse(
            {
                "choices": [{"message": {"content": json.dumps(plan.model_dump(mode="json"), ensure_ascii=False)}}],
                "usage": {"prompt_tokens": 10, "completion_tokens": 4, "total_tokens": 14},
            }
        )

    urllib.request.urlopen = fake_probe
    try:
        OpenAICompatibleMechanicalPlanner(
            MechanicalPlannerConfig(
                base_url="https://api.deepseek.com",
                model="deepseek-chat",
                api_key=test_secret,
                timeout_seconds=10,
            )
        ).plan_complete_concept(
            brief=plan.brief,
            pack=domain_pack_by_id(plan.domain_pack_id),
            project_id=None,
            action_loop_enabled=False,
        )
    finally:
        urllib.request.urlopen = original
    assert len(probe_requests) == 1
    assert "tools" not in probe_requests[0], "provider:check must not expand into the paid Action Loop"


class _FailingPlanner:
    provider_id = "failing_test_provider"
    model_name = None
    last_call_telemetry = None
    last_execution_trace = None

    def connection_state(self) -> ProviderConnectionState:
        return ProviderConnectionState(
            status="failed",
            provider_id=self.provider_id,
            configured=True,
            metadata_status="valid",
            secret_status="available",
            supervisor_status="not_checked",
            capability_status="unavailable",
            failure_code="PROVIDER_NETWORK_ERROR",
            message="test provider unavailable",
        )

    def plan_complete_concept(self, **kwargs: Any) -> Any:
        del kwargs
        raise MechanicalPlannerError(
            "PROVIDER_NETWORK_ERROR",
            "Provider disconnected during test.",
            recoverable=True,
            network_call_made=True,
        )


def _restart_and_zero_side_effects() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad-a004-") as raw:
        factory = SQLiteConnectionFactory(Path(raw) / "library.db")
        SQLiteMigrationRunner(factory, ROOT / "migrations").run()
        service = AgentKernelService(factory)
        thread = service.create_thread(
            CreateAgentThreadRequest(client_request_id="a004-thread", title="A004 lifecycle"),
            "idem-a004-thread",
        )
        turn = service.start_turn(
            thread.thread_id,
            StartAgentTurnRequest(client_request_id="a004-turn", message="设计一辆紧凑的未来汽车概念外观"),
            "idem-a004-turn",
        )
        assert turn.status == "completed"
        connection = factory.connect()
        try:
            assert connection.execute("SELECT COUNT(*) FROM agent_asset_versions").fetchone()[0] == 0
            assert connection.execute("SELECT COUNT(*) FROM agent_asset_change_sets").fetchone()[0] == 0
            assert connection.execute("SELECT COUNT(*) FROM active_design_snapshots").fetchone()[0] == 0
        finally:
            connection.close()
        restarted = AgentKernelService(factory).get_thread(thread.thread_id)
        assert restarted.turns[-1].status == "completed"
        assert any(item.payload.get("tool_name") == "compile_readback_candidate" for item in restarted.turns[-1].items)

        failed_service = AgentKernelService(factory, planner=_FailingPlanner())
        failed_thread = failed_service.create_thread(
            CreateAgentThreadRequest(client_request_id="a004-failed-thread", title="A004 failed"),
            "idem-a004-failed-thread",
        )
        try:
            failed_service.start_turn(
                failed_thread.thread_id,
                StartAgentTurnRequest(client_request_id="a004-failed-turn", message="设计一辆未来汽车概念外观"),
                "idem-a004-failed-turn",
            )
        except AgentKernelError as exc:
            assert exc.code == "PROVIDER_NETWORK_ERROR"
        else:
            raise AssertionError("failed Provider Turn must not report success")
        failed_restarted = AgentKernelService(factory).get_thread(failed_thread.thread_id)
        assert failed_restarted.turns[-1].status == "failed"
        assert failed_restarted.turns[-1].error_code == "PROVIDER_NETWORK_ERROR"


def main() -> int:
    registry = forgecad_product_tool_registry()
    manifest = registry.public_manifest()
    assert manifest.schema_version == "ForgeCADProductToolRegistry@1"
    assert len(manifest.tools) == 13
    serialized_manifest = json.dumps(manifest.model_dump(mode="json"), ensure_ascii=False).casefold()
    for forbidden in ("shell", "python", "javascript", "arbitrary_url", "file_path", "database", "mcp"):
        assert f'"name":"{forbidden}"' not in serialized_manifest.replace(" ", "")

    plan = DeterministicMechanicalPlanner().plan_complete_concept(
        brief="设计一辆紧凑的未来汽车概念外观",
        pack=domain_pack_by_id("pack_vehicle_concept"),
        project_id="project_a004_fixture",
    )
    _direct_failure_boundaries(plan)
    _deepseek_roundtrip(plan)
    _restart_and_zero_side_effects()
    print(
        "A004 Agent Action Loop smoke passed: product registry, DeepSeek reasoning roundtrip, "
        "real readback/render, limits, cancellation, timeout, conflicts, restart, zero permanent side effects"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
