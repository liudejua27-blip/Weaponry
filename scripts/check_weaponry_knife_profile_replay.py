#!/usr/bin/env python3
"""Focused WPN-KNIFE-PROFILE-001 stdio replay/checker.

This checker is deliberately independent from ``forgecad-mcp`` source wiring:
it drives the executable as an MCP client and reads only the live manifest and
responses.  It does not read or write contracts, Store/CAS data, documentation,
or historical evidence.  A missing knife profile is reported as NOT_PROVEN,
not as a passing compatibility result.

Exit status:
  0 - every requested gate passed
  1 - an observed surface violates a requested gate
  2 - a required surface is not wired or cannot be probed
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from collections.abc import Iterable
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
DEFAULT_BINARY = ROOT / "apps/desktop/src-tauri/target/debug/forgecad-mcp"
DEFAULT_COMPAT_BINARY = (
    ROOT / "apps/desktop/src-tauri/target/debug/forgecad-mcp-compat"
)

PROTOCOL_VERSION = "2025-11-25"
PONYTAIL_PREFLIGHT_REQUIRED = "PONYTAIL_PREFLIGHT_REQUIRED"
PROFILE_ENV = "WEAPONRY_MCP_TOOL_PROFILE"

# These are the public names fixed by ADR-0030/WPN-KNIFE-PROFILE-001.  The
# profile module is intentionally not imported here: importing it would couple
# this replay to the implementation under test and make a missing wiring path
# look like a source-only pass.
DEFAULT_KNIFE_TOOLS = (
    "weapon_preflight",
    "reference_intake",
    "observe",
    "authoring_transaction",
    "surface_pipeline",
    "fps_presentation",
    "quality_review",
    "delivery",
    "approval",
    "recovery",
    "job",
)

# Current-source compatibility baseline.  These hashes come from the current
# executable's --tool-manifest-summary and are checked as immutable protocol
# values, not regenerated from a possibly changed response.
COMPAT_READ_COUNT = 131
COMPAT_WRITE_COUNT = 95
COMPAT_TOTAL_COUNT = 226
COMPAT_READ_MANIFEST_SHA256 = (
    "e7653110f1111a95e6020d71dc9d45f99c00628c4b08885f01a832aebf0c506d"
)
COMPAT_WRITE_MANIFEST_SHA256 = (
    "3d5a70ddc68b291ee321419cd40bbb9e3481bb5b43df67eb67d9c213d8985c18"
)

# No Runtime request is needed for this checker.  Supplying an unreachable,
# synthetic endpoint prevents an ambient Runtime from turning a replay into a
# stateful operation while still exercising MCP validation and gates.
PROBE_SOCKET = "/tmp/forgecad-weaponry-knife-profile-replay.sock"
PROBE_AUTH_VALUE = "weaponry-knife-profile-replay-token"


class ProbeError(RuntimeError):
    """A process or response could not be probed safely."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    default_command = str(DEFAULT_BINARY) if DEFAULT_BINARY.exists() else "forgecad-mcp"
    default_compat_command = (
        str(DEFAULT_COMPAT_BINARY)
        if DEFAULT_COMPAT_BINARY.exists()
        else "forgecad-mcp-compat"
    )
    parser.add_argument(
        "--command",
        default=os.environ.get("FORGECAD_MCP_COMMAND", default_command),
        help="forgecad-mcp executable; defaults to FORGECAD_MCP_COMMAND or the debug binary",
    )
    parser.add_argument(
        "--compat-command",
        default=os.environ.get("FORGECAD_MCP_COMPAT_COMMAND", default_compat_command),
        help=(
            "explicit forgecad-mcp-compat executable; defaults to "
            "FORGECAD_MCP_COMPAT_COMMAND or the debug compatibility binary"
        ),
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=15.0,
        help="maximum seconds for each isolated process probe",
    )
    return parser.parse_args()


def probe_environment(
    *, writes_enabled: bool, profile: str | None = None
) -> dict[str, str]:
    environment = os.environ.copy()
    environment["FORGECAD_RUNTIME_SOCKET"] = PROBE_SOCKET
    environment["FORGECAD_RUNTIME_TOKEN"] = PROBE_AUTH_VALUE
    if profile is None:
        environment.pop(PROFILE_ENV, None)
    else:
        environment[PROFILE_ENV] = profile
    if writes_enabled:
        environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"
    else:
        environment.pop("FORGECAD_MCP_ENABLE_MCP004_WRITES", None)
    return environment


