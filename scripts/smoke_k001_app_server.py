#!/usr/bin/env python3
"""FGC-K001 smoke for the Python app-server compatibility oracle."""

from __future__ import annotations

import asyncio
import base64
import json
import tempfile
from pathlib import Path
from typing import Any, Optional

from fastapi import FastAPI
from fastapi.responses import Response

from forgecad_agent.api.agent_routes import build_agent_router
from forgecad_agent.api.app_server_compat_routes import build_app_server_compat_router
from forgecad_agent.api.factory import LocalApiSettings, create_local_api
from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.app_server_compat import (
    HTTP_REQUEST_SCHEMA,
    INPUT_TOO_LARGE,
    MAX_FRAME_BYTES,
    MAX_RAW_BODY_BYTES,
    PROTOCOL_VERSION,
    AppServerCompatibilityAdapter,
    InProcessAsgiClient,
    ProtocolFailure,
    canonical_json_sha256,
    canonical_turn_item_hash,
)
from forgecad_agent.application.agent_models import AgentItem, CreateAgentThreadRequest
from forgecad_agent.application.mechanical_planner import (
    DeterministicMechanicalPlanner,
    MechanicalPlannerError,
)
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner


ROOT = Path(__file__).resolve().parents[1]
FIXTURE_PATH = ROOT / "packages/concept-spec/fixtures/k001-a004-turn-compatibility.json"
MANIFEST_PATH = ROOT / "packages/concept-spec/fixtures/forgecad-app-server-protocol-manifest.json"


class _BlockingPlanner(DeterministicMechanicalPlanner):
    def plan_complete_concept(self, *, cancel_event=None, **kwargs: Any):  # type: ignore[no-untyped-def]
        del kwargs
        if cancel_event is None:
            raise AssertionError("cancel_event must reach the Provider boundary")
        while not cancel_event.wait(0.01):
            pass
        raise MechanicalPlannerError(
            "PROVIDER_CANCELLED",
            "Provider request was cancelled.",
            recoverable=False,
        )


def _make_app(
    root: Path,
    *,
    planner: Optional[DeterministicMechanicalPlanner] = None,
    max_frame_bytes: int = MAX_FRAME_BYTES,
    max_raw_body_bytes: int = MAX_RAW_BODY_BYTES,
) -> tuple[FastAPI, AgentKernelService, AppServerCompatibilityAdapter, dict[str, int]]:
    root.mkdir(parents=True, exist_ok=True)
    factory = SQLiteConnectionFactory(root / "library.db")
    applied = SQLiteMigrationRunner(factory, ROOT / "migrations").run()
    assert "0019" in applied, applied
    service = AgentKernelService(factory, planner=planner)
    app = create_local_api(LocalApiSettings(title="K001 smoke", version="1"))
    counters = {"slow": 0}

    @app.get("/api/health")
    async def health() -> dict[str, str]:
        return {"status": "ok", "service": "k001-smoke"}

    @app.get("/api/v1/k001/binary")
    async def binary() -> Response:
        return Response(content=b"\x00\xffglTF", media_type="application/octet-stream")

    @app.get("/api/v1/k001/oversize")
    async def oversize() -> Response:
        return Response(content=b"x" * 9, media_type="application/octet-stream")

    @app.get("/api/v1/k001/slow")
    async def slow() -> dict[str, bool]:
        counters["slow"] += 1
        await asyncio.sleep(30)
        return {"late": True}

    app.include_router(
        build_agent_router(service, allow_test_only_legacy_lifecycle=True)
    )
    adapter = AppServerCompatibilityAdapter(
        app,
        max_frame_bytes=max_frame_bytes,
        max_raw_body_bytes=max_raw_body_bytes,
    )
    app.include_router(build_app_server_compat_router(adapter))
    return app, service, adapter, counters


def _initialize_frame(request_id: str = "req_initialize") -> dict[str, Any]:
    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "initialize",
        "params": {
            "schema_version": "ForgeCADInitializeParams@1",
            "supported_protocol_versions": [PROTOCOL_VERSION],
            "client_info": {
                "name": "forgecad-desktop",
                "version": "0.1.0",
                "transport": "browser-loopback-compatibility",
            },
            "capabilities": {
                "notifications": True,
                "cursor_replay": True,
                "cancellation": True,
                "notification_ack": True,
                "binary_body_base64": True,
            },
        },
    }


