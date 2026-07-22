#!/usr/bin/env python3
"""Fast, local-only host preflight for the layered ForgeCAD gate.

This module intentionally owns no product state and never starts or stops a
service.  It probes the host, a temporary directory, loopback port policy and
configured artifact paths, then emits one machine-readable report.
"""

from __future__ import annotations

import argparse
import errno
import hashlib
import json
import multiprocessing
import os
import platform
import resource
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, Iterable, List, Mapping, Optional, Sequence, Tuple


SCHEMA_VERSION = "ForgeCADHostPreflightReport@1"
PHASE = "host_preflight"
DEFAULT_REQUIRED_COMMANDS = ("rustc", "cargo", "node", "npm", "python3", "git", "lsof")
DEFAULT_MIN_FREE_BYTES = 1024 * 1024 * 1024

EXIT_OK = 0
EXIT_WARNING = 1
EXIT_HARD_FAIL = 2
EXIT_INVALID_ARGUMENT = 3


class ProbeFailure(Exception):
    """A local probe failure carrying only a stable public code."""

    def __init__(
        self,
        code: str,
        subsystem: str,
        severity: str = "hard_fail",
        details: Optional[Mapping[str, Any]] = None,
    ) -> None:
        super().__init__(code)
        self.code = code
        self.subsystem = subsystem
        self.severity = severity
        self.details: Dict[str, Any] = dict(details or {})


class FilesystemCapacityFailure(ProbeFailure):
    """A corroborated filesystem-capacity failure without a path leak."""

    def __init__(
        self,
        *,
        root_label: str,
        operation: str,
        errno_class: str,
        cleanup_status: str = "not_needed",
        cleanup_residue: bool = False,
        worker_cleanup_status: str = "not_needed",
        worker_process_residue: bool = False,
    ) -> None:
        # Keep the established public vnode code, but only after a real
        # filesystem operation has corroborated that the host cannot service
        # the work required by the gate.
        super().__init__("HOST_VNODES_PRESSURE_HARD", "resources")
        self.root_label = root_label
        self.operation = operation
        self.errno_class = errno_class
        self.cleanup_status = cleanup_status
        self.cleanup_residue = cleanup_residue
        self.worker_cleanup_status = worker_cleanup_status
        self.worker_process_residue = worker_process_residue


@dataclass(frozen=True)
class PreflightConfig:
    workspace: Path
    library: Optional[Path] = None
    sidecar: Optional[Path] = None
    app_artifact: Optional[Path] = None
    expected_sha256: Mapping[str, str] = field(default_factory=dict)
    tmp_dir: Path = Path("/tmp")
    min_free_bytes: int = DEFAULT_MIN_FREE_BYTES
    port: Optional[int] = None
    dynamic_port: bool = False
    expected_owner: Optional[str] = None
    required_commands: Tuple[str, ...] = DEFAULT_REQUIRED_COMMANDS
    venv: Optional[Path] = None
    fd_warning_ratio: float = 0.80
    fd_hard_ratio: float = 0.95
    system_warning_ratio: float = 0.80
    system_hard_ratio: float = 0.98
    warnings_fail: bool = False
    holder_summary_limit: int = 12
    filesystem_probe_file_count: int = 64
    filesystem_probe_payload_bytes: int = 4096
    filesystem_probe_deadline_seconds: float = 2.0


def _finding(
    *,
    subsystem: str,
    code: str,
    severity: str,
    check: str,
    details: Optional[Mapping[str, Any]] = None,
) -> Dict[str, Any]:
    value: Dict[str, Any] = {
        "phase": PHASE,
        "subsystem": subsystem,
        "stable_error_code": code,
        "severity": severity,
        "check": check,
    }
    if details:
        value["details"] = dict(details)
    return value


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _resolve_command(command: str) -> Optional[str]:
    return shutil.which(command)