def run_process(
    command: list[str],
    *,
    input_text: str | None,
    environment: dict[str, str],
    timeout: float,
) -> list[dict[str, Any]]:
    try:
        completed = subprocess.run(
            command,
            input=input_text,
            capture_output=True,
            text=True,
            encoding="utf-8",
            env=environment,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProbeError("MCP process could not be probed") from error
    if completed.returncode != 0:
        raise ProbeError("MCP process exited unsuccessfully")

    responses: list[dict[str, Any]] = []
    for line in completed.stdout.splitlines():
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as error:
            raise ProbeError("MCP process emitted a non-JSON line") from error
        if not isinstance(value, dict):
            raise ProbeError("MCP process emitted a non-object response")
        responses.append(value)
    return responses


def run_stdio_probe(
    executable: str, *, writes_enabled: bool, timeout: float
) -> dict[int, dict[str, Any]]:
    requests: list[dict[str, Any]] = [
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "weaponry-knife-profile-replay",
                    "version": "1",
                },
            },
        },
        {"jsonrpc": "2.0", "method": "notifications/initialized"},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
    ]
    if writes_enabled:
        # Empty arguments are intentional: this exercises the compatibility
        # write route's declared envelope without asking Runtime to mutate.
        requests.append(
            {
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "project_create", "arguments": {}},
            }
        )
    else:
        requests.extend(
            [
                {
                    "jsonrpc": "2.0",
                    "id": 3,
                    "method": "tools/call",
                    "params": {
                        "name": "weapon_preflight",
                        "arguments": {
                            "operation": "runtime_status",
                            "request": {},
                        },
                    },
                },
                {
                    "jsonrpc": "2.0",
                    "id": 4,
                    "method": "tools/call",
                    "params": {
                        "name": "weapon_preflight",
                        "arguments": {
                            "operation": "skill_get",
                            "request": {
                                "skill_id": "ponytail-preflight",
                                "version": "0.1.0",
                            },
                        },
                    },
                },
                {
                    "jsonrpc": "2.0",
                    "id": 5,
                    "method": "tools/call",
                    "params": {
                        "name": "weapon_preflight",
                        "arguments": {
                            "operation": "runtime_status",
                            "request": {},
                            "unexpected": True,
                        },
                    },
                },
                {
                    "jsonrpc": "2.0",
                    "id": 6,
                    "method": "tools/call",
                    "params": {"name": "project_create", "arguments": {}},
                },
                {
                    "jsonrpc": "2.0",
                    "id": 7,
                    "method": "tools/call",
                    "params": {
                        "name": "observe",
                        "arguments": {
                            "operation": "selection_get",
                            "request": {},
                        },
                    },
                },
                {
                    "jsonrpc": "2.0",
                    "id": 8,
                    "method": "tools/call",
                    "params": {
                        "name": "weapon_preflight",
                        "arguments": {
                            "operation": "authoring_mesh_transaction_get",
                            "request": {},
                        },
                    },
                },
            ]
        )

    payload = "".join(
        json.dumps(request, separators=(",", ":")) + "\n" for request in requests
    )
    responses = run_process(
        [executable, "serve", "--stdio"],
        input_text=payload,
        environment=probe_environment(
            writes_enabled=writes_enabled,
            profile="compatibility" if writes_enabled else None,
        ),
        timeout=timeout,
    )
    by_id: dict[int, dict[str, Any]] = {}
    for response in responses:
        response_id = response.get("id")
        if isinstance(response_id, bool) or not isinstance(response_id, int):
            continue
        if response_id in by_id:
            raise ProbeError("MCP emitted duplicate response ids")
        by_id[response_id] = response
    return by_id


def run_manifest_summary(executable: str, timeout: float) -> dict[str, Any]:
    responses = run_process(
        [executable, "--tool-manifest-summary"],
        input_text=None,
        environment=probe_environment(writes_enabled=False),
        timeout=timeout,
    )
    if len(responses) != 1:
        raise ProbeError("manifest summary did not return one JSON object")
    return responses[0]


def run_knife_manifest_summary(executable: str, timeout: float) -> dict[str, Any]:
    responses = run_process(
        [executable, "--knife-tool-manifest-summary"],
        input_text=None,
        environment=probe_environment(writes_enabled=False),
        timeout=timeout,
    )
    if len(responses) != 1:
        raise ProbeError("knife profile summary did not return one JSON object")
    return responses[0]