async def _ready(adapter: AppServerCompatibilityAdapter, *, capacity: Optional[int] = None) -> str:
    connection_id = await adapter.open_connection(capacity)
    initialized = await adapter.send_frame(connection_id, _initialize_frame(f"init_{connection_id}"))
    result = initialized["frame"]["result"]
    assert result["protocol_version"] == PROTOCOL_VERSION
    assert result["migration_state"] == {"state_owner": "python_compatibility_adapter"}
    assert result["limits"]["max_frame_bytes"] == adapter._max_frame_bytes  # noqa: SLF001
    notification = await adapter.send_frame(
        connection_id,
        {
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {"protocol_version": PROTOCOL_VERSION},
        },
    )
    assert notification == {"frames": []}
    return connection_id


def _http_frame(
    request_id: str,
    method: str,
    path: str,
    *,
    json_body: Optional[dict[str, Any]] = None,
    headers: Optional[list[list[str]]] = None,
    body: Optional[dict[str, Any]] = None,
) -> dict[str, Any]:
    actual_headers = list(headers or [])
    actual_body = body or {"encoding": "empty"}
    if json_body is not None:
        actual_headers = [["content-type", "application/json"], *actual_headers]
        actual_body = {
            "encoding": "utf8",
            "data": json.dumps(json_body, ensure_ascii=False, separators=(",", ":")),
        }
    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "compat/http",
        "params": {
            "schema_version": HTTP_REQUEST_SCHEMA,
            "path": path,
            "method": method,
            "headers": actual_headers,
            "body": actual_body,
        },
    }


def _http_json(result: dict[str, Any]) -> Any:
    response = result["frame"]["result"]
    assert response["schema_version"] == "ForgeCADHttpCompatibilityResponse@1"
    assert response["body"]["encoding"] in {"empty", "base64"}
    if response["body"]["encoding"] == "empty":
        return None
    return json.loads(base64.b64decode(response["body"]["data"], validate=True))


def _marker(item: dict[str, Any]) -> str:
    item_type = item["item_type"]
    payload = item["payload"]
    if item_type in {"tool_call", "tool_result"}:
        name = payload.get("tool_name") or payload.get("tool")
        if isinstance(name, str):
            return f"{item_type}:{name}"
    return str(item_type)


def _sse_items(frames: list[dict[str, Any]]) -> list[dict[str, Any]]:
    items: list[dict[str, Any]] = []
    for frame in frames:
        if frame.get("method") != "compat/sse":
            continue
        params = frame["params"]
        if params["event"] != "agent.item":
            continue
        event = json.loads(params["data"])
        assert str(event["sequence"]) == params["id"]
        assert event["sequence"] == event["item"]["sequence"]
        items.append(event["item"])
    return items


