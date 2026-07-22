#!/usr/bin/env python3
"""Verify the packaged K003 Rust product-state cutover without a Provider.

The real smoke launches the macOS ``.app`` twice with one isolated Library.
It deliberately leaves every Provider unconfigured and never enables either
legacy Python oracle.  A successful run therefore requires an opt-in packaged
probe implemented on the Rust production path:

``NativeProductToolExecutor -> RestrictedGeometryExecutor -> RustCoreRuntime``

The probe must create and commit one non-functional concept asset on the first
launch and only recover it on the second.  This script independently inspects
SQLite and the content-addressed object library, authenticates the restricted
Python geometry facet, proves old Python product routes are tombstoned, and
compares semantic hashes across the two launches. The same two launches also
run the migrated K001 native Thread/Turn + editable-GLB probe and the K002
native lifecycle probe, preserving their historical acceptance semantics
without starting a Python product or lifecycle writer.

The packaged app implements this opt-in probe on its production Rust path.
``--self-test`` exercises the local validators without launching the app or
claiming that the real packaged K003 gate passed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
from collections.abc import Iterable, Mapping
from pathlib import Path
from typing import Any

from smoke_k002_packaged_tauri_native import (
    PROBE_MARKER as K002_PROBE_MARKER,
    PROBE_SCHEMA as K002_PROBE_SCHEMA,
    _validate_probe_report as validate_k002_probe_report,
)
from smoke_packaged_sidecar_alpha import _assert
from smoke_packaged_tauri_alpha import (
    APP_BINARY,
    APP_BUNDLE,
    K001_PROBE_MARKER,
    _desktop_pids,
    _is_descendant,
    _listener_pid,
    _stop_desktop_and_listener,
    _validate_k001_probe_report,
    _wait_for_log_count,
)


ROOT = Path(__file__).resolve().parents[1]
HEALTH_URL = "http://127.0.0.1:8000/api/health"
OWNERSHIP_URL = (
    "http://127.0.0.1:8000/api/v1/internal/geometry/capability/ownership"
)
RESTRICTED_CAPABILITY_ENV = "FORGECAD_RESTRICTED_GEOMETRY_CAPABILITY_TOKEN"
RESTRICTED_CAPABILITY_HEADER = "X-ForgeCAD-Restricted-Geometry-Capability"
K003_PROBE_MARKER = "ForgeCAD K003 packaged Rust core probe report="
K003_PROBE_SCHEMA = "ForgeCADK003PackagedCoreProbe@1"
K001_PROBE_SCHEMA = "ForgeCADK001PackagedProbe@1"
K003_EXECUTION_PATH = (
    "compat_project_create>compat_blockout_build>compat_blockout_segment>"
    "compat_blockout_commit>compat_snapshot_quality_glb_readback>"
    "compat_render_package_readback"
)
APP_SERVER_READY_PREFIX = (
    "ForgeCAD app-server ready protocol=forgecad.app-server/1 "
    "lifecycle_owner=rust-app-server"
)
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
STABLE_ID_PATTERN = re.compile(r"^[A-Za-z0-9_.:-]{1,160}$")
ARTIFACT_DIRECTORY = os.environ.get(
    "FORGECAD_K003_PACKAGED_SMOKE_ARTIFACT_DIR",
    os.environ.get("FORGECAD_NATIVE_SMOKE_ARTIFACT_DIR", ""),
)

PROVIDER_ENVIRONMENT_KEYS = (
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
    "WUSHEN_LLM_PROVIDER",
    "WUSHEN_LLM_BASE_URL",
    "WUSHEN_LLM_MODEL",
    "WUSHEN_LLM_API_KEY",
    "WUSHEN_LLM_API_KEY_FILE",
    "OPENAI_API_KEY",
    "DEEPSEEK_API_KEY",
    "ANTHROPIC_API_KEY",
)
FORBIDDEN_PYTHON_STATE_KEYS = (
    "WUSHEN_LIBRARY_ROOT",
    "WUSHEN_MIGRATIONS_DIR",
    "FORGECAD_TEST_ONLY_LEGACY_AGENT_LIFECYCLE",
    "FORGECAD_TEST_ONLY_LEGACY_PRODUCT_CORE",
    "FORGECAD_K001_PACKAGED_PROBE",
    "FORGECAD_K001_PACKAGED_PROBE_PHASE",
    "FORGECAD_K001_EXPECT_PROJECT_ID",
    "FORGECAD_K001_EXPECT_THREAD_ID",
    "FORGECAD_K001_EXPECT_ASSET_VERSION_ID",
    "FORGECAD_K001_EXPECT_LAST_EVENT_ID",
    "FORGECAD_K001_EXPECT_CURSOR",
    "FORGECAD_K001_EXPECT_GLB_SHA256",
    "FORGECAD_K002_PACKAGED_PROBE",
    "FORGECAD_K002_PACKAGED_PROBE_PHASE",
    "FORGECAD_K002_EXPECT_THREAD_ID",
    "FORGECAD_K002_EXPECT_TURN_ID",
    "FORGECAD_K002_EXPECT_ITEMS_SHA256",
    "FORGECAD_K002_EXPECT_ITEM_COUNT",
    "FORGECAD_K002_EXPECT_LAST_SEQUENCE",
    "FORGECAD_K002_EXPECT_TURN_ERROR_CODE",
    "FORGECAD_K003_PACKAGED_PROBE",
    "FORGECAD_K003_PACKAGED_PROBE_PHASE",
    "FORGECAD_K003_EXPECT_PROJECT_ID",
    "FORGECAD_K003_EXPECT_ASSET_VERSION_ID",
    "FORGECAD_K003_EXPECT_SNAPSHOT_ETAG",
    "FORGECAD_K003_EXPECT_PROJECT_SHA256",
    "FORGECAD_K003_EXPECT_SNAPSHOT_SHA256",
    "FORGECAD_K003_EXPECT_GLB_SHA256",
    "FORGECAD_K003_EXPECT_RENDER_SET_SHA256",
    "FORGECAD_K003_EXPECT_RENDER_PACKAGE_SHA256",
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="exercise validators without launching an app or claiming K003 passed",
    )
    arguments = parser.parse_args()
    if arguments.self_test:
        report = _run_self_test()
    else:
        _assert(APP_BINARY.is_file(), "build the macOS .app before the K003 packaged smoke")
        _assert(_listener_pid() is None, "port 8000 must be free before the K003 packaged smoke")
        with tempfile.TemporaryDirectory(prefix="forgecad_k003_packaged_") as temporary:
            temporary_path = Path(temporary)
            log_path = temporary_path / "WushenForge/agent.log"
            try:
                report = _run_native_smoke(temporary_path)
            except Exception as error:
                _write_artifact(
                    {
                        "schema_version": "ForgeCADK003PackagedSmoke@1",
                        "ok": False,
                        "provider_calls": 0,
                        "error": _safe_diagnostic(str(error), temporary_path),
                    },
                    log_path=log_path,
                    temporary_path=temporary_path,
                )
                raise
    _write_artifact(report)
    print(json.dumps(report, ensure_ascii=False, separators=(",", ":")))
    return 0


def _run_native_smoke(temporary_path: Path) -> dict[str, object]:
    library_root = temporary_path / "library"
    log_path = temporary_path / "WushenForge/agent.log"
    environment = _safe_environment(temporary_path, library_root)

    initial_desktop = _start_desktop(temporary_path, environment)
    try:
        initial_listener = _wait_for_restricted_health(initial_desktop)
        _assert(
            _is_descendant(initial_listener, initial_desktop),
            "restricted geometry listener is not owned by the packaged desktop",
        )
        _wait_for_log_count(log_path, APP_SERVER_READY_PREFIX, 1)
        _assert_k003_ready_log(log_path, 1)
        _verify_python_boundary(initial_listener, library_root, log_path)
        _verify_restricted_ownership_and_tombstones(initial_listener)
        initial_k001 = _wait_for_report(
            log_path,
            K001_PROBE_MARKER,
            K001_PROBE_SCHEMA,
            "initial",
        )
        _validate_k001_probe_report(initial_k001, "initial")
        initial_k002 = _wait_for_report(
            log_path,
            K002_PROBE_MARKER,
            K002_PROBE_SCHEMA,
            "initial",
        )
        validate_k002_probe_report(initial_k002, "initial")
        initial_k003 = _wait_for_report(
            log_path,
            K003_PROBE_MARKER,
            K003_PROBE_SCHEMA,
            "initial",
        )
        _validate_k003_probe_report(initial_k003, "initial")
        _assert((library_root / "library.db").is_file(), "Rust core did not create library.db")
    finally:
        _stop_desktop_and_listener(initial_desktop)

    _assert(_listener_pid() is None, "sidecar survived K003 first-launch cleanup")
    initial_k001_facts = _read_library_facts(
        library_root,
        project_id=_required_id(initial_k001, "project_id"),
        thread_id=_required_id(initial_k001, "thread_id"),
        asset_version_id=_required_id(initial_k001, "asset_version_id"),
        glb_sha256=_required_sha(initial_k001, "glb_sha256"),
    )
    initial_facts = _read_library_facts(
        library_root,
        project_id=_required_id(initial_k003, "project_id"),
        thread_id=_required_id(initial_k002, "thread_id"),
        asset_version_id=_required_id(initial_k003, "asset_version_id"),
        glb_sha256=_required_sha(initial_k003, "glb_sha256"),
    )

    _configure_restart(environment, initial_k001, initial_k002, initial_k003)
    restarted_desktop = _start_desktop(temporary_path, environment)
    try:
        restart_listener = _wait_for_restricted_health(restarted_desktop)
        _assert(
            _is_descendant(restart_listener, restarted_desktop),
            "restarted restricted geometry listener is not desktop-owned",
        )
        _wait_for_log_count(log_path, APP_SERVER_READY_PREFIX, 2)
        _assert_k003_ready_log(log_path, 2)
        _verify_python_boundary(restart_listener, library_root, log_path)
        _verify_restricted_ownership_and_tombstones(restart_listener)
        restart_k001 = _wait_for_report(
            log_path,
            K001_PROBE_MARKER,
            K001_PROBE_SCHEMA,
            "restart",
        )
        _validate_k001_probe_report(restart_k001, "restart")
        restart_k002 = _wait_for_report(
            log_path,
            K002_PROBE_MARKER,
            K002_PROBE_SCHEMA,
            "restart",
        )
        validate_k002_probe_report(restart_k002, "restart")
        restart_k003 = _wait_for_report(
            log_path,
            K003_PROBE_MARKER,
            K003_PROBE_SCHEMA,
            "restart",
        )
        _validate_k003_probe_report(restart_k003, "restart")
    finally:
        _stop_desktop_and_listener(restarted_desktop)

    _assert(_listener_pid() is None, "sidecar survived K003 restart cleanup")
    restart_k001_facts = _read_library_facts(
        library_root,
        project_id=_required_id(restart_k001, "project_id"),
        thread_id=_required_id(restart_k001, "thread_id"),
        asset_version_id=_required_id(restart_k001, "asset_version_id"),
        glb_sha256=_required_sha(restart_k001, "glb_sha256"),
    )
    restart_facts = _read_library_facts(
        library_root,
        project_id=_required_id(restart_k003, "project_id"),
        thread_id=_required_id(restart_k002, "thread_id"),
        asset_version_id=_required_id(restart_k003, "asset_version_id"),
        glb_sha256=_required_sha(restart_k003, "glb_sha256"),
    )
    _assert_restart_reports(
        initial_k001,
        restart_k001,
        initial_k002,
        restart_k002,
        initial_k003,
        restart_k003,
    )
    _assert_semantic_recovery(initial_k001_facts, restart_k001_facts)
    _assert_semantic_recovery(initial_facts, restart_facts)

    return {
        "schema_version": "ForgeCADK003PackagedSmoke@1",
        "ok": True,
        "supervisor_mode": "packaged-sidecar",
        "k001_native_lifecycle_transport": True,
        "k001_native_item_replay_verified": True,
        "k001_rust_product_state_verified": True,
        "k002_native_lifecycle_verified": True,
        "rust_core_state_owner": True,
        "rust_thread_turn_item_owner": True,
        "restricted_geometry_ownership_verified": True,
        "python_product_routes_status": 410,
        "python_lifecycle_routes_status": 410,
        "python_database_path_present": False,
        "python_object_path_present": False,
        "python_provider_path_present": False,
        "provider_status": "unconfigured",
        "provider_network_call_made": False,
        "provider_calls": 0,
        "restart_semantic_hashes_consistent": True,
        "k001_project_semantic_sha256": initial_k001_facts[
            "project_semantic_sha256"
        ],
        "k001_thread_semantic_sha256": initial_k001_facts[
            "thread_semantic_sha256"
        ],
        "k001_snapshot_semantic_sha256": initial_k001_facts[
            "snapshot_semantic_sha256"
        ],
        "k001_asset_version_semantic_sha256": initial_k001_facts[
            "asset_version_semantic_sha256"
        ],
        "k001_glb_sha256": initial_k001_facts["glb_sha256"],
        "project_semantic_sha256": initial_facts["project_semantic_sha256"],
        "thread_semantic_sha256": initial_facts["thread_semantic_sha256"],
        "snapshot_semantic_sha256": initial_facts["snapshot_semantic_sha256"],
        "asset_version_semantic_sha256": initial_facts[
            "asset_version_semantic_sha256"
        ],
        "glb_sha256": initial_facts["glb_sha256"],
        "glb_byte_size": initial_facts["glb_byte_size"],
        "render_set_sha256": _required_sha(initial_k003, "render_set_sha256"),
        "render_package_sha256": _required_sha(
            initial_k003, "render_package_sha256"
        ),
        "rust_writer_epoch_advanced": True,
        "execution_path": K003_EXECUTION_PATH,
    }


def _safe_environment(temporary: Path, library_root: Path) -> dict[str, str]:
    environment = os.environ.copy()
    for name in (
        *PROVIDER_ENVIRONMENT_KEYS,
        *FORBIDDEN_PYTHON_STATE_KEYS,
    ):
        environment.pop(name, None)
    environment.update(
        {
            "HOME": str(temporary),
            "WUSHEN_LIBRARY_ROOT": str(library_root),
            "WUSHEN_AGENT_RUNTIME_MODE": "packaged-sidecar",
            "FORGECAD_DISABLE_PROVIDER_CONFIG": "1",
            "FORGECAD_CONCEPT_WORKER_ENABLED": "0",
            "WUSHEN_LOCAL_WORKER_ENABLED": "0",
            "FORGECAD_K001_PACKAGED_PROBE": "1",
            "FORGECAD_K001_PACKAGED_PROBE_PHASE": "initial",
            "FORGECAD_K002_PACKAGED_PROBE": "1",
            "FORGECAD_K002_PACKAGED_PROBE_PHASE": "initial",
            "FORGECAD_K003_PACKAGED_PROBE": "1",
            "FORGECAD_K003_PACKAGED_PROBE_PHASE": "initial",
        }
    )
    return environment


def _configure_restart(
    environment: dict[str, str],
    k001: Mapping[str, object],
    k002: Mapping[str, object],
    k003: Mapping[str, object],
) -> None:
    environment.update(
        {
            "FORGECAD_K001_PACKAGED_PROBE_PHASE": "restart",
            "FORGECAD_K001_EXPECT_PROJECT_ID": _required_id(k001, "project_id"),
            "FORGECAD_K001_EXPECT_THREAD_ID": _required_id(k001, "thread_id"),
            "FORGECAD_K001_EXPECT_ASSET_VERSION_ID": _required_id(
                k001, "asset_version_id"
            ),
            "FORGECAD_K001_EXPECT_LAST_EVENT_ID": _required_id(
                k001, "last_event_id"
            ),
            "FORGECAD_K001_EXPECT_CURSOR": _required_text(k001, "cursor", 2048),
            "FORGECAD_K001_EXPECT_GLB_SHA256": _required_sha(k001, "glb_sha256"),
            "FORGECAD_K002_PACKAGED_PROBE_PHASE": "restart",
            "FORGECAD_K002_EXPECT_THREAD_ID": _required_id(k002, "thread_id"),
            "FORGECAD_K002_EXPECT_TURN_ID": _required_id(k002, "turn_id"),
            "FORGECAD_K002_EXPECT_ITEMS_SHA256": _required_sha(k002, "items_sha256"),
            "FORGECAD_K002_EXPECT_ITEM_COUNT": str(_required_int(k002, "item_count")),
            "FORGECAD_K002_EXPECT_LAST_SEQUENCE": str(
                _required_int(k002, "last_sequence")
            ),
            "FORGECAD_K002_EXPECT_TURN_ERROR_CODE": _required_id(
                k002, "turn_error_code"
            ),
            "FORGECAD_K003_PACKAGED_PROBE_PHASE": "restart",
            "FORGECAD_K003_EXPECT_PROJECT_ID": _required_id(k003, "project_id"),
            "FORGECAD_K003_EXPECT_ASSET_VERSION_ID": _required_id(
                k003, "asset_version_id"
            ),
            "FORGECAD_K003_EXPECT_SNAPSHOT_ETAG": _required_text(
                k003, "snapshot_etag", 256
            ),
            "FORGECAD_K003_EXPECT_PROJECT_SHA256": _required_sha(
                k003, "project_semantic_sha256"
            ),
            "FORGECAD_K003_EXPECT_SNAPSHOT_SHA256": _required_sha(
                k003, "snapshot_semantic_sha256"
            ),
            "FORGECAD_K003_EXPECT_GLB_SHA256": _required_sha(k003, "glb_sha256"),
            "FORGECAD_K003_EXPECT_RENDER_SET_SHA256": _required_sha(
                k003, "render_set_sha256"
            ),
            "FORGECAD_K003_EXPECT_RENDER_PACKAGE_SHA256": _required_sha(
                k003, "render_package_sha256"
            ),
        }
    )


def _start_desktop(temporary: Path, environment: Mapping[str, str]) -> int:
    existing = _desktop_pids()
    forwarded_names = {
        "HOME",
        "WUSHEN_LIBRARY_ROOT",
        "WUSHEN_AGENT_RUNTIME_MODE",
        "FORGECAD_DISABLE_PROVIDER_CONFIG",
        "FORGECAD_CONCEPT_WORKER_ENABLED",
        "WUSHEN_LOCAL_WORKER_ENABLED",
        "FORGECAD_K001_PACKAGED_PROBE",
        "FORGECAD_K001_PACKAGED_PROBE_PHASE",
        "FORGECAD_K001_EXPECT_PROJECT_ID",
        "FORGECAD_K001_EXPECT_THREAD_ID",
        "FORGECAD_K001_EXPECT_ASSET_VERSION_ID",
        "FORGECAD_K001_EXPECT_LAST_EVENT_ID",
        "FORGECAD_K001_EXPECT_CURSOR",
        "FORGECAD_K001_EXPECT_GLB_SHA256",
        "FORGECAD_K002_PACKAGED_PROBE",
        "FORGECAD_K002_PACKAGED_PROBE_PHASE",
        "FORGECAD_K002_EXPECT_THREAD_ID",
        "FORGECAD_K002_EXPECT_TURN_ID",
        "FORGECAD_K002_EXPECT_ITEMS_SHA256",
        "FORGECAD_K002_EXPECT_ITEM_COUNT",
        "FORGECAD_K002_EXPECT_LAST_SEQUENCE",
        "FORGECAD_K002_EXPECT_TURN_ERROR_CODE",
        "FORGECAD_K003_PACKAGED_PROBE",
        "FORGECAD_K003_PACKAGED_PROBE_PHASE",
        "FORGECAD_K003_EXPECT_PROJECT_ID",
        "FORGECAD_K003_EXPECT_ASSET_VERSION_ID",
        "FORGECAD_K003_EXPECT_SNAPSHOT_ETAG",
        "FORGECAD_K003_EXPECT_PROJECT_SHA256",
        "FORGECAD_K003_EXPECT_SNAPSHOT_SHA256",
        "FORGECAD_K003_EXPECT_GLB_SHA256",
        "FORGECAD_K003_EXPECT_RENDER_SET_SHA256",
        "FORGECAD_K003_EXPECT_RENDER_PACKAGE_SHA256",
    }
    command = ["open", "-n"]
    for name in sorted(forwarded_names):
        if name in environment:
            command.extend(["--env", f"{name}={environment[name]}"])
    command.append(str(APP_BUNDLE))
    subprocess.run(command, cwd=temporary, env=environment, check=True)
    for _ in range(100):
        created = _desktop_pids() - existing
        if created:
            return max(created)
        time.sleep(0.1)
    raise AssertionError("LaunchServices did not start the packaged desktop for K003")


def _wait_for_restricted_health(desktop_pid: int) -> int:
    # Match the Rust supervisor's bounded cold-start window on macOS.
    for _ in range(1_200):
        if not _process_exists(desktop_pid):
            raise AssertionError("packaged desktop exited before restricted geometry health")
        try:
            payload = _url_json(HEALTH_URL)
            _validate_restricted_health(payload)
            listener = _listener_pid()
            _assert(listener is not None, "healthy restricted geometry service has no listener")
            return listener
        except (AssertionError, OSError, ValueError):
            time.sleep(0.1)
    raise AssertionError("packaged restricted geometry service was not healthy within 120 seconds")


def _validate_restricted_health(payload: object) -> None:
    _assert(isinstance(payload, dict), "restricted health must be a JSON object")
    expected = {
        "status": "ok",
        "service": "forgecad-restricted-geometry-executor",
        "mode": "restricted_geometry_executor",
        "schema_version": "RestrictedGeometryExecutorHealth@1",
        "python_role": "restricted_geometry_executor",
        "database_access": False,
        "object_store_access": False,
        "provider_access": False,
        "snapshot_write": False,
        "persistent_state_writer": False,
    }
    _assert(payload == expected, "packaged sidecar is not the restricted geometry executor")


def _assert_k003_ready_log(path: Path, minimum_count: int) -> None:
    _assert(path.is_file(), "K003 supervisor log is missing")
    ready_lines = [
        line
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines()
        if line.startswith("ForgeCAD app-server ready protocol=forgecad.app-server/1")
    ]
    _assert(len(ready_lines) >= minimum_count, "K003 Rust app-server ready marker is missing")
    for line in ready_lines[-minimum_count:]:
        _assert("lifecycle_owner=rust-app-server" in line, "Rust lifecycle owner is missing")
        _assert(
            "python_role=restricted_geometry_executor" in line,
            "app-server did not report the restricted Python role",
        )


def _verify_python_boundary(listener_pid: int, library_root: Path, log_path: Path) -> None:
    process_text = _read_process_environment_text(listener_pid)
    capability = _validate_sidecar_environment_text(process_text)
    _assert(SHA256_PATTERN.fullmatch(capability) is not None, "invalid geometry capability")
    _assert_python_open_file_boundary(listener_pid, library_root)
    if log_path.is_file():
        log = log_path.read_text(encoding="utf-8", errors="replace")
        _assert(capability not in log, "restricted geometry capability leaked into logs")
        _assert(str(library_root) not in log, "Rust Library path leaked into shared logs")
        for name in PROVIDER_ENVIRONMENT_KEYS:
            _assert(f"{name}=" not in log, "Provider environment metadata leaked into logs")


def _read_process_environment_text(pid: int) -> str:
    result = subprocess.run(
        ["ps", "eww", "-p", str(pid), "-o", "command="],
        check=False,
        capture_output=True,
        text=True,
    )
    _assert(result.returncode == 0 and result.stdout, "could not inspect sidecar environment")
    return result.stdout


def _validate_sidecar_environment_text(process_text: str) -> str:
    for name in (*PROVIDER_ENVIRONMENT_KEYS, *FORBIDDEN_PYTHON_STATE_KEYS):
        _assert(
            re.search(rf"(?:^|\s){re.escape(name)}=", process_text) is None,
            "Python sidecar received a forbidden state or Provider environment key",
        )
    match = re.search(
        rf"(?:^|\s){RESTRICTED_CAPABILITY_ENV}=([0-9a-f]{{64}})(?:\s|$)",
        process_text,
    )
    _assert(match is not None, "Python sidecar did not receive one restricted capability")
    return match.group(1)


def _assert_python_open_file_boundary(pid: int, library_root: Path) -> None:
    result = subprocess.run(
        ["lsof", "-Fn", "-p", str(pid)],
        check=False,
        capture_output=True,
        text=True,
    )
    _assert(result.returncode == 0, "could not inspect Python sidecar open files")
    resolved_library = library_root.resolve()
    for line in result.stdout.splitlines():
        if not line.startswith("n/"):
            continue
        candidate = Path(line[1:])
        try:
            inside_library = candidate.resolve(strict=False).is_relative_to(resolved_library)
        except (OSError, RuntimeError):
            inside_library = False
        _assert(not inside_library, "Python sidecar opened the Rust Library or CAS")
        _assert(candidate.name != "library.db", "Python sidecar opened a product database")


def _verify_restricted_ownership_and_tombstones(listener_pid: int) -> None:
    process_text = _read_process_environment_text(listener_pid)
    capability = _validate_sidecar_environment_text(process_text)
    ownership = _url_json(
        OWNERSHIP_URL,
        headers={RESTRICTED_CAPABILITY_HEADER: capability},
    )
    _validate_restricted_ownership(ownership)
    for path in (
        "/api/v1/internal/k002/lifecycle",
        "/api/v1/internal/k002/product-tools/execute",
        "/api/v1/projects",
    ):
        status, payload = _url_status_json(
            f"http://127.0.0.1:8000{path}",
            method="POST",
            payload={},
        )
        _assert(status == 410, "old Python lifecycle/product route did not return HTTP 410")
        _validate_rust_owned_tombstone(payload)


def _validate_restricted_ownership(payload: object) -> None:
    _assert(isinstance(payload, dict), "restricted ownership must be an object")
    expected = {
        "schema_version": "RestrictedGeometryCapabilityOwnership@1",
        "protocol_version": "forgecad.restricted-geometry/1",
        "capability_owner": "rust_forgecad_core",
        "python_role": "restricted_geometry_executor",
        "database_access": False,
        "object_store_access": False,
        "provider_access": False,
        "thread_session_access": False,
        "snapshot_write": False,
        "accepts_caller_glb": False,
        "persistent_artifacts": False,
        "actions": ["compile_readback", "render"],
    }
    _assert(payload == expected, "restricted geometry ownership payload diverged")


def _validate_rust_owned_tombstone(payload: object) -> None:
    _assert(isinstance(payload, dict), "Python tombstone must be a JSON object")
    error = payload.get("error")
    _assert(isinstance(error, dict), "Python tombstone error is missing")
    _assert(error.get("code") == "PRODUCT_STATE_RUST_OWNED", "wrong tombstone code")
    _assert(error.get("recoverable") is False, "Python product tombstone is recoverable")


def _url_json(url: str, *, headers: Mapping[str, str] | None = None) -> object:
    request = urllib.request.Request(url, headers=dict(headers or {}))
    with urllib.request.urlopen(request, timeout=2) as response:
        _assert(response.status == 200, "loopback JSON request did not return HTTP 200")
        return json.loads(response.read().decode("utf-8"))


def _url_status_json(
    url: str,
    *,
    method: str,
    payload: Mapping[str, object],
) -> tuple[int, object]:
    body = json.dumps(payload, separators=(",", ":")).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=body,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=2) as response:
            return response.status, json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        return error.code, json.loads(error.read().decode("utf-8"))


def _wait_for_report(
    path: Path,
    marker: str,
    schema: str,
    phase: str,
) -> dict[str, object]:
    for _ in range(1_900):
        if path.is_file():
            reports: list[dict[str, object]] = []
            for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
                if not line.startswith(marker):
                    continue
                try:
                    value = json.loads(line[len(marker) :])
                except (TypeError, ValueError):
                    continue
                if isinstance(value, dict) and value.get("phase") == phase:
                    reports.append(value)
            if reports:
                report = reports[-1]
                _assert(report.get("schema_version") == schema, "packaged probe schema changed")
                if report.get("ok") is not True:
                    raise AssertionError(
                        f"packaged {phase} probe failed with "
                        f"{report.get('error_code', 'UNKNOWN')}"
                    )
                return report
        time.sleep(0.1)
    if marker == K003_PROBE_MARKER:
        raise AssertionError(
            "K003 Rust production-asset probe did not run; no provider-free "
            "NativeProductToolExecutor -> RestrictedGeometryExecutor -> "
            "RustCoreRuntime commit entry is currently available"
        )
    raise AssertionError(f"packaged {phase} probe did not finish within 190 seconds")


def _validate_k003_probe_report(report: Mapping[str, object], phase: str) -> None:
    _assert(report.get("schema_version") == K003_PROBE_SCHEMA, "wrong K003 probe schema")
    _assert(report.get("phase") == phase, "wrong K003 probe phase")
    _assert(report.get("ok") is True, "K003 Rust core probe did not pass")
    _required_id(report, "project_id")
    _required_id(report, "asset_version_id")
    _required_text(report, "snapshot_etag", 256)
    for field in (
        "project_semantic_sha256",
        "snapshot_semantic_sha256",
        "glb_sha256",
        "render_set_sha256",
        "render_package_sha256",
    ):
        _required_sha(report, field)
    _assert(report.get("provider_calls") == 0, "K003 probe made a Provider call")
    _assert(
        report.get("provider_network_call_made") is False,
        "K003 probe reported Provider network activity",
    )
    _assert(
        report.get("execution_path") == K003_EXECUTION_PATH,
        "K003 probe did not use the native-tool/restricted-geometry/Rust-core path",
    )


def _read_library_facts(
    library_root: Path,
    *,
    project_id: str,
    thread_id: str,
    asset_version_id: str,
    glb_sha256: str,
) -> dict[str, object]:
    db_path = library_root / "library.db"
    _assert(db_path.is_file(), "Rust-owned library.db is missing")
    connection = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    connection.row_factory = sqlite3.Row
    try:
        _require_tables(
            connection,
            (
                "forgecad_core_schema_migrations",
                "forgecad_core_ownership",
                "forgecad_core_objects",
                "forgecad_core_object_references",
                "projects",
                "agent_threads",
                "agent_turns",
                "agent_items",
                "agent_asset_versions",
                "active_design_snapshots",
            ),
        )
        migration = connection.execute(
            "SELECT name FROM forgecad_core_schema_migrations WHERE version='0035'"
        ).fetchone()
        _assert(migration is not None, "K003 Rust schema migration is missing")
        ownership = connection.execute(
            "SELECT state_owner, active_writer_instance_id, writer_epoch "
            "FROM forgecad_core_ownership WHERE singleton=1"
        ).fetchone()
        _assert(ownership is not None, "K003 Rust ownership row is missing")
        _assert(ownership["state_owner"] == "rust_app_server", "Rust is not state owner")
        # The smoke intentionally terminates the .app through the same helper
        # used by the existing packaged gates. On Unix, SIGTERM may not run
        # Rust destructors, so an instance ID can remain as a crash-recovery
        # marker. The next successful launch must supersede it by advancing
        # writer_epoch; requiring NULL here would test graceful Drop rather
        # than K003 restart recovery.

        project = _one_semantic_row(
            connection,
            "projects",
            "project_id=?",
            (project_id,),
            excluded={"created_at", "updated_at"},
        )
        snapshot = _one_semantic_row(
            connection,
            "active_design_snapshots",
            "project_id=?",
            (project_id,),
            excluded={"updated_at"},
        )
        _assert(
            snapshot.get("active_asset_version_id") == asset_version_id,
            "Snapshot does not point at the packaged probe asset",
        )
        asset = _one_semantic_row(
            connection,
            "agent_asset_versions",
            "asset_version_id=?",
            (asset_version_id,),
            excluded={"created_at"},
        )
        thread = _thread_semantics(connection, thread_id)
        object_row = connection.execute(
            "SELECT sha256, object_path, extension, byte_size, ref_count "
            "FROM forgecad_core_objects WHERE sha256=?",
            (glb_sha256,),
        ).fetchone()
        _assert(object_row is not None, "production GLB is not indexed by Rust CAS")
        reference = connection.execute(
            "SELECT sha256 FROM forgecad_core_object_references "
            "WHERE reference_kind='asset_version' AND owner_id=? "
            "AND role='production_glb'",
            (asset_version_id,),
        ).fetchone()
        _assert(
            reference is not None and reference["sha256"] == glb_sha256,
            "asset version does not own the production GLB reference",
        )
        reference_count = connection.execute(
            "SELECT COUNT(*) FROM forgecad_core_object_references WHERE sha256=?",
            (glb_sha256,),
        ).fetchone()[0]
        _assert(
            object_row["ref_count"] == reference_count and reference_count > 0,
            "Rust CAS reference count is inconsistent",
        )
        _assert(object_row["extension"] == "glb", "production CAS object is not GLB")
        relative = _safe_relative_object_path(object_row["object_path"], glb_sha256)
        object_path = library_root / "objects" / "sha256" / relative
        bytes_value = object_path.read_bytes()
        _assert(bytes_value[:4] == b"glTF" and len(bytes_value) >= 20, "CAS GLB is invalid")
        _assert(hashlib.sha256(bytes_value).hexdigest() == glb_sha256, "CAS GLB hash changed")
        _assert(len(bytes_value) == object_row["byte_size"], "CAS GLB byte size changed")
        _assert_empty_directory(library_root / "objects" / ".staging")
        _assert_empty_directory(library_root / "objects" / ".pending")

        return {
            "writer_epoch": int(ownership["writer_epoch"]),
            "project_semantic_sha256": _semantic_sha256(project),
            "thread_semantic_sha256": _semantic_sha256(thread),
            "snapshot_semantic_sha256": _semantic_sha256(snapshot),
            "asset_version_semantic_sha256": _semantic_sha256(asset),
            "glb_sha256": glb_sha256,
            "glb_byte_size": len(bytes_value),
        }
    finally:
        connection.close()


def _require_tables(connection: sqlite3.Connection, names: Iterable[str]) -> None:
    existing = {
        row[0]
        for row in connection.execute(
            "SELECT name FROM sqlite_master WHERE type='table'"
        ).fetchall()
    }
    missing = sorted(set(names) - existing)
    _assert(not missing, "Rust library is missing required K003 tables")


def _one_semantic_row(
    connection: sqlite3.Connection,
    table: str,
    predicate: str,
    parameters: tuple[object, ...],
    *,
    excluded: set[str],
) -> dict[str, object]:
    rows = connection.execute(
        f"SELECT * FROM {table} WHERE {predicate}",  # noqa: S608 - code-owned identifiers
        parameters,
    ).fetchall()
    _assert(len(rows) == 1, "authoritative K003 row is missing or duplicated")
    return _semantic_row(rows[0], excluded=excluded)


def _semantic_row(row: sqlite3.Row, *, excluded: set[str]) -> dict[str, object]:
    result: dict[str, object] = {}
    for key in sorted(row.keys()):
        if key in excluded:
            continue
        value: object = row[key]
        if isinstance(value, str) and key.endswith("_json"):
            value = json.loads(value)
        result[key] = value
    return result


def _thread_semantics(connection: sqlite3.Connection, thread_id: str) -> dict[str, object]:
    thread = _one_semantic_row(
        connection,
        "agent_threads",
        "thread_id=?",
        (thread_id,),
        excluded={"created_at", "updated_at"},
    )
    turns = [
        _semantic_row(row, excluded={"created_at", "updated_at"})
        for row in connection.execute(
            "SELECT * FROM agent_turns WHERE thread_id=? ORDER BY turn_id",
            (thread_id,),
        ).fetchall()
    ]
    items = [
        _semantic_row(row, excluded={"created_at"})
        for row in connection.execute(
            "SELECT * FROM agent_items WHERE thread_id=? ORDER BY sequence, item_id",
            (thread_id,),
        ).fetchall()
    ]
    _assert(turns and items, "K003 lifecycle semantic state is incomplete")
    return {"thread": thread, "turns": turns, "items": items}


def _safe_relative_object_path(value: object, sha256: str) -> Path:
    _assert(isinstance(value, str) and value, "CAS object path is invalid")
    relative = Path(value)
    _assert(not relative.is_absolute() and ".." not in relative.parts, "CAS path escapes Library")
    _assert(relative.name == f"{sha256}.glb", "CAS path does not match GLB identity")
    return relative


def _assert_empty_directory(path: Path) -> None:
    _assert(path.is_dir(), "Rust CAS staging directory is missing")
    _assert(not any(path.iterdir()), "Rust CAS left an unfinished object journal or stage")


def _semantic_sha256(value: object) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _assert_restart_reports(
    initial_k001: Mapping[str, object],
    restart_k001: Mapping[str, object],
    initial_k002: Mapping[str, object],
    restart_k002: Mapping[str, object],
    initial_k003: Mapping[str, object],
    restart_k003: Mapping[str, object],
) -> None:
    for field in (
        "project_id",
        "thread_id",
        "asset_version_id",
        "last_event_id",
        "cursor",
        "glb_sha256",
        "protocol_glb_sha256",
        "resource_glb_sha256",
        "turn_status",
        "turn_error_code",
    ):
        _assert(restart_k001.get(field) == initial_k001.get(field), "K001 truth changed")
    _assert(
        restart_k001.get("first_event_id") == "1",
        "K001 restart did not replay the first native Item",
    )
    _assert(
        restart_k001.get("resume_from_event_id") == initial_k001.get("last_event_id"),
        "K001 restart did not acknowledge the initial Item sequence",
    )
    _assert(
        restart_k001.get("resume_from_cursor") == initial_k001.get("cursor"),
        "K001 restart did not retain the initial Rust cursor",
    )
    for field in (
        "thread_id",
        "turn_id",
        "turn_status",
        "turn_error_code",
        "item_count",
        "last_sequence",
        "item_sequences",
        "item_ids",
        "item_types",
        "items_sha256",
        "replay_items_sha256",
    ):
        _assert(restart_k002.get(field) == initial_k002.get(field), "K002 lifecycle changed")
    for field in (
        "project_id",
        "asset_version_id",
        "snapshot_etag",
        "project_semantic_sha256",
        "snapshot_semantic_sha256",
        "glb_sha256",
        "render_set_sha256",
        "render_package_sha256",
        "execution_path",
    ):
        _assert(restart_k003.get(field) == initial_k003.get(field), "K003 product state changed")


def _assert_semantic_recovery(
    initial: Mapping[str, object], restart: Mapping[str, object]
) -> None:
    for field in (
        "project_semantic_sha256",
        "thread_semantic_sha256",
        "snapshot_semantic_sha256",
        "asset_version_semantic_sha256",
        "glb_sha256",
        "glb_byte_size",
    ):
        _assert(initial.get(field) == restart.get(field), f"restart changed {field}")
    _assert(
        _required_int(restart, "writer_epoch") > _required_int(initial, "writer_epoch"),
        "Rust writer epoch did not advance on packaged restart",
    )


def _required_id(value: Mapping[str, object], field: str) -> str:
    result = value.get(field)
    _assert(
        isinstance(result, str) and STABLE_ID_PATTERN.fullmatch(result) is not None,
        f"{field} is not a bounded stable ID",
    )
    return result


def _required_sha(value: Mapping[str, object], field: str) -> str:
    result = value.get(field)
    _assert(
        isinstance(result, str) and SHA256_PATTERN.fullmatch(result) is not None,
        f"{field} is not a SHA-256",
    )
    return result


def _required_int(value: Mapping[str, object], field: str) -> int:
    result = value.get(field)
    _assert(type(result) is int and result >= 0, f"{field} is not a non-negative integer")
    return result


def _required_text(value: Mapping[str, object], field: str, maximum: int) -> str:
    result = value.get(field)
    _assert(
        isinstance(result, str) and result == result.strip() and 1 <= len(result) <= maximum,
        f"{field} is not bounded text",
    )
    return result


def _process_exists(pid: int) -> bool:
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False


def _write_artifact(
    report: Mapping[str, object],
    *,
    log_path: Path | None = None,
    temporary_path: Path | None = None,
) -> None:
    if not ARTIFACT_DIRECTORY:
        return
    directory = Path(ARTIFACT_DIRECTORY)
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "k003-packaged-report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    if log_path is not None and log_path.is_file():
        contents = log_path.read_text(encoding="utf-8", errors="replace")[-16_384:]
        (directory / "k003-packaged-supervisor.log").write_text(
            _safe_diagnostic(contents, temporary_path) + "\n",
            encoding="utf-8",
        )


def _safe_diagnostic(value: str, temporary_path: Path | None) -> str:
    sanitized = value.replace(str(ROOT), "<workspace>")
    if temporary_path is not None:
        sanitized = sanitized.replace(str(temporary_path), "<temporary>")
    sanitized = re.sub(
        rf"{RESTRICTED_CAPABILITY_ENV}=[0-9a-f]{{64}}",
        f"{RESTRICTED_CAPABILITY_ENV}=<redacted>",
        sanitized,
    )
    return sanitized


def _run_self_test() -> dict[str, object]:
    token = "a" * 64
    process_text = f"wushen-agent HOME=/tmp/isolated {RESTRICTED_CAPABILITY_ENV}={token}"
    _assert(_validate_sidecar_environment_text(process_text) == token, "boundary parser failed")
    _expect_assertion(
        lambda: _validate_sidecar_environment_text(
            f"{process_text} WUSHEN_LIBRARY_ROOT=/tmp/forbidden"
        )
    )
    _validate_restricted_health(
        {
            "status": "ok",
            "service": "forgecad-restricted-geometry-executor",
            "mode": "restricted_geometry_executor",
            "schema_version": "RestrictedGeometryExecutorHealth@1",
            "python_role": "restricted_geometry_executor",
            "database_access": False,
            "object_store_access": False,
            "provider_access": False,
            "snapshot_write": False,
            "persistent_state_writer": False,
        }
    )
    _validate_restricted_ownership(
        {
            "schema_version": "RestrictedGeometryCapabilityOwnership@1",
            "protocol_version": "forgecad.restricted-geometry/1",
            "capability_owner": "rust_forgecad_core",
            "python_role": "restricted_geometry_executor",
            "database_access": False,
            "object_store_access": False,
            "provider_access": False,
            "thread_session_access": False,
            "snapshot_write": False,
            "accepts_caller_glb": False,
            "persistent_artifacts": False,
            "actions": ["compile_readback", "render"],
        }
    )
    _validate_rust_owned_tombstone(
        {
            "error": {
                "code": "PRODUCT_STATE_RUST_OWNED",
                "recoverable": False,
                "message": "Rust owned",
                "details": {},
            }
        }
    )
    sample_sha = "b" * 64
    sample_report: dict[str, object] = {
        "schema_version": K003_PROBE_SCHEMA,
        "phase": "initial",
        "ok": True,
        "project_id": "prj_k003_self_test",
        "asset_version_id": "assetver_k003_self_test",
        "snapshot_etag": "snapshot:1:self-test",
        "project_semantic_sha256": sample_sha,
        "snapshot_semantic_sha256": sample_sha,
        "glb_sha256": sample_sha,
        "render_set_sha256": sample_sha,
        "render_package_sha256": sample_sha,
        "provider_calls": 0,
        "provider_network_call_made": False,
        "execution_path": K003_EXECUTION_PATH,
    }
    _validate_k003_probe_report(sample_report, "initial")

    with tempfile.TemporaryDirectory(prefix="forgecad_k003_self_test_") as directory:
        library_root = Path(directory) / "library"
        ids = _create_self_test_library(library_root)
        first = _read_library_facts(library_root, **ids)
        second = _read_library_facts(library_root, **ids)
        second["writer_epoch"] = int(second["writer_epoch"]) + 1
        _assert_semantic_recovery(first, second)
        connection = sqlite3.connect(library_root / "library.db")
        connection.execute("UPDATE projects SET name='changed' WHERE project_id=?", (ids["project_id"],))
        connection.commit()
        connection.close()
        changed = _read_library_facts(library_root, **ids)
        _assert(
            changed["project_semantic_sha256"] != first["project_semantic_sha256"],
            "project semantic hash ignored a product-state mutation",
        )

    return {
        "schema_version": "ForgeCADK003PackagedSmokeSelfTest@1",
        "ok": True,
        "real_packaged_app_launched": False,
        "packaged_k003_claimed": False,
        "validators": [
            "restricted_health",
            "restricted_ownership",
            "python_environment_boundary",
            "python_product_tombstone",
            "k003_probe_contract",
            "sqlite_semantic_hashes",
            "cas_glb_identity",
            "restart_comparison",
        ],
    }


def _create_self_test_library(library_root: Path) -> dict[str, str]:
    library_root.mkdir(parents=True)
    for name in (".staging", ".pending"):
        (library_root / "objects" / name).mkdir(parents=True)
    database = sqlite3.connect(library_root / "library.db")
    database.executescript(
        """
        CREATE TABLE forgecad_core_schema_migrations(version TEXT PRIMARY KEY, name TEXT);
        CREATE TABLE forgecad_core_ownership(
          singleton INTEGER PRIMARY KEY, state_owner TEXT,
          active_writer_instance_id TEXT, writer_epoch INTEGER
        );
        CREATE TABLE forgecad_core_objects(
          sha256 TEXT PRIMARY KEY, object_path TEXT, extension TEXT,
          byte_size INTEGER, ref_count INTEGER
        );
        CREATE TABLE forgecad_core_object_references(
          reference_kind TEXT, owner_id TEXT, role TEXT, sha256 TEXT
        );
        CREATE TABLE projects(
          project_id TEXT PRIMARY KEY, profile_id TEXT, domain_type TEXT, name TEXT,
          status TEXT, current_version_id TEXT, created_at TEXT, updated_at TEXT
        );
        CREATE TABLE agent_threads(
          thread_id TEXT PRIMARY KEY, project_id TEXT, title TEXT, status TEXT,
          summary TEXT, provider_id TEXT, created_at TEXT, updated_at TEXT,
          last_turn_id TEXT
        );
        CREATE TABLE agent_turns(
          turn_id TEXT PRIMARY KEY, thread_id TEXT, request_text TEXT, status TEXT,
          error_code TEXT, error_message TEXT, usage_json TEXT,
          created_at TEXT, updated_at TEXT
        );
        CREATE TABLE agent_items(
          item_id TEXT PRIMARY KEY, thread_id TEXT, turn_id TEXT, sequence INTEGER,
          item_type TEXT, status TEXT, payload_json TEXT, created_at TEXT
        );
        CREATE TABLE agent_asset_versions(
          asset_version_id TEXT PRIMARY KEY, project_id TEXT,
          parent_asset_version_id TEXT, version_no INTEGER, status TEXT, summary TEXT,
          stage TEXT, plan_id TEXT, direction_id TEXT, domain_pack_id TEXT,
          artifact_id TEXT, parts_json TEXT, shape_program_json TEXT,
          assembly_graph_json TEXT, material_bindings_json TEXT, created_at TEXT
        );
        CREATE TABLE active_design_snapshots(
          project_id TEXT PRIMARY KEY, source TEXT, active_asset_version_id TEXT,
          active_assembly_graph_id TEXT, selected_part_id TEXT,
          quality_report_id TEXT, export_source TEXT, export_source_version_id TEXT,
          revision INTEGER, updated_at TEXT
        );
        """
    )
    project_id = "prj_k003_self"
    thread_id = "thread_k003_self"
    turn_id = "turn_k003_self"
    asset_version_id = "assetver_k003_self"
    artifact_id = "artifact_k003_self"
    timestamp = "2026-07-17T00:00:00Z"
    glb = b"glTF" + bytes(range(16))
    glb_sha256 = hashlib.sha256(glb).hexdigest()
    relative = Path(glb_sha256[:2]) / glb_sha256[2:4] / f"{glb_sha256}.glb"
    glb_path = library_root / "objects" / "sha256" / relative
    glb_path.parent.mkdir(parents=True)
    glb_path.write_bytes(glb)
    database.execute(
        "INSERT INTO forgecad_core_schema_migrations VALUES ('0035','k003')"
    )
    database.execute(
        "INSERT INTO forgecad_core_ownership VALUES (1,'rust_app_server',NULL,1)"
    )
    database.execute(
        "INSERT INTO projects VALUES (?,?,?,?,?,?,?,?)",
        (
            project_id,
            "profile_robot",
            "weapon_concept",
            "Self test",
            "active",
            asset_version_id,
            timestamp,
            timestamp,
        ),
    )
    database.execute(
        "INSERT INTO agent_threads VALUES (?,?,?,?,?,?,?,?,?)",
        (
            thread_id,
            project_id,
            "Self test",
            "error",
            "",
            "deepseek",
            timestamp,
            timestamp,
            turn_id,
        ),
    )
    database.execute(
        "INSERT INTO agent_turns VALUES (?,?,?,?,?,?,?,?,?)",
        (
            turn_id,
            thread_id,
            "concept",
            "failed",
            "PROVIDER_NOT_CONFIGURED",
            "unconfigured",
            '{"provider_requests":0}',
            timestamp,
            timestamp,
        ),
    )
    database.execute(
        "INSERT INTO agent_items VALUES (?,?,?,?,?,?,?,?)",
        (
            "item_k003_self",
            thread_id,
            turn_id,
            1,
            "user_message",
            "completed",
            '{"content":"concept"}',
            timestamp,
        ),
    )
    database.execute(
        "INSERT INTO agent_asset_versions VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
        (
            asset_version_id,
            project_id,
            None,
            1,
            "committed",
            "Self test",
            "editable_asset",
            "plan_k003_self",
            "direction_primary",
            "pack_robotic_arm_concept",
            artifact_id,
            "[]",
            '{"schema_version":"ShapeProgram@1"}',
            '{"graph_id":"graph_k003_self","parts":[]}',
            "{}",
            timestamp,
        ),
    )
    database.execute(
        "INSERT INTO active_design_snapshots VALUES (?,?,?,?,?,?,?,?,?,?)",
        (
            project_id,
            "agent_asset",
            asset_version_id,
            "graph_k003_self",
            None,
            "quality_k003_self",
            "agent_asset",
            asset_version_id,
            1,
            timestamp,
        ),
    )
    database.execute(
        "INSERT INTO forgecad_core_objects VALUES (?,?,?,?,1)",
        (glb_sha256, str(relative), "glb", len(glb)),
    )
    database.execute(
        "INSERT INTO forgecad_core_object_references VALUES "
        "('asset_version',?,'production_glb',?)",
        (asset_version_id, glb_sha256),
    )
    database.commit()
    database.close()
    return {
        "project_id": project_id,
        "thread_id": thread_id,
        "asset_version_id": asset_version_id,
        "glb_sha256": glb_sha256,
    }


def _expect_assertion(callback: Any) -> None:
    try:
        callback()
    except AssertionError:
        return
    raise AssertionError("negative validator fixture unexpectedly passed")


if __name__ == "__main__":
    raise SystemExit(main())
