#!/usr/bin/env python3
"""Self-tests for the isolated ForgeCAD Rust Core gate runner.

These tests deliberately exercise the runner's command/report boundary rather
than the Rust product behavior. They use injected command runners, so they do
not start Cargo, Python, Tauri, WebView, or any other workspace gate.
"""

from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).with_name("forgecad_gate_rust_core.py")
SPEC = importlib.util.spec_from_file_location("forgecad_gate_rust_core", SCRIPT)
assert SPEC and SPEC.loader
gate = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = gate
SPEC.loader.exec_module(gate)


class ForgeCADRustCoreGateRunnerTests(unittest.TestCase):
    def test_parser_uses_first_structured_reason_and_bounds_diagnostics(self) -> None:
        long_reason = "x" * (gate.MAX_DIAGNOSTIC_CHARS + 100)
        output = "\n".join(
            [
                "ordinary compiler context",
                '{"error_code":"FIRST_FAILURE","message":"%s","body":"secret body"}'
                % long_reason,
                '{"error_code":"SECOND_FAILURE","message":"should not win"}',
            ]
        )

        reason = gate.first_structured_reason(output)

        self.assertEqual(reason["stable_error_code"], "FIRST_FAILURE")
        self.assertLessEqual(len(reason["message"]), gate.MAX_DIAGNOSTIC_CHARS)
        self.assertNotIn("secret body", str(reason))

    def test_timeout_is_stable_and_does_not_leak_output(self) -> None:
        def timeout_runner(*_args: object, **_kwargs: object) -> subprocess.CompletedProcess[str]:
            raise subprocess.TimeoutExpired(
                cmd=["cargo", "test"],
                timeout=1,
                output='{"body":"hidden"}',
                stderr='{"error_code":"LATE"}',
            )

        result = gate.run_command(
            ["cargo", "test", "-p", "forgecad-core", "--offline"],
            timeout_seconds=1,
            runner=timeout_runner,
        )

        self.assertEqual(result.status, "timeout")
        self.assertEqual(result.stable_error_code, "COMMAND_TIMEOUT")
        self.assertNotIn("hidden", str(result.to_report()))

    def test_missing_tool_report_has_stable_exit_code(self) -> None:
        with patch.object(gate, "wrapper_cargo_available", return_value=False), patch.object(
            gate.os, "access", return_value=True
        ):
            failure = gate.preflight(
                wrapper=Path(__file__).resolve(),
                manifest=Path(__file__).resolve(),
            )

        self.assertIsNotNone(failure)
        assert failure is not None
        self.assertEqual(failure.stable_error_code, "MISSING_TOOL")
        self.assertEqual(failure.exit_code, gate.EXIT_MISSING_TOOL)

    def test_failed_facet_stops_remaining_facets_and_preserves_first_reason(self) -> None:
        calls: list[list[str]] = []

        def fake_runner(command: list[str], **_kwargs: object) -> gate.CommandResult:
            calls.append(command)
            return gate.CommandResult(
                status="failed",
                returncode=1,
                stdout='{"error_code":"RUST_ASSERTION","message":"first reason"}',
                stderr="",
                duration_ms=4,
            )

        report = gate.execute_facets(
            [
                gate.FacetSpec("first", "core", ["cargo", "test", "first"]),
                gate.FacetSpec("second", "core", ["cargo", "test", "second"]),
            ],
            command_runner=fake_runner,
        )

        self.assertEqual(len(calls), 1)
        self.assertEqual(report["status"], "fail")
        self.assertEqual(report["exit_code"], gate.EXIT_FACET_FAILED)
        self.assertEqual(report["stable_error_code"], "RUST_ASSERTION")
        self.assertEqual(report["facets"][0]["status"], "fail")
        self.assertEqual(report["facets"][1]["status"], "not_run")
        self.assertEqual(
            report["facets"][1]["stable_error_code"], "NOT_RUN_AFTER_FAILURE"
        )

    def test_success_report_is_stable_json_shape(self) -> None:
        def fake_runner(command: list[str], **_kwargs: object) -> gate.CommandResult:
            return gate.CommandResult(
                status="pass",
                returncode=0,
                stdout="",
                stderr="",
                duration_ms=3,
            )

        report = gate.execute_facets(
            [gate.FacetSpec("core", "forgecad-core", ["cargo", "test"])],
            command_runner=fake_runner,
        )

        self.assertEqual(report["schema_version"], "ForgeCADRustCoreGateReport@1")
        self.assertEqual(report["phase"], "gate")
        self.assertEqual(report["subsystem"], "forgecad-core")
        self.assertEqual(report["status"], "pass")
        self.assertEqual(report["exit_code"], gate.EXIT_PASS)
        self.assertIsNone(report["stable_error_code"])
        self.assertEqual(report["facets"][0]["command"], ["cargo", "test"])

    def test_commands_cannot_recurse_into_other_gate_layers(self) -> None:
        for facet in gate.GATE_FACETS:
            with self.subTest(facet=facet.facet_id):
                gate.validate_core_only_command(facet.command)

        with self.assertRaises(gate.InvalidCommand):
            gate.validate_core_only_command(["npm", "run", "k002:code-gate"])
        with self.assertRaises(gate.InvalidCommand):
            gate.validate_core_only_command(["python", "scripts/smoke_k003_rust_core.py"])
        with self.assertRaises(gate.InvalidCommand):
            gate.validate_core_only_command(["cargo", "test", "-p", "wushen-forge-desktop"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