def add_gate(
    gates: list[dict[str, Any]],
    name: str,
    status: str,
    detail: str,
    **evidence: Any,
) -> None:
    gate: dict[str, Any] = {"gate": name, "status": status, "detail": detail}
    gate.update(evidence)
    gates.append(gate)


def response_code(response: dict[str, Any] | None) -> str | None:
    if not response:
        return None
    error = response.get("error")
    if isinstance(error, dict):
        data = error.get("data")
        if isinstance(data, dict) and isinstance(data.get("code"), str):
            return data["code"]
    result = response.get("result")
    if isinstance(result, dict):
        structured = result.get("structuredContent")
        if isinstance(structured, dict) and isinstance(structured.get("code"), str):
            return structured["code"]
    return None


def tool_list(response: dict[str, Any] | None) -> list[dict[str, Any]]:
    if not response:
        raise ProbeError("tools/list response is missing")
    result = response.get("result")
    if not isinstance(result, dict) or not isinstance(result.get("tools"), list):
        raise ProbeError("tools/list response has no tools array")
    tools = result["tools"]
    if not all(isinstance(tool, dict) for tool in tools):
        raise ProbeError("tools/list contains a non-object tool descriptor")
    return tools


def tool_names(tools: Iterable[dict[str, Any]]) -> list[str]:
    names: list[str] = []
    for tool in tools:
        name = tool.get("name")
        if not isinstance(name, str) or not name:
            raise ProbeError("tool descriptor has no non-empty name")
        if name in names:
            raise ProbeError(f"duplicate tool descriptor: {name}")
        names.append(name)
    return names


def profile_status(
    observed_names: list[str] | None,
    *,
    expected: set[str],
) -> tuple[str, str]:
    if observed_names is None:
        return "NOT_PROVEN", "default tools/list could not be probed"
    observed = set(observed_names)
    if observed == expected and len(observed_names) == len(expected):
        return "PASS", "the exact 11-name knife façade set is advertised"
    # The known pre-profile surface has no façade names at all.  Keep this a
    # truthful missing-wiring result so the checker is useful before main.rs is
    # switched, while still treating a partial/incorrect switch as a failure.
    if not observed.intersection(expected):
        return (
            "NOT_PROVEN",
            "knife façade profile is not wired; observed compatibility names instead",
        )
    return "FAIL", "knife façade set is partial or contains unexpected names"


def short_names(names: Iterable[str], limit: int = 12) -> list[str]:
    ordered = sorted(names)
    return ordered[:limit] + ([f"... (+{len(ordered) - limit})"] if len(ordered) > limit else [])


def is_object_schema(schema: dict[str, Any]) -> bool:
    schema_type = schema.get("type")
    return (
        schema_type == "object"
        or (isinstance(schema_type, list) and "object" in schema_type)
        or "properties" in schema
    )


def closed_wrapper_issues(tool: dict[str, Any]) -> list[str]:
    issues: list[str] = []
    name = tool.get("name", "<unnamed>")
    schema = tool.get("inputSchema")
    if not isinstance(schema, dict):
        issues.append(f"{name}: inputSchema is not an object schema")
    elif is_object_schema(schema):
        if schema.get("additionalProperties") is not False:
            issues.append(f"{name}: inputSchema.additionalProperties is not false")
    elif isinstance(schema.get("oneOf"), list) and schema["oneOf"]:
        # The façade schema is a closed tagged union.  Its root has no
        # properties of its own; closure is provided by every object branch.
        # The request branch is the underlying operation's existing schema and
        # is intentionally not reclassified here; this gate is about the
        # façade boundary, while the route probe below exercises its nesting.
        for index, alternative in enumerate(schema["oneOf"]):
            if not isinstance(alternative, dict) or not is_object_schema(alternative):
                issues.append(f"{name}: inputSchema.oneOf[{index}] is not an object schema")
            elif alternative.get("additionalProperties") is not False:
                issues.append(
                    f"{name}: inputSchema.oneOf[{index}].additionalProperties is not false"
                )
            else:
                properties = alternative.get("properties")
                if not isinstance(properties, dict) or set(properties) != {
                    "operation",
                    "request",
                }:
                    issues.append(
                        f"{name}: inputSchema.oneOf[{index}] is not the closed operation/request envelope"
                    )
                elif not isinstance(properties.get("operation"), dict) or not isinstance(
                    properties["operation"].get("const"), str
                ):
                    issues.append(
                        f"{name}: inputSchema.oneOf[{index}].operation is not a string const"
                    )
    else:
        issues.append(f"{name}: inputSchema is not a closed object/union schema")

    annotations = tool.get("annotations")
    required_annotations = (
        "readOnlyHint",
        "destructiveHint",
        "idempotentHint",
        "openWorldHint",
    )
    if not isinstance(annotations, dict):
        issues.append(f"{name}: annotations missing")
    else:
        for field in required_annotations:
            if not isinstance(annotations.get(field), bool):
                issues.append(f"{name}: annotations.{field} is not boolean")

    return issues


