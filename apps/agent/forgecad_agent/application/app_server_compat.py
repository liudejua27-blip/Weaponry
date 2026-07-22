from __future__ import annotations

import asyncio
import base64
import binascii
import hashlib
import json
import re
import uuid
from dataclasses import dataclass, field
from typing import Any, Iterable, Mapping, Optional
from urllib.parse import urlsplit

from fastapi import FastAPI

from .agent_models import AgentEvent


JSONRPC_VERSION = "2.0"
PROTOCOL_VERSION = "forgecad.app-server/1"
INITIALIZE_PARAMS_SCHEMA = "ForgeCADInitializeParams@1"
INITIALIZE_RESULT_SCHEMA = "ForgeCADInitializeResult@1"
HTTP_REQUEST_SCHEMA = "ForgeCADHttpCompatibilityRequest@1"
HTTP_RESPONSE_SCHEMA = "ForgeCADHttpCompatibilityResponse@1"
SSE_SUBSCRIPTION_SCHEMA = "ForgeCADSseSubscription@1"
SSE_UNSUBSCRIBE_SCHEMA = "ForgeCADSseUnsubscribe@1"
SSE_NOTIFICATION_SCHEMA = "ForgeCADSseNotification@1"

# A 47 MiB binary leaves bounded room for base64 padding plus the JSON-RPC
# envelope inside a 64 MiB frame.  This is deliberately larger than the old
# 1 MiB JSON default so M108A production GLB/render artifacts can cross the
# temporary bridge.  Neither request nor response bodies are persisted here.
MAX_FRAME_BYTES = 64 * 1024 * 1024
MAX_RAW_BODY_BYTES = 47 * 1024 * 1024
DEFAULT_EVENT_QUEUE_CAPACITY = 128
MAX_EVENT_QUEUE_CAPACITY = 4096
MAX_IN_FLIGHT_REQUESTS = 32

PARSE_ERROR = -32700
INVALID_REQUEST = -32600
METHOD_NOT_FOUND = -32601
INVALID_PARAMS = -32602
INTERNAL_ERROR = -32603
SERVER_OVERLOADED = -32001
NOT_INITIALIZED = -32002
PROTOCOL_VERSION_UNSUPPORTED = -32003
ALREADY_INITIALIZED = -32004
DUPLICATE_REQUEST_ID = -32005
UNKNOWN_REQUEST_ID = -32006
REQUEST_CANCELLED = -32007
CURSOR_RESYNC_REQUIRED = -32008
COMPAT_BACKEND_UNAVAILABLE = -32009
MALFORMED_UPSTREAM_EVENT = -32010
SLOW_CONSUMER = -32011
INPUT_TOO_LARGE = -32012
CAPABILITY_UNSUPPORTED = -32013

_STABLE_ID = re.compile(r"^[A-Za-z0-9_.-]{1,160}$")
_TURN_START_PATH = re.compile(r"^/api/v1/agent/threads/([A-Za-z0-9_.-]{1,160})/turns$")
_FORBIDDEN_ENCODED_PATH_PARTS = ("%2e", "%2f", "%5c", "%00")
_ALLOWED_HEADERS = {
    "accept",
    "content-type",
    "if-match",
    "if-none-match",
    "range",
    "cache-control",
    "last-event-id",
    "idempotency-key",
    "x-client-request-id",
    "x-provider-check-id",
}
_LEGACY_PRODUCT_API_PREFIXES = (
    "/api/weapons",
    "/api/assets",
    "/api/jobs",
    "/api/runtime",
    "/api/provider-settings",
)
_REQUIRED_CAPABILITIES = {
    "notifications": True,
    "cursor_replay": True,
    "cancellation": True,
    "notification_ack": True,
    "binary_body_base64": True,
}


