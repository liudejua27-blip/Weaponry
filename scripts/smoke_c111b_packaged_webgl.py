#!/usr/bin/env python3
"""C111B packaged WebGL renderer checks.

The default mode is the historical external-reference readback. The
``FORGECAD_C111B_WEBVIEW_QA_MODE=agent_asset`` mode drives the visible Agent
composer and A005 confirmation path, then requires the same committed Agent
asset, production export hash and single WebGL renderer before and after a
new-process restart. Neither mode claims visual similarity or human score.
"""

from __future__ import annotations

import hashlib
import json
import os
import plistlib
import shutil
import subprocess
import tempfile
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path

from smoke_arm_webview_packaged_tauri import (
    APP_BUNDLE,
    APP_BINARY,
    _assert,
    _desktop_pids,
    _is_descendant,
    _listener_pid,
    _stop_desktop_and_listener,
    _wait_for_native_health,
)


ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "C111BPackagedWebGL@1"
MARKER = "ForgeCAD C111B packaged WebGL QA report="
PROGRESS_MARKER = "ForgeCAD C111B packaged WebGL QA progress="
MODE = os.environ.get("FORGECAD_C111B_WEBVIEW_QA_MODE", "external_reference")
SOURCE = ROOT / "output" / "c111b-contract-iteration-79" / "robotic-arm-golden-surface-production.glb"
ACCEPTANCE_CONTRACT = ROOT / "packages" / "concept-spec" / "fixtures" / "c111b-visual-acceptance-contract.json"
EVIDENCE = ROOT / "output" / ("c111b-packaged-agent-webgl" if MODE == "agent_asset" else "c111b-packaged-webgl") / "packaged-webgl-qa.json"
ATTEMPT = EVIDENCE.parent / "packaged-webgl-qa-attempt.json"
FAILURE_LOG = EVIDENCE.parent / "packaged-webgl-qa-failure.log"
CAPTURE_ROOT = EVIDENCE.parent / "captures"
SOURCE_SHA256 = "48ccc5c6a725936d43cb731ed5e20b93f10ef751712ed79469ea406318160b6b"
AGENT_V2_MATERIAL_COUNT = 14
VIEWS = ("iso", "front", "back", "left", "right", "top", "gripper_iso", "gripper_front")
REPORT_TIMEOUT_SECONDS = 900.0
RUN_ID = f"c111b_{uuid.uuid4().hex}"
RUN_STARTED_MONOTONIC = time.monotonic()
RUN_STARTED_AT = datetime.now(timezone.utc).isoformat()
CURRENT_PHASE = "preflight"


def _generation_timing_target_ms() -> int:
    contract = json.loads(ACCEPTANCE_CONTRACT.read_text(encoding="utf-8"))
    timing = contract.get("budgets", {}).get("timing", {})
    seconds = timing.get("target_total_seconds")
    required_stages = timing.get("required_stage_keys")
    _assert(seconds == 120, "C111B generation timing target drifted")
    _assert(
        required_stages == ["author", "lower", "compile_readback", "render", "evaluate", "preview"],
        "C111B generation timing stage contract drifted",
    )
    return int(seconds) * 1000


def main() -> int:
    global CURRENT_PHASE
    FAILURE_LOG.unlink(missing_ok=True)
    _write_attempt("running")
    try:
        _assert(APP_BINARY.is_file(), "build the macOS .app before C111B packaged WebGL QA")
        if MODE == "external_reference":
            _assert(SOURCE.is_file(), "C111B exact production GLB is missing")
        _assert(_listener_pid() is None, "port 8000 must be free before C111B packaged WebGL QA")
        _assert(not _macos_console_screen_locked(), "C111B_PACKAGED_SCREEN_LOCKED")
        with tempfile.TemporaryDirectory(prefix="forgecad_c111b_webgl_qa_") as raw:
            temporary = Path(raw)
            if MODE == "external_reference":
                input_path = temporary / "qa-inputs" / "c111b-production.glb"
                input_path.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(SOURCE, input_path)
            try:
                result = _run(temporary)
            except BaseException:
                log = temporary / "WushenForge" / "agent.log"
                if log.is_file():
                    FAILURE_LOG.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(log, FAILURE_LOG)
                failed_captures = temporary / "WushenForge" / "qa-artifacts" / "c111b-webgl"
                if failed_captures.is_dir():
                    failure_capture_root = EVIDENCE.parent / "failed-captures" / RUN_ID
                    shutil.copytree(failed_captures, failure_capture_root)
                raise
    except BaseException as exc:
        _write_attempt("failed", _failure_code(exc))
        raise
    CURRENT_PHASE = "evidence_write"
    result["run_id"] = RUN_ID
    evidence_sha256 = _write_evidence(result)
    _write_attempt(
        "passed",
        evidence_sha256=evidence_sha256,
        export_sha256=str(result["source_sha256"]),
    )
    print(json.dumps(result, ensure_ascii=False, separators=(",", ":")))
    return 0