async def _protocol_and_a004_golden(root: Path) -> None:
    fixture = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))
    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    assert manifest["limits"] == {
        "max_frame_bytes": MAX_FRAME_BYTES,
        "max_in_flight_requests": 32,
        "default_event_queue": 128,
        "max_event_queue": 4096,
    }
    assert manifest["http_compatibility"]["max_raw_body_bytes"] == MAX_RAW_BODY_BYTES
    golden = fixture["canonical_golden"]
    golden_items = [AgentItem.model_validate(item).model_dump(mode="json") for item in golden["items"]]
    assert [canonical_json_sha256(item) for item in golden_items] == golden["item_sha256"]
    assert canonical_turn_item_hash(golden_items) == golden["turn_items_sha256"]
    assert [_marker(item) for item in golden_items] == fixture["expected_ordered_markers"]

    app, service, adapter, _ = _make_app(root)

    # The loopback routes themselves are covered, not only the in-memory API.
    outer = InProcessAsgiClient(app)
    opened = await outer.request(
        "POST",
        "/api/v1/app-server/connections",
        headers=[("content-type", "application/json")],
        body=b"{}",
    )
    assert opened.status == 200, opened.body
    route_connection = json.loads(opened.body)["connection_id"]
    sent = await outer.request(
        "POST",
        f"/api/v1/app-server/connections/{route_connection}/frames",
        headers=[("content-type", "application/json")],
        body=json.dumps({"frame": _initialize_frame("route_initialize")}).encode("utf-8"),
    )
    assert json.loads(sent.body)["frame"]["result"]["protocol_version"] == PROTOCOL_VERSION
    closed = await outer.request("POST", f"/api/v1/app-server/connections/{route_connection}:close")
    assert json.loads(closed.body) == {"closed": True}
    remote = InProcessAsgiClient(app, client_host="192.0.2.1")
    denied = await remote.request(
        "POST",
        "/api/v1/app-server/connections",
        headers=[("content-type", "application/json")],
        body=b"{}",
    )
    assert denied.status == 403
    assert json.loads(denied.body)["error"]["code"] == "LOOPBACK_REQUIRED"

    # initialize is mandatory and malformed/unsupported frames fail stably.
    cold = await adapter.open_connection()
    preinit = await adapter.send_frame(cold, _http_frame("preinit", "GET", "/api/health"))
    assert preinit["frame"]["error"]["data"]["application_code"] == "NOT_INITIALIZED"
    malformed = await adapter.send_frame(cold, {"jsonrpc": "1.0", "id": "bad", "method": "initialize", "params": {}})
    assert malformed["frame"]["error"]["data"]["application_code"] == "JSONRPC_VERSION_INVALID"
    non_object = await adapter.send_frame(cold, [])
    assert non_object["frame"]["error"]["data"]["application_code"] == "INVALID_REQUEST"
    duplicate_initialize_connection = await adapter.open_connection()
    initialize = _initialize_frame("stable_initialize")
    initialized_once = await adapter.send_frame(duplicate_initialize_connection, initialize)
    initialized_replay = await adapter.send_frame(duplicate_initialize_connection, initialize)
    assert initialized_replay["frame"] == initialized_once["frame"]
    changed_initialize = json.loads(json.dumps(initialize))
    changed_initialize["params"]["client_info"]["version"] = "different"
    initialize_conflict = await adapter.send_frame(duplicate_initialize_connection, changed_initialize)
    assert initialize_conflict["frame"]["error"]["data"]["application_code"] == "DUPLICATE_REQUEST_ID"
    assert await adapter.close_connection(duplicate_initialize_connection) is True
    connection_id = await _ready(adapter)
    unknown = await adapter.send_frame(
        connection_id,
        {"jsonrpc": "2.0", "id": "unknown", "method": "shell/run", "params": {}},
    )
    assert unknown["frame"]["error"]["data"]["application_code"] == "METHOD_NOT_FOUND"

    for request_id, path, headers in [
        ("url", "https://example.com/api/v1/agent/threads", []),
        ("recursive", "/api/v1/app-server/connections", []),
        ("secret", "/api/health", [["authorization", "redacted-test-value"]]),
        ("prefixed_secret", "/api/health", [["x-forgecad-provider-key", "redacted-test-value"]]),
        ("traversal", "/api/v1/agent/../secrets", []),
    ]:
        rejected = await adapter.send_frame(connection_id, _http_frame(request_id, "GET", path, headers=headers))
        assert rejected["frame"]["error"]["data"]["application_code"] == "INVALID_PARAMS"

    binary = await adapter.send_frame(connection_id, _http_frame("binary", "GET", "/api/v1/k001/binary"))
    binary_response = binary["frame"]["result"]
    assert binary_response["body"]["encoding"] == "base64"
    assert base64.b64decode(binary_response["body"]["data"], validate=True) == b"\x00\xffglTF"
    legacy_allowed = await adapter.send_frame(
        connection_id,
        _http_frame(
            "legacy_allowlist",
            "GET",
            "/api/provider-settings",
            headers=[["x-provider-check-id", "check_1"]],
        ),
    )
    assert legacy_allowed["frame"]["result"]["status"] == 404
    unknown_cancel = await adapter.send_frame(
        connection_id,
        {
            "jsonrpc": "2.0",
            "id": "cancel_unknown",
            "method": "request/cancel",
            "params": {"request_id": "not_pending", "cancel_token": "not_pending"},
        },
    )
    assert unknown_cancel["frame"]["error"]["data"]["application_code"] == "UNKNOWN_REQUEST_ID"

    create_input = {
        "client_request_id": "k001-thread-1",
        "title": "冰原探索车设计",
        "provider_id": fixture["provider_id"],
    }
    create_frame = _http_frame(
        "create_thread",
        "POST",
        "/api/v1/agent/threads",
        json_body=create_input,
        headers=[["idempotency-key", create_input["client_request_id"]]],
    )
    created_result = await adapter.send_frame(connection_id, create_frame)
    created = _http_json(created_result)
    thread_id = created["thread_id"]

    # Same ID + same canonical request replays the completed response without
    # executing the Python write again.  A different payload conflicts.
    completed_replay = await adapter.send_frame(connection_id, create_frame)
    assert completed_replay["frame"] == created_result["frame"]
    assert len(service.list_threads().items) == 1
    changed = json.loads(json.dumps(create_frame))
    changed["params"]["body"]["data"] = changed["params"]["body"]["data"].replace("冰原探索车设计", "不同标题")
    duplicate_conflict = await adapter.send_frame(connection_id, changed)
    assert duplicate_conflict["frame"]["error"]["data"]["application_code"] == "DUPLICATE_REQUEST_ID"

    turn_input = {"client_request_id": "k001-turn-1", "message": fixture["brief"]}
    turn_result = await adapter.send_frame(
        connection_id,
        _http_frame(
            "start_turn",
            "POST",
            f"/api/v1/agent/threads/{thread_id}/turns",
            json_body=turn_input,
            headers=[["idempotency-key", turn_input["client_request_id"]]],
        ),
    )
    adapter_turn = _http_json(turn_result)
    assert adapter_turn["status"] == fixture["expected_turn_status"]
    assert len(adapter_turn["items"]) == fixture["expected_item_count"]

    subscription = await adapter.send_frame(
        connection_id,
        {
            "jsonrpc": "2.0",
            "id": "subscribe_turn",
            "method": "compat/subscribe",
            "params": {
                "schema_version": "ForgeCADSseSubscription@1",
                "stream_id": "stream_a004",
                "path": f"/api/v1/agent/threads/{thread_id}/events?after=0",
            },
        },
    )
    sse_items = _sse_items(subscription["frames"])
    direct_turn = service.get_thread(thread_id).turns[-1].model_dump(mode="json")
    direct_items = [event.item.model_dump(mode="json") for event in service.events(thread_id)]

    assert adapter_turn == direct_turn
    assert sse_items == direct_items == adapter_turn["items"]
    assert [item["sequence"] for item in direct_items] == list(range(1, 17))
    assert [_marker(item) for item in direct_items] == fixture["expected_ordered_markers"]
    assert all("reasoning_content" not in json.dumps(item, ensure_ascii=False) for item in direct_items)
    hashes = [canonical_json_sha256(item) for item in direct_items]
    assert hashes == [canonical_json_sha256(item) for item in sse_items]
    assert canonical_turn_item_hash(direct_items) == canonical_turn_item_hash(adapter_turn["items"])
    preview = next(
        item
        for item in direct_items
        if item["item_type"] == "tool_result" and item["payload"].get("tool_name") == "prepare_candidate_preview"
    )
    assert preview["payload"]["result"]["permanent_side_effects"] == 0
    assert preview["payload"]["result"]["requires_user_confirmation"] is True
    unsubscribed = await adapter.send_frame(
        connection_id,
        {
            "jsonrpc": "2.0",
            "id": "unsubscribe_turn",
            "method": "compat/unsubscribe",
            "params": {
                "schema_version": "ForgeCADSseUnsubscribe@1",
                "stream_id": "stream_a004",
            },
        },
    )
    assert unsubscribed["frame"]["result"]["unsubscribed"] is True

    # Ack advances the bounded transport queue without altering persisted Item
    # sequence.  Then an opaque Item cursor survives adapter process restart.
    eighth = next(frame for frame in subscription["frames"] if frame.get("cursor") and frame["params"].get("id") == "8")
    ack = await adapter.send_frame(
        connection_id,
        {
            "jsonrpc": "2.0",
            "method": "notification/ack",
            "params": {"notification_id": eighth["notification_id"], "cursor": eighth["cursor"]},
        },
    )
    assert ack == {"frames": []}
    transport_replay = await adapter.replay_transport(connection_id, after=0)
    assert all(record["frame"].get("notification_id") != eighth["notification_id"] for record in transport_replay["frames"])

    restarted_adapter = AppServerCompatibilityAdapter(app)
    restarted_connection = await _ready(restarted_adapter)
    replay = await restarted_adapter.send_frame(
        restarted_connection,
        {
            "jsonrpc": "2.0",
            "id": "replay_after_8",
            "method": "thread/events/replay",
            "params": {"cursor": eighth["cursor"]},
        },
    )
    replay_items = _sse_items(replay["frames"])
    assert [item["sequence"] for item in replay_items] == list(range(9, 17))
    assert replay_items == direct_items[8:]
    assert len(service.list_threads().items) == 1

    # A too-small event queue never exposes a partial Item prefix.
    constrained = await _ready(adapter, capacity=4)
    overflow = await adapter.send_frame(
        constrained,
        {
            "jsonrpc": "2.0",
            "id": "overflow",
            "method": "compat/subscribe",
            "params": {
                "schema_version": "ForgeCADSseSubscription@1",
                "stream_id": "stream_overflow",
                "path": f"/api/v1/agent/threads/{thread_id}/events?after=0",
            },
        },
    )
    assert overflow["frame"]["error"]["data"]["application_code"] == "SLOW_CONSUMER"
    assert [frame["method"] for frame in overflow["frames"]] == ["stream/resyncRequired"]
    queued = await adapter.replay_transport(constrained)
    assert [record["frame"]["method"] for record in queued["frames"]] == ["stream/resyncRequired"]


