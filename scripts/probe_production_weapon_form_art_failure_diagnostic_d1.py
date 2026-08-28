#!/usr/bin/env python3
"""Read and restart-verify the exact 04BE-E FormArt failure diagnostic.

This focused evidence probe performs the mandatory Ponytail preflight, calls
the Runtime-owned read-only diagnostic against the existing D1 durable
candidate, restarts Runtime, calls the same diagnostic again and proves exact
canonical equality with no SQLite/CAS mutation.
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

PROJECT_ID = "project-0d236b8acdde4f1187b3a46a7d5e4f0f"
SESSION_ID = "fps-form-04a-session"
PROPOSAL_ID = "fps-form-04be-e-owner-void-half-y-v1"
EVIDENCE_ID = "fps-form-04be-e-owner-void-half-y-evidence-v1"
EVIDENCE_RECORD = "8f03de05bd08af1c54e62cfaad603204cef72f224ed47c12eabd8ec391eacd88"
EVIDENCE_RECEIPT = "350555facd3ea2ca5fe994652f56603c9b73c65842152afbbad91102c8cdb33e"
CROSS_VIEW = "64eb80b5866b32e7d08e1484edf9f036ee20b0cdc4ab9da409f953bacdd043ea"
FORM_ART = "0af3ded026923d2612560ba3e446e8c777887a3a4c2436385e83e8a110d15bff"
MAX_RESPONSE_BYTES = 1_048_576
DIAGNOSTIC_POLICY = "exact-parent-proposal-cross-view-form-art-delta-diagnostic@1"
CANONICALIZATION_POLICY = "canonical-json-sha256-excluding-input-sha256@1"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateFailure(message)


def canonical_hash(value: Any) -> str:
    return hashlib.sha256(
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    ).hexdigest()


def diagnostic_request() -> dict[str, Any]:
    value = {
        "schema_version": "ProductionWeaponFormArtFailureDiagnosticGetRequest@1",
        "operation": "forgecad.production.weapon.form-art-failure-diagnostic-get@1",
        "diagnostic_id": "fps-form-04be-f-failure-diagnostic-v1",
        "composite_evidence_id": EVIDENCE_ID,
        "proposal_id": PROPOSAL_ID,
        "session_id": SESSION_ID,
        "project_id": PROJECT_ID,
        "composite_evidence_record_canonical_sha256": EVIDENCE_RECORD,
        "composite_evidence_receipt_object_sha256": EVIDENCE_RECEIPT,
        "cross_view_evidence_bundle_sha256": CROSS_VIEW,
        "proposal_form_art_evidence_receipt_object_sha256": FORM_ART,
        "max_response_bytes": MAX_RESPONSE_BYTES,
        "runtime_write_performed": False,
        "persistent_user_data_touched": False,
        "diagnostic_policy": DIAGNOSTIC_POLICY,
        "canonicalization_policy": CANONICALIZATION_POLICY,
    }
    value["input_sha256"] = canonical_hash(value)
    return value


def sqlite_snapshot(path: Path) -> dict[str, Any]:
    connection = sqlite3.connect(str(path))
    try:
        connection.execute("PRAGMA query_only = ON")
        tables = [
            row[0]
            for row in connection.execute(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
            )
        ]
        counts = {
            table: connection.execute(f'SELECT COUNT(*) FROM "{table}"').fetchone()[0]
            for table in tables
        }
    finally:
        connection.close()
    return {"table_count": len(tables), "row_counts": counts}


def cas_snapshot(path: Path) -> dict[str, Any]:
    files = sorted(candidate for candidate in path.rglob("*") if candidate.is_file())
    digest = hashlib.sha256()
    for candidate in files:
        relative = candidate.relative_to(path).as_posix().encode("utf-8")
        digest.update(len(relative).to_bytes(4, "big"))
        digest.update(relative)
        digest.update(candidate.stat().st_size.to_bytes(8, "big"))
    return {"object_count": len(files), "path_size_index_sha256": digest.hexdigest()}


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
    return process, ready_path, wait_for_ready(ready_path, process, timeout)


def open_client(
    mcp_binary: Path,
    runtime_binary: Path,
    data_root: Path,
    endpoint_root: Path,
    timeout: float,
) -> tuple[subprocess.Popen[str], Path, dict[str, Any], McpClient]:
    environment = os.environ.copy()
    for key in (
        "FORGECAD_RUNTIME_SOCKET",
        "FORGECAD_RUNTIME_TOKEN",
        "FORGECAD_RUNTIME_DATA_DIR",
        "FORGECAD_RUNTIME_COMMAND",
    ):
        environment.pop(key, None)
    runtime, ready_path, ready = start_runtime(
        runtime_binary, data_root, endpoint_root, environment, timeout
    )
    environment["FORGECAD_RUNTIME_SOCKET"] = str(ready["socket_path"])
    environment["FORGECAD_RUNTIME_TOKEN"] = str(ready["token"])
    client = McpClient(mcp_binary, environment, timeout)
    initialized = client.request(
        "initialize",
        {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {"name": "forgecad-04be-f-d1-probe", "version": "1"},
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
        and preflight.get("skill", {}).get("skill_id") == "ponytail-preflight",
        "mandatory Ponytail preflight failed",
    )
    return runtime, ready_path, ready, client


def close_client(
    runtime: subprocess.Popen[str],
    ready_path: Path,
    ready: dict[str, Any],
    client: McpClient,
) -> None:
    client.close()
    shutdown_runtime(ready, ready_path, runtime)
    if runtime.stderr is not None:
        diagnostic = runtime.stderr.read().strip()
        if diagnostic:
            print(diagnostic, file=sys.stderr)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mcp", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--expected-build-cohort", required=True)
    parser.add_argument("--timeout", type=float, default=180.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    require(args.mcp.is_file() and args.runtime.is_file(), "build binaries are unavailable")
    identities = {"mcp": build_identity(args.mcp), "runtime": build_identity(args.runtime)}
    require(
        all(
            identity.get("build_cohort_sha256") == args.expected_build_cohort
            for identity in identities.values()
        ),
        "Runtime/MCP build cohort differs",
    )
    database = args.data_root / "runtime.sqlite"
    cas_root = args.data_root / "runtime.cas"
    require(database.is_file() and cas_root.is_dir(), "D1 Runtime data is unavailable")
    root = SCRIPT_ROOT.parent
    evidence_path = args.evidence if args.evidence.is_absolute() else root / args.evidence
    evidence_path.resolve().relative_to((root / "docs" / "evidence").resolve())

    sqlite_before = sqlite_snapshot(database)
    cas_before = cas_snapshot(cas_root)
    request = diagnostic_request()
    runs: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="forgecad-04be-f-", dir="/tmp") as temporary:
        temporary_root = Path(temporary)
        for index in range(2):
            runtime, ready_path, ready, client = open_client(
                args.mcp,
                args.runtime,
                args.data_root,
                temporary_root / f"run-{index + 1}",
                args.timeout,
            )
            try:
                names = {
                    item.get("name")
                    for item in client.request("tools/list").get("result", {}).get("tools", [])
                    if isinstance(item, dict)
                }
                require(
                    "production_weapon_form_art_failure_diagnostic_get" in names,
                    "04BE-F diagnostic tool is not exposed",
                )
                runs.append(
                    client.tool(
                        "production_weapon_form_art_failure_diagnostic_get", request
                    )
                )
            finally:
                close_client(runtime, ready_path, ready, client)

    sqlite_after = sqlite_snapshot(database)
    cas_after = cas_snapshot(cas_root)
    first, restart = runs
    require(
        first.get("canonical_sha256") == restart.get("canonical_sha256"),
        "04BE-F restart canonical hash differs",
    )
    require(sqlite_before == sqlite_after, "04BE-F read-only diagnostic changed SQLite")
    require(cas_before == cas_after, "04BE-F read-only diagnostic changed CAS")
    require(
        first.get("diagnostic_status")
        == "FAILURE_ROOT_CAUSES_SEPARATED_NO_GEOMETRY_REPAIR_AUTHORIZED"
        and first.get("quality_status") == "QUALITY_TARGET_NOT_MET"
        and first.get("form_quality_v2_status") == "NOT_CREATED"
        and first.get("runtime_write_performed") is False
        and first.get("persistent_user_data_touched") is False
        and first.get("worker_started") is False,
        "04BE-F non-promotion boundary differs",
    )
    require(
        first.get("geometry_delta", {}).get("changed_vertex_count") == 6
        and first.get("geometry_delta", {}).get("y_delta_direction") == "negative",
        "04BE-F exact geometry delta differs",
    )
    diagnoses = {
        row.get("diagnosis_id"): row.get("status")
        for row in first.get("diagnoses", [])
        if isinstance(row, dict)
    }
    require(
        diagnoses.get("rear-three-quarter-owner-attribution")
        == "ATTRIBUTION_CALIBRATION_CONFLICT_GEOMETRY_VOID_OBSERVED_OWNER_REGION_ZERO_VERTICAL_FLIP"
        and diagnoses.get("side-trigger-aperture-visibility")
        == "SEALED_IN_LEFT_RIGHT_OBSERVED_IN_REAR_THREE_QUARTER",
        "04BE-F root-cause separation differs",
    )

    receipt = {
        "schema_version": "ForgeCADProductionWeaponFormArtFailureDiagnosticRealD1Gate@1",
        "task_id": "FPS-FORM-04BE-F",
        "recorded_on": "2026-08-28",
        "status": "PASS_READ_ONLY_FAILURE_ROOT_CAUSES_SEPARATED",
        "build": {
            "build_cohort_sha256": args.expected_build_cohort,
            "mcp_identity": identities["mcp"],
            "runtime_identity": identities["runtime"],
        },
        "mandatory_ponytail_preflight": "PASS_EACH_RUNTIME_SESSION",
        "request": request,
        "diagnostic": first,
        "restart_readback": {
            "diagnostic": restart,
            "canonical_hash_equal": True,
        },
        "read_only_integrity": {
            "sqlite_before": sqlite_before,
            "sqlite_after": sqlite_after,
            "sqlite_unchanged": True,
            "cas_before": cas_before,
            "cas_after": cas_after,
            "cas_unchanged": True,
        },
        "non_promotion_boundary": {
            "quality_status": "QUALITY_TARGET_NOT_MET",
            "form_quality_v2_status": "NOT_CREATED",
            "candidate_confirm_allowed": False,
            "secondary_form_approved": "NOT_CREATED",
            "production_stage_advanced": False,
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
    print(json.dumps(receipt, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateFailure as error:
        print(f"04BE-F probe failed: {error}", file=sys.stderr)
        raise SystemExit(1)