def _run(temporary: Path) -> dict[str, object]:
    global CURRENT_PHASE
    library = temporary / "library"
    log = temporary / "WushenForge" / "agent.log"
    environment = _environment(temporary, library)
    initial_started = time.monotonic()
    CURRENT_PHASE = "initial"
    first_pid = _start(temporary, environment)
    try:
        listener = _wait_for_native_health(first_pid)
        _assert(_is_descendant(listener, first_pid), "C111B packaged sidecar is not desktop-owned")
        initial = _wait_report(log, "initial")
        _validate_report(initial, "initial")
        initial_export_sha256 = str(initial["source_sha256"])
        initial_wall_ms = round((time.monotonic() - initial_started) * 1000)
    finally:
        _stop_desktop_and_listener(first_pid)
    _assert(_listener_pid() is None, "C111B packaged sidecar survived initial shutdown")

    environment.update({
        "FORGECAD_C111B_WEBVIEW_QA_PHASE": "restart",
        "FORGECAD_C111B_WEBVIEW_QA_EXPECT_PROJECT_ID": str(initial["project_id"]),
        "FORGECAD_C111B_WEBVIEW_QA_EXPECT_ASSET_VERSION_ID": str(initial["asset_version_id"]),
        "FORGECAD_C111B_WEBVIEW_QA_EXPECT_SNAPSHOT_REVISION": str(initial["snapshot_revision"]),
        "FORGECAD_C111B_WEBVIEW_QA_EXPECT_EXPORT_SHA256": initial_export_sha256,
    })
    CURRENT_PHASE = "restart"
    restart_started = time.monotonic()
    restarted_pid = _start(temporary, environment)
    try:
        listener = _wait_for_native_health(restarted_pid)
        _assert(_is_descendant(listener, restarted_pid), "C111B restarted sidecar is not desktop-owned")
        restart = _wait_report(log, "restart")
        _validate_report(restart, "restart", expected_agent_sha256=initial_export_sha256)
        _assert(restart["project_id"] == initial["project_id"], "C111B restart project drifted")
        _assert(restart["asset_version_id"] == initial["asset_version_id"], "C111B restart asset drifted")
        _assert(restart["snapshot_revision"] == initial["snapshot_revision"], "C111B restart Snapshot drifted")
        _assert(restart["source_sha256"] == initial_export_sha256, "C111B restart export SHA drifted")
        _assert(restart["renderer_generation"] > 0, "C111B restart renderer was not created")
        restart_wall_ms = round((time.monotonic() - restart_started) * 1000)
    finally:
        _stop_desktop_and_listener(restarted_pid)
    _assert(_listener_pid() is None, "C111B packaged sidecar survived restart shutdown")

    CURRENT_PHASE = "preserve"
    copied = _preserve_captures(temporary, initial, restart)
    generation_timing_target_ms = _generation_timing_target_ms()
    generation_timing_elapsed_ms = int(initial["turn_total_elapsed_ms"])
    generation_timing_passed = generation_timing_elapsed_ms <= generation_timing_target_ms
    _assert(generation_timing_passed, "C111B_GENERATION_TIMING_TARGET_EXCEEDED")
    return {
        "schema_version": SCHEMA,
        "ok": True,
        "real_packaged_webview": True,
        "mode": MODE,
        "exact_source_glb": str(SOURCE.relative_to(ROOT)) if MODE == "external_reference" else None,
        "source_sha256": initial_export_sha256,
        "triangle_count": 138248,
        "primitive_count": 157,
        "material_count": AGENT_V2_MATERIAL_COUNT if MODE == "agent_asset" else 12,
        "single_renderer": True,
        "provider_protocol_requests": int(initial["provider_protocol_requests"]) + int(restart["provider_protocol_requests"]),
        "product_tool_calls": int(initial["product_tool_calls"]) + int(restart["product_tool_calls"]),
        "input_tokens": int(initial["input_tokens"]) + int(restart["input_tokens"]),
        "output_tokens": int(initial["output_tokens"]) + int(restart["output_tokens"]),
        "prompt_cache_hit_tokens": int(initial["prompt_cache_hit_tokens"]) + int(restart["prompt_cache_hit_tokens"]),
        "prompt_cache_miss_tokens": int(initial["prompt_cache_miss_tokens"]) + int(restart["prompt_cache_miss_tokens"]),
        "same_intent_repair_attempts": int(initial["same_intent_repair_attempts"]) + int(restart["same_intent_repair_attempts"]),
        "same_intent_repairs_applied": int(initial["same_intent_repairs_applied"]) + int(restart["same_intent_repairs_applied"]),
        "provider_schema_repair_requests": int(initial["provider_schema_repair_requests"]) + int(restart["provider_schema_repair_requests"]),
        "product_tool_schema_repair_requests": int(initial["product_tool_schema_repair_requests"]) + int(restart["product_tool_schema_repair_requests"]),
        "provider_usage_estimated_cost_microusd": int(initial["estimated_cost_microusd"]) + int(restart["estimated_cost_microusd"]),
        "billable_variable_cost_microusd": int(initial["billable_variable_cost_microusd"]) + int(restart["billable_variable_cost_microusd"]),
        "network_provider_calls": int(initial["network_provider_calls"]) + int(restart["network_provider_calls"]),
        "network_call_made": bool(initial["network_call_made"]) or bool(restart["network_call_made"]),
        "credential_reads": int(initial["credential_reads"]) + int(restart["credential_reads"]),
        "initial_wall_ms": initial_wall_ms,
        "restart_wall_ms": restart_wall_ms,
        "total_wall_ms": initial_wall_ms + restart_wall_ms,
        "generation_timing_target_ms": generation_timing_target_ms,
        "generation_timing_elapsed_ms": generation_timing_elapsed_ms,
        "generation_timing_passed": generation_timing_passed,
        "generation_timing_source": "rust_terminal_turn_six_phase_total",
        "workflow_wall_timing_source": "packaged_generation_edit_export_restart_qa",
        "formal_eligible": False,
        "reference_comparison": False,
        "human_benchmark_evidence": False,
        "initial": initial,
        "restart": restart,
        "captures": copied,
    }


