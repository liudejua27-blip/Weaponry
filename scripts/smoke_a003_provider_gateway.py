#!/usr/bin/env python3
"""Offline A003 Gate for Provider preflight, lifecycle, errors and cancellation."""

from __future__ import annotations

import json
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.agent_models import CreateAgentThreadRequest, StartAgentTurnRequest
from forgecad_agent.application.domain_packs import domain_pack_by_id
from forgecad_agent.application.mechanical_planner import (
    MechanicalPlannerConfig,
    MechanicalPlannerError,
    OpenAICompatibleMechanicalPlanner,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner, SQLiteUnitOfWork


ROOT = Path(__file__).resolve().parents[1]
TEST_SECRET = "-".join(("a003", "secret", "must", "never", "appear"))
NETWORK_TEST_SECRET = "-".join(("network", "test", "secret"))


def _plan_payload() -> dict:
    return {
        "schema_version": "MechanicalConceptPlan@1",
        "plan_id": "plan_a003_fake",
        "domain_pack_id": "pack_vehicle_concept",
        "brief": "设计一辆完整外观的冰原探索车",
        "generation_stage": "blockout",
        "spec": {"non_functional_only": True},
        "directions": [
            {
                "direction_id": f"direction_{index}",
                "title": f"方向 {index}",
                "summary": "完整外观概念。",
                "silhouette": silhouette,
                "primary_part_roles": ["body_shell", "cabin"],
                "material_direction": "视觉涂层",
            }
            for index, silhouette in enumerate(("compact", "balanced", "industrial"), start=1)
        ],
        "provider_id": "fake",
        "model": "fake",
        "shape_program_ready": False,
    }


class ProviderHandler(BaseHTTPRequestHandler):
    mode = "success"
    request_count = 0
    request_bodies: list[dict] = []

    def do_POST(self) -> None:  # noqa: N802
        length = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(length).decode("utf-8"))
        ProviderHandler.request_count += 1
        ProviderHandler.request_bodies.append(body)
        if self.mode.startswith("http_"):
            self.send_response(int(self.mode.removeprefix("http_")))
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Connection", "close")
        self.end_headers()
        if self.mode == "empty":
            content = ""
        elif self.mode == "invalid_json":
            content = "{not-json"
        elif self.mode == "schema_mismatch":
            content = "{}"
        else:
            content = json.dumps(_plan_payload(), ensure_ascii=False)
        midpoint = max(1, len(content) // 2)
        chunks = (content[:midpoint], content[midpoint:])
        for index, value in enumerate(chunks):
            payload = {"choices": [{"delta": {"content": value}}], "usage": None}
            self.wfile.write(f"data: {json.dumps(payload, ensure_ascii=False)}\n\n".encode())
            self.wfile.flush()
            if self.mode in {"slow_cancel", "timeout"} and index == 0:
                time.sleep(0.5)
        usage = {
            "choices": [],
            "usage": {
                "prompt_tokens": 120,
                "completion_tokens": 30,
                "total_tokens": 150,
                "prompt_cache_hit_tokens": 80,
                "prompt_cache_miss_tokens": 40,
            },
        }
        try:
            self.wfile.write(f"data: {json.dumps(usage)}\n\ndata: [DONE]\n\n".encode())
            self.wfile.flush()
        except BrokenPipeError:
            pass

    def log_message(self, format: str, *args: object) -> None:
        return


def _planner(server: ThreadingHTTPServer, *, timeout_seconds: float = 3) -> OpenAICompatibleMechanicalPlanner:
    return OpenAICompatibleMechanicalPlanner(
        MechanicalPlannerConfig(
            base_url=f"http://127.0.0.1:{server.server_address[1]}/v1",
            model="deepseek-v4-pro",
            api_key=TEST_SECRET,
            response_mode="json_object",
            timeout_seconds=timeout_seconds,
        )
    )


def _call(planner: OpenAICompatibleMechanicalPlanner) -> None:
    planner.plan_complete_concept(
        brief="设计一辆完整外观的冰原探索车",
        pack=domain_pack_by_id("pack_vehicle_concept"),
        project_id=None,
    )


def _assert_error(planner: OpenAICompatibleMechanicalPlanner, expected: str) -> None:
    try:
        _call(planner)
    except MechanicalPlannerError as exc:
        assert exc.code == expected, (exc.code, expected)
        assert exc.network_call_made is True
        assert planner.last_execution_trace is not None
        assert planner.last_execution_trace.phase == "failed"
        assert planner.last_execution_trace.error_code == expected
    else:
        raise AssertionError(f"expected {expected}")


def main() -> int:
    ProviderHandler.request_count = 0
    ProviderHandler.request_bodies = []
    unconfigured = OpenAICompatibleMechanicalPlanner(
        MechanicalPlannerConfig(base_url="https://api.deepseek.com", model="deepseek-v4-pro", api_key=None)
    )
    try:
        _call(unconfigured)
    except MechanicalPlannerError as exc:
        assert exc.code == "PROVIDER_UNCONFIGURED"
        assert exc.network_call_made is False
        assert unconfigured.last_execution_trace is not None
        assert unconfigured.last_execution_trace.network_call_made is False
    else:
        raise AssertionError("missing Keychain secret must reject before network access")
    assert ProviderHandler.request_count == 0
    unconfigured_check = AgentKernelService(
        SQLiteConnectionFactory(Path(tempfile.gettempdir()) / "forgecad-a003-unconfigured-unused.db"),
        planner=unconfigured,
    ).check_provider()
    assert unconfigured_check.status == "not_configured"
    assert unconfigured_check.network_call_made is False

    server = ThreadingHTTPServer(("127.0.0.1", 0), ProviderHandler)
    server_thread = threading.Thread(target=server.serve_forever, daemon=True)
    server_thread.start()
    try:
        ProviderHandler.mode = "success"
        planner = _planner(server)
        traces = []
        result = planner.plan_complete_concept(
            brief="设计一辆完整外观的冰原探索车",
            pack=domain_pack_by_id("pack_vehicle_concept"),
            project_id=None,
            trace_observer=traces.append,
        )
        assert result.provider_id == planner.provider_id
        assert [trace.phase for trace in traces] == [
            "preflight", "request_started", "streaming", "validating", "completed"
        ]
        assert traces[-1].total_tokens == 150
        assert traces[-1].prompt_cache_hit_tokens == 80
        request = ProviderHandler.request_bodies[-1]
        assert request["stream"] is True and request["stream_options"]["include_usage"] is True
        assert "versioned_json_output_example" in request["messages"][1]["content"]

        for status, code in {
            400: "DEEPSEEK_INVALID_REQUEST",
            401: "DEEPSEEK_AUTH_FAILED",
            402: "DEEPSEEK_BALANCE_EXHAUSTED",
            422: "DEEPSEEK_INVALID_PARAMETERS",
            429: "DEEPSEEK_RATE_LIMITED",
            500: "DEEPSEEK_SERVER_ERROR",
            503: "DEEPSEEK_SERVER_BUSY",
        }.items():
            ProviderHandler.mode = f"http_{status}"
            _assert_error(_planner(server), code)
        for mode, code in {
            "empty": "PROVIDER_EMPTY_CONTENT",
            "invalid_json": "PROVIDER_INVALID_JSON",
            "schema_mismatch": "PROVIDER_SCHEMA_MISMATCH",
        }.items():
            ProviderHandler.mode = mode
            _assert_error(_planner(server), code)
        ProviderHandler.mode = "timeout"
        _assert_error(_planner(server, timeout_seconds=0.05), "PROVIDER_TIMEOUT")
        network_failure = OpenAICompatibleMechanicalPlanner(
            MechanicalPlannerConfig(
                base_url="http://127.0.0.1:1/v1",
                model="deepseek-v4-pro",
                api_key=NETWORK_TEST_SECRET,
                timeout_seconds=0.1,
            )
        )
        _assert_error(network_failure, "PROVIDER_NETWORK_ERROR")

        with tempfile.TemporaryDirectory() as temporary:
            factory = SQLiteConnectionFactory(Path(temporary) / "library.db")
            SQLiteMigrationRunner(factory, ROOT / "migrations").run()
            planner = _planner(server)
            service = AgentKernelService(factory, planner=planner)
            ProviderHandler.mode = "success"
            provider_check = service.check_provider()
            assert provider_check.status == "ready" and provider_check.network_call_made is True
            assert [trace.phase for trace in provider_check.execution_trace] == [
                "preflight", "request_started", "streaming", "validating", "completed"
            ]
            ProviderHandler.mode = "slow_cancel"
            check_results: list[object] = []
            check_worker = threading.Thread(
                target=lambda: check_results.append(service.check_provider("check_cancel"))
            )
            check_worker.start()
            cancel_requested = False
            for _ in range(100):
                cancel_requested = service.cancel_provider_check("check_cancel")
                if cancel_requested:
                    break
                time.sleep(0.01)
            assert cancel_requested
            check_worker.join(timeout=3)
            assert not check_worker.is_alive() and check_results
            assert check_results[0].status == "cancelled"
            assert check_results[0].execution_trace[-1].phase == "cancelled"
            thread = service.create_thread(
                CreateAgentThreadRequest(client_request_id="a003-thread", title="A003"),
                "a003-thread",
            )
            ProviderHandler.mode = "success"
            turn = service.start_turn(
                thread.thread_id,
                StartAgentTurnRequest(client_request_id="a003-turn", message="设计一辆完整外观的冰原探索车"),
                "a003-turn",
            )
            phases = [
                item.payload["provider_execution_trace"]["phase"]
                for item in turn.items
                if "provider_execution_trace" in item.payload
            ]
            assert phases == ["preflight", "request_started", "streaming", "validating", "completed"]
            assert turn.usage["network_call_made"] is True
            assert turn.usage["fallback_used"] is False

            ProviderHandler.mode = "slow_cancel"
            result_holder: list[object] = []
            error_holder: list[BaseException] = []

            def run_turn() -> None:
                try:
                    result_holder.append(service.start_turn(
                        thread.thread_id,
                        StartAgentTurnRequest(client_request_id="a003-cancel", message="把这辆完整外观汽车做得更紧凑"),
                        "a003-cancel",
                    ))
                except BaseException as exc:  # noqa: BLE001
                    error_holder.append(exc)

            worker = threading.Thread(target=run_turn)
            worker.start()
            cancel_turn_id = None
            for _ in range(100):
                with SQLiteUnitOfWork(factory) as unit:
                    rows = unit.agent_kernel.list_turns(thread.thread_id)
                    running = [row for row in rows if row["status"] == "running"]
                    if running:
                        cancel_turn_id = str(running[-1]["turn_id"])
                        break
                time.sleep(0.01)
            assert cancel_turn_id is not None
            cancelled = service.cancel_turn(cancel_turn_id, "cancel-idem")
            assert cancelled.status == "cancelled"
            worker.join(timeout=3)
            assert not worker.is_alive() and not error_holder
            assert result_holder and result_holder[0].status == "cancelled"
            restored = AgentKernelService(factory, planner=_planner(server)).get_thread(thread.thread_id)
            assert restored.turns[-1].status == "cancelled"

            redacted = json.dumps(restored.model_dump(mode="json"), ensure_ascii=False)
            for forbidden in (
                TEST_SECRET,
                f"127.0.0.1:{server.server_address[1]}",
                "reasoning_content",
            ):
                assert forbidden not in redacted
    finally:
        server.shutdown()
        server_thread.join(timeout=2)

    print("A003 Provider Gateway smoke passed: preflight, SSE lifecycle, cancellation, usage/cache, stable errors, redaction, restart readback, no fallback")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