def check_summary(summary: dict[str, Any], gates: list[dict[str, Any]]) -> None:
    expected = {
        "read_count": COMPAT_READ_COUNT,
        "write_count": COMPAT_WRITE_COUNT,
        "total_count": COMPAT_TOTAL_COUNT,
        "read_manifest_sha256": COMPAT_READ_MANIFEST_SHA256,
        "write_enabled_manifest_sha256": COMPAT_WRITE_MANIFEST_SHA256,
    }
    mismatches = {
        key: {"expected": value, "observed": summary.get(key)}
        for key, value in expected.items()
        if summary.get(key) != value
    }
    if mismatches:
        add_gate(
            gates,
            "compatibility_manifest_226_and_hash",
            "FAIL",
            "current-source compatibility count or manifest hash drifted",
            mismatches=mismatches,
        )
        return
    add_gate(
        gates,
        "compatibility_manifest_226_and_hash",
        "PASS",
        "the source-derived compatibility summary matches the pinned 226/hash baseline",
        read_count=COMPAT_READ_COUNT,
        write_count=COMPAT_WRITE_COUNT,
        total_count=COMPAT_TOTAL_COUNT,
        read_manifest_sha256=COMPAT_READ_MANIFEST_SHA256,
        write_enabled_manifest_sha256=COMPAT_WRITE_MANIFEST_SHA256,
    )


def check_knife_summary(summary: dict[str, Any], gates: list[dict[str, Any]]) -> None:
    expected = {
        "default_tool_count": 11,
        "default_tool_names": list(DEFAULT_KNIFE_TOOLS),
        "compatibility_tool_count": COMPAT_TOTAL_COUNT,
        # The active 11-façade summary no longer materializes the raw registry,
        # even in a feature-enabled build.  Its nullable active-manifest field
        # must remain null; the immutable 226-tool replay hash is carried by
        # the explicit declared compatibility binding below.
        "compatibility_manifest_sha256": None,
        "compatibility_declared_write_enabled_manifest_sha256": COMPAT_WRITE_MANIFEST_SHA256,
        "compatibility_profile_available": False,
        "compatibility_requires_explicit_profile": True,
    }
    mismatches = {
        key: {"expected": value, "observed": summary.get(key)}
        for key, value in expected.items()
        if summary.get(key) != value
    }
    if mismatches:
        add_gate(
            gates,
            "knife_profile_manifest_summary",
            "FAIL",
            "knife profile summary does not bind the exact 11 façades to the 226-tool compatibility manifest",
            mismatches=mismatches,
        )
        return
    add_gate(
        gates,
        "knife_profile_manifest_summary",
        "PASS",
        "profile summary binds the exact façade order and explicit compatibility requirement",
        default_tool_count=11,
        compatibility_tool_count=COMPAT_TOTAL_COUNT,
        compatibility_declared_write_enabled_manifest_sha256=COMPAT_WRITE_MANIFEST_SHA256,
    )


