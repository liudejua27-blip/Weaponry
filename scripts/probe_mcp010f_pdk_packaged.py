#!/usr/bin/env python3
"""Exercise the packaged read-only ParametricDesignKit branch in isolation."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
import time
from pathlib import Path
from typing import Any

from probe_mcp010a_dev_app import (
    DEFAULT_APP,
    McpClient,
    shutdown_isolated_runtime,
    verify_app,
    write_receipt,
)


ROOT = Path(__file__).resolve().parents[1]


def canonical_hash(value: Any) -> str:
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, default=DEFAULT_APP)
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--timeout", type=float, default=20.0)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    manifest, paths = verify_app(args.app)
    cohort = str(manifest["build_cohort_sha256"])
    environment = os.environ.copy()
    for key in (
        "FORGECAD_RUNTIME_COMMAND",
        "FORGECAD_RUNTIME_SOCKET",
        "FORGECAD_RUNTIME_TOKEN",
        "FORGECAD_RUNTIME_READY_FILE",
        "FORGECAD_RUNTIME_STATUS_FILE",
    ):
        environment.pop(key, None)
    environment["FORGECAD_MCP_ENABLE_MCP004_WRITES"] = "1"

    with tempfile.TemporaryDirectory(prefix="forgecad-mcp010f-pdk-") as temporary:
        runtime_data = Path(temporary) / "runtime-data"
        environment["FORGECAD_RUNTIME_DATA_DIR"] = str(runtime_data)
        client = McpClient(paths["forgecad-mcp"], environment, args.timeout)
        try:
            initialized = client.request(
                "initialize",
                {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "forgecad-mcp010f-pdk-probe", "version": "1"},
                },
            )
            if initialized.get("result", {}).get("protocolVersion") != "2025-06-18":
                raise SystemExit("packaged PDK probe initialize failed")
            client.notify("notifications/initialized")

            state = "Starting"
            deadline = time.monotonic() + args.timeout
            while time.monotonic() < deadline:
                status = client.tool("runtime_status")
                state = status.get("state") if isinstance(status, dict) else None
                if state == "Ready":
                    break
                if state in {"Degraded", "Busy"}:
                    raise SystemExit(f"packaged PDK sibling Runtime failed: {state}")
                time.sleep(0.1)
            if state != "Ready":
                raise SystemExit("packaged PDK sibling Runtime did not become Ready")

            capabilities = client.tool("capabilities_get")
            if not isinstance(capabilities, dict) or capabilities.get("build_cohort_match") is not True:
                raise SystemExit("packaged PDK cohort binding failed")
            if capabilities.get("build_cohort_sha256") != cohort:
                raise SystemExit("packaged PDK Runtime cohort differs from app manifest")
            if capabilities.get("mcp_build_cohort_sha256") != cohort:
                raise SystemExit("packaged PDK MCP cohort differs from app manifest")

            preflight = client.tool(
                "skill_get", {"skill_id": "ponytail-preflight", "version": "0.1.0"}
            )
            if not isinstance(preflight, dict):
                raise SystemExit("packaged PDK preflight result was not an object")
            if not (
                isinstance(preflight.get("skill"), dict)
                and preflight["skill"].get("skill_id") == "ponytail-preflight"
                and preflight["skill"].get("version") == "0.1.0"
                and isinstance(preflight["skill"].get("canonical_sha256"), str)
                and len(preflight["skill"]["canonical_sha256"]) == 64
                and isinstance(preflight.get("knowledge"), dict)
                and isinstance(preflight["knowledge"].get("canonical_sha256"), str)
                and len(preflight["knowledge"]["canonical_sha256"]) == 64
            ):
                raise SystemExit("packaged PDK preflight was not verified")

            project = client.tool(
                "project_create",
                {"name": "MCP010F packaged PDK probe", "policy": {"profile": "mvp"}},
            )
            project_id = project.get("project_id") if isinstance(project, dict) else None
            if not isinstance(project_id, str) or not project_id:
                raise SystemExit("packaged PDK probe project was not created")

            request: dict[str, Any] = {
                "schema_version": "ParametricDesignKitRequest@1",
                "project_id": project_id,
                "representation_plan_sha256": "a" * 64,
                "kit_id": "forgecad.kit.sensor@1",
                "part_id": "sensor",
                "material_zone_id": "zone-black-mechanical",
                "intent": {
                    "radius_m": 0.12,
                    "height_m": 0.32,
                    "radial_segments": 16,
                    "position_m": [0.0, 2.0, 0.0],
                    "rotation_rad": [0.0, 0.0, 0.0],
                },
                "input_sha256": "",
            }
            input_binding = dict(request)
            input_binding.pop("input_sha256")
            request["input_sha256"] = canonical_hash(input_binding)
            result = client.tool("geometry_program_hash", request)
            if not isinstance(result, dict):
                raise SystemExit("packaged PDK result was not an object")
            if result.get("schema_version") != "ParametricDesignKitProgram@1":
                raise SystemExit("packaged PDK result schema drifted")
            if result.get("kit_id") != "forgecad.kit.sensor@1":
                raise SystemExit("packaged PDK kit binding drifted")
            geometry = result.get("geometry_program")
            if not isinstance(geometry, dict) or geometry.get("nodes", [{}])[0].get("operator_id") != "forgecad.geometry.primitive@2":
                raise SystemExit("packaged PDK operator binding drifted")
        finally:
            try:
                shutdown_isolated_runtime(runtime_data)
            finally:
                client.close()

    receipt = {
        "schema_version": "ForgeCADMCP010FPackagedParametricDesignKitProbe@1",
        "task_id": "FGC-MCP010F",
        "status": "PASS",
        "build_cohort_sha256": cohort,
        "runtime_state": "Ready",
        "ponytail_preflight": "PASS",
        "parametric_design_kit_read_only_round_trip": "PASS",
        "kit_id": "forgecad.kit.sensor@1",
        "operator_id": "forgecad.geometry.primitive@2",
        "candidate_created": False,
        "persistent_user_data_touched": False,
        "codex_desktop_restart_gate": "NOT_RUN",
    }
    write_receipt(args.evidence, receipt)
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