def _environment(temporary: Path, library: Path) -> dict[str, str]:
    environment = {
        name: os.environ[name]
        for name in (
            "PATH", "TMPDIR", "LANG", "LC_ALL", "USER", "LOGNAME", "SHELL",
            "__CF_USER_TEXT_ENCODING",
        )
        if name in os.environ
    }
    environment.update({
        "HOME": str(temporary),
        "WUSHEN_LIBRARY_ROOT": str(library),
        "WUSHEN_AGENT_RUNTIME_MODE": "packaged-sidecar",
        "FORGECAD_DISABLE_PROVIDER_CONFIG": "1",
        "FORGECAD_CONCEPT_WORKER_ENABLED": "0",
        "WUSHEN_LOCAL_WORKER_ENABLED": "0",
        "FORGECAD_C111B_WEBVIEW_QA": "1",
        "FORGECAD_C111B_WEBVIEW_QA_PHASE": "initial",
        "FORGECAD_C111B_WEBVIEW_QA_MODE": MODE,
    })
    if MODE == "agent_asset":
        environment.update({
            "FORGECAD_MVP_OFFLINE_ARM": "1",
            "FORGECAD_MVP_ARM_ARCHITECTURE": "golden_surface",
            "FORGECAD_MVP_VISUAL_PROGRAM_E2E": "1",
        })
    return environment


