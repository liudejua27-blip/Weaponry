#!/usr/bin/env python3
"""Validate committed G824C macOS packaged-candidate evidence."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
report = json.loads((ROOT / "evaluations/csg-g824c/report.json").read_text(encoding="utf-8"))
assert report["schema_version"] == "ForgeCADCSGPackagedCandidateEvidence@1"
python = report["python_candidate"]
budget = report["budget"]
assert python["mach_o_arm64"] is True
assert python["archive_contains_manifold3d"] is True
assert python["archive_contains_numpy"] is True
assert python["runtime_hook_import_and_health_verified"] is True
assert python["provider_calls"] == 0
assert python["candidate_sidecar_bytes"] <= budget["max_total_bytes"]
assert python["increment_bytes"] <= budget["max_increment_bytes"]
assert python["cold_start_regression_ms"] <= budget["max_cold_start_regression_ms"]
assert python["peak_rss_kib"] <= budget["max_peak_rss_kib"]
assert python["budget_passed"] is True
assert {item["name"] for item in python["licenses"]} == {"manifold3d", "numpy"}
assert all(item["license_files"] for item in python["licenses"])
assert report["wasm_candidate"]["current_python_sidecar_execution_host"] is False
assert report["recommendation"]["candidate"] == "manifold_python"
assert report["recommendation"]["status"] == "recommended_pending_windows_runtime"
assert report["recommendation"]["production_dependency_added"] is False
assert len(report["recommendation"]["remaining_blockers"]) == 2
package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
assert "manifold-3d" not in package.get("dependencies", {})
assert "manifold-3d" not in package.get("devDependencies", {})
assert "manifold3d" not in (ROOT / "apps/agent/pyproject.toml").read_text(encoding="utf-8")
print("G824C packaged candidate passed: Python fits the macOS budget and is recommended; Windows runtime and final ADR remain blocked")