async def _duplicate_inflight_and_generic_cancel(root: Path) -> None:
    _, _, adapter, counters = _make_app(root)
    connection_id = await _ready(adapter)
    slow_frame = _http_frame("slow_request", "GET", "/api/v1/k001/slow")
    running = asyncio.create_task(adapter.send_frame(connection_id, slow_frame))
    for _ in range(100):
        if counters["slow"]:
            break
        await asyncio.sleep(0.01)
    assert counters["slow"] == 1
    duplicate = await adapter.send_frame(connection_id, slow_frame)
    assert duplicate["frame"]["error"]["data"]["application_code"] == "REQUEST_IN_FLIGHT"
    cancelled = await adapter.send_frame(
        connection_id,
        {
            "jsonrpc": "2.0",
            "method": "request/cancel",
            "params": {"request_id": "slow_request", "cancel_token": "slow_request"},
        },
    )
    assert cancelled == {"frames": []}
    outcome = await asyncio.wait_for(running, timeout=2)
    assert outcome["frame"]["error"]["data"]["application_code"] == "REQUEST_CANCELLED"
    cached = await adapter.send_frame(connection_id, slow_frame)
    assert cached["frame"] == outcome["frame"]
    assert counters["slow"] == 1


async def _disconnect_cancels_inflight(root: Path) -> None:
    _, _, adapter, counters = _make_app(root)
    connection_id = await _ready(adapter)
    slow_frame = _http_frame("disconnect_target", "GET", "/api/v1/k001/slow")
    running = asyncio.create_task(adapter.send_frame(connection_id, slow_frame))
    for _ in range(100):
        if counters["slow"]:
            break
        await asyncio.sleep(0.01)
    assert counters["slow"] == 1
    assert await adapter.close_connection(connection_id) is True
    outcome = await asyncio.wait_for(running, timeout=2)
    assert outcome["frame"]["error"]["data"]["application_code"] == "REQUEST_CANCELLED"
    try:
        await adapter.replay_transport(connection_id)
    except ProtocolFailure as exc:
        assert exc.application_code == "CONNECTION_NOT_FOUND"
    else:
        raise AssertionError("closed compatibility connection remained replayable")


