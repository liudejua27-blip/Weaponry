#!/usr/bin/env python3
from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Optional

from forgecad_agent.application.concept_briefs import ConceptBriefError, ConceptBriefService
from forgecad_agent.application.concept_models import (
    GenerateDesignVariantsRequest,
    InterpretDesignBriefRequest,
)
from forgecad_agent.application.concept_planner import (
    ConceptPlannerError,
    ConceptVariantPlan,
    DeterministicConceptPlanner,
    OpenAICompatibleConceptPlanner,
    OpenAICompatibleConceptPlannerConfig,
    planner_provenance,
)
from forgecad_agent.domain.concepts.models import ModuleGraph, WeaponConceptSpec
from smoke_r2_concept_contracts import _concept_spec
from smoke_r2_concept_projects import _assert
from smoke_r2_module_registry import _graph


class _PlannerHandler(BaseHTTPRequestHandler):
    requests: list[dict[str, Any]] = []

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API.
        length = int(self.headers.get("Content-Length", "0"))
        payload = json.loads(self.rfile.read(length).decode("utf-8"))
        self.__class__.requests.append(payload)
        response_format = payload["response_format"]
        schema_name = (
            response_format["json_schema"]["name"]
            if response_format["type"] == "json_schema"
            else "forgecad_concept_brief_patch"
        )
        if schema_name == "forgecad_concept_brief_patch":
            content = {
                "keywords": ["寒地", "工业", "精密"],
                "palette": ["graphite", "signal_blue"],
                "detail_density": 0.84,
                "overall_length_mm": 212.0,
                "body_height_mm": 52.0,
                "grip_angle_deg": 14.0,
                "symmetry": "mostly_symmetric",
            }
        else:
            content = {
                "variants": [
                    _variant(1, "A · AI 紧凑", [0.9, 0.96, 0.96]),
                    _variant(2, "B · AI 均衡", [1.0, 0.94, 1.0]),
                    _variant(3, "C · AI 延展", [1.1, 1.04, 1.04]),
                ]
            }
        body = json.dumps(
            {
                "choices": [{"message": {"content": json.dumps(content, ensure_ascii=False)}}],
                "usage": {
                    "prompt_tokens": 120,
                    "completion_tokens": 40,
                    "total_tokens": 160,
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


class _FailingPlanner:
    provider_id = "failing_concept_planner"
    provider_type = "openai_compatible"
    model_name: Optional[str] = "failing-model"

    def interpret_brief(self, **_kwargs: Any) -> WeaponConceptSpec:
        raise ConceptPlannerError("PLANNER_TIMEOUT", "synthetic timeout")

    def plan_variants(self, **_kwargs: Any) -> list[Any]:
        raise ConceptPlannerError("PLANNER_TIMEOUT", "synthetic timeout")


class _UnknownModulePlanner:
    provider_id = "unknown_module_planner"
    provider_type = "openai_compatible"
    model_name: Optional[str] = "bad-registry-model"

    def interpret_brief(self, *, current_spec: WeaponConceptSpec, **_kwargs: Any) -> WeaponConceptSpec:
        return current_spec

    def plan_variants(self, **_kwargs: Any) -> list[ConceptVariantPlan]:
        return [
            ConceptVariantPlan.model_validate(
                {
                    **_variant(rank, f"Bad {rank}", scale),
                    "recommended_module_ids": ["module_not_registered"],
                }
            )
            for rank, scale in (
                (1, [0.9, 0.96, 0.96]),
                (2, [1.0, 0.94, 1.0]),
                (3, [1.1, 1.04, 1.04]),
            )
        ]


def main() -> int:
    server = ThreadingHTTPServer(("127.0.0.1", 0), _PlannerHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        provider = OpenAICompatibleConceptPlanner(
            OpenAICompatibleConceptPlannerConfig(
                base_url=f"http://127.0.0.1:{server.server_port}/v1",
                model="fake-concept-model",
                api_key="fake-secret",
                timeout_seconds=5,
            )
        )
        spec = WeaponConceptSpec.model_validate(_concept_spec())
        graph = ModuleGraph.model_validate(_graph(spec.project_id))
        catalog = [
            {"module_id": "module_core_shell_01", "category": "core_shell"},
            {"module_id": "module_front_shell_01", "category": "front_shell"},
        ]
        interpreted = provider.interpret_brief(
            source_text="寒地、精密、蓝色点缀的紧凑非功能概念资产",
            current_spec=spec,
            module_catalog=catalog,
        )
        plans = provider.plan_variants(
            source_text="寒地、精密、蓝色点缀的紧凑非功能概念资产",
            interpreted_spec=interpreted,
            base_graph=graph,
            module_catalog=catalog,
        )
        _assert(interpreted.proportions.overall_length_mm == 212.0, "provider brief output mismatch")
        _assert([item.rank for item in plans] == [1, 2, 3], "provider variant ranks mismatch")
        _assert(
            all(item.recommended_module_ids == ["module_front_shell_01"] for item in plans),
            "provider module recommendations mismatch",
        )
        _assert(len(_PlannerHandler.requests) == 2, "provider HTTP request count mismatch")
        _assert(
            provider.last_call_metrics is not None
            and provider.last_call_metrics.latency_ms >= 0
            and provider.last_call_metrics.input_tokens == 120
            and provider.last_call_metrics.output_tokens == 40
            and provider.last_call_metrics.total_tokens == 160,
            "provider usage telemetry mismatch",
        )
        provenance = planner_provenance(
            provider,
            input_payload={"brief": "寒地、精密、蓝色点缀的紧凑非功能概念资产"},
            output_payload=[item.model_dump(mode="json") for item in plans],
            registry_module_ids=[item["module_id"] for item in catalog],
        )
        _assert(
            provenance.latency_ms is not None
            and provenance.input_tokens == 120
            and provenance.output_tokens == 40
            and provenance.total_tokens == 160,
            "planner provenance did not retain provider telemetry",
        )
        _assert(
            all(
                request["response_format"]["type"] == "json_schema"
                and request["response_format"]["json_schema"]["strict"] is True
                for request in _PlannerHandler.requests
            ),
            "provider structured output contract missing",
        )
        system_prompts = [request["messages"][0]["content"] for request in _PlannerHandler.requests]
        _assert(
            all("manufactur" in prompt.lower() for prompt in system_prompts),
            "provider safety boundary missing from prompts",
        )

        deterministic = DeterministicConceptPlanner()
        family_expectations = {
            "侦察轻型、紧凑、非功能展示道具": "A · 侦察短构",
            "堡垒重装、层级装甲、非功能展示道具": "A · 堡垒核心",
            "典藏仪式、长轴展示、非功能影视道具": "A · 典藏短轴",
            "棱镜脉冲、非对称、非功能游戏道具": "A · 棱镜短脉",
        }
        family_signatures: list[str] = []
        for family_brief, expected_first_name in family_expectations.items():
            family_plans = deterministic.plan_variants(
                source_text=family_brief,
                interpreted_spec=spec,
                base_graph=graph,
                module_catalog=catalog,
            )
            _assert(
                len(family_plans) == 3
                and family_plans[0].name == expected_first_name
                and all(plan.recommended_module_ids for plan in family_plans),
                f"concept family variants mismatch: {family_brief}",
            )
            family_signatures.append(
                json.dumps(
                    [plan.node_transforms[0].model_dump(mode="json") for plan in family_plans],
                    ensure_ascii=False,
                    sort_keys=True,
                )
            )
        _assert(
            len(set(family_signatures)) == len(family_expectations),
            "concept families must produce distinct visual transform templates",
        )

        _PlannerHandler.requests.clear()
        deepseek_compatible = OpenAICompatibleConceptPlanner(
            OpenAICompatibleConceptPlannerConfig(
                base_url=f"http://127.0.0.1:{server.server_port}/v1",
                model="deepseek-v4-pro",
                api_key="fake-secret",
                timeout_seconds=5,
                response_mode="json_object",
                max_output_tokens=2048,
            )
        )
        deepseek_compatible.interpret_brief(
            source_text="紧凑、精密的非功能未来概念资产",
            current_spec=spec,
            module_catalog=catalog,
        )
        deepseek_request = _PlannerHandler.requests[0]
        _assert(
            deepseek_request["response_format"] == {"type": "json_object"}
            and deepseek_request["max_tokens"] == 2048,
            "DeepSeek JSON output contract mismatch",
        )
        _assert(
            "JSON Schema" in deepseek_request["messages"][0]["content"],
            "DeepSeek JSON mode did not receive the schema prompt",
        )

        fallback_service = ConceptBriefService(None, _FailingPlanner())  # type: ignore[arg-type]
        fallback_request = InterpretDesignBriefRequest(
            client_request_id="r4-fallback-brief",
            source_text="紧凑、精密的非功能展示模型",
            generator="auto",
        )
        fallback_provider, fallback_spec, fallback_used, warnings, attempted_provider = (
            fallback_service._interpret_with_provider(  # noqa: SLF001 - boundary smoke.
                request=fallback_request,
                current_spec=spec,
                module_catalog=catalog,
            )
        )
        _assert(
            fallback_provider.provider_type == "deterministic"
            and fallback_used
            and attempted_provider is not None
            and attempted_provider.provider_id == "failing_concept_planner"
            and warnings[0].startswith("PLANNER_TIMEOUT")
            and fallback_spec.proportions.overall_length_mm == 207.0,
            "auto fallback semantics mismatch",
        )
        strict_error = False
        try:
            fallback_service._interpret_with_provider(  # noqa: SLF001 - boundary smoke.
                request=fallback_request.model_copy(
                    update={"generator": "configured_provider"}
                ),
                current_spec=spec,
                module_catalog=catalog,
            )
        except ConceptBriefError as exc:
            strict_error = exc.code == "PLANNER_TIMEOUT"
        _assert(strict_error, "configured provider failure was silently downgraded")

        (
            variant_provider,
            fallback_plans,
            variant_fallback,
            variant_warnings,
            attempted_variant_provider,
        ) = (
            fallback_service._variants_with_provider(  # noqa: SLF001 - boundary smoke.
                request=GenerateDesignVariantsRequest(
                    client_request_id="r4-fallback-variants",
                    brief_id="brief_r4_fallback",
                    generator="auto",
                ),
                source_text=fallback_request.source_text,
                interpreted_spec=fallback_spec,
                base_graph=graph,
                module_catalog=catalog,
            )
        )
        _assert(
            variant_provider.provider_type == "deterministic"
            and variant_fallback
            and attempted_variant_provider is not None
            and len(fallback_plans) == 3
            and variant_warnings[0].startswith("PLANNER_TIMEOUT"),
            "variant auto fallback semantics mismatch",
        )
        registry_error = False
        bad_registry_service = ConceptBriefService(  # type: ignore[arg-type]
            None, _UnknownModulePlanner()
        )
        try:
            bad_registry_service._variants_with_provider(  # noqa: SLF001 - boundary smoke.
                request=GenerateDesignVariantsRequest(
                    client_request_id="r4-unregistered-module",
                    brief_id="brief_r4_unregistered_module",
                    generator="configured_provider",
                ),
                source_text=fallback_request.source_text,
                interpreted_spec=fallback_spec,
                base_graph=graph,
                module_catalog=catalog,
            )
        except ConceptBriefError as exc:
            registry_error = exc.code == "PLANNER_BAD_OUTPUT" and "unregistered" in str(exc)
        _assert(registry_error, "unregistered planner module was not rejected")
        print(
            json.dumps(
                {
                    "ok": True,
                    "openai_compatible_requests": len(_PlannerHandler.requests),
                    "structured_output_verified": True,
                    "safety_prompt_verified": True,
                    "provider_telemetry_verified": True,
                    "provider_variant_count": len(plans),
                    "concept_family_variants_verified": len(family_expectations),
                    "auto_fallback_verified": True,
                    "configured_provider_failure_preserved": True,
                    "unregistered_module_rejected": True,
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


def _variant(rank: int, name: str, scale: list[float]) -> dict[str, Any]:
    return {
        "rank": rank,
        "name": name,
        "summary": f"结构化方案 {rank}",
        "target_node_id": "node_front",
        "scale": scale,
        "recommended_module_ids": ["module_front_shell_01"],
        "rationale": ["注册模块", "保持根节点锁定", "非功能概念比例"],
    }


if __name__ == "__main__":
    raise SystemExit(main())
