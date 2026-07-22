from __future__ import annotations

import asyncio
import json as json_module
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from fastapi import FastAPI

from forgecad_agent.api.agent_routes import (
    K001_PACKAGED_PROBE_ENV,
    RUST_OWNED_AGENT_LIFECYCLE_CODE,
    RUST_OWNED_PRODUCT_STATE_CODE,
    TEST_ONLY_LEGACY_AGENT_LIFECYCLE_ENV,
    build_agent_router,
    legacy_lifecycle_compat_enabled,
)
from forgecad_agent.api.k002_internal_routes import (
    K002_INTERNAL_CAPABILITY_HEADER,
    K002_INTERNAL_CAPABILITY_TOKEN_ENV,
    K002_INTERNAL_PREFIX,
    build_k002_internal_router,
)
from forgecad_agent.application.agent_kernel import AgentKernelService
from forgecad_agent.application.k002_port_security import (
    MAX_PORT_JSON_BYTES,
    canonical_json_sha256,
)
from forgecad_agent.application.mechanical_planner import DeterministicMechanicalPlanner
from forgecad_agent.application.k002_python_ports import (
    LifecyclePersistencePort,
    ProductToolExecutorPort,
)
from forgecad_agent.application.k002_sqlite_lifecycle import K002SQLiteLifecycleBackend
from forgecad_agent.application.product_tool_registry import forgecad_product_tool_registry
from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner


MIGRATIONS_DIR = Path(__file__).resolve().parents[3] / "migrations"
CAPABILITY = "k002-test-capability-" + "a" * 48


@dataclass(frozen=True)
class _Response:
    status_code: int
    body: bytes

    @property
    def text(self) -> str:
        return self.body.decode("utf-8")

    def json(self) -> Any:
        return json_module.loads(self.text)


class _AsgiClient:
    """Tiny dependency-free ASGI client for the focused internal-port tests."""

    def __init__(self, app: FastAPI) -> None:
        self.app = app

    def post(
        self,
        path: str,
        *,
        headers: dict[str, str] | None = None,
        json: Any | None = None,
        content: bytes | None = None,
    ) -> _Response:
        if json is not None and content is not None:
            raise ValueError("json and content are mutually exclusive")
        body = (
            json_module.dumps(json, ensure_ascii=False).encode("utf-8")
            if json is not None
            else (content or b"")
        )
        request_headers = dict(headers or {})
        if json is not None:
            request_headers["Content-Type"] = "application/json"
        return asyncio.run(self._request("POST", path, request_headers, body))

    def get(
        self,
        path: str,
        *,
        headers: dict[str, str] | None = None,
    ) -> _Response:
        return asyncio.run(self._request("GET", path, dict(headers or {}), b""))

    async def _request(
        self,
        method: str,
        path: str,
        headers: dict[str, str],
        body: bytes,
    ) -> _Response:
        incoming = True
        outgoing: list[dict[str, Any]] = []

        async def receive() -> dict[str, Any]:
            nonlocal incoming
            if incoming:
                incoming = False
                return {"type": "http.request", "body": body, "more_body": False}
            return {"type": "http.disconnect"}

        async def send(message: dict[str, Any]) -> None:
            outgoing.append(message)

        scope = {
            "type": "http",
            "asgi": {"version": "3.0", "spec_version": "2.3"},
            "http_version": "1.1",
            "method": method,
            "scheme": "http",
            "path": path,
            "raw_path": path.encode("ascii"),
            "query_string": b"",
            "root_path": "",
            "headers": [
                (name.lower().encode("ascii"), value.encode("ascii"))
                for name, value in headers.items()
            ],
            "client": ("testclient", 50000),
            "server": ("testserver", 80),
            "state": {},
        }
        await self.app(scope, receive, send)
        start = next(item for item in outgoing if item["type"] == "http.response.start")
        response_body = b"".join(
            item.get("body", b"")
            for item in outgoing
            if item["type"] == "http.response.body"
        )
        return _Response(status_code=start["status"], body=response_body)


def _client(
    tmp_path: Path,
    *,
    environment: dict[str, str] | None = None,
) -> _AsgiClient:
    values = (
        {K002_INTERNAL_CAPABILITY_TOKEN_ENV: CAPABILITY}
        if environment is None
        else environment
    )
    tmp_path.mkdir(parents=True, exist_ok=True)
    factory = SQLiteConnectionFactory(tmp_path / "forgecad.db")
    SQLiteMigrationRunner(factory, MIGRATIONS_DIR).run()
    lifecycle = LifecyclePersistencePort(
        K002SQLiteLifecycleBackend(factory),
        environment=values,
    )
    product_tools = ProductToolExecutorPort(environment=values)
    app = FastAPI()
    app.include_router(
        build_k002_internal_router(
            lifecycle,
            product_tools,
            environment=values,
        )
    )
    return _AsgiClient(app)