class ProtocolFailure(Exception):
    def __init__(
        self,
        code: int,
        application_code: str,
        message: str,
        *,
        recoverable: bool = False,
        details: Optional[dict[str, Any]] = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.application_code = application_code
        self.recoverable = recoverable
        self.details = details or {}

    def envelope(self, request_id: Optional[str] = None) -> dict[str, Any]:
        data: dict[str, Any] = {
            "schema_version": "ForgeCADProtocolError@1",
            "application_code": self.application_code,
            "recoverable": self.recoverable,
        }
        if self.details:
            data["details"] = self.details
        if request_id is not None:
            data["request_id"] = request_id
        return {"code": self.code, "message": str(self), "data": data}


@dataclass(frozen=True)
class AsgiResponse:
    status: int
    headers: list[tuple[str, str]]
    body: bytes


class InProcessAsgiClient:
    """A tiny dependency-free ASGI client for the code-owned compatibility port."""

    def __init__(self, app: FastAPI, *, client_host: str = "127.0.0.1") -> None:
        self._app = app
        self._client_host = client_host

    async def request(
        self,
        method: str,
        path: str,
        *,
        headers: Iterable[tuple[str, str]] = (),
        body: bytes = b"",
    ) -> AsgiResponse:
        parsed = urlsplit(path)
        scope = {
            "type": "http",
            "asgi": {"version": "3.0", "spec_version": "2.3"},
            "http_version": "1.1",
            "method": method,
            "scheme": "http",
            "path": parsed.path,
            "raw_path": parsed.path.encode("utf-8"),
            "query_string": parsed.query.encode("ascii"),
            "root_path": "",
            "headers": [
                (name.lower().encode("ascii"), value.encode("latin-1"))
                for name, value in headers
            ],
            "client": (self._client_host, 0),
            "server": ("forgecad-python-compatibility-adapter", 80),
        }
        started: Optional[dict[str, Any]] = None
        chunks: list[bytes] = []
        received = False
        never_disconnect = asyncio.Event()

        async def receive() -> dict[str, Any]:
            nonlocal received
            if not received:
                received = True
                return {"type": "http.request", "body": body, "more_body": False}
            # StreamingResponse races its producer against a disconnect
            # listener.  Waiting here lets a finite SSE producer finish and be
            # collected without falsely simulating an early disconnect.
            await never_disconnect.wait()
            return {"type": "http.disconnect"}

        async def send(message: dict[str, Any]) -> None:
            nonlocal started
            if message["type"] == "http.response.start":
                started = message
            elif message["type"] == "http.response.body":
                chunks.append(bytes(message.get("body", b"")))

        await self._app(scope, receive, send)
        if started is None:
            raise ProtocolFailure(
                COMPAT_BACKEND_UNAVAILABLE,
                "ADAPTER_UNAVAILABLE",
                "The Python compatibility backend returned no HTTP response.",
                recoverable=True,
            )
        response_headers = [
            (name.decode("latin-1"), value.decode("latin-1"))
            for name, value in started.get("headers", [])
        ]
        return AsgiResponse(
            status=int(started["status"]),
            headers=response_headers,
            body=b"".join(chunks),
        )


@dataclass
class QueuedNotification:
    transport_cursor: int
    frame: dict[str, Any]


@dataclass
class PendingRequest:
    request_id: str
    method: str
    path: Optional[str] = None
    cancellation_requested: bool = False
    task: Optional[asyncio.Task[Any]] = None


@dataclass
class CompatibilityConnection:
    connection_id: str
    queue_capacity: int
    state: str = "opened"
    request_fingerprints: dict[str, str] = field(default_factory=dict)
    completed_responses: dict[str, dict[str, Any]] = field(default_factory=dict)
    notifications: list[QueuedNotification] = field(default_factory=list)
    next_transport_cursor: int = 1
    pending: dict[str, PendingRequest] = field(default_factory=dict)
    streams: set[str] = field(default_factory=set)
    lock: asyncio.Lock = field(default_factory=asyncio.Lock)


class AppServerCompatibilityAdapter:
    """K001 Python oracle for the Rust-owned app-server wire contract.

    This class owns only ephemeral connections, request IDs and notification
    queues.  Every product read/write is executed through the existing FastAPI
    app, so Python remains the sole persistent state owner during K001.
    """

    def __init__(
        self,
        app: FastAPI,
        *,
        max_frame_bytes: int = MAX_FRAME_BYTES,
        max_raw_body_bytes: int = MAX_RAW_BODY_BYTES,
        default_queue_capacity: int = DEFAULT_EVENT_QUEUE_CAPACITY,
    ) -> None:
        self._client = InProcessAsgiClient(app)
        self._max_frame_bytes = max_frame_bytes
        self._max_raw_body_bytes = max_raw_body_bytes
        self._default_queue_capacity = default_queue_capacity
        self._connections: dict[str, CompatibilityConnection] = {}
        self._connections_lock = asyncio.Lock()

    async def open_connection(self, queue_capacity: Optional[int] = None) -> str:
        capacity = self._default_queue_capacity if queue_capacity is None else queue_capacity
        if isinstance(capacity, bool) or not isinstance(capacity, int) or not 1 <= capacity <= MAX_EVENT_QUEUE_CAPACITY:
            raise ProtocolFailure(
                INVALID_PARAMS,
                "INVALID_PARAMS",
                f"queue_capacity must be between 1 and {MAX_EVENT_QUEUE_CAPACITY}.",
            )
        connection_id = f"conn_{uuid.uuid4().hex}"
        async with self._connections_lock:
            self._connections[connection_id] = CompatibilityConnection(connection_id, capacity)
        return connection_id

    async def close_connection(self, connection_id: str) -> bool:
        async with self._connections_lock:
            connection = self._connections.pop(connection_id, None)
        if connection is None:
            return False
        async with connection.lock:
            connection.state = "closed"
            pending = list(connection.pending.values())
            for request in pending:
                request.cancellation_requested = True
            connection.notifications.clear()
            connection.streams.clear()
        # Closing/reconnecting a browser transport must propagate cancellation
        # to the same in-flight work that a Rust disconnect would cancel. Turn
        # requests first receive a durable product cancellation; all adapter
        # tasks are then stopped so no late compatibility response can escape.
        for request in pending:
            match = _TURN_START_PATH.fullmatch(request.path or "")
            if match is not None:
                await self._cancel_running_turn(match.group(1), request.request_id)
        current = asyncio.current_task()
        tasks = [
            request.task
            for request in pending
            if request.task is not None and request.task is not current and not request.task.done()
        ]
        for task in tasks:
            task.cancel()
        if tasks:
            await asyncio.gather(*tasks, return_exceptions=True)
        async with connection.lock:
            connection.pending.clear()
        return True

    async def close_all(self) -> None:
        async with self._connections_lock:
            connection_ids = list(self._connections)
        for connection_id in connection_ids:
            await self.close_connection(connection_id)

    async def send_frame(self, connection_id: str, frame: Any) -> dict[str, Any]:
        connection = await self._require_connection(connection_id)
        request_id: Optional[str] = None
        registered_request = False
        try:
            message = self._parse_client_frame(frame)
            request_id = message.get("id")
            method = message["method"]
            is_request = request_id is not None

            if is_request:
                fingerprint = canonical_json_sha256(message)
                async with connection.lock:
                    existing_fingerprint = connection.request_fingerprints.get(request_id)
                    if existing_fingerprint is not None and existing_fingerprint != fingerprint:
                        raise ProtocolFailure(
                            DUPLICATE_REQUEST_ID,
                            "DUPLICATE_REQUEST_ID",
                            "The JSON-RPC request ID was reused with a different canonical request.",
                        )
                    completed = connection.completed_responses.get(request_id)
                    if completed is not None:
                        return {"frame": completed}
                    if existing_fingerprint is not None:
                        raise ProtocolFailure(
                            SERVER_OVERLOADED,
                            "REQUEST_IN_FLIGHT",
                            "The identical JSON-RPC request is already in flight.",
                            recoverable=True,
                        )
                    in_flight = sum(
                        1
                        for active_id in connection.request_fingerprints
                        if active_id not in connection.completed_responses
                    )
                    if in_flight >= MAX_IN_FLIGHT_REQUESTS:
                        raise ProtocolFailure(
                            SERVER_OVERLOADED,
                            "SERVER_OVERLOADED",
                            "The connection reached its bounded in-flight request limit.",
                            recoverable=True,
                        )
                    connection.request_fingerprints[request_id] = fingerprint
                    registered_request = True

            if method == "initialize":
                if not is_request:
                    raise ProtocolFailure(INVALID_REQUEST, "INVALID_REQUEST", "initialize must be a request.")
                result = await self._initialize(connection, message["params"])
                response = self._success(request_id, result)
                self._check_outgoing_frame_size(response)
                await self._cache_completed_response(connection, request_id, response)
                return {"frame": response}

            if method == "initialized":
                if is_request:
                    raise ProtocolFailure(INVALID_REQUEST, "INVALID_REQUEST", "initialized must be a notification.")
                await self._initialized(connection, message["params"])
                return {"frames": []}

            if connection.state != "ready":
                raise ProtocolFailure(
                    NOT_INITIALIZED,
                    "NOT_INITIALIZED",
                    "The connection must complete initialize/initialized before this method is used.",
                )

            if method == "notification/ack":
                result = await self._acknowledge(connection, message["params"])
                if is_request:
                    response = self._success(request_id, result)
                    await self._cache_completed_response(connection, request_id, response)
                    return {"frame": response}
                return {"frames": []}

            if method == "request/cancel":
                result = await self._cancel_request(connection, message["params"])
                if is_request:
                    response = self._success(request_id, result)
                    await self._cache_completed_response(connection, request_id, response)
                    return {"frame": response}
                return {"frames": []}

            if not is_request:
                raise ProtocolFailure(
                    METHOD_NOT_FOUND,
                    "METHOD_NOT_FOUND",
                    "The requested app-server notification is not registered.",
                    details={"method": method},
                )

            pending = PendingRequest(request_id=request_id, method=method)
            pending.task = asyncio.current_task()
            async with connection.lock:
                connection.pending[request_id] = pending
            notifications: list[dict[str, Any]] = []
            try:
                if method == "compat/http":
                    result = await self._compat_http(connection, pending, message["params"])
                elif method == "compat/subscribe":
                    result, notifications = await self._subscribe(connection, message["params"])
                elif method == "compat/unsubscribe":
                    result = await self._unsubscribe(connection, message["params"])
                elif method == "thread/events/replay":
                    result, notifications = await self._replay_events(connection, message["params"])
                else:
                    raise ProtocolFailure(
                        METHOD_NOT_FOUND,
                        "METHOD_NOT_FOUND",
                        "The requested app-server method is not registered.",
                        details={"method": method},
                    )
            finally:
                async with connection.lock:
                    connection.pending.pop(request_id, None)

            response = self._success(request_id, result)
            self._check_outgoing_frame_size(response)
            if notifications:
                queued = await self._enqueue_notifications(connection, notifications)
                if not queued:
                    failure = ProtocolFailure(
                        SLOW_CONSUMER,
                        "SLOW_CONSUMER",
                        "The bounded event queue cannot accept this notification batch; replay is required.",
                        recoverable=True,
                    )
                    response = self._failure(request_id, failure)
                    await self._cache_completed_response(connection, request_id, response)
                    return {"frame": response, "frames": [record.frame for record in connection.notifications]}
            output: dict[str, Any] = {"frame": response}
            if notifications:
                output["frames"] = notifications
            await self._cache_completed_response(connection, request_id, response)
            return output
        except ProtocolFailure as exc:
            response = self._failure(request_id, exc)
            self._check_outgoing_frame_size(response)
            if registered_request and request_id is not None:
                await self._cache_completed_response(connection, request_id, response)
            return {"frame": response}
        except asyncio.CancelledError:
            response = self._failure(
                request_id,
                ProtocolFailure(REQUEST_CANCELLED, "REQUEST_CANCELLED", "Request cancelled.", recoverable=True),
            )
            if registered_request and request_id is not None:
                await self._cache_completed_response(connection, request_id, response)
            return {"frame": response}
        except Exception:
            # The wire must never expose Python tracebacks, paths or secrets.
            response = self._failure(
                request_id,
                ProtocolFailure(INTERNAL_ERROR, "INTERNAL_ERROR", "The compatibility adapter failed safely."),
            )
            if registered_request and request_id is not None:
                await self._cache_completed_response(connection, request_id, response)
            return {"frame": response}

    async def _cache_completed_response(
        self,
        connection: CompatibilityConnection,
        request_id: Optional[str],
        response: dict[str, Any],
    ) -> None:
        if request_id is None:
            return
        async with connection.lock:
            connection.completed_responses[request_id] = response

    async def replay_transport(self, connection_id: str, after: int = 0) -> dict[str, Any]:
        connection = await self._require_connection(connection_id)
        if isinstance(after, bool) or not isinstance(after, int) or after < 0:
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "after must be a non-negative integer.")
        async with connection.lock:
            if connection.notifications:
                earliest = connection.notifications[0].transport_cursor
                if after and after < earliest - 1:
                    raise ProtocolFailure(
                        CURSOR_RESYNC_REQUIRED,
                        "CURSOR_RESYNC_REQUIRED",
                        "The requested transport cursor is no longer available.",
                        recoverable=True,
                    )
            frames = [
                {"cursor": record.transport_cursor, "frame": record.frame}
                for record in connection.notifications
                if record.transport_cursor > after
            ]
            return {"frames": frames, "latest_cursor": connection.next_transport_cursor - 1}

    async def _require_connection(self, connection_id: str) -> CompatibilityConnection:
        async with self._connections_lock:
            connection = self._connections.get(connection_id)
        if connection is None or connection.state == "closed":
            raise ProtocolFailure(UNKNOWN_REQUEST_ID, "CONNECTION_NOT_FOUND", "The app-server connection was not found.")
        return connection

    def _parse_client_frame(self, frame: Any) -> dict[str, Any]:
        if not isinstance(frame, dict):
            raise ProtocolFailure(INVALID_REQUEST, "INVALID_REQUEST", "A JSON-RPC frame must be an object.")
        try:
            encoded = _canonical_json(frame).encode("utf-8")
        except (TypeError, ValueError):
            raise ProtocolFailure(PARSE_ERROR, "PARSE_ERROR", "The JSON-RPC frame is not valid JSON.") from None
        if len(encoded) > self._max_frame_bytes:
            raise ProtocolFailure(INPUT_TOO_LARGE, "INPUT_TOO_LARGE", "The JSON-RPC frame exceeds the negotiated byte limit.")
        allowed = {"jsonrpc", "id", "method", "params"}
        if set(frame) - allowed:
            raise ProtocolFailure(INVALID_REQUEST, "INVALID_REQUEST", "The JSON-RPC frame contains unknown fields.")
        if frame.get("jsonrpc") != JSONRPC_VERSION:
            raise ProtocolFailure(INVALID_REQUEST, "JSONRPC_VERSION_INVALID", 'The jsonrpc field must be exactly "2.0".')
        method = frame.get("method")
        if not isinstance(method, str) or not 1 <= len(method) <= 160:
            raise ProtocolFailure(INVALID_REQUEST, "METHOD_INVALID", "The JSON-RPC method is invalid.")
        if "id" in frame:
            request_id = frame["id"]
            if not isinstance(request_id, str) or not request_id.isascii() or not 1 <= len(request_id) <= 160:
                raise ProtocolFailure(INVALID_REQUEST, "INVALID_REQUEST_ID", "Request IDs must contain 1 to 160 ASCII bytes.")
        params = frame.get("params", {})
        if not isinstance(params, dict):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "JSON-RPC params must be an object.")
        return {**frame, "params": params}

    async def _initialize(self, connection: CompatibilityConnection, params: dict[str, Any]) -> dict[str, Any]:
        async with connection.lock:
            if connection.state != "opened":
                raise ProtocolFailure(ALREADY_INITIALIZED, "ALREADY_INITIALIZED", "The connection is already initialized.")
            _require_exact_keys(params, {"schema_version", "supported_protocol_versions", "client_info", "capabilities"})
            if params.get("schema_version") != INITIALIZE_PARAMS_SCHEMA:
                raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", f"schema_version must be {INITIALIZE_PARAMS_SCHEMA}.")
            versions = params.get("supported_protocol_versions")
            if not isinstance(versions, list) or not all(isinstance(value, str) for value in versions):
                raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "supported_protocol_versions must be a string array.")
            if PROTOCOL_VERSION not in versions:
                raise ProtocolFailure(
                    PROTOCOL_VERSION_UNSUPPORTED,
                    "PROTOCOL_VERSION_UNSUPPORTED",
                    "The client and server do not share a ForgeCAD app-server protocol version.",
                )
            client_info = params.get("client_info")
            if not isinstance(client_info, dict):
                raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "client_info must be an object.")
            _require_exact_keys(client_info, {"name", "version", "transport"})
            if (
                not _bounded_text(client_info.get("name"), 1, 160)
                or not _bounded_text(client_info.get("version"), 1, 160)
                or client_info.get("transport") not in {"tauri", "browser-loopback-compatibility"}
            ):
                raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "client_info is invalid.")
            capabilities = params.get("capabilities")
            if not isinstance(capabilities, dict):
                raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "capabilities must be an object.")
            _require_exact_keys(capabilities, set(_REQUIRED_CAPABILITIES))
            if capabilities != _REQUIRED_CAPABILITIES:
                raise ProtocolFailure(
                    CAPABILITY_UNSUPPORTED,
                    "CAPABILITY_UNSUPPORTED",
                    "ForgeCAD protocol v1 requires notifications, replay, cancellation, acknowledgements, and base64 binary bodies.",
                )
            connection.state = "initialize_accepted"
            return {
                "schema_version": INITIALIZE_RESULT_SCHEMA,
                "protocol_version": PROTOCOL_VERSION,
                "connection_id": connection.connection_id,
                "server_info": {"name": "forgecad-python-compatibility-adapter", "version": "1"},
                "capabilities": dict(_REQUIRED_CAPABILITIES),
                "limits": {
                    "max_in_flight_requests": MAX_IN_FLIGHT_REQUESTS,
                    "max_event_queue": connection.queue_capacity,
                    "max_frame_bytes": self._max_frame_bytes,
                },
                "migration_state": {"state_owner": "python_compatibility_adapter"},
            }

    async def _initialized(self, connection: CompatibilityConnection, params: dict[str, Any]) -> None:
        _require_exact_keys(params, {"protocol_version"})
        if params.get("protocol_version") != PROTOCOL_VERSION:
            raise ProtocolFailure(PROTOCOL_VERSION_UNSUPPORTED, "PROTOCOL_VERSION_UNSUPPORTED", "initialized protocol_version is unsupported.")
        async with connection.lock:
            if connection.state != "initialize_accepted":
                raise ProtocolFailure(ALREADY_INITIALIZED, "ALREADY_INITIALIZED", "initialized was received out of order.")
            connection.state = "ready"

    async def _compat_http(
        self,
        connection: CompatibilityConnection,
        pending: PendingRequest,
        params: dict[str, Any],
    ) -> dict[str, Any]:
        del connection
        method, path, headers, body = self._prepare_http_request(params)
        pending.path = urlsplit(path).path
        response = await self._client.request(method, path, headers=headers, body=body)
        if pending.cancellation_requested and _TURN_START_PATH.fullmatch(pending.path or "") is None:
            raise ProtocolFailure(
                REQUEST_CANCELLED,
                "REQUEST_CANCELLED",
                "The compatibility request was cancelled; a late result was discarded.",
                recoverable=True,
            )
        if len(response.body) > self._max_raw_body_bytes:
            raise ProtocolFailure(INPUT_TOO_LARGE, "COMPAT_BODY_TOO_LARGE", "The compatibility response body exceeds the bounded adapter limit.")
        response_body: dict[str, Any]
        if not response.body:
            response_body = {"encoding": "empty"}
        else:
            response_body = {"encoding": "base64", "data": base64.b64encode(response.body).decode("ascii")}
        result = {
            "schema_version": HTTP_RESPONSE_SCHEMA,
            "status": response.status,
            "headers": [[name, value] for name, value in response.headers],
            "body": response_body,
        }
        self._check_outgoing_frame_size(self._success(pending.request_id, result))
        return result

    def _prepare_http_request(
        self,
        params: dict[str, Any],
    ) -> tuple[str, str, list[tuple[str, str]], bytes]:
        _require_exact_keys(params, {"schema_version", "path", "method", "headers", "body"})
        if params.get("schema_version") != HTTP_REQUEST_SCHEMA:
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", f"schema_version must be {HTTP_REQUEST_SCHEMA}.")
        method = params.get("method")
        path = params.get("path")
        headers = params.get("headers")
        body = params.get("body")
        if method not in {"GET", "POST", "PUT", "PATCH"}:
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http method is not in the code-owned allow-list.")
        if not isinstance(path, str):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http path must be a string.")
        _validate_product_path(path, method)
        parsed_headers = _validate_headers(headers)
        decoded_body = self._decode_http_body(body)
        return method, path, parsed_headers, decoded_body

    def _decode_http_body(self, body: Any) -> bytes:
        if not isinstance(body, dict):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http body must be an object.")
        encoding = body.get("encoding")
        if encoding == "empty":
            _require_exact_keys(body, {"encoding"})
            return b""
        if encoding not in {"utf8", "base64"}:
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http body encoding is unsupported.")
        _require_exact_keys(body, {"encoding", "data"})
        data = body.get("data")
        if not isinstance(data, str):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http body data must be text.")
        if encoding == "utf8":
            payload = data.encode("utf-8")
        else:
            if len(data) > ((self._max_raw_body_bytes + 2) // 3) * 4:
                raise ProtocolFailure(INPUT_TOO_LARGE, "COMPAT_BODY_TOO_LARGE", "The compatibility request body exceeds the bounded adapter limit.")
            try:
                payload = base64.b64decode(data, validate=True)
            except (binascii.Error, ValueError):
                raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http body is not valid base64.") from None
        if len(payload) > self._max_raw_body_bytes:
            raise ProtocolFailure(INPUT_TOO_LARGE, "COMPAT_BODY_TOO_LARGE", "The compatibility request body exceeds the bounded adapter limit.")
        return payload

    async def _subscribe(
        self,
        connection: CompatibilityConnection,
        params: dict[str, Any],
    ) -> tuple[dict[str, Any], list[dict[str, Any]]]:
        _require_exact_keys(params, {"schema_version", "stream_id", "path"})
        if params.get("schema_version") != SSE_SUBSCRIPTION_SCHEMA:
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", f"schema_version must be {SSE_SUBSCRIPTION_SCHEMA}.")
        stream_id = params.get("stream_id")
        path = params.get("path")
        if not isinstance(stream_id, str) or _STABLE_ID.fullmatch(stream_id) is None:
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "stream_id is invalid.")
        if not isinstance(path, str):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "SSE path must be a string.")
        _validate_product_path(path, "GET")
        if urlsplit(path).path.startswith("/api/v1/app-server"):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "Recursive app-server subscription is forbidden.")
        async with connection.lock:
            if stream_id in connection.streams:
                raise ProtocolFailure(INVALID_PARAMS, "DUPLICATE_STREAM_ID", "The SSE stream_id is already active.")
            connection.streams.add(stream_id)
        response = await self._client.request("GET", path, headers=[("accept", "text/event-stream")])
        if response.status >= 400:
            raise ProtocolFailure(COMPAT_BACKEND_UNAVAILABLE, "ADAPTER_UNAVAILABLE", f"SSE compatibility endpoint returned HTTP {response.status}.", recoverable=True)
        notifications = _sse_notifications(stream_id, response.body)
        return (
            {"schema_version": "ForgeCADSseSubscriptionResult@1", "stream_id": stream_id, "subscribed": True},
            notifications,
        )

    async def _unsubscribe(self, connection: CompatibilityConnection, params: dict[str, Any]) -> dict[str, Any]:
        _require_exact_keys(params, {"schema_version", "stream_id"})
        if params.get("schema_version") != SSE_UNSUBSCRIBE_SCHEMA:
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", f"schema_version must be {SSE_UNSUBSCRIBE_SCHEMA}.")
        stream_id = params.get("stream_id")
        if not isinstance(stream_id, str) or _STABLE_ID.fullmatch(stream_id) is None:
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "stream_id is invalid.")
        async with connection.lock:
            was_active = stream_id in connection.streams
            connection.streams.discard(stream_id)
        return {"schema_version": "ForgeCADSseUnsubscribeResult@1", "stream_id": stream_id, "unsubscribed": was_active}

    async def _replay_events(
        self,
        connection: CompatibilityConnection,
        params: dict[str, Any],
    ) -> tuple[dict[str, Any], list[dict[str, Any]]]:
        _require_exact_keys(params, {"cursor"})
        cursor = _decode_cursor(params.get("cursor"))
        path = f"/api/v1/agent/threads/{cursor['thread_id']}/events?after={cursor['source_sequence']}"
        stream_id = f"replay_{hashlib.sha256(str(params['cursor']).encode('utf-8')).hexdigest()[:24]}"
        response = await self._client.request("GET", path, headers=[("accept", "text/event-stream")])
        if response.status >= 400:
            raise ProtocolFailure(COMPAT_BACKEND_UNAVAILABLE, "ADAPTER_UNAVAILABLE", f"Replay endpoint returned HTTP {response.status}.", recoverable=True)
        notifications = _sse_notifications(stream_id, response.body)
        return {"notifications": notifications}, notifications

    async def _acknowledge(self, connection: CompatibilityConnection, params: dict[str, Any]) -> dict[str, Any]:
        if set(params) - {"notification_id", "cursor"} or not params:
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "notification/ack requires notification_id or cursor.")
        notification_id = params.get("notification_id")
        cursor = params.get("cursor")
        if notification_id is not None and (not isinstance(notification_id, str) or _STABLE_ID.fullmatch(notification_id) is None):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "notification_id is invalid.")
        if cursor is not None and not isinstance(cursor, str):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "cursor is invalid.")
        async with connection.lock:
            acknowledged_transport_cursor = 0
            for record in connection.notifications:
                frame = record.frame
                if notification_id is not None and frame.get("notification_id") == notification_id:
                    acknowledged_transport_cursor = max(acknowledged_transport_cursor, record.transport_cursor)
                if cursor is not None and frame.get("cursor") == cursor:
                    acknowledged_transport_cursor = max(acknowledged_transport_cursor, record.transport_cursor)
            if acknowledged_transport_cursor:
                connection.notifications = [
                    record for record in connection.notifications if record.transport_cursor > acknowledged_transport_cursor
                ]
        return {"acknowledged": bool(acknowledged_transport_cursor)}

    async def _cancel_request(self, connection: CompatibilityConnection, params: dict[str, Any]) -> dict[str, Any]:
        _require_exact_keys(params, {"request_id", "cancel_token"})
        request_id = params.get("request_id")
        cancel_token = params.get("cancel_token")
        if (
            not isinstance(request_id, str)
            or not isinstance(cancel_token, str)
            or not request_id
            or cancel_token != request_id
        ):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "request/cancel params are invalid.")
        async with connection.lock:
            pending = connection.pending.get(request_id)
            if pending is None:
                raise ProtocolFailure(UNKNOWN_REQUEST_ID, "UNKNOWN_REQUEST_ID", "The cancellation target is not in flight.")
            pending.cancellation_requested = True
            path = pending.path
        match = _TURN_START_PATH.fullmatch(path or "")
        if match is not None:
            await self._cancel_running_turn(match.group(1), request_id)
        elif pending.task is not None:
            pending.task.cancel()
        return {"request_id": request_id, "cancel_requested": True}

    async def _cancel_running_turn(self, thread_id: str, request_id: str) -> None:
        for _ in range(100):
            response = await self._client.request("GET", f"/api/v1/agent/threads/{thread_id}")
            if response.status == 200:
                try:
                    thread = json.loads(response.body)
                except (TypeError, ValueError):
                    thread = {}
                turns = thread.get("turns", []) if isinstance(thread, dict) else []
                running = next(
                    (
                        turn
                        for turn in reversed(turns)
                        if isinstance(turn, dict) and turn.get("status") in {"queued", "running"}
                    ),
                    None,
                )
                if running is not None and isinstance(running.get("turn_id"), str):
                    key = f"k001-cancel-{hashlib.sha256(request_id.encode('ascii')).hexdigest()[:24]}"
                    await self._client.request(
                        "POST",
                        f"/api/v1/agent/turns/{running['turn_id']}/cancel",
                        headers=[("idempotency-key", key)],
                    )
                    return
            await asyncio.sleep(0.01)

    async def _enqueue_notifications(
        self,
        connection: CompatibilityConnection,
        frames: list[dict[str, Any]],
    ) -> bool:
        for frame in frames:
            self._check_outgoing_frame_size(frame)
        async with connection.lock:
            if len(connection.notifications) + len(frames) > connection.queue_capacity:
                connection.notifications.clear()
                resync = {
                    "jsonrpc": JSONRPC_VERSION,
                    "method": "stream/resyncRequired",
                    "params": {
                        "schema_version": "ForgeCADResyncRequired@1",
                        "reason": "slow_consumer",
                    },
                    "notification_id": f"notification_{uuid.uuid4().hex}",
                }
                record = QueuedNotification(connection.next_transport_cursor, resync)
                connection.next_transport_cursor += 1
                connection.notifications.append(record)
                return False
            for frame in frames:
                record = QueuedNotification(connection.next_transport_cursor, frame)
                connection.next_transport_cursor += 1
                connection.notifications.append(record)
            return True

    def _success(self, request_id: Optional[str], result: Any) -> dict[str, Any]:
        if request_id is None:
            raise ProtocolFailure(INVALID_REQUEST, "INVALID_REQUEST", "A request ID is required for a response.")
        return {"jsonrpc": JSONRPC_VERSION, "id": request_id, "result": result}

    def _failure(self, request_id: Optional[str], failure: ProtocolFailure) -> dict[str, Any]:
        return {"jsonrpc": JSONRPC_VERSION, "id": request_id, "error": failure.envelope(request_id)}

    def _check_outgoing_frame_size(self, frame: dict[str, Any]) -> None:
        if len(_canonical_json(frame).encode("utf-8")) > self._max_frame_bytes:
            raise ProtocolFailure(INPUT_TOO_LARGE, "INPUT_TOO_LARGE", "The JSON-RPC response exceeds the negotiated byte limit.")


