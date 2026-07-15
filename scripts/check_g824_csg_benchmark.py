#!/usr/bin/env python3
"""Validate the committed G824 report and ensure candidates stayed isolated."""

from __future__ import annotations

import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
report = json.loads((ROOT / "evaluations/csg-g824/report.json").read_text(encoding="utf-8"))
assert report["schema_version"] == "ForgeCADCSGBenchmark@1"
assert [item["adapter"] for item in report["candidates"]] == [
    "forgecad_restricted_current",
    "manifold_python",
    "manifold_wasm",
]
assert report["decision"]["selected"] is None
assert report["decision"]["status"] == "no_candidate_meets_all_gates"
for candidate in report["candidates"][1:]:
    assert candidate["deterministic_identical_fixture"] is True
    assert candidate["coplanar_completed"] is True
    assert candidate["near_degenerate_completed"] is True
    assert candidate["material_surface_provenance_verified"] is False
    assert candidate["cancellation_verified"] is False
    assert candidate["windows_runtime_executed"] is False
    assert len(candidate["fixture_results"]) == 4
package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
assert "manifold-3d" not in package.get("dependencies", {})
assert "manifold-3d" not in package.get("devDependencies", {})
pyproject = (ROOT / "apps/agent/pyproject.toml").read_text(encoding="utf-8")
assert "manifold3d" not in pyproject
print("G824 CSG benchmark report passed: candidates are comparable, rejected, and absent from production dependencies")
