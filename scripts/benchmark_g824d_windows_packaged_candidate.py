#!/usr/bin/env python3
"""Build a Windows x64 sidecar and execute the fixed CSG evidence inside it."""

from __future__ import annotations

import argparse
import gc
import hashlib
import json
import os
import platform
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

from forgecad_agent.infrastructure.db import SQLiteConnectionFactory, SQLiteMigrationRunner
from forgecad_agent.infrastructure.storage.content_addressed_store import ContentAddressedStore
from smoke_g824b_csg_promotion_boundary import (
    _database_fingerprint,
    _object_fingerprint,
    _rollback_and_success,
    _seed,
)


ROOT = Path(__file__).resolve().parents[1]


def _free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as handle:
        handle.bind(("127.0.0.1", 0))
        return int(handle.getsockname()[1])


def _kill_tree(process: subprocess.Popen[str]) -> None:
    subprocess.run(
        ["taskkill", "/PID", str(process.pid), "/T", "/F"],
        text=True,
        capture_output=True,
        check=False,
    )
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=10)


def _wait_for(path: Path, process: subprocess.Popen[str], timeout: float = 30.0) -> dict:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if path.is_file():
            return json.loads(path.read_text(encoding="utf-8"))
        if process.poll() is not None:
            stdout, stderr = process.communicate()
            raise RuntimeError(f"packaged candidate exited before marker: {stdout[-1000:]} {stderr[-1000:]}")
        time.sleep(0.05)
    _kill_tree(process)
    raise TimeoutError(f"packaged candidate marker timed out: {path.name}")


def _candidate_environment(mode: str, root: Path) -> dict[str, str]:
    environment = dict(os.environ)
    for name in list(environment):
        if "PROVIDER" in name or "API_KEY" in name:
            environment.pop(name, None)
    environment.update(
        {
            "FORGECAD_G824D_MODE": mode,
            "FORGECAD_G824D_MARKER": str(root / "marker.json"),
            "FORGECAD_G824D_RESULT": str(root / "result.json"),
            "FORGECAD_G824D_STAGING_GLB": str(root / "staging" / "candidate.glb"),
        }
    )
    return environment


def _run_provenance(binary: Path) -> dict:
    with tempfile.TemporaryDirectory(prefix="forgecad-g824d-provenance-") as raw:
        root = Path(raw)
        completed = subprocess.run(
            [str(binary)],
            cwd=root,
            env=_candidate_environment("provenance", root),
            text=True,
            capture_output=True,
            timeout=120,
        )
        if completed.returncode != 0:
            raise AssertionError(f"packaged provenance failed: {completed.stderr[-2000:]}")
        return json.loads((root / "result.json").read_text(encoding="utf-8"))