def _start(temporary: Path, environment: dict[str, str]) -> int:
    _assert(not _macos_console_screen_locked(), "C111B_PACKAGED_SCREEN_LOCKED")
    existing = _desktop_pids()
    names = {
        "HOME", "WUSHEN_LIBRARY_ROOT", "WUSHEN_AGENT_RUNTIME_MODE", "FORGECAD_DISABLE_PROVIDER_CONFIG",
        "FORGECAD_CONCEPT_WORKER_ENABLED", "WUSHEN_LOCAL_WORKER_ENABLED", "FORGECAD_C111B_WEBVIEW_QA",
        "FORGECAD_C111B_WEBVIEW_QA_MODE",
        "FORGECAD_C111B_WEBVIEW_QA_PHASE", "FORGECAD_C111B_WEBVIEW_QA_EXPECT_PROJECT_ID",
        "FORGECAD_C111B_WEBVIEW_QA_EXPECT_ASSET_VERSION_ID", "FORGECAD_C111B_WEBVIEW_QA_EXPECT_SNAPSHOT_REVISION",
        "FORGECAD_MVP_OFFLINE_ARM", "FORGECAD_MVP_ARM_ARCHITECTURE",
        "FORGECAD_MVP_VISUAL_PROGRAM_E2E",
    }
    command = ["open", "-n"]
    for name in sorted(names):
        if name in environment:
            command.extend(["--env", f"{name}={environment[name]}"])
    command.append(str(APP_BUNDLE))
    subprocess.run(command, cwd=temporary, env=environment, check=True)
    for _ in range(100):
        created = _desktop_pids() - existing
        if created:
            return max(created)
        time.sleep(0.1)
    raise AssertionError("LaunchServices did not start C111B packaged WebGL QA")


def _wait_report(path: Path, phase: str) -> dict[str, object]:
    deadline = time.monotonic() + REPORT_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        if path.is_file():
            for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
                if not line.startswith(MARKER):
                    continue
                try:
                    report = json.loads(line[len(MARKER):])
                except ValueError:
                    continue
                if isinstance(report, dict) and report.get("phase") == phase:
                    _assert(report.get("schema_version") == SCHEMA, "C111B packaged WebGL schema drifted")
                    if report.get("ok") is not True:
                        code = report.get("error_code")
                        _assert(isinstance(code, str) and code.startswith("C111B_") and len(code) <= 160, "C111B packaged WebGL failure code is invalid")
                        raise AssertionError(code)
                    return report
        time.sleep(0.2)
    progress = "none"
    if path.is_file():
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith(PROGRESS_MARKER):
                progress = line[len(PROGRESS_MARKER):].strip()
    raise AssertionError(f"C111B packaged WebGL {phase} timed out; last_progress={progress}")


