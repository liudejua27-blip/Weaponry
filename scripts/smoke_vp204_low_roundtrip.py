#!/usr/bin/env python3
"""VP204 offline one-author/one-patch, cache, receipt and recovery Gate."""

from __future__ import annotations

import hashlib
import json
import math
import subprocess
import time
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

from forgecad_agent.application.restricted_geometry_executor import (
    RestrictedGeometryBoundaryError,
    RestrictedGeometryCancellationRequest,
    RestrictedGeometryExecutionRequest,
    RestrictedGeometryExecutor,
)
from forgecad_agent.application.visual_program_compile_cache import (
    VisualProgramCompileCache,
)


ROOT = Path(__file__).resolve().parents[1]
SCHEMA_ROOT = ROOT / "packages" / "concept-spec" / "schemas"
FIXTURE_ROOT = ROOT / "packages" / "concept-spec" / "fixtures"
RUST_MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
SCHEMAS = {
    path.name: json.loads(path.read_text(encoding="utf-8"))
    for path in sorted(SCHEMA_ROOT.glob("*.json"))
}
SCHEMA_REGISTRY = Registry().with_resources(
    (schema["$id"], Resource.from_contents(schema)) for schema in SCHEMAS.values()
)


def canonical_sha256(value: Any) -> str:
    payload = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    )
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def validate(schema_name: str, payload: dict[str, Any]) -> None:
    validator = Draft202012Validator(SCHEMAS[schema_name], registry=SCHEMA_REGISTRY)
    errors = sorted(validator.iter_errors(payload), key=lambda item: list(item.path))
    if errors:
        raise AssertionError(f"VP204_SCHEMA_INVALID:{schema_name}:{errors[0].message}")


def run_rust(binary: str, payload: dict[str, Any], *, expect_success: bool = True) -> dict[str, Any]:
    result = subprocess.run(
        [
            str(ROOT / "script" / "with_rust_toolchain.sh"),
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(RUST_MANIFEST),
            "-p",
            "forgecad-core",
            "--bin",
            binary,
            "--offline",
        ],
        cwd=ROOT,
        input=json.dumps(payload, ensure_ascii=False),
        capture_output=True,
        text=True,
        timeout=180,
    )
    if expect_success:
        if result.returncode != 0:
            raise AssertionError(f"VP204_RUST_FAILED:{binary}:{result.stderr[-3000:]}")
        return json.loads(result.stdout)
    if result.returncode == 0:
        raise AssertionError(f"VP204_RUST_UNEXPECTED_SUCCESS:{binary}")
    return {"stderr": result.stderr}


def vp203_lowerings() -> dict[str, dict[str, Any]]:
    result = subprocess.run(
        [
            str(ROOT / "script" / "with_rust_toolchain.sh"),
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(RUST_MANIFEST),
            "-p",
            "forgecad-core",
            "--bin",
            "vp203_high_level_geometry_dump",
            "--offline",
        ],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        timeout=180,
    )
    if result.returncode != 0:
        raise AssertionError(f"VP204_VP203_LOWERING_FAILED:{result.stderr[-3000:]}")
    return {item["fixture_id"]: item["lowering"] for item in json.loads(result.stdout)["results"]}


def fixture(name: str) -> dict[str, Any]:
    return json.loads(
        (FIXTURE_ROOT / f"forge-visual-geometry-v2-{name}.json").read_text(encoding="utf-8")
    )


def milliseconds(start: float) -> int:
    return max(0, math.ceil((time.perf_counter() - start) * 1000))


