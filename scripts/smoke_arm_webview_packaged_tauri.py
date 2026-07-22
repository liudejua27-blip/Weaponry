#!/usr/bin/env python3
"""Exact-lineage mechanical-arm QA through the real packaged WebView.

Unlike the Rust-only MVP protocol proof and the V003 fixture browser test,
this launches the macOS bundle with its actual rendered workbench.  The
opt-in WebView module types the brief, confirms the one result, applies and
retains A005, saves a browser-generated reference PNG, previews and retains
R007B as V3, then downloads V3 through the visible export drawer. It reports
only sealed DOM/renderer lineage to the production Tauri command. No
Accessibility API, fixture, proxy, second renderer, Provider credential or
network request is involved.
"""

from __future__ import annotations

import json
import hashlib
import os
import plistlib
import re
import shutil
import struct
import subprocess
import tempfile
import time
from pathlib import Path

from smoke_packaged_tauri_alpha import (
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
MARKER = "ForgeCAD mechanical-arm packaged WebView QA report="
PROGRESS_MARKER = "ForgeCAD mechanical-arm packaged WebView QA progress="
SCHEMA = "ForgeCADArmWebViewQa@1"
STABLE = re.compile(r"^[A-Za-z0-9_.-]{1,160}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
EVIDENCE_PATH = ROOT / "output" / "arm-mvp-golden-path" / "packaged-webview-qa.json"
CAPTURE_PATH = EVIDENCE_PATH.parent / "packaged-webview-captures"
PRODUCTION_TRIANGLE_MIN = 12_000
PRODUCTION_TRIANGLE_MAX = 24_000
# The initial run deliberately executes more than thirty visible UI actions.
# Each WebView wait remains independently bounded at 180 seconds; this outer
# observer must cover the whole sequence instead of expiring as V3 becomes
# ready on a background-throttled macOS session.
REPORT_TIMEOUT_SECONDS = 600.0


def main() -> int:
    _assert(APP_BINARY.is_file(), "build the macOS .app before WebView QA")
    _assert(_listener_pid() is None, "port 8000 must be free before WebView QA")
    _assert(
        not _macos_console_screen_locked(),
        "macOS console must be unlocked for real WebGL/PBR screenshot evidence",
    )
    with tempfile.TemporaryDirectory(prefix="forgecad_arm_webview_qa_") as raw:
        temporary = Path(raw)
        try:
            report = _run(temporary)
        except BaseException:
            # Preserve the bounded, already-redacted native log before the
            # isolated HOME is deleted.  This is diagnostic evidence only and
            # cannot turn a failed WebView run into a passing report.
            source = temporary / "WushenForge" / "agent.log"
            if source.is_file():
                destination = EVIDENCE_PATH.parent / "packaged-webview-qa-failure.log"
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(source, destination)
            raise
    _write_evidence(report)
    print(json.dumps(report, ensure_ascii=False, separators=(",", ":")))
    return 0


def _macos_console_screen_locked() -> bool:
    """Fail closed when WebKit cannot produce visible screenshot evidence."""
    try:
        completed = subprocess.run(
            ["ioreg", "-n", "Root", "-d1", "-a"],
            capture_output=True,
            check=True,
            timeout=10,
        )
        roots = plistlib.loads(completed.stdout)
    except (OSError, subprocess.SubprocessError, plistlib.InvalidFileException) as exc:
        raise AssertionError("macOS console lock state could not be verified") from exc
    root = (
        roots[0]
        if isinstance(roots, list) and roots
        else roots
        if isinstance(roots, dict)
        else {}
    )
    users = root.get("IOConsoleUsers", [])
    for user in users if isinstance(users, list) else []:
        if not isinstance(user, dict) or user.get("kCGSSessionOnConsoleKey") is not True:
            continue
        return user.get("CGSSessionScreenIsLocked") is True
    raise AssertionError("active macOS console session could not be verified")


def _write_evidence(report: dict[str, object]) -> None:
    """Persist only the sealed, non-secret terminal report for the aggregate MVP gate."""
    EVIDENCE_PATH.parent.mkdir(parents=True, exist_ok=True)
    temporary = EVIDENCE_PATH.with_suffix(".tmp")
    temporary.write_text(
        json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    temporary.replace(EVIDENCE_PATH)


def _run(temporary: Path) -> dict[str, object]:
    library = temporary / "library"
    log = temporary / "WushenForge" / "agent.log"
    environment = _environment(temporary, library)
    first = _start(temporary, environment)
    try:
        listener = _wait_for_native_health(first)
        _assert(_is_descendant(listener, first), "packaged sidecar is not desktop-owned")
        initial = _wait_report(log, "initial")
        _validate_initial(initial)
        captures = _preserve_initial_captures(temporary, initial)
    finally:
        _stop_desktop_and_listener(first)
    _assert(_listener_pid() is None, "sidecar survived initial WebView QA shutdown")

    environment.update({
        "FORGECAD_ARM_WEBVIEW_QA_PHASE": "restart",
        "FORGECAD_ARM_WEBVIEW_QA_EXPECT_PROJECT_ID": str(initial["project_id"]),
        "FORGECAD_ARM_WEBVIEW_QA_EXPECT_V3_ASSET_VERSION_ID": str(initial["v3_asset_version_id"]),
        "FORGECAD_ARM_WEBVIEW_QA_EXPECT_SNAPSHOT_REVISION": str(initial["snapshot_revision"]),
    })
    restarted = _start(temporary, environment)
    try:
        listener = _wait_for_native_health(restarted)
        _assert(_is_descendant(listener, restarted), "restarted sidecar is not desktop-owned")
        restart = _wait_report(log, "restart")
        _validate_restart(restart, initial)
    finally:
        _stop_desktop_and_listener(restarted)
    _assert(_listener_pid() is None, "sidecar survived restart WebView QA shutdown")
    return {
        "schema_version": SCHEMA,
        "ok": True,
        "real_packaged_webview": True,
        "fixture_or_proxy_used": False,
        "accessibility_api_used": False,
        "provider_network_calls": 0,
        "credential_reads": 0,
        "single_renderer": True,
        "c106_single_result_confirmed": True,
        "a005_v2_confirmed": True,
        "r007b_preview_seen": True,
        "r007b_v3_confirmed": True,
        "v3_glb_download_confirmed": True,
        "visual_fidelity_validated": False,
        "snapshot_restart_restored": True,
        "captures": captures,
        "initial": initial,
        "restart": restart,
    }


def _environment(temporary: Path, library: Path) -> dict[str, str]:
    environment = os.environ.copy()
    for name in tuple(environment):
        if name.startswith("FORGECAD_ARM_WEBVIEW_QA") or name in {
            "FORGECAD_AGENT_API_KEY", "FORGECAD_AGENT_BASE_URL", "FORGECAD_AGENT_MODEL",
            "FORGECAD_K001_PACKAGED_PROBE", "FORGECAD_K002_PACKAGED_PROBE",
        }:
            environment.pop(name, None)
    environment.update({
        "HOME": str(temporary),
        "WUSHEN_LIBRARY_ROOT": str(library),
        "WUSHEN_AGENT_RUNTIME_MODE": "packaged-sidecar",
        "FORGECAD_DISABLE_PROVIDER_CONFIG": "1",
        "FORGECAD_MVP_OFFLINE_ARM": "1",
        "FORGECAD_CONCEPT_WORKER_ENABLED": "0",
        "WUSHEN_LOCAL_WORKER_ENABLED": "0",
        "FORGECAD_ARM_WEBVIEW_QA": "1",
        "FORGECAD_ARM_WEBVIEW_QA_PHASE": "initial",
    })
    return environment


def _start(temporary: Path, environment: dict[str, str]) -> int:
    existing = _desktop_pids()
    names = {
        "HOME", "WUSHEN_LIBRARY_ROOT", "WUSHEN_AGENT_RUNTIME_MODE",
        "FORGECAD_DISABLE_PROVIDER_CONFIG", "FORGECAD_MVP_OFFLINE_ARM",
        "FORGECAD_CONCEPT_WORKER_ENABLED", "WUSHEN_LOCAL_WORKER_ENABLED",
        "FORGECAD_ARM_WEBVIEW_QA", "FORGECAD_ARM_WEBVIEW_QA_PHASE",
        "FORGECAD_ARM_WEBVIEW_QA_EXPECT_PROJECT_ID",
        "FORGECAD_ARM_WEBVIEW_QA_EXPECT_V3_ASSET_VERSION_ID",
        "FORGECAD_ARM_WEBVIEW_QA_EXPECT_SNAPSHOT_REVISION",
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
    raise AssertionError("LaunchServices did not start packaged mechanical-arm WebView QA")


def _wait_report(path: Path, phase: str) -> dict[str, object]:
    # The WebView's longest bounded UI wait is 180 seconds and starts only
    # after LaunchServices, the sidecar handshake and React hydration. Keep
    # the outer observer wider so it can receive the probe's fail-closed
    # report instead of racing that inner deadline.
    deadline = time.monotonic() + REPORT_TIMEOUT_SECONDS
    while time.monotonic() < deadline:
        if path.is_file():
            reports = []
            for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
                if not line.startswith(MARKER):
                    continue
                try:
                    value = json.loads(line[len(MARKER):])
                except ValueError:
                    continue
                if isinstance(value, dict) and value.get("phase") == phase:
                    reports.append(value)
            if reports:
                report = reports[-1]
                _assert(report.get("schema_version") == SCHEMA, "WebView QA schema drifted")
                _assert(report.get("ok") is True, f"WebView QA {phase} failed: {report.get('error_code')}")
                return report
        time.sleep(0.1)
    last_progress = "none"
    if path.is_file():
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith(PROGRESS_MARKER):
                candidate = line[len(PROGRESS_MARKER):].strip()
                if STABLE.fullmatch(candidate) is not None:
                    last_progress = candidate
    raise AssertionError(
        f"WebView QA {phase} did not finish within {REPORT_TIMEOUT_SECONDS:.0f} seconds; "
        f"last_progress={last_progress}"
    )


def _validate_common(report: dict[str, object]) -> None:
    _assert(STABLE.fullmatch(str(report.get("project_id", ""))) is not None, "invalid project ID")
    _assert(STABLE.fullmatch(str(report.get("v3_asset_version_id", ""))) is not None, "invalid V3 ID")
    _assert(type(report.get("snapshot_revision")) is int and report["snapshot_revision"] > 0, "invalid Snapshot revision")
    _assert(type(report.get("renderer_generation")) is int and report["renderer_generation"] > 0, "renderer was not created")
    _assert(report.get("active_webgl_contexts") == 1, "WebView created more than one WebGL renderer")
    _assert(report.get("production_glb_render_source") == "glb_pbr", "viewport did not display production GLB PBR")


def _validate_initial(report: dict[str, object]) -> None:
    _validate_common(report)
    _assert(report.get("phase") == "initial", "wrong initial QA phase")
    for name in ("turn_id", "preview_id", "v1_asset_version_id", "v2_asset_version_id"):
        _assert(STABLE.fullmatch(str(report.get(name, ""))) is not None, f"invalid {name}")
    _assert(SHA256.fullmatch(str(report.get("preview_artifact_sha256", ""))) is not None, "invalid preview hash")
    _assert(report["v1_asset_version_id"] != report["v2_asset_version_id"], "A005 did not create V2")
    _assert(report["v2_asset_version_id"] != report["v3_asset_version_id"], "R007B did not create V3")
    _assert(report.get("a005_preview_seen") is True, "A005 preview was not displayed")
    _assert(report.get("r007b_preview_seen") is True, "R007B preview was not displayed")
    _assert(report.get("r007b_v3_confirmed") is True, "R007B did not confirm V3")
    _assert(report.get("v3_glb_download_confirmed") is True, "visible V3 GLB download was not confirmed")
    _assert(report.get("visual_fidelity_validated") is False, "engineering QA must not claim visual fidelity approval")
    _validate_initial_captures(report)
    _assert(report.get("restart_hydrated") is False, "initial run claimed restart")


def _validate_restart(report: dict[str, object], initial: dict[str, object]) -> None:
    _validate_common(report)
    _assert(report.get("phase") == "restart", "wrong restart QA phase")
    _assert(report.get("project_id") == initial.get("project_id"), "restart project drifted")
    _assert(report.get("v3_asset_version_id") == initial.get("v3_asset_version_id"), "restart V3 drifted")
    _assert(report.get("snapshot_revision") == initial.get("snapshot_revision"), "restart Snapshot drifted")
    _assert(report.get("a005_preview_seen") is False, "restart replayed A005 edit")
    _assert(report.get("r007b_preview_seen") is False, "restart replayed R007B preview")
    _assert(report.get("r007b_v3_confirmed") is False, "restart replayed R007B confirmation")
    _assert(report.get("v3_glb_download_confirmed") is False, "restart replayed V3 download")
    _assert("v3_production_glb" not in report and "v3_viewport_screenshot" not in report, "restart replayed initial QA captures")
    _assert("visual_fidelity_validated" not in report, "restart replayed visual-fidelity status")
    _assert(report.get("restart_hydrated") is True, "restart hydration not observed")


def _validate_initial_captures(report: dict[str, object]) -> None:
    glb = report.get("v3_production_glb")
    screenshot = report.get("v3_viewport_screenshot")
    _assert(isinstance(glb, dict), "V3 production GLB capture is missing")
    _assert(isinstance(screenshot, dict), "V3 viewport screenshot capture is missing")
    _assert(glb.get("relative_path") == "qa-artifacts/arm-webview/initial/v3_production_glb.glb", "unexpected V3 GLB capture path")
    _assert(screenshot.get("relative_path") == "qa-artifacts/arm-webview/initial/v3_viewport_png.png", "unexpected V3 screenshot capture path")
    _assert(SHA256.fullmatch(str(glb.get("sha256", ""))) is not None, "invalid V3 GLB capture hash")
    _assert(SHA256.fullmatch(str(screenshot.get("sha256", ""))) is not None, "invalid V3 screenshot capture hash")
    _assert(isinstance(glb.get("byte_size"), int) and glb["byte_size"] > 0, "invalid V3 GLB capture bytes")
    _assert(isinstance(screenshot.get("byte_size"), int) and screenshot["byte_size"] > 0, "invalid V3 screenshot capture bytes")
    _assert(isinstance(glb.get("triangle_count"), int) and PRODUCTION_TRIANGLE_MIN <= glb["triangle_count"] <= PRODUCTION_TRIANGLE_MAX, "V3 GLB is outside the production triangle envelope")
    _assert(isinstance(glb.get("complete_pbr_material_count"), int) and glb["complete_pbr_material_count"] > 0, "V3 GLB readback has no complete PBR material")
    _assert(isinstance(screenshot.get("width"), int) and screenshot["width"] >= 320, "V3 screenshot width is too small")
    _assert(isinstance(screenshot.get("height"), int) and screenshot["height"] >= 240, "V3 screenshot height is too small")


def _preserve_initial_captures(temporary: Path, report: dict[str, object]) -> dict[str, dict[str, object]]:
    """Copy Rust-written QA artifacts before the temporary HOME is removed.

    The browser cannot choose an output path; it reports only Rust's fixed
    relative receipt. This verifier copies that receipt into the repository
    evidence directory, rechecks hash/format, and reports only a repository-
    relative path. A dev-shell fixture cannot satisfy this chain.
    """
    _validate_initial_captures(report)
    raw = {
        "v3_production_glb": report["v3_production_glb"],
        "v3_viewport_screenshot": report["v3_viewport_screenshot"],
    }
    copied: dict[str, dict[str, object]] = {}
    for name, receipt in raw.items():
        _assert(isinstance(receipt, dict), f"invalid {name} receipt")
        relative = str(receipt["relative_path"])
        source = temporary / "WushenForge" / relative
        _assert(source.is_file(), f"Rust QA {name} file was not written")
        suffix = ".glb" if name == "v3_production_glb" else ".png"
        target = CAPTURE_PATH / f"{name}{suffix}"
        target.parent.mkdir(parents=True, exist_ok=True)
        temporary_target = target.with_suffix(f"{suffix}.tmp")
        shutil.copyfile(source, temporary_target)
        temporary_target.replace(target)
        bytes_ = target.read_bytes()
        _assert(len(bytes_) == receipt["byte_size"], f"{name} byte size drifted after copy")
        _assert(hashlib.sha256(bytes_).hexdigest() == receipt["sha256"], f"{name} hash drifted after copy")
        if name == "v3_production_glb":
            _assert(len(bytes_) >= 20 and bytes_[:4] == b"glTF", "captured V3 payload is not a GLB")
            _assert(struct.unpack_from("<I", bytes_, 8)[0] == len(bytes_), "captured V3 GLB total length drifted")
        else:
            _assert(bytes_[:8] == b"\x89PNG\r\n\x1a\n", "captured V3 viewport frame is not a PNG")
            width, height = _png_dimensions(bytes_)
            _assert((width, height) == (receipt["width"], receipt["height"]), "captured V3 screenshot dimensions drifted")
        copied[name] = {
            **receipt,
            "path": str(target.relative_to(ROOT)),
        }
    return copied


def _png_dimensions(value: bytes) -> tuple[int, int]:
    _assert(len(value) >= 24 and value[:8] == b"\x89PNG\r\n\x1a\n" and value[12:16] == b"IHDR", "PNG IHDR is invalid")
    return struct.unpack_from(">II", value, 16)


if __name__ == "__main__":
    raise SystemExit(main())
