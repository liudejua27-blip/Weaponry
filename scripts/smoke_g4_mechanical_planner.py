#!/usr/bin/env python3
"""Offline contract smoke for the general mechanical planner Provider port."""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

from forgecad_agent.application.domain_packs import domain_pack_for_message
from forgecad_agent.application.mechanical_planner import (
    DeterministicMechanicalPlanner,
    MechanicalConceptPlan,
    MechanicalPlannerConfig,
    MechanicalPlannerError,
    OpenAICompatibleMechanicalPlanner,
)


class FakeProviderHandler(BaseHTTPRequestHandler):
    request_body = None

    def do_POST(self) -> None:  # noqa: N802 - stdlib handler API
        length = int(self.headers.get("Content-Length", "0"))
        FakeProviderHandler.request_body = json.loads(self.rfile.read(length).decode("utf-8"))
        response = {
            "id": "fake-completion",
            "choices": [
                {
                    "message": {
                        "content": json.dumps(
                            {
                                "schema_version": "MechanicalConceptPlan@1",
                                "plan_id": "plan_fake_provider",
                                "domain_pack_id": "pack_robotic_arm_concept",
                                "brief": "设计一台三关节机械臂",
                                "generation_stage": "blockout",
                                "spec": {"non_functional_only": True},
                                "directions": [
                                    {
                                        "direction_id": "direction_1",
                                        "title": "精密桌面",
                                        "summary": "紧凑的桌面机械臂完整外观。",
                                        "silhouette": "compact",
                                        "primary_part_roles": ["base", "upper_link", "end_effector"],
                                        "material_direction": "深色金属与橡胶",
                                    },
                                ],
                                "provider_id": "fake",
                                "model": "fake-model",
                                "shape_program_ready": False,
                            },
                            ensure_ascii=False,
                        )
                    }
                }
            ],
            "usage": {"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30},
        }
        encoded = json.dumps(response, ensure_ascii=False).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def log_message(self, format: str, *args: object) -> None:
        return


def main() -> int:
    deterministic = DeterministicMechanicalPlanner().plan_complete_concept(
        brief="设计一台三关节机械臂",
        pack=domain_pack_for_message("设计一台三关节机械臂"),
        project_id=None,
    )
    assert isinstance(deterministic, MechanicalConceptPlan)
    assert deterministic.domain_pack_id == "pack_robotic_arm_concept"
    assert len(deterministic.directions) == 1
    assert deterministic.spec["visual_intent_mapping"]["schema_version"] == "VisualIntentMapping@2"

    server = ThreadingHTTPServer(("127.0.0.1", 0), FakeProviderHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        planner = OpenAICompatibleMechanicalPlanner(
            MechanicalPlannerConfig(
                base_url=f"http://127.0.0.1:{server.server_address[1]}/v1",
                model="fake-model",
                api_key="unit" + "-secret",
                response_mode="json_schema",
            )
        )
        plan = planner.plan_complete_concept(
            brief="设计一台三关节机械臂",
            pack=domain_pack_for_message("设计一台三关节机械臂"),
            project_id="prj_fake",
        )
        assert plan.provider_id == "openai_compatible_mechanical_planner"
        assert plan.model == "fake-model"
        assert len(plan.directions) == 1
        mapping = plan.spec["visual_intent_mapping"]
        assert mapping["schema_version"] == "VisualIntentMapping@2"
        assert [item["direction_id"] for item in mapping["directions"]] == [item.direction_id for item in plan.directions]
        assert all(direction.material_direction for direction in plan.directions)
        request_body = FakeProviderHandler.request_body or {}
        assert request_body["model"] == "fake-model"
        assert request_body["stream"] is True
        assert request_body["stream_options"] == {"include_usage": True}
        assert request_body["response_format"]["type"] == "json_schema"
        assert "non-functional" in request_body["messages"][0]["content"]
        assert "versioned_json_output_example" in request_body["messages"][1]["content"]
        assert planner.last_execution_trace is not None
        assert planner.last_execution_trace.phase == "completed"
        assert planner.last_execution_trace.network_call_made is True
    finally:
        server.shutdown()
        thread.join(timeout=2)

    try:
        OpenAICompatibleMechanicalPlanner(
            MechanicalPlannerConfig(base_url="http://127.0.0.1:1/v1", model="fake-model", api_key=None)
        ).plan_complete_concept(
            brief="测试未配置",
            pack=domain_pack_for_message("测试未来概念道具"),
            project_id=None,
        )
    except MechanicalPlannerError as exc:
        assert exc.code == "PROVIDER_UNCONFIGURED"
        assert exc.network_call_made is False
    else:
        raise AssertionError("unconfigured provider must fail before network access")

    print("G4 mechanical planner smoke passed: deterministic fallback, streamed OpenAI-compatible JSON contract, safe unconfigured failure")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
