#!/usr/bin/env python3
from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Optional

from forgecad_agent.application.concept_change_sets import (
    ConceptChangeSetError,
    ConceptChangeSetService,
)
from forgecad_agent.application.concept_models import PlanDesignChangeSetRequest
from forgecad_agent.application.concept_planner import (
    ConceptChangeOperationPlan,
    ConceptChangePlan,
    ConceptPlannerError,
    OpenAICompatibleConceptPlanner,
    OpenAICompatibleConceptPlannerConfig,
)
from forgecad_agent.domain.concepts.models import ModuleGraph, WeaponConceptSpec
from smoke_r2_concept_contracts import _concept_spec
from smoke_r2_concept_projects import _assert
from smoke_r2_module_registry import _graph


class _ChangePlannerHandler(BaseHTTPRequestHandler):
    requests: list[dict[str, Any]] = []

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API.
        length = int(self.headers.get("Content-Length", "0"))
        payload = json.loads(self.rfile.read(length).decode("utf-8"))
        self.__class__.requests.append(payload)
        content = {
            "summary": "替换前部注册模块并调整展示比例。",
            "operations": [
                _operation(
                    op="replace_module",
                    node_id="node_front",
                    module_id="module_front_shell_02",
                    rationale="使用同类别注册模块。",
                ),
                _operation(
                    op="set_parameter",
                    path="proportions.overall_length_mm",
                    value=218.0,
                    rationale="采用明确的视觉总长。",
                ),
                _operation(
                    op="set_style",
                    path="style.detail_density",
                    value=0.84,
                    rationale="提高展示细节密度。",
                ),
            ],
            "rationale": [
                "只引用当前 Graph 与注册模块。",
                "确认前保持为 ghost preview。",
            ],
        }
        body = json.dumps(
            {
                "choices": [{"message": {"content": json.dumps(content, ensure_ascii=False)}}],
                "usage": {
                    "input_tokens": 140,
                    "output_tokens": 45,
                    "total_tokens": 185,
                },
            }
        ).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, _format: str, *_args: Any) -> None:
        return


class _FailingChangePlanner:
    provider_id = "failing_change_planner"
    provider_type = "openai_compatible"
    model_name: Optional[str] = "failing-change-model"

    def plan_change_set(self, **_kwargs: Any) -> ConceptChangePlan:
        raise ConceptPlannerError("PLANNER_TIMEOUT", "synthetic change timeout")


class _FixedChangePlanner:
    provider_id = "fixed_change_planner"
    provider_type = "openai_compatible"
    model_name: Optional[str] = "fixed-change-model"

    def __init__(self, plan: ConceptChangePlan) -> None:
        self.plan = plan

    def plan_change_set(self, **_kwargs: Any) -> ConceptChangePlan:
        return self.plan