def _public_and_internal_client(
    tmp_path: Path,
    *,
    allow_test_only_legacy_lifecycle: bool = False,
) -> _AsgiClient:
    values = {K002_INTERNAL_CAPABILITY_TOKEN_ENV: CAPABILITY}
    tmp_path.mkdir(parents=True, exist_ok=True)
    factory = SQLiteConnectionFactory(tmp_path / "forgecad.db")
    SQLiteMigrationRunner(factory, MIGRATIONS_DIR).run()
    lifecycle = LifecyclePersistencePort(
        K002SQLiteLifecycleBackend(factory),
        environment=values,
    )
    product_tools = ProductToolExecutorPort(environment=values)
    app = FastAPI()
    app.include_router(
        build_agent_router(
            AgentKernelService(factory, planner=DeterministicMechanicalPlanner()),
            allow_test_only_legacy_lifecycle=allow_test_only_legacy_lifecycle,
        )
    )
    app.include_router(
        build_k002_internal_router(
            lifecycle,
            product_tools,
            environment=values,
        )
    )
    return _AsgiClient(app)


def _headers(token: str = CAPABILITY) -> dict[str, str]:
    return {K002_INTERNAL_CAPABILITY_HEADER: token}


class _NeverAccessedPort:
    def execute(self, *_args: Any, **_kwargs: Any) -> Any:
        raise AssertionError("capability ownership check accessed an execution port")

    def cancel(self, *_args: Any, **_kwargs: Any) -> Any:
        raise AssertionError("capability ownership check accessed a cancellation port")


def _capability_client(environment: dict[str, str]) -> _AsgiClient:
    app = FastAPI()
    never_accessed = _NeverAccessedPort()
    app.include_router(
        build_k002_internal_router(
            never_accessed,  # type: ignore[arg-type]
            never_accessed,  # type: ignore[arg-type]
            environment=environment,
        )
    )
    return _AsgiClient(app)


def _key(label: str) -> str:
    return canonical_json_sha256({"test": label})


def _thread_command(label: str = "create_thread") -> dict:
    return {
        "schema_version": "LifecyclePersistenceCommand@1",
        "command_id": f"command_{label}",
        "idempotency_key": _key(label),
        "command": {
            "operation": "create_thread",
            "thread": {
                "thread_id": f"thread_{label}",
                "project_id": "project_k002_internal",
                "title": "K002 internal lifecycle",
                "status": "idle",
                "summary": "",
                "provider_id": "deepseek",
                "created_at": "2026-07-17T00:00:00Z",
                "updated_at": "2026-07-17T00:00:00Z",
            },
        },
    }


def _load_command(thread_id: str, label: str) -> dict:
    return {
        "schema_version": "LifecyclePersistenceCommand@1",
        "command_id": f"command_{label}",
        "idempotency_key": _key(label),
        "command": {
            "operation": "load_thread",
            "thread_id": thread_id,
        },
    }


def _tool_request(
    *,
    execution_id: str = "execution_internal_1",
    call_id: str = "call_internal_1",
    cancellation_id: str = "cancel_internal_1",
    cancellation_token: str = "cancel_token_internal_1",
    arguments: dict | None = None,
) -> dict:
    registry = forgecad_product_tool_registry()
    tool = registry.require("infer_product_domain")
    values = arguments or {"brief": "设计一个非功能性未来机械概念道具"}
    return {
        "schema_version": "ProductToolExecutionRequest@1",
        "execution_id": execution_id,
        "turn_id": "turn_internal_1",
        "call_id": call_id,
        "tool_id": tool.tool_id,
        "tool_name": tool.name,
        "registry_schema_version": "ForgeCADProductToolRegistry@1",
        "idempotency_key": canonical_json_sha256(
            {
                "turn_id": "turn_internal_1",
                "call_id": call_id,
                "tool_id": tool.tool_id,
                "arguments": values,
            }
        ),
        "validated_arguments": {
            "schema_id": f"{tool.tool_id}:input",
            "schema_sha256": canonical_json_sha256(tool.input_schema),
            "value": values,
        },
        "approval_policy": tool.approval_policy,
        "cancellation_id": cancellation_id,
        "cancellation_token": cancellation_token,
    }


