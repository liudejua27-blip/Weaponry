#!/usr/bin/env python3
"""Process-level smoke for the K003 restricted Python geometry facet.

The smoke starts the default ``wushen_agent.main:create_app`` factory with a
minimal environment and a one-process capability.  It deliberately does not
enable the legacy product oracle, persistence, an object store, or a Provider.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


ROOT = Path(__file__).resolve().parents[1]
PYTHON = ROOT / ".venv" / "bin" / "python"
CAPABILITY_ENV = "FORGECAD_RESTRICTED_GEOMETRY_CAPABILITY_TOKEN"
CAPABILITY_HEADER = "X-ForgeCAD-Restricted-Geometry-Capability"
CAPABILITY = "c" * 64
WRONG_CAPABILITY = "d" * 64
INTERNAL_PREFIX = "/api/v1/internal/geometry"

FORBIDDEN_ENVIRONMENT_NAMES = {
    "WUSHEN_LIBRARY_ROOT",
    "WUSHEN_MIGRATIONS_DIR",
    "DATABASE_URL",
    "FORGECAD_DATABASE_PATH",
    "FORGECAD_SQLITE_PATH",
    "FORGECAD_OBJECT_STORE_ROOT",
    "FORGECAD_LIBRARY_ROOT",
    "FORGECAD_AGENT_PROVIDER",
    "FORGECAD_AGENT_BASE_URL",
    "FORGECAD_AGENT_MODEL",
    "FORGECAD_AGENT_API_KEY",
    "FORGECAD_AGENT_API_KEY_FILE",
    "FORGECAD_CONCEPT_PLANNER_PROVIDER",
    "FORGECAD_CONCEPT_PLANNER_BASE_URL",
    "FORGECAD_CONCEPT_PLANNER_MODEL",
    "FORGECAD_CONCEPT_PLANNER_API_KEY",
    "FORGECAD_CONCEPT_PLANNER_API_KEY_FILE",
    "DEEPSEEK_API_KEY",
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "ANTHROPIC_API_KEY",
    "DASHSCOPE_API_KEY",
    "WUSHEN_LLM_PROVIDER",
    "WUSHEN_LLM_BASE_URL",
    "WUSHEN_LLM_MODEL",
    "WUSHEN_LLM_API_KEY",
    "WUSHEN_LLM_API_KEY_FILE",
}

SHAPE_PROGRAM = {
    "schema_version": "ShapeProgram@1",
    "program_id": "shape_k003_process_smoke",
    "units": "millimeter",
    "seed": 317,
    "triangle_budget": 1_000,
    "parameters": [],
    "operations": [
        {
            "operation_id": "op_body",
            "op": "box",
            "inputs": [],
            "args": {
                "size": [100.0, 40.0, 20.0],
                "part_role": "body_shell",
                "zone_id": "zone_body_shell",
                "material_id": "mat_graphite",
            },
        }
    ],
    "outputs": [
        {
            "output_id": "output_body",
            "operation_id": "op_body",
            "kind": "mesh",
            "part_role": "body_shell",
        }
    ],
    "non_functional_only": True,
}


class SmokeFailure(RuntimeError):
    pass


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise SmokeFailure(message)


def _free_loopback_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def _request(
    base_url: str,
    method: str,
    path: str,
    *,
    capability: str | None = CAPABILITY,
    payload: Any | None = None,
    timeout: float = 35.0,
) -> tuple[int, bytes]:
    body = None
    headers: dict[str, str] = {}
    if capability is not None:
        headers[CAPABILITY_HEADER] = capability
    if payload is not None:
        body = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode(
            "utf-8"
        )
        headers["Content-Type"] = "application/json"
    request = Request(
        f"{base_url}{path}",
        data=body,
        headers=headers,
        method=method,
    )
    try:
        with urlopen(request, timeout=timeout) as response:
            return int(response.status), response.read()
    except HTTPError as error:
        return int(error.code), error.read()


def _json_response(response: tuple[int, bytes]) -> tuple[int, dict[str, Any]]:
    status, body = response
    try:
        payload = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise SmokeFailure(f"HTTP {status} returned invalid JSON") from error
    _require(isinstance(payload, dict), f"HTTP {status} payload must be an object")
    return status, payload


def _wait_for_health(base_url: str, process: subprocess.Popen[bytes]) -> dict[str, Any]:
    deadline = time.monotonic() + 20.0
    last_error = "not started"
    while time.monotonic() < deadline:
        if process.poll() is not None:
            output = process.communicate(timeout=1)[0].decode("utf-8", errors="replace")
            raise SmokeFailure(
                f"restricted geometry process exited with {process.returncode}: {output[-4000:]}"
            )
        try:
            status, payload = _json_response(
                _request(base_url, "GET", "/api/health", capability=None, timeout=1.0)
            )
            if status == 200:
                return payload
            last_error = f"HTTP {status}"
        except (OSError, URLError, SmokeFailure) as error:
            last_error = str(error)
        time.sleep(0.05)
    raise SmokeFailure(f"restricted geometry health did not become ready: {last_error}")


def _assert_no_persistence(root: Path) -> None:
    forbidden_files = {
        "library.db",
        "library.db-shm",
        "library.db-wal",
        "forgecad.db",
        "forgecad.sqlite",
    }
    forbidden_directories = {
        "objects",
        "object_store",
        "content_addressed_objects",
    }
    violations = [
        path
        for path in root.rglob("*")
        if path.name in forbidden_files
        or (path.is_dir() and path.name in forbidden_directories)
    ]
    _require(not violations, f"restricted process created persistent product data: {violations}")


def _compile_payload(
    *,
    execution_id: str,
    idempotency_key: str,
    cancellation_id: str,
    cancellation_token: str,
) -> dict[str, Any]:
    return {
        "schema_version": "RestrictedGeometryExecutionRequest@1",
        "protocol_version": "forgecad.restricted-geometry/1",
        "execution_id": execution_id,
        "idempotency_key": idempotency_key,
        "cancellation_id": cancellation_id,
        "cancellation_token": cancellation_token,
        "action": "compile_readback",
        "timeout_ms": 30_000,
        "artifact_profile_id": "interactive_preview",
        "shape_program": SHAPE_PROGRAM,
    }


def _run_protocol(base_url: str) -> dict[str, Any]:
    ownership_path = f"{INTERNAL_PREFIX}/capability/ownership"
    wrong_status, wrong = _json_response(
        _request(
            base_url,
            "GET",
            ownership_path,
            capability=WRONG_CAPABILITY,
        )
    )
    _require(wrong_status == 403, "wrong capability must return 403")
    _require(
        wrong.get("error", {}).get("code") == "GEOMETRY_EXECUTOR_CAPABILITY_REJECTED",
        "wrong capability returned an unstable error",
    )

    ownership_status, ownership = _json_response(
        _request(base_url, "GET", ownership_path)
    )
    _require(ownership_status == 200, "ownership endpoint failed")
    for key in (
        "database_access",
        "object_store_access",
        "provider_access",
        "thread_session_access",
        "snapshot_write",
        "accepts_caller_glb",
        "persistent_artifacts",
    ):
        _require(ownership.get(key) is False, f"ownership {key} must be false")
    _require(
        ownership.get("capability_owner") == "rust_forgecad_core",
        "capability owner drifted",
    )
    _require(
        ownership.get("actions") == ["compile_readback", "render"],
        "restricted action list drifted",
    )

    for legacy_method, legacy_path in (
        ("POST", "/api/v1/agent/threads"),
        ("GET", "/api/v1/agent/threads"),
        ("GET", "/api/v1/agent/threads/thread_missing"),
        ("GET", "/api/v1/agent/threads/thread_missing/events"),
        ("POST", "/api/v1/internal/k002/lifecycle/execute"),
        ("POST", "/api/v1/internal/k002/product-tools/execute"),
    ):
        legacy_status, legacy = _json_response(
            _request(
                base_url,
                legacy_method,
                legacy_path,
                payload={} if legacy_method == "POST" else None,
            )
        )
        _require(
            legacy_status == 410,
            f"legacy product/lifecycle route must be tombstoned: {legacy_path}",
        )
        _require(
            legacy.get("error", {}).get("code") == "PRODUCT_STATE_RUST_OWNED",
            f"legacy route returned the wrong ownership error: {legacy_path}",
        )

    execute_path = f"{INTERNAL_PREFIX}/execute"
    compile_request = _compile_payload(
        execution_id="exec_k003_smoke_compile",
        idempotency_key="idem_k003_smoke_compile",
        cancellation_id="cancel_k003_smoke_compile",
        cancellation_token="cancel_token_k003_smoke_compile",
    )
    compile_status, compiled = _json_response(
        _request(base_url, "POST", execute_path, payload=compile_request)
    )
    _require(compile_status == 200, f"compile/readback failed: {compiled}")
    glb = base64.b64decode(compiled.get("glb_base64", ""), validate=True)
    _require(glb[:4] == b"glTF", "compiled bytes are not GLB")
    glb_sha256 = hashlib.sha256(glb).hexdigest()
    _require(compiled.get("glb_sha256") == glb_sha256, "GLB hash mismatch")
    _require(compiled.get("glb_byte_size") == len(glb), "GLB byte size mismatch")
    readback = compiled.get("readback")
    _require(isinstance(readback, dict), "compile response omitted readback")
    _require(readback.get("glb_sha256") == glb_sha256, "readback GLB hash mismatch")
    _require(
        readback.get("shape_program_sha256") == compiled.get("shape_program_sha256"),
        "ShapeProgram identity mismatch",
    )
    _require(readback.get("triangle_count", 0) > 0, "readback has no triangles")
    _require(readback.get("mesh_count", 0) > 0, "readback has no meshes")
    _require(readback.get("primitive_count", 0) > 0, "readback has no primitives")
    _require(readback.get("material_count", 0) > 0, "readback has no materials")
    _require(readback.get("closed_manifold") is True, "readback is not manifold")
    _require(
        readback.get("surface_provenance_present") is True,
        "readback omitted surface provenance",
    )
    _require(
        str(compiled.get("artifact_handle", "")).startswith("geomart_"),
        "compile response omitted opaque artifact handle",
    )

    replay_status, replay_body = _request(
        base_url, "POST", execute_path, payload=compile_request
    )
    _require(replay_status == 200, "compile idempotency replay failed")
    _require(
        json.loads(replay_body.decode("utf-8")) == compiled,
        "compile idempotency replay changed semantic output",
    )

    render_request = {
        "schema_version": "RestrictedGeometryExecutionRequest@1",
        "protocol_version": "forgecad.restricted-geometry/1",
        "execution_id": "exec_k003_smoke_render",
        "idempotency_key": "idem_k003_smoke_render",
        "cancellation_id": "cancel_k003_smoke_render",
        "cancellation_token": "cancel_token_k003_smoke_render",
        "action": "render",
        "timeout_ms": 30_000,
        "artifact_handle": compiled["artifact_handle"],
        "shape_program_sha256": compiled["shape_program_sha256"],
        "render": {"width": 64, "height": 64, "exploded_parts": []},
    }
    render_status, rendered = _json_response(
        _request(base_url, "POST", execute_path, payload=render_request)
    )
    _require(render_status == 200, f"render failed: {rendered}")
    _require(rendered.get("glb_base64") is None, "render repeated caller-visible GLB")
    _require(rendered.get("glb_sha256") == glb_sha256, "render GLB identity drifted")
    _require(
        rendered.get("renderer_id") == "forgecad-agent-software-raster@1",
        "renderer identity drifted",
    )
    render_views = rendered.get("render_views")
    render_hashes = rendered.get("render_view_sha256")
    _require(isinstance(render_views, dict), "render omitted view bytes")
    _require(isinstance(render_hashes, dict), "render omitted view hashes")
    _require(
        set(render_views) == {"iso", "front", "side", "top"},
        "render did not return exactly four views",
    )
    _require(set(render_views) == set(render_hashes), "render view/hash IDs drifted")
    for view_id, encoded in render_views.items():
        png = base64.b64decode(encoded, validate=True)
        _require(png.startswith(b"\x89PNG\r\n\x1a\n"), f"{view_id} is not PNG")
        _require(
            hashlib.sha256(png).hexdigest() == render_hashes[view_id],
            f"{view_id} hash mismatch",
        )

    cancel_id = "cancel_k003_smoke_before_start"
    cancel_token = "cancel_token_k003_smoke_before_start"
    cancel_status, cancelled = _json_response(
        _request(
            base_url,
            "POST",
            f"{INTERNAL_PREFIX}/cancel",
            payload={
                "schema_version": "RestrictedGeometryCancellationRequest@1",
                "protocol_version": "forgecad.restricted-geometry/1",
                "cancellation_id": cancel_id,
                "cancellation_token": cancel_token,
            },
        )
    )
    _require(cancel_status == 200, "cancel-before-start was not accepted")
    _require(cancelled.get("accepted") is True, "cancellation acceptance drifted")
    cancelled_request = _compile_payload(
        execution_id="exec_k003_smoke_cancelled",
        idempotency_key="idem_k003_smoke_cancelled",
        cancellation_id=cancel_id,
        cancellation_token=cancel_token,
    )
    cancelled_status, cancelled_error = _json_response(
        _request(base_url, "POST", execute_path, payload=cancelled_request)
    )
    _require(cancelled_status == 409, "cancelled execution must fail with conflict")
    _require(
        cancelled_error.get("error", {}).get("code")
        == "GEOMETRY_EXECUTION_CANCELLED",
        "cancelled execution returned an unstable error",
    )
    second_status, second_error = _json_response(
        _request(base_url, "POST", execute_path, payload=cancelled_request)
    )
    _require(second_status == cancelled_status, "cancelled replay status drifted")
    _require(
        second_error.get("error", {}).get("code")
        == "GEOMETRY_EXECUTION_CANCELLED",
        "cancelled replay error drifted",
    )

    return {
        "glb_sha256": glb_sha256,
        "glb_byte_size": len(glb),
        "triangle_count": readback["triangle_count"],
        "view_sha256": render_hashes,
        "provider_calls": 0,
        "persistent_product_artifacts": 0,
        "rust_owned_tombstones": [
            "/api/v1/agent/threads",
            "/api/v1/agent/threads/thread_missing",
            "/api/v1/agent/threads/thread_missing/events",
            "/api/v1/internal/k002/lifecycle/execute",
            "/api/v1/internal/k002/product-tools/execute",
        ],
        "capability_rejection": "GEOMETRY_EXECUTOR_CAPABILITY_REJECTED",
        "cancel_error": "GEOMETRY_EXECUTION_CANCELLED",
    }


def main() -> int:
    if not PYTHON.is_file():
        raise SmokeFailure(f"workspace Python runtime is missing: {PYTHON}")
    port = _free_loopback_port()
    base_url = f"http://127.0.0.1:{port}"
    with tempfile.TemporaryDirectory(prefix="forgecad-k003-runtime-") as directory:
        temporary_root = Path(directory)
        environment = {
            "PATH": os.pathsep.join(
                [str(PYTHON.parent), "/usr/bin", "/bin"]
            ),
            "PYTHONPATH": str(ROOT / "apps" / "agent"),
            "PYTHONUNBUFFERED": "1",
            "PYTHONDONTWRITEBYTECODE": "1",
            "HOME": str(temporary_root / "home"),
            "TMPDIR": str(temporary_root / "tmp"),
            CAPABILITY_ENV: CAPABILITY,
        }
        (temporary_root / "home").mkdir()
        (temporary_root / "tmp").mkdir()
        _require(
            not (set(environment) & FORBIDDEN_ENVIRONMENT_NAMES),
            "smoke environment included persistence or Provider authority",
        )
        process = subprocess.Popen(
            [
                str(PYTHON),
                "-m",
                "uvicorn",
                "wushen_agent.main:create_app",
                "--factory",
                "--host",
                "127.0.0.1",
                "--port",
                str(port),
                "--log-level",
                "warning",
                "--no-access-log",
            ],
            cwd=ROOT,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        try:
            health = _wait_for_health(base_url, process)
            _require(health.get("status") == "ok", "health status is not ok")
            _require(
                health.get("mode") == "restricted_geometry_executor",
                "default app did not start restricted geometry mode",
            )
            for key in (
                "database_access",
                "object_store_access",
                "provider_access",
                "snapshot_write",
                "persistent_state_writer",
            ):
                _require(health.get(key) is False, f"health {key} must be false")
            _assert_no_persistence(temporary_root)
            evidence = _run_protocol(base_url)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
            output = process.communicate(timeout=1)[0].decode(
                "utf-8", errors="replace"
            )
        _require(process.returncode in {0, -15}, f"sidecar exit failed: {output[-4000:]}")
        _assert_no_persistence(temporary_root)
        _require(
            not any(name in environment for name in FORBIDDEN_ENVIRONMENT_NAMES),
            "Provider or persistence environment appeared during smoke",
        )

    print(
        json.dumps(
            {
                "status": "pass",
                "schema_version": "K003RestrictedGeometryRuntimeSmoke@1",
                "default_factory": "wushen_agent.main:create_app",
                "health_authority": {
                    "database_access": False,
                    "object_store_access": False,
                    "provider_access": False,
                    "snapshot_write": False,
                    "persistent_state_writer": False,
                },
                **evidence,
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except SmokeFailure as error:
        print(f"K003 restricted geometry runtime smoke failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
