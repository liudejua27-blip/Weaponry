#!/usr/bin/env python3
"""Static boundary and evidence check for the FGC-MCP004 transaction core."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(relative: str) -> str:
    path = ROOT / relative
    if not path.exists():
        raise SystemExit(f"MCP004 input missing: {relative}")
    return path.read_text(encoding="utf-8")


def main() -> int:
    migration = read("migrations-runtime-v1/0001_runtime.sql")
    for marker in (
        "prepared_object_sha256",
        "source_version_id",
        "quality_hard_gate_passed",
        "approval_receipts",
        "write_idempotency",
        "export_manifests",
    ):
        if marker not in migration:
            raise SystemExit(f"MCP004 migration marker missing: {marker}")

    runtime = read("apps/desktop/src-tauri/crates/forgecad-runtime/src/lib.rs")
    for marker in (
        "prepare_candidate",
        "prepare_diagnostic_candidate",
        "project_create",
        "prepare_restore",
        "confirm_restore",
        "prepare_export",
        "confirm_export",
        "confirm_candidate",
        "reject_candidate",
        "mark_candidate_quality",
        "CANDIDATE_HASH_MISMATCH",
        "STALE_BASE_VERSION",
        "APPROVAL_EXPIRED",
        "QUALITY_HARD_GATE_FAILED",
        "IDEMPOTENCY_KEY_REUSED",
    ):
        if marker not in runtime:
            raise SystemExit(f"MCP004 Runtime marker missing: {marker}")

    store = read("apps/desktop/src-tauri/crates/forgecad-store/src/lib.rs")
    for marker in (
        "insert_candidate_and_job",
        "write_idempotency",
        "transaction.commit()?",
        "cancel_job",
        "prepare_restore_candidate",
        "restore_confirm",
        "prepare_export",
        "confirm_export",
    ):
        if marker not in store:
            raise SystemExit(f"MCP004 Store marker missing: {marker}")

    mcp = read("apps/desktop/src-tauri/crates/forgecad-mcp/src/main.rs")
    for marker in (
        "FORGECAD_MCP_ENABLE_MCP004_WRITES",
        "MCP004_WRITE_TOOLS_DISABLED",
        "mcp004_write_opt_in",
        "Backend::AuthenticatedIpc",
        "tools_with_writes",
        '"candidate_prepare"',
        '"project_create"',
        '"candidate_confirm"',
        '"restore_confirm"',
        '"export_confirm"',
    ):
        if marker not in mcp:
            raise SystemExit(f"MCP004 wire adapter marker missing: {marker}")
    if "tools_with_writes(false)" not in mcp:
        raise SystemExit("MCP003 default stdio adapter must remain read-only")

    launcher = read("apps/desktop/src-tauri/crates/forgecad-runtime/src/bin/forgecad-runtime.rs")
    for marker in (
        "serve_forever",
        "--diagnostic-fixture",
        "fixture_scope",
        "ReadyFileGuard",
        "0o700",
        "0o600",
    ):
        if marker not in launcher:
            raise SystemExit(f"MCP004 Runtime launcher marker missing: {marker}")

    mcp_entrypoint = read("apps/desktop/src-tauri/crates/forgecad-mcp/src/main.rs")
    supervisor = read("apps/desktop/src-tauri/crates/forgecad-mcp/src/supervisor.rs")
    for marker in (
        "MvpSupervisor",
        "backend_from_environment",
        "runtime_supervisor.poll",
        "drop(supervisor)",
    ):
        if marker not in mcp_entrypoint:
            raise SystemExit(f"MCP004 MCP entrypoint marker missing: {marker}")
    for marker in (
        "RUNTIME_COMMAND_ENV",
        "RUNTIME_DATA_DIR_ENV",
        "MvpSupervisor",
        "Starting",
        "Ready",
        "Degraded",
        "Restarting",
        "Busy",
        "MAX_RESTARTS",
        "RUNTIME_BUSY",
        "existing_handoff",
        "0o700",
    ):
        if marker not in supervisor:
            raise SystemExit(f"MCP004 MVP supervisor marker missing: {marker}")
    if "forgecad-mcp-host" in mcp_entrypoint or "--diagnostic-fixture" in supervisor:
        raise SystemExit("MCP004 MVP entrypoint must not depend on the removed Host or fixture")
    for script in ("script/build_and_run.sh", "script/test_mcp004.sh", "script/verify_package.sh"):
        if not (ROOT / script).is_file():
            raise SystemExit(f"MCP004 lifecycle script missing: {script}")

    cli_probe = read("scripts/probe_mcp004_codex_cli.py")
    for marker in (
        "diagnostic-codex-cli-mcp004-write",
        "FORGECAD_MCP_ENABLE_MCP004_WRITES",
        "--approve-for-me",
        "production_file_export",
        "--viewer-command",
        "viewer_read_model",
    ):
        if marker not in cli_probe:
            raise SystemExit(f"MCP004 Codex CLI probe marker missing: {marker}")

    manifest = json.loads(read("packages/forgecad-contracts/manifest.json"))
    for schema in (
        "approval-receipt.schema.json",
        "candidate-confirm-result.schema.json",
        "candidate-reject-result.schema.json",
        "restore-prepare-result.schema.json",
        "restore-confirm-result.schema.json",
        "export-manifest.schema.json",
        "export-prepare-result.schema.json",
        "export-confirm-result.schema.json",
    ):
        if schema not in manifest.get("schemas", []):
            raise SystemExit(f"MCP004 schema is not in the contract manifest: {schema}")

    evidence = json.loads(read("docs/evidence/mcp004/manifest.json"))
    if evidence.get("task_id") != "FGC-MCP004":
        raise SystemExit("MCP004 evidence task id mismatch")
    if evidence.get("status") not in {"in_progress", "done"}:
        raise SystemExit("MCP004 evidence must remain in_progress or done")
    required_gates = evidence.get("required_gates", {})
    for gate in (
        "candidate_prepare",
        "job_durability",
        "confirm_atomicity",
        "reject_atomicity",
        "idempotency",
        "negative_paths",
        "mcp_wire_adapter",
        "authenticated_ipc",
    ):
        if required_gates.get(gate) != "PASS":
            raise SystemExit(f"MCP004 required gate is not PASS: {gate}")
    commands = evidence.get("commands", {})
    for gate in (
        "development_runtime_launcher",
        "mcp_builtin_supervisor",
        "runtime_process_lock_release",
        "diagnostic_fixture_repeat_startup",
        "packaged_mcp_resource_probe",
        "codex_cli_diagnostic_write_e2e",
        "viewer_read_model",
    ):
        allowed = {"PASS"}
        if gate == "packaged_mcp_resource_probe":
            allowed.add("NOT_RUN")
        if commands.get(gate, {}).get("status") not in allowed:
            expected = "PASS or NOT_RUN" if gate == "packaged_mcp_resource_probe" else "PASS"
            raise SystemExit(f"MCP004 diagnostic gate is not {expected}: {gate}")
    if commands.get("packaged_signing", {}).get("status") != "BLOCKED":
        raise SystemExit("MCP004 packaged signing must remain explicitly BLOCKED until codesign passes")
    if commands.get("macos_signing_diagnostic", {}).get("status") != "BLOCKED":
        raise SystemExit("MCP004 macOS signing diagnostic must remain explicitly BLOCKED")
    live_host_audit = commands.get("live_connected_host_write_audit", {})
    if live_host_audit.get("status") == "NOT_RUN":
        host_write = json.loads(read("docs/evidence/mcp004/host-write-e2e-not-run.json"))
        if host_write.get("status") != "NOT_RUN":
            raise SystemExit("MCP004 live Desktop write audit must remain NOT_RUN")
        session = host_write.get("current_connected_session", {})
        if session.get("runtime_status") != "alpha-mcp003":
            raise SystemExit("MCP004 NOT_RUN audit must identify the observed pre-MCP004 Runtime")
        if session.get("exposed_write_tools") != [] or session.get("write_transactions") is not False:
            raise SystemExit("MCP004 NOT_RUN audit must prove no write surface or write transaction")
        process_audit = host_write.get("live_process_audit", {})
        if process_audit.get("mcp004_host_supervisor") is not False or process_audit.get("write_request_sent") is not False:
            raise SystemExit("MCP004 NOT_RUN process audit must prove no MCP004 host or write request")
        observer_behavior = process_audit.get("observer_behavior", "")
        if "no request rewriting" not in observer_behavior or "synthetic messages" not in observer_behavior:
            raise SystemExit("MCP004 live observer audit must declare non-rewriting, non-synthetic behavior")
    elif live_host_audit.get("status") != "PASS":
        raise SystemExit("MCP004 live Desktop write audit must be PASS or explicitly NOT_RUN")
    print("ForgeCAD MCP004 transaction core boundary OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
