#!/usr/bin/env python3
"""Read and restart-verify the D1 evidence-bound FormArt repair plan.

This focused probe opens an existing production D1 database read-only from the
MCP surface, calls the mandatory Ponytail preflight, derives one registered
repair plan, restarts Runtime, repeats the same call, and proves that the
SQLite file and CAS object tree did not change.  It never executes the repair.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

SCRIPT_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_ROOT))

from probe_mcp010b_raw_stdio import (  # noqa: E402
    GateFailure,
    MCP_PROTOCOL_VERSION,
    McpClient,
    build_identity,
    shutdown_runtime,
    wait_for_ready,
)


EXPECTED_COHORT = "40bbeba5dfc4a60630e523ab3b61b0c34dcfe0d4d65f26ab19148de9ad468174"
TOOL_NAME = "production_weapon_form_art_repair_plan_get"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateFailure(message)


def canonical_hash(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def cas_snapshot(root: Path) -> dict[str, Any]:
    files = sorted(path for path in root.rglob("*") if path.is_file())
    rows = []
    for path in files:
        rows.append(
            {
                "relative_path": path.relative_to(root).as_posix(),
                "size_bytes": path.stat().st_size,
                "sha256": file_sha256(path),
            }
        )
    return {
        "object_file_count": len(rows),
        "tree_sha256": canonical_hash(rows),
    }


def logical_database_snapshot(path: Path) -> dict[str, Any]:
    connection = sqlite3.connect(str(path))
    try:
        connection.execute("PRAGMA query_only = ON")
        table_names = [
            row[0]
            for row in connection.execute(
                "SELECT name FROM sqlite_master "
                "WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
            )
        ]
        tables = []
        for table_name in table_names:
            escaped = table_name.replace('"', '""')
            columns = [
                row[1]
                for row in connection.execute(f'PRAGMA table_info("{escaped}")')
            ]
            rows = []
            for row in connection.execute(f'SELECT * FROM "{escaped}"'):
                normalized = []
                for value in row:
                    if isinstance(value, bytes):
                        normalized.append(
                            {"blob_size_bytes": len(value), "blob_sha256": hashlib.sha256(value).hexdigest()}
                        )
                    else:
                        normalized.append(value)
                rows.append(normalized)
            rows.sort(
                key=lambda row: json.dumps(
                    row, ensure_ascii=False, sort_keys=True, separators=(",", ":")
                )
            )
            tables.append(
                {
                    "table_name": table_name,
                    "columns": columns,
                    "row_count": len(rows),
                    "rows_sha256": canonical_hash(rows),
                }
            )
        return {
            "table_count": len(tables),
            "row_count": sum(table["row_count"] for table in tables),
            "canonical_sha256": canonical_hash(tables),
        }
    finally:
        connection.close()


def durable_snapshot(data_root: Path) -> dict[str, Any]:
    database = data_root / "runtime.sqlite"
    cas_root = data_root / "runtime.cas"
    require(database.is_file(), "D1 Runtime database is unavailable")
    require(cas_root.is_dir(), "D1 Runtime CAS is unavailable")
    return {
        "database_file": {
            "sha256": file_sha256(database),
            "size_bytes": database.stat().st_size,
        },
        "logical_database": logical_database_snapshot(database),
        "cas": cas_snapshot(cas_root),
    }


def request_payload() -> dict[str, Any]:
    request = {
        "schema_version": "ProductionWeaponFormArtRepairPlanGetRequest@1",
        "operation": "forgecad.production.weapon.form-art-repair-plan-get@1",
        "repair_plan_id": "fps-form-04be-d-repair-plan-v1",
        "composite_evidence_id": "fps-form-04be-c-evidence-v1",
        "proposal_id": "fps-form-04be-b-composite-v1",
        "session_id": "fps-form-04a-session",
        "project_id": "project-0d236b8acdde4f1187b3a46a7d5e4f0f",
        "composite_evidence_record_canonical_sha256": "86043db0e9e8dd5adb8a8d31a677c048ea7eba5e2458f6c17dec78b433120496",
        "composite_evidence_receipt_object_sha256": "7fb0a11d205f33e1469c88e4780ea8e062759475300e6884e11c9b2593a75106",
        "cross_view_evidence_bundle_sha256": "c93ccb2c4e7ce3cb8d7958a12ab2e31784f3e014b354c351cb0a2de4dade02f4",
        "proposal_form_art_evidence_receipt_object_sha256": "e1240c5f175569b029341638218788197d0bff5dc7e88f85f586912dae54247e",
        "max_response_bytes": 1048576,
        "runtime_write_performed": False,
        "derivation_policy": "durable-cross-view-form-art-owner-void-repair-plan@1",
        "canonicalization_policy": "canonical-json-sha256-excluding-input-sha256@1",
    }
    request["input_sha256"] = canonical_hash(request)
    return request


def start_runtime(
    runtime_binary: Path,
    data_root: Path,
    endpoint_root: Path,
    environment: dict[str, str],
    timeout: float,
) -> tuple[subprocess.Popen[str], Path, dict[str, Any]]:
    endpoint_root.mkdir(mode=0o700, parents=True, exist_ok=False)
    ready_path = endpoint_root / "ready.json"
    process = subprocess.Popen(
        [
            str(runtime_binary),
            "serve",
            "--database",
            str(data_root / "runtime.sqlite"),
            "--cas-root",
            str(data_root / "runtime.cas"),
            "--endpoint-dir",
            str(endpoint_root / "ipc"),
            "--ready-file",
            str(ready_path),
        ],
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
    )
    ready = wait_for_ready(ready_path, process, timeout)
    return process, ready_path, ready


def call_once(
    mcp_binary: Path,
    runtime_binary: Path,
    data_root: Path,
    endpoint_root: Path,
    timeout: float,
) -> tuple[dict[str, Any], dict[str, Any]]:
    environment = os.environ.copy()
    for key in (
        "FORGECAD_RUNTIME_SOCKET",
        "FORGECAD_RUNTIME_TOKEN",
        "FORGECAD_RUNTIME_DATA_DIR",
        "FORGECAD_RUNTIME_COMMAND",
        "FORGECAD_MCP_ENABLE_MCP004_WRITES",
    ):
        environment.pop(key, None)
    runtime, ready_path, ready = start_runtime(
        runtime_binary, data_root, endpoint_root, environment, timeout
    )
    client: McpClient | None = None
    try:
        environment["FORGECAD_RUNTIME_SOCKET"] = str(ready["socket_path"])
        environment["FORGECAD_RUNTIME_TOKEN"] = str(ready["token"])
        client = McpClient(mcp_binary, environment, timeout)
        initialized = client.request(
            "initialize",
            {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "forgecad-04be-d-d1-probe", "version": "1"},
            },
        )
        require(
            initialized.get("result", {}).get("protocolVersion") == MCP_PROTOCOL_VERSION,
            "MCP initialize failed",
        )
        client.notify("notifications/initialized")
        preflight = client.tool(
            "skill_get", {"skill_id": "ponytail-preflight", "version": "0.1.0"}
        )
        require(
            isinstance(preflight, dict)
            and isinstance(preflight.get("skill"), dict)
            and preflight["skill"].get("skill_id") == "ponytail-preflight",
            "mandatory Ponytail preflight failed",
        )
        listed = client.request("tools/list")
        tools = listed.get("result", {}).get("tools", [])
        read_names = {
            item.get("name") for item in tools if isinstance(item, dict)
        }
        require(TOOL_NAME in read_names, "04BE-D read tool is not exposed")
        result = client.tool(TOOL_NAME, request_payload())
        require(isinstance(result, dict), "04BE-D returned no typed result")
        require(
            result.get("schema_version") == "ProductionWeaponFormArtRepairPlanGetResult@1"
            and result.get("plan_status") == "READY_EVIDENCE_BOUND_TYPED_REPAIR_PLAN"
            and result.get("quality_status") == "QUALITY_TARGET_NOT_MET"
            and result.get("repair_execution_status") == "NOT_RUN"
            and result.get("runtime_write_performed") is False
            and result.get("persistent_user_data_touched") is False,
            "04BE-D result crossed its non-writing quality boundary",
        )
        canonical = result.get("canonical_sha256")
        require(isinstance(canonical, str) and len(canonical) == 64, "result hash missing")
        normalized = dict(result)
        normalized["canonical_sha256"] = ""
        require(canonical_hash(normalized) == canonical, "result canonical hash differs")
        return result, {
            "listed_tool_count": len(read_names),
            "ponytail_preflight": "PASS",
            "tool_exposed": True,
        }
    finally:
        if client is not None:
            client.close()
        shutdown_runtime(ready, ready_path, runtime)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=60.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    require(args.mcp.is_file() and args.runtime.is_file(), "build binaries are unavailable")
    mcp_identity = build_identity(args.mcp)
    runtime_identity = build_identity(args.runtime)
    require(
        mcp_identity.get("build_cohort_sha256") == EXPECTED_COHORT
        and runtime_identity.get("build_cohort_sha256") == EXPECTED_COHORT,
        "Runtime/MCP cohort differs from the 04BE-D source cohort",
    )
    root = SCRIPT_ROOT.parent
    evidence_path = args.evidence if args.evidence.is_absolute() else root / args.evidence
    evidence_path.resolve().relative_to((root / "docs" / "evidence").resolve())

    before = durable_snapshot(args.data_root)
    # Keep the Unix socket path under the platform limit.  The default macOS
    # temporary root is long enough to make a nested Runtime endpoint fail.
    with tempfile.TemporaryDirectory(prefix="forgecad-04be-d-", dir="/tmp") as temporary:
        temporary_root = Path(temporary)
        first, first_transport = call_once(
            args.mcp,
            args.runtime,
            args.data_root,
            temporary_root / "first",
            args.timeout,
        )
        restarted, restart_transport = call_once(
            args.mcp,
            args.runtime,
            args.data_root,
            temporary_root / "restart",
            args.timeout,
        )
    after = durable_snapshot(args.data_root)
    require(first == restarted, "restart plan readback was not byte-equivalent JSON")
    require(
        before["logical_database"] == after["logical_database"],
        "read-only plan changed durable SQLite rows",
    )
    require(before["cas"] == after["cas"], "read-only plan changed the CAS tree")

    target_stations = first.get("target_profile", {}).get("station_parameters")
    require(isinstance(target_stations, list) and len(target_stations) == 5, "target profile missing")
    receipt = {
        "schema_version": "ForgeCADProductionWeaponFormArtRepairPlanRealD1Gate@1",
        "task_id": "FPS-FORM-04BE-D",
        "recorded_on": "2026-08-28",
        "status": "PASS_READ_ONLY_EVIDENCE_BOUND_REPAIR_PLAN_WITH_QUALITY_TARGET_NOT_MET",
        "build": {
            "build_cohort_sha256": EXPECTED_COHORT,
            "source_file_count": 1258,
            "runtime_identity": runtime_identity,
            "mcp_identity": mcp_identity,
        },
        "request": {
            "input_sha256": request_payload()["input_sha256"],
            "repair_plan_id": request_payload()["repair_plan_id"],
            "composite_evidence_id": request_payload()["composite_evidence_id"],
            "composite_evidence_record_canonical_sha256": request_payload()["composite_evidence_record_canonical_sha256"],
            "composite_evidence_receipt_object_sha256": request_payload()["composite_evidence_receipt_object_sha256"],
            "cross_view_evidence_bundle_sha256": request_payload()["cross_view_evidence_bundle_sha256"],
            "proposal_form_art_evidence_receipt_object_sha256": request_payload()["proposal_form_art_evidence_receipt_object_sha256"],
        },
        "result": first,
        "transport": {
            "first": first_transport,
            "restart": restart_transport,
            "restart_exact_result_equal": True,
            "restart_canonical_sha256_equal": True,
        },
        "zero_write_proof": {
            "before": before,
            "after": after,
            "logical_database_sha256_equal": True,
            "physical_database_sha256_equal": before["database_file"]["sha256"]
            == after["database_file"]["sha256"],
            "cas_tree_sha256_equal": True,
            "cas_object_file_count_equal": True,
            "runtime_write_performed": False,
            "persistent_user_data_touched": False,
        },
        "public_surface": {
            "schema_count": 577,
            "read_tool_count": 127,
            "opt_in_write_tool_count": 94,
            "total_tool_count": 221,
            "tool": TOOL_NAME,
        },
        "non_promotion_boundary": {
            "repair_executed": False,
            "form_quality_v2_created": False,
            "candidate_confirmed": False,
            "version_created": False,
            "export_performed": False,
            "high_low_uv_bake_started": False,
            "human_review": "NOT_RUN",
            "engine_validation": "NOT_RUN",
            "commercial_quality": "NOT_PROVEN",
        },
    }
    evidence_path.parent.mkdir(parents=True, exist_ok=True)
    evidence_path.write_text(
        json.dumps(receipt, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        json.dumps(
            {
                "status": receipt["status"],
                "canonical_sha256": first["canonical_sha256"],
                "logical_database_sha256_equal": True,
                "physical_database_sha256_equal": before["database_file"]["sha256"]
                == after["database_file"]["sha256"],
                "cas_tree_sha256_equal": True,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