def compile_render(
    executor: RestrictedGeometryExecutor,
    cache: VisualProgramCompileCache,
    name: str,
    lowering: dict[str, Any],
    suffix: str,
) -> dict[str, Any]:
    started = time.perf_counter()
    compiled, miss = cache.compile(
        execution_id=f"exec_vp204_{name}_{suffix}_miss",
        idempotency_key=f"idem_vp204_{name}_{suffix}_miss",
        cancellation_id=f"cancel_vp204_{name}_{suffix}_miss",
        cancellation_token=f"token_vp204_{name}_{suffix}_miss",
        shape_program=lowering["shape_program"],
    )
    compile_ms = milliseconds(started)
    if miss.hit or miss.shape_program_sha256 != lowering["shape_program_sha256"]:
        raise AssertionError("VP204_FIRST_COMPILE_MUST_MISS")
    started = time.perf_counter()
    replay, hit = cache.compile(
        execution_id=f"exec_vp204_{name}_{suffix}_hit",
        idempotency_key=f"idem_vp204_{name}_{suffix}_hit",
        cancellation_id=f"cancel_vp204_{name}_{suffix}_hit",
        cancellation_token=f"token_vp204_{name}_{suffix}_hit",
        shape_program=lowering["shape_program"],
    )
    cache_hit_ms = milliseconds(started)
    if not hit.hit or hit.cache_key_sha256 != miss.cache_key_sha256:
        raise AssertionError("VP204_EXACT_REPLAY_CACHE_MISS")
    if replay.glb_sha256 != compiled.glb_sha256 or replay.artifact_handle != compiled.artifact_handle:
        raise AssertionError("VP204_CACHE_ARTIFACT_LINEAGE_CHANGED")

    render_request = RestrictedGeometryExecutionRequest.model_validate(
        {
            "schema_version": "RestrictedGeometryExecutionRequest@1",
            "protocol_version": "forgecad.restricted-geometry/1",
            "execution_id": f"exec_vp204_{name}_{suffix}_render",
            "idempotency_key": f"idem_vp204_{name}_{suffix}_render",
            "cancellation_id": f"cancel_vp204_{name}_{suffix}_render",
            "cancellation_token": f"token_vp204_{name}_{suffix}_render",
            "action": "render",
            "timeout_ms": 120_000,
            "artifact_handle": replay.artifact_handle,
            "shape_program_sha256": replay.shape_program_sha256,
            "render": {
                "width": 64,
                "height": 64,
                "view_profile": "workbench_four",
                "exploded_parts": [],
            },
        }
    )
    started = time.perf_counter()
    rendered = executor.execute(render_request)
    render_ms = milliseconds(started)
    if rendered.glb_sha256 != compiled.glb_sha256 or rendered.renderer_id != "forgecad-agent-software-raster@1":
        raise AssertionError("VP204_RENDER_LINEAGE_INVALID")
    if set(rendered.render_view_sha256 or {}) != {"iso", "front", "side", "top"}:
        raise AssertionError("VP204_RENDER_VIEW_SET_INVALID")
    render_sha256 = canonical_sha256(rendered.render_view_sha256)
    return {
        "compile_ms": compile_ms,
        "cache_hit_ms": cache_hit_ms,
        "render_ms": render_ms,
        "cold_geometry_ms": compile_ms + render_ms,
        "glb_sha256": compiled.glb_sha256,
        "render_sha256": render_sha256,
        "cache_key_sha256": hit.cache_key_sha256,
        "triangle_count": compiled.triangle_count,
        "fragment_cache_hit_operation_ids": compiled.fragment_cache_hit_operation_ids,
        "fragment_cache_miss_operation_ids": compiled.fragment_cache_miss_operation_ids,
    }


def phase(sequence: int, name: str, duration_ms: int, input_hash: str, output_hash: str, cache: str) -> dict[str, Any]:
    return {
        "sequence": sequence,
        "phase": name,
        "duration_ms": duration_ms,
        "input_sha256": input_hash,
        "output_sha256": output_hash,
        "cache": cache,
    }


