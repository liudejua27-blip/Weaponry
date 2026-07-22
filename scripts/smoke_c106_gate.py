#!/usr/bin/env python3
"""Fail-closed aggregate gate for the C106 robotic-arm production path.

The production compiler runs once.  The lifecycle wrapper is invoked with its
explicit aggregate-only switch so Version/Snapshot/confirm/restart coverage is
kept without compiling the three candidates a second time.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys
import time
from typing import Any, Callable


ROOT = Path(__file__).resolve().parents[1]
RUST = ROOT / "script" / "with_rust_toolchain.sh"
MANIFEST = ROOT / "apps" / "desktop" / "src-tauri" / "Cargo.toml"
SCHEMA_VERSION = "C106AggregateGate@1"


class GateFailure(RuntimeError):
    pass


FacetValidator = Callable[[str], int | None]


def cargo(*arguments: str) -> list[str]:
    command = [str(RUST), "cargo"]
    if "--" not in arguments:
        return [*command, *arguments, "--manifest-path", str(MANIFEST), "--offline"]
    divider = arguments.index("--")
    return [
        *command,
        *arguments[:divider],
        "--manifest-path",
        str(MANIFEST),
        "--offline",
        "--",
        *arguments[divider + 1 :],
    ]


def require_test_count(expected: int) -> FacetValidator:
    def validate(output: str) -> int | None:
        if f"test result: ok. {expected} passed" not in output:
            raise GateFailure(f"C106_AGGREGATE_TEST_COUNT_INVALID:{expected}")
        return None

    return validate


def parse_last_json(output: str, *, schema_version: str) -> dict[str, Any]:
    for line in reversed(output.splitlines()):
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict) and value.get("schema_version") == schema_version:
            return value
    raise GateFailure(f"C106_AGGREGATE_STRUCTURED_OUTPUT_MISSING:{schema_version}")


def validate_provider_measurement(
    report: dict[str, Any],
    *,
    schema_version: str,
    measurement_source: str,
) -> int:
    if "provider_calls" in report:
        raise GateFailure("C106_AGGREGATE_LEGACY_PROVIDER_COUNT_REJECTED")
    measured = report.get("measured_provider_calls")
    measurement = report.get("provider_measurement")
    if (
        type(measured) is not int
        or measured != 0
        or not isinstance(measurement, dict)
        or measurement.get("schema_version") != schema_version
        or measurement.get("measurement_source") != measurement_source
        or measurement.get("measured_provider_calls") != measured
    ):
        raise GateFailure("C106_AGGREGATE_PROVIDER_MEASUREMENT_INVALID")
    return measured


def validate_production(output: str) -> int:
    report = parse_last_json(output, schema_version="C106RoboticArmProductionGate@1")
    if report.get("status") != "pass":
        raise GateFailure("C106_AGGREGATE_PRODUCTION_REPORT_INVALID")
    measured = validate_provider_measurement(
        report,
        schema_version="C106ProviderCallMeasurement@1",
        measurement_source="execution_node_trace",
    )
    roots = report.get("roots")
    if not isinstance(roots, list) or len(roots) != 3:
        raise GateFailure("C106_AGGREGATE_PRODUCTION_ROOTS_INVALID")
    for root in roots:
        if not isinstance(root, dict):
            raise GateFailure("C106_AGGREGATE_PRODUCTION_ROOTS_INVALID")
        gate_evidence = root.get("gate_evidence")
        if not isinstance(gate_evidence, dict) or not {"m108a", "q003", "g826"} <= set(gate_evidence):
            raise GateFailure("C106_AGGREGATE_SAME_GLB_EVIDENCE_INVALID")
        for gate_id in ("m108a", "q003", "g826"):
            evidence = gate_evidence[gate_id]
            if not isinstance(evidence, dict) or not isinstance(evidence.get("report"), dict):
                raise GateFailure("C106_AGGREGATE_SAME_GLB_EVIDENCE_INVALID")
            validate_provider_measurement(
                evidence["report"],
                schema_version="C106ProviderCallMeasurement@1",
                measurement_source="execution_node_trace",
            )
    return measured


def validate_lifecycle(output: str) -> int:
    report = parse_last_json(output, schema_version="C106RoboticArmLifecycleGate@1")
    if (
        report.get("status") != "pass"
        or report.get("production_gate") != "skipped_by_aggregate"
        or report.get("negative_evidence")
        != [
            "transient_cancel_reject_zero_persistent_writes",
            "in_flight_cancel_discards_late_geometry_and_never_promotes_state",
            "restricted_executor_timeout_crash_and_late_result_tombstone",
        ]
    ):
        raise GateFailure("C106_AGGREGATE_LIFECYCLE_REPORT_INVALID")
    return validate_provider_measurement(
        report,
        schema_version="C106LifecycleMeasuredEvidence@1",
        measurement_source="FakeDeepSeekClient.records",
    )


def facets() -> list[tuple[str, list[str], FacetValidator | None]]:
    return [
        (
            "core_recipe_contracts",
            cargo(
                "test",
                "-p",
                "forgecad-core",
                "--test",
                "c106_robotic_arm_recipes",
            ),
            require_test_count(4),
        ),
        (
            "app_server_style_selection",
            cargo(
                "test",
                "-p",
                "forgecad-app-server",
                "c106_arm_style_routes_one_deterministic_root_without_changing_c105_catalogs",
            ),
            require_test_count(1),
        ),
        (
            "production_same_glb_m108a_q003_g826_spatial",
            ["npm", "run", "agent:c106-robotic-arm-production-gate"],
            validate_production,
        ),
        (
            "lifecycle_single_generation_cancel",
            ["npm", "run", "agent:c106-robotic-arm-lifecycle-gate", "--", "--skip-production"],
            validate_lifecycle,
        ),
        (
            "a005_surface_adornment",
            ["npm", "run", "agent:a005-surface-adornment-pbr-smoke"],
            None,
        ),
        (
            "part_role_labels",
            ["npm", "run", "desktop:c101-part-role-labels-smoke"],
            None,
        ),
        ("desktop_typecheck", ["npm", "run", "desktop:typecheck"], None),
        ("contract_types", ["npm", "run", "contracts:types:check"], None),
        (
            "provider_runtime_boundary",
            ["npm", "run", "agent:p0-provider-runtime-boundary-smoke"],
            None,
        ),
        ("secrets_boundary", ["npm", "run", "release:secrets-files"], None),
        ("safety_scope", ["npm", "run", "release:safety-scope"], None),
    ]


def run_facet(name: str, command: list[str], validator: FacetValidator | None) -> dict[str, Any]:
    started = time.monotonic()
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            capture_output=True,
            timeout=1_200,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise GateFailure(f"C106_AGGREGATE_{name}_UNAVAILABLE") from exc
    elapsed_ms = round((time.monotonic() - started) * 1_000)
    if completed.returncode != 0:
        detail = (completed.stdout + "\n" + completed.stderr).strip()[-1_800:]
        raise GateFailure(f"C106_AGGREGATE_{name}_FAILED:{detail}")
    measured_provider_calls = validator(completed.stdout) if validator is not None else None
    report: dict[str, Any] = {"id": name, "status": "pass", "elapsed_ms": elapsed_ms}
    if measured_provider_calls is not None:
        report["measured_provider_calls"] = measured_provider_calls
    return report


def self_test() -> int:
    expected = [
        "core_recipe_contracts",
        "app_server_style_selection",
        "production_same_glb_m108a_q003_g826_spatial",
        "lifecycle_single_generation_cancel",
        "a005_surface_adornment",
        "part_role_labels",
        "desktop_typecheck",
        "contract_types",
        "provider_runtime_boundary",
        "secrets_boundary",
        "safety_scope",
    ]
    configured = facets()
    if [name for name, _, _ in configured] != expected:
        raise GateFailure("C106_AGGREGATE_ORDER_INVALID")
    production_calls = sum(
        command == ["npm", "run", "agent:c106-robotic-arm-production-gate"]
        for _, command, _ in configured
    )
    lifecycle = configured[3][1]
    if production_calls != 1 or lifecycle[-1:] != ["--skip-production"]:
        raise GateFailure("C106_AGGREGATE_PRODUCTION_DUPLICATED")
    for invalid in (
        {"provider_calls": 0},
        {"measured_provider_calls": 0},
        {
            "measured_provider_calls": 0,
            "provider_measurement": {
                "schema_version": "C106ProviderCallMeasurement@1",
                "measurement_source": "unmeasured_constant",
                "measured_provider_calls": 0,
            },
        },
    ):
        try:
            validate_provider_measurement(
                invalid,
                schema_version="C106ProviderCallMeasurement@1",
                measurement_source="execution_node_trace",
            )
        except GateFailure:
            pass
        else:
            raise GateFailure("C106_AGGREGATE_UNMEASURED_PROVIDER_COUNT_ACCEPTED")
    print(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "status": "pass",
                "mode": "self_test",
                "facet_ids": expected,
                "formal_eligible": False,
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


def main(*, self_test_only: bool = False) -> int:
    if self_test_only:
        return self_test()
    started = time.monotonic()
    reports: list[dict[str, Any]] = []
    for name, command, validator in facets():
        try:
            reports.append(run_facet(name, command, validator))
        except GateFailure as exc:
            reports.append({"id": name, "status": "fail"})
            print(
                json.dumps(
                    {
                        "schema_version": SCHEMA_VERSION,
                        "status": "fail",
                        "error": str(exc),
                        "elapsed_ms": round((time.monotonic() - started) * 1_000),
                        "facets": reports,
                        "formal_eligible": False,
                    },
                    ensure_ascii=False,
                    sort_keys=True,
                ),
                file=sys.stderr,
            )
            return 1
    measured_facets = [
        report
        for report in reports
        if report["id"]
        in {"production_same_glb_m108a_q003_g826_spatial", "lifecycle_single_generation_cancel"}
    ]
    if len(measured_facets) != 2 or any(
        type(report.get("measured_provider_calls")) is not int for report in measured_facets
    ):
        raise GateFailure("C106_AGGREGATE_PROVIDER_MEASUREMENT_INCOMPLETE")
    measured_provider_calls = sum(report["measured_provider_calls"] for report in measured_facets)
    print(
        json.dumps(
            {
                "schema_version": SCHEMA_VERSION,
                "status": "pass",
                "elapsed_ms": round((time.monotonic() - started) * 1_000),
                "facets": reports,
                "measured_provider_calls": measured_provider_calls,
                "provider_measurement_facets": [report["id"] for report in measured_facets],
                "formal_eligible": False,
            },
            ensure_ascii=False,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run the C106 aggregate gate.")
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    try:
        raise SystemExit(main(self_test_only=arguments.self_test))
    except GateFailure as error:
        print(
            json.dumps(
                {
                    "schema_version": SCHEMA_VERSION,
                    "status": "fail",
                    "error": str(error),
                    "formal_eligible": False,
                },
                ensure_ascii=False,
                sort_keys=True,
            ),
            file=sys.stderr,
        )
        raise SystemExit(1) from None