async def _turn_cancel_race(root: Path) -> None:
    _, service, adapter, _ = _make_app(root, planner=_BlockingPlanner())
    thread = service.create_thread(
        CreateAgentThreadRequest(
            client_request_id="cancel-thread",
            title="取消竞态",
            provider_id="blocking-planner",
        ),
        "cancel-thread",
    )
    connection_id = await _ready(adapter)
    start = asyncio.create_task(
        adapter.send_frame(
            connection_id,
            _http_frame(
                "cancel_target",
                "POST",
                f"/api/v1/agent/threads/{thread.thread_id}/turns",
                json_body={"client_request_id": "cancel-turn", "message": "设计一辆双座冰原探索车。"},
                headers=[["idempotency-key", "cancel-turn"]],
            ),
        )
    )
    for _ in range(200):
        detail = service.get_thread(thread.thread_id)
        if detail.turns and detail.turns[-1].status == "running":
            break
        await asyncio.sleep(0.01)
    else:
        raise AssertionError("blocking Turn never entered running state")
    sent = await adapter.send_frame(
        connection_id,
        {
            "jsonrpc": "2.0",
            "method": "request/cancel",
            "params": {"request_id": "cancel_target", "cancel_token": "cancel_target"},
        },
    )
    assert sent == {"frames": []}
    response = await asyncio.wait_for(start, timeout=5)
    cancelled_turn = _http_json(response)
    assert cancelled_turn.get("status") == "cancelled", (response, cancelled_turn)
    assert cancelled_turn["error_code"] == "PROVIDER_CANCELLED"
    detail = service.get_thread(thread.thread_id)
    restarted = detail.turns[-1]
    assert restarted.status == "cancelled"
    assert restarted.error_code == "PROVIDER_CANCELLED"
    assert sum(1 for turn in detail.turns if turn.turn_id == restarted.turn_id) == 1
    assert all(turn.status != "failed" for turn in detail.turns)
    assert all(item.status != "failed" for item in restarted.items)
    assert sum(1 for item in restarted.items if item.item_type == "assistant_message" and item.status == "cancelled") == 1