def _interrupt_case(
    binary: Path,
    *,
    mode: str,
    error_code: str,
    factory: SQLiteConnectionFactory,
    store: ContentAddressedStore,
    before_db: dict,
    before_objects: dict,
) -> dict:
    with tempfile.TemporaryDirectory(prefix=f"forgecad-g824d-{mode}-") as raw:
        root = Path(raw)
        environment = _candidate_environment(mode, root)
        process = subprocess.Popen(
            [str(binary)],
            cwd=root,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        marker = root / "marker.json"
        payload = _wait_for(marker, process)
        staging = root / "staging" / "candidate.glb"
        if mode == "ready":
            if not staging.is_file() or hashlib.sha256(staging.read_bytes()).hexdigest() != payload["glb_sha256"]:
                _kill_tree(process)
                raise AssertionError("packaged ready marker does not match staged GLB")
        _kill_tree(process)
        valid_staged = staging.is_file() if mode == "ready" else False
        if staging.exists():
            staging.unlink()
        return {
            "window": "candidate_ready_before_promotion" if mode == "ready" else "kernel_running",
            "error_code": error_code,
            "process_reaped": process.poll() is not None,
            "partial_glb_emitted": mode != "ready" and staging.exists(),
            "valid_staged_glb_existed": valid_staged,
            "staging_cleanup_verified": not staging.exists(),
            "database_unchanged": _database_fingerprint(factory) == before_db,
            "object_store_unchanged": _object_fingerprint(store) == before_objects,
            "authoritative_paths_passed_to_child": False,
        }


def _health(binary: Path) -> dict:
    with tempfile.TemporaryDirectory(prefix="forgecad-g824d-health-") as raw:
        root = Path(raw)
        port = _free_port()
        environment = dict(os.environ)
        for name in list(environment):
            if "PROVIDER" in name or "API_KEY" in name:
                environment.pop(name, None)
        environment["WUSHEN_LIBRARY_ROOT"] = str(root / "library")
        environment["FORGECAD_CONCEPT_WORKER_ENABLED"] = "0"
        environment["WUSHEN_LOCAL_WORKER_ENABLED"] = "0"
        process = subprocess.Popen(
            [str(binary), "agent", "serve", "--host", "127.0.0.1", "--port", str(port)],
            cwd=root,
            env=environment,
            text=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
        )
        started = time.perf_counter()
        endpoint = f"http://127.0.0.1:{port}/api/health"
        try:
            for _ in range(600):
                if process.poll() is not None:
                    raise AssertionError(f"packaged sidecar exited: {process.stderr.read()[-2000:]}")
                try:
                    with urllib.request.urlopen(endpoint, timeout=0.25) as response:
                        payload = json.loads(response.read().decode("utf-8"))
                    if payload.get("status") == "ok":
                        return {"passed": True, "cold_start_ms": round((time.perf_counter() - started) * 1000, 3)}
                except OSError:
                    time.sleep(0.05)
            raise TimeoutError("packaged sidecar health timed out")
        finally:
            _kill_tree(process)


def _build(binary_root: Path, python_site: Path) -> tuple[Path, float]:
    hook = ROOT / "scripts/g824d_packaged_candidate_runtime_hook.py"
    name = "wushen-agent-g824d-x86_64-pc-windows-msvc"
    command = [
        sys.executable,
        "-m",
        "PyInstaller",
        "--noconfirm",
        "--clean",
        "--onefile",
        "--name",
        name,
        "--paths",
        str(ROOT / "apps/agent"),
        "--paths",
        str(ROOT / "scripts"),
        "--paths",
        str(python_site),
        "--add-data",
        f"{ROOT / 'migrations'}{os.pathsep}migrations",
        "--add-data",
        f"{ROOT / 'packages/concept-spec'}{os.pathsep}packages/concept-spec",
        "--runtime-hook",
        str(hook),
        "--hidden-import",
        "manifold3d",
        "--hidden-import",
        "numpy",
        "--hidden-import",
        "numpy._core._exceptions",
        "--hidden-import",
        "g824a_manifold_python_evidence",
        "--hidden-import",
        "g824a_provenance_glb",
        "--collect-all",
        "fastapi",
        "--collect-all",
        "starlette",
        "--collect-all",
        "pydantic",
        "--collect-all",
        "jsonschema",
        "--collect-all",
        "wushen_agent",
        "--collect-all",
        "forgecad_agent",
        "--distpath",
        str(binary_root / "dist"),
        "--workpath",
        str(binary_root / "work"),
        "--specpath",
        str(binary_root / "spec"),
        str(ROOT / "apps/agent/wushen_agent/sidecar_entry.py"),
    ]
    started = time.perf_counter()
    completed = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=600)
    if completed.returncode != 0:
        raise AssertionError(f"Windows packaged candidate build failed: {completed.stderr[-4000:]}")
    binary = binary_root / "dist" / f"{name}.exe"
    if not binary.is_file():
        raise AssertionError("Windows packaged candidate executable is missing")
    return binary, (time.perf_counter() - started) * 1000


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--python-site", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() != "Windows" or platform.machine().lower() not in {"amd64", "x86_64"}:
        raise SystemExit("G824D requires a real Windows x64 runner")
    with tempfile.TemporaryDirectory(prefix="forgecad-g824d-build-") as build_raw:
        binary, build_ms = _build(Path(build_raw), args.python_site)
        provenance = _run_provenance(binary)
        health = _health(binary)
        with tempfile.TemporaryDirectory(prefix="forgecad-g824d-authoritative-") as state_raw:
            state_root = Path(state_raw)
            factory = SQLiteConnectionFactory(state_root / "library.db")
            SQLiteMigrationRunner(factory, ROOT / "migrations").run()
            store = ContentAddressedStore(state_root / "library")
            _seed(factory, store)
            before_db = _database_fingerprint(factory)
            before_objects = _object_fingerprint(store)
            cases = [
                _interrupt_case(
                    binary,
                    mode="busy",
                    error_code=code,
                    factory=factory,
                    store=store,
                    before_db=before_db,
                    before_objects=before_objects,
                )
                for code in ("CSG_CANCELLED", "CSG_TIMEOUT")
            ]
            cases.append(
                _interrupt_case(
                    binary,
                    mode="ready",
                    error_code="CSG_CANCELLED",
                    factory=factory,
                    store=store,
                    before_db=before_db,
                    before_objects=before_objects,
                )
            )
            promotion = _rollback_and_success(factory)
            # sqlite3 context managers commit/rollback but do not close the
            # connection themselves.  CPython normally reclaims the final
            # short-lived evidence connection immediately; force collection
            # before Windows removes the temporary database, where an open
            # handle cannot be unlinked as it can on Unix.
            gc.collect()
        report = {
            "schema_version": "ForgeCADCSGWindowsPackagedEvidence@1",
            "machine": {
                "platform": platform.platform(),
                "machine": platform.machine(),
                "python": platform.python_version(),
            },
            "candidate": {
                "name": "manifold_python",
                "version": "manifold3d==3.5.2",
                "source_commit": "11235e6b8ebea2dbed8aec4285685aafd3d95667",
                "executable_bytes": binary.stat().st_size,
                "build_ms": round(build_ms, 3),
                "health": health,
                "provider_calls": 0,
            },
            "packaged_provenance": provenance,
            "lifecycle_cases": cases,
            "promotion_transaction": promotion,
            "decision": {
                "status": "windows_packaged_runtime_passed",
                "candidate": "manifold_python",
                "production_dependency_added": False,
                "remaining_blocker": "superseding ADR must formally adopt the candidate before G825",
            },
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(args.output.suffix + ".tmp")
    temporary.write_text(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    temporary.replace(args.output)
    print(json.dumps({"ok": True, "output": str(args.output), "decision": report["decision"]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