def canonical_json_sha256(value: Any) -> str:
    return hashlib.sha256(_canonical_json(value).encode("utf-8")).hexdigest()


def canonical_turn_item_hash(items: Iterable[Mapping[str, Any]]) -> str:
    manifest = [
        {"sequence": int(item["sequence"]), "item_sha256": canonical_json_sha256(item)}
        for item in items
    ]
    return canonical_json_sha256(manifest)


def _canonical_json(value: Any) -> str:
    return json.dumps(
        value,
        ensure_ascii=False,
        allow_nan=False,
        sort_keys=True,
        separators=(",", ":"),
    )


def _bounded_text(value: Any, minimum: int, maximum: int) -> bool:
    return isinstance(value, str) and minimum <= len(value) <= maximum


def _require_exact_keys(value: Mapping[str, Any], required: set[str]) -> None:
    if set(value) != required:
        raise ProtocolFailure(
            INVALID_PARAMS,
            "INVALID_PARAMS",
            "Protocol object fields do not match the versioned contract.",
            details={"required": sorted(required), "received": sorted(value)},
        )


def _validate_product_path(path: str, method: str) -> None:
    if (
        not path
        or len(path) > 4096
        or not path.startswith("/")
        or path.startswith("//")
        or "\\" in path
        or "#" in path
        or "://" in path
        or any(ord(character) < 32 or ord(character) == 127 for character in path)
    ):
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http path is not a safe relative path.")
    lower = path.lower()
    if any(part in lower for part in _FORBIDDEN_ENCODED_PATH_PARTS):
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http path contains an encoded path-control sequence.")
    parsed = urlsplit(path)
    if parsed.scheme or parsed.netloc:
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http path must not contain an origin.")
    if any(segment in {".", ".."} for segment in parsed.path.split("/")):
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http path traversal is forbidden.")
    if parsed.path == "/api/v1/app-server" or parsed.path.startswith("/api/v1/app-server/"):
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "Recursive app-server compatibility routing is forbidden.")
    if parsed.path == "/api/health":
        if method != "GET":
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "The health endpoint is read-only.")
        return
    if any(
        parsed.path == prefix or parsed.path.startswith(f"{prefix}/")
        for prefix in _LEGACY_PRODUCT_API_PREFIXES
    ):
        return
    if parsed.path != "/api/v1" and not parsed.path.startswith("/api/v1/"):
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http path is outside the code-owned product API prefixes.")