def _validate_report(
    report: dict[str, object],
    phase: str,
    *,
    expected_agent_sha256: str | None = None,
) -> None:
    _assert(report.get("schema_version") == SCHEMA, "C111B report schema is invalid")
    _assert(report.get("phase") == phase, "C111B report phase is invalid")
    report_sha256 = report.get("source_sha256")
    if MODE == "agent_asset":
        _assert(_is_sha256(report_sha256), "C111B Agent export SHA is invalid")
        _assert(report_sha256 != SOURCE_SHA256, "C111B Agent V2 did not change the V1 production GLB")
        if expected_agent_sha256 is not None:
            _assert(report_sha256 == expected_agent_sha256, "C111B Agent restart export SHA drifted")
    else:
        _assert(report_sha256 == SOURCE_SHA256, "C111B source SHA drifted")
    _assert(report.get("triangle_count") == 138248, "C111B triangle inventory drifted")
    _assert(report.get("primitive_count") == 157, "C111B primitive inventory drifted")
    _assert(
        report.get("material_count") == (AGENT_V2_MATERIAL_COUNT if MODE == "agent_asset" else 12),
        "C111B material inventory drifted",
    )
    _assert(report.get("complete_pbr_material_count", 0) > 0, "C111B PBR inventory is incomplete")
    _assert(report.get("active_webgl_contexts") == 1, "C111B created more than one WebGL context")
    _assert(report.get("canvas_count") == 1, "C111B created more than one canvas")
    if MODE == "agent_asset":
        _assert(report.get("blockout_glb_kind") == "compiled_agent_production_pbr", "C111B did not display the committed Agent production asset")
        _assert(report.get("render_source") == "glb_pbr", "C111B Agent renderer source is not production GLB PBR")
    else:
        _assert(report.get("blockout_glb_kind") == "external_reference", "C111B did not display the exact external GLB")
        _assert(report.get("render_source") == "external_reference", "C111B renderer source is not exact external GLB")
    _assert(report.get("light_preset") == "soft_studio", "C111B light preset is not soft_studio")
    _assert(report.get("provider_protocol_requests") == (1 if MODE == "agent_asset" and phase == "initial" else 0), "C111B packaged QA Provider protocol accounting is invalid")
    expected_turn = MODE == "agent_asset" and phase == "initial"
    _assert(report.get("product_tool_calls") == (6 if expected_turn else 0), "C111B packaged QA Product Tool accounting is invalid")
    _assert(report.get("input_tokens") == (1 if expected_turn else 0), "C111B packaged QA input token accounting is invalid")
    _assert(report.get("output_tokens") == (1 if expected_turn else 0), "C111B packaged QA output token accounting is invalid")
    _assert(report.get("prompt_cache_hit_tokens") == 0 and report.get("prompt_cache_miss_tokens") == 0, "C111B packaged QA cache accounting is invalid")
    for field in ("same_intent_repair_attempts", "same_intent_repairs_applied", "provider_schema_repair_requests", "product_tool_schema_repair_requests"):
        _assert(report.get(field) == 0, f"C111B packaged QA {field} is invalid")
    _assert(report.get("estimated_cost_microusd") == (1 if expected_turn else 0), "C111B packaged QA Provider usage cost is invalid")
    _assert(report.get("billable_variable_cost_microusd") == 0, "C111B packaged QA billable variable cost is nonzero")
    _assert(
        report.get("billable_variable_cost_source")
        == ("native_offline_no_billable_transport" if MODE == "agent_asset" else "native_no_agent_provider_path"),
        "C111B packaged QA billable cost source is invalid",
    )
    _assert(report.get("network_provider_calls") == 0 and report.get("network_call_made") is False, "C111B packaged QA made a forbidden network Provider call")
    _assert(report.get("credential_reads") == 0, "C111B packaged QA read credentials")
    _assert(
        report.get("provider_metrics_source")
        == ("rust_terminal_turn_plus_native_local_mvp_counter" if expected_turn else "native_local_mvp_atomic_counter" if MODE == "agent_asset" else "native_no_agent_provider_path"),
        "C111B packaged QA Provider accounting is not native-measured",
    )
    _assert(
        report.get("credential_metrics_source")
        == ("native_structural_no_credential_source" if MODE == "agent_asset" else "native_no_agent_provider_path"),
        "C111B packaged QA credential accounting source is invalid",
    )
    _assert(report.get("formal_eligible") is False, "C111B packaged QA overclaimed formal eligibility")
    _assert(report.get("reference_comparison") is False, "C111B packaged QA overclaimed reference comparison")
    _assert(report.get("human_benchmark_evidence") is False, "C111B packaged QA overclaimed human evidence")
    _assert(report.get("restart_hydrated") is (phase == "restart"), "C111B restart state is invalid")
    if expected_turn:
        _assert(isinstance(report.get("thread_id"), str) and isinstance(report.get("turn_id"), str), "C111B terminal Turn identity is missing")
        _assert(report.get("turn_metrics_source") == "rust_terminal_turn_readback", "C111B terminal Turn source is invalid")
        timing_target_ms = _generation_timing_target_ms()
        _assert(isinstance(report.get("turn_total_elapsed_ms"), int) and 0 < report["turn_total_elapsed_ms"] <= timing_target_ms, "C111B_GENERATION_TIMING_TARGET_EXCEEDED")
        phases = report.get("turn_phase_timings_ms")
        _assert(isinstance(phases, dict) and set(phases) == {"author", "lower", "compile_readback", "render", "evaluate", "preview"}, "C111B Turn stage timing set is invalid")
        _assert(all(isinstance(value, int) and not isinstance(value, bool) and 0 <= value <= report["turn_total_elapsed_ms"] for value in phases.values()), "C111B Turn stage timing value is invalid")
        _assert(isinstance(report.get("turn_trace_sha256"), str) and len(report["turn_trace_sha256"]) == 64, "C111B Turn trace digest is invalid")
    else:
        _assert(report.get("turn_total_elapsed_ms") == 0 and report.get("turn_phase_timings_ms") == {}, "C111B non-Turn phase fabricated Turn timing")
        _assert("turn_trace_sha256" not in report and "thread_id" not in report and "turn_id" not in report, "C111B non-Turn phase fabricated Turn identity")
    expected_timing_stages = {
        ("agent_asset", "initial"): ["agent_workbench_ready", "agent_brief_sent", "agent_v1_confirmed", "agent_selection_card_ready", "agent_link_part_selected", "agent_adornment_drawer_ready", "agent_v2_confirmed", "agent_export_readback_ready", "agent_captures_ready", "report_received"],
        ("agent_asset", "restart"): ["agent_restart_workbench_ready", "agent_restart_snapshot_hydrated", "agent_restart_export_readback_ready", "agent_restart_captures_ready", "report_received"],
        ("external_reference", "initial"): ["workbench_ready", "visible_import_requested", "external_asset_ready", "external_captures_ready", "report_received"],
        ("external_reference", "restart"): ["external_restart_workbench_ready", "external_restart_snapshot_hydrated", "external_restart_captures_ready", "report_received"],
    }[(MODE, phase)]
    timings = report.get("stage_timings")
    _assert(isinstance(timings, list) and [item.get("stage") for item in timings if isinstance(item, dict)] == expected_timing_stages, "C111B native lifecycle stage set is invalid")
    previous = 0
    for timing in timings:
        _assert(isinstance(timing.get("elapsed_ms"), int) and not isinstance(timing["elapsed_ms"], bool) and previous <= timing["elapsed_ms"] <= 900000, "C111B native lifecycle elapsed time is invalid")
        _assert(timing.get("duration_since_previous_ms") == timing["elapsed_ms"] - previous, "C111B native lifecycle duration is invalid")
        previous = timing["elapsed_ms"]
    _assert(report.get("end_to_end_elapsed_ms") == previous and report.get("timing_metrics_source") == "native_monotonic_progress_receipts", "C111B native lifecycle total or source is invalid")
    captures = report.get("captures")
    _assert(isinstance(captures, list) and len(captures) == 8, "C111B must capture eight fixed views")
    _assert({item.get("view_id") for item in captures if isinstance(item, dict)} == set(VIEWS), "C111B fixed view set drifted")
    for item in captures:
        _assert(isinstance(item, dict), "C111B capture receipt is invalid")
        view = str(item["view_id"])
        _assert(item.get("relative_path") == f"qa-artifacts/c111b-webgl/{phase}/{view}.png", "C111B capture path is not fixed")
        _assert(item.get("source_sha256") == report_sha256, "C111B capture lineage drifted")
        _assert(isinstance(item.get("sha256"), str) and len(item["sha256"]) == 64, "C111B capture hash is invalid")
        _assert(item.get("byte_size", 0) > 0 and item.get("width", 0) >= 320 and item.get("height", 0) >= 240, "C111B capture dimensions are invalid")
        readability = item.get("readability")
        _assert(isinstance(readability, dict), "C111B capture readability evidence is missing")
        _assert(readability.get("pixel_encoding") == "display_srgb", "C111B capture pixel encoding is invalid")
        _assert(
            readability.get("display_transfer") == "wkwebview_linear_lit_surface_to_srgb",
            "C111B capture display transfer is invalid",
        )
        _assert(readability.get("sample_pixel_count") == 96 * 96, "C111B readability sample size drifted")
        _assert(isinstance(readability.get("foreground_pixel_count"), int) and readability["foreground_pixel_count"] > 0, "C111B readability foreground is missing")
        _assert(isinstance(readability.get("foreground_coverage_bps"), int) and readability["foreground_coverage_bps"] >= 100, "C111B foreground coverage is too small")
        _assert(isinstance(readability.get("foreground_median_luma"), int) and readability["foreground_median_luma"] >= 24, "C111B foreground median luminance is unreadable")
        _assert(isinstance(readability.get("foreground_readable_bps"), int) and readability["foreground_readable_bps"] >= 5000, "C111B readable foreground ratio is too low")
        _assert(
            isinstance(readability.get("background_rgb"), list)
            and len(readability["background_rgb"]) == 3
            and all(isinstance(value, int) and 0 <= value <= 255 for value in readability["background_rgb"]),
            "C111B readability background color is invalid",
        )
    readback = report.get("readback")
    _assert(isinstance(readback, dict), "C111B exact readback is missing")
    if MODE == "agent_asset":
        _assert(readback.get("source_sha256") == report_sha256 and readback.get("shape_program_schema") == "ShapeProgram@1" and readback.get("external_reference") is False, "C111B Agent readback lineage is invalid")
    else:
        _assert(readback.get("source_sha256") == SOURCE_SHA256 and readback.get("shape_program_schema") == "ExternalGLBReference@1" and readback.get("external_reference") is True, "C111B readback lineage is invalid")