def check_compatibility_list(
    tools: list[dict[str, Any]] | None,
    summary: dict[str, Any] | None,
    gates: list[dict[str, Any]],
) -> None:
    if tools is None:
        add_gate(
            gates,
            "explicit_compatibility_tools_list",
            "NOT_PROVEN",
            "explicit compatibility tools/list could not be probed",
        )
        return
    try:
        names = tool_names(tools)
    except ProbeError as error:
        add_gate(gates, "explicit_compatibility_tools_list", "FAIL", str(error))
        return
    read_count = sum(
        tool.get("annotations", {}).get("readOnlyHint") is True for tool in tools
    )
    write_count = sum(
        tool.get("annotations", {}).get("readOnlyHint") is False for tool in tools
    )
    invalid_annotations = len(tools) - read_count - write_count
    expected_names: set[str] | None = None
    if isinstance(summary, dict):
        read_names = summary.get("read_names")
        write_names = summary.get("write_names")
        if isinstance(read_names, list) and isinstance(write_names, list):
            expected_names = {name for name in [*read_names, *write_names] if isinstance(name, str)}
    if (
        len(names) != COMPAT_TOTAL_COUNT
        or read_count != COMPAT_READ_COUNT
        or write_count != COMPAT_WRITE_COUNT
        or invalid_annotations
    ):
        add_gate(
            gates,
            "explicit_compatibility_tools_list",
            "FAIL",
            "explicit write opt-in did not expose the exact read/write compatibility surface",
            observed_count=len(names),
            observed_read_count=read_count,
            observed_write_count=write_count,
            invalid_annotation_count=invalid_annotations,
        )
        return
    if expected_names is not None and set(names) != expected_names:
        add_gate(
            gates,
            "explicit_compatibility_tools_list",
            "FAIL",
            "tools/list names differ from the source-derived compatibility summary",
            missing=short_names(expected_names - set(names)),
            unexpected=short_names(set(names) - expected_names),
        )
        return
    add_gate(
        gates,
        "explicit_compatibility_tools_list",
        "PASS",
        "explicit authenticated write opt-in exposes 131 read + 95 write = 226 unique tools",
        observed_count=len(names),
        observed_read_count=read_count,
        observed_write_count=write_count,
    )


def check_default_profile(
    tools: list[dict[str, Any]] | None, gates: list[dict[str, Any]]
) -> list[str] | None:
    if tools is None:
        status, detail = profile_status(None, expected=set(DEFAULT_KNIFE_TOOLS))
        add_gate(gates, "default_tools_list_exact_11", status, detail, expected_count=11)
        add_gate(gates, "default_no_legacy_tool_leakage", status, detail)
        add_gate(gates, "knife_facade_wrappers_closed", status, detail)
        return None
    names = tool_names(tools)
    expected = set(DEFAULT_KNIFE_TOOLS)
    status, detail = profile_status(names, expected=expected)
    add_gate(
        gates,
        "default_tools_list_exact_11",
        status,
        detail,
        expected_count=11,
        observed_count=len(names),
        expected_names=list(DEFAULT_KNIFE_TOOLS),
        observed_names=names if len(names) <= 20 else short_names(names),
    )

    leaked = set(names) - expected
    missing = expected - set(names)
    if status == "PASS":
        add_gate(
            gates,
            "default_no_legacy_tool_leakage",
            "PASS",
            "default tools/list contains no name outside the 11 knife façades",
        )
    elif not set(names).intersection(expected):
        add_gate(
            gates,
            "default_no_legacy_tool_leakage",
            "NOT_PROVEN",
            "knife profile is not wired; current compatibility names remain visible",
            observed_count=len(names),
            leaked_sample=short_names(leaked),
        )
    else:
        add_gate(
            gates,
            "default_no_legacy_tool_leakage",
            "FAIL",
            "default tools/list contains partial knife profile plus legacy names",
            missing=short_names(missing),
            leaked_sample=short_names(leaked),
        )

    if status != "PASS":
        wrapper_status = "NOT_PROVEN" if status == "NOT_PROVEN" else "FAIL"
        add_gate(
            gates,
            "knife_facade_wrappers_closed",
            wrapper_status,
            "closed façade wrappers cannot be accepted before all 11 names are advertised",
        )
        return names

    by_name = {tool["name"]: tool for tool in tools}
    issues: list[str] = []
    for name in DEFAULT_KNIFE_TOOLS:
        issues.extend(closed_wrapper_issues(by_name[name]))
    if issues:
        add_gate(
            gates,
            "knife_facade_wrappers_closed",
            "FAIL",
            "one or more knife façade descriptors are open or missing required annotations",
            issues=issues[:20],
            issue_count=len(issues),
        )
    else:
        add_gate(
            gates,
            "knife_facade_wrappers_closed",
            "PASS",
            "all 11 knife façade descriptors have closed schemas and typed annotations",
        )
    return names