def _validate_headers(value: Any) -> list[tuple[str, str]]:
    if not isinstance(value, list) or len(value) > 64:
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http headers must be a bounded pair array.")
    result: list[tuple[str, str]] = []
    seen: set[str] = set()
    for pair in value:
        if not isinstance(pair, list) or len(pair) != 2 or not all(isinstance(part, str) for part in pair):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http headers must contain string pairs.")
        name, header_value = pair
        lower = name.lower()
        allowed = lower in _ALLOWED_HEADERS or lower.startswith("x-forgecad-")
        sensitive = any(
            marker in lower
            for marker in ("authorization", "cookie", "api-key", "apikey", "provider-key", "secret", "token")
        )
        if (
            not allowed
            or sensitive
            or lower in seen
            or not name
            or len(name) > 128
            or len(header_value) > 8192
            or any(not (character.isascii() and (character.isalnum() or character == "-")) for character in name)
            or any(character in "\r\n\x00" for character in header_value)
        ):
            raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "compat/http contains a forbidden or malformed header.")
        seen.add(lower)
        result.append((lower, header_value))
    return result


def _parse_sse(payload: bytes) -> list[tuple[Optional[str], str, str]]:
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError:
        raise ProtocolFailure(MALFORMED_UPSTREAM_EVENT, "MALFORMED_UPSTREAM_EVENT", "The upstream SSE stream is not UTF-8.") from None
    events: list[tuple[Optional[str], str, str]] = []
    event_id: Optional[str] = None
    event_name = "message"
    data_lines: list[str] = []

    def flush() -> None:
        nonlocal event_id, event_name, data_lines
        if data_lines or event_id is not None or event_name != "message":
            events.append((event_id, event_name, "\n".join(data_lines)))
        event_id = None
        event_name = "message"
        data_lines = []

    for line in text.splitlines():
        if line == "":
            flush()
            continue
        if line.startswith(":"):
            continue
        field, separator, value = line.partition(":")
        if separator and value.startswith(" "):
            value = value[1:]
        if field == "id":
            if "\x00" in value:
                raise ProtocolFailure(MALFORMED_UPSTREAM_EVENT, "MALFORMED_UPSTREAM_EVENT", "The upstream SSE id is malformed.")
            event_id = value
        elif field == "event":
            event_name = value or "message"
        elif field == "data":
            data_lines.append(value)
    flush()
    return events