def _is_sha256(value: object) -> bool:
    return isinstance(value, str) and len(value) == 64 and all(character in "0123456789abcdef" for character in value)


def _preserve_captures(temporary: Path, initial: dict[str, object], restart: dict[str, object]) -> dict[str, dict[str, dict[str, object]]]:
    copied: dict[str, dict[str, dict[str, object]]] = {}
    for phase, report in (("initial", initial), ("restart", restart)):
        copied[phase] = {}
        for capture in report["captures"]:
            relative = str(capture["relative_path"])
            source = temporary / "WushenForge" / relative
            _assert(source.is_file(), f"C111B {phase} capture was not written")
            target = CAPTURE_ROOT / phase / f"{capture['view_id']}.png"
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
            data = target.read_bytes()
            _assert(hashlib.sha256(data).hexdigest() == capture["sha256"], f"C111B {phase} capture hash drifted")
            _assert(len(data) == capture["byte_size"], f"C111B {phase} capture byte size drifted")
            copied[phase][str(capture["view_id"])] = {**capture, "path": str(target.relative_to(ROOT))}
    return copied


def _write_evidence(value: dict[str, object]) -> str:
    EVIDENCE.parent.mkdir(parents=True, exist_ok=True)
    temporary = EVIDENCE.with_suffix(".tmp")
    payload = (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode("utf-8")
    temporary.write_bytes(payload)
    temporary.replace(EVIDENCE)
    return hashlib.sha256(payload).hexdigest()


def _write_attempt(
    status: str,
    error_code: str | None = None,
    evidence_sha256: str | None = None,
    export_sha256: str | None = None,
) -> None:
    value: dict[str, object] = {
        "schema_version": "C111BPackagedWebGLAttempt@2",
        "run_id": RUN_ID,
        "mode": MODE,
        "status": status,
        "ok": status == "passed",
        "phase": CURRENT_PHASE,
        "started_at": RUN_STARTED_AT,
        "elapsed_ms": round((time.monotonic() - RUN_STARTED_MONOTONIC) * 1000),
        "source_sha256": SOURCE_SHA256,
        "formal_eligible": False,
        "reference_comparison": False,
        "human_benchmark_evidence": False,
    }
    if error_code is not None:
        value["error_code"] = error_code
    if status != "running":
        value["finished_at"] = datetime.now(timezone.utc).isoformat()
    if evidence_sha256 is not None:
        value["evidence_sha256"] = evidence_sha256
    if export_sha256 is not None:
        _assert(_is_sha256(export_sha256), "C111B attempt export SHA is invalid")
        value["export_sha256"] = export_sha256
    if status == "failed":
        progress = _last_progress()
        if progress is not None:
            value["last_progress"] = progress
    ATTEMPT.parent.mkdir(parents=True, exist_ok=True)
    temporary = ATTEMPT.with_suffix(".tmp")
    temporary.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(ATTEMPT)


def _last_progress() -> str | None:
    if CURRENT_PHASE == "preflight":
        return None
    if not FAILURE_LOG.is_file():
        return None
    progress = None
    for line in FAILURE_LOG.read_text(encoding="utf-8", errors="replace").splitlines():
        if line.startswith(PROGRESS_MARKER):
            progress = line[len(PROGRESS_MARKER):].strip()
    return progress


def _failure_code(exc: BaseException) -> str:
    message = str(exc)
    if message.startswith("C111B_") and all(character.isupper() or character.isdigit() or character == "_" for character in message):
        return message[:160]
    return "C111B_PACKAGED_QA_FAILED"


def _macos_console_screen_locked() -> bool:
    try:
        completed = subprocess.run(["ioreg", "-n", "Root", "-d1", "-a"], capture_output=True, check=True, timeout=10)
        roots = plistlib.loads(completed.stdout)
    except (OSError, subprocess.SubprocessError, plistlib.InvalidFileException) as exc:
        raise AssertionError("C111B_PACKAGED_SCREEN_STATE_UNAVAILABLE") from exc
    root = roots[0] if isinstance(roots, list) and roots else roots if isinstance(roots, dict) else {}
    for user in root.get("IOConsoleUsers", []) if isinstance(root, dict) else []:
        if isinstance(user, dict) and user.get("kCGSSessionOnConsoleKey") is True:
            return user.get("CGSSessionScreenIsLocked") is True
    raise AssertionError("C111B_PACKAGED_CONSOLE_SESSION_UNAVAILABLE")


if __name__ == "__main__":
    raise SystemExit(main())