def _rustup_tool_path(rustup: str, command: str) -> Optional[str]:
    try:
        completed = subprocess.run(
            [rustup, "which", command],
            check=False,
            capture_output=True,
            text=True,
            timeout=0.75,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if completed.returncode != 0:
        return None
    candidate = completed.stdout.strip().splitlines()[-1] if completed.stdout.strip() else ""
    path = Path(candidate)
    return str(path) if path.is_file() and os.access(path, os.X_OK) else None


def _rust_toolchain_preflight(workspace: Path) -> Dict[str, Any]:
    wrapper = workspace / "script" / "with_rust_toolchain.sh"
    wrapper_status: Dict[str, Any] = {
        "path": "script/with_rust_toolchain.sh",
        "exists": wrapper.is_file(),
        "executable": wrapper.is_file() and os.access(wrapper, os.X_OK),
    }
    result: Dict[str, Any] = {"wrapper": wrapper_status, "rustup": {"status": "unresolved"}, "commands": {}}
    rustup = _resolve_command("rustup")
    if rustup:
        result["rustup"] = {"status": "resolved", "resolution": "path"}
    if not wrapper_status["exists"] or not wrapper_status["executable"]:
        for command in ("rustc", "cargo"):
            result["commands"][command] = {"status": "unresolved", "resolver": "workspace_wrapper"}
        return result
    for command in ("rustc", "cargo"):
        tool_path = _rustup_tool_path(rustup, command) if rustup else None
        probe_status = "not_run"
        try:
            probe = subprocess.run(
                [str(wrapper), command, "--version"],
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                timeout=1.0,
            )
            probe_status = "passed" if probe.returncode == 0 else "failed"
        except (OSError, subprocess.SubprocessError):
            probe_status = "failed"
        result["commands"][command] = {
            "status": "resolved_via_wrapper" if probe_status == "passed" else "unresolved",
            "resolver": "workspace_wrapper",
            "toolchain_resolution": "rustup" if tool_path else "not_resolved",
            "wrapper_path": "script/with_rust_toolchain.sh",
            "wrapper_probe": probe_status,
        }
    return result


def _read_sysctl(name: str) -> Optional[int]:
    if platform.system() != "Darwin":
        return None
    executable = shutil.which("sysctl") or "/usr/sbin/sysctl"
    try:
        completed = subprocess.run(
            [executable, "-n", name],
            check=False,
            capture_output=True,
            text=True,
            timeout=0.5,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    if completed.returncode != 0:
        return None
    try:
        return int(completed.stdout.strip())
    except (TypeError, ValueError):
        return None


def _count_open_fds() -> int:
    for candidate in (Path("/dev/fd"), Path("/proc/self/fd")):
        try:
            return len(list(candidate.iterdir()))
        except OSError:
            continue
    raise ProbeFailure("HOST_FD_COUNT_UNAVAILABLE", "resources", "warning")


def _fd_snapshot() -> Dict[str, Any]:
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    return {
        "open_count": _count_open_fds(),
        "soft_limit": None if soft == resource.RLIM_INFINITY else int(soft),
        "hard_limit": None if hard == resource.RLIM_INFINITY else int(hard),
    }


def _disk_free_bytes(path: Path) -> int:
    return int(shutil.disk_usage(path).free)


def probe_tmp_transaction(root: Path) -> Dict[str, Any]:
    """Exercise a disposable child of TMPDIR, then remove it."""

    if not root.exists() or not root.is_dir():
        raise ProbeFailure("HOST_TMP_UNWRITABLE", "tmp")
    probe_dir: Optional[Path] = None
    failure: Optional[ProbeFailure] = None
    result: Optional[Dict[str, Any]] = None
    try:
        probe_dir = Path(tempfile.mkdtemp(prefix="forgecad-host-preflight-", dir=str(root)))
        source = probe_dir / "source"
        renamed = probe_dir / "renamed"
        with source.open("wb") as handle:
            handle.write(b"ForgeCADHostPreflight@1\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(source, renamed)
        with renamed.open("rb") as handle:
            if handle.read() != b"ForgeCADHostPreflight@1\n":
                raise ProbeFailure("HOST_TMP_IO_FAILED", "tmp")
            os.fsync(handle.fileno())
        renamed.unlink()
        probe_dir.rmdir()
        probe_dir = None
        result = {"status": "passed", "operations": ["create", "write", "fsync", "rename", "delete"]}
    except ProbeFailure as error:
        failure = error
    except (OSError, ValueError):
        failure = ProbeFailure("HOST_TMP_UNWRITABLE", "tmp")
    finally:
        if probe_dir is not None:
            try:
                shutil.rmtree(probe_dir)
            except OSError as error:
                # Do not let cleanup obscure the operation that originally
                # failed.  Conversely, a cleanup-only residue remains a hard
                # host failure rather than a silently ignored directory.
                if failure is None:
                    failure = ProbeFailure(
                        "HOST_TMP_CLEANUP_RESIDUE",
                        "tmp",
                        details={"cleanup_status": "residue", "cleanup_residue": True},
                    )
                else:
                    failure.details.update({"cleanup_status": "residue", "cleanup_residue": True})
    if failure is not None:
        raise failure
    assert result is not None
    return result


def _filesystem_errno_class(error: OSError) -> str:
    if error.errno in {errno.ENFILE, errno.EMFILE}:
        return "file_table_exhausted"
    if error.errno == errno.ENOSPC:
        return "disk_full"
    if error.errno == errno.EIO:
        return "io_error"
    if error.errno == errno.EROFS:
        return "read_only"
    return "operation_error"


def _capacity_probe_failure(root_label: str, operation: str, error: Optional[OSError] = None) -> FilesystemCapacityFailure:
    return FilesystemCapacityFailure(
        root_label=root_label,
        operation=operation,
        errno_class=_filesystem_errno_class(error) if error is not None else "operation_error",
    )


def _capacity_payload(payload_bytes: int) -> bytes:
    seed = b"ForgeCADFilesystemCapacity@1\n"
    return seed[:payload_bytes] if payload_bytes <= len(seed) else seed + b"." * (payload_bytes - len(seed))


def _capacity_worker(probe_dir_text: str, file_count: int, payload_bytes: int, deadline_seconds: float, result_queue: Any) -> None:
    """Child-only filesystem work; parent owns the true wall-clock timeout."""

    probe_dir = Path(probe_dir_text)
    payload = _capacity_payload(payload_bytes)
    deadline = time.monotonic() + deadline_seconds
    operation = "create_probe_root"
    completed = 0
    try:
        probe_dir.mkdir()
        # Phase A intentionally keeps every file alive together: this is the
        # vnode capacity proof, not 64 sequential one-file transactions.
        operation = "phase_a_create_write_fsync"
        for index in range(file_count):
            if time.monotonic() > deadline:
                raise TimeoutError
            bucket = probe_dir / ("bucket-%02d" % (index % 8))
            bucket.mkdir(exist_ok=True)
            source = bucket / ("item-%03d.source" % index)
            with source.open("wb") as handle:
                handle.write(payload)
                handle.flush()
                os.fsync(handle.fileno())
        # Phase B verifies each retained file, then removes it.
        operation = "phase_b_atomic_rename_read"
        for index in range(file_count):
            if time.monotonic() > deadline:
                raise TimeoutError
            bucket = probe_dir / ("bucket-%02d" % (index % 8))
            source = bucket / ("item-%03d.source" % index)
            renamed = bucket / ("item-%03d.renamed" % index)
            os.replace(source, renamed)
            if renamed.stat().st_size != payload_bytes:
                result_queue.put({"status": "failed", "operation": "stat_size", "errno_class": "operation_error"})
                return
            with renamed.open("rb") as handle:
                if handle.read() != payload:
                    result_queue.put({"status": "failed", "operation": "reopen_read", "errno_class": "operation_error"})
                    return
            renamed.unlink()
            completed += 1
        operation = "phase_c_rmdir"
        if time.monotonic() > deadline:
            raise TimeoutError
        for bucket in probe_dir.iterdir():
            bucket.rmdir()
        probe_dir.rmdir()
        result_queue.put({"status": "passed", "completed_files": completed, "peak_files": file_count})
    except TimeoutError:
        result_queue.put({"status": "failed", "operation": operation, "errno_class": "timeout"})
    except OSError as error:
        result_queue.put({"status": "failed", "operation": operation, "errno_class": _filesystem_errno_class(error)})
    except BaseException:
        result_queue.put({"status": "failed", "operation": operation, "errno_class": "operation_error"})


def _capacity_context() -> Any:
    # ForgeCAD is macOS-first; fork lets the small child execute a top-level
    # local worker without importing user state or serializing paths to output.
    return multiprocessing.get_context("fork")


def _stop_worker_bounded(process: Any, *, join_seconds: float = 0.25) -> str:
    """Stop a child without ever blocking the host gate indefinitely."""

    if not process.is_alive():
        return "exited"
    process.terminate()
    process.join(join_seconds)
    if not process.is_alive():
        return "terminated"
    process.kill()
    process.join(join_seconds)
    return "killed" if not process.is_alive() else "residue"


def _close_queue_safely(result_queue: Any) -> None:
    """Do not wait for a feeder thread after the worker deadline has expired."""

    try:
        result_queue.cancel_join_thread()
    except (AttributeError, OSError):
        pass
    try:
        result_queue.close()
    except (AttributeError, OSError):
        pass


def probe_filesystem_capacity(
    root: Path,
    *,
    root_label: str,
    file_count: int = 64,
    payload_bytes: int = 4096,
    deadline_seconds: float = 2.0,
) -> Dict[str, Any]:
    """Prove bounded filesystem capacity with a killable child process."""

    if file_count < 1 or payload_bytes < 1 or file_count * payload_bytes > 1024 * 1024 or deadline_seconds <= 0:
        raise ValueError("filesystem capacity probe bounds are invalid")
    if not root.is_dir():
        raise _capacity_probe_failure(root_label, "root_precondition")
    probe_dir = root / ("forgecad-capacity-" + uuid.uuid4().hex)
    context = _capacity_context()
    result_queue = context.Queue(maxsize=1)
    process = context.Process(target=_capacity_worker, args=(str(probe_dir), file_count, payload_bytes, deadline_seconds, result_queue))
    failure: Optional[FilesystemCapacityFailure] = None
    result: Optional[Dict[str, Any]] = None
    cleanup_status = "not_needed"
    worker_cleanup_status = "not_needed"
    worker_process_residue = False
    try:
        process.start()
        process.join(deadline_seconds)
        if process.is_alive():
            worker_cleanup_status = _stop_worker_bounded(process)
            worker_process_residue = worker_cleanup_status == "residue"
            failure = FilesystemCapacityFailure(
                root_label=root_label,
                operation="deadline",
                errno_class="timeout",
                worker_cleanup_status=worker_cleanup_status,
                worker_process_residue=worker_process_residue,
            )
        else:
            try:
                child_result = result_queue.get(timeout=0.25)
            except Exception:
                child_result = {"status": "failed", "operation": "worker_exit", "errno_class": "operation_error"}
            if child_result.get("status") != "passed":
                failure = FilesystemCapacityFailure(
                    root_label=root_label,
                    operation=str(child_result.get("operation", "worker_exit")),
                    errno_class=str(child_result.get("errno_class", "operation_error")),
                )
            else:
                result = child_result
    except OSError as error:
        failure = _capacity_probe_failure(root_label, "worker_start", error)
    finally:
        if process.is_alive():
            worker_cleanup_status = _stop_worker_bounded(process)
            worker_process_residue = worker_cleanup_status == "residue"
        if worker_process_residue:
            # A still-live child may be using the tree.  Never claim it was
            # cleaned or delete it underneath that process.
            cleanup_status = "not_attempted_worker_residue"
            if failure is None:
                failure = FilesystemCapacityFailure(
                    root_label=root_label,
                    operation="worker_shutdown",
                    errno_class="operation_error",
                    cleanup_status=cleanup_status,
                    cleanup_residue=True,
                    worker_cleanup_status=worker_cleanup_status,
                    worker_process_residue=True,
                )
            else:
                failure.cleanup_status = cleanup_status
                failure.cleanup_residue = True
                failure.worker_cleanup_status = worker_cleanup_status
                failure.worker_process_residue = True
        else:
            try:
                if probe_dir.exists():
                    shutil.rmtree(probe_dir)
                    cleanup_status = "cleaned"
            except OSError:
                cleanup_status = "residue"
                if failure is None:
                    failure = FilesystemCapacityFailure(root_label=root_label, operation="cleanup", errno_class="cleanup_residue", cleanup_status=cleanup_status, cleanup_residue=True, worker_cleanup_status=worker_cleanup_status, worker_process_residue=False)
                else:
                    failure.cleanup_status = cleanup_status
                    failure.cleanup_residue = True
                    failure.worker_cleanup_status = worker_cleanup_status
                    failure.worker_process_residue = False
        if not process.is_alive():
            try:
                process.close()
            except (AttributeError, OSError, ValueError):
                pass
        _close_queue_safely(result_queue)
    if failure is not None:
        raise failure
    assert result is not None
    return {
        "root_label": root_label,
        "status": "passed",
        "file_count": file_count,
        "peak_files": int(result["peak_files"]),
        "payload_bytes": payload_bytes,
        "total_bytes": file_count * payload_bytes,
        "deadline_seconds": deadline_seconds,
        "operations": ["create", "write", "fsync", "close", "reopen", "read", "stat", "atomic_rename", "unlink", "rmdir"],
        "completed_files": int(result["completed_files"]),
        "cleanup_status": cleanup_status,
        "worker_cleanup_status": worker_cleanup_status,
        "worker_process_residue": worker_process_residue,
    }


def _port_is_occupied(port: int) -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.settimeout(0.15)
        return sock.connect_ex(("127.0.0.1", port)) == 0


def _lookup_port_owners(port: int) -> List[Dict[str, Any]]:
    executable = _resolve_command("lsof")
    if not executable:
        return []
    try:
        completed = subprocess.run(
            [executable, "-nP", "-a", "-iTCP:%d" % port, "-sTCP:LISTEN", "-Fpc"],
            check=False,
            capture_output=True,
            text=True,
            timeout=0.75,
        )
    except (OSError, subprocess.SubprocessError):
        return []
    owners: List[Dict[str, Any]] = []
    command: Optional[str] = None
    pid: Optional[int] = None
    for line in completed.stdout.splitlines():
        if line.startswith("p") and line[1:].isdigit():
            pid = int(line[1:])
        elif line.startswith("c") and line[1:]:
            command = line[1:][:96]
        if pid is not None and command is not None:
            owners.append({"pid": pid, "command": command})
            pid = None
            command = None
    return owners


def _process_name_map() -> Dict[str, str]:
    """Return short process names only; never capture command-line arguments."""

    try:
        completed = subprocess.run(
            ["/bin/ps", "-axo", "pid=,ucomm="],
            check=False,
            capture_output=True,
            text=True,
            timeout=1.0,
        )
    except (OSError, subprocess.SubprocessError):
        return {}
    result: Dict[str, str] = {}
    for line in completed.stdout.splitlines():
        parts = line.strip().split(None, 1)
        if len(parts) == 2 and parts[0].isdigit():
            result[parts[0]] = parts[1][:96]
    return result


def _holder_category(process_name: str, lsof_name: str) -> str:
    value = (process_name + " " + lsof_name).lower()
    if "syspolicyd" in value:
        return "syspolicyd"
    if "launchservicesd" in value:
        return "launchservicesd"
    if "chrome" in value:
        return "chrome"
    if any(token in value for token in ("forgecad", "wushen", "cargo", "rustc", "playwright", "vite", "esbuild")):
        return "forgecad_or_test_tool"
    if any(token in value for token in ("node", "npm", "python", "pytest")):
        return "dev_runtime_or_test"
    return "system_or_other"


def _collect_holder_summary(limit: int) -> Dict[str, Any]:
    """Count visible lsof records without returning file names or full paths."""

    executable = _resolve_command("lsof")
    if not executable:
        return {"status": "unavailable", "source": "lsof", "visibility": "not_available", "holders": []}
    process_names = _process_name_map()
    counts: Dict[str, int] = {}
    lsof_names: Dict[str, str] = {}
    try:
        process = subprocess.Popen(
            [executable, "-nP", "-w", "+c", "0", "-Fpcf"],
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
        assert process.stdout is not None
        current_pid = ""
        current_name = ""
        for line in process.stdout:
            if line.startswith("p"):
                current_pid = line[1:].strip()
            elif line.startswith("c"):
                current_name = line[1:].strip()[:96]
            elif line.startswith("f") and current_pid:
                counts[current_pid] = counts.get(current_pid, 0) + 1
                lsof_names[current_pid] = current_name
        return_code = process.wait()
    except (OSError, subprocess.SubprocessError):
        return {"status": "unavailable", "source": "lsof", "visibility": "not_available", "holders": []}
    holders = []
    for pid, count in sorted(counts.items(), key=lambda item: (-item[1], int(item[0]) if item[0].isdigit() else 0))[: max(1, limit)]:
        process_name = process_names.get(pid, lsof_names.get(pid, "unknown"))
        holders.append(
            {
                "pid": int(pid) if pid.isdigit() else None,
                "process_class": _holder_category(process_name, lsof_names.get(pid, "")),
                "open_file_count": count,
            }
        )
    known_processes = []
    for pid, process_name in sorted(process_names.items(), key=lambda item: int(item[0]) if item[0].isdigit() else 0):
        process_class = _holder_category(process_name, lsof_names.get(pid, ""))
        if process_class in {"forgecad_or_test_tool", "chrome", "syspolicyd", "launchservicesd"}:
            known_processes.append(
                {
                    "pid": int(pid) if pid.isdigit() else None,
                    "process_class": process_class,
                    "lsof_visible": pid in counts,
                }
            )
    return {
        "status": "available" if return_code == 0 else "partial",
        "source": "lsof",
        "visibility": "visible_user_scope_only",
        "system_process_visibility": "not_guaranteed_without_privileged_access",
        "holders": holders,
        "known_processes": known_processes[:50],
    }


def _recovery_guidance(holder_summary: Mapping[str, Any]) -> Dict[str, Any]:
    classes = {item.get("process_class") for item in holder_summary.get("holders", [])}
    if "forgecad_or_test_tool" in classes:
        return {
            "status": "task_residue_possible",
            "recommended_next_step": "wait_for_task_exit_then_rerun_host_preflight",
            "destructive_action": "none",
        }
    return {
        "status": "no_visible_task_residue" if holder_summary.get("status") != "unavailable" else "system_reclaim_or_restart_review",
        "recommended_next_step": "rerun_after_idle_then_normal_macos_restart_if_pressure_persists",
        "destructive_action": "none",
    }


def _collect_no_interference_sampling() -> Dict[str, Any]:
    """Check for active heavy ForgeCAD/test processes without exposing command lines."""

    try:
        completed = subprocess.run(
            ["/bin/ps", "-axo", "pid=,ppid=,ucomm=,command="],
            check=False,
            capture_output=True,
            text=True,
            timeout=1.0,
        )
    except (OSError, subprocess.SubprocessError):
        return {
            "schema_version": "ForgeCADHostNoInterferenceSampling@1",
            "phase": PHASE,
            "subsystem": "sampling",
            "stable_error_code": "HOST_SAMPLING_UNAVAILABLE",
            "no_interference": False,
            "status": "unavailable",
            "forgecad_process_attribution": {"status": "unknown", "processes": []},
        }
    records: List[Tuple[int, int, str, str]] = []
    parent_by_pid: Dict[int, int] = {}
    for line in completed.stdout.splitlines():
        parts = line.strip().split(None, 3)
        if len(parts) != 4 or not parts[0].isdigit() or not parts[1].isdigit():
            continue
        pid, ppid, executable, command = parts
        pid_number = int(pid)
        ppid_number = int(ppid)
        records.append((pid_number, ppid_number, executable, command))
        parent_by_pid[pid_number] = ppid_number

    ancestry = set()
    cursor = os.getpid()
    while cursor in parent_by_pid and cursor not in ancestry:
        ancestry.add(cursor)
        cursor = parent_by_pid[cursor]

    def process_class_for(executable: str, command: str) -> Optional[str]:
        """Classify only known heavy executables; command text is never evidence by itself."""

        executable_name = Path(executable).name.lower()
        if executable_name == "git":
            # The aggregate runner itself fingerprints source with git.  A git
            # argument mentioning a smoke script must not become an active
            # workbench process merely because it contains a matching token.
            return None
        lowered = command.lower()
        if executable_name in {"cargo", "rustc", "rustdoc"}:
            return "forgecad_cargo_or_rust"
        if executable_name in {"node", "nodejs", "playwright", "chromium", "chrome", "google chrome"}:
            if any(token in lowered for token in ("desktop:t002-workbench-e2e-scenarios", "smoke_workbench_e2e_scenarios", "playwright_chromiumdev_profile", "playwright")):
                return "playwright_or_workbench_test"
        if executable_name in {"python", "python3", "python.exe", "forgecad", "wushen"}:
            if "forgecad" in lowered or "wushen" in lowered:
                return "forgecad_runtime"
        return None

    observed: List[Dict[str, Any]] = []
    for pid_number, ppid_number, executable, command in records:
        if pid_number in ancestry:
            continue
        process_class = process_class_for(executable, command)
        if process_class:
            observed.append({"pid": pid_number, "ppid": ppid_number, "process_class": process_class, "observed_executable": Path(executable).name[:64]})
    return {
        "schema_version": "ForgeCADHostNoInterferenceSampling@1",
        "phase": PHASE,
        "subsystem": "sampling",
        "stable_error_code": "HOST_NO_INTERFERENCE_SAMPLING_CLEAR" if not observed else "HOST_SAMPLING_INTERFERENCE_ACTIVE",
        "no_interference": not observed,
        "status": "clear" if not observed else "active_heavy_processes",
        "forgecad_process_attribution": {
            "status": "none_observed" if not observed else "active_processes_observed",
            "processes": observed[:50],
            "basis": "short_executable_allowlist_and_redacted_classification_only",
            "excluded_probe_ancestry_count": len(ancestry),
        },
    }


def _sampling_resource_snapshot(resources: Mapping[str, Any]) -> Dict[str, Any]:
    system = resources.get("system", {})
    process = resources.get("process", {})
    files = system.get("files", {}) if isinstance(system, Mapping) else {}
    vnodes = system.get("vnodes", {}) if isinstance(system, Mapping) else {}
    return {
        "num_vnodes": vnodes.get("current"),
        "max_vnodes": vnodes.get("maximum"),
        "num_files": files.get("current"),
        "max_files": files.get("maximum"),
        "ulimit_nofile_soft": process.get("soft_limit"),
        "ulimit_nofile_hard": process.get("hard_limit"),
        "process_open_file_count": process.get("open_count"),
    }


def _inspect_fixed_port(port: int, expected_owner: Optional[str]) -> Dict[str, Any]:
    occupied = _port_is_occupied(port)
    owners = _lookup_port_owners(port) if occupied else []
    status = "available"
    if occupied:
        matches = [
            owner
            for owner in owners
            if expected_owner
            and (str(owner.get("pid")) == expected_owner or owner.get("command") == expected_owner)
        ]
        if matches:
            status = "expected_owner"
        elif expected_owner:
            status = "unexpected_owner"
        else:
            status = "occupied_owner_unconfigured"
    return {
        "policy": "fixed",
        "port": port,
        "occupied": occupied,
        "owners": owners,
        "expected_owner": expected_owner,
        "owner_status": status,
        "action": "observe_only_no_kill",
    }


def _inspect_dynamic_port() -> Dict[str, Any]:
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.bind(("127.0.0.1", 0))
            port = int(sock.getsockname()[1])
    except OSError:
        raise ProbeFailure("HOST_DYNAMIC_PORT_UNAVAILABLE", "ports")
    return {
        "policy": "dynamic",
        "port": port,
        "occupied": False,
        "owners": [],
        "expected_owner": None,
        "owner_status": "available_at_probe",
        "action": "observe_only_no_kill",
    }


def _classify_ratio(
    ratio: float,
    warning_ratio: float,
    hard_ratio: float,
    *,
    hard_code: str,
    warning_code: str,
    subsystem: str,
    check: str,
    details: Mapping[str, Any],
) -> Optional[Dict[str, Any]]:
    if ratio >= hard_ratio:
        return _finding(subsystem=subsystem, code=hard_code, severity="hard_fail", check=check, details=details)
    if ratio >= warning_ratio:
        return _finding(subsystem=subsystem, code=warning_code, severity="warning", check=check, details=details)
    return None


def _filesystem_capacity_roots(config: PreflightConfig) -> List[Tuple[str, Path]]:
    """Return writable runtime roots only; source/workspace is deliberately absent."""

    roots: List[Tuple[str, Path]] = [("tmp", config.tmp_dir)]
    if config.library is not None:
        roots.append(("library", config.library))
    return roots


def _check_filesystem_capacity(config: PreflightConfig) -> Tuple[Dict[str, Any], List[Dict[str, Any]]]:
    report: Dict[str, Any] = {"status": "running", "roots": []}
    findings: List[Dict[str, Any]] = []
    for root_label, root in _filesystem_capacity_roots(config):
        try:
            result = probe_filesystem_capacity(
                root,
                root_label=root_label,
                file_count=config.filesystem_probe_file_count,
                payload_bytes=config.filesystem_probe_payload_bytes,
                deadline_seconds=config.filesystem_probe_deadline_seconds,
            )
            report["roots"].append(result)
        except FilesystemCapacityFailure as failure:
            report["roots"].append(
                {
                    "root_label": failure.root_label,
                    "status": "failed",
                    "operation": failure.operation,
                    "errno_class": failure.errno_class,
                }
            )
            findings.append(
                _finding(
                    subsystem="resources",
                    code=failure.code,
                    severity="hard_fail",
                    check="filesystem_capacity",
                    details={
                        "corroborated": True,
                        "operation": failure.operation,
                        "errno_class": failure.errno_class,
                        "root_label": failure.root_label,
                        "cleanup_status": failure.cleanup_status,
                        "cleanup_residue": failure.cleanup_residue,
                        "worker_cleanup_status": failure.worker_cleanup_status,
                        "worker_process_residue": failure.worker_process_residue,
                    },
                )
            )
            break
        except (OSError, ValueError):
            # A malformed configuration or an unclassified filesystem failure
            # cannot be allowed to turn a saturated vnode counter into a pass.
            report["roots"].append(
                {
                    "root_label": root_label,
                    "status": "failed",
                    "operation": "capacity_probe",
                    "errno_class": "operation_error",
                }
            )
            findings.append(
                _finding(
                    subsystem="resources",
                    code="HOST_VNODES_PRESSURE_HARD",
                    severity="hard_fail",
                    check="filesystem_capacity",
                    details={
                        "corroborated": True,
                        "operation": "capacity_probe",
                        "errno_class": "operation_error",
                        "root_label": root_label,
                        "cleanup_status": "unknown",
                        "cleanup_residue": False,
                        "worker_cleanup_status": "unknown",
                        "worker_process_residue": False,
                    },
                )
            )
            break
    report["status"] = "passed" if not findings else "failed"
    return report, findings


def _check_resources(config: PreflightConfig) -> Tuple[Dict[str, Any], List[Dict[str, Any]]]:
    findings: List[Dict[str, Any]] = []
    resources_report: Dict[str, Any] = {"attribution": "host_resource_only", "process": {}, "system": {}}
    system_pressure = False
    vnode_hard_pressure = False
    try:
        fd = _fd_snapshot()
        resources_report["process"] = fd
        soft = fd.get("soft_limit")
        if isinstance(soft, int) and soft > 0:
            ratio = float(fd["open_count"]) / float(soft)
            resources_report["process"]["usage_ratio"] = round(ratio, 6)
            result = _classify_ratio(
                ratio,
                config.fd_warning_ratio,
                config.fd_hard_ratio,
                hard_code="HOST_FD_PRESSURE_HARD",
                warning_code="HOST_FD_PRESSURE_WARNING",
                subsystem="resources",
                check="process_file_descriptors",
                details={"open_count": fd["open_count"], "soft_limit": soft},
            )
            if result:
                findings.append(result)
    except ProbeFailure as failure:
        findings.append(_finding(subsystem=failure.subsystem, code=failure.code, severity=failure.severity, check="process_file_descriptors"))

    if platform.system() == "Darwin":
        for current_name, maximum_name, label in (("kern.num_files", "kern.maxfiles", "files"), ("kern.num_vnodes", "kern.maxvnodes", "vnodes")):
            current = _read_sysctl(current_name)
            maximum = _read_sysctl(maximum_name)
            resources_report["system"][label] = {"current": current, "maximum": maximum}
            if current is None or maximum is None or maximum <= 0:
                findings.append(
                    _finding(
                        subsystem="resources",
                        code="HOST_SYSTEM_RESOURCE_UNAVAILABLE",
                        severity="warning",
                        check=label,
                    )
                )
                continue
            ratio = float(current) / float(maximum)
            resources_report["system"][label]["usage_ratio"] = round(ratio, 6)
            if label == "vnodes":
                threshold_level = "hard" if ratio >= config.system_hard_ratio else "warning"
                if ratio >= config.system_warning_ratio:
                    findings.append(
                        _finding(
                            subsystem="resources",
                            code="HOST_VNODES_PRESSURE_WARNING",
                            severity="warning",
                            check=label,
                            details={
                                "current": current,
                                "maximum": maximum,
                                "attribution": "host_resource_only",
                                "metric_kind": "kernel_cache_proxy",
                                "threshold_level": threshold_level,
                                "requires_capability_probe": ratio >= config.system_hard_ratio,
                            },
                        )
                    )
                    system_pressure = True
                if ratio >= config.system_hard_ratio:
                    vnode_hard_pressure = True
                continue
            result = _classify_ratio(
                ratio,
                config.system_warning_ratio,
                config.system_hard_ratio,
                hard_code="HOST_%s_PRESSURE_HARD" % label.upper(),
                warning_code="HOST_%s_PRESSURE_WARNING" % label.upper(),
                subsystem="resources",
                check=label,
                details={"current": current, "maximum": maximum, "attribution": "host_resource_only"},
            )
            if result:
                findings.append(result)
                system_pressure = True
    else:
        resources_report["system"] = {"status": "not_applicable_non_macos"}
    if vnode_hard_pressure:
        probe_report, probe_findings = _check_filesystem_capacity(config)
        resources_report["filesystem_capacity_probe"] = probe_report
        findings.extend(probe_findings)
    else:
        resources_report["filesystem_capacity_probe"] = {"status": "not_needed", "roots": []}
    if system_pressure:
        holder_summary = _collect_holder_summary(config.holder_summary_limit)
        resources_report["holder_summary"] = holder_summary
        resources_report["recovery_guidance"] = _recovery_guidance(holder_summary)
    else:
        resources_report["holder_summary"] = {"status": "not_needed", "holders": []}
        resources_report["recovery_guidance"] = {"status": "not_needed", "destructive_action": "none"}
    return resources_report, findings


def _check_tools(config: PreflightConfig) -> Tuple[Dict[str, Any], List[Dict[str, Any]]]:
    findings: List[Dict[str, Any]] = []
    rust_toolchain = _rust_toolchain_preflight(config.workspace)
    report: Dict[str, Any] = {"required": {}, "venv": None, "rust_toolchain": rust_toolchain}
    for command in config.required_commands:
        resolved = _resolve_command(command)
        resolution = "path"
        if not resolved and command in ("rustc", "cargo"):
            wrapper_command = rust_toolchain["commands"].get(command, {})
            if wrapper_command.get("status") == "resolved_via_wrapper":
                resolved = wrapper_command.get("wrapper_path", "script/with_rust_toolchain.sh")
                resolution = "workspace_wrapper"
        report["required"][command] = {
            "status": "resolved" if resolved else "missing",
            "resolution": resolution,
            "location": "workspace_wrapper" if resolution == "workspace_wrapper" else "path" if resolved else "not_found",
        }
        if not resolved:
            findings.append(
                _finding(
                    subsystem="tools",
                    code="HOST_TOOL_MISSING",
                    severity="hard_fail",
                    check="required_command",
                    details={"command": command},
                )
            )
    if config.venv is not None:
        resolved = config.venv.resolve(strict=False)
        usable = resolved.is_file() and os.access(resolved, os.X_OK)
        report["venv"] = {"label": "configured_venv", "status": "resolved" if usable else "missing_or_not_executable"}
        if not usable:
            findings.append(
                _finding(
                    subsystem="tools",
                    code="HOST_VENV_UNRESOLVED",
                    severity="hard_fail",
                    check="python_venv",
                )
            )
    return report, findings


def _check_tmp(config: PreflightConfig) -> Tuple[Dict[str, Any], List[Dict[str, Any]]]:
    findings: List[Dict[str, Any]] = []
    report: Dict[str, Any] = {"root_label": "tmp", "free_bytes": None, "minimum_free_bytes": config.min_free_bytes}
    try:
        report["free_bytes"] = _disk_free_bytes(config.tmp_dir)
        if report["free_bytes"] < config.min_free_bytes:
            findings.append(
                _finding(
                    subsystem="tmp",
                    code="HOST_TMP_LOW_DISK",
                    severity="hard_fail",
                    check="tmp_disk_capacity",
                    details={"free_bytes": report["free_bytes"], "minimum_free_bytes": config.min_free_bytes},
                )
            )
        report["transaction"] = probe_tmp_transaction(config.tmp_dir)
    except ProbeFailure as failure:
        report["transaction"] = {"status": "failed"}
        findings.append(
            _finding(
                subsystem=failure.subsystem,
                code=failure.code,
                severity=failure.severity,
                check="tmp_transaction",
                details=failure.details,
            )
        )
    except OSError:
        report["transaction"] = {"status": "failed"}
        findings.append(_finding(subsystem="tmp", code="HOST_TMP_UNWRITABLE", severity="hard_fail", check="tmp_transaction"))
    return report, findings


def _path_entry(label: str, path: Optional[Path], config: PreflightConfig, *, directory: bool, executable: bool) -> Tuple[Dict[str, Any], List[Dict[str, Any]]]:
    if path is None:
        return {"status": "not_configured"}, []
    findings: List[Dict[str, Any]] = []
    entry: Dict[str, Any] = {"path_label": label, "status": "present"}
    if not path.exists():
        entry["status"] = "missing"
        findings.append(_finding(subsystem="paths", code="HOST_PATH_MISSING", severity="hard_fail", check=label))
        return entry, findings
    if directory and not path.is_dir():
        entry["status"] = "wrong_type"
        findings.append(_finding(subsystem="paths", code="HOST_PATH_WRONG_TYPE", severity="hard_fail", check=label))
        return entry, findings
    if not directory and not path.is_file():
        entry["status"] = "wrong_type"
        findings.append(_finding(subsystem="paths", code="HOST_PATH_WRONG_TYPE", severity="hard_fail", check=label))
        return entry, findings
    readable = os.access(path, os.R_OK)
    writable = os.access(path, os.W_OK) if directory else None
    executable_ok = os.access(path, os.X_OK) if executable else None
    entry["permissions"] = {"readable": readable, "writable": writable, "executable": executable_ok}
    if not readable:
        findings.append(_finding(subsystem="paths", code="HOST_PATH_UNREADABLE", severity="hard_fail", check=label))
    if directory and not writable:
        findings.append(_finding(subsystem="paths", code="HOST_PATH_NOT_WRITABLE", severity="hard_fail", check=label))
    if executable and not executable_ok:
        findings.append(_finding(subsystem="paths", code="HOST_PATH_NOT_EXECUTABLE", severity="hard_fail", check=label))
    expected = config.expected_sha256.get(str(path.resolve()))
    if expected and path.is_file() and readable:
        actual = _sha256_file(path)
        entry["fingerprint"] = {"algorithm": "sha256", "actual": actual, "expected": expected}
        if actual != expected:
            findings.append(_finding(subsystem="paths", code="HOST_PATH_FINGERPRINT_MISMATCH", severity="hard_fail", check=label))
    else:
        entry["fingerprint"] = {"status": "precondition_not_configured" if not expected else "not_applicable"}
    return entry, findings


def _check_paths(config: PreflightConfig) -> Tuple[Dict[str, Any], List[Dict[str, Any]]]:
    reports: Dict[str, Any] = {}
    findings: List[Dict[str, Any]] = []
    for label, path, directory, executable in (
        ("workspace", config.workspace, True, False),
        ("library", config.library, True, False),
        ("sidecar", config.sidecar, False, True),
        ("app_artifact", config.app_artifact, config.app_artifact is not None and config.app_artifact.suffix == ".app", True),
    ):
        entry, path_findings = _path_entry(label, path, config, directory=directory, executable=executable)
        reports[label] = entry
        findings.extend(path_findings)
    return reports, findings


def _check_ports(config: PreflightConfig) -> Tuple[Dict[str, Any], List[Dict[str, Any]]]:
    if config.dynamic_port:
        try:
            return _inspect_dynamic_port(), []
        except ProbeFailure as failure:
            return {}, [_finding(subsystem=failure.subsystem, code=failure.code, severity=failure.severity, check="dynamic_port")]
    if config.port is None:
        return {"policy": "not_configured", "action": "observe_only_no_kill"}, []
    report = _inspect_fixed_port(config.port, config.expected_owner)
    if not report["occupied"] or report["owner_status"] == "expected_owner":
        return report, []
    code = "HOST_PORT_OWNER_MISMATCH" if config.expected_owner else "HOST_PORT_OCCUPIED"
    return report, [_finding(subsystem="ports", code=code, severity="hard_fail", check="loopback_port", details={"port": config.port})]


def run_preflight(config: PreflightConfig) -> Dict[str, Any]:
    """Run all host checks without launching or terminating any process."""

    checks: Dict[str, Any] = {}
    findings: List[Dict[str, Any]] = []
    sampling = _collect_no_interference_sampling()
    if not sampling["no_interference"]:
        findings.append(
            _finding(
                subsystem="sampling",
                code=sampling["stable_error_code"],
                severity="warning",
                check="no_interference_sampling",
            )
        )
    for name, checker in (
        ("tools", _check_tools),
        ("tmp", _check_tmp),
        ("resources", _check_resources),
        ("ports", _check_ports),
        ("paths", _check_paths),
    ):
        check, check_findings = checker(config)
        checks[name] = check
        findings.extend(check_findings)
    sampling["resource_snapshot"] = _sampling_resource_snapshot(checks["resources"])

    hard = [item for item in findings if item["severity"] == "hard_fail"]
    warnings = [item for item in findings if item["severity"] == "warning"]
    if hard:
        exit_code = EXIT_HARD_FAIL
        stable_code = hard[0]["stable_error_code"]
        status = "hard_fail"
    elif warnings and config.warnings_fail:
        exit_code = EXIT_WARNING
        stable_code = "HOST_PREFLIGHT_WARNINGS"
        status = "warning_fail"
    elif warnings:
        exit_code = EXIT_OK
        stable_code = "HOST_PREFLIGHT_WARNINGS"
        status = "pass_with_warnings"
    else:
        exit_code = EXIT_OK
        stable_code = "HOST_PREFLIGHT_OK"
        status = "passed"
    return {
        "schema_version": SCHEMA_VERSION,
        "status": status,
        "ok": not hard and not (warnings and config.warnings_fail),
        "exit_code": exit_code,
        "phase": PHASE,
        "subsystem": "host",
        "stable_error_code": stable_code,
        "platform": platform.system(),
        "network_access": "disabled",
        "process_termination": "none",
        "no_interference_sampling": sampling,
        "checks": checks,
        "findings": findings,
        "redaction": {"secrets_collected": False, "request_content_collected": False},
    }


def _parse_expected_sha256(values: Iterable[str]) -> Dict[str, str]:
    result: Dict[str, str] = {}
    for value in values:
        if "=" not in value:
            raise ValueError("expected fingerprint must be PATH=SHA256")
        path_text, digest = value.split("=", 1)
        if len(digest) != 64 or any(character not in "0123456789abcdefABCDEF" for character in digest):
            raise ValueError("expected fingerprint must be SHA256")
        result[str(Path(path_text).resolve())] = digest.lower()
    return result


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="ForgeCAD local-only host preflight")
    parser.add_argument("--workspace", type=Path, default=Path.cwd())
    parser.add_argument("--library", type=Path, default=Path(os.environ["WUSHEN_LIBRARY_ROOT"]) if os.environ.get("WUSHEN_LIBRARY_ROOT") else None)
    parser.add_argument("--sidecar", type=Path, default=Path(os.environ["WUSHEN_SIDECAR_PATH"]) if os.environ.get("WUSHEN_SIDECAR_PATH") else None)
    parser.add_argument("--app-artifact", type=Path, default=Path(os.environ["WUSHEN_APP_ARTIFACT"]) if os.environ.get("WUSHEN_APP_ARTIFACT") else None)
    parser.add_argument("--expected-sha256", action="append", default=[])
    parser.add_argument("--tmp-dir", type=Path, default=Path(os.environ.get("TMPDIR", tempfile.gettempdir())))
    parser.add_argument("--min-free-bytes", type=int, default=DEFAULT_MIN_FREE_BYTES)
    port_group = parser.add_mutually_exclusive_group()
    port_group.add_argument("--port", type=int)
    port_group.add_argument("--dynamic-port", action="store_true")
    parser.add_argument("--expected-owner")
    parser.add_argument("--required-command", action="append", dest="required_commands")
    parser.add_argument("--no-default-commands", action="store_true")
    parser.add_argument("--venv", type=Path, default=None)
    parser.add_argument("--no-venv", action="store_true")
    parser.add_argument("--fd-warning-ratio", type=float, default=0.80)
    parser.add_argument("--fd-hard-ratio", type=float, default=0.95)
    parser.add_argument("--system-warning-ratio", type=float, default=0.80)
    parser.add_argument("--system-hard-ratio", type=float, default=0.98)
    parser.add_argument("--holder-summary-limit", type=int, default=12)
    parser.add_argument("--warnings-fail", action="store_true")
    return parser


def _config_from_args(args: argparse.Namespace) -> PreflightConfig:
    if args.min_free_bytes < 0:
        raise ValueError("minimum free bytes cannot be negative")
    if args.port is not None and not 1 <= args.port <= 65535:
        raise ValueError("port must be between 1 and 65535")
    if not 0 <= args.fd_warning_ratio <= args.fd_hard_ratio <= 1:
        raise ValueError("fd ratios must satisfy 0 <= warning <= hard <= 1")
    if not 0 <= args.system_warning_ratio <= args.system_hard_ratio <= 1:
        raise ValueError("system ratios must satisfy 0 <= warning <= hard <= 1")
    if args.holder_summary_limit < 1 or args.holder_summary_limit > 50:
        raise ValueError("holder summary limit must be between 1 and 50")
    commands: Tuple[str, ...]
    if args.required_commands is not None:
        commands = tuple(args.required_commands)
    elif args.no_default_commands:
        commands = ()
    else:
        commands = DEFAULT_REQUIRED_COMMANDS
    venv = None if args.no_venv else (args.venv or (args.workspace / ".venv" / "bin" / "python"))
    return PreflightConfig(
        workspace=args.workspace,
        library=args.library,
        sidecar=args.sidecar,
        app_artifact=args.app_artifact,
        expected_sha256=_parse_expected_sha256(args.expected_sha256),
        tmp_dir=args.tmp_dir,
        min_free_bytes=args.min_free_bytes,
        port=args.port,
        dynamic_port=args.dynamic_port,
        expected_owner=args.expected_owner,
        required_commands=commands,
        venv=venv,
        fd_warning_ratio=args.fd_warning_ratio,
        fd_hard_ratio=args.fd_hard_ratio,
        system_warning_ratio=args.system_warning_ratio,
        system_hard_ratio=args.system_hard_ratio,
        warnings_fail=args.warnings_fail,
        holder_summary_limit=args.holder_summary_limit,
    )


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = _build_parser()
    try:
        config = _config_from_args(parser.parse_args(argv))
        report = run_preflight(config)
    except (ValueError, OSError):
        report = {
            "schema_version": SCHEMA_VERSION,
            "status": "invalid_argument",
            "ok": False,
            "exit_code": EXIT_INVALID_ARGUMENT,
            "phase": PHASE,
            "subsystem": "host",
            "stable_error_code": "HOST_PREFLIGHT_INVALID_ARGUMENT",
            "platform": platform.system(),
            "network_access": "disabled",
            "process_termination": "none",
            "checks": {},
            "findings": [],
            "redaction": {"secrets_collected": False, "request_content_collected": False},
        }
    print(json.dumps(report, ensure_ascii=False, sort_keys=True, separators=(",", ":")))
    return int(report["exit_code"])


if __name__ == "__main__":
    sys.exit(main())