def receipt(
    session_id: str,
    lowering: dict[str, Any],
    evidence: dict[str, Any],
    request_sha256: str,
    gate_sha256: str,
    include_preview: bool,
) -> dict[str, Any]:
    source_hash = lowering["source_program_sha256"]
    expanded_hash = lowering["expanded_dag"]["expanded_program_sha256"]
    shape_hash = lowering["shape_program_sha256"]
    phases = [
        phase(1, "author", 0, request_sha256, source_hash, "not_applicable"),
        phase(2, "validate", 0, source_hash, source_hash, "not_applicable"),
        phase(3, "expand", 0, source_hash, expanded_hash, "not_applicable"),
        phase(4, "lower", 0, expanded_hash, shape_hash, "not_applicable"),
        phase(5, "compile_readback", evidence["compile_ms"], shape_hash, evidence["glb_sha256"], "miss"),
        phase(6, "render", evidence["render_ms"], evidence["glb_sha256"], evidence["render_sha256"], "miss"),
        phase(7, "evaluate", 0, evidence["render_sha256"], gate_sha256, "not_applicable"),
    ]
    if include_preview:
        phases.append(phase(8, "preview", 0, gate_sha256, source_hash, "not_applicable"))
    result = {
        "schema_version": "VisualProgramExecutionReceipt@1",
        "receipt_id": f"receipt_{session_id}_0",
        "session_id": session_id,
        "authoring_count": 1,
        "patch_count": 0,
        "source_program_sha256": source_hash,
        "expanded_program_sha256": expanded_hash,
        "shape_program_sha256": shape_hash,
        "glb_sha256": evidence["glb_sha256"],
        "phases": phases,
        "usage": {
            "provider_requests": 0,
            "product_tool_calls": 2,
            "input_tokens": 0,
            "output_tokens": 0,
            "prompt_cache_hit_tokens": 0,
            "prompt_cache_miss_tokens": 0,
            "estimated_cost_microusd": 0,
        },
        "cancelled": False,
    }
    validate("visual-program-execution-receipt-v1.schema.json", result)
    return result


def patched_receipt(
    session_id: str,
    initial: dict[str, Any],
    initial_evidence: dict[str, Any],
    patched: dict[str, Any],
    patched_evidence: dict[str, Any],
    request_sha256: str,
    initial_gate_sha256: str,
    patched_gate_sha256: str,
) -> dict[str, Any]:
    initial_source = initial["source_program_sha256"]
    initial_expanded = initial["expanded_dag"]["expanded_program_sha256"]
    initial_shape = initial["shape_program_sha256"]
    patched_source = patched["source_program_sha256"]
    patched_expanded = patched["expanded_dag"]["expanded_program_sha256"]
    patched_shape = patched["shape_program_sha256"]
    chain = [
        ("author", 0, request_sha256, initial_source, "not_applicable"),
        ("validate", 0, initial_source, initial_source, "not_applicable"),
        ("expand", 0, initial_source, initial_expanded, "not_applicable"),
        ("lower", 0, initial_expanded, initial_shape, "not_applicable"),
        ("compile_readback", initial_evidence["compile_ms"], initial_shape, initial_evidence["glb_sha256"], "miss"),
        ("render", initial_evidence["render_ms"], initial_evidence["glb_sha256"], initial_evidence["render_sha256"], "miss"),
        ("evaluate", 0, initial_evidence["render_sha256"], initial_gate_sha256, "not_applicable"),
        ("patch", 0, initial_gate_sha256, patched_source, "not_applicable"),
        ("validate", 0, patched_source, patched_source, "not_applicable"),
        ("expand", 0, patched_source, patched_expanded, "not_applicable"),
        ("lower", 0, patched_expanded, patched_shape, "not_applicable"),
        ("compile_readback", patched_evidence["compile_ms"], patched_shape, patched_evidence["glb_sha256"], "miss"),
        ("render", patched_evidence["render_ms"], patched_evidence["glb_sha256"], patched_evidence["render_sha256"], "miss"),
        ("evaluate", 0, patched_evidence["render_sha256"], patched_gate_sha256, "not_applicable"),
        ("preview", 0, patched_gate_sha256, patched_source, "not_applicable"),
    ]
    result = {
        "schema_version": "VisualProgramExecutionReceipt@1",
        "receipt_id": f"receipt_{session_id}_1",
        "session_id": session_id,
        "authoring_count": 1,
        "patch_count": 1,
        "source_program_sha256": patched_source,
        "expanded_program_sha256": patched_expanded,
        "shape_program_sha256": patched_shape,
        "glb_sha256": patched_evidence["glb_sha256"],
        "phases": [phase(index + 1, *item) for index, item in enumerate(chain)],
        "usage": {
            "provider_requests": 0,
            "product_tool_calls": 4,
            "input_tokens": 0,
            "output_tokens": 0,
            "prompt_cache_hit_tokens": 0,
            "prompt_cache_miss_tokens": 0,
            "estimated_cost_microusd": 0,
        },
        "cancelled": False,
    }
    validate("visual-program-execution-receipt-v1.schema.json", result)
    return result