def test_default_public_python_lifecycle_mutations_are_rust_owned(
    tmp_path: Path,
) -> None:
    client = _public_and_internal_client(tmp_path)
    mutation_requests = (
        (
            "/api/v1/agent/threads",
            {
                "client_request_id": "request_default_thread",
                "title": "must not be written by Python",
                "provider_id": "deterministic_kernel",
            },
            {"Idempotency-Key": _key("default_thread")},
        ),
        ("/api/v1/agent/provider:check", None, None),
        ("/api/v1/agent/provider-checks/check_default/cancel", None, None),
        (
            "/api/v1/agent/threads/thread_missing/turns",
            {
                "client_request_id": "request_default_turn",
                "message": "must not run through the Python lifecycle",
            },
            {"Idempotency-Key": _key("default_turn")},
        ),
        (
            "/api/v1/agent/turns/turn_missing/cancel",
            None,
            {"Idempotency-Key": _key("default_cancel")},
        ),
        (
            "/api/v1/agent/threads/thread_missing/approvals",
            {
                "client_request_id": "request_default_approval",
                "turn_id": "turn_missing",
                "action": "confirm_preview",
                "payload": {},
            },
            {"Idempotency-Key": _key("default_approval")},
        ),
        (
            "/api/v1/agent/approvals/approval_missing/resolve",
            {
                "client_request_id": "request_default_resolution",
                "decision": "rejected",
                "note": "Rust owns this mutation",
            },
            {"Idempotency-Key": _key("default_resolution")},
        ),
    )

    for path, body, headers in mutation_requests:
        response = client.post(path, json=body, headers=headers)
        assert response.status_code == 410, (path, response.text)
        assert response.json()["error"] == {
            "code": RUST_OWNED_AGENT_LIFECYCLE_CODE,
            "message": (
                "Agent Thread, Turn, Approval and Provider lifecycle is owned "
                "by the Rust app-server."
            ),
            "recoverable": False,
            "details": {},
        }

    bodyless = client.post("/api/v1/agent/threads")
    assert bodyless.status_code == 410
    assert bodyless.json()["error"]["code"] == RUST_OWNED_AGENT_LIFECYCLE_CODE

    # K003 moves every lifecycle read, including list/detail/SSE replay, to
    # the Rust app-server. The Python compatibility router must not become a
    # second read model merely because those endpoints are GET requests.
    for path in (
        "/api/v1/agent/threads",
        "/api/v1/agent/threads/thread_missing",
        "/api/v1/agent/threads/thread_missing/events",
    ):
        response = client.get(path)
        assert response.status_code == 410, (path, response.text)
        assert response.json()["error"]["code"] == RUST_OWNED_PRODUCT_STATE_CODE

    # Static registry metadata is not lifecycle/product state and remains
    # readable from the restricted boundary.
    assert client.get("/api/v1/agent/product-tools").status_code == 200
    provider = client.get("/api/v1/agent/provider")
    assert provider.status_code == 410
    assert provider.json()["error"]["code"] == RUST_OWNED_AGENT_LIFECYCLE_CODE

    # The capability-gated Rust-to-Python K002 persistence port remains the
    # sole transitional write path until K003 removes Python persistence.
    internal = client.post(
        f"{K002_INTERNAL_PREFIX}/lifecycle/execute",
        headers=_headers(),
        json=_thread_command("rust_owner"),
    )
    assert internal.status_code == 200
    assert internal.json()["result"]["thread_id"] == "thread_rust_owner"


def test_explicit_test_only_switch_keeps_k001_legacy_oracle_available(
    tmp_path: Path,
) -> None:
    client = _public_and_internal_client(
        tmp_path,
        allow_test_only_legacy_lifecycle=True,
    )
    created = client.post(
        "/api/v1/agent/threads",
        headers={"Idempotency-Key": _key("legacy_k001_thread")},
        json={
            "client_request_id": "request_legacy_k001_thread",
            "title": "K001 compatibility oracle",
            "provider_id": "deterministic_kernel",
        },
    )

    assert created.status_code == 201
    assert created.json()["title"] == "K001 compatibility oracle"
    assert created.json()["provider_id"] == "deterministic_kernel"
    listed = client.get("/api/v1/agent/threads")
    assert listed.status_code == 200
    assert [thread["thread_id"] for thread in listed.json()["items"]] == [
        created.json()["thread_id"]
    ]
    detail = client.get(f"/api/v1/agent/threads/{created.json()['thread_id']}")
    assert detail.status_code == 200
    assert detail.json()["thread_id"] == created.json()["thread_id"]
    events = client.get(f"/api/v1/agent/threads/{created.json()['thread_id']}/events")
    assert events.status_code == 200
    provider = client.get("/api/v1/agent/provider")
    assert provider.status_code == 200
    assert provider.json()["network_call_made"] is False

    ownership = client.get(
        f"{K002_INTERNAL_PREFIX}/capability/ownership",
        headers=_headers(),
    )
    assert ownership.status_code == 200
    assert ownership.json()["lifecycle_owner"] == "rust_app_server"