def _sse_notifications(stream_id: str, payload: bytes) -> list[dict[str, Any]]:
    notifications: list[dict[str, Any]] = []
    for event_id, event_name, data in _parse_sse(payload):
        cursor: Optional[str] = None
        if event_name == "agent.item":
            try:
                parsed = json.loads(data)
                event = AgentEvent.model_validate(parsed)
            except Exception:
                raise ProtocolFailure(
                    MALFORMED_UPSTREAM_EVENT,
                    "MALFORMED_UPSTREAM_EVENT",
                    "The upstream Agent Item event does not match AgentEvent.",
                ) from None
            if str(event.sequence) != event_id or event.sequence != event.item.sequence:
                raise ProtocolFailure(
                    MALFORMED_UPSTREAM_EVENT,
                    "MALFORMED_UPSTREAM_EVENT",
                    "The upstream Agent Item sequence is inconsistent.",
                )
            if event.thread_id != event.item.thread_id or event.turn_id != event.item.turn_id:
                raise ProtocolFailure(
                    MALFORMED_UPSTREAM_EVENT,
                    "MALFORMED_UPSTREAM_EVENT",
                    "The upstream Agent Item identity is inconsistent.",
                )
            if "reasoning_content" in _canonical_json(parsed):
                raise ProtocolFailure(
                    MALFORMED_UPSTREAM_EVENT,
                    "MALFORMED_UPSTREAM_EVENT",
                    "Hidden reasoning content must not cross the app-server protocol.",
                )
            cursor = _encode_cursor(
                {
                    "schema_version": "ForgeCADAppServerCursor@1",
                    "thread_id": event.thread_id,
                    "turn_id": event.turn_id,
                    "source_sequence": event.sequence,
                    "phase": "item",
                    "item_id": event.item.item_id,
                }
            )
        notification_id = "notification_" + hashlib.sha256(
            _canonical_json(
                {"stream_id": stream_id, "event": event_name, "id": event_id, "data": data}
            ).encode("utf-8")
        ).hexdigest()[:24]
        frame: dict[str, Any] = {
            "jsonrpc": JSONRPC_VERSION,
            "method": "compat/sse",
            "params": {
                "schema_version": SSE_NOTIFICATION_SCHEMA,
                "stream_id": stream_id,
                "event": event_name,
                "data": data,
                **({"id": event_id} if event_id is not None else {}),
            },
            "notification_id": notification_id,
        }
        if cursor is not None:
            frame["cursor"] = cursor
        notifications.append(frame)
    return notifications


