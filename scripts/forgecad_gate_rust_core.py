#!/usr/bin/env python3
"""Layer 2: isolated, pure-Rust ForgeCAD Core gate.

This runner owns only the ``forgecad-core`` Cargo package. It does not invoke
K001/K002 gates, Python, Tauri, WebView, packaged builds, network services, or
the desktop workspace test package. The Rust tests remain the source of truth;
the Python layer only groups named tests and emits a bounded diagnostic report.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
from typing import Any, Callable, Iterable, Sequence


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
RUST_WRAPPER = ROOT / "script" / "with_rust_toolchain.sh"
PACKAGE = "forgecad-core"
REPORT_SCHEMA = "ForgeCADRustCoreGateReport@1"
MAX_DIAGNOSTIC_CHARS = 640
DEFAULT_TIMEOUT_SECONDS = 900
DEFAULT_REPORT_PATH = ROOT / "build" / "verification" / "forgecad-rust-core-gate.json"

EXIT_PASS = 0
EXIT_USAGE = 2
EXIT_MISSING_TOOL = 3
EXIT_TIMEOUT = 4
EXIT_FACET_FAILED = 5
EXIT_INTERNAL = 6

_FORBIDDEN_COMMAND_TOKENS = {
    "k001",
    "k002",
    "python",
    "pytest",
    "webview",
    "packaged",
    "wushen-forge-desktop",
}


@dataclass(frozen=True)
class FacetSpec:
    facet_id: str
    subsystem: str
    command: list[str]
    required_tests: tuple[str, ...] = ()


@dataclass(frozen=True)
class CommandResult:
    status: str
    returncode: int | None
    stdout: str
    stderr: str
    duration_ms: int
    stable_error_code: str | None = None
    diagnostic: dict[str, str] | None = None

    def to_report(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "status": "pass" if self.status == "pass" else "fail",
            "duration_ms": self.duration_ms,
            "return_code": self.returncode,
            "stable_error_code": self.stable_error_code,
        }
        if self.diagnostic:
            result["first_structured_reason"] = dict(self.diagnostic)
        return result


@dataclass(frozen=True)
class PreflightFailure:
    stable_error_code: str
    message: str
    exit_code: int

    def to_report(self) -> dict[str, Any]:
        return {
            "schema_version": REPORT_SCHEMA,
            "layer": 2,
            "phase": "preflight",
            "subsystem": PACKAGE,
            "status": "fail",
            "exit_code": self.exit_code,
            "stable_error_code": self.stable_error_code,
            "facets": [],
            "first_structured_reason": {
                "stable_error_code": self.stable_error_code,
                "message": self.message,
            },
        }


class InvalidCommand(ValueError):
    """Raised when a command would escape the pure Core gate boundary."""


def _bounded(value: str, limit: int = MAX_DIAGNOSTIC_CHARS) -> str:
    value = value.replace("\x00", " ").strip()
    if len(value) <= limit:
        return value
    return value[: limit - 1].rstrip() + "…"


def _safe_message(value: Any) -> str:
    text = _bounded(str(value))
    if re.search(r"(?i)(prompt|request_body|authorization|api[_-]?key|secret|token)", text):
        return "structured command failure (sensitive field omitted)"
    return text


def first_structured_reason(output: str) -> dict[str, str]:
    """Return only the first bounded, non-sensitive structured failure reason."""

    fallback: str | None = None
    for line in output.splitlines():
        candidate = line.strip()
        if not candidate:
            continue
        if fallback is None:
            fallback = candidate
        try:
            value = json.loads(candidate)
        except json.JSONDecodeError:
            continue
        if not isinstance(value, dict):
            continue
        code = value.get("stable_error_code") or value.get("error_code") or value.get("code")
        message = value.get("message") or value.get("reason") or value.get("error")
        if code is None and message is None:
            continue
        stable_code = _bounded(str(code or "STRUCTURED_COMMAND_FAILURE"), 96)
        if not re.fullmatch(r"[A-Za-z0-9_.:-]+", stable_code):
            stable_code = "STRUCTURED_COMMAND_FAILURE"
        return {
            "stable_error_code": stable_code,
            "message": _safe_message(message or "structured command failure"),
        }
    return {
        "stable_error_code": "COMMAND_FAILED",
        "message": _safe_message(fallback or "command failed without structured diagnostics"),
    }


def validate_core_only_command(command: Sequence[str]) -> None:
    """Reject commands that can recurse into another Gate or product layer."""

    joined = " ".join(str(part) for part in command).lower()
    command_parts = {Path(str(part)).name.lower() for part in command}
    if any(token in command_parts for token in _FORBIDDEN_COMMAND_TOKENS):
        raise InvalidCommand("command escapes the forgecad-core-only gate boundary")
    if any(
        re.search(rf"(?:^|[:/]){re.escape(token)}(?::|$)", joined)
        for token in ("k001", "k002", "webview")
    ):
        raise InvalidCommand("command escapes the forgecad-core-only gate boundary")
    if "cargo" not in {Path(str(part)).name.lower() for part in command}:
        raise InvalidCommand("core gate command must invoke cargo directly")
    if "-p" not in command or PACKAGE not in command:
        raise InvalidCommand("core gate command must target forgecad-core")
    if "--offline" not in command:
        raise InvalidCommand("core gate command must run offline")


def run_command(
    command: Sequence[str],
    *,
    timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> CommandResult:
    started = time.monotonic()
    try:
        completed = runner(
            list(command),
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
    except FileNotFoundError:
        return CommandResult(
            status="missing_tool",
            returncode=None,
            stdout="",
            stderr="",
            duration_ms=int((time.monotonic() - started) * 1000),
            stable_error_code="MISSING_TOOL",
            diagnostic={
                "stable_error_code": "MISSING_TOOL",
                "message": "required command executable was not found",
            },
        )
    except subprocess.TimeoutExpired as error:
        return CommandResult(
            status="timeout",
            returncode=None,
            stdout="",
            stderr="",
            duration_ms=int((time.monotonic() - started) * 1000),
            stable_error_code="COMMAND_TIMEOUT",
            diagnostic={
                "stable_error_code": "COMMAND_TIMEOUT",
                "message": f"command exceeded {timeout_seconds:g}s timeout",
            },
        )

    stdout = completed.stdout or ""
    stderr = completed.stderr or ""
    duration_ms = int((time.monotonic() - started) * 1000)
    if completed.returncode == 0:
        return CommandResult(
            status="pass",
            returncode=0,
            stdout=stdout,
            stderr=stderr,
            duration_ms=duration_ms,
        )
    diagnostic = first_structured_reason("\n".join((stdout, stderr)))
    return CommandResult(
        status="failed",
        returncode=completed.returncode,
        stdout=stdout,
        stderr=stderr,
        duration_ms=duration_ms,
        stable_error_code=diagnostic["stable_error_code"],
        diagnostic=diagnostic,
    )


def wrapper_cargo_available(
    wrapper: Path,
    *,
    probe_runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> bool:
    """Check the repository toolchain wrapper, allowing cargo to be wrapper-owned."""

    if shutil.which("cargo") is not None:
        return True
    try:
        probe = probe_runner(
            [str(wrapper), "cargo", "--version"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )
    except (FileNotFoundError, OSError, subprocess.TimeoutExpired):
        return False
    return probe.returncode == 0


def preflight(*, wrapper: Path = RUST_WRAPPER, manifest: Path = MANIFEST) -> PreflightFailure | None:
    if not wrapper.is_file() or not os.access(wrapper, os.X_OK):
        return PreflightFailure(
            "MISSING_RUNNER",
            "the Rust toolchain wrapper is missing or not executable",
            EXIT_MISSING_TOOL,
        )
    if not manifest.is_file():
        return PreflightFailure(
            "MISSING_MANIFEST",
            "the desktop Cargo manifest is missing",
            EXIT_MISSING_TOOL,
        )
    if not wrapper_cargo_available(wrapper):
        return PreflightFailure(
            "MISSING_TOOL",
            "the repository Rust wrapper could not execute cargo",
            EXIT_MISSING_TOOL,
        )
    return None


def _relative(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def _cargo_command(*cargo_args: str) -> list[str]:
    return [
        _relative(RUST_WRAPPER),
        "cargo",
        "test",
        "--manifest-path",
        _relative(MANIFEST),
        "-p",
        PACKAGE,
        "--offline",
        *cargo_args,
    ]


def _facet(facet_id: str, filter_value: str, required_tests: Iterable[str] = ()) -> FacetSpec:
    return FacetSpec(
        facet_id=facet_id,
        subsystem=PACKAGE,
        command=_cargo_command("--", filter_value),
        required_tests=tuple(required_tests),
    )


GATE_FACETS: tuple[FacetSpec, ...] = (
    FacetSpec("cargo_test_offline_full_suite", PACKAGE, _cargo_command()),
    _facet(
        "sqlite_migration_wal_restart",
        "migration::tests::",
        (
            "migration::tests::empty_database_reaches_current_schema_in_wal_mode",
            "migration::tests::historical_legacy_rows_keep_semantic_hash_after_upgrade",
            "migration::tests::interruption_rolls_back_schema_and_marker_then_retries_cleanly",
        ),
    ),
    _facet(
        "transaction_snapshot_cas_stale_etag",
        "repository::tests::stale_etag_rolls_back_without_partial_state",
        ("repository::tests::stale_etag_rolls_back_without_partial_state",),
    ),
    _facet(
        "immutable_version_undo_redo",
        "repository::tests::preview_confirm_and_immutable_undo_redo_are_atomic",
        ("repository::tests::preview_confirm_and_immutable_undo_redo_are_atomic",),
    ),
    _facet(
        "single_writer_fence",
        "ownership::tests::",
        (
            "ownership::tests::cutover_is_explicit_and_second_writer_is_rejected",
            "ownership::tests::unpublished_first_cutover_rolls_back_but_published_cutover_cannot",
        ),
    ),
    _facet(
        "durable_writer_epoch_recheck",
        "repository::tests::repository_rechecks_durable_writer_epoch_inside_every_transaction",
        ("repository::tests::repository_rechecks_durable_writer_epoch_inside_every_transaction",),
    ),
    _facet(
        "cas_object_integrity",
        "object_store::tests::",
        (
            "object_store::tests::staged_object_promotes_deterministically_and_reads_verified_bytes",
            "object_store::tests::cancelled_stage_is_removed_and_corruption_is_rejected",
        ),
    ),
    _facet(
        "journal_recovery",
        "deletion_journal",
        (
            "object_store::tests::pending_journal_recovers_promote_before_database_commit",
            "repository::tests::deletion_journal_recovers_crash_after_index_commit_without_scanning",
        ),
    ),
    _facet(
        "sqlite_backup_restore",
        "sqlite_checkpoint_and_cas_restore_is_readable",
        ("sqlite_checkpoint_and_cas_restore_is_readable",),
    ),
    _facet(
        "shape_program_persisted_normalization",
        "shape_program::tests::",
        (
            "shape_program::tests::k001_set_part_parameter_shape_program_persistence_normalization_is_idempotent",
            "shape_program::tests::normalized_shape_program_preserves_number_contract_and_is_idempotent",
        ),
    ),
    _facet(
        "artifact_readback",
        "artifact_readback::tests::",
        (
            "artifact_readback::tests::closed_manifold_readback_welds_split_normal_vertices",
            "artifact_readback::tests::open_surface_stays_rejected_after_vertex_welding",
        ),
    ),
    _facet(
        "candidate_and_changeset_atomic_bundles",
        "bundle_is_atomic_replayable_and_restart_readable",
        (
            "bundle_is_atomic_replayable_and_restart_readable_with_two_distinct_hashes",
            "preview_and_confirm_bundles_are_atomic_replayable_and_restart_readable",
        ),
    ),
)


def parse_test_list(output: str) -> set[str]:
    return {
        line.rsplit(": test", 1)[0].strip()
        for line in output.splitlines()
        if line.rstrip().endswith(": test")
    }


def missing_required_tests(available: set[str], facets: Iterable[FacetSpec]) -> list[str]:
    missing: list[str] = []
    for facet in facets:
        for expected in facet.required_tests:
            if not any(
                candidate == expected
                or candidate.endswith(f"::{expected}")
                or candidate.endswith(expected)
                for candidate in available
            ):
                missing.append(f"{facet.facet_id}:{expected}")
    return missing


def _facet_report(facet: FacetSpec, result: CommandResult) -> dict[str, Any]:
    report = {
        "facet": facet.facet_id,
        "phase": "core-tests",
        "subsystem": facet.subsystem,
        "command": list(facet.command),
        "status": "pass" if result.status == "pass" else "fail",
        "duration_ms": result.duration_ms,
        "stable_error_code": result.stable_error_code,
    }
    if result.diagnostic:
        report["first_structured_reason"] = dict(result.diagnostic)
    return report


def _not_run_report(facet: FacetSpec) -> dict[str, Any]:
    return {
        "facet": facet.facet_id,
        "phase": "core-tests",
        "subsystem": facet.subsystem,
        "command": list(facet.command),
        "status": "not_run",
        "duration_ms": 0,
        "stable_error_code": "NOT_RUN_AFTER_FAILURE",
    }


def execute_facets(
    facets: Sequence[FacetSpec],
    *,
    command_runner: Callable[[list[str]], CommandResult],
) -> dict[str, Any]:
    reports: list[dict[str, Any]] = []
    first_failure: dict[str, str] | None = None
    failed = False
    for index, facet in enumerate(facets):
        if failed:
            reports.extend(_not_run_report(remaining) for remaining in facets[index:])
            break
        result = command_runner(facet.command)
        diagnostic = result.diagnostic
        if result.status != "pass" and diagnostic is None:
            diagnostic = first_structured_reason("\n".join((result.stdout, result.stderr)))
        if diagnostic is not None and result.diagnostic is None:
            result = CommandResult(
                status=result.status,
                returncode=result.returncode,
                stdout=result.stdout,
                stderr=result.stderr,
                duration_ms=result.duration_ms,
                stable_error_code=diagnostic["stable_error_code"],
                diagnostic=diagnostic,
            )
        facet_report = _facet_report(facet, result)
        reports.append(facet_report)
        if result.status != "pass":
            failed = True
            first_failure = result.diagnostic or {
                "stable_error_code": result.stable_error_code or "COMMAND_FAILED",
                "message": "facet command failed",
            }
    return {
        "schema_version": REPORT_SCHEMA,
        "layer": 2,
        "phase": "gate",
        "subsystem": PACKAGE,
        "status": "fail" if failed else "pass",
        "exit_code": EXIT_FACET_FAILED if failed else EXIT_PASS,
        "stable_error_code": first_failure["stable_error_code"] if first_failure else None,
        "facets": reports,
        **({"first_structured_reason": first_failure} if first_failure else {}),
    }


def run_gate(*, timeout_seconds: float = DEFAULT_TIMEOUT_SECONDS) -> dict[str, Any]:
    failure = preflight()
    if failure:
        return failure.to_report()

    def command_runner(command: list[str]) -> CommandResult:
        validate_core_only_command(command)
        return run_command(command, timeout_seconds=timeout_seconds)

    inventory_facet = FacetSpec(
        "named_test_inventory",
        PACKAGE,
        _cargo_command("--", "--list"),
    )
    inventory_result = command_runner(inventory_facet.command)
    inventory_report = _facet_report(inventory_facet, inventory_result)
    if inventory_result.status != "pass":
        remaining = [_not_run_report(facet) for facet in GATE_FACETS]
        report = execute_facets([], command_runner=command_runner)
        report["facets"] = [inventory_report, *remaining]
        report["status"] = "fail"
        report["exit_code"] = EXIT_TIMEOUT if inventory_result.status == "timeout" else EXIT_FACET_FAILED
        report["stable_error_code"] = inventory_result.stable_error_code
        if inventory_result.diagnostic:
            report["first_structured_reason"] = inventory_result.diagnostic
        return report

    missing = missing_required_tests(parse_test_list(inventory_result.stdout), GATE_FACETS)
    if missing:
        reason = {
            "stable_error_code": "MISSING_ACCEPTANCE_TEST",
            "message": _bounded("required Core test disappeared: " + ", ".join(missing)),
        }
        return {
            "schema_version": REPORT_SCHEMA,
            "layer": 2,
            "phase": "gate",
            "subsystem": PACKAGE,
            "status": "fail",
            "exit_code": EXIT_FACET_FAILED,
            "stable_error_code": reason["stable_error_code"],
            "facets": [
                {**inventory_report, "status": "fail", "stable_error_code": reason["stable_error_code"], "first_structured_reason": reason},
                *[_not_run_report(facet) for facet in GATE_FACETS],
            ],
            "first_structured_reason": reason,
        }

    report = execute_facets(GATE_FACETS, command_runner=command_runner)
    report["facets"].insert(0, inventory_report)
    if report["status"] == "pass":
        report["stable_error_code"] = None
    return report


def _parse_args(argv: Sequence[str] | None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run the isolated ForgeCAD Rust Core gate")
    parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=DEFAULT_TIMEOUT_SECONDS,
        help="per-facet Cargo timeout",
    )
    parser.add_argument(
        "--report-path",
        type=Path,
        default=DEFAULT_REPORT_PATH,
        help="JSON report output path",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    try:
        report = run_gate(timeout_seconds=args.timeout_seconds)
    except InvalidCommand as error:
        report = PreflightFailure(
            "INVALID_GATE_COMMAND",
            str(error),
            EXIT_INTERNAL,
        ).to_report()
    except Exception as error:  # pragma: no cover - final fail-closed boundary
        report = PreflightFailure(
            "RUNNER_INTERNAL_ERROR",
            _safe_message(error),
            EXIT_INTERNAL,
        ).to_report()
    report_path = args.report_path
    if not report_path.is_absolute():
        report_path = ROOT / report_path
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )
    print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    return int(report["exit_code"])


if __name__ == "__main__":
    raise SystemExit(main())