def gate(source_hash: str, report_id: str, verdict: str, repairable: bool) -> dict[str, Any]:
    result = {
        "schema_version": "VisualProgramGateOutcome@1",
        "gate_report_id": report_id,
        "source_program_sha256": source_hash,
        "verdict": verdict,
        "repairable": repairable,
    }
    validate("visual-program-gate-outcome-v1.schema.json", result)
    return result


def hang_worker(_connection: Any, _cancel_event: Any, _payload: dict[str, Any], _resource_root: str | None) -> None:
    time.sleep(2)


def verify_failure_boundaries(shape_program: dict[str, Any]) -> None:
    cancelled_executor = RestrictedGeometryExecutor(environment={})
    cancelled_executor.cancel(
        RestrictedGeometryCancellationRequest(
            cancellation_id="cancel_vp204_prestart",
            cancellation_token="token_vp204_prestart",
        )
    )
    cancelled_cache = VisualProgramCompileCache(cancelled_executor)
    try:
        cancelled_cache.compile(
            execution_id="exec_vp204_cancelled",
            idempotency_key="idem_vp204_cancelled",
            cancellation_id="cancel_vp204_prestart",
            cancellation_token="token_vp204_prestart",
            shape_program=shape_program,
        )
    except RestrictedGeometryBoundaryError as error:
        if error.code != "GEOMETRY_EXECUTION_CANCELLED":
            raise
    else:
        raise AssertionError("VP204_CANCELLED_COMPILE_SUCCEEDED")
    if cancelled_cache.entry_count != 0 or cancelled_cache.retained_bytes != 0:
        raise AssertionError("VP204_CANCELLED_RESULT_WAS_CACHED")

    timeout_cache = VisualProgramCompileCache(
        RestrictedGeometryExecutor(environment={}, worker_target=hang_worker)
    )
    try:
        timeout_cache.compile(
            execution_id="exec_vp204_timeout",
            idempotency_key="idem_vp204_timeout",
            cancellation_id="cancel_vp204_timeout",
            cancellation_token="token_vp204_timeout",
            shape_program=shape_program,
            timeout_ms=50,
        )
    except RestrictedGeometryBoundaryError as error:
        if error.code != "GEOMETRY_EXECUTION_TIMEOUT":
            raise
    else:
        raise AssertionError("VP204_TIMEOUT_COMPILE_SUCCEEDED")
    if timeout_cache.entry_count != 0 or timeout_cache.retained_bytes != 0:
        raise AssertionError("VP204_TIMEOUT_RESULT_WAS_CACHED")