def test_legacy_lifecycle_environment_requires_both_k001_probe_flags() -> None:
    assert not legacy_lifecycle_compat_enabled({})
    assert not legacy_lifecycle_compat_enabled(
        {TEST_ONLY_LEGACY_AGENT_LIFECYCLE_ENV: "1"}
    )
    assert not legacy_lifecycle_compat_enabled({K001_PACKAGED_PROBE_ENV: "1"})
    assert legacy_lifecycle_compat_enabled(
        {
            TEST_ONLY_LEGACY_AGENT_LIFECYCLE_ENV: "1",
            K001_PACKAGED_PROBE_ENV: "1",
        }
    )


def test_internal_capability_is_snapshotted_and_rejects_missing_wrong_and_oversized(
    tmp_path: Path,
) -> None:
    unavailable_environment: dict[str, str] = {}
    unavailable = _client(tmp_path / "unavailable", environment=unavailable_environment)
    unavailable_environment[K002_INTERNAL_CAPABILITY_TOKEN_ENV] = CAPABILITY
    response = unavailable.post(
        f"{K002_INTERNAL_PREFIX}/lifecycle/execute",
        headers=_headers(),
        json=_thread_command("unavailable"),
    )
    assert response.status_code == 503
    assert response.json()["error"]["code"] == "K002_INTERNAL_PORT_UNAVAILABLE"

    client = _client(tmp_path / "available")
    body = _thread_command("authenticated")
    missing = client.post(f"{K002_INTERNAL_PREFIX}/lifecycle/execute", json=body)
    assert missing.status_code == 403
    assert missing.json()["error"]["code"] == "K002_INTERNAL_CAPABILITY_REJECTED"

    wrong_token = "wrong-k002-internal-capability"
    wrong = client.post(
        f"{K002_INTERNAL_PREFIX}/lifecycle/execute",
        headers=_headers(wrong_token),
        json=body,
    )
    assert wrong.status_code == 403
    assert wrong_token not in wrong.text

    oversized_token = "z" * 257
    oversized = client.post(
        f"{K002_INTERNAL_PREFIX}/lifecycle/execute",
        headers=_headers(oversized_token),
        json=body,
    )
    assert oversized.status_code == 403
    assert oversized_token not in oversized.text

    accepted = client.post(
        f"{K002_INTERNAL_PREFIX}/lifecycle/execute",
        headers=_headers(),
        json=body,
    )
    assert accepted.status_code == 200
    assert accepted.json()["result"]["thread_id"] == "thread_authenticated"


def test_capability_ownership_is_fixed_authorized_and_has_no_port_side_effects() -> None:
    environment = {K002_INTERNAL_CAPABILITY_TOKEN_ENV: CAPABILITY}
    client = _capability_client(environment)
    path = f"{K002_INTERNAL_PREFIX}/capability/ownership"

    missing = client.get(path)
    assert missing.status_code == 403
    assert missing.json()["error"]["code"] == "K002_INTERNAL_CAPABILITY_REJECTED"

    wrong_token = "wrong-k002-ownership-capability"
    wrong = client.get(path, headers=_headers(wrong_token))
    assert wrong.status_code == 403
    assert wrong.json()["error"]["code"] == "K002_INTERNAL_CAPABILITY_REJECTED"
    assert wrong_token not in wrong.text

    accepted = client.get(path, headers=_headers())
    assert accepted.status_code == 200
    assert accepted.json() == {
        "schema_version": "K002InternalCapabilityOwnership@1",
        "capability_owner": "rust_app_server",
        "port_owner": "python_compatibility_service",
        "lifecycle_owner": "rust_app_server",
    }
    assert CAPABILITY not in accepted.text

    unavailable_environment: dict[str, str] = {}
    unavailable = _capability_client(unavailable_environment)
    unavailable_environment[K002_INTERNAL_CAPABILITY_TOKEN_ENV] = CAPABILITY
    disabled = unavailable.get(path, headers=_headers())
    assert disabled.status_code == 503
    assert disabled.json()["error"]["code"] == "K002_INTERNAL_PORT_UNAVAILABLE"