def check_default_routes(
    responses: dict[int, dict[str, Any]] | None,
    gates: list[dict[str, Any]],
) -> None:
    if responses is None:
        for name in (
            "representative_read_route",
            "preflight_route",
            "preflight_gate",
            "write_gate",
            "closed_cross_facade_rejection",
        ):
            add_gate(gates, name, "NOT_PROVEN", "default stdio route probe could not be run")
        return

    runtime_status = responses.get(3)
    if isinstance(runtime_status, dict) and isinstance(runtime_status.get("result"), dict):
        add_gate(
            gates,
            "representative_read_route",
            "PASS",
            "weapon_preflight/runtime_status façade route returned a typed MCP result",
        )
    elif response_code(runtime_status) == "METHOD_NOT_FOUND":
        add_gate(
            gates,
            "representative_read_route",
            "NOT_PROVEN",
            "legacy read route is hidden, so the new observe façade needs its own route probe",
        )
    else:
        add_gate(
            gates,
            "representative_read_route",
            "FAIL",
            "runtime_status did not return a typed MCP result",
            response_code=response_code(runtime_status),
        )

    preflight = responses.get(4)
    if isinstance(preflight, dict) and isinstance(preflight.get("result"), dict):
        code = response_code(preflight)
        if code not in {"INVALID_TOOL_PARAMS", "METHOD_NOT_FOUND"}:
            add_gate(
                gates,
                "preflight_route",
                "PASS",
                "valid ponytail-preflight request reached the typed route boundary",
                response_code=code,
            )
        elif code == "METHOD_NOT_FOUND":
            add_gate(
                gates,
                "preflight_route",
                "NOT_PROVEN",
                "weapon_preflight façade is not wired in the executable under test",
            )
        else:
            add_gate(
                gates,
                "preflight_route",
                "FAIL",
                "valid ponytail-preflight request was rejected as an unknown/invalid route",
                response_code=code,
            )
    else:
        add_gate(
            gates,
            "preflight_route",
            "FAIL",
            "valid ponytail-preflight request did not return a typed result",
            response_code=response_code(preflight),
        )

    preflight_gate = responses.get(7)
    preflight_code = response_code(preflight_gate)
    if preflight_code == PONYTAIL_PREFLIGHT_REQUIRED:
        add_gate(
            gates,
            "preflight_gate",
            "PASS",
            "design write route is rejected until ponytail-preflight is read in-session",
        )
    elif preflight_code == "METHOD_NOT_FOUND":
        add_gate(
            gates,
            "preflight_gate",
            "NOT_PROVEN",
            "legacy transaction route is hidden; new authoring_transaction preflight replay is still needed",
        )
    else:
        add_gate(
            gates,
            "preflight_gate",
            "FAIL",
            "design write route did not fail closed on missing preflight",
            response_code=preflight_code,
        )

    write_gate = responses.get(6)
    write_code = response_code(write_gate)
    if write_code in {
        "MCP004_WRITE_TOOLS_DISABLED",
        "WRITE_TOOLS_DISABLED",
        "AUTHORING_MESH_TRANSACTION_WRITE_TOOLS_DISABLED",
        "WEAPONRY_KNIFE_PROFILE_TOOL_HIDDEN",
    }:
        add_gate(
            gates,
            "write_gate",
            "PASS",
            "default raw write route is either disabled or hidden behind the knife façade boundary",
        )
    elif write_code == "METHOD_NOT_FOUND":
        add_gate(
            gates,
            "write_gate",
            "PASS",
            "default compatibility write route is hidden and therefore cannot execute",
        )
    else:
        add_gate(
            gates,
            "write_gate",
            "FAIL",
            "default write route was not rejected by the write boundary",
            response_code=write_code,
        )

    cross_facade = responses.get(8)
    cross_code = response_code(cross_facade)
    if cross_code in {
        "INVALID_TOOL_PARAMS",
        "METHOD_NOT_FOUND",
        "WEAPONRY_KNIFE_PROFILE_ROUTE_DENIED",
    }:
        add_gate(
            gates,
            "closed_cross_facade_rejection",
            "PASS",
            "a foreign path field cannot cross the runtime-status façade boundary",
            response_code=cross_code,
        )
    else:
        add_gate(
            gates,
            "closed_cross_facade_rejection",
            "FAIL",
            "foreign façade input was not rejected",
            response_code=cross_code,
        )

    malformed = responses.get(5)
    malformed_code = response_code(malformed)
    if malformed_code == "METHOD_NOT_FOUND":
        add_gate(
            gates,
            "closed_preflight_envelope",
            "NOT_PROVEN",
            "weapon_preflight façade is not wired, so its nested closure cannot be replayed",
        )
    elif malformed_code not in {
        "INVALID_TOOL_PARAMS",
        "WEAPONRY_KNIFE_PROFILE_INVALID",
    }:
        add_gate(
            gates,
            "closed_preflight_envelope",
            "FAIL",
            "unknown field in the preflight envelope was not rejected",
            response_code=malformed_code,
        )
    else:
        add_gate(
            gates,
            "closed_preflight_envelope",
            "PASS",
            "unknown preflight field was rejected before Runtime dispatch",
        )