def percentile(values: list[int], fraction: float) -> int:
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def main() -> int:
    for schema in SCHEMAS.values():
        Draft202012Validator.check_schema(schema)
    lowerings = vp203_lowerings()
    sources = {name: fixture(name) for name in ("bracket", "rotor", "duct")}
    executor = RestrictedGeometryExecutor(environment={})
    cache = VisualProgramCompileCache(executor)

    evidence = {
        name: compile_render(executor, cache, name, lowerings[name], "initial")
        for name in ("bracket", "rotor", "duct")
    }
    verify_failure_boundaries(lowerings["bracket"]["shape_program"])

    bracket_request = canonical_sha256({"fixture": "bracket", "mode": "offline_author"})
    bracket_gate = gate(lowerings["bracket"]["source_program_sha256"], "gate_bracket_pass", "pass", False)
    bracket_result = run_rust(
        "vp204_low_roundtrip",
        {
            "action": "session",
            "session_id": "vpsession_bracket",
            "idempotency_key": "idem_bracket",
            "request_sha256": bracket_request,
            "source": sources["bracket"],
            "initial_receipt": receipt(
                "vpsession_bracket",
                lowerings["bracket"],
                evidence["bracket"],
                bracket_request,
                canonical_sha256(bracket_gate),
                True,
            ),
            "initial_gate": bracket_gate,
        },
    )
    bracket_session = bracket_result["session"]
    validate("visual-program-authoring-session-v1.schema.json", bracket_session)
    if bracket_session["state"] != "ready_for_preview" or bracket_session["patch_count"] != 0:
        raise AssertionError("VP204_ZERO_PATCH_SESSION_INVALID")

    rotor_patch = {
        "schema_version": "ForgeVisualGeometryPatch@1",
        "patch_id": "patch_rotor_spacing",
        "expected_source_sha256": lowerings["rotor"]["source_program_sha256"],
        "operations": [
            {"op": "set_array", "node_id": "node_rotor_bank", "count": 4, "spacing": 760.0}
        ],
    }
    validate("forge-visual-geometry-patch-v1.schema.json", rotor_patch)
    patched_payload = run_rust(
        "vp204_low_roundtrip", {"action": "patch", "source": sources["rotor"], "patch": rotor_patch}
    )
    validate("geometry-incremental-plan-v1.schema.json", patched_payload["incremental_plan"])
    plan = patched_payload["incremental_plan"]
    if "node_rotor" not in plan["reused_source_node_ids"]:
        raise AssertionError("VP204_UNCHANGED_SOURCE_NODE_NOT_REUSED")
    for node_id in ("node_rotor_bank", "node_rotor_part", "node_rotor_zone"):
        if node_id not in plan["invalidated_source_node_ids"]:
            raise AssertionError(f"VP204_DEPENDENT_NODE_NOT_INVALIDATED:{node_id}")
    if "op_rotor" not in plan["reused_shape_operation_ids"] or "op_rotor_bank" not in plan["invalidated_shape_operation_ids"]:
        raise AssertionError("VP204_SHAPE_OPERATION_INVALIDATION_WRONG")
    if "output_rotor_bank" not in plan["invalidated_output_ids"] or plan["full_compile_cache_hit"]:
        raise AssertionError("VP204_OUTPUT_OR_FULL_CACHE_INVALIDATION_WRONG")

    patched_lowering = patched_payload["lowering"]
    patched_evidence = compile_render(executor, cache, "rotor", patched_lowering, "patched")
    if patched_evidence["cache_key_sha256"] == evidence["rotor"]["cache_key_sha256"]:
        raise AssertionError("VP204_CHANGED_GRAPH_REUSED_FULL_COMPILE_CACHE")
    if (
        "op_rotor" not in patched_evidence["fragment_cache_hit_operation_ids"]
        or "op_rotor_bank" not in patched_evidence["fragment_cache_miss_operation_ids"]
    ):
        raise AssertionError("VP204_COMPILED_FRAGMENT_REUSE_NOT_PROVEN")
    rotor_request = canonical_sha256({"fixture": "rotor", "mode": "offline_author_patch"})
    initial_rotor_gate = gate(lowerings["rotor"]["source_program_sha256"], "gate_rotor_repair", "fail", True)
    patched_rotor_gate = gate(patched_lowering["source_program_sha256"], "gate_rotor_pass", "pass", False)
    rotor_result = run_rust(
        "vp204_low_roundtrip",
        {
            "action": "session",
            "session_id": "vpsession_rotor",
            "idempotency_key": "idem_rotor",
            "request_sha256": rotor_request,
            "source": sources["rotor"],
            "initial_receipt": receipt(
                "vpsession_rotor",
                lowerings["rotor"],
                evidence["rotor"],
                rotor_request,
                canonical_sha256(initial_rotor_gate),
                False,
            ),
            "initial_gate": initial_rotor_gate,
            "patch": rotor_patch,
            "patched_receipt": patched_receipt(
                "vpsession_rotor",
                lowerings["rotor"],
                evidence["rotor"],
                patched_lowering,
                patched_evidence,
                rotor_request,
                canonical_sha256(initial_rotor_gate),
                canonical_sha256(patched_rotor_gate),
            ),
            "patched_gate": patched_rotor_gate,
            "replay_patch": True,
            "second_patch": {
                "schema_version": "ForgeVisualGeometryPatch@1",
                "patch_id": "patch_rotor_second",
                "expected_source_sha256": patched_lowering["source_program_sha256"],
                "operations": [
                    {"op": "set_array", "node_id": "node_rotor_bank", "count": 5, "spacing": 800.0}
                ],
            },
        },
    )
    rotor_session = rotor_result["session"]
    validate("visual-program-authoring-session-v1.schema.json", rotor_session)
    if (
        rotor_session["state"] != "ready_for_preview"
        or rotor_session["current_revision"] != 2
        or rotor_session["parent_source_sha256"] != lowerings["rotor"]["source_program_sha256"]
        or rotor_result["same_patch_replay_idempotent"] is not True
        or rotor_result["second_patch_error_code"] != "FORGE_VISUAL_VP204_PATCH_LIMIT_REACHED"
    ):
        raise AssertionError("VP204_ONE_PATCH_SESSION_INVALID")

    stale_patch = json.loads(json.dumps(rotor_patch))
    stale_patch["expected_source_sha256"] = "0" * 64
    stale = run_rust(
        "vp204_low_roundtrip",
        {"action": "patch", "source": sources["rotor"], "patch": stale_patch},
        expect_success=False,
    )
    if "FORGE_VISUAL_VP204_PATCH_STALE" not in stale["stderr"]:
        raise AssertionError("VP204_STALE_PATCH_DID_NOT_FAIL_CLOSED")

    duct_request = canonical_sha256({"fixture": "duct", "mode": "offline_hard_fail"})
    duct_gate = gate(lowerings["duct"]["source_program_sha256"], "gate_duct_hard_fail", "fail", False)
    duct_result = run_rust(
        "vp204_low_roundtrip",
        {
            "action": "session",
            "session_id": "vpsession_duct",
            "idempotency_key": "idem_duct",
            "request_sha256": duct_request,
            "source": sources["duct"],
            "initial_receipt": receipt(
                "vpsession_duct",
                lowerings["duct"],
                evidence["duct"],
                duct_request,
                canonical_sha256(duct_gate),
                False,
            ),
            "initial_gate": duct_gate,
        },
    )
    validate("visual-program-authoring-session-v1.schema.json", duct_result["session"])
    if duct_result["session"]["state"] != "failed":
        raise AssertionError("VP204_HARD_GATE_FAILURE_NOT_TERMINAL")

    cold_values = [item["cold_geometry_ms"] for item in evidence.values()]
    cold_values.append(patched_evidence["cold_geometry_ms"])
    hit_values = [item["cache_hit_ms"] for item in evidence.values()] + [patched_evidence["cache_hit_ms"]]
    p50 = percentile(cold_values, 0.50)
    p90 = percentile(cold_values, 0.90)
    if p50 > 32_000 or p90 > 70_000 or max(cold_values) > 105_000:
        raise AssertionError(f"VP204_OFFLINE_GEOMETRY_BASELINE_SLOW:p50={p50}:p90={p90}:max={max(cold_values)}")
    print(
        "VP204 low-roundtrip gate passed: "
        f"sessions=zero-patch+one-patch+hard-fail; cache=miss/hit/change-miss/hit; "
        f"offline_geometry_ms p50={p50} p90={p90} max={max(cold_values)}; "
        f"cache_hit_ms p50={percentile(hit_values, 0.50)} p90={percentile(hit_values, 0.90)}; "
        "provider_requests=0 billable_cost=0 (provider authoring latency excluded)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