def test_internal_lifecycle_persists_and_reads_through_strict_dto(tmp_path: Path) -> None:
    client = _client(tmp_path)
    created = client.post(
        f"{K002_INTERNAL_PREFIX}/lifecycle/execute",
        headers=_headers(),
        json=_thread_command("persisted"),
    )
    assert created.status_code == 200
    assert created.json()["schema_version"] == "LifecyclePersistenceResult@1"

    loaded = client.post(
        f"{K002_INTERNAL_PREFIX}/lifecycle/execute",
        headers=_headers(),
        json=_load_command("thread_persisted", "load_persisted"),
    )
    assert loaded.status_code == 200
    detail = loaded.json()["result"]["thread"]
    assert detail["thread_id"] == "thread_persisted"
    assert detail["status"] == "idle"


def test_internal_product_tool_executes_and_cancel_before_start_discards_result(
    tmp_path: Path,
) -> None:
    client = _client(tmp_path)
    cancellation_id = "cancel_before_start"
    cancellation_token = "cancel_token_before_start"
    cancelled = client.post(
        f"{K002_INTERNAL_PREFIX}/product-tools/cancel",
        headers=_headers(),
        json={
            "schema_version": "ProductToolCancellationRequest@1",
            "cancellation_id": cancellation_id,
            "cancellation_token": cancellation_token,
        },
    )
    assert cancelled.status_code == 200
    assert cancelled.json() == {
        "schema_version": "ProductToolCancellationResult@1",
        "cancellation_id": cancellation_id,
        "accepted": True,
    }

    result = client.post(
        f"{K002_INTERNAL_PREFIX}/product-tools/execute",
        headers=_headers(),
        json=_tool_request(
            cancellation_id=cancellation_id,
            cancellation_token=cancellation_token,
        ),
    )
    assert result.status_code == 200
    assert result.json()["status"] == "cancelled"
    assert result.json()["permanent_side_effects"] == 0
    assert result.json()["validated_output"] is None

    completed = client.post(
        f"{K002_INTERNAL_PREFIX}/product-tools/execute",
        headers=_headers(),
        json=_tool_request(
            execution_id="execution_internal_2",
            call_id="call_internal_2",
            cancellation_id="cancel_internal_2",
            cancellation_token="cancel_token_internal_2",
        ),
    )
    assert completed.status_code == 200
    assert completed.json()["status"] == "completed"
    assert completed.json()["permanent_side_effects"] == 0


def test_internal_routes_are_hidden_bounded_and_do_not_echo_forbidden_context(
    tmp_path: Path,
) -> None:
    client = _client(tmp_path)
    assert not any(
        path.startswith(K002_INTERNAL_PREFIX)
        for path in client.get("/openapi.json").json()["paths"]
    )

    oversized = client.post(
        f"{K002_INTERNAL_PREFIX}/product-tools/execute",
        headers=_headers(),
        content=b"{" + b"x" * MAX_PORT_JSON_BYTES + b"}",
    )
    assert oversized.status_code == 413
    assert oversized.json()["error"]["code"] == "K002_INTERNAL_PAYLOAD_TOO_LARGE"

    provider_secret = "must-never-appear-in-response"
    forbidden = client.post(
        f"{K002_INTERNAL_PREFIX}/product-tools/execute",
        headers=_headers(),
        json=_tool_request(
            execution_id="execution_forbidden",
            call_id="call_forbidden",
            cancellation_id="cancel_forbidden",
            cancellation_token="cancel_token_forbidden",
            arguments={"brief": "safe", "provider_key": provider_secret},
        ),
    )
    assert forbidden.status_code == 400
    assert forbidden.json()["error"]["code"] == "K002_FORBIDDEN_CONTEXT_FIELD"
    assert provider_secret not in forbidden.text


def test_internal_ports_reject_provider_environment_without_exposing_secret(
    tmp_path: Path,
) -> None:
    provider_secret = "deepseek-secret-must-not-cross-python-port"
    environment = {
        K002_INTERNAL_CAPABILITY_TOKEN_ENV: CAPABILITY,
        "DEEPSEEK_API_KEY": provider_secret,
    }
    client = _client(tmp_path, environment=environment)

    lifecycle = client.post(
        f"{K002_INTERNAL_PREFIX}/lifecycle/execute",
        headers=_headers(),
        json=_thread_command("provider_rejected"),
    )
    assert lifecycle.status_code == 403
    assert lifecycle.json()["error"]["code"] == "K002_PROVIDER_ENVIRONMENT_FORBIDDEN"
    assert provider_secret not in lifecycle.text

    product_tool = client.post(
        f"{K002_INTERNAL_PREFIX}/product-tools/execute",
        headers=_headers(),
        json=_tool_request(),
    )
    assert product_tool.status_code == 403
    assert product_tool.json()["error"]["code"] == "K002_PROVIDER_ENVIRONMENT_FORBIDDEN"
    assert provider_secret not in product_tool.text