def check_explicit_write_route(
    responses: dict[int, dict[str, Any]] | None,
    tools: list[dict[str, Any]] | None,
    gates: list[dict[str, Any]],
) -> None:
    if responses is None or tools is None:
        add_gate(
            gates,
            "representative_explicit_write_route",
            "NOT_PROVEN",
            "explicit compatibility route could not be probed",
        )
        return
    names = set(tool_names(tools))
    response = responses.get(3)
    if "project_create" not in names:
        add_gate(
            gates,
            "representative_explicit_write_route",
            "FAIL",
            "compatibility tools/list omitted the representative project_create write route",
        )
        return
    if response_code(response) != "INVALID_TOOL_PARAMS":
        add_gate(
            gates,
            "representative_explicit_write_route",
            "FAIL",
            "explicit write route did not stop at its declared closed schema",
            response_code=response_code(response),
        )
        return
    add_gate(
        gates,
        "representative_explicit_write_route",
        "PASS",
        "explicit compatibility write route is exposed but empty arguments stop at schema validation",
    )


def main() -> int:
    args = parse_args()
    gates: list[dict[str, Any]] = []
    summary: dict[str, Any] | None = None
    knife_summary: dict[str, Any] | None = None
    default_responses: dict[int, dict[str, Any]] | None = None
    compatibility_responses: dict[int, dict[str, Any]] | None = None
    default_tools: list[dict[str, Any]] | None = None
    compatibility_tools: list[dict[str, Any]] | None = None

    try:
        summary = run_manifest_summary(args.compat_command, args.timeout)
        check_summary(summary, gates)
    except ProbeError as error:
        add_gate(
            gates,
            "compatibility_manifest_226_and_hash",
            "NOT_PROVEN",
            str(error),
        )

    try:
        knife_summary = run_knife_manifest_summary(args.command, args.timeout)
        check_knife_summary(knife_summary, gates)
    except ProbeError as error:
        add_gate(gates, "knife_profile_manifest_summary", "NOT_PROVEN", str(error))

    try:
        default_responses = run_stdio_probe(
            args.command, writes_enabled=False, timeout=args.timeout
        )
        default_tools = tool_list(default_responses.get(2))
    except ProbeError:
        # check_default_profile emits the three profile-specific NOT_PROVEN
        # rows below, keeping one row per gate even when the process is absent.
        default_tools = None

    try:
        compatibility_responses = run_stdio_probe(
            args.compat_command, writes_enabled=True, timeout=args.timeout
        )
        compatibility_tools = tool_list(compatibility_responses.get(2))
    except ProbeError:
        # check_compatibility_list emits the single compatibility-list row
        # below, keeping the report deterministic on an unavailable binary.
        compatibility_tools = None

    try:
        check_default_profile(default_tools, gates)
    except ProbeError as error:
        add_gate(gates, "default_tools_list_exact_11", "FAIL", str(error))
        add_gate(gates, "default_no_legacy_tool_leakage", "FAIL", str(error))
        add_gate(gates, "knife_facade_wrappers_closed", "FAIL", str(error))

    try:
        check_compatibility_list(compatibility_tools, summary, gates)
    except ProbeError as error:
        add_gate(gates, "explicit_compatibility_tools_list", "FAIL", str(error))

    check_default_routes(default_responses, gates)
    check_explicit_write_route(compatibility_responses, compatibility_tools, gates)

    statuses = {gate["status"] for gate in gates}
    if "FAIL" in statuses:
        overall = "FAIL"
        exit_code = 1
    elif "NOT_PROVEN" in statuses:
        overall = "NOT_PROVEN"
        exit_code = 2
    else:
        overall = "PASS"
        exit_code = 0

    print(
        json.dumps(
            {
                "task_id": "WPN-KNIFE-PROFILE-001",
                "status": overall,
                "default_profile_names": list(DEFAULT_KNIFE_TOOLS),
                "gates": gates,
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