async def _bounded_body_and_frame(root: Path) -> None:
    _, _, adapter, _ = _make_app(root, max_frame_bytes=1024, max_raw_body_bytes=8)
    connection_id = await _ready(adapter)
    request_too_large = await adapter.send_frame(
        connection_id,
        _http_frame(
            "raw_too_large",
            "POST",
            "/api/v1/k001/binary",
            body={"encoding": "base64", "data": base64.b64encode(b"123456789").decode("ascii")},
        ),
    )
    assert request_too_large["frame"]["error"]["data"]["application_code"] == "COMPAT_BODY_TOO_LARGE"
    response_too_large = await adapter.send_frame(connection_id, _http_frame("response_too_large", "GET", "/api/v1/k001/oversize"))
    assert response_too_large["frame"]["error"]["data"]["application_code"] == "COMPAT_BODY_TOO_LARGE"
    huge = await adapter.send_frame(
        connection_id,
        {
            "jsonrpc": "2.0",
            "id": "frame_too_large",
            "method": "compat/http",
            "params": {"padding": "x" * 2048},
        },
    )
    assert huge["frame"]["error"]["code"] == INPUT_TOO_LARGE


def _production_size_limit_contract() -> None:
    adapter = AppServerCompatibilityAdapter(FastAPI())
    exact_payload = b"x" * MAX_RAW_BODY_BYTES
    encoded = base64.b64encode(exact_payload).decode("ascii")
    del exact_payload
    exact_frame = _http_frame(
        "production_body_boundary",
        "POST",
        "/api/v1/k001/binary",
        body={"encoding": "base64", "data": encoded},
    )
    adapter._parse_client_frame(exact_frame)  # noqa: SLF001
    decoded = adapter._decode_http_body(exact_frame["params"]["body"])  # noqa: SLF001
    assert len(decoded) == MAX_RAW_BODY_BYTES
    del decoded

    try:
        adapter._decode_http_body({"encoding": "base64", "data": encoded + "eA=="})  # noqa: SLF001
    except ProtocolFailure as exc:
        assert exc.application_code == "COMPAT_BODY_TOO_LARGE"
    else:
        raise AssertionError("47 MiB raw body ceiling accepted an extra decoded byte")

    current_size = len(json.dumps(exact_frame, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8"))
    assert current_size < MAX_FRAME_BYTES
    exact_frame["params"]["body"]["data"] += "A" * (MAX_FRAME_BYTES - current_size + 1)
    try:
        adapter._parse_client_frame(exact_frame)  # noqa: SLF001
    except ProtocolFailure as exc:
        assert exc.code == INPUT_TOO_LARGE
    else:
        raise AssertionError("64 MiB encoded frame ceiling accepted an oversized frame")


async def main_async() -> None:
    with tempfile.TemporaryDirectory(prefix="forgecad-k001-") as raw:
        root = Path(raw)
        await _protocol_and_a004_golden(root / "golden")
        await _duplicate_inflight_and_generic_cancel(root / "generic-cancel")
        await _disconnect_cancels_inflight(root / "disconnect-cancel")
        await _turn_cancel_race(root / "turn-cancel")
        await _bounded_body_and_frame(root / "limits")
        _production_size_limit_contract()


def main() -> int:
    asyncio.run(main_async())
    print(
        "K001 app-server smoke passed: initialize, exact compat/http+SSE DTO, "
        "A004 fixed+live hash/order parity, replay/restart, duplicate IDs, cancellation/disconnect, "
        "bounded queue, 64/47 MiB limits, loopback and security rejection"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