def _encode_cursor(value: Mapping[str, Any]) -> str:
    return "fc1_" + _canonical_json(value).encode("utf-8").hex()


def _decode_cursor(value: Any) -> dict[str, Any]:
    if not isinstance(value, str) or not value.startswith("fc1_") or len(value) > 4096:
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "cursor is not a ForgeCAD v1 opaque cursor.")
    try:
        decoded = json.loads(bytes.fromhex(value[4:]).decode("utf-8"))
    except (ValueError, UnicodeDecodeError, json.JSONDecodeError):
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "cursor encoding is invalid.") from None
    required = {"schema_version", "thread_id", "turn_id", "source_sequence", "phase", "item_id"}
    if not isinstance(decoded, dict) or set(decoded) != required:
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "cursor payload is invalid.")
    if decoded.get("schema_version") != "ForgeCADAppServerCursor@1":
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "cursor schema version is invalid.")
    if _STABLE_ID.fullmatch(str(decoded.get("thread_id", ""))) is None:
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "cursor thread_id is invalid.")
    if isinstance(decoded.get("source_sequence"), bool) or not isinstance(decoded.get("source_sequence"), int) or decoded["source_sequence"] < 0:
        raise ProtocolFailure(INVALID_PARAMS, "INVALID_PARAMS", "cursor source_sequence is invalid.")
    return decoded