def main() -> int:
    server = ThreadingHTTPServer(("127.0.0.1", 0), _ChangePlannerHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        provider = OpenAICompatibleConceptPlanner(
            OpenAICompatibleConceptPlannerConfig(
                base_url=f"http://127.0.0.1:{server.server_port}/v1",
                model="fake-change-model",
                api_key="fake-secret",
                timeout_seconds=5,
            )
        )
        spec = WeaponConceptSpec.model_validate(_concept_spec())
        graph = ModuleGraph.model_validate(_graph(spec.project_id))
        catalog = _catalog()
        instruction = (
            "将选中前部替换为候选模块，整体长度调整为 218 mm，"
            "细节密度调整为 84%。"
        )
        plan = provider.plan_change_set(
            instruction=instruction,
            current_spec=spec,
            base_graph=graph,
            module_catalog=catalog,
            selected_node_id="node_front",
            selected_module_id="module_front_shell_02",
        )
        _assert(len(plan.operations) == 3, "provider change operation count mismatch")
        _assert(
            [item.op for item in plan.operations]
            == ["replace_module", "set_parameter", "set_style"],
            "provider change operations mismatch",
        )
        _assert(len(_ChangePlannerHandler.requests) == 1, "change provider request count mismatch")
        _assert(
            provider.last_call_metrics is not None
            and provider.last_call_metrics.latency_ms >= 0
            and provider.last_call_metrics.input_tokens == 140
            and provider.last_call_metrics.output_tokens == 45
            and provider.last_call_metrics.total_tokens == 185,
            "Change Planner usage telemetry mismatch",
        )
        request = _ChangePlannerHandler.requests[0]
        _assert(
            request["response_format"]["type"] == "json_schema"
            and request["response_format"]["json_schema"]["strict"] is True,
            "change provider strict JSON Schema missing",
        )
        schema = request["response_format"]["json_schema"]["schema"]
        _assert(
            set(schema["properties"]).issubset(set(schema["required"])),
            "top-level strict schema properties are not all required",
        )
        operation_schema = schema["$defs"]["ConceptChangeOperationPlan"]
        _assert(
            set(operation_schema["properties"]).issubset(set(operation_schema["required"])),
            "operation strict schema properties are not all required",
        )
        system_prompt = request["messages"][0]["content"].lower()
        _assert(
            "manufactur" in system_prompt and "locked/root" in system_prompt,
            "change provider safety or lock boundary missing",
        )

        auto_service = ConceptChangeSetService(None, _FailingChangePlanner())  # type: ignore[arg-type]
        auto_request = PlanDesignChangeSetRequest(
            client_request_id="r4-change-auto-fallback",
            instruction=instruction,
            generator="auto",
            selected_node_id="node_front",
            selected_module_id="module_front_shell_02",
        )
        (
            fallback_provider,
            fallback_plan,
            fallback_used,
            warnings,
            attempted_provider,
        ) = auto_service._change_plan_with_provider(  # noqa: SLF001 - boundary smoke.
            request=auto_request,
            current_spec=spec,
            base_graph=graph,
            module_catalog=catalog,
        )
        _assert(
            fallback_provider.provider_type == "deterministic"
            and fallback_used
            and attempted_provider is not None
            and attempted_provider.provider_id == "failing_change_planner"
            and warnings[0].startswith("PLANNER_TIMEOUT")
            and len(fallback_plan.operations) == 3,
            "change auto fallback semantics mismatch",
        )
        strict_failed = False
        try:
            auto_service._change_plan_with_provider(  # noqa: SLF001 - boundary smoke.
                request=auto_request.model_copy(
                    update={"generator": "configured_provider"}
                ),
                current_spec=spec,
                base_graph=graph,
                module_catalog=catalog,
            )
        except ConceptChangeSetError as exc:
            strict_failed = exc.code == "PLANNER_TIMEOUT"
        _assert(strict_failed, "configured Change Planner failure was silently downgraded")

        unknown_module_plan = ConceptChangePlan(
            summary="引用未注册模块。",
            operations=[
                ConceptChangeOperationPlan(
                    **_operation(
                        op="replace_module",
                        node_id="node_front",
                        module_id="module_not_registered",
                        rationale="非法测试引用。",
                    )
                )
            ],
            rationale=["测试注册表拒绝。"],
        )
        _assert_bad_plan(
            unknown_module_plan,
            spec=spec,
            graph=graph,
            catalog=catalog,
            expected="unregistered",
        )
        locked_node_plan = ConceptChangePlan(
            summary="尝试修改锁定核心。",
            operations=[
                ConceptChangeOperationPlan(
                    **_operation(
                        op="set_mirror",
                        node_id="node_core",
                        mirror_axis="x",
                        rationale="非法锁定节点测试。",
                    )
                )
            ],
            rationale=["测试锁定拒绝。"],
        )
        _assert_bad_plan(
            locked_node_plan,
            spec=spec,
            graph=graph,
            catalog=catalog,
            expected="editable non-root",
        )
        print(
            json.dumps(
                {
                    "ok": True,
                    "openai_compatible_requests": len(_ChangePlannerHandler.requests),
                    "structured_output_verified": True,
                    "safety_prompt_verified": True,
                    "provider_telemetry_verified": True,
                    "provider_operation_count": len(plan.operations),
                    "auto_fallback_verified": True,
                    "configured_provider_failure_preserved": True,
                    "unregistered_module_rejected": True,
                    "locked_node_rejected": True,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    finally:
        server.shutdown()
        server.server_close()
        thread.join(timeout=5)
    return 0


def _assert_bad_plan(
    plan: ConceptChangePlan,
    *,
    spec: WeaponConceptSpec,
    graph: ModuleGraph,
    catalog: list[dict[str, str]],
    expected: str,
) -> None:
    service = ConceptChangeSetService(  # type: ignore[arg-type]
        None, _FixedChangePlanner(plan)
    )
    rejected = False
    try:
        service._change_plan_with_provider(  # noqa: SLF001 - boundary smoke.
            request=PlanDesignChangeSetRequest(
                client_request_id=f"r4-change-bad-{expected}",
                instruction="执行测试修改指令。",
                generator="configured_provider",
            ),
            current_spec=spec,
            base_graph=graph,
            module_catalog=catalog,
        )
    except ConceptChangeSetError as exc:
        rejected = exc.code == "PLANNER_BAD_OUTPUT" and expected in str(exc)
    _assert(rejected, f"bad Change Planner output was not rejected: {expected}")


def _catalog() -> list[dict[str, str]]:
    return [
        {"module_id": "module_core_shell_01", "category": "core_shell"},
        {"module_id": "module_front_shell_01", "category": "front_shell"},
        {"module_id": "module_front_shell_02", "category": "front_shell"},
    ]


def _operation(
    *,
    op: str,
    node_id: Optional[str] = None,
    module_id: Optional[str] = None,
    path: Optional[str] = None,
    value: Any = None,
    mirror_axis: Optional[str] = None,
    rationale: str,
) -> dict[str, Any]:
    return {
        "op": op,
        "node_id": node_id,
        "module_id": module_id,
        "path": path,
        "value": value,
        "mirror_axis": mirror_axis,
        "rationale": rationale,
    }


if __name__ == "__main__":
    raise SystemExit(main())
